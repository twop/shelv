use std::rc::Rc;

use boa_engine::{context::HostHooks, Context, Source};
use boa_runtime::Console;
use itertools::Itertools;
use similar::DiffableStr;
use smallvec::SmallVec;

use crate::{
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    text_structure::{CodeBlockMeta, SpanIndex, SpanKind, SpanMeta, TextStructure},
};

use super::{
    js_console_logger::JsLogCollector,
    note_eval_context::{BlockEvalResult, SourceHash},
};

#[derive(Debug, PartialEq, Clone, Copy)]
enum CodeBlock {
    Source,
    Output,
}

pub const JS_OUTPUT_LANG: &str = "js#";
pub const JS_SOURCE_LANG: &str = "js";

pub struct JsEvaluator {
    context: Context,
    console_logger: JsLogCollector,
}

pub struct HostWithLocalTimezone;

impl HostHooks for HostWithLocalTimezone {
    fn local_timezone_offset_seconds(&self, _: i64) -> i32 {
        let local = chrono::Local::now();
        // let utc = chrono::Utc::now();
        let offset = local.offset().local_minus_utc();
        offset
    }
}

impl JsEvaluator {
    pub fn new() -> Self {
        let console_logger = JsLogCollector::new();
        let mut context = Context::builder()
            .host_hooks(&HostWithLocalTimezone)
            .build()
            .unwrap();

        let console = Console::init_with_logger(&mut context, console_logger.clone());
        context
            .register_global_property(
                Console::NAME,
                console,
                boa_engine::property::Attribute::all(),
            )
            .expect("the console builtin shouldn't exist");

        Self {
            context,
            console_logger,
        }
    }

    pub fn try_parse_block_lang(lang: &str) -> Option<(bool, Option<SourceHash>)> {
        match lang {
            "js" => Some((true, None)), // Source block

            output if output.starts_with(JS_OUTPUT_LANG) => {
                let hex_str = &output[JS_OUTPUT_LANG.len()..];
                Some((false, SourceHash::parse(hex_str))) // Output block with hash
            }

            _ => None,
        }
    }

    pub fn eval_block(&mut self, body: &str, hash: SourceHash) -> BlockEvalResult {
        let result = self.context.eval(Source::from_bytes(body));
        let logged = self.console_logger.flush().ok();

        BlockEvalResult {
            body: match result {
                Ok(boa_engine::JsValue::Undefined) if logged.is_some() => {
                    logged.unwrap_or_default()
                }

                Ok(res) => format!(
                    "{}{}",
                    logged.unwrap_or_default(),
                    res.display().to_string()
                ),

                Err(err) => format!("{}{:#}", logged.unwrap_or_default(), err),
            },

            output_lang: format!("{}{}", JS_OUTPUT_LANG, hash.to_string()),
        }
    }
}

// Evaluate a single JavaScript block
pub fn evaluate_js_block(
    span_index: SpanIndex,
    text_structure: &TextStructure,
    text: &str,
) -> Option<Vec<TextChange>> {
    let mut evaluator = JsEvaluator::new();

    let Some((desc, SpanMeta::CodeBlock(code_meta))) =
        text_structure.get_span_with_meta(span_index)
    else {
        return None;
    };

    if code_meta.lang != JS_SOURCE_LANG {
        return None;
    }

    let (_, code_text_desc) = text_structure
        .iterate_immediate_children_of(span_index)
        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

    let code = &text[code_text_desc.byte_pos.range()];
    if code.trim().is_empty() {
        return None;
    }

    let eval_result = evaluator.eval_block(code, SourceHash::from(code));
    let output_block = print_output_block(eval_result);

    let output_block_range = find_js_output_block_right_after(text_structure, span_index);

    let mut changes = SmallVec::<[TextChange; 1]>::new();

    if let Some(range) = output_block_range {
        // Replace the existing output block
        changes.push(TextChange::Insert(range, output_block));
    } else {
        // Insert a new output block
        changes.push(TextChange::Insert(
            ByteSpan::point(desc.byte_pos.end),
            "\n".to_string() + &output_block,
        ));
    }

    if changes.is_empty() {
        None
    } else {
        Some(changes.into_vec())
    }
}

fn find_js_output_block_right_after(
    text_structure: &TextStructure,
    span_index: SpanIndex,
) -> Option<ByteSpan> {
    text_structure
        .filter_map_codeblocks(|lang| match lang {
            JS_SOURCE_LANG => Some(CodeBlock::Source),
            lang if lang.starts_with(JS_OUTPUT_LANG) => Some(CodeBlock::Output),
            _ => None,
        })
        .tuple_windows()
        .find_map(
            |((first_index, _, _, first_type), (_, second_desc, _, second_type))| {
                (first_index == span_index
                    && first_type == CodeBlock::Source
                    && second_type == CodeBlock::Output)
                    .then(|| second_desc.byte_pos)
            },
        )
}

fn print_output_block(eval_result: BlockEvalResult) -> String {
    format!("```{}\n{}\n```", eval_result.output_lang, eval_result.body)
}

#[cfg(test)]
mod tests {

    use crate::effects::text_change_effect::apply_text_changes;

    use super::*;
    #[test]
    pub fn test_evaluating_js_blocks() {
        let test_cases = [
            (
                "## computes an output for a standalone jsblock ##",
                r#"
```js
'hello world' + '!'
```{||}
"#,
                Some(
                    r#"
```js
'hello world' + '!'
```
```js#da0b
"hello world!"
```{||}
"#,
                ),
            ),
            // ________________________________________________
            (
                "## overrides a block if hashes don't match ##",
                r#"
```js
'hello world' + '!'
```{||}
```js#aaa
I will be overwritten
```
"#,
                Some(
                    r#"
```js
'hello world' + '!'
```{||}
```js#da0b
"hello world!"
```
"#,
                ),
            ),
            // ________________________________________________
            (
                "## and it doesn't override output block if hashes match ##",
                r#"
```js
'hello world' + '!'
```{||}
```js#da0b
I should be overwritten, but I won't
```
"#,
                None,
            ),
            // ________________________________________________
            (
                "## replaces the content of the output block if cache doesn't match or doesn't parse ##",
                r#"
```js
2 + 2
```{||}
```js#oops
1
```
"#,
                Some(
                    r#"
```js
2 + 2
```{||}
```js#2cd1
4
```
"#,
                ),
            ),
            // ________________________________________________
            (
                "## doesn't remove orhpant output blocks ##",
                r#"
```#dfgh
1
```
```js
2 + 2
```{||}
```js#2
3
```
"#,
                Some(
                    r#"
```#dfgh
1
```
```js
2 + 2
```{||}
```js#2cd1
4
```
"#,
                ),
            ),
            // ________________________________________________
            (
                "## prints an error ##",
                r#"
```js
throw new Error("yo!")
```{||}
"#,
                Some(
                    r#"
```js
throw new Error("yo!")
```
```js#b511
Error: yo!
```{||}
"#,
                ),
            ),
            // ________________________________________________
            (
                "## if we identified a missing output (for example when you copy paste blocks)
                then we
                ##",
                r#"
```js
1
```
```js#4422
1
```

```js
1 + 1
```
--- start: this is copy pasted ---
```js
45{||}
```
--- end: this is copy pasted ---

```js#c9f6
2
```
"#,
                Some(
                    r#"
```js
1
```
```js#4422
1
```

```js
1 + 1
```
```js#c9f6
2
```
--- start: this is copy pasted ---
```js
45{||}
```
--- end: this is copy pasted ---

```js#a4a
45
```
"#,
                ),
            ),
        ];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            // Find the JavaScript block index
            let js_block_index = structure
                .filter_map_codeblocks(|lang| (lang == JS_SOURCE_LANG).then_some(()))
                .next()
                .map(|(index, _, _, _)| index);

            let changes =
                js_block_index.and_then(|index| evaluate_js_block(index, &structure, &text));

            match (changes, expected_output) {
                (Some(changes), Some(expected_output)) => {
                    let cursor =
                        apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();
                    let res = TextChange::encode_cursor(&text, cursor.unwrap());
                    println!("res:\n{res:#}\n\nexpected:{expected_output:#}");
                    assert_eq!(res, expected_output, "test case: \n{}\n", desc)
                }
                (None, None) => (),
                (changes, expected) => assert!(
                    false,
                    "\n{:?}\nexpected:\n{:?}\n\nbut got this changes:\n{:?}\n",
                    desc, expected, changes
                ),
            }
        }
    }
}

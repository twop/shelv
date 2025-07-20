use boa_engine::{context::HostHooks, Context, Source};
use boa_runtime::Console;
use smallvec::SmallVec;

use crate::{
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    text_structure::{SpanIndex, SpanKind, SpanMeta, TextStructure},
};

use super::{
    js_console_logger::JsLogCollector,
    note_eval_context::{BlockEvalResult, BlockId, SourceHash},
};

#[derive(Debug, PartialEq)]
pub enum JSBlockLang {
    Source(Option<BlockId>),
    Output(BlockId, SourceHash),
}

impl JSBlockLang {
    /// Parses JavaScript block language parameters
    ///
    /// # Requirements
    /// - source can be either just "js" or "js <n>" where n is a BlockId
    /// - output has to be in the form of "js <n> > #<source_hash>"
    /// - None in all other cases
    ///
    /// # Examples
    /// - "js" -> Some(Source(None))
    /// - "js 5" -> Some(Source(Some(BlockId(5))))
    /// - "js 5 > #bc" -> Some(Output(BlockId(5), SourceHash))
    pub fn parse(lang: &str) -> Option<JSBlockLang> {
        if lang == "js" {
            return Some(JSBlockLang::Source(None));
        }

        if let Some(rest) = lang.strip_prefix("js ") {
            if let Some((id_part, hash_part)) = rest.split_once(" > #") {
                // Output format: "js <n> > #<hash>"
                if let Ok(id) = id_part.parse::<u32>() {
                    let hash = SourceHash::parse(hash_part)?;
                    return Some(JSBlockLang::Output(BlockId(id), hash));
                }
            } else {
                // Source format: "js <n>"
                if let Ok(id) = rest.parse::<u32>() {
                    return Some(JSBlockLang::Source(Some(BlockId(id))));
                }
            }
        }

        None
    }

    fn source_lang_with_id(id: BlockId) -> String {
        format!("js {}", id.to_string())
    }

    fn output_lang(id: BlockId, hash: SourceHash) -> String {
        format!("js {} > #{}", id.to_string(), hash.to_string())
    }
}

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

    pub fn eval_block(&mut self, body: &str, id: BlockId, hash: SourceHash) -> BlockEvalResult {
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

            output_lang: JSBlockLang::output_lang(id, hash),
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

    let (_, code_text_desc) = text_structure
        .iterate_immediate_children_of(span_index)
        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

    let code = &text[code_text_desc.byte_pos.range()];
    if code.trim().is_empty() {
        return None;
    }

    let mut changes = SmallVec::<[TextChange; 1]>::new();

    // Check if this is a source block that needs ID assignment
    let (block_id, source_change) = match JSBlockLang::parse(&code_meta.lang) {
        Some(JSBlockLang::Source(Some(existing_id))) => (existing_id, None),
        Some(JSBlockLang::Source(None)) => {
            let next_id = find_next_available_block_id(text_structure);

            (
                next_id,
                Some(TextChange::Insert(
                    code_meta.lang_byte_span,
                    JSBlockLang::source_lang_with_id(next_id),
                )),
            )
        }
        _ => return None,
    };

    if let Some(source_lang_change) = source_change {
        changes.push(source_lang_change);
    }

    let hash = SourceHash::from(code);
    let eval_result = evaluator.eval_block(code, block_id, hash);
    let output_block = print_output_block(eval_result);

    let output_block_range = find_js_output_block_by_id(text_structure, block_id);

    if let Some((range, _hash)) = output_block_range {
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

fn find_next_available_block_id(text_structure: &TextStructure) -> BlockId {
    let mut max_id = 0u32;

    // Find the highest existing block ID
    text_structure
        .filter_map_codeblocks(|lang| match JSBlockLang::parse(lang) {
            Some(JSBlockLang::Source(block_id)) => block_id,
            _ => None,
        })
        .for_each(|(_, _, _, block_id)| {
            max_id = max_id.max(block_id.0);
        });

    BlockId(max_id + 1)
}

fn find_js_output_block_by_id(
    text_structure: &TextStructure,
    target_id: BlockId,
) -> Option<(ByteSpan, SourceHash)> {
    text_structure
        .filter_map_codeblocks(|lang| match JSBlockLang::parse(lang) {
            Some(JSBlockLang::Output(output_block_id, hash)) if output_block_id == target_id => {
                Some(hash)
            }
            _ => None,
        })
        .next()
        .map(|(_, desc, _, hash)| (desc.byte_pos, hash))
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
```js 1
'hello world' + '!'
```
```js 1 > #da0b
"hello world!"
```{||}
"#,
                ),
            ),
            // ________________________________________________
            (
                "## overrides a block if hashes don't match ##",
                r#"
```js 2
'hello world' + '!'
```{||}
```js 2 > #aaa
I will be overwritten
```
"#,
                Some(
                    r#"
```js 2
'hello world' + '!'
```{||}
```js 2 > #da0b
"hello world!"
```
"#,
                ),
            ),
            // ________________________________________________
            (
                "## and it doesn't override output block if hashes match ##",
                r#"
```js 1
'hello world' + '!'
```{||}
```js 1 > #da0b
I should be overwritten, but I won't
```
"#,
                None,
            ),
            // ________________________________________________
            (
                "## replaces the content of the output block if cache doesn't match or doesn't parse ##",
                r#"
```js 5
2 + 2
```{||}
```js 5 > #oops
1
```
"#,
                Some(
                    r#"
```js 5
2 + 2
```{||}
```js 5 > 2cd1
4
```
"#,
                ),
            ),
            // ________________________________________________
            (
                "## doesn't remove orhpant output blocks ##",
                r#"
```js 10 > #dfgh
1
```
```js 5
2 + 2
```{||}
```js 5 > #2
3
```
"#,
                Some(
                    r#"
```js 10 > #dfgh
1
```
```js 5
2 + 2
```{||}
```js 5 > #2cd1
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
```js 1
throw new Error("yo!")
```
```js 1 > #b511
Error: yo!
```{||}
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
                .filter_map_codeblocks(|lang| match JSBlockLang::parse(lang) {
                    Some(JSBlockLang::Source(_)) => Some(()),
                    _ => None,
                })
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

    #[test]
    fn test_javascript_block_language_parse() {
        use super::JSBlockLang;
        use crate::scripting::note_eval_context::{BlockId, SourceHash};

        // Test "js" - source without ID
        assert_eq!(JSBlockLang::parse("js"), Some(JSBlockLang::Source(None)));

        // Test "js 5" - source with ID
        assert_eq!(
            JSBlockLang::parse("js 5"),
            Some(JSBlockLang::Source(Some(BlockId(5))))
        );

        // Test "js 5 > #bc" - output with ID and hash
        let hash = SourceHash::parse("bc").unwrap();
        assert_eq!(
            JSBlockLang::parse("js 5 > #bc"),
            Some(JSBlockLang::Output(BlockId(5), hash))
        );

        // Test invalid cases
        assert_eq!(JSBlockLang::parse(""), None);
        assert_eq!(JSBlockLang::parse("python"), None);
        assert_eq!(JSBlockLang::parse("js abc"), None);
        assert_eq!(JSBlockLang::parse("js 5 > #"), None);
        assert_eq!(JSBlockLang::parse("js 5 >"), None);
        assert_eq!(JSBlockLang::parse("js > #bc"), None);
        assert_eq!(JSBlockLang::parse("javascript"), None);
    }
}

// Evaluate all live JavaScript blocks (blocks with IDs) in a text structure
pub fn evaluate_all_live_js_blocks(
    text_structure: &TextStructure,
    text: &str,
) -> Option<Vec<TextChange>> {
    let mut evaluator = JsEvaluator::new();
    let mut all_changes = Vec::new();

    let live_blocks = text_structure
        .filter_map_codeblocks(|lang| match JSBlockLang::parse(lang) {
            Some(JSBlockLang::Source(Some(source_block_id))) => Some(source_block_id),
            _ => None,
        })
        .map(|(span_index, _, _, block_id)| (span_index, block_id));

    for (span_index, block_id) in live_blocks {
        // Get the code for this block
        let (_, code_text_desc) = text_structure
            .iterate_immediate_children_of(span_index)
            .find(|(_, desc)| desc.kind == SpanKind::Text)?;

        let code = &text[code_text_desc.byte_pos.range()];
        if code.trim().is_empty() {
            continue;
        }

        let source_hash = SourceHash::from(code);

        let output_block_range = find_js_output_block_by_id(text_structure, block_id);
        let needs_update = if let Some((_output_range, existing_output_hash)) = output_block_range {
            existing_output_hash != source_hash
        } else {
            // No output block found, skip evaluation (no outlet for output for this source)
            continue;
        };

        if needs_update {
            let eval_result = evaluator.eval_block(code, block_id, source_hash);
            let output_block = print_output_block(eval_result);

            if let Some((range, _hash)) = output_block_range {
                all_changes.push(TextChange::Insert(range, output_block));
            }
        }
    }

    if all_changes.is_empty() {
        None
    } else {
        Some(all_changes)
    }
}

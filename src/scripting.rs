use boa_engine::{Context, Source};
use smallvec::SmallVec;
use std::{
    fmt::format,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    text_structure::{SpanIndex, SpanKind, TextStructure},
};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SourceHash(u16);

impl SourceHash {
    pub fn parse(hex: &str) -> Option<Self> {
        u16::from_str_radix(hex, 16).ok().map(SourceHash)
    }

    pub fn from(code: &str) -> Self {
        let mut s = DefaultHasher::new();
        code.hash(&mut s);
        SourceHash(s.finish() as u16)
    }

    pub fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

pub const OUTPUT_LANG: &str = "js#";

#[derive(Debug, Clone, Copy)]
enum CodeBlock {
    Source(ByteSpan, SourceHash),
    Output(ByteSpan, Option<SourceHash>),
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CodeBlockKind {
    Source,
    Output(Option<SourceHash>),
}

pub struct BlockEvalResult {
    pub body: String,
    pub output_lang: String,
}

pub trait NoteEvalContext {
    type State;
    fn begin(&mut self) -> Self::State;
    fn try_parse_block_lang(lang: &str) -> Option<CodeBlockKind>;
    fn eval_block(
        &mut self,
        body: &str,
        hash: SourceHash,
        state: &mut Self::State,
    ) -> BlockEvalResult;
    fn should_force_eval(&self) -> bool;
}

struct JsNoteEvalContext {
    context: Context,
}

impl NoteEvalContext for JsNoteEvalContext {
    type State = ();

    fn try_parse_block_lang(lang: &str) -> Option<CodeBlockKind> {
        match lang {
            "js" => Some(CodeBlockKind::Source),

            output if output.starts_with(OUTPUT_LANG) => {
                let hex_str = &output[OUTPUT_LANG.len()..];
                Some(CodeBlockKind::Output(SourceHash::parse(hex_str)))
            }

            _ => None,
        }
    }

    fn eval_block(&mut self, body: &str, hash: SourceHash, _: &mut Self::State) -> BlockEvalResult {
        let result = self.context.eval(Source::from_bytes(body));

        BlockEvalResult {
            body: match result {
                Ok(res) => res.display().to_string(),
                Err(err) => format!("{:#}", err),
            },

            output_lang: format!("{}{:x}", OUTPUT_LANG, hash.0),
        }
    }

    fn should_force_eval(&self) -> bool {
        false
    }

    fn begin(&mut self) -> Self::State {
        ()
    }
}

pub fn execute_code_blocks<Ctx: NoteEvalContext>(
    cx: &mut Ctx,
    text_structure: &TextStructure,
    text: &str,
) -> Option<Vec<TextChange>> {
    let script_blocks: SmallVec<[_; 8]> = text_structure
        .filter_map_codeblocks(Ctx::try_parse_block_lang)
        .filter_map(|(index, desc, block_kind)| {
            let byte_range = desc.byte_pos.clone();

            let (_, code_desc) = text_structure
                .iterate_immediate_children_of(index)
                .find(|(_, desc)| desc.kind == SpanKind::Text)?;

            let code = &text[code_desc.byte_pos.range()];

            match block_kind {
                CodeBlockKind::Source if code.trim().len() > 0 => Some((
                    index,
                    CodeBlock::Source(byte_range, SourceHash::from(code)),
                    code,
                )),

                CodeBlockKind::Output(hash) => {
                    Some((index, CodeBlock::Output(byte_range, hash), code))
                }

                _ => None,
            }
        })
        .collect();

    if script_blocks.is_empty() {
        return None;
    }

    // println!("#### SCRIPT blocks: {:#?}", script_blocks);

    let mut changes: SmallVec<[TextChange; 4]> = SmallVec::new();

    let mut last_was_source: Option<(SourceHash, ByteSpan, &str)> = None;

    let needs_re_eval = script_blocks.len() % 2 != 0 ||  script_blocks[..]
        .chunks_exact(2)
        .any(|elements| match &elements {
                // if hash was parsed check if it matches
               &[(_,CodeBlock::Source(_, source_hash), _), (_, CodeBlock::Output(_, Some(output_source_hash)), _)] =>  source_hash != output_source_hash,
               // failed to parse
               // &[(_,CodeBlock::LiveJS(_, _), _), (_, CodeBlock::Output(_, None), _)] => true ,
            _ => true,
        });

    if !needs_re_eval && !cx.should_force_eval() {
        return None;
    }

    let mut state = cx.begin();

    for (_, block, inner_body) in script_blocks {
        match block {
            CodeBlock::Source(current_block_range, current_hash) => {
                // this branch means that we are missing an ouput block => add it
                if let Some((source_hash, block_range, prev_block_body)) = last_was_source.take() {
                    changes.push(TextChange::Insert(
                        ByteSpan::point(block_range.end),
                        "\n".to_string()
                            + &print_output_block(cx.eval_block(
                                prev_block_body,
                                source_hash,
                                &mut state,
                            )),
                    ));
                };

                last_was_source = Some((current_hash, current_block_range, inner_body));
            }
            CodeBlock::Output(output_range, _) => match last_was_source.take() {
                Some((source_hash, _, source_code)) => {
                    // this branch means that we have a corresponding output block
                    let eval_res =
                        print_output_block(cx.eval_block(source_code, source_hash, &mut state));
                    if eval_res.as_str() != inner_body {
                        // don't add a text change if the result is the same
                        // note that we still need to compute JS for JS context to be consistent
                        changes.push(TextChange::Insert(output_range, eval_res));
                    }
                }
                None => {
                    // this branch means that we have an orphant code block => remove it
                    // changes.push(TextChange::Replace(output_range, "".to_string()));
                }
            },
        }
    }

    if let Some((source_hash, range, body)) = last_was_source {
        changes.push(TextChange::Insert(
            ByteSpan::new(range.end, range.end),
            "\n".to_string() + &print_output_block(cx.eval_block(body, source_hash, &mut state)),
        ));
    };

    if changes.is_empty() {
        None
    } else {
        Some(changes.into_vec())
    }
}

pub fn execute_live_scripts(text_structure: &TextStructure, text: &str) -> Option<Vec<TextChange>> {
    let mut cx = JsNoteEvalContext {
        context: Context::default(),
    };
    execute_code_blocks(&mut cx, text_structure, text)
}

fn print_output_block(eval_result: BlockEvalResult) -> String {
    format!("```{}\n{}\n```", eval_result.output_lang, eval_result.body)
}

#[cfg(test)]
mod tests {

    use crate::effects::text_change_effect::apply_text_changes;

    use super::*;
    #[test]
    pub fn test_executing_live_js_blocks() {
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
                Some(r#"
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
"#),
            ),
        ];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = execute_live_scripts(&structure, &text);

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

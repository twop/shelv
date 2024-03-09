use boa_engine::{Context, Source};
use eframe::egui::output;
use smallvec::SmallVec;
use std::{
    fmt::format,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    app_actions::TextChange,
    text_structure::{ByteRange, SpanIndex, SpanKind, TextStructure},
};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SourceHash(u16);

impl SourceHash {
    fn parse(hex: &str) -> Option<Self> {
        u16::from_str_radix(hex, 16).ok().map(SourceHash)
    }
}

pub const OUTPUT_LANG: &str = "#";

enum CodeBlock {
    LiveJS(ByteRange, SourceHash),
    Output(ByteRange, Option<SourceHash>),
}

pub fn execute_live_scripts(text_structure: &TextStructure, text: &str) -> Option<Vec<TextChange>> {
    let script_blocks: SmallVec<[_; 8]> = text_structure
        .iter()
        .filter_map(|(index, desc)| match desc.kind {
            SpanKind::CodeBlock => text_structure.find_meta(index).and_then(|meta| match meta {
                crate::text_structure::SpanMeta::CodeBlock { lang } => {
                    let byte_range = ByteRange(desc.byte_pos.clone());

                    let (_, code_desc) = text_structure
                        .iterate_immediate_children_of(index)
                        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

                    let code = &text[code_desc.byte_pos.clone()];

                    match lang.as_str() {
                        "js" => Some((
                            index,
                            CodeBlock::LiveJS(byte_range, calculate_hash(code)),
                            code,
                        )),
                        output if output.starts_with(OUTPUT_LANG) => {
                            let hex_str = &output[OUTPUT_LANG.len()..];
                            Some((
                                index,
                                CodeBlock::Output(byte_range, SourceHash::parse(hex_str)),
                                code,
                            ))
                        }
                        _ => None,
                    }
                }

                _ => None,
            }),
            _ => None,
        })
        .collect();

    if script_blocks.is_empty() {
        return None;
    }

    let mut changes: Vec<TextChange> = vec![];

    let mut last_was_js: Option<(SourceHash, ByteRange, &str)> = None;
    let mut context = Context::default();

    for (block_index, block, inner_body) in script_blocks {
        match block {
            CodeBlock::LiveJS(current_block_range, current_hash) => {
                // this branch means that we are missing an ouput block => add it
                if let Some((source_hash, block_range, prev_block_body)) = last_was_js.take() {
                    changes.push(TextChange::Replace(
                        ByteRange(block_range.end..block_range.end),
                        "\n".to_string()
                            + &print_output_block(prev_block_body, source_hash, &mut context),
                    ));
                };

                last_was_js = Some((current_hash, current_block_range, inner_body));
            }
            CodeBlock::Output(output_range, maybe_hash) => match last_was_js.take() {
                Some((source_hash, source_range, source_code)) => {
                    // TOD optimization: only update code block if a previous block was changed.
                    let eval_res = print_output_block(source_code, source_hash, &mut context);
                    changes.push(TextChange::Replace(output_range, eval_res));
                }
                None => {
                    // this branch means that we have an orphant code block => remove it
                    changes.push(TextChange::Replace(output_range, "".to_string()));
                }
            },
        }
    }

    if let Some((source_hash, range, body)) = last_was_js {
        changes.push(TextChange::Replace(
            ByteRange(range.end..range.end),
            "\n".to_string() + &print_output_block(body, source_hash, &mut context),
        ));
    };

    if changes.is_empty() {
        None
    } else {
        Some(changes)
    }
}

fn print_output_block(body: &str, hash: SourceHash, context: &mut Context) -> String {
    let result = context.eval(Source::from_bytes(body));
    format!(
        "```{}{:x}\n{}\n```",
        OUTPUT_LANG,
        hash.0,
        match result {
            Ok(res) => res.display().to_string(),
            Err(err) => format!("{:#}", err),
        }
    )
}

fn calculate_hash(t: &str) -> SourceHash {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    SourceHash(s.finish() as u16)
}

#[cfg(test)]
mod tests {
    use crate::app_actions::apply_text_changes;

    use super::*;
    #[test]
    pub fn test_splitting_list_item_via_enter() {
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
```{||}
```#da0b
"hello world!"
```
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
```#aaa
I will be overwritten
```
"#,
                Some(
                    r#"
```js
'hello world' + '!'
```{||}
```#da0b
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
```#da0b
I should be overwritten, but I won't
```
"#,
                None,
            ),
            // ________________________________________________
            (
                "## replaces the content of the output block if cache is empty ##",
                r#"
```js
2 + 2
```{||}
```#asd
1
```
"#,
                Some(
                    r#"
```js
2 + 2
```{||}
```#2cd1
4
```
"#,
                ),
            ),
            // ________________________________________________
            (
                "## removes orhpant output blocks ##",
                r#"
```#dfgh
1
```
```js
2 + 2
```{||}
```#2
3
```
"#,
                Some(
                    r#"

```js
2 + 2
```{||}
```#2cd1
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
```{||}
```#b511
Error: yo!
```
"#,
                ),
            ),
        ];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::create_from(&text);

            let changes = execute_live_scripts(&structure, &text);

            match (changes, expected_output) {
                (Some(changes), Some(expected_output)) => {
                    let cursor = apply_text_changes(&mut text, cursor, changes).unwrap();
                    assert_eq!(
                        TextChange::encode_cursor(&text, cursor),
                        expected_output,
                        "test case: {}",
                        desc
                    )
                }
                (None, None) => (),
                (changes, expected) => assert!(
                    false,
                    "expected={:?}, but got this changes={:?}",
                    expected, changes
                ),
            }
        }
    }
}

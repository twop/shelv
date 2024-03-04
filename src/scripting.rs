use boa_engine::{Context, Source};
use smallvec::SmallVec;
use std::{
    fmt::format,
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::{
    app_actions::TextChange,
    text_structure::{ByteRange, SpanIndex, SpanKind, TextStructure},
};

#[derive(PartialEq, Debug)]
struct SourceHash(pub u64);

pub struct ScriptComputeCache {
    evals: Vec<(SourceHash, String)>,
}

enum CodeBlock {
    LiveJS(ByteRange),
    Output(ByteRange),
}

pub fn execute_live_scripts(
    text_structure: &TextStructure,
    text: &str,
    // compute_cache: &mut ScriptComputeCache,
) -> Option<Vec<TextChange>> {
    // Instantiate the execution context

    let script_blocks: SmallVec<[_; 8]> = text_structure
        .iter()
        .filter_map(|(index, desc)| match desc.kind {
            SpanKind::CodeBlock => text_structure.find_meta(index).and_then(|meta| match meta {
                crate::text_structure::SpanMeta::CodeBlock { lang } => {
                    let byte_range = ByteRange(desc.byte_pos.clone());
                    match lang.as_str() {
                        "js" => Some((index, CodeBlock::LiveJS(byte_range))),
                        "output" => Some((index, CodeBlock::Output(byte_range))),
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

    let mut last_was_js: Option<(SpanIndex, ByteRange)> = None;

    for (block_index, block) in script_blocks {
        match block {
            CodeBlock::LiveJS(current_block_range) => match last_was_js.take() {
                Some((prev_js_block_index, _)) => {
                    // it means that the last block didn't produce an output yet
                    let code = text_structure
                        .iterate_immediate_children_of(prev_js_block_index)
                        .find(|(_, desc)| desc.kind == SpanKind::Text);

                    let (_, code) = code.unwrap();

                    let mut context = Context::default();
                    let result = context
                        .eval(Source::from_bytes(&text[code.byte_pos.clone()]))
                        .unwrap();

                    let to_insert = format!(" {}", result.display());
                    changes.push(TextChange::Replace(
                        ByteRange(current_block_range.end..current_block_range.end),
                        to_insert,
                    ));
                }
                None => last_was_js = Some((block_index, current_block_range)),
            },
            CodeBlock::Output(output_range) => match last_was_js.take() {
                Some(prev_js_block_index) => {
                    todo!("check if the block needs to be recomputed");
                }
                None => {
                    // it means that we have an orphant code block => remote it
                    changes.push(TextChange::Replace(output_range, "".to_string()));
                }
            },
        }
    }

    if let Some((block_index, range)) = last_was_js {
        // TODO make the code prettier
        let code = text_structure
            .iterate_immediate_children_of(block_index)
            .find(|(_, desc)| desc.kind == SpanKind::Text);

        let (_, code) = code.unwrap();

        let mut context = Context::default();
        let result = context
            .eval(Source::from_bytes(&text[code.byte_pos.clone()]))
            .unwrap();

        let to_insert = format!("\n```output\n{}\n```", result.display());
        changes.push(TextChange::Replace(
            ByteRange(range.end..range.end),
            to_insert,
        ));
    };

    if changes.is_empty() {
        None
    } else {
        Some(changes)
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[cfg(test)]
mod tests {
    use crate::app_actions::apply_text_changes;

    use super::*;
    #[test]
    pub fn test_splitting_list_item_via_enter() {
        let test_cases = [(
            "## computes an output for a standalone jsblock ##",
            r#"
```js
'hello world' + '!'
```{||}
"#,
            r#"
```js
'hello world' + '!'
```{||}
```output
"hello world!"
```
"#,
        )];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::create_from(&text);

            let changes = execute_live_scripts(&structure, &text).unwrap();

            let cursor = apply_text_changes(&mut text, cursor, changes).unwrap();
            assert_eq!(
                TextChange::encode_cursor(&text, cursor),
                expected_output,
                "test case: {}",
                desc
            );
        }
    }
}

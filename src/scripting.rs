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
pub struct SourceHash(u64);

pub struct ScriptComputeCache {
    evals: Vec<(SourceHash, u8)>,
    generation: u8,
}

enum CodeBlock {
    LiveJS(ByteRange),
    Output(ByteRange),
}

pub fn execute_live_scripts(
    text_structure: &TextStructure,
    text: &str,
    compute_cache: &mut ScriptComputeCache,
) -> Option<Vec<TextChange>> {
    let this_generation = compute_cache.generation.wrapping_add(1);

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
                        "js" => Some((index, CodeBlock::LiveJS(byte_range), code)),
                        "output" => Some((index, CodeBlock::Output(byte_range), code)),
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

    let mut last_was_js: Option<(SpanIndex, ByteRange, &str)> = None;

    for (block_index, block, inner_body) in script_blocks {
        match block {
            CodeBlock::LiveJS(current_block_range) => {
                if let Some((_, prev_code_block_range, prev_block_body)) = last_was_js.take() {
                    compute_cache.evals.push((
                        SourceHash(calculate_hash(&prev_block_body)),
                        this_generation,
                    ));
                    changes.push(TextChange::Replace(
                        ByteRange(prev_code_block_range.end..prev_code_block_range.end),
                        "\n".to_string() + &print_output_block(prev_block_body),
                    ));
                };

                last_was_js = Some((block_index, current_block_range, inner_body));
            }
            CodeBlock::Output(output_range) => match last_was_js.take() {
                Some((prev_js_block_index, source_range, source_code)) => {
                    let source_hash = SourceHash(calculate_hash(&source_code));
                    let exisiting = compute_cache
                        .evals
                        .iter_mut()
                        .find(|(hash, _)| hash == &source_hash);

                    if let Some((_, gen)) = exisiting {
                        // just update generation for gc
                        *gen = this_generation;
                    } else {
                        let eval_res = print_output_block(source_code);
                        compute_cache.evals.push((source_hash, this_generation));
                        changes.push(TextChange::Replace(output_range, eval_res));
                    }
                }
                None => {
                    // it means that we have an orphant code block => remote it
                    changes.push(TextChange::Replace(output_range, "".to_string()));
                }
            },
        }
    }

    if let Some((block_index, range, body)) = last_was_js {
        compute_cache
            .evals
            .push((SourceHash(calculate_hash(&body)), this_generation));

        changes.push(TextChange::Replace(
            ByteRange(range.end..range.end),
            "\n".to_string() + &print_output_block(body),
        ));
    };

    // GC for unused evals
    compute_cache
        .evals
        .retain(|(_, gen)| *gen == this_generation);

    if changes.is_empty() {
        None
    } else {
        Some(changes)
    }
}

fn print_output_block(body: &str) -> String {
    let mut context = Context::default();
    let result = context.eval(Source::from_bytes(body));
    format!(
        "```output\n{}\n```",
        match result {
            Ok(res) => res.display().to_string(),
            Err(err) => format!("{:#}", err),
        }
    )
}

fn calculate_hash(t: &str) -> u64 {
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
        let test_cases = [
            (
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
            ),
            (
                "## replaces the content of the output block if cache is empty ##",
                r#"
```js
2 + 2
```{||}
```output
1
```
"#,
                r#"
```js
2 + 2
```{||}
```output
4
```
"#,
            ),
            (
                "## removes orhpant output blocks ##",
                r#"
```output
1
```
```js
2 + 2
```{||}
```output
3
```
"#,
                r#"

```js
2 + 2
```{||}
```output
4
```
"#,
            ),
            (
                "## prints an error ##",
                r#"
```js
throw new Error("yo!")
```{||}
"#,
                r#"
```js
throw new Error("yo!")
```{||}
```output
Error: yo!
```
"#,
            ),
        ];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::create_from(&text);

            let changes = execute_live_scripts(
                &structure,
                &text,
                &mut ScriptComputeCache {
                    evals: vec![],
                    generation: 0,
                },
            )
            .unwrap();

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

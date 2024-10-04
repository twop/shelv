use std::fmt::format;

use itertools::Itertools;
use smallvec::SmallVec;
use tree_sitter::Point;

use crate::{
    byte_span::{self, ByteSpan},
    command::TextCommandContext,
    effects::text_change_effect::TextChange,
    settings::SETTINGS_BLOCK_LANG,
    text_structure::{SpanKind, SpanMeta},
};

#[derive(Debug)]
struct KDLChildrenScope {
    byte_span: ByteSpan,
    left_bracket: (ByteSpan, tree_sitter::Point), // "{",
    right_bracket: (ByteSpan, tree_sitter::Point), // "} "
}

// handler on ENTER
pub fn on_enter_inside_kdl_block(
    TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: cursor,
    }: TextCommandContext,
) -> Option<Vec<TextChange>> {
    let settings_block_span = structure
        .find_span_at(SpanKind::CodeBlock, cursor)
        .and_then(|(span_range, item_index)| structure.find_meta(item_index).zip(Some(span_range)))
        .and_then(|(meta, span_range)| match meta {
            SpanMeta::CodeBlock { lang } if lang == SETTINGS_BLOCK_LANG => Some(span_range),
            _ => None,
        })?;

    let query = r#"
    (node_children (
        "{" @open
        "}" @close
    )) @scope
    "#;

    let settings_body = &text[settings_block_span.range()];

    let relative_cursor = cursor.move_by(-(settings_block_span.start as isize));
    let language = tree_sitter_kdl::language();
    let query = tree_sitter::Query::new(language, query).unwrap();

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).unwrap();

    let tree = parser.parse(settings_body, None).unwrap();
    let mut query_cursor = tree_sitter::QueryCursor::new();

    let kdl_scopes: SmallVec<[KDLChildrenScope; 4]> = query_cursor
        .matches(&query, tree.root_node(), settings_body.as_bytes())
        .filter_map(|match_| {
            let mut block: Option<ByteSpan> = None;
            let mut left_bracket: Option<(ByteSpan, tree_sitter::Point)> = None;
            let mut right_bracket: Option<(ByteSpan, tree_sitter::Point)> = None;
            for capture in match_.captures {
                let node = capture.node;
                let node_name = query.capture_names()[capture.index as usize].as_str();
                match node_name {
                    "open" => {
                        left_bracket = Some((
                            ByteSpan::from_range(&node.byte_range()),
                            node.start_position(),
                        ));
                    }
                    "close" => {
                        right_bracket = Some((
                            ByteSpan::from_range(&node.byte_range()),
                            node.start_position(),
                        ));
                    }
                    "scope" => block = Some(ByteSpan::from_range(&node.byte_range())),

                    _ => (),
                };
            }

            match (block, left_bracket, right_bracket) {
                (Some(byte_span), Some(left_bracket), Some(right_bracket)) => {
                    Some(KDLChildrenScope {
                        byte_span,
                        left_bracket,
                        right_bracket,
                    })
                }
                _ => None,
            }
        })
        .filter(|scope| scope.byte_span.contains(relative_cursor))
        .sorted_by(|a, b| a.byte_span.range().len().cmp(&b.byte_span.range().len()))
        .collect();

    println!("found {kdl_scopes:#?} scopes, relative cursor = {relative_cursor:?}");

    if kdl_scopes.is_empty() {
        return None;
    }

    let indent_level = kdl_scopes.len();
    let surrounding_scope = &kdl_scopes[0];

    let indent_for_cursor = "\t".repeat(indent_level.max(0) as usize);
    let mut changes = vec![TextChange::Insert(
        cursor,
        format!("\n{indent_for_cursor}{}", TextChange::CURSOR),
    )];

    // if both {} are on the same line we need to move } accordingly
    // it will be shifted one level left relative to the cursor postion
    if surrounding_scope.left_bracket.1.row == surrounding_scope.right_bracket.1.row {
        let indent = "\t".repeat(indent_level - 1);
        changes.push(TextChange::Insert(
            surrounding_scope
                .right_bracket
                .0
                // shift back from relateto absolute position within a note
                .move_by(settings_block_span.start as isize),
            format!("\n{indent}}}"),
        ));
    }

    Some(changes)
}

pub fn autoclose_bracket_inside_kdl_block(
    TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: cursor,
    }: TextCommandContext,
) -> Option<Vec<TextChange>> {
    structure
        .find_span_at(SpanKind::CodeBlock, cursor.clone())
        .and_then(|(_, item_index)| structure.find_meta(item_index))
        .and_then(|meta| match meta {
            SpanMeta::CodeBlock { lang } if lang == SETTINGS_BLOCK_LANG => {
                if cursor.is_empty() {
                    Some(
                        [TextChange::Insert(
                            cursor,
                            format!("{{{caret}}}", caret = TextChange::CURSOR),
                        )]
                        .into(),
                    )
                } else {
                    Some(
                        [TextChange::Insert(
                            cursor,
                            format!("{{{selection}}}", selection = &text[cursor.range()]),
                        )]
                        .into(),
                    )
                }
            }
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::text_change_effect::apply_text_changes;
    use crate::text_structure::TextStructure;

    #[test]
    fn test_enter_inside_kdl_block() {
        let test_cases = [
            (
                "Simple split",
                r#"
```settings
global "Cmd Ctrl Option Shift S" {{||}ShowHideApp;}
```"#,
                Some(
                    r#"
```settings
global "Cmd Ctrl Option Shift S" {
	{||}ShowHideApp;
}
```"#,
                ),
            ),
            (
                "Cursor is just outside '}' should not trigger",
                r#"
```settings
block {
    child
}{||}
```"#,
                None,
            ),
            (
                "preserve indentation",
                "```settings\nnode {\n\tchild {||}\n}\n```",
                Some("```settings\nnode {\n\tchild \n\t{||}\n}\n```"),
            ),
            ("No indentation needed", "```settings\nnode{||}\n```", None),
            (
                "Multiple levels of indentation",
                "```settings\nnode {\n\tchild {\n\t\tgrandchild {||}\n\t}\n}\n```",
                Some("```settings\nnode {\n\tchild {\n\t\tgrandchild \n\t\t{||}\n\t}\n}\n```"),
            ),
        ];

        for (desc, input, expected) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = on_enter_inside_kdl_block(TextCommandContext::new(
                &structure,
                &text,
                cursor.clone(),
            ));

            if let Some(expected_result) = expected {
                let changes = changes.expect("Expected changes, but got None");
                let cursor =
                    apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();
                let res = TextChange::encode_cursor(&text, cursor.unwrap());

                assert_eq!(res, expected_result, "Test case: {}", desc);
            } else {
                assert!(
                    changes.is_none(),
                    "Test case: {} - Expected None, but got Some",
                    desc
                );
            }
        }
    }

    #[test]
    fn test_enter_outside_kdl_block() {
        let input = "Some text {||}\n```settings\nnode\n```";
        let (text, cursor) = TextChange::try_extract_cursor(input.to_string());
        let cursor = cursor.unwrap();

        let structure = TextStructure::new(&text);

        let changes =
            on_enter_inside_kdl_block(TextCommandContext::new(&structure, &text, cursor.clone()));

        assert!(
            changes.is_none(),
            "Should not handle enter outside KDL block"
        );
    }

    #[test]
    fn test_autoclose_bracket_inside_kdl_block() {
        let test_cases = [
            (
                "Simple autoclose",
                r#"
```settings
node {||}
```"#,
                Some(
                    r#"
```settings
node {{||}}
```"#,
                ),
            ),
            (
                "Autoclose with selection",
                r#"
```settings
{|}selection{|}
```"#,
                Some(
                    r#"
```settings
{|}{selection}{|}
```"#,
                ),
            ),
            (
                "No autoclose outside settings block",
                r#"Some text {||}
```settings
node
```"#,
                None,
            ),
            (
                "No autoclose in different code block",
                r#"```javascript
function() {||}
```"#,
                None,
            ),
        ];

        for (desc, input, expected) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = autoclose_bracket_inside_kdl_block(TextCommandContext::new(
                &structure,
                &text,
                cursor.clone(),
            ));

            if let Some(expected_result) = expected {
                let changes = changes.expect("Expected changes, but got None");
                let cursor =
                    apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();
                let res = TextChange::encode_cursor(&text, cursor.unwrap());

                assert_eq!(res, expected_result, "Test case: {}", desc);
            } else {
                assert!(
                    changes.is_none(),
                    "Test case: {} - Expected None, but got Some",
                    desc
                );
            }
        }
    }
}

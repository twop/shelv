use crate::{
    byte_span::ByteSpan, command::TextCommandContext, effects::text_change_effect::TextChange,
    text_structure::SpanKind,
};

use super::select_unordered_list_marker;

pub fn on_space_after_task_markers(context: TextCommandContext) -> Option<Vec<TextChange>> {
    let TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: cursor,
    } = context;

    let unexpanded_task_markers = text[..cursor.start].ends_with("[]");

    if !unexpanded_task_markers {
        return None;
    }

    let (line_loc, start_line_span, _) = structure.find_line_location(cursor)?;

    match structure
        .find_span_on_the_line(SpanKind::ListItem, line_loc.line_start)
        .map(|(_range, idx, _)| structure.get_span_inner_content(idx))
    {
        Some(inner_content_span) if inner_content_span.start + "[]".len() == cursor.start => {
            {
                // At the beginning of a list item with unexpanded task markers "- []{}"
                Some(vec![TextChange::Insert(
                    ByteSpan::new(inner_content_span.start, cursor.start),
                    "[ ]".to_string(),
                )])
            }
        }
        _ => None,
    }
    .or_else(
        || match structure.find_span_at(SpanKind::CodeBlock, cursor) {
            // do nothing inside code blocks
            None if start_line_span.start + "[]".len() == cursor.start => {
                // Start of the line, so we must add a new list item
                Some(vec![TextChange::Insert(
                    ByteSpan::new(start_line_span.start, cursor.start),
                    format!("{} [ ]", select_unordered_list_marker(0)),
                )])
            }
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::{effects::text_change_effect::apply_text_changes, text_structure::TextStructure};

    use super::*;
    #[test]
    pub fn test_skips_expanding_task_markers_when_not_start_of_line() {
        let (text, cursor) = TextChange::try_extract_cursor("a[]{||}".to_string());
        let changes = on_space_after_task_markers(TextCommandContext::new(
            &TextStructure::new(&text),
            &text,
            cursor.unwrap().clone(),
        ));
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_skips_expanding_task_markers_when_in_code_block() {
        let (text, cursor) = TextChange::try_extract_cursor("```\n[]{||}```".to_string());
        let changes = on_space_after_task_markers(TextCommandContext::new(
            &TextStructure::new(&text),
            &text,
            cursor.unwrap().clone(),
        ));
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_space_expands_task_markerss() {
        // Note: the [Space] press in reality adds a space after the cursor
        let test_cases = [
            (
                "## Expanding a task marker on the first empty line ##",
                "[]{||}",
                "- [ ]{||}",
            ),
            (
                "## Expanding a task marker on any empty line ##",
                "a\n[]{||}",
                "a\n- [ ]{||}",
            ),
            (
                "## Expanding a task marker on the start of a list item ##",
                "- []{||}",
                "- [ ]{||}",
            ),
            (
                "## Expanding a task marker on a nested list item ##",
                "- a\n\t* []{||}",
                "- a\n\t* [ ]{||}",
            ),
            (
                "## Expanding a task marker from the start of a non-empty line##",
                "[]{||}abc",
                "- [ ]{||}abc",
            ),
        ];

        for (desc, input, output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = on_space_after_task_markers(TextCommandContext::new(
                &structure,
                &text,
                cursor.clone(),
            ))
            .unwrap();

            let cursor = apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();
            assert_eq!(
                TextChange::encode_cursor(&text, cursor.unwrap()),
                output,
                "test case: {}",
                desc
            );
        }
    }
}

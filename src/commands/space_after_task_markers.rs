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

    let unexpanded_task_markers = &text[..cursor.start].ends_with("[]");

    if !unexpanded_task_markers {
        return None;
    }

    if structure
        .find_span_at(SpanKind::ListItem, cursor.clone())
        .is_some_and(|(span, _)| {
            // At the beggining of a list item with unexpanded task markers "- []{}"
            cursor.start == span.start + 4
        })
    {
        // Ie. "- []{}" -> "- [ ]{}"
        return Some(vec![TextChange::Insert(
            ByteSpan::new(cursor.start - 2, cursor.start),
            "[ ]".to_string(),
        )]);
    }

    if structure
        .find_span_at(SpanKind::Paragraph, cursor.clone())
        .is_some_and(|(_, _)| {
            // Start of the file or a new line, so we must add a new list item
            let text_before_task_markers = &text[..cursor.start - 2];
            text_before_task_markers.len() == 0 || text_before_task_markers.ends_with("\n")
        })
    {
        // Ie. "[]{}" -> "- []{}"
        return Some(vec![TextChange::Insert(
            ByteSpan::new(cursor.start - 2, cursor.start),
            select_unordered_list_marker(0).to_string() + " [ ]",
        )]);
    }

    None
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

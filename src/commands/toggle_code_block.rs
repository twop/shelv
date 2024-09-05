use crate::{
    command::TextCommandContext, effects::text_change_effect::TextChange, text_structure::SpanKind,
};

pub fn toggle_code_block(
    TextCommandContext {
        text_structure,
        text,
        byte_cursor,
    }: TextCommandContext,
) -> Option<Vec<TextChange>> {
    match text_structure
        .find_span_at(SpanKind::CodeBlock, byte_cursor)
        .map(|(span_range, idx)| (span_range, text_structure.get_span_inner_content(idx)))
    {
        // replace block with the inner content, note that the cursor will be expanded automatically
        Some((span_byte_range, content_byte_range)) => Some(vec![TextChange::Replace(
            span_byte_range,
            text[content_byte_range.range()].to_string(),
        )]),

        None => {
            // TODO(perf) that operation is likely to be inneficient, due to traversing &str by chars
            let (before, selection, after) = (
                &text[..byte_cursor.start],
                &text[byte_cursor.range()],
                &text[byte_cursor.end..],
            );

            let mut replacement = String::with_capacity(
                selection.len() + "```".len() * 2 + "\n".len() * 4 + TextChange::CURSOR.len(),
            );

            if !before.ends_with("\n") && before.len() > 0 {
                replacement.push('\n');
            }
            replacement.push_str("```");
            replacement.push_str(TextChange::CURSOR);
            replacement.push('\n');

            if selection.len() > 0 {
                replacement.push_str(selection);

                if !selection.ends_with("\n") {
                    replacement.push('\n');
                }
            }

            replacement.push_str("```");

            if !after.starts_with("\n") {
                replacement.push('\n');
            }

            Some(Vec::from([TextChange::Replace(byte_cursor, replacement)]))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{effects::text_change_effect::apply_text_changes, text_structure::TextStructure};

    use super::*;

    #[test]
    pub fn tests_for_toggle_code_block() {
        let test_cases = [
            (
                "## doesn't produce new line at the begining of the doc ##",
                "{||}rest",
                "```{||}\n```\nrest",
            ),
            (
                "## wraps selection with ```##",
                "before {|}selection{|} after",
                "before \n```{||}\nselection\n```\n after",
            ),
            (
                "## deletes code block wrapping and selects the conent ##",
                "before \n```\nse{|}lec{|}tion\n```\n after",
                "before \n{|}selection\n{|}\n after",
            ),
        ];

        for (desc, input, output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes =
                toggle_code_block(TextCommandContext::new(&structure, &text, cursor.clone()))
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

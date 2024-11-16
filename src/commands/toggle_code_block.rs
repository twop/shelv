use crate::{
    byte_span::ByteSpan, command::TextCommandContext, effects::text_change_effect::TextChange,
    text_structure::SpanKind,
};

pub fn toggle_code_block(
    TextCommandContext {
        text_structure,
        text,
        byte_cursor,
    }: TextCommandContext,
    lang: Option<&str>,
) -> Option<Vec<TextChange>> {
    match text_structure
        .find_span_at(SpanKind::CodeBlock, byte_cursor)
        .map(|(span_range, idx)| (span_range, text_structure.get_span_inner_content(idx)))
    {
        // replace block with the inner content, note that the cursor will be expanded automatically
        Some((span_byte_range, content_byte_range)) => Some(vec![TextChange::Insert(
            span_byte_range,
            text[content_byte_range.range()].to_string(),
        )]),

        None => {
            println!("[TOGGLECODEBLOCK] C={:#?} T='{}'", byte_cursor, text);
            let (before, selection, after) = (
                &text[..byte_cursor.start],
                &text[byte_cursor.range()],
                &text[byte_cursor.end..],
            );

            let mut before_selection = String::new();
            if !before.ends_with("\n") && before.len() > 0 {
                before_selection.push('\n');
            }
            before_selection.push_str("```");
            if let Some(lang) = lang {
                before_selection.push_str(&lang);
            } else {
                before_selection.push_str(TextChange::CURSOR);
            }
            before_selection.push('\n');

            let mut after_selection = String::new();
            if selection.len() > 0 && !selection.ends_with("\n") {
                after_selection.push('\n');
            }
            after_selection.push_str("```");
            if !after.starts_with("\n") {
                after_selection.push('\n');
            }

            if byte_cursor.is_empty() {
                Some(
                    [TextChange::Insert(
                        byte_cursor,
                        before_selection
                            + match lang.is_some() {
                                true => TextChange::CURSOR,
                                false => "",
                            }
                            + after_selection.as_str(),
                    )]
                    .into(),
                )
            } else {
                Some(
                    [
                        TextChange::Insert(ByteSpan::point(byte_cursor.start), before_selection),
                        TextChange::Insert(ByteSpan::point(byte_cursor.end), after_selection),
                    ]
                    .into(),
                )
            }
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

            let changes = toggle_code_block(
                TextCommandContext::new(&structure, &text, cursor.clone()),
                None,
            )
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
    #[test]
    pub fn tests_for_toggle_code_block_with_language() {
        let test_cases = [
            (
                "## doesn't produce new line at the begining of the doc with language ##",
                "{||}rest",
                "```js\n{||}```\nrest",
            ),
            (
                "## wraps selection with ```js and keeps the selection ##",
                "before {|}selection{|} after",
                "before \n```js\n{|}selection{|}\n```\n after",
            ),
        ];

        for (desc, input, output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = toggle_code_block(
                TextCommandContext::new(&structure, &text, cursor.clone()),
                Some("js"),
            )
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

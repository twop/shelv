use crate::{
    byte_span::ByteSpan, command::TextCommandContext, effects::text_change_effect::TextChange,
    text_structure::SpanKind,
};

pub fn toggle_simple_md_annotations(
    TextCommandContext {
        text_structure,
        text,
        byte_cursor,
    }: TextCommandContext,
    target_span: SpanKind,
    annotation: &str,
) -> Option<Vec<TextChange>> {
    match text_structure
        .find_span_at(target_span, byte_cursor)
        .map(|(span_range, idx)| (span_range, text_structure.get_span_inner_content(idx)))
    {
        // replace block with the inner content, note that the cursor will be expanded automatically
        Some((span_byte_range, content_byte_range)) => Some(vec![TextChange::Replace(
            span_byte_range,
            text[content_byte_range.range()].to_string(),
        )]),

        None => {
            if byte_cursor.is_empty() {
                Some(Vec::from([TextChange::Replace(
                    byte_cursor,
                    format!("{start}{{||}}{end}", start = annotation, end = annotation),
                )]))
            } else {
                Some(Vec::from([
                    TextChange::Replace(ByteSpan::point(byte_cursor.start), annotation.to_string()),
                    TextChange::Replace(ByteSpan::point(byte_cursor.end), annotation.to_string()),
                ]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{effects::text_change_effect::apply_text_changes, text_structure::TextStructure};

    use super::*;

    #[test]
    pub fn tests_for_toggle_simple_annotations() {
        let test_cases = [
            (
                "## puts cursor inside if selection is empty ##",
                "{||}rest",
                "*{||}*rest",
            ),
            (
                "## wraps selection with annotations ##",
                "{|}selection{|}",
                "*{|}selection{|}*",
            ),
            (
                "## reverts annotations if already annotated ##",
                "*sel{|}ect{|}ion*",
                "{|}selection{|}",
            ),
        ];

        for (desc, input, output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = toggle_simple_md_annotations(
                TextCommandContext::new(&structure, &text, cursor.clone()),
                SpanKind::Emphasis,
                "*",
            )
            .unwrap();

            let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
            assert_eq!(
                TextChange::encode_cursor(&text, cursor),
                output,
                "test case: {}",
                desc
            );
        }
    }
}

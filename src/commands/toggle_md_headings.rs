use pulldown_cmark::HeadingLevel;
use smallvec::SmallVec;

use crate::{
    byte_span::ByteSpan,
    command::TextCommandContext,
    effects::text_change_effect::TextChange,
    text_structure::{SpanKind, SpanMeta},
};

fn heading_level_to_annotation(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "#",
        HeadingLevel::H2 => "##",
        HeadingLevel::H3 => "###",
        HeadingLevel::H4 => "####",
        HeadingLevel::H5 => "#####",
        HeadingLevel::H6 => "######",
    }
}

pub fn toggle_md_heading(
    TextCommandContext {
        text_structure,
        text,
        byte_cursor,
    }: TextCommandContext,
    level: HeadingLevel,
) -> Option<Vec<TextChange>> {
    let (span_range, span_kind, span_index) = text_structure.find_any_span_at(byte_cursor)?;

    let parents: SmallVec<[_; 6]> = text_structure.iterate_parents_of(span_index).collect();

    let parent_heading_info = parents.iter().find_map(|(pi, pdesc)| match pdesc.kind {
        SpanKind::Heading(level) => Some((*pi, level, pdesc.byte_pos)),
        _ => None,
    });

    match parent_heading_info {
        Some((index, heading_level, heading_byte_span)) => {
            let parent_inner_conent = text_structure.get_span_inner_content(index);

            // just remove the annotations for the current level
            Some(Vec::from([TextChange::Replace(
                ByteSpan::new(heading_byte_span.start, parent_inner_conent.start),
                match heading_level == level {
                    // just toggle the annotations
                    true => "".to_string(),
                    // if it doesn't match the same level, then set to the target one
                    false => format!("{} ", heading_level_to_annotation(level)),
                },
            )]))
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{effects::text_change_effect::apply_text_changes, text_structure::TextStructure};

    use super::*;

    #[test]
    pub fn tests_for_toggle_md_headings() {
        let test_cases = [
            (
                "## toggles the annotations for the same level ##",
                "## {|}hea{|}ding",
                "{|}hea{|}ding",
                HeadingLevel::H2,
            ),
            (
                "## changes the levels ##",
                "## **bold{||}** heading",
                "### **bold{||}** heading",
                HeadingLevel::H3,
            ),
        ];

        for (desc, input, output, level) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = toggle_md_heading(
                TextCommandContext::new(&structure, &text, cursor.clone()),
                level,
            )
            .expect(&format!("test case '{desc}': didn't produce results"));

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

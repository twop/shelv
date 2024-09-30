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
    let Some((span_range, span_kind, span_index)) = text_structure.find_any_span_at(byte_cursor)
    else {
        // that means that there is nothing in the doc just yet
        // or that we are in a weird place in the doc like "\n\n\n\n{||}"
        let annotation_insertion_pos = text[0..byte_cursor.start]
            .rfind("\n")
            .map(|pos| pos + 1)
            .unwrap_or(0);
        return Some(Vec::from([TextChange::Insert(
            ByteSpan::point(annotation_insertion_pos),
            format!("{} ", heading_level_to_annotation(level)),
        )]));
    };

    let parents: SmallVec<[_; 6]> = text_structure
        .iterate_parents_of(span_index)
        .map(|(index, desc)| (index, desc.kind, desc.byte_pos))
        .chain([(span_index, span_kind, span_range)])
        .collect();

    let surraunding_heading_info =
        parents
            .iter()
            .find_map(|(pi, pkind, pbyte_spand)| match pkind {
                SpanKind::Heading(level) => Some((*pi, level, pbyte_spand)),
                _ => None,
            });

    // println!("%%\nparents={parents:#?}\nparent_heading_info={surraunding_heading_info:#?}\n");
    //
    let target_annotation = heading_level_to_annotation(level);

    match surraunding_heading_info {
        Some((index, heading_level, heading_byte_span)) if index == span_index => {
            // that means that we found itself
            // that can be either because there are no nested spans (empty)
            // or because the cursor is somewhere between elements (like for example "# {||} some content")
            text.get(heading_byte_span.range())
                .and_then(|heading_full_body| {
                    let cur_heading_annotation = heading_level_to_annotation(*heading_level);

                    // in case the heading starts with the pattern like "###" vs "---" (underline)
                    // we just trim the annotation and whitespace
                    if heading_full_body.starts_with(cur_heading_annotation) {
                        let heading_inner_content =
                            heading_full_body[cur_heading_annotation.len()..].trim_start();

                        // just remove the annotations for the current level
                        Some(Vec::from([TextChange::Insert(
                            ByteSpan::new(
                                heading_byte_span.start,
                                heading_byte_span.start + heading_full_body.len()
                                    - heading_inner_content.len(),
                            ),
                            match *heading_level == level {
                                // just toggle the annotations
                                true => "".to_string(),
                                // if it doesn't match the same level, then set to the target one
                                false => format!("{} ", target_annotation),
                            },
                        )]))
                    } else {
                        None
                    }
                })
        }

        Some((index, heading_level, heading_byte_span)) => {
            let parent_inner_conent = text_structure.get_span_inner_content(index);

            // just remove the annotations for the current level
            Some(Vec::from([TextChange::Insert(
                ByteSpan::new(heading_byte_span.start, parent_inner_conent.start),
                match *heading_level == level {
                    // just toggle the annotations
                    true => "".to_string(),
                    // if it doesn't match the same level, then set to the target one
                    false => format!("{} ", target_annotation),
                },
            )]))
        }

        None => {
            // try find a paragraph to add heading annotations

            parents
                .iter()
                .find_map(|(_, kind, range)| match kind {
                    SpanKind::Paragraph => Some(range),
                    _ => None,
                })
                .map(|paragraph_byte_span| {
                    Vec::from([TextChange::Insert(
                        ByteSpan::point(paragraph_byte_span.start),
                        format!("{} ", heading_level_to_annotation(level)),
                    )])
                })
        }
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
                Some("{|}hea{|}ding"),
                HeadingLevel::H2,
            ),
            (
                "## changes the levels ##",
                "## **bold{||}** heading",
                Some("### **bold{||}** heading"),
                HeadingLevel::H3,
            ),
            (
                "## makes paragraph a heading ##",
                "first paragraph\n\nsecond paragraph with **bo{||}ld**",
                Some("first paragraph\n\n# second paragraph with **bo{||}ld**"),
                HeadingLevel::H1,
            ),
            (
                "## if there is no elements besides root insert annotations in the begining ##",
                "  \n{||}",
                Some("  \n# {||}"),
                HeadingLevel::H1,
            ),
            (
                "## if the content is empty adds annotations ##",
                "{||}",
                Some("# {||}"),
                HeadingLevel::H1,
            ),
            (
                "## if cursor is outside inner spans -> changes annotations and trims whitespace ##",
                "#  {||}  some content",
                Some("{|}## {|}some content"),
                HeadingLevel::H2,
            ),
            (
                "## if there is not content in a different level heading -> replaces it cleanly ##",
                "# {||}",
                Some("## {||}"),
                HeadingLevel::H2,
            ),
            (
                "## doesn't do anything if inside other containers like code block ##",
                "```js\nlog({||})\n```\n",
                None,
                HeadingLevel::H2,
            ),
            (
                "## edge case ##",
                "# a\n\n\n\n{||}",
                Some("# a\n\n\n\n## {||}"),
                HeadingLevel::H2,
            ),
        ];

        for (desc, input, expected_output, level) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = toggle_md_heading(
                TextCommandContext::new(&structure, &text, cursor.clone()),
                level,
            );

            match (changes, expected_output) {
                (None, None) => (),
                (Some(changes), Some(expected_output)) => {
                    let cursor =
                        apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();

                    assert_eq!(
                        TextChange::encode_cursor(&text, cursor.unwrap()),
                        expected_output,
                        "test case: {}",
                        desc
                    );
                }
                (changes, expected_output) => {
                    assert!(
                        false,
                        "unexpected matching, text case:{desc} \nchanges = {changes:#?}\nexpected = {expected_output:#?}",
                        // changes, expected_output
                    );
                }
            }
        }
    }
}

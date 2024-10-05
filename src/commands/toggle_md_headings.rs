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
    command_heading_level: HeadingLevel,
) -> Option<Vec<TextChange>> {
    let (line_loc, start_line_span, _) = text_structure.find_line_location(byte_cursor)?;

    if start_line_span.is_empty() {
        // it means that we are on the new line OR empty doc
        // just insert heading annotations there
        return Some(Vec::from([TextChange::Insert(
            ByteSpan::point(start_line_span.start),
            format!("{} ", heading_level_to_annotation(command_heading_level)),
        )]));
    }

    // now we care about two cases:
    // we are either on the same line as heading or paragraph,
    // heading => then we can either toggle/change heading
    // paragraph => just add heading annotations

    let cmd_level_prefix = format!("{} ", heading_level_to_annotation(command_heading_level));

    // Paragraph case
    text_structure
        .find_span_on_the_line(SpanKind::Paragraph, line_loc.line_start)
        .map(|_| {
            Vec::from([TextChange::Insert(
                ByteSpan::point(start_line_span.start),
                cmd_level_prefix.clone(),
            )])
        })
        .or_else(|| {
            // Heading case
            text_structure
                .find_map_span_on_the_line(line_loc.line_start, |desc| match desc.kind {
                    SpanKind::Heading(level) => Some(level),
                    _ => None,
                })
                .and_then(|(span, _index, level)| {
                    // println!("------ found {level}");
                    match level {
                        level
                            if level == command_heading_level
                                && text[span.range()].starts_with(&cmd_level_prefix) =>
                        {
                            // if it is the same level just remove the annotation
                            Some(Vec::from([TextChange::Insert(
                                ByteSpan::new(span.start, span.start + cmd_level_prefix.len()),
                                "".to_string(),
                            )]))
                        }
                        level
                            if level != command_heading_level
                                && text[span.range()]
                                    .starts_with(heading_level_to_annotation(level)) =>
                        {
                            // if it is a different heading level, then just swap the annoatation
                            // with the intended one
                            Some(Vec::from([TextChange::Insert(
                                ByteSpan::new(
                                    span.start,
                                    span.start + heading_level_to_annotation(level).len(),
                                ),
                                heading_level_to_annotation(command_heading_level).to_string(),
                            )]))
                        }
                        _ => None,
                    }
                })
        })
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
                "## if cursor is outside inner spans -> changes annotations ##",
                "#  {||}  some content",
                Some("##  {||}  some content"),
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

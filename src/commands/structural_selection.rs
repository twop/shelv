use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    byte_span::ByteSpan,
    text_structure::{SpanDesc, SpanIndex, SpanKind, TextStructure},
};

/// Like `ByteSpan::contains_pos`, but the end is inclusive,
/// that is, a cursor right after the last char still belongs to the span
pub fn touches(span: ByteSpan, pos: usize) -> bool {
    span.start <= pos && pos <= span.end
}

pub fn innermost_span_at(
    structure: &TextStructure,
    pos: usize,
    is_target: impl Fn(SpanKind) -> bool,
) -> Option<(SpanIndex, SpanDesc)> {
    // spans are stored parents first, so the last match is the innermost one
    structure
        .iter()
        .filter(|(_, desc)| is_target(desc.kind) && touches(desc.byte_pos, pos))
        .last()
        .map(|(index, desc)| (index, *desc))
}

pub fn trim_span(text: &str, span: ByteSpan) -> ByteSpan {
    let slice = &text[span.range()];
    let start = span.start + (slice.len() - slice.trim_start().len());
    let end = span.start + slice.trim_end().len();
    ByteSpan::new(start, end.max(start))
}

/// The text of a list item without the marker, task marker and nested lists
pub fn list_item_text(structure: &TextStructure, item: SpanIndex) -> Option<ByteSpan> {
    structure
        .iterate_immediate_children_of(item)
        .filter(|(_, desc)| !matches!(desc.kind, SpanKind::TaskMarker | SpanKind::List))
        .map(|(_, desc)| desc.byte_pos)
        .reduce(|a, b| ByteSpan::new(a.start.min(b.start), a.end.max(b.end)))
}

pub struct CodeBlockLines {
    /// lines between the fences, without "\n"
    pub code: SmallVec<[ByteSpan; 8]>,
}

pub fn code_block_lines(text: &str, block: ByteSpan) -> CodeBlockLines {
    let mut lines: SmallVec<[ByteSpan; 8]> = SmallVec::new();
    let mut line_start = block.start;
    for (i, ch) in text[block.range()].char_indices() {
        if ch == '\n' {
            lines.push(ByteSpan::new(line_start, block.start + i));
            line_start = block.start + i + 1;
        }
    }
    if line_start < block.end {
        lines.push(ByteSpan::new(line_start, block.end));
    }

    let is_fence = |line: &ByteSpan| {
        let line = text[line.range()].trim();
        line.starts_with("```") || line.starts_with("~~~")
    };

    // the first line is always the opening fence
    let code_start = 1.min(lines.len());
    let code_end = match lines.last() {
        Some(last) if lines.len() > 1 && is_fence(last) => lines.len() - 1,
        _ => lines.len(),
    };

    CodeBlockLines {
        code: lines[code_start..code_end].iter().copied().collect(),
    }
}

/// Heading + everything until the next top level heading of the same or higher level
pub fn heading_section(text: &str, structure: &TextStructure, heading: SpanIndex) -> ByteSpan {
    let desc = structure.span(heading);
    let SpanKind::Heading(level) = desc.kind else {
        return desc.byte_pos;
    };

    let end = structure
        .iterate_top_level()
        .filter(|(_, other)| other.byte_pos.start > desc.byte_pos.start)
        .find_map(|(_, other)| match other.kind {
            SpanKind::Heading(other_level) if other_level <= level => Some(other.byte_pos.start),
            _ => None,
        })
        .unwrap_or(text.len());

    trim_span(text, ByteSpan::new(desc.byte_pos.start, end))
}

/// Every unit (word, inline span, sentence, block, list, section, document) that touches `head`
fn unit_candidates(text: &str, structure: &TextStructure, head: usize) -> SmallVec<[ByteSpan; 16]> {
    let mut candidates: SmallVec<[ByteSpan; 16]> = SmallVec::new();
    let mut push = |span: ByteSpan| {
        if !span.is_empty() && touches(span, head) {
            candidates.push(span);
        }
    };

    // words never cross lines, hence only look at the current one
    if let Some((_, line, _)) = structure.find_line_location(ByteSpan::point(head)) {
        for (offset, word) in text[line.range()].split_word_bound_indices() {
            if word.chars().any(char::is_alphanumeric) {
                let start = line.start + offset;
                push(ByteSpan::new(start, start + word.len()));
            }
        }
    }

    let code_block = innermost_span_at(structure, head, |kind| kind == SpanKind::CodeBlock);

    for (index, desc) in structure.iter() {
        if !touches(desc.byte_pos, head) {
            continue;
        }

        match desc.kind {
            SpanKind::Bold | SpanKind::Emphasis | SpanKind::Strike | SpanKind::InlineCode => {
                push(structure.get_span_inner_content(index));
                push(desc.byte_pos);
            }
            SpanKind::MdLink => {
                // link text, then the whole link
                if let Some(link_text) = structure
                    .iterate_immediate_children_of(index)
                    .map(|(_, child)| child.byte_pos)
                    .reduce(|a, b| ByteSpan::new(a.start.min(b.start), a.end.max(b.end)))
                {
                    push(link_text);
                }
                push(desc.byte_pos);
            }
            SpanKind::Heading(_) => {
                push(structure.get_span_inner_content(index));
                push(desc.byte_pos);
            }
            SpanKind::ListItem => {
                if let Some(item_text) = list_item_text(structure, index) {
                    push(item_text);
                }
                // includes task marker and nested lists
                push(trim_span(text, structure.get_span_inner_content(index)));
                push(desc.byte_pos);
            }
            SpanKind::CodeBlock => {
                let lines = code_block_lines(text, desc.byte_pos);
                if let Some(line) = lines.code.iter().find(|line| touches(**line, head)) {
                    push(*line);
                }
                if let (Some(first), Some(last)) = (lines.code.first(), lines.code.last()) {
                    push(ByteSpan::new(first.start, last.end));
                }
                push(trim_span(text, desc.byte_pos));
            }
            SpanKind::Paragraph | SpanKind::List | SpanKind::Image => {
                push(trim_span(text, desc.byte_pos));
            }
            SpanKind::Text | SpanKind::TaskMarker | SpanKind::Html | SpanKind::Root => (),
        }
    }

    // sentences, but not inside code
    let sentence_context = innermost_span_at(structure, head, |kind| {
        matches!(
            kind,
            SpanKind::Paragraph | SpanKind::Heading(_) | SpanKind::ListItem
        )
    })
    .filter(|(index, _)| match code_block {
        // code block can be nested in a list item, so check what is the innermost
        Some((code_index, _)) => code_index < *index,
        None => true,
    })
    .and_then(|(index, desc)| match desc.kind {
        SpanKind::Paragraph => Some(desc.byte_pos),
        SpanKind::Heading(_) => Some(structure.get_span_inner_content(index)),
        SpanKind::ListItem => list_item_text(structure, index),
        _ => None,
    });

    if let Some(context) = sentence_context {
        for (offset, sentence) in text[context.range()].split_sentence_bound_indices() {
            let start = context.start + offset;
            push(trim_span(
                text,
                ByteSpan::new(start, start + sentence.len()),
            ));
        }
    }

    for (index, desc) in structure.iterate_top_level() {
        if matches!(desc.kind, SpanKind::Heading(_)) {
            push(heading_section(text, structure, index));
        }
    }

    push(ByteSpan::new(0, text.len()));

    candidates
}

fn strictly_contains(outer: ByteSpan, inner: ByteSpan) -> bool {
    if inner.is_empty() {
        !outer.is_empty() && touches(outer, inner.start)
    } else {
        outer.start <= inner.start
            && outer.end >= inner.end
            && outer.range().len() > inner.range().len()
    }
}

/// The smallest unit that contains the selection and is larger than it
pub fn expand_selection(
    text: &str,
    structure: &TextStructure,
    selection: ByteSpan,
    head: usize,
) -> Option<ByteSpan> {
    unit_candidates(text, structure, head)
        .into_iter()
        .filter(|candidate| strictly_contains(*candidate, selection))
        .min_by_key(|candidate| candidate.range().len())
}

/// The largest unit inside the selection that contains the head
pub fn shrink_selection(
    text: &str,
    structure: &TextStructure,
    selection: ByteSpan,
    head: usize,
) -> Option<ByteSpan> {
    unit_candidates(text, structure, head)
        .into_iter()
        .filter(|candidate| strictly_contains(selection, *candidate))
        .max_by_key(|candidate| candidate.range().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::text_change_effect::TextChange;

    fn expand_steps(input: &str, steps: usize) -> String {
        let (text, cursor) = TextChange::try_extract_cursor(input.to_string());
        let mut selection = cursor.unwrap();
        let structure = TextStructure::new(&text);
        for _ in 0..steps {
            selection = expand_selection(&text, &structure, selection, selection.end)
                .expect("expected to expand");
        }
        TextChange::encode_cursor(&text, selection.unordered())
    }

    #[test]
    fn test_expand_chain_in_paragraph() {
        let input = "First one. Sec{||}ond **bold** sentence here.\n\nOther paragraph.";
        let expected = [
            "First one. {|}Second{|} **bold** sentence here.\n\nOther paragraph.",
            "First one. {|}Second **bold** sentence here.{|}\n\nOther paragraph.",
            "{|}First one. Second **bold** sentence here.{|}\n\nOther paragraph.",
            "{|}First one. Second **bold** sentence here.\n\nOther paragraph.{|}",
        ];
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(&expand_steps(input, i + 1), expected, "step {}", i + 1);
        }
    }

    #[test]
    fn test_expand_inside_inline_spans() {
        let input = "Some **bo{||}ld text** and more.";
        let expected = [
            "Some **{|}bold{|} text** and more.",
            "Some **{|}bold text{|}** and more.",
            "Some {|}**bold text**{|} and more.",
            "{|}Some **bold text** and more.{|}",
        ];
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(&expand_steps(input, i + 1), expected, "step {}", i + 1);
        }
    }

    #[test]
    fn test_expand_cursor_at_word_end() {
        assert_eq!(expand_steps("hello{||} world", 1), "{|}hello{|} world");
    }

    #[test]
    fn test_expand_in_list() {
        let input = "- [ ] first ta{||}sk. More\n\t* nested\n- second";
        let expected = [
            "- [ ] first {|}task{|}. More\n\t* nested\n- second",
            "- [ ] {|}first task.{|} More\n\t* nested\n- second",
            "- [ ] {|}first task. More{|}\n\t* nested\n- second",
            "- {|}[ ] first task. More\n\t* nested{|}\n- second",
            "{|}- [ ] first task. More\n\t* nested{|}\n- second",
            "{|}- [ ] first task. More\n\t* nested\n- second{|}",
        ];
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(&expand_steps(input, i + 1), expected, "step {}", i + 1);
        }
    }

    #[test]
    fn test_expand_in_code_block() {
        let input = "text\n\n```js\nlet a = 1;\nlet b{||} = 2;\n```\n\nafter";
        let expected = [
            "text\n\n```js\nlet a = 1;\nlet {|}b{|} = 2;\n```\n\nafter",
            "text\n\n```js\nlet a = 1;\n{|}let b = 2;{|}\n```\n\nafter",
            "text\n\n```js\n{|}let a = 1;\nlet b = 2;{|}\n```\n\nafter",
            "text\n\n{|}```js\nlet a = 1;\nlet b = 2;\n```{|}\n\nafter",
        ];
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(&expand_steps(input, i + 1), expected, "step {}", i + 1);
        }
    }

    #[test]
    fn test_expand_to_sections() {
        let input = "# Top\n\n## Sub\n\npara{||}graph\n\n## Other\n\ntext";
        let expected = [
            "# Top\n\n## Sub\n\n{|}paragraph{|}\n\n## Other\n\ntext",
            "# Top\n\n{|}## Sub\n\nparagraph{|}\n\n## Other\n\ntext",
            "{|}# Top\n\n## Sub\n\nparagraph\n\n## Other\n\ntext{|}",
        ];
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(&expand_steps(input, i + 1), expected, "step {}", i + 1);
        }
    }

    #[test]
    fn test_shrink_without_history() {
        let text = "First one. Second here.";
        let structure = TextStructure::new(text);
        let selection = ByteSpan::new(0, text.len());
        // head is at the end of "Second"
        let head = text.find(" here").unwrap();
        let shrunk = shrink_selection(text, &structure, selection, head).unwrap();
        assert_eq!(&text[shrunk.range()], "Second here.");

        let shrunk = shrink_selection(text, &structure, shrunk, head).unwrap();
        assert_eq!(&text[shrunk.range()], "Second");
    }
}

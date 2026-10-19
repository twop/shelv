use smallvec::SmallVec;

use crate::{
    byte_span::{ByteSpan, UnOrderedByteSpan},
    command::TextCommandContext,
    effects::text_change_effect::TextChange,
    text_structure::{SpanIndex, SpanKind, TextStructure},
};

use super::structural_selection::{
    code_block_lines, heading_section, innermost_span_at, list_item_marker, touches, trim_span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
}

/// A movable unit, note that `marker.end == body.start`.
/// Markers stay in place and only bodies are moved around,
/// that way ordered lists keep their numbering.
#[derive(Debug, Clone, Copy)]
struct Slot {
    marker: ByteSpan,
    body: ByteSpan,
}

impl Slot {
    fn plain(body: ByteSpan) -> Self {
        Self {
            marker: ByteSpan::point(body.start),
            body,
        }
    }
}

/// All the siblings at the same level, plus which of them (inclusive) are moving
struct Siblings {
    slots: SmallVec<[Slot; 8]>,
    first: usize,
    last: usize,
}

/// Moves the markdown unit under the cursor (code line, list item, heading section or block)
/// up or down, swapping it with its sibling. The selection moves together with the text.
pub fn move_structure(
    TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: selection,
    }: TextCommandContext,
    direction: MoveDirection,
) -> Option<Vec<TextChange>> {
    let siblings = code_line_siblings(text, structure, selection)
        .or_else(|| list_item_siblings(text, structure, selection))
        .or_else(|| section_siblings(text, structure, selection))
        .or_else(|| top_level_block_siblings(text, structure, selection))?;

    rotate(text, &siblings, direction, selection)
}

fn code_line_siblings(
    text: &str,
    structure: &TextStructure,
    selection: ByteSpan,
) -> Option<Siblings> {
    let (_, block) = innermost_span_at(structure, selection.start, |kind| {
        kind == SpanKind::CodeBlock
    })?;

    if !touches(block.byte_pos, selection.end) {
        return None;
    }

    // note that in case of the cursor being on a fence line that returns None
    let lines = code_block_lines(text, block.byte_pos);
    let line_index = |pos| lines.code.iter().position(|line| touches(*line, pos));
    let (first, last) = (line_index(selection.start)?, line_index(selection.end)?);

    Some(Siblings {
        slots: lines.code.iter().map(|line| Slot::plain(*line)).collect(),
        first,
        last,
    })
}

fn list_item_siblings(
    text: &str,
    structure: &TextStructure,
    selection: ByteSpan,
) -> Option<Siblings> {
    let item_at = |pos| innermost_span_at(structure, pos, |kind| kind == SpanKind::ListItem);
    let (start_item, _) = item_at(selection.start)?;
    let (end_item, _) = item_at(selection.end)?;

    let items_chain = |item: SpanIndex| {
        [item]
            .into_iter()
            .chain(
                structure
                    .iterate_parents_of(item)
                    .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
                    .map(|(index, _)| index),
            )
            .collect::<SmallVec<[SpanIndex; 4]>>()
    };

    // find the innermost pair of items that belong to the same list
    let end_chain = items_chain(end_item);
    let (first_item, last_item) = items_chain(start_item).into_iter().find_map(|a| {
        end_chain
            .iter()
            .find(|b| structure.span(**b).parent == structure.span(a).parent)
            .map(|b| (a, *b))
    })?;

    let list = structure.span(first_item).parent;
    let items: SmallVec<[(SpanIndex, ByteSpan); 8]> = structure
        .iterate_immediate_children_of(list)
        .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
        .map(|(index, desc)| (index, desc.byte_pos))
        .collect();

    let position = |target: SpanIndex| items.iter().position(|(index, _)| *index == target);
    let (first, last) = (position(first_item)?, position(last_item)?);

    Some(Siblings {
        slots: items
            .iter()
            .map(|(_, item)| {
                let marker = list_item_marker(text, *item);
                Slot {
                    marker,
                    body: ByteSpan::new(marker.end, item.end),
                }
            })
            .collect(),
        first: first.min(last),
        last: first.max(last),
    })
}

fn top_level_block_at(
    text: &str,
    structure: &TextStructure,
    pos: usize,
) -> Option<(SpanIndex, SpanKind)> {
    // note that lists and code blocks include the trailing new lines,
    // so without trimming a list would "touch" the start of the paragraph right after it
    structure
        .iterate_top_level()
        .find(|(_, desc)| {
            is_movable_block(desc.kind) && touches(trim_span(text, desc.byte_pos), pos)
        })
        .map(|(index, desc)| (index, desc.kind))
}

fn is_movable_block(kind: SpanKind) -> bool {
    matches!(
        kind,
        SpanKind::Paragraph | SpanKind::Heading(_) | SpanKind::CodeBlock | SpanKind::List
    )
}

/// Cursor on a heading line moves the whole section among the sections of the same level
fn section_siblings(
    text: &str,
    structure: &TextStructure,
    selection: ByteSpan,
) -> Option<Siblings> {
    let (heading, kind) = top_level_block_at(text, structure, selection.start)?;
    let SpanKind::Heading(level) = kind else {
        return None;
    };

    if !touches(structure.span(heading).byte_pos, selection.end) {
        return None;
    }

    let headings: SmallVec<[(SpanIndex, _); 8]> = structure
        .iterate_top_level()
        .filter_map(|(index, desc)| match desc.kind {
            SpanKind::Heading(level) => Some((index, level)),
            _ => None,
        })
        .collect();

    let position = headings.iter().position(|(index, _)| *index == heading)?;

    // siblings are the headings of the same level, between the headings of a higher level (e.g. "#" for "##")
    let parent_start = headings[..position]
        .iter()
        .rposition(|(_, other)| *other < level)
        .map_or(0, |parent| parent + 1);

    let same_level: SmallVec<[SpanIndex; 8]> = headings[parent_start..]
        .iter()
        .take_while(|(_, other)| *other >= level)
        .filter(|(_, other)| *other == level)
        .map(|(index, _)| *index)
        .collect();

    let before = same_level.iter().position(|index| *index == heading)?;

    let slots: SmallVec<[Slot; 8]> = same_level
        .iter()
        .map(|index| Slot::plain(heading_section(text, structure, *index)))
        .collect();

    Some(Siblings {
        slots,
        first: before,
        last: before,
    })
}

fn top_level_block_siblings(
    text: &str,
    structure: &TextStructure,
    selection: ByteSpan,
) -> Option<Siblings> {
    let (start_block, _) = top_level_block_at(text, structure, selection.start)?;
    let (end_block, _) = top_level_block_at(text, structure, selection.end)?;

    let blocks: SmallVec<[(SpanIndex, ByteSpan); 8]> = structure
        .iterate_top_level()
        .filter(|(_, desc)| is_movable_block(desc.kind))
        .map(|(index, desc)| (index, desc.byte_pos))
        .collect();

    let position = |target: SpanIndex| blocks.iter().position(|(index, _)| *index == target);

    Some(Siblings {
        slots: blocks
            .iter()
            .map(|(_, block)| Slot::plain(trim_span(text, *block)))
            .collect(),
        first: position(start_block)?,
        last: position(end_block)?,
    })
}

/// Swaps the moving slots with their neighbour, returns None if there is nothing to swap with
fn rotate(
    text: &str,
    Siblings { slots, first, last }: &Siblings,
    direction: MoveDirection,
    selection: ByteSpan,
) -> Option<Vec<TextChange>> {
    let (first, last) = (*first, *last);
    let (low, high) = match direction {
        MoveDirection::Up => (first.checked_sub(1)?, last),
        MoveDirection::Down if last + 1 < slots.len() => (first, last + 1),
        MoveDirection::Down => return None,
    };

    // a detector bug shouldn't crash the app, a noop is fine
    if first > last || high >= slots.len() {
        return None;
    }

    // trailing spaces of the last line belong to the unit, not to the gap between units,
    // otherwise they would end up after a different unit
    let region: SmallVec<[Slot; 8]> = slots[low..=high]
        .iter()
        .map(|slot| {
            let trailing_spaces = text[slot.body.end..]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            Slot {
                body: ByteSpan::new(slot.body.start, slot.body.end + trailing_spaces),
                ..*slot
            }
        })
        .collect();
    let count = region.len();

    let gap = |slot: usize| ByteSpan::new(region[slot].body.end, region[slot + 1].marker.start);

    // anything other than whitespace between the units means that there is a structure we don't know about,
    // e.g. a blockquote or a table, so don't touch it
    if (0..count - 1).any(|i| !text[gap(i).range()].chars().all(char::is_whitespace)) {
        return None;
    }

    // `source[t]` is the body that ends up in slot `t`
    let source: SmallVec<[usize; 8]> = match direction {
        MoveDirection::Up => (1..count).chain([0]).collect(),
        MoveDirection::Down => [count - 1].into_iter().chain(0..count - 1).collect(),
    };

    let mut destination: SmallVec<[usize; 8]> = SmallVec::from_elem(0, count);
    let mut body_start: SmallVec<[usize; 8]> = SmallVec::from_elem(0, count);
    let mut marker_start: SmallVec<[usize; 8]> = SmallVec::from_elem(0, count);

    let mut result = String::new();
    for t in 0..count {
        marker_start[t] = result.len();
        result.push_str(&text[region[t].marker.range()]);

        let s = source[t];
        destination[s] = t;
        body_start[s] = result.len();
        result.push_str(&text[region[s].body.range()]);

        if t + 1 < count {
            result.push_str(&text[gap(t).range()]);
        }
    }

    // maps an absolute position to a position inside `result`
    let map = |pos: usize| -> usize {
        for s in 0..count {
            let Slot { marker, body } = region[s];
            let dest = destination[s];

            if touches(body, pos) {
                return body_start[s] + (pos - body.start);
            }

            if marker.contains_pos(pos) {
                // follow the body, e.g. a cursor at the beginning of a list item
                let dest_marker_len = region[dest].marker.range().len();
                return marker_start[dest] + (pos - marker.start).min(dest_marker_len);
            }

            if s + 1 < count && gap(s).contains_pos(pos) {
                let dest_gap_len = if dest + 1 < count {
                    gap(dest).range().len()
                } else {
                    0
                };
                return body_start[s] + body.range().len() + (pos - body.end).min(dest_gap_len);
            }
        }

        // shouldn't happen, the selection is always within the moving units
        pos.clamp(region[0].marker.start, region[count - 1].body.end) - region[0].marker.start
    };

    let cursor = UnOrderedByteSpan::new(map(selection.start), map(selection.end));

    Some(vec![TextChange::Insert(
        ByteSpan::new(region[0].marker.start, region[count - 1].body.end),
        TextChange::encode_cursor(&result, cursor),
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::text_change_effect::apply_text_changes;

    fn run(input: &str, direction: MoveDirection) -> String {
        let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
        let cursor = cursor.unwrap();
        let structure = TextStructure::new(&text);

        let Some(changes) = move_structure(
            TextCommandContext::new(&structure, &text, cursor),
            direction,
        ) else {
            return TextChange::encode_cursor(&text, cursor.unordered());
        };

        let cursor = apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();
        TextChange::encode_cursor(&text, cursor.unwrap())
    }

    use MoveDirection::{Down, Up};

    #[test]
    fn test_move_structure() {
        let cases = [
            (
                "list item down",
                "- fi{||}rst\n- second\n- third",
                Down,
                "- second\n- fi{||}rst\n- third",
            ),
            (
                "list item up, cursor at the line start follows the item",
                "- first\n{||}- second\n- third",
                Up,
                "{||}- second\n- first\n- third",
            ),
            (
                "last list item down is a noop",
                "- first\n- sec{||}ond",
                Down,
                "- first\n- sec{||}ond",
            ),
            (
                "ordered list keeps numbering",
                "1. one{||}\n2. two\n3. three",
                Down,
                "1. two\n2. one{||}\n3. three",
            ),
            (
                "nested children travel with the item",
                "- a{||}\n\t* a1\n\t* a2\n- b",
                Down,
                "- b\n- a{||}\n\t* a1\n\t* a2",
            ),
            (
                "nested item moves within its list",
                "- a\n\t* a1\n\t* a2{||}\n- b",
                Up,
                "- a\n\t* a2{||}\n\t* a1\n- b",
            ),
            (
                "task items",
                "- [ ] one\n- [x] two{||}",
                Up,
                "- [x] two{||}\n- [ ] one",
            ),
            (
                "multiple selected items move together",
                "- {|}a\n- b{|}\n- c",
                Down,
                "- c\n- {|}a\n- b{|}",
            ),
            (
                "paragraph swaps with the next paragraph",
                "fir{||}st\n\nsecond",
                Down,
                "second\n\nfir{||}st",
            ),
            (
                "paragraph swaps with a whole list",
                "- a\n- b\n\npara{||}graph",
                Up,
                "para{||}graph\n\n- a\n- b",
            ),
            (
                "cursor at the start of a paragraph right after a list moves the paragraph",
                "# Tools\n- a\n\t* a1 \n\n{||}paragraph\n\n## Next",
                Up,
                "# Tools\n{||}paragraph\n\n- a\n\t* a1 \n\n## Next",
            ),
            (
                "paragraph swaps with a code block",
                "para{||}\n\n```js\nlet a = 1;\n```\n",
                Down,
                "```js\nlet a = 1;\n```\n\npara{||}\n",
            ),
            (
                "code line moves inside the block",
                "```js\nlet a = 1;\nlet b{||} = 2;\n```",
                Up,
                "```js\nlet b{||} = 2;\nlet a = 1;\n```",
            ),
            (
                "code line stops at the fence",
                "```js\nlet a{||} = 1;\nlet b = 2;\n```",
                Up,
                "```js\nlet a{||} = 1;\nlet b = 2;\n```",
            ),
            (
                "fence line moves the whole block",
                "before\n\n```js{||}\nlet a = 1;\n```",
                Up,
                "```js{||}\nlet a = 1;\n```\n\nbefore",
            ),
            (
                "heading moves the whole section",
                "# Top\n\n## A{||}\n\ntext a\n\n### A1\n\nsub\n\n## B\n\ntext b",
                Down,
                "# Top\n\n## B\n\ntext b\n\n## A{||}\n\ntext a\n\n### A1\n\nsub",
            ),
            (
                "section moves up past a section with subsections",
                "## A\n\n### A1\n\nsub\n\n## B{||}\n\ntext b",
                Up,
                "## B{||}\n\ntext b\n\n## A\n\n### A1\n\nsub",
            ),
            (
                "subsection moves only within its parent section",
                "## A\n\n### A1\n\n### A2{||}\n\n## B\n\n### B1",
                Up,
                "## A\n\n### A2{||}\n\n### A1\n\n## B\n\n### B1",
            ),
            (
                "last subsection doesn't escape its parent section",
                "## A\n\n### A1{||}\n\n## B\n\n### B1",
                Down,
                "## A\n\n### A1{||}\n\n## B\n\n### B1",
            ),
            (
                "first section of its parent doesn't move up",
                "# Top\n\nintro\n\n## A{||}\n\ntext a",
                Up,
                "# Top\n\nintro\n\n## A{||}\n\ntext a",
            ),
            (
                "unknown structure in between is not touched",
                "para{||}\n\n> quote\n\nother",
                Down,
                "para{||}\n\n> quote\n\nother",
            ),
        ];

        for (desc, input, direction, expected) in cases {
            assert_eq!(run(input, direction), expected, "case: {desc}");
        }
    }
}

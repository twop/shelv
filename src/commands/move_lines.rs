use crate::{
    byte_span::{ByteSpan, UnOrderedByteSpan},
    command::TextCommandContext,
    effects::text_change_effect::TextChange,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
}

/// Swaps the textual (not visual) lines touched by the selection with the line above or below,
/// the selection then covers the moved lines, so that they can be moved again
pub fn move_lines(
    TextCommandContext {
        text,
        byte_cursor: selection,
        ..
    }: TextCommandContext,
    direction: MoveDirection,
) -> Option<Vec<TextChange>> {
    let line_start = |pos: usize| text[..pos].rfind('\n').map_or(0, |i| i + 1);
    let line_end = |pos: usize| text[pos..].find('\n').map_or(text.len(), |i| pos + i);

    // the line right after a selected line break is not selected
    let last_pos = if !selection.is_empty() && line_start(selection.end) == selection.end {
        selection.end - 1
    } else {
        selection.end
    };

    let lines = ByteSpan::new(line_start(selection.start), line_end(last_pos));
    let block = &text[lines.range()];

    let (replaced, replacement, block_start) = match direction {
        MoveDirection::Up => {
            if lines.start == 0 {
                return None;
            }
            let prev = ByteSpan::new(line_start(lines.start - 1), lines.start - 1);
            (
                ByteSpan::new(prev.start, lines.end),
                format!("{block}\n{}", &text[prev.range()]),
                0,
            )
        }
        MoveDirection::Down => {
            if lines.end == text.len() {
                return None;
            }
            let next = ByteSpan::new(lines.end + 1, line_end(lines.end + 1));
            let next_text = &text[next.range()];
            (
                ByteSpan::new(lines.start, next.end),
                format!("{next_text}\n{block}"),
                next_text.len() + 1,
            )
        }
    };

    let cursor = UnOrderedByteSpan::new(block_start, block_start + block.len());

    Some(vec![TextChange::Insert(
        replaced,
        TextChange::encode_cursor(&replacement, cursor),
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{effects::text_change_effect::apply_text_changes, text_structure::TextStructure};

    fn run(input: &str, direction: MoveDirection) -> String {
        let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
        let cursor = cursor.unwrap();
        let structure = TextStructure::new(&text);

        let Some(changes) = move_lines(TextCommandContext::new(&structure, &text, cursor), direction)
        else {
            return TextChange::encode_cursor(&text, cursor.unordered());
        };

        let cursor = apply_text_changes(&mut text, Some(cursor.unordered()), changes).unwrap();
        TextChange::encode_cursor(&text, cursor.unwrap())
    }

    use MoveDirection::{Down, Up};

    #[test]
    fn test_move_lines() {
        let cases = [
            (
                "line down selects the moved line",
                "fi{||}rst\nsecond\nthird",
                Down,
                "second\n{|}first{|}\nthird",
            ),
            (
                "line up selects the moved line",
                "first\nsec{||}ond\nthird",
                Up,
                "{|}second{|}\nfirst\nthird",
            ),
            (
                "first line up is a noop",
                "fi{||}rst\nsecond",
                Up,
                "fi{||}rst\nsecond",
            ),
            (
                "last line down is a noop",
                "first\nsec{||}ond",
                Down,
                "first\nsec{||}ond",
            ),
            (
                "last line before a trailing new line moves down past the empty line",
                "first\nsec{||}ond\n",
                Down,
                "first\n\n{|}second{|}",
            ),
            (
                "multiple selected lines move together",
                "a\nb{|}b\nc{|}c\nd",
                Up,
                "{|}bb\ncc{|}\na\nd",
            ),
            (
                "selection ending at the start of a line doesn't include that line",
                "{|}a\n{|}b\nc",
                Down,
                "b\n{|}a{|}\nc",
            ),
            (
                "moved lines can be moved again",
                "a\n{|}b\nc{|}\nd",
                Down,
                "a\nd\n{|}b\nc{|}",
            ),
            (
                "list markers and numbers travel with the line",
                "1. one{||}\n2. two\n\t- nested",
                Down,
                "2. two\n{|}1. one{|}\n\t- nested",
            ),
            (
                "markdown structure is ignored",
                "```js\nlet a{||} = 1;\n```",
                Up,
                "{|}let a = 1;{|}\n```js\n```",
            ),
            (
                "empty line moves",
                "a\n{||}\nb",
                Up,
                "{||}\na\nb",
            ),
        ];

        for (desc, input, direction, expected) in cases {
            assert_eq!(run(input, direction), expected, "case: {desc}");
        }
    }
}

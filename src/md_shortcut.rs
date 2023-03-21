use std::ops::Range;

use eframe::egui::KeyboardShortcut;

use crate::text_structure::SpanKind;

pub struct MdAnnotationShortcut {
    pub name: &'static str,
    pub shortcut: KeyboardShortcut,
    pub instruction: Instruction,
    pub target_span: SpanKind,
}

pub enum Edge {
    Start,
    End,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Source {
    Selection,
    BeforeSelection,
    AfterSelection,
    SurroundingSpanContent(SpanKind),
}

pub enum InstructionCondition {
    IsInside(SpanKind),
    IsInsideUnmarkedArea,
    IsNoneOrEmpty(Source),
    EndsWith(Source, &'static str),
    StartsWith(Source, &'static str),
    Any(Vec<InstructionCondition>),
    // Any(Vec<InstructionCondition>),
}

pub enum Instruction {
    Insert(&'static str),
    PlaceCursor(Edge),
    SetReplaceArea(SpanKind),
    CopyFrom(Source),
    Seq(Vec<Instruction>),
    Condition {
        cond: InstructionCondition,
        if_true: Box<Instruction>,
        if_false: Box<Instruction>,
    },
    MatchFirst(Vec<(InstructionCondition, Instruction)>),
}

impl Instruction {
    pub fn condition(
        cond: InstructionCondition,
        if_true: Instruction,
        if_false: Instruction,
    ) -> Instruction {
        Self::Condition {
            cond,
            if_true: Box::new(if_true),
            if_false: Box::new(if_false),
        }
    }

    pub fn sequence(seq: impl IntoIterator<Item = Instruction>) -> Instruction {
        Self::Seq(seq.into_iter().collect())
    }
}

pub trait ShortcutContext<'a> {
    fn get_source(&self, source: Source) -> Option<&'a str>;
    fn is_inside_span(&self, kind: SpanKind) -> bool;
    fn is_inside_unmarked(&self) -> bool;
    fn set_replace_area(&mut self, kind: SpanKind);
}

#[derive(Debug, PartialEq)]
pub struct ShortcutResult {
    pub content: String,
    pub relative_char_cursor: Range<usize>,
}

struct EvalState {
    cursor_start: usize,
    cursor_end: usize,
    chars_inserted: usize,
    bytes_inserted: usize,
    result: String,
}

fn eval_instruction<'a>(
    eval_state: &mut EvalState,
    cx: &mut impl ShortcutContext<'a>,
    instruction: &Instruction,
) -> Option<()> {
    let EvalState {
        cursor_start,
        cursor_end,
        chars_inserted,
        bytes_inserted,
        result,
    } = eval_state;

    match instruction {
        Instruction::Insert(s) => {
            result.insert_str(*bytes_inserted, s);
            *bytes_inserted += s.as_bytes().len();
            *chars_inserted += s.chars().count()
        }
        Instruction::CopyFrom(source) => {
            let source_str = cx.get_source(*source)?;

            result.insert_str(*bytes_inserted, source_str);
            *bytes_inserted += source_str.as_bytes().len();
            *chars_inserted += source_str.chars().count()
        }
        Instruction::PlaceCursor(edge) => match edge {
            Edge::Start => *cursor_start = *chars_inserted,
            Edge::End => *cursor_end = *chars_inserted,
        },
        Instruction::Seq(seq) => {
            for instruction in seq {
                eval_instruction(eval_state, cx, instruction)?;
            }
        }
        Instruction::Condition {
            cond,
            if_true,
            if_false,
        } => {
            let cond_res = eval_condition(cond, cx)?;

            eval_instruction(
                eval_state,
                cx,
                match cond_res {
                    true => if_true.as_ref(),
                    false => if_false.as_ref(),
                },
            )?;
        }
        Instruction::MatchFirst(pairs) => {
            pairs
                .iter()
                .find(|(cond, _)| eval_condition(cond, cx).unwrap_or(false))
                .and_then(|(_, instruction)| eval_instruction(eval_state, cx, instruction));
        }

        Instruction::SetReplaceArea(kind) => cx.set_replace_area(*kind),
    }

    Some(())
}

fn eval_condition<'a>(cond: &InstructionCondition, cx: &impl ShortcutContext<'a>) -> Option<bool> {
    match cond {
        InstructionCondition::IsNoneOrEmpty(source) => {
            Some(cx.get_source(*source).unwrap_or("") == "")
        }
        InstructionCondition::EndsWith(source, pattern) => {
            Some(cx.get_source(*source)?.ends_with(pattern))
        }
        InstructionCondition::StartsWith(source, pattern) => {
            Some(cx.get_source(*source)?.starts_with(pattern))
        }
        InstructionCondition::Any(conditions) => {
            for c in conditions {
                if eval_condition(c, cx)? {
                    return Some(true);
                }
            }
            Some(false)
        }

        InstructionCondition::IsInside(kind) => Some(cx.is_inside_span(*kind)),
        InstructionCondition::IsInsideUnmarkedArea => Some(cx.is_inside_unmarked()),
    }
}

pub fn execute_instruction<'a>(
    cx: &mut impl ShortcutContext<'a>,
    instruction: &Instruction,
) -> Option<ShortcutResult> {
    let mut eval_state = EvalState {
        cursor_start: 0,
        cursor_end: 0,
        chars_inserted: 0,
        bytes_inserted: 0,
        result: "".to_string(),
    };

    eval_instruction(&mut eval_state, cx, instruction)?;

    Some(ShortcutResult {
        content: eval_state.result,
        relative_char_cursor: eval_state.cursor_start..eval_state.cursor_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestShortcutContext<'a> {
        sources: Vec<(Source, &'a str)>,
        inside: Vec<SpanKind>,
        replace_area: Option<SpanKind>,
    }

    impl<'a> TestShortcutContext<'a> {
        fn new(sources: impl IntoIterator<Item = (Source, &'a str)>) -> Self {
            Self {
                sources: sources.into_iter().collect(),
                inside: vec![],
                replace_area: None,
            }
        }
    }

    impl<'a> ShortcutContext<'a> for TestShortcutContext<'a> {
        fn get_source(&self, source: Source) -> Option<&'a str> {
            self.sources
                .iter()
                .find(|(s, _)| s == &source)
                .map(|(_, string)| *string)
        }

        fn is_inside_span(&self, kind: SpanKind) -> bool {
            self.inside.iter().any(|k| *k == kind)
        }

        fn set_replace_area(&mut self, kind: SpanKind) {
            self.replace_area = Some(kind);
        }

        fn is_inside_unmarked(&self) -> bool {
            // TODO fill it out for tests
            false
        }
    }

    #[test]
    pub fn test_bold() {
        use Instruction::*;

        let bold = Condition {
            cond: InstructionCondition::IsNoneOrEmpty(Source::Selection),
            if_true: Box::new(Seq(vec![
                PlaceCursor(Edge::Start),
                Insert("**"),
                CopyFrom(Source::Selection),
                Insert("**"),
                PlaceCursor(Edge::End),
            ])),
            if_false: Box::new(Seq(vec![
                Insert("**"),
                PlaceCursor(Edge::Start),
                PlaceCursor(Edge::End),
                Insert("**"),
            ])),
        };

        assert_eq!(
            execute_instruction(
                &mut TestShortcutContext::new([(Source::Selection, "bold")]),
                &bold,
            ),
            Some(ShortcutResult {
                content: "**bold**".to_string(),
                relative_char_cursor: 0..8
            }),
        );

        assert_eq!(
            execute_instruction(&mut TestShortcutContext::new([]), &bold,),
            Some(ShortcutResult {
                content: "****".to_string(),
                relative_char_cursor: 2..2
            }),
        );
    }

    #[test]
    pub fn test_end_with_condition() {
        use Instruction::*;

        let newline_cond = Condition {
            cond: InstructionCondition::EndsWith(Source::BeforeSelection, "\n"),
            if_true: Box::new(Insert("newline")),
            if_false: Box::new(Insert("no newline")),
        };

        assert_eq!(
            execute_instruction(
                &mut TestShortcutContext::new([(Source::BeforeSelection, "\n")]),
                &newline_cond,
            ),
            Some(ShortcutResult {
                content: "newline".to_string(),
                relative_char_cursor: 0..0
            }),
        );

        assert_eq!(
            execute_instruction(
                &mut TestShortcutContext::new([(Source::BeforeSelection, "just space ")]),
                &newline_cond,
            ),
            Some(ShortcutResult {
                content: "no newline".to_string(),
                relative_char_cursor: 0..0
            }),
        );
    }
}

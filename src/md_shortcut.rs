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

#[derive(Debug, Clone, Copy)]
pub enum Source {
    Selection,
    BeforeSelection,
    AfterSelection,
}

pub enum InstructionCondition {
    IsNoneOrEmpty(Source),
    EndsWith(Source, &'static str),
    StartsWith(Source, &'static str),
    EitherOne(Vec<InstructionCondition>),
}

pub enum Instruction {
    Insert(&'static str),
    PlaceCursor(Edge),
    CopyFrom(Source),
    Seq(Vec<Instruction>),
    Condition {
        cond: InstructionCondition,
        if_true: Box<Instruction>,
        if_false: Box<Instruction>,
    },
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

#[derive(Debug)]
pub struct ShortcutContext<'a> {
    pub selection: Option<&'a str>,
    pub before_selection: Option<&'a str>,
    pub after_selection: Option<&'a str>,
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
    cx: &ShortcutContext<'a>,
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
            let source_str = select_source(*source, cx)?;

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
    }

    Some(())
}

fn eval_condition(cond: &InstructionCondition, cx: &ShortcutContext) -> Option<bool> {
    match cond {
        InstructionCondition::IsNoneOrEmpty(source) => {
            Some(select_source(*source, cx).unwrap_or("") == "")
        }
        InstructionCondition::EndsWith(source, pattern) => {
            Some(select_source(*source, cx)?.ends_with(pattern))
        }
        InstructionCondition::StartsWith(source, pattern) => {
            Some(select_source(*source, cx)?.starts_with(pattern))
        }
        InstructionCondition::EitherOne(conditions) => {
            for c in conditions {
                if eval_condition(c, cx)? {
                    return Some(true);
                }
            }
            Some(false)
        }
    }
}

fn select_source<'a>(source: Source, cx: &'a ShortcutContext) -> Option<&'a str> {
    Some(match source {
        Source::Selection => cx.selection?,
        Source::BeforeSelection => cx.before_selection?,
        Source::AfterSelection => cx.after_selection?,
    })
}

pub fn execute_instruction<'a>(
    cx: ShortcutContext<'a>,
    instruction: &Instruction,
) -> Option<ShortcutResult> {
    let mut eval_state = EvalState {
        cursor_start: 0,
        cursor_end: 0,
        chars_inserted: 0,
        bytes_inserted: 0,
        result: "".to_string(),
    };

    eval_instruction(&mut eval_state, &cx, instruction)?;

    Some(ShortcutResult {
        content: eval_state.result,
        relative_char_cursor: eval_state.cursor_start..eval_state.cursor_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
                ShortcutContext {
                    selection: Some("bold"),
                    before_selection: None,
                    after_selection: None
                },
                &bold,
            ),
            Some(ShortcutResult {
                content: "**bold**".to_string(),
                relative_char_cursor: 0..8
            }),
        );

        assert_eq!(
            execute_instruction(
                ShortcutContext {
                    selection: None,
                    before_selection: None,
                    after_selection: None
                },
                &bold,
            ),
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
                ShortcutContext {
                    selection: None,
                    before_selection: Some("\n"),
                    after_selection: None
                },
                &newline_cond,
            ),
            Some(ShortcutResult {
                content: "newline".to_string(),
                relative_char_cursor: 0..0
            }),
        );

        assert_eq!(
            execute_instruction(
                ShortcutContext {
                    selection: None,
                    before_selection: Some("just space "),
                    after_selection: None
                },
                &newline_cond,
            ),
            Some(ShortcutResult {
                content: "no newline".to_string(),
                relative_char_cursor: 0..0
            }),
        );
    }
}

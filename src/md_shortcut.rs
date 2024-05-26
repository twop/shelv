use std::ops::Range;

use eframe::egui::KeyboardShortcut;

use crate::{
    app_actions::TextChange,
    byte_span::ByteSpan,
    commands::{EditorCommand, EditorCommandContext},
    text_structure::{SpanKind, TextStructure},
};

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
    // pub relative_byte_cursor: ByteRange,
}

struct EvalState {
    // bytes_inserted: usize,
    result: String,
}

pub fn handle_md_annotation_command(
    md_shortcut: &MdAnnotationShortcut,
    context: EditorCommandContext,
) -> Option<Vec<crate::app_actions::TextChange>> {
    let EditorCommandContext {
        text_structure,
        text,
        byte_cursor,
    } = context;

    let span = text_structure
        .find_span_at(md_shortcut.target_span, byte_cursor.clone())
        .map(|(span_range, idx)| (span_range, text_structure.get_span_inner_content(idx)));

    match span {
        Some((span_byte_range, content_byte_range)) => {
            // we need to remove the annotations because it is already annotated
            // for example: if it is already "bold" then remove "**" on each side

            Some(vec![TextChange::Replace(
                span_byte_range,
                TextChange::CURSOR_EDGE.to_string()
                    + &text[content_byte_range.range()]
                    + TextChange::CURSOR_EDGE,
            )])
        }
        None => {
            // means that we need to execute instruction for the shortcut, presumably to add annotations
            let mut cx = ShortcutExecContext {
                structure: &text_structure,
                text,
                selection_byte_range: byte_cursor.clone(),
                replace_range: byte_cursor.clone(),
            };

            execute_instruction(&mut cx, &md_shortcut.instruction)
                .map(|result| vec![TextChange::Replace(cx.replace_range, result.content)])
        }
    }
}

#[derive(Debug)]
struct ShortcutExecContext<'a> {
    structure: &'a TextStructure,
    text: &'a str,
    selection_byte_range: ByteSpan,
    replace_range: ByteSpan,
}

impl<'a> ShortcutContext<'a> for ShortcutExecContext<'a> {
    fn get_source(&self, source: Source) -> Option<&'a str> {
        match source {
            Source::Selection => {
                if self.selection_byte_range.is_empty() {
                    None
                } else {
                    self.text.get(self.selection_byte_range.range())
                }
            }
            Source::BeforeSelection => self.text.get(0..self.selection_byte_range.start),
            Source::AfterSelection => self.text.get(self.selection_byte_range.end..),
            Source::SurroundingSpanContent(kind) => self
                .structure
                .find_span_at(kind, self.selection_byte_range)
                .map(|(_, index)| self.structure.get_span_inner_content(index))
                .and_then(|content| self.text.get(content.range())),
        }
    }

    fn is_inside_span(&self, kind: SpanKind) -> bool {
        self.structure
            .find_span_at(kind, self.selection_byte_range)
            .is_some()
    }

    fn set_replace_area(&mut self, kind: SpanKind) {
        if let Some((range, _)) = self.structure.find_span_at(kind, self.selection_byte_range) {
            self.replace_range = range;
        }
    }

    fn is_inside_unmarked(&self) -> bool {
        self.structure
            .find_any_span_at(self.selection_byte_range)
            .is_none()
    }
}

fn eval_instruction<'a>(
    eval_state: &mut EvalState,
    cx: &mut impl ShortcutContext<'a>,
    instruction: &Instruction,
) -> Option<()> {
    let EvalState {
        // cursor_start,
        // cursor_end,
        // chars_inserted,
        // bytes_inserted,
        result,
    } = eval_state;

    match instruction {
        Instruction::Insert(s) => {
            result.push_str(s);
        }
        Instruction::CopyFrom(source) => {
            let source_str = cx.get_source(*source)?;
            result.push_str(source_str);
        }
        Instruction::PlaceCursor(edge) => match edge {
            Edge::Start | Edge::End => {
                result.push_str(TextChange::CURSOR_EDGE);
            }
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
        result: "".to_string(),
    };

    eval_instruction(&mut eval_state, cx, instruction)?;

    Some(ShortcutResult {
        content: eval_state.result,
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
            if_false: Box::new(Seq(vec![
                PlaceCursor(Edge::Start),
                Insert("**"),
                CopyFrom(Source::Selection),
                Insert("**"),
                PlaceCursor(Edge::End),
            ])),
            if_true: Box::new(Seq(vec![
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
                content: "{|}**bold**{|}".to_string(),
            }),
        );

        assert_eq!(
            execute_instruction(&mut TestShortcutContext::new([]), &bold,),
            Some(ShortcutResult {
                content: "**{|}{|}**".to_string(),
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
            }),
        );

        assert_eq!(
            execute_instruction(
                &mut TestShortcutContext::new([(Source::BeforeSelection, "just space ")]),
                &newline_cond,
            ),
            Some(ShortcutResult {
                content: "no newline".to_string(),
            }),
        );
    }
}

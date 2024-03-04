use std::sync::{mpsc::Receiver, Arc};

use eframe::{
    egui::{self, text_edit::CCursorRange, Key, KeyboardShortcut, Modifiers, Ui},
    epaint::Galley,
};
use pulldown_cmark::HeadingLevel;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    app_actions::{EnterInsideListCommand, ShiftTabInsideListCommand, TabInsideListCommand},
    commands::EditorCommand,
    md_shortcut::{Edge, Instruction, InstructionCondition, MdAnnotationShortcut, Source},
    persistent_state::PersistentState,
    text_structure::{SpanKind, TextStructure},
    theme::AppTheme,
};

pub struct Note {
    pub text: String,
    pub shortcut: KeyboardShortcut,
    pub cursor: Option<CCursorRange>,
}

pub struct AppState {
    pub notes: Vec<Note>,
    pub selected_note: u32,
    pub save_to_storage: bool,
    pub is_settings_opened: bool,

    pub theme: AppTheme,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    pub msg_queue: Receiver<MsgToApp>,
    pub hidden: bool,
    pub prev_focused: bool,
    pub md_annotation_shortcuts: Vec<MdAnnotationShortcut>,
    pub app_shortcuts: AppShortcuts,
    pub editor_commands: Vec<Box<dyn EditorCommand>>,

    pub computed_layout: Option<ComputedLayout>,
    pub font_scale: i32,
}

pub struct AppShortcuts {
    bold: KeyboardShortcut,
    emphasize: KeyboardShortcut,
    strikethrough: KeyboardShortcut,
    code_block: KeyboardShortcut,
    h1: KeyboardShortcut,
    h2: KeyboardShortcut,
    h3: KeyboardShortcut,
    pub increase_font: KeyboardShortcut,
    pub decrease_font: KeyboardShortcut,
    // h4: KeyboardShortcut,
}

pub struct ComputedLayout {
    pub galley: Arc<Galley>,
    pub wrap_width: f32,
    pub text_structure: TextStructure,
    pub computed_for: String, // maybe use hash not to double store the string content?
    pub font_size: i32,
}

impl ComputedLayout {
    pub fn should_recompute(&self, text: &str, max_width: f32, font_size: i32) -> bool {
        // TODO might want to check for any changes to theme, not just font_size
        self.wrap_width != max_width || self.computed_for != text || self.font_size != font_size
    }

    pub fn compute(
        text: &str,
        wrap_width: f32,
        ui: &Ui,
        font_size: i32,
        theme: &AppTheme,
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
    ) -> Self {
        let text_structure = TextStructure::create_from(text);

        let mut job = text_structure.create_layout_job(text, theme, syntax_set, theme_set);

        job.wrap.max_width = wrap_width;

        let galley = ui.fonts(|f| f.layout_job(job));

        Self {
            galley,
            wrap_width,
            font_size,
            text_structure,
            computed_for: text.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum MsgToApp {
    ToggleVisibility,
}

// struct MdAnnotationShortcut {
//     name: &'static str,
//     annotation: &'static str,
//     shortcut: KeyboardShortcut,
// }

impl AppState {
    pub fn new(init_data: AppInitData) -> Self {
        let AppInitData {
            theme,
            msg_queue,
            persistent_state,
        } = init_data;

        use Instruction::*;
        use InstructionCondition::*;

        let app_shortcuts = AppShortcuts {
            bold: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::B),
            emphasize: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::I),
            strikethrough: KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                egui::Key::E,
            ),
            code_block: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::C),
            h1: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num1),
            h2: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num2),
            h3: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num3),
            // h4: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num4),
            increase_font: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::PlusEquals),
            decrease_font: KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::Minus),
        };

        let notes_count = 4;

        let (selected_note, mut notes) = persistent_state
            .map(|s| (s.selected_note, s.notes))
            .unwrap_or_else(|| (0, (0..notes_count).map(|i| format!("{i}")).collect()));

        let restored_notes_count = notes.len();
        if restored_notes_count != notes_count {
            for i in restored_notes_count..notes_count {
                notes.push(format!("{i}"));
            }
        }

        let notes: Vec<Note> = notes
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let key = number_to_key(index as u8 + 1).unwrap();
                Note {
                    text,
                    shortcut: KeyboardShortcut::new(Modifiers::COMMAND, key),
                    cursor: None,
                }
            })
            .collect();

        //             note: "# title
        // - adsd
        // - fdsf
        // 	- [ ] fdsf
        // 	- [x] fdsf
        // 1. fa
        // 2. fdsf
        // 3.
        // bo**dy**
        // i*tali*c
        // https://www.nordtheme.com/docs/colors-and-palettes
        // ```rs
        // let a = Some(115);
        // ```"
        // .to_string(),

        Self {
            is_settings_opened: false,
            save_to_storage: false,
            theme,
            notes,
            computed_layout: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            msg_queue,
            selected_note,
            hidden: false,
            prev_focused: false,
            font_scale: 0,
            editor_commands: vec![
                Box::new(TabInsideListCommand),
                Box::new(EnterInsideListCommand),
                Box::new(ShiftTabInsideListCommand),
            ],
            md_annotation_shortcuts: [
                ("Bold", "**", app_shortcuts.bold, SpanKind::Bold),
                ("Italic", "*", app_shortcuts.emphasize, SpanKind::Emphasis),
                (
                    "Strikethrough",
                    "~~",
                    app_shortcuts.strikethrough,
                    SpanKind::Strike,
                ),
            ]
            .map(
                |(name, annotation, shortcut, target_span)| MdAnnotationShortcut {
                    name,
                    shortcut,
                    instruction: Condition {
                        cond: IsNoneOrEmpty(Source::Selection),
                        if_true: Box::new(Seq(vec![
                            Insert(annotation),
                            PlaceCursor(Edge::Start),
                            PlaceCursor(Edge::End),
                            Insert(annotation),
                        ])),
                        if_false: Box::new(Seq(vec![
                            PlaceCursor(Edge::Start),
                            Insert(annotation),
                            CopyFrom(Source::Selection),
                            Insert(annotation),
                            PlaceCursor(Edge::End),
                        ])),
                    },
                    target_span,
                },
            )
            .into_iter()
            .chain(std::iter::once(MdAnnotationShortcut {
                name: "Code Block",
                shortcut: app_shortcuts.code_block,
                instruction: Instruction::sequence([
                    Instruction::condition(
                        // add new line prior if we start in the middle of the text
                        Any(vec![
                            IsNoneOrEmpty(Source::BeforeSelection),
                            EndsWith(Source::BeforeSelection, "\n"),
                        ]),
                        Insert(""),
                        Insert("\n"),
                    ),
                    Insert("```"),
                    PlaceCursor(Edge::Start),
                    PlaceCursor(Edge::End),
                    Insert("\n"),
                    Instruction::condition(
                        IsNoneOrEmpty(Source::Selection),
                        Insert(""),
                        CopyFrom(Source::Selection),
                    ),
                    Instruction::condition(
                        Any(vec![
                            IsNoneOrEmpty(Source::Selection),
                            EndsWith(Source::Selection, "\n"),
                        ]),
                        Insert(""),
                        Insert("\n"),
                    ),
                    Insert("```"),
                    Instruction::condition(
                        Any(vec![
                            IsNoneOrEmpty(Source::AfterSelection),
                            StartsWith(Source::AfterSelection, "\n"),
                        ]),
                        Insert(""),
                        Insert("\n"),
                    ),
                ]),
                target_span: SpanKind::CodeBlock,
            }))
            .chain(
                [
                    ("H1", "#", HeadingLevel::H1, app_shortcuts.h1),
                    ("H2", "##", HeadingLevel::H2, app_shortcuts.h2),
                    ("H3", "###", HeadingLevel::H3, app_shortcuts.h3),
                    // ("H4", "####", HeadingLevel::H4, app_shortcuts.h4),
                ]
                .map(|(name, prefix, level, shortcut)| MdAnnotationShortcut {
                    name,
                    shortcut,
                    instruction: MatchFirst(
                        [
                            SpanKind::Heading(HeadingLevel::H1),
                            SpanKind::Heading(HeadingLevel::H2),
                            SpanKind::Heading(HeadingLevel::H3),
                            SpanKind::Heading(HeadingLevel::H4),
                            SpanKind::Heading(HeadingLevel::H5),
                            SpanKind::Heading(HeadingLevel::H6),
                            SpanKind::Paragraph,
                        ]
                        .into_iter()
                        .filter(|kind| *kind != SpanKind::Heading(level))
                        .map(|kind| {
                            (
                                InstructionCondition::IsInside(kind),
                                Seq([
                                    SetReplaceArea(kind),
                                    Insert(prefix),
                                    Insert(" "),
                                    PlaceCursor(Edge::Start),
                                    CopyFrom(Source::SurroundingSpanContent(kind)),
                                    PlaceCursor(Edge::End),
                                ]
                                .into()),
                            )
                        })
                        .chain(
                            [(
                                InstructionCondition::IsInsideUnmarkedArea,
                                Seq([
                                    Insert(prefix),
                                    Insert(" "),
                                    PlaceCursor(Edge::Start),
                                    PlaceCursor(Edge::End),
                                ]
                                .into()),
                            )]
                            .into_iter(),
                        )
                        .collect(),
                    ),
                    target_span: SpanKind::Heading(level),
                }),
            )
            .collect(),
            app_shortcuts,
        }
    }

    pub fn should_persist(&mut self) -> Option<PersistentState> {
        if self.save_to_storage {
            self.save_to_storage = false;
            Some(PersistentState {
                notes: self.notes.iter().map(|n| n.text.clone()).collect(),
                selected_note: self.selected_note,
            })
        } else {
            None
        }
    }
}

pub struct AppInitData {
    pub theme: AppTheme,
    pub msg_queue: Receiver<MsgToApp>,
    pub persistent_state: Option<PersistentState>,
}

fn number_to_key(key: u8) -> Option<Key> {
    match key {
        0 => Some(Key::Num0),
        1 => Some(Key::Num1),
        2 => Some(Key::Num2),
        3 => Some(Key::Num3),
        4 => Some(Key::Num4),
        5 => Some(Key::Num5),
        6 => Some(Key::Num6),
        7 => Some(Key::Num7),
        8 => Some(Key::Num8),
        9 => Some(Key::Num9),
        _ => None,
    }
}

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::{mpsc::Receiver, Arc},
};

use eframe::{
    egui::{self, Key, KeyboardShortcut, Modifiers, Ui},
    epaint::Galley,
};
use itertools::Itertools;
use pulldown_cmark::HeadingLevel;
use smallvec::SmallVec;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    app_actions::AppAction,
    byte_span::UnOrderedByteSpan,
    command::{
        CommandContext, CommandList, EditorCommand, EditorCommandOutput, TextCommandContext,
    },
    commands::{
        enter_in_list::on_enter_inside_list_item,
        space_after_task_markers::on_space_after_task_markers,
        tabbing_in_list::{on_shift_tab_inside_list, on_tab_inside_list},
    },
    effects::text_change_effect::TextChange,
    md_shortcut::{
        handle_md_annotation_command, Edge, Instruction, InstructionCondition,
        MdAnnotationShortcut, Source,
    },
    persistent_state::{DataToSave, NoteFile, RestoredData},
    text_structure::{SpanKind, TextStructure},
    theme::AppTheme,
};

#[derive(Debug)]
pub struct Note {
    pub text: String,
    pub cursor: Option<UnOrderedByteSpan>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum UnsavedChange {
    NoteContentChanged(NoteFile),
    SelectionChanged,
    LastUpdated,
}

pub struct AppState {
    // -----this is persistent model-------
    pub notes: Vec<Note>,
    pub selected_note: u32,
    // ------------------------------------
    // -------- emphemeral state ----------
    pub unsaved_changes: SmallVec<[UnsavedChange; 2]>,
    pub scheduled_script_run_version: Option<u64>,

    // ------------------------------------
    pub is_settings_opened: bool,
    pub is_pinned: bool,

    pub theme: AppTheme,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    pub msg_queue: Receiver<MsgToApp>,
    pub hidden: bool,
    pub prev_focused: bool,
    pub editor_commands: CommandList,

    pub computed_layout: Option<ComputedLayout>,
    pub text_structure: Option<TextStructure>,
    pub font_scale: i32,
}

pub struct ComputedLayout {
    pub galley: Arc<Galley>,
    pub layout_params_hash: u64,
}

pub struct LayoutParams<'a> {
    text: &'a str,
    wrap_width: f32,
    hash: u64,
}

impl<'a> LayoutParams<'a> {
    pub fn new(text: &'a str, wrap_width: f32, font_size: i32) -> Self {
        Self {
            text,
            wrap_width,
            hash: {
                let mut s = DefaultHasher::new();
                text.hash(&mut s);
                font_size.hash(&mut s);
                // note that it is OK to round it up
                ((wrap_width * 100.0) as i64).hash(&mut s);
                s.finish()
            },
        }
    }
}

impl ComputedLayout {
    pub fn should_recompute(&self, layout_params: &LayoutParams) -> bool {
        // TODO might want to check for any changes to theme, not just font_size
        self.layout_params_hash != layout_params.hash
    }

    pub fn compute(
        text_structure: &TextStructure,
        layout_params: &LayoutParams,
        ui: &Ui,
        theme: &AppTheme,
        syntax_set: &SyntaxSet,
        theme_set: &ThemeSet,
    ) -> Self {
        // let text_structure = TextStructure::create_from(text);

        let mut job =
            text_structure.create_layout_job(layout_params.text, theme, syntax_set, theme_set);

        job.wrap.max_width = layout_params.wrap_width;

        let galley = ui.fonts(|f| f.layout_job(job));

        Self {
            galley,
            layout_params_hash: layout_params.hash,
        }
    }
}

#[derive(Debug)]
pub enum MsgToApp {
    ToggleVisibility,
    NoteFileChanged(NoteFile, PathBuf),
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

        let RestoredData {
            state: saved_state,
            notes,
            settings: _,
        } = persistent_state;

        let notes: Vec<Note> = notes
            .into_iter()
            .map(|text| Note { text, cursor: None })
            .collect();

        let selected_note = match saved_state.selected {
            NoteFile::Note(index) => index,
            NoteFile::Settings => 0,
        };

        use Instruction::*;
        use InstructionCondition::*;

        let text_structure = TextStructure::new(&notes[selected_note as usize].text);

        let md_annotation_shortcuts: Vec<MdAnnotationShortcut> = [
            (
                CommandList::MARKDOWN_BOLD,
                "**",
                KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::B),
                SpanKind::Bold,
            ),
            (
                CommandList::MARKDOWN_ITALIC,
                "*",
                KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::I),
                SpanKind::Emphasis,
            ),
            (
                CommandList::MARKDOWN_STRIKETHROUGH,
                "~~",
                KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::E),
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
            name: CommandList::MARKDOWN_CODEBLOCK,
            shortcut: KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::C),
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
                (
                    CommandList::MARKDOWN_H1,
                    "#",
                    HeadingLevel::H1,
                    KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num1),
                ),
                (
                    CommandList::MARKDOWN_H2,
                    "##",
                    HeadingLevel::H2,
                    KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num2),
                ),
                (
                    CommandList::MARKDOWN_H3,
                    "###",
                    HeadingLevel::H3,
                    KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num3),
                ),
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
        .collect();

        fn map_text_command_to_command_handler(
            f: impl Fn(TextCommandContext) -> Option<Vec<TextChange>> + 'static,
        ) -> Box<dyn Fn(CommandContext) -> EditorCommandOutput> {
            Box::new(move |CommandContext { app_state }| {
                let note = &app_state.notes[app_state.selected_note as usize];

                let Some(cursor) = note.cursor else {
                    return SmallVec::new();
                };

                let Some(text_structure) = app_state.text_structure.as_ref() else {
                    return SmallVec::new();
                };

                let text_command_ctx = TextCommandContext {
                    text_structure,
                    text: &note.text,
                    byte_cursor: cursor.ordered(),
                };

                let note_file = NoteFile::Note(app_state.selected_note);

                f(text_command_ctx)
                    .map(|changes| {
                        SmallVec::from([AppAction::ApplyTextChanges(note_file, changes)])
                    })
                    .unwrap_or_default()
            })
        }

        let mut editor_commands: Vec<EditorCommand> = [
            (
                CommandList::EXPAND_TASK_MARKER,
                KeyboardShortcut::new(Modifiers::NONE, eframe::egui::Key::Space),
                map_text_command_to_command_handler(on_space_after_task_markers),
            ),
            (
                CommandList::INDENT_LIST_ITEM,
                KeyboardShortcut::new(Modifiers::NONE, eframe::egui::Key::Tab),
                map_text_command_to_command_handler(on_tab_inside_list),
            ),
            (
                CommandList::UNINDENT_LIST_ITEM,
                KeyboardShortcut::new(Modifiers::SHIFT, eframe::egui::Key::Tab),
                map_text_command_to_command_handler(on_shift_tab_inside_list),
            ),
            (
                CommandList::SPLIT_LIST_ITEM,
                KeyboardShortcut::new(Modifiers::NONE, eframe::egui::Key::Enter),
                map_text_command_to_command_handler(on_enter_inside_list_item),
            ),
        ]
        .map(|(name, shortcut, handler)| EditorCommand {
            name: name.to_string(),
            shortcut: Some(shortcut),
            try_handle: handler,
        })
        .into_iter()
        .collect();

        for md_shortcut in md_annotation_shortcuts {
            editor_commands.push(EditorCommand {
                name: md_shortcut.name.to_string(),
                shortcut: Some(md_shortcut.shortcut.clone()),
                try_handle: map_text_command_to_command_handler(move |context| {
                    handle_md_annotation_command(&md_shortcut, context)
                }),
            });
        }

        for note_index in 0..notes.len() {
            editor_commands.push(EditorCommand {
                name: CommandList::switch_to_note(note_index as u8).to_string(),
                shortcut: Some(KeyboardShortcut::new(
                    Modifiers::COMMAND,
                    number_to_key(note_index as u8 + 1).unwrap(),
                )),
                try_handle: Box::new(move |_| {
                    [AppAction::SwitchToNote {
                        index: note_index as u32,
                        via_shortcut: true,
                    }]
                    .into()
                }),
            })
        }

        editor_commands.push(EditorCommand {
            name: CommandList::INCREASE_FONT_SIZE.to_string(),
            shortcut: Some(KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::Plus)),
            try_handle: Box::new(|_| [AppAction::IncreaseFontSize].into()),
        });

        editor_commands.push(EditorCommand {
            name: CommandList::DECREASE_FONT_SIZE.to_string(),
            shortcut: Some(KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::Minus)),
            try_handle: Box::new(|_| [AppAction::DecreaseFontSize].into()),
        });

        editor_commands.push(EditorCommand {
            name: CommandList::PIN_WINDOW.to_string(),
            shortcut: Some(KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::P)),
            try_handle: Box::new(|ctx| {
                [AppAction::SetWindowPinned(!ctx.app_state.is_pinned)].into()
            }),
        });

        Self {
            is_settings_opened: false,
            is_pinned: false,
            unsaved_changes: Default::default(),
            scheduled_script_run_version: None,
            theme,
            notes,
            computed_layout: None,
            text_structure: Some(text_structure),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            msg_queue,
            selected_note,
            hidden: false,
            prev_focused: false,
            font_scale: 0,
            editor_commands: CommandList::new(editor_commands),
        }
    }

    pub fn should_persist<'s>(&'s mut self) -> Option<DataToSave> {
        if !self.unsaved_changes.is_empty() {
            let changes: SmallVec<[_; 4]> = self.unsaved_changes.drain(..).unique().collect();
            Some(DataToSave {
                files: changes
                    .into_iter()
                    .filter_map(|change| match change {
                        UnsavedChange::NoteContentChanged(NoteFile::Note(index)) => self
                            .notes
                            .get(index as usize)
                            .map(|n| (NoteFile::Note(index), n.text.as_str())),
                        _ => None,
                    })
                    .collect(),
                selected: NoteFile::Note(self.selected_note),
            })
        } else {
            None
        }
    }
}

pub struct AppInitData {
    pub theme: AppTheme,
    pub msg_queue: Receiver<MsgToApp>,
    pub persistent_state: RestoredData,
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

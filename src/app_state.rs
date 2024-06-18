use std::{
    collections::BTreeMap,
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
        toggle_code_block::toggle_code_block,
        toggle_md_headings::toggle_md_heading,
        toggle_simple_md_annotations::toggle_simple_md_annotations,
    },
    effects::text_change_effect::TextChange,
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
    pub notes: BTreeMap<NoteFile, Note>,
    pub selected_note: NoteFile,
    // ------------------------------------
    // -------- emphemeral state ----------
    pub last_saved: u128,
    pub unsaved_changes: SmallVec<[UnsavedChange; 2]>,
    pub scheduled_script_run_version: Option<u64>,

    // ------------------------------------
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
    pub fn new(text: &'a str, wrap_width: f32) -> Self {
        Self {
            text,
            wrap_width,
            hash: {
                let mut s = DefaultHasher::new();
                text.hash(&mut s);
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
            last_saved,
        } = init_data;

        let RestoredData {
            state: saved_state,
            notes,
            settings,
        } = persistent_state;

        let shelf_count = notes.len();

        let notes: BTreeMap<NoteFile, Note> = notes
            .into_iter()
            .enumerate()
            .map(|(i, text)| (NoteFile::Note(i as u32), Note { text, cursor: None }))
            .chain([(
                NoteFile::Settings,
                Note {
                    text: settings,
                    cursor: None,
                },
            )])
            .collect();

        let selected_note = saved_state.selected;

        let text_structure = TextStructure::new(&notes.get(&selected_note).unwrap().text);

        fn map_text_command_to_command_handler(
            f: impl Fn(TextCommandContext) -> Option<Vec<TextChange>> + 'static,
        ) -> Box<dyn Fn(CommandContext) -> EditorCommandOutput> {
            Box::new(move |CommandContext { app_state }| {
                let note = app_state.notes.get(&app_state.selected_note).unwrap();

                let Some(cursor) = note.cursor else {
                    return SmallVec::new();
                };

                let Some(text_structure) = app_state.text_structure.as_ref() else {
                    return SmallVec::new();
                };

                f(TextCommandContext::new(
                    text_structure,
                    &note.text,
                    cursor.ordered(),
                ))
                .map(|changes| {
                    SmallVec::from([AppAction::ApplyTextChanges(
                        app_state.selected_note,
                        changes,
                    )])
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
                KeyboardShortcut::new(Modifiers::NONE, egui::Key::Tab),
                map_text_command_to_command_handler(on_tab_inside_list),
            ),
            (
                CommandList::UNINDENT_LIST_ITEM,
                KeyboardShortcut::new(Modifiers::SHIFT, egui::Key::Tab),
                map_text_command_to_command_handler(on_shift_tab_inside_list),
            ),
            (
                CommandList::SPLIT_LIST_ITEM,
                KeyboardShortcut::new(Modifiers::NONE, egui::Key::Enter),
                map_text_command_to_command_handler(on_enter_inside_list_item),
            ),
            (
                CommandList::MARKDOWN_CODEBLOCK,
                KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), egui::Key::B),
                map_text_command_to_command_handler(toggle_code_block),
            ),
            (
                CommandList::MARKDOWN_BOLD,
                KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::B),
                map_text_command_to_command_handler(|text_context| {
                    toggle_simple_md_annotations(text_context, SpanKind::Bold, "**")
                }),
            ),
            (
                CommandList::MARKDOWN_ITALIC,
                KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::I),
                map_text_command_to_command_handler(|text_context| {
                    toggle_simple_md_annotations(text_context, SpanKind::Emphasis, "*")
                }),
            ),
            (
                CommandList::MARKDOWN_STRIKETHROUGH,
                KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), egui::Key::E),
                map_text_command_to_command_handler(|text_context| {
                    toggle_simple_md_annotations(text_context, SpanKind::Strike, "~~")
                }),
            ),
            (
                CommandList::MARKDOWN_H1,
                KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num1),
                map_text_command_to_command_handler(|text_context| {
                    toggle_md_heading(text_context, HeadingLevel::H1)
                }),
            ),
            (
                CommandList::MARKDOWN_H2,
                KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num2),
                map_text_command_to_command_handler(|text_context| {
                    toggle_md_heading(text_context, HeadingLevel::H2)
                }),
            ),
            (
                CommandList::MARKDOWN_H3,
                KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Num3),
                map_text_command_to_command_handler(|text_context| {
                    toggle_md_heading(text_context, HeadingLevel::H3)
                }),
            ),
        ]
        .into_iter()
        .map(|(name, shortcut, handler)| EditorCommand {
            name: name.to_string(),
            shortcut: Some(shortcut),
            try_handle: handler,
        })
        .collect();

        for note_index in 0..shelf_count {
            editor_commands.push(EditorCommand {
                name: CommandList::switch_to_note(note_index as u8).to_string(),
                shortcut: Some(KeyboardShortcut::new(
                    Modifiers::COMMAND,
                    number_to_key(note_index as u8 + 1).unwrap(),
                )),
                try_handle: Box::new(move |_| {
                    [AppAction::SwitchToNote {
                        note_file: NoteFile::Note(note_index as u32),
                        via_shortcut: true,
                    }]
                    .into()
                }),
            })
        }

        editor_commands.push(EditorCommand {
            name: CommandList::OPEN_SETTIGS.to_string(),
            shortcut: Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma)),
            try_handle: Box::new(move |_| {
                [AppAction::SwitchToNote {
                    note_file: NoteFile::Settings,
                    via_shortcut: true,
                }]
                .into()
            }),
        });

        editor_commands.push(EditorCommand {
            name: CommandList::PIN_WINDOW.to_string(),
            shortcut: Some(KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::P)),
            try_handle: Box::new(|ctx| {
                [AppAction::SetWindowPinned(!ctx.app_state.is_pinned)].into()
            }),
        });

        editor_commands.push(EditorCommand {
            name: CommandList::HIDE_WINDOW.to_string(),
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, egui::Key::Escape)),
            try_handle: Box::new(|_| {
                [AppAction::HandleMsgToApp(MsgToApp::ToggleVisibility)].into()
            }),
        });

        Self {
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
            last_saved,
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
                        UnsavedChange::NoteContentChanged(note_file) => self
                            .notes
                            .get(&note_file)
                            .map(|n| (note_file, n.text.as_str())),
                        _ => None,
                    })
                    .collect(),
                selected: self.selected_note,
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
    pub last_saved: u128,
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

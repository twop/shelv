use std::{
    collections::BTreeMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::{mpsc::Receiver, Arc},
};

use eframe::{
    egui::{self, text::CCursor, Key, KeyboardShortcut, Modifiers, Rect, Ui},
    epaint::Galley,
};
use itertools::Itertools;
use pulldown_cmark::HeadingLevel;
use smallvec::SmallVec;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    app_actions::{AppAction, AppIO},
    app_ui::char_index_from_byte_index,
    byte_span::{ByteSpan, UnOrderedByteSpan},
    command::{
        map_text_command_to_command_handler, BuiltInCommand, CommandContext, CommandList,
        EditorCommand, EditorCommandOutput, TextCommandContext,
    },
    commands::{
        enter_in_list::on_enter_inside_list_item,
        run_llm::run_llm_block,
        space_after_task_markers::on_space_after_task_markers,
        tabbing_in_list::{on_shift_tab_inside_list, on_tab_inside_list},
        toggle_code_block::toggle_code_block,
        toggle_md_headings::toggle_md_heading,
        toggle_simple_md_annotations::toggle_simple_md_annotations,
    },
    effects::text_change_effect::{apply_text_changes, TextChange},
    persistent_state::{DataToSave, NoteFile, RestoredData},
    scripting::execute_code_blocks,
    settings::{LlmSettings, SettingsNoteEvalContext},
    text_structure::{SpanIndex, SpanKind, SpanMeta, TextStructure},
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
    PinStateChanged,
}

pub struct AppState {
    // -----this is persistent model-------
    pub notes: BTreeMap<NoteFile, Note>,
    pub selected_note: NoteFile,
    // ------------------------------------
    // -------- emphemeral state ----------
    pub last_saved: u128,
    unsaved_changes: SmallVec<[UnsavedChange; 2]>,
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
    pub llm_settings: Option<LlmSettings>,

    pub computed_layout: Option<ComputedLayout>,
    pub text_structure: Option<TextStructure>,
    pub deferred_to_post_render: Vec<AppAction>,
}

impl AppState {
    pub fn add_unsaved_change(&mut self, change: UnsavedChange) {
        if self.unsaved_changes.iter().any(|c| c == &change) {
            // if we already have a change pending do nothing
            return;
        }

        self.unsaved_changes.push(change);
    }
}

pub struct CodeArea {
    pub rect: Rect,
    // TODO: use small string
    pub lang: String,
    pub code_block_span_index: SpanIndex,
}

pub struct ComputedLayout {
    pub galley: Arc<Galley>,
    pub layout_params_hash: u64,
    pub code_areas: SmallVec<[CodeArea; 6]>,
}

#[derive(Debug)]
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

        let code_areas: SmallVec<[CodeArea; 6]> = text_structure
            .iter()
            .filter_map(|(index, desc)| match desc.kind {
                SpanKind::CodeBlock => {
                    text_structure.find_meta(index).and_then(|meta| match meta {
                        // TODO use small string instead
                        SpanMeta::CodeBlock { lang } => {
                            Some((desc.byte_pos, lang.to_owned(), index))
                        }
                        _ => None,
                    })
                }
                _ => None,
            })
            .map(|(byte_span, lang, index)| {
                let [mut r_start, r_end] = [byte_span.start, byte_span.end].map(|byte_pos| {
                    let char_pos = char_index_from_byte_index(layout_params.text, byte_pos);
                    galley.pos_from_ccursor(CCursor::new(char_pos))
                });

                // TODO make a prettier math
                r_start.extend_with(r_end.min);
                r_start.extend_with(r_end.max);
                r_start.set_right(r_start.right().max(layout_params.wrap_width));
                CodeArea {
                    rect: r_start,
                    lang,
                    code_block_span_index: index,
                }
            })
            .collect();

        // println!("^^^^ compute layout, code_areas = {code_areas:#?}");
        // println!(
        //     "^^^^ galley rect={:#?}, mesh_rect={:#?}",
        //     galley.rect, galley.mesh_bounds
        // );

        Self {
            galley,
            code_areas,
            layout_params_hash: layout_params.hash,
        }
    }
}

#[derive(Debug)]
pub struct LLMResponseChunk {
    pub chunk: String,
    pub address: String,
    pub note_id: NoteFile,
}

#[derive(Debug)]
pub enum MsgToApp {
    ToggleVisibility,
    NoteFileChanged(NoteFile, PathBuf),
    GlobalHotkey(u32),
    LLMResponseChunk(LLMResponseChunk),
}

// struct MdAnnotationShortcut {
//     name: &'static str,
//     annotation: &'static str,
//     shortcut: KeyboardShortcut,
// }

impl AppState {
    pub fn new(init_data: AppInitData, app_io: &mut impl AppIO) -> Self {
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

        let mut notes: BTreeMap<NoteFile, Note> = notes
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
        let is_window_pinned = saved_state.is_pinned;

        let text_structure = TextStructure::new(&notes.get(&selected_note).unwrap().text);

        let mut editor_commands: Vec<EditorCommand> = [
            (
                BuiltInCommand::ExpandTaskMarker,
                map_text_command_to_command_handler(on_space_after_task_markers),
            ),
            (
                BuiltInCommand::IndentListItem,
                map_text_command_to_command_handler(on_tab_inside_list),
            ),
            (
                BuiltInCommand::UnindentListItem,
                map_text_command_to_command_handler(on_shift_tab_inside_list),
            ),
            (
                BuiltInCommand::SplitListItem,
                map_text_command_to_command_handler(on_enter_inside_list_item),
            ),
            (
                BuiltInCommand::MarkdownCodeBlock,
                map_text_command_to_command_handler(toggle_code_block),
            ),
            (
                BuiltInCommand::MarkdownBold,
                map_text_command_to_command_handler(|text_context| {
                    toggle_simple_md_annotations(text_context, SpanKind::Bold, "**")
                }),
            ),
            (
                BuiltInCommand::MarkdownItalic,
                map_text_command_to_command_handler(|text_context| {
                    toggle_simple_md_annotations(text_context, SpanKind::Emphasis, "*")
                }),
            ),
            (
                BuiltInCommand::MarkdownStrikethrough,
                map_text_command_to_command_handler(|text_context| {
                    toggle_simple_md_annotations(text_context, SpanKind::Strike, "~~")
                }),
            ),
            (
                BuiltInCommand::MarkdownH1,
                map_text_command_to_command_handler(|text_context| {
                    toggle_md_heading(text_context, HeadingLevel::H1)
                }),
            ),
            (
                BuiltInCommand::MarkdownH2,
                map_text_command_to_command_handler(|text_context| {
                    toggle_md_heading(text_context, HeadingLevel::H2)
                }),
            ),
            (
                BuiltInCommand::MarkdownH3,
                map_text_command_to_command_handler(|text_context| {
                    toggle_md_heading(text_context, HeadingLevel::H3)
                }),
            ),
        ]
        .into_iter()
        .map(|(cmd, handler)| EditorCommand::built_in(cmd, handler))
        .collect();

        for note_index in 0..shelf_count {
            let cmd = BuiltInCommand::SwitchToNote(note_index as u8);
            editor_commands.push(EditorCommand::built_in(cmd, move |_| {
                [AppAction::SwitchToNote {
                    note_file: NoteFile::Note(note_index as u32),
                    via_shortcut: true,
                }]
                .into()
            }))
        }

        editor_commands.push(EditorCommand::built_in(
            BuiltInCommand::SwitchToSettings,
            |_| {
                [AppAction::SwitchToNote {
                    note_file: NoteFile::Settings,
                    via_shortcut: true,
                }]
                .into()
            },
        ));

        editor_commands.push(EditorCommand::built_in(BuiltInCommand::PinWindow, |ctx| {
            [AppAction::SetWindowPinned(!ctx.app_state.is_pinned)].into()
        }));

        editor_commands.push(EditorCommand::built_in(BuiltInCommand::HideApp, |_| {
            [AppAction::HandleMsgToApp(MsgToApp::ToggleVisibility)].into()
        }));

        editor_commands.push(EditorCommand::built_in(
            BuiltInCommand::RunLLMBlock,
            |ctx| run_llm_block(ctx).unwrap_or_default(),
        ));

        let mut editor_commands = CommandList::new(editor_commands);
        let mut llm_settings: Option<LlmSettings> = None;

        // TODO this is ugly, refactor this to be a bit nicer
        // at least from the error reporting point of view
        if let Some(settings) = notes.get_mut(&NoteFile::Settings) {
            match {
                let mut cx = SettingsNoteEvalContext {
                    cmd_list: &mut editor_commands,
                    should_force_eval: true,
                    app_io,
                    llm_settings: &mut llm_settings,
                };

                execute_code_blocks(&mut cx, &TextStructure::new(&settings.text), &settings.text)
            } {
                Some(requested_changes) => {
                    match apply_text_changes(&mut settings.text, settings.cursor, requested_changes)
                    {
                        Ok(_) => println!("applied settings note successfully"),
                        Err(err) => println!("failed to write back settings results {:#?}", err),
                    }
                }
                None => (),
            }
        }

        Self {
            is_pinned: is_window_pinned,
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
            editor_commands,
            llm_settings,
            deferred_to_post_render: vec![],
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
                is_pinned: self.is_pinned,
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

use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{mpsc::Receiver, Arc},
};

use eframe::{
    egui::{
        text::{CCursor, LayoutJob},
        Id, Key, Rect, Ui,
    },
    epaint::Galley,
};
use itertools::Itertools;
use pulldown_cmark::HeadingLevel;
use similar::DiffableStr;
use smallvec::SmallVec;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};

use crate::{
    app_actions::{AppAction, AppIO},
    app_ui::char_index_from_byte_index,
    byte_span::{ByteSpan, UnOrderedByteSpan},
    command::{
        call_with_text_ctx, AppFocus, CommandContext, CommandHandler, CommandInstance,
        CommandInstruction, CommandList, EditorCommandOutput, SlashPaletteCmd, TextCommandContext,
    },
    commands::{
        enter_in_list::on_enter_inside_list_item,
        inline_llm_prompt::inline_llm_prompt_command_handler,
        insert_text::call_replace_text,
        kdl_lang::{autoclose_bracket_inside_kdl_block, on_enter_inside_kdl_block},
        run_llm::{prepare_to_run_llm_block, CodeBlockAddress},
        slash_pallete::{
            execute_slash_cmd, hide_slash_pallete, next_slash_cmd, prev_slash_cmd,
            show_slash_pallete,
        },
        space_after_task_markers::on_space_after_task_markers,
        tabbing_in_list::{on_shift_tab_inside_list, on_tab_inside_list},
        toggle_code_block::toggle_code_block,
        toggle_md_headings::toggle_md_heading,
        toggle_simple_md_annotations::toggle_simple_md_annotations,
    },
    effects::text_change_effect::{apply_text_changes, TextChange},
    persistent_state::{DataToSave, NoteFile, RestoredData},
    scripting::execute_code_blocks,
    settings_eval::{
        parse_and_eval_settings_script_block, Scripts, SettingsNoteEvalContext,
        SETTINGS_SCRIPT_BLOCK_LANG,
    },
    settings_parsing::LlmSettings,
    text_structure::{
        CodeBlockMeta, SpanIndex, SpanKind, SpanMeta, TextDiffPart, TextStructure,
        TextStructureVersion,
    },
    theme::AppTheme,
};

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct TextSelectionAddress {
    pub span: ByteSpan,
    pub note_file: NoteFile,
    pub text_version: TextStructureVersion,
}

#[derive(Debug)]
pub enum InlinePromptStatus {
    NotStarted,
    Streaming { prompt: String },
    Done { prompt: String },
}

#[derive(Debug)]
pub struct InlineLLMPromptState {
    pub prompt: String,
    pub address: TextSelectionAddress,
    pub response_text: String,
    pub diff_parts: Vec<TextDiffPart>,
    pub layout_job: LayoutJob,
    pub status: InlinePromptStatus,
    pub fresh_response: bool,
}
#[derive(Debug)]
pub struct Note {
    pub text: String,
    cursor: Option<UnOrderedByteSpan>,
    last_cursor: Option<UnOrderedByteSpan>,
}

impl Note {
    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn update_cursor(&mut self, updated_cursor: UnOrderedByteSpan) {
        self.last_cursor = Some(updated_cursor);
        self.cursor = Some(updated_cursor);
    }

    pub fn cursor(&self) -> Option<UnOrderedByteSpan> {
        self.cursor
    }

    pub fn last_cursor(&self) -> Option<UnOrderedByteSpan> {
        self.last_cursor
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum UnsavedChange {
    NoteContentChanged(NoteFile),
    SelectionChanged,
    LastUpdated,
    PinStateChanged,
}

#[derive(Debug)]
pub struct SlashPalette {
    pub note_file: NoteFile,
    pub slash_byte_pos: usize,
    pub search_term: String,
    pub options: Vec<SlashPaletteCmd>,
    pub selected: usize,
    pub update_count: u32,
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
    pub commands: CommandList,
    pub llm_settings: Option<LlmSettings>,

    pub inline_llm_prompt: Option<InlineLLMPromptState>,
    pub slash_palette: Option<SlashPalette>,

    pub computed_layout: Option<ComputedLayout>,
    pub text_structure: Option<TextStructure>,
    pub settings_scripts: Option<Scripts>,
    pub deferred_actions: Vec<AppAction>,
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
#[derive(Debug)]
pub struct CodeArea {
    pub rect: Rect,
    // TODO: use small string
    pub lang: String,
    pub code_block_span_index: SpanIndex,
}

#[derive(Debug)]
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
    pub fn new(text: &'a str, wrap_width: f32, dpi: f32) -> Self {
        Self {
            text,
            wrap_width,
            hash: {
                let mut hasher = fxhash::FxHasher::default();
                text.hash(&mut hasher);
                // note that it is OK to round it up
                ((wrap_width * 100.0) as i64).hash(&mut hasher);
                ((dpi * 100.0) as i64).hash(&mut hasher);
                hasher.finish()
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
                        SpanMeta::CodeBlock(CodeBlockMeta { lang, .. }) => {
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
pub struct LLMBlockResponseChunk {
    pub chunk: String,
    pub address: String,
    pub note_id: NoteFile,
}

#[derive(Debug)]
pub enum InlineLLMResponseChunk {
    Chunk(String),
    End,
}

#[derive(Debug)]
pub enum MsgToApp {
    ToggleVisibility,
    NoteFileChanged(NoteFile, PathBuf),
    GlobalHotkey(u32),
    LLMBlockResponseChunk(LLMBlockResponseChunk),

    InlineLLMResponse {
        response: InlineLLMResponseChunk,
        address: TextSelectionAddress,
    },
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

        let notes: BTreeMap<NoteFile, Note> = notes
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                (
                    NoteFile::Note(i as u32),
                    Note {
                        text,
                        cursor: None,
                        last_cursor: None,
                    },
                )
            })
            .chain([(
                NoteFile::Settings,
                Note {
                    text: settings,
                    cursor: None,
                    last_cursor: None,
                },
            )])
            .collect();

        let selected_note = saved_state.selected;
        let is_window_pinned = saved_state.is_pinned;

        let text_structure = TextStructure::new(&notes.get(&selected_note).unwrap().text);

        let keybord_instructions: Vec<CommandInstruction> = Vec::from_iter(
            [
                CommandInstruction::ExpandTaskMarker,
                CommandInstruction::IndentListItem,
                CommandInstruction::UnindentListItem,
                CommandInstruction::SplitListItem,
                CommandInstruction::MarkdownCodeBlock(None),
                CommandInstruction::MarkdownBold,
                CommandInstruction::MarkdownItalic,
                CommandInstruction::MarkdownStrikethrough,
                CommandInstruction::MarkdownH1,
                CommandInstruction::MarkdownH2,
                CommandInstruction::MarkdownH3,
                CommandInstruction::EnterInsideKDL,
                CommandInstruction::SwitchToSettings,
                CommandInstruction::PinWindow,
                CommandInstruction::HideApp,
                CommandInstruction::HidePrompt,
                CommandInstruction::RunLLMBlock,
                CommandInstruction::ShowPrompt,
                CommandInstruction::ShowSlashPallete,
                CommandInstruction::HideSlashPallete,
                CommandInstruction::NextSlashPalleteCmd,
                CommandInstruction::PrevSlashPalleteCmd,
                CommandInstruction::ExecuteSlashPalleteCmd,
            ]
            .into_iter()
            .chain(
                (0..shelf_count)
                    .map(|note_index| CommandInstruction::SwitchToNote(note_index as u8)),
            ),
        );

        use egui_phosphor::light as P;
        let slash_palette_commands = []
            .into_iter()
            .chain(
                [
                    ("ai", CommandInstruction::ShowPrompt, P::SPARKLE),
                    (
                        "code",
                        CommandInstruction::MarkdownCodeBlock(None),
                        P::CODE_BLOCK,
                    ),
                    ("h1", CommandInstruction::MarkdownH1, P::TEXT_H_ONE),
                    ("h2", CommandInstruction::MarkdownH2, P::TEXT_H_TWO),
                    ("h3", CommandInstruction::MarkdownH3, P::TEXT_H_THREE),
                    ("bold", CommandInstruction::MarkdownBold, P::TEXT_BOLDER),
                    ("italic", CommandInstruction::MarkdownItalic, P::TEXT_ITALIC),
                    (
                        "strike",
                        CommandInstruction::MarkdownStrikethrough,
                        P::TEXT_STRIKETHROUGH,
                    ),
                ]
                .into_iter()
                .map(|(prefix, builtin, phosphor_icon)| {
                    SlashPaletteCmd::from_built_in_instruction(prefix, builtin)
                        .icon(phosphor_icon.to_string())
                }),
            )
            .collect();

        let editor_commands = CommandList::new(
            execute_instruction,
            keybord_instructions,
            slash_palette_commands,
        );

        let deferred_actions = vec![AppAction::EvalNote(NoteFile::Settings)];

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
            commands: editor_commands,
            llm_settings: None,
            deferred_actions,
            inline_llm_prompt: None,
            slash_palette: None,
            settings_scripts: None,
        }
    }

    pub fn should_persist(&mut self) -> Option<DataToSave> {
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

fn execute_instruction(
    instruction: &CommandInstruction,
    ctx: CommandContext,
) -> EditorCommandOutput {
    use CommandInstruction as CI;
    match instruction {
        CI::ExpandTaskMarker => call_with_text_ctx(ctx, on_space_after_task_markers),
        CI::IndentListItem => call_with_text_ctx(ctx, on_tab_inside_list),
        CI::UnindentListItem => call_with_text_ctx(ctx, on_shift_tab_inside_list),
        CI::SplitListItem => call_with_text_ctx(ctx, on_enter_inside_list_item),
        CI::MarkdownCodeBlock(lang) => call_with_text_ctx(ctx, |cx| {
            toggle_code_block(cx, lang.as_ref().map(|s| s.as_str()))
        }),
        CI::MarkdownBold => call_with_text_ctx(ctx, |text_context| {
            toggle_simple_md_annotations(text_context, SpanKind::Bold, "**")
        }),
        CI::MarkdownItalic => call_with_text_ctx(ctx, |text_context| {
            toggle_simple_md_annotations(text_context, SpanKind::Emphasis, "*")
        }),
        CI::MarkdownStrikethrough => call_with_text_ctx(ctx, |text_context| {
            toggle_simple_md_annotations(text_context, SpanKind::Strike, "~~")
        }),
        CI::MarkdownH1 => call_with_text_ctx(ctx, |text_context| {
            toggle_md_heading(text_context, HeadingLevel::H1)
        }),
        CI::MarkdownH2 => call_with_text_ctx(ctx, |text_context| {
            toggle_md_heading(text_context, HeadingLevel::H2)
        }),
        CI::MarkdownH3 => call_with_text_ctx(ctx, |text_context| {
            toggle_md_heading(text_context, HeadingLevel::H3)
        }),
        CI::EnterInsideKDL => call_with_text_ctx(ctx, on_enter_inside_kdl_block),

        CI::SwitchToNote(note_index) => SmallVec::from([AppAction::SwitchToNote {
            note_file: NoteFile::Note(*note_index as u32),
            via_shortcut: true,
        }]),

        CI::SwitchToSettings => [AppAction::SwitchToNote {
            note_file: NoteFile::Settings,
            via_shortcut: true,
        }]
        .into(),

        CI::PinWindow => [AppAction::SetWindowPinned(!ctx.app_state.is_pinned)].into(),

        CI::HideApp => match (
            ctx.app_focus.is_menu_opened,
            &ctx.app_state.slash_palette,
            ctx.app_focus.focus,
        ) {
            (false, None, None | Some(AppFocus::NoteEditor)) => {
                [AppAction::HandleMsgToApp(MsgToApp::ToggleVisibility)].into()
            }
            _ => SmallVec::new(),
        },

        CI::HidePrompt => match ctx.app_focus.focus {
            Some(AppFocus::InlinePropmptEditor) => {
                [AppAction::AcceptPromptSuggestion { accept: false }].into()
            }
            _ => SmallVec::new(),
        },

        CI::RunLLMBlock => {
            prepare_to_run_llm_block(ctx, CodeBlockAddress::NoteSelection).unwrap_or_default()
        }

        CI::ShowPrompt => inline_llm_prompt_command_handler(ctx).unwrap_or_default(),

        CI::ShowSlashPallete => show_slash_pallete(ctx).unwrap_or_default(),

        CI::HideSlashPallete => hide_slash_pallete(ctx).unwrap_or_default(),

        CI::NextSlashPalleteCmd => next_slash_cmd(ctx).unwrap_or_default(),

        CI::PrevSlashPalleteCmd => prev_slash_cmd(ctx).unwrap_or_default(),

        CI::ExecuteSlashPalleteCmd => execute_slash_cmd(ctx).unwrap_or_default(),

        CI::BracketAutoclosingInsideKDL => todo!(),
        CI::InsertText(text_source) => call_replace_text(text_source, ctx),
        // Disable for now
        // CI::BracketAutoclosingInsideKDL => map_text_command_to_command_handler(autoclose_bracket_inside_kdl_block).call(ctx),
    }
}

pub fn compute_editor_text_id(selected_note_file: NoteFile) -> Id {
    Id::new(match selected_note_file {
        NoteFile::Note(index) => ("text_edit_id", index),
        NoteFile::Settings => ("text_edit_id_settings", 4568),
    })
}

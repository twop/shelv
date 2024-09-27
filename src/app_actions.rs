use std::{io, path::PathBuf};

use eframe::egui::{
    text::LayoutJob, Context, Id, KeyboardShortcut, Memory, OpenUrl, ViewportCommand,
};

use similar::{ChangeTag, TextDiff};
use smallvec::{smallvec, SmallVec};

use crate::{
    app_state::{
        compute_editor_text_id, AppState, InlineLLMPropmptState, InlineLLMResponseChunk,
        InlinePromptStatus, MsgToApp, TextSelectionAddress, UnsavedChange,
    },
    byte_span::{ByteSpan, UnOrderedByteSpan},
    command::{AppFocus, AppFocusState, CommandContext},
    commands::{
        inline_llm_prompt::compute_inline_prompt_text_input_id,
        run_llm::{prepare_to_run_llm_block, CodeBlockAddress, DEFAULT_LLM_MODEL},
    },
    effects::text_change_effect::{apply_text_changes, TextChange},
    persistent_state::{get_tutorial_note_content, NoteFile},
    scripting::{execute_code_blocks, execute_live_scripts},
    settings::SettingsNoteEvalContext,
    text_structure::{
        create_layout_job_from_text_diff, SpanIndex, SpanKind, SpanMeta, TextDiffPart,
        TextStructure, TextStructureVersion,
    },
};

#[derive(Clone, Copy, Debug)]
pub enum FocusTarget {
    CurrentNote,
    SpecificId(Id),
}

#[derive(Debug)]
pub enum AppAction {
    SwitchToNote {
        note_file: NoteFile,
        via_shortcut: bool,
    },
    // HideApp,
    // ShowApp,
    OpenLink(String),
    SetWindowPinned(bool),
    ApplyTextChanges {
        target: NoteFile,
        changes: Vec<TextChange>,
        should_trigger_eval: bool,
    },
    HandleMsgToApp(MsgToApp),
    EvalNote(NoteFile),
    AskLLM(LLMBlockRequest),
    RunLLMBLock(NoteFile, SpanIndex),
    SendFeedback(NoteFile),
    StartTutorial,
    DeferToPostRender(Box<AppAction>),
    FocusRequest(FocusTarget),
    OpenNotesInFinder,
    ShowPrompt(TextSelectionAddress),
    ExecutePrompt,
    AcceptPromptSuggestion {
        accept: bool,
    },
}

impl AppAction {
    pub fn apply_text_changes(target: NoteFile, changes: Vec<TextChange>) -> Self {
        Self::ApplyTextChanges {
            target,
            changes,
            should_trigger_eval: true,
        }
    }
}

#[derive(Debug)]
pub enum ConversationPart {
    Markdown(String),
    Question(String),
    Answer(String),
}

#[derive(Debug)]
pub struct Conversation {
    pub parts: Vec<ConversationPart>,
}

#[derive(Debug)]
pub struct LLMBlockRequest {
    pub system_prompt: Option<String>,
    pub model: String,
    pub conversation: Conversation,
    pub output_code_block_address: String,
    pub note_id: NoteFile,
}

#[derive(Debug)]
pub struct LLMPromptRequest {
    pub prompt: String,
    pub before_selection: String,
    pub after_selection: String,
    pub selection: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub selection_location: TextSelectionAddress,
}

// TODO consider focus, opening links etc as IO operations
pub trait AppIO {
    fn hide_app(&self);
    fn open_shelv_folder(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn try_read_note_if_newer(
        &self,
        path: &PathBuf,
        last_saved: u128,
    ) -> Result<Option<String>, io::Error>;

    fn cleanup_all_global_hotkeys(&mut self) -> Result<(), String>;

    fn try_map_hotkey(&self, hotkey_id: u32) -> Option<MsgToApp>;

    fn bind_global_hotkey(
        &mut self,
        shortcut: KeyboardShortcut,
        to: Box<dyn Fn() -> MsgToApp>,
    ) -> Result<(), String>;

    fn execute_llm_block(&self, question: LLMBlockRequest);
    fn execute_llm_prompt(&self, quesion: LLMPromptRequest);
}

pub fn process_app_action(
    action: AppAction,
    ctx: &Context,
    state: &mut AppState,
    text_edit_id: Id,
    app_io: &mut impl AppIO,
) -> SmallVec<[AppAction; 1]> {
    match action {
        AppAction::SwitchToNote {
            note_file,
            via_shortcut,
        } => {
            if note_file != state.selected_note {
                state.add_unsaved_change(UnsavedChange::SelectionChanged);

                if via_shortcut {
                    let note = &mut state.notes.get_mut(&note_file).unwrap();
                    note.cursor = match note.cursor.clone() {
                        None => {
                            let len = note.text.len();
                            Some(UnOrderedByteSpan::new(len, len))
                        }
                        prev => prev,
                    };
                } else {
                    // means that we reselected via UI

                    // if that is the case then reset cursors from both of the notes
                    if let Some(prev_note) = state.notes.get_mut(&state.selected_note) {
                        prev_note.cursor = None;
                    }

                    if let Some(cur_note) = state.notes.get_mut(&note_file) {
                        cur_note.cursor = None;
                    }
                }

                // reset inline prompt state if we switched to a different note
                state.inline_llm_prompt = None;

                if let Some(cur_note) = state.notes.get(&note_file) {
                    let text = &cur_note.text;
                    state.selected_note = note_file;
                    state.text_structure = state.text_structure.take().map(|s| s.recycle(text));
                };
            }

            match via_shortcut {
                true => [AppAction::DeferToPostRender(Box::new(
                    AppAction::FocusRequest(FocusTarget::CurrentNote),
                ))]
                .into(),
                false => SmallVec::new(),
            }
        }

        AppAction::OpenLink(url) => {
            ctx.open_url(OpenUrl::new_tab(url));
            SmallVec::new()
        }

        AppAction::SetWindowPinned(is_pinned) => {
            state.is_pinned = is_pinned;
            state.add_unsaved_change(UnsavedChange::PinStateChanged);
            SmallVec::new()
        }

        AppAction::ApplyTextChanges {
            target: note_file,
            changes,
            should_trigger_eval,
        } => {
            let note = &mut state.notes.get_mut(&note_file).unwrap();
            let text = &mut note.text;
            let cursor = note.cursor;

            let next_action = match apply_text_changes(text, cursor, changes) {
                Ok(updated_cursor) => {
                    if note_file == state.selected_note {
                        // if the changes are for the selected note we need to recompute TextStructure
                        state.text_structure = state.text_structure.take().map(|s| s.recycle(text));
                    }

                    note.cursor = updated_cursor;

                    state.add_unsaved_change(UnsavedChange::NoteContentChanged(note_file));
                    // reset the inline prompt state if any changes happened
                    // it maybe a bit too  aggressive, but let's live with the simplest approach first
                    state.inline_llm_prompt = None;
                    should_trigger_eval.then(|| AppAction::EvalNote(note_file))
                }
                Err(_) => None,
            };

            match next_action {
                Some(a) => [a].into(),
                None => SmallVec::new(),
            }
        }

        AppAction::HandleMsgToApp(msg) => {
            match msg {
                MsgToApp::ToggleVisibility => {
                    state.hidden = !state.hidden;

                    if state.hidden {
                        println!("Toggle visibility: hide");
                        app_io.hide_app();
                        SmallVec::new()
                    } else {
                        ctx.send_viewport_cmd(ViewportCommand::Visible(!state.hidden));
                        ctx.send_viewport_cmd(ViewportCommand::Focus);
                        // ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
                        println!("Toggle visibility: show + focus");

                        SmallVec::from_buf([AppAction::DeferToPostRender(Box::new(
                            AppAction::FocusRequest(FocusTarget::CurrentNote),
                        ))])
                    }
                }

                MsgToApp::NoteFileChanged(note_file, path) => {
                    match app_io.try_read_note_if_newer(&path, state.last_saved) {
                        Ok(Some(note_content)) => {
                            if let Some(note) = state.notes.get_mut(&note_file) {
                                // TODO don't reset the cursor
                                note.cursor = None;
                                if note_file == state.selected_note {
                                    state.text_structure = state
                                        .text_structure
                                        .take()
                                        .map(|s| s.recycle(&note_content));
                                }

                                if Some(note_file)
                                    == state
                                        .inline_llm_prompt
                                        .as_ref()
                                        .map(|prompt| prompt.address.note_file)
                                {
                                    // if we get an external text change for the note we currently have an inline prompt reset the prompt
                                    state.inline_llm_prompt = None;
                                }

                                note.text = note_content;
                                state.add_unsaved_change(UnsavedChange::LastUpdated);
                            }

                            [AppAction::EvalNote(note_file)].into()
                        }
                        Ok(None) => {
                            // no updates needed we already have the newest version
                            SmallVec::new()
                        }
                        Err(err) => {
                            // failed to read note file
                            println!("failed to read {path:#?}, err={err:#?}");
                            SmallVec::new()
                        }
                    }
                }

                MsgToApp::GlobalHotkey(hotkey_id) => app_io
                    .try_map_hotkey(hotkey_id)
                    .map(|msg| [AppAction::HandleMsgToApp(msg)].into())
                    .unwrap_or_default(),

                MsgToApp::LLMBlockResponseChunk(resp) => {
                    // TODO(simon): this entire code is VERY ugly, and deserves to be better
                    let note = &mut state.notes.get_mut(&resp.note_id).unwrap();
                    let text = &mut note.text;

                    let text_structure = match resp.note_id == state.selected_note {
                        true => state
                            .text_structure
                            .take()
                            .unwrap_or_else(|| TextStructure::new(text)),
                        false => TextStructure::new(text),
                    };

                    enum InsertMode {
                        Initial { pos: usize },
                        Subsequent { pos: usize },
                    }

                    let insertion_pos = text_structure
                        .filter_map_codeblocks(|lang| (lang == &resp.address).then(|| ()))
                        .next()
                        .map(|(_index, desc, _)| {
                            let entire_block_text = &text[desc.byte_pos.range()];
                            match entire_block_text.lines().count() {
                                // right before "```"
                                2 => InsertMode::Initial {
                                    pos: desc.byte_pos.end - 3,
                                },

                                // right before "\n```"
                                _ => InsertMode::Subsequent {
                                    pos: desc.byte_pos.end - 4,
                                },
                            }
                        });

                    if resp.note_id == state.selected_note {
                        state.text_structure = Some(text_structure);
                    }

                    let mut chunk = resp.chunk;

                    if chunk.contains("```") {
                        // sanitze llm response so it can be nested inside llm output code block
                        chunk = chunk.replace("```", "-```");
                    }

                    insertion_pos
                        .map(|insert_mode| {
                            [AppAction::ApplyTextChanges {
                                target: resp.note_id,
                                changes: [match insert_mode {
                                    InsertMode::Initial { pos } => {
                                        TextChange::Replace(ByteSpan::point(pos), chunk + "\n")
                                    }
                                    InsertMode::Subsequent { pos } => {
                                        TextChange::Replace(ByteSpan::point(pos), chunk)
                                    }
                                }]
                                .into(),
                                should_trigger_eval: false,
                            }]
                            .into()
                        })
                        .unwrap_or_default()
                }

                MsgToApp::InlineLLMResponse {
                    response,
                    address: target_address,
                } => match state.inline_llm_prompt.take() {
                    Some(prompt_state) if prompt_state.address == target_address => {
                        match response {
                            InlineLLMResponseChunk::Chunk(chunk) => {
                                let InlineLLMPropmptState {
                                    mut response_text,
                                    prompt,
                                    address,
                                    mut diff_parts,
                                    layout_job: _,
                                    status,
                                    fresh_response: _,
                                } = prompt_state;

                                response_text.push_str(&chunk);

                                diff_parts.clear();

                                let selection = &state.notes.get(&address.note_file).unwrap().text
                                    [address.span.range()];

                                diff_parts.extend(
                                    TextDiff::from_words(selection, &response_text)
                                        .iter_all_changes()
                                        .map(|change| {
                                            let part_str = change.to_string_lossy().to_string();
                                            match change.tag() {
                                                ChangeTag::Equal => TextDiffPart::Equal(part_str),
                                                ChangeTag::Delete => TextDiffPart::Delete(part_str),
                                                ChangeTag::Insert => TextDiffPart::Insert(part_str),
                                            }
                                        }),
                                );

                                // println!("----diff_parts: {diff_parts:#?}");

                                let layout_job =
                                    create_layout_job_from_text_diff(&diff_parts, &state.theme);

                                state.inline_llm_prompt = Some(InlineLLMPropmptState {
                                    address,
                                    response_text,
                                    diff_parts,
                                    prompt,
                                    layout_job,
                                    status,
                                    fresh_response: true,
                                });
                                SmallVec::new()
                            }

                            InlineLLMResponseChunk::End => {
                                let InlineLLMPropmptState {
                                    response_text,
                                    prompt,
                                    address,
                                    diff_parts,
                                    layout_job,
                                    status,
                                    fresh_response,
                                } = prompt_state;

                                let status = match status {
                                    InlinePromptStatus::NotStarted => InlinePromptStatus::Done {
                                        prompt: prompt.clone(),
                                    },
                                    InlinePromptStatus::Streaming { prompt } => {
                                        InlinePromptStatus::Done { prompt }
                                    }
                                    InlinePromptStatus::Done { prompt } => {
                                        InlinePromptStatus::Done { prompt }
                                    }
                                };

                                state.inline_llm_prompt = Some(InlineLLMPropmptState {
                                    response_text,
                                    prompt,
                                    address,
                                    diff_parts,
                                    layout_job,
                                    status,
                                    fresh_response,
                                });
                                SmallVec::new()
                            }
                        }
                    }

                    prompt_state => {
                        state.inline_llm_prompt = prompt_state;
                        SmallVec::new()
                    }
                },
            }
        }

        AppAction::EvalNote(note_file) => {
            let note = &mut state.notes.get_mut(&note_file).unwrap();
            let text = &mut note.text;

            let text_structure = match note_file == state.selected_note {
                true => state
                    .text_structure
                    .take()
                    .unwrap_or_else(|| TextStructure::new(text)),
                false => TextStructure::new(text),
            };

            let requested_changes = match note_file {
                NoteFile::Note(_) => execute_live_scripts(&text_structure, text),

                NoteFile::Settings => {
                    let mut cx = SettingsNoteEvalContext {
                        cmd_list: &mut state.editor_commands,
                        should_force_eval: false,
                        app_io,
                        llm_settings: &mut state.llm_settings,
                    };

                    execute_code_blocks(&mut cx, &text_structure, &text)
                }
            };

            if note_file == state.selected_note {
                state.text_structure = Some(text_structure);
            }

            requested_changes
                .map(|changes| AppAction::ApplyTextChanges {
                    target: note_file,
                    changes,
                    should_trigger_eval: false,
                })
                .map(|a| [a].into())
                .unwrap_or_default()
        }

        AppAction::AskLLM(question) => {
            app_io.execute_llm_block(question);
            SmallVec::new()
        }
        AppAction::SendFeedback(selected) => {
            let Some(note) = state.notes.get(&selected) else {
                return SmallVec::new();
            };

            if selected == state.selected_note {
                sentry::configure_scope(|scope| {
                    let mut map = std::collections::BTreeMap::new();
                    map.insert(
                        String::from("text_structure"),
                        format!("{:#?}", state.text_structure).into(),
                    );
                    map.insert(
                        String::from("selected_note"),
                        format!("{:?}", state.selected_note).into(),
                    );

                    scope.set_context("state", sentry::protocol::Context::Other(map));
                });

                let result = sentry::capture_message(
                    format!("Feedback: {}", note.text).as_str(),
                    sentry::Level::Info,
                );

                println!("Feedback sent: {:?}", result);

                [AppAction::ApplyTextChanges {
                    target: selected,
                    changes: vec![TextChange::Replace(
                        ByteSpan::point(note.text.len()),
                        // TODO Add a link to join the discord server (as a way to encourage feedback discussion)
                        format!(
                            "\n---\n\
                            Thank you for your feedback! (reference: {:?})\n",
                            result
                        ),
                    )],
                    should_trigger_eval: false,
                }]
                .into()
            } else {
                SmallVec::new()
            }
        }

        AppAction::StartTutorial => {
            // Plan:
            // append "default" notes to the existing notes
            // if a note is empty, just insert as is
            // if there was a conent, insert it in the begining of the note but also add a separator
            // "----END of tutorial----"

            let actions_iter = state
                .notes
                .iter()
                .map(|(&id, note)| (id, note.text.trim().is_empty()))
                .filter_map(|(id, is_empty)| match get_tutorial_note_content(id) {
                    "" => None,
                    tutorial_conent => {
                        let to_insert = match is_empty {
                            true => format!("{cursor}{tutorial_conent}", cursor=TextChange::CURSOR),
                            false => {
                                format!("{cursor}{tutorial_conent}\n\n-------end of tutorial-------\n\n", cursor=TextChange::CURSOR)
                            }
                        };
                        Some((id, TextChange::Replace(ByteSpan::point(0), to_insert)))
                    }
                })
                .map(|(target, change)| AppAction::ApplyTextChanges {
                    target,
                    changes: [change].into(),
                    should_trigger_eval: false,
                });

            SmallVec::from_iter(actions_iter.chain([AppAction::SwitchToNote {
                note_file: NoteFile::Note(0),
                via_shortcut: true,
            }]))
        }

        AppAction::DeferToPostRender(action) => {
            state.deferred_to_post_render.push(*action);
            SmallVec::new()
        }

        AppAction::FocusRequest(target) => {
            // it is possible that text editing was out of focus
            // hence, refocus it again
            ctx.memory_mut(|mem| {
                mem.request_focus(match target {
                    FocusTarget::CurrentNote => text_edit_id,
                    FocusTarget::SpecificId(id) => id,
                });
            });

            SmallVec::new()
        }

        AppAction::OpenNotesInFinder => {
            if let Err(e) = app_io.open_shelv_folder() {
                println!("Error opening shelv folder: {}", e);
            }
            SmallVec::new()
        }

        AppAction::RunLLMBLock(note_file, span_index) => prepare_to_run_llm_block(
            CommandContext {
                app_state: state,
                app_focus: compute_app_focus(ctx, state),
            },
            CodeBlockAddress::TargetBlock(note_file, span_index),
        )
        .unwrap_or_default(),

        AppAction::ShowPrompt(address) => {
            println!("Triggering inline prompt {address:#?}",);

            state.inline_llm_prompt = Some(InlineLLMPropmptState {
                address,
                response_text: "".to_string(),
                diff_parts: vec![],
                layout_job: LayoutJob::default(),
                prompt: "".to_string(),
                status: InlinePromptStatus::NotStarted,
                fresh_response: false,
            });

            SmallVec::from_buf([AppAction::DeferToPostRender(Box::new(
                AppAction::FocusRequest(FocusTarget::SpecificId(
                    compute_inline_prompt_text_input_id(address),
                )),
            ))])
        }

        AppAction::ExecutePrompt => {
            let Some(prompt) = &mut state.inline_llm_prompt else {
                return SmallVec::default();
            };

            let model = state
                .llm_settings
                .as_ref()
                .map(|s| s.model.clone())
                .unwrap_or_else(|| DEFAULT_LLM_MODEL.to_string());

            prompt.response_text = "".to_string();
            prompt.layout_job = LayoutJob::default();
            prompt.status = InlinePromptStatus::Streaming {
                prompt: prompt.prompt.clone(),
            };

            let prompt_span = prompt.address.span;
            let note_text = &state.notes.get(&prompt.address.note_file).unwrap().text;
            let selection = note_text[prompt_span.range()].to_string();
            let before_selection = note_text[..prompt_span.start].to_string();
            let after_selection = note_text[prompt_span.end..].to_string();

            app_io.execute_llm_prompt(LLMPromptRequest {
                prompt: prompt.prompt.clone(),
                selection,
                model,
                system_prompt: state
                    .llm_settings
                    .as_ref()
                    .and_then(|settings| settings.system_prompt.clone()),
                selection_location: prompt.address,
                before_selection,
                after_selection,
            });

            SmallVec::new()
        }

        AppAction::AcceptPromptSuggestion { accept } => {
            let mut resulting_actions = SmallVec::from_buf([AppAction::DeferToPostRender(
                Box::new(AppAction::FocusRequest(FocusTarget::CurrentNote)),
            )]);

            let Some(prompt) = state.inline_llm_prompt.take() else {
                return resulting_actions;
            };
            let target_note = state.notes.get_mut(&prompt.address.note_file).unwrap();

            let text_lenght = target_note.text.len();
            let ByteSpan { start, end, .. } = prompt.address.span;
            target_note.cursor = target_note.cursor.or(Some(UnOrderedByteSpan::new(
                start.min(text_lenght),
                end.min(text_lenght),
            )));

            if accept {
                let changes = vec![TextChange::Replace(
                    prompt.address.span,
                    prompt.response_text,
                )];

                resulting_actions.push(AppAction::ApplyTextChanges {
                    target: prompt.address.note_file,
                    changes,
                    should_trigger_eval: true,
                });

                resulting_actions
            } else {
                resulting_actions
            }
        }
    }
}

pub fn compute_app_focus(ctx: &Context, app_state: &AppState) -> AppFocusState {
    ctx.memory(|m| AppFocusState {
        is_menu_opened: m.any_popup_open(),
        focus: match m.focused() {
            Some(id)
                if app_state
                    .inline_llm_prompt
                    .as_ref()
                    .map(|p| compute_inline_prompt_text_input_id(p.address))
                    == Some(id) =>
            {
                Some(AppFocus::InlinePropmptEditor)
            }

            Some(id) if compute_editor_text_id(app_state.selected_note) == id => {
                Some(AppFocus::NoteEditor)
            }

            _ => None,
        },
    })
}

use std::{io, path::PathBuf};

use eframe::egui::{text::LayoutJob, Context, Id, KeyboardShortcut, OpenUrl, ViewportCommand};

use similar::{ChangeTag, TextDiff};
use smallvec::{smallvec, SmallVec};

use crate::{
    app_state::{
        AppState, InlineLLMPropmptState, InlineLLMResponseChunk, MsgToApp, TextSelectionAddress,
        UnsavedChange,
    },
    byte_span::{ByteSpan, UnOrderedByteSpan},
    commands::run_llm::{prepare_to_run_llm_block, CodeBlockAddress, DEFAULT_LLM_MODEL},
    effects::text_change_effect::{apply_text_changes, TextChange},
    persistent_state::{get_tutorial_note_content, NoteFile},
    scripting::{execute_code_blocks, execute_live_scripts},
    settings::SettingsNoteEvalContext,
    text_structure::{
        create_layout_job_from_text_diff, SpanIndex, SpanKind, SpanMeta, TextDiffPart,
        TextStructure, TextStructureVersion,
    },
};

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
    FocusOnEditor,
    OpenNotesInFinder,
    TriggerInlinePrompt(ByteSpan, NoteFile, TextStructureVersion),
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
pub struct InlineLLMPromptRequest {
    pub prompt: String,
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

    fn ask_llm(&self, question: LLMBlockRequest);
    fn ask_llm_inline(&self, quesion: InlineLLMPromptRequest);
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

                if let Some(cur_note) = state.notes.get(&note_file) {
                    let text = &cur_note.text;
                    state.selected_note = note_file;
                    state.text_structure = state.text_structure.take().map(|s| s.recycle(text));
                };
            }

            match via_shortcut {
                true => [AppAction::DeferToPostRender(Box::new(
                    AppAction::FocusOnEditor,
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
                    } else {
                        ctx.send_viewport_cmd(ViewportCommand::Visible(!state.hidden));
                        ctx.send_viewport_cmd(ViewportCommand::Focus);
                        ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
                        println!("Toggle visibility: show + focus");
                    }

                    SmallVec::new()
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
                                    address,
                                    mut diff_parts,
                                    layout_job: _,
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

                                let layout_job =
                                    create_layout_job_from_text_diff(&diff_parts, &state.theme);

                                state.inline_llm_prompt = Some(InlineLLMPropmptState {
                                    address,
                                    response_text,
                                    diff_parts,
                                    layout_job,
                                });
                                SmallVec::new()
                            }

                            InlineLLMResponseChunk::End => {
                                state.inline_llm_prompt = Some(prompt_state);
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
            app_io.ask_llm(question);
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

        AppAction::FocusOnEditor => {
            // it is possible that text editing was out of focus
            // hence, refocus it again
            ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
            SmallVec::new()
        }

        AppAction::OpenNotesInFinder => {
            if let Err(e) = app_io.open_shelv_folder() {
                println!("Error opening shelv folder: {}", e);
            }
            SmallVec::new()
        }

        AppAction::RunLLMBLock(note_file, span_index) => prepare_to_run_llm_block(
            crate::command::CommandContext { app_state: state },
            CodeBlockAddress::TargetBlock(note_file, span_index),
        )
        .unwrap_or_default(),

        AppAction::TriggerInlinePrompt(byte_span, note_file, version) => {
            println!(
                "Triggering inline prompt at {:?} in {:?}, version={:?}",
                byte_span, note_file, version
            );

            // TODO clean it up when detecting changes
            let address = TextSelectionAddress {
                span: byte_span,
                note_file,
                text_version: version,
            };

            state.inline_llm_prompt = Some(InlineLLMPropmptState {
                address,
                response_text: "".to_string(),
                diff_parts: vec![],
                layout_job: LayoutJob::default(),
            });

            let model = state
                .llm_settings
                .as_ref()
                .map(|s| s.model.clone())
                .unwrap_or_else(|| DEFAULT_LLM_MODEL.to_string());

            app_io.ask_llm_inline(InlineLLMPromptRequest {
                prompt: "Fix gramma, stylistic and spelling errors".to_string(),
                selection: state.notes.get(&note_file).unwrap().text[byte_span.range()].to_string(),
                model,
                system_prompt: Some(
                    "Answer ONLY what you have been asked for, no extra comments".to_string(),
                ),

                selection_location: address,
            });

            SmallVec::new()
        }
    }
}

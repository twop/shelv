use eframe::egui::{Context, Id, OpenUrl};

use crate::{
    app_state::{AppState, UnsavedChange},
    byte_span::UnOrderedByteSpan,
    effects::text_change_effect::{apply_text_changes, TextChange},
    persistent_state::NoteFile,
};

#[derive(Debug)]
pub enum AppAction {
    SwitchToNote { index: u32, via_shortcut: bool },
    // HideApp,
    // ShowApp,
    OpenLink(String),
    SetWindowPinned(bool),
    ApplyTextChanges(NoteFile, Vec<TextChange>),
}

pub fn process_app_action(
    action: AppAction,
    ctx: &Context,
    state: &mut AppState,
    text_edit_id: Id,
) {
    match action {
        AppAction::SwitchToNote {
            index,
            via_shortcut,
        } => {
            if index != state.selected_note {
                state.unsaved_changes.push(UnsavedChange::SelectionChanged);

                if via_shortcut {
                    let note = &mut state.notes[index as usize];
                    note.cursor = match note.cursor.clone() {
                        None => {
                            let len = note.text.len();
                            Some(UnOrderedByteSpan::new(len, len))
                        }
                        prev => prev,
                    };

                    // it is possible that text editing was out of focus
                    // hence, refocus it again
                    ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
                } else {
                    // means that we reselected via UI

                    // if that is the case then reset cursors from both of the notes
                    state.notes[state.selected_note as usize].cursor = None;
                    state.notes[index as usize].cursor = None;
                }

                let text = &state.notes[index as usize].text;
                state.selected_note = index;
                state.text_structure = state.text_structure.take().map(|s| s.recycle(text));
            }
        }
        AppAction::OpenLink(url) => ctx.open_url(OpenUrl::new_tab(url)),

        AppAction::SetWindowPinned(is_pinned) => {
            state.is_pinned = is_pinned;
        }

        AppAction::ApplyTextChanges(note_file, changes) => {
            let NoteFile::Note(note_index) = note_file else {
                return;
            };

            let index = note_index as usize;
            let note = &mut state.notes[index];
            let text = &mut note.text;
            let cursor = note.cursor;
            if let Some(byte_range) = cursor {
                if let Ok(updated_cursor) = apply_text_changes(text, byte_range, changes) {
                    if note_index == state.selected_note {
                        // if the changes are for the selected note we need to recompute TextStructure
                        state.text_structure = state.text_structure.take().map(|s| s.recycle(text));
                    }
                    note.cursor = Some(updated_cursor);
                }
            }
        }
    }
}

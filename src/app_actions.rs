use eframe::egui::{text::CCursor, text_edit::CCursorRange, Context, Id, OpenUrl, TextEdit};

use crate::app_state::AppState;

pub enum AppAction {
    SwitchToNote { index: u32, via_shortcut: bool },
    // HideApp,
    // ShowApp,
    OpenLink(String),
}

pub fn proccess_app_action(
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
                state.save_to_storage = true;

                if via_shortcut {
                    let note = &mut state.notes[index as usize];
                    note.cursor = match note.cursor {
                        None => Some(CCursorRange::one(CCursor::new(note.text.chars().count()))),
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

                state.selected_note = index;
            }
        }
        AppAction::OpenLink(url) => ctx.open_url(OpenUrl::new_tab(url)),
    }
}

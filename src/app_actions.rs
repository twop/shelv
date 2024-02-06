use eframe::egui::{
    self, text::CCursor, text_edit::CCursorRange, Context, Id, OpenUrl, TextBuffer, TextEdit,
};
use smallvec::SmallVec;

use crate::{app_state::AppState, text_structure::SpanKind};

pub enum AppAction {
    SwitchToNote { index: u32, via_shortcut: bool },
    // HideApp,
    // ShowApp,
    OpenLink(String),
    IncreaseFontSize,
    DecreaseFontSize,
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
        AppAction::IncreaseFontSize => {
            state.font_scale += 1;
        }
        AppAction::DecreaseFontSize => {
            state.font_scale -= 1;
        }
    }
}

// required
// cursor
// being inside the list
pub fn on_enter_inside_list_item(state: &mut AppState) {
    // ---- AUTO INDENT LISTS ----

    let current_note = &mut state.notes[state.selected_note as usize];

    if let (Some(text_cursor_range), Some(computed_layout)) =
        (current_note.cursor, &state.computed_layout)
    {
        use egui::TextBuffer as _;

        let [char_range_start, char_range_end] = text_cursor_range.sorted().map(|c| c.index);

        let [byte_start, byte_end] = [char_range_start, char_range_end]
            .map(|char_idx| current_note.text.byte_index_from_char_index(char_idx));

        let structure = &computed_layout.text_structure;
        if let Some((span_range, item_index)) =
            structure.find_span_at(SpanKind::ListItem, byte_start..byte_end)
        {
            // requirements
            // 1.depth
            // 2. if numbered what is the intex
            // 3. if numbered what are the siblings and what are the numbers (to adjust enumeration)
            // 4. Is there any content inside the list, if not need to break the list (if last)
            //
            //
            // Plan
            // 0. Check the list item content => "is_empty" list item ready to be broken
            // 1. Get parent (list) => numbered
            // 2. traverse parent chain (kind = list) => depth
            // 3. if list is numbered
            //    yay: collect all list items (including this list item) => ability to modify the enumeration
            //    nay: easy, can do all operations needed

            let parents: SmallVec<[_; 4]> = structure
                .iterate_parents_of(item_index)
                .filter(|desc| desc.kind == SpanKind::List)
                .collect();

            //--------
            // structure.find_interactive_text_part(byte_cursor_pos)
            // let text_to_insert = match inside_list_item.started_numbered_index {
            //     Some(starting_index) => format!(
            //         "{}{}. ",
            //         "\t".repeat(inside_list_item.depth as usize),
            //         starting_index + inside_list_item.item_index as u64 + 1
            //     ),
            //     None => {
            //         format!("{}- ", "\t".repeat(inside_list_item.depth as usize))
            //     }
            // };

            // current_note
            //     .text
            //     .insert_text(text_to_insert.as_str(), char_range_start);
        }
    }
}

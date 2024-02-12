use std::ops::{Deref, Range};

use eframe::egui::{
    text::CCursor, text_edit::CCursorRange, Context, Id, OpenUrl, TextBuffer, TextEdit,
};
use smallvec::SmallVec;

use crate::{
    app_state::AppState,
    text_structure::{ByteRange, SpanKind, SpanMeta, TextStructure},
};

pub enum TextChange {
    // Delete(ByteRange),
    Replace(ByteRange, String),
    // Insert { insertion: String, byte_pos: usize },
}

impl TextChange {
    const CURSOR_EDGE: &'static str = "{|}";
    const CURSOR: &'static str = "{||}";

    pub fn try_extract_cursor(mut text: String) -> (String, Option<ByteRange>) {
        // let mut text = text.to_string();
        if let Some(start) = text.find(TextChange::CURSOR) {
            text.replace_range(start..(start + TextChange::CURSOR.len()), "");
            (text, Some(ByteRange(start..start)))
        } else {
            let Some(start) = text.find(TextChange::CURSOR_EDGE) else {
                return (text, None);
            };
            text.replace_range(start..(start + TextChange::CURSOR_EDGE.len()), "");
            let Some(end) = text.find(TextChange::CURSOR_EDGE) else {
                // undo the first removal
                text.insert_str(start, Self::CURSOR_EDGE);
                return (text, None);
            };
            text.replace_range(end..(end + TextChange::CURSOR_EDGE.len()), "");
            (text, Some(ByteRange(start..end)))
        }
    }
}

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
pub fn on_enter_inside_list_item(
    structure: &TextStructure,
    text: &str,
    cursor: ByteRange,
) -> Option<Vec<TextChange>> {
    let (span_range, item_index) = structure.find_span_at(SpanKind::ListItem, cursor.clone())?;

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
    //

    let parents: SmallVec<[_; 4]> = structure
        .iterate_parents_of(item_index)
        .filter(|(_, desc)| desc.kind == SpanKind::List)
        .filter_map(|(idx, _)| match structure.find_meta(idx) {
            Some(SpanMeta::List(list_desc)) => Some(list_desc),
            _ => None,
        })
        .collect();

    let depth = parents.len() - 1;

    let is_empty_list_item = structure.iterate_immediate_children_of(item_index).count() == 0;

    // first parent is the immediate parent
    if let Some(starting_index) = parents[0].starting_index {
        todo!()
    } else {
        // means that the list is unordered

        if is_empty_list_item {
            // then we just remove the entire list item and break
            Some(
                [TextChange::Replace(
                    ByteRange(span_range),
                    format!("{}\n", TextChange::CURSOR),
                )]
                .into(),
            )
        } else {
            Some(
                [TextChange::Replace(
                    ByteRange(cursor.clone()),
                    "\n".to_string() + &"\t".repeat(depth) + "- " + TextChange::CURSOR,
                )]
                .into(),
            )
        }
    }

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
    // }
}

#[derive(Debug)]
pub enum TextChangeError {
    OverlappingChanges,
}

pub fn apply_text_changes(
    text: &mut String,
    prev_cursor: ByteRange,
    changes: impl IntoIterator<Item = TextChange>,
) -> Result<ByteRange, TextChangeError> {
    #[derive(Debug, Clone)]
    // #[allow(dead_code)]
    struct Log {
        removed: Range<usize>,
        inserted_len: usize,
    }
    type Logs = SmallVec<[Log; 4]>;

    // None -> there is an overlap
    // Some -> successfully adjusted
    fn append(
        range: &ByteRange,
        to_insert: usize,
        logs: &[Log],
    ) -> Result<(Logs, Range<usize>), TextChangeError> {
        let mut res: Logs = logs.iter().map(Log::clone).collect();
        res.sort_by(|a, b| a.removed.end.cmp(&b.removed.end));

        let mut actual_range: Range<usize> = range.deref().clone();

        let mut split_point: Option<usize> = None;

        // find a splitting point in the insertion logs
        for (i, log) in logs.iter().enumerate() {
            // check for overlaps
            if log.removed.contains(&actual_range.start) || log.removed.contains(&actual_range.end)
            {
                // it means that we have overlapping ranges for removal
                // that is not allowed
                return Err(TextChangeError::OverlappingChanges);
            }
            // println!("\n\n$append\n log={res:?} to_add={actual_range:?}");
            // for log in res.iter() {
            // }

            if log.removed.end <= actual_range.start {
                // that means that the removal happened earlier
                // thus, we need to adjust starting position
                let delta = log.inserted_len - log.removed.len();
                actual_range = (actual_range.start + delta)..(actual_range.end + delta);
            } else {
                split_point = Some(i);
                break;
            }
        }

        // if we need to insert somewhere in the middle we need to shift spans that come after
        if let Some(split_point) = split_point {
            // we need to move what comes after the split
            let delta = to_insert - actual_range.len();
            for log in res[split_point..].iter_mut() {
                log.removed = (log.removed.start + delta)..(log.removed.end + delta);
            }
        }

        // finally insert the element at a proper position
        res.insert(
            split_point.unwrap_or(res.len()),
            Log {
                removed: actual_range.clone(),
                inserted_len: to_insert,
            },
        );

        Ok((res, actual_range))
    }

    let mut logs: Logs = Logs::new();

    let mut actual_changes: SmallVec<[TextChange; 4]> = SmallVec::new();

    let mut inserted_cursor: Option<ByteRange> = None;

    for change in changes.into_iter() {
        match change {
            TextChange::Replace(range, with) => {
                let (with, extracted_cursor) = TextChange::try_extract_cursor(with);
                let to_insert = with.len();
                let (new_logs, target) = append(&range, to_insert, &logs)?;
                logs = new_logs;
                if let Some(extracted_cursor) = extracted_cursor {
                    inserted_cursor = Some(ByteRange(
                        target.start + extracted_cursor.start..target.start + extracted_cursor.end,
                    ));
                }
                actual_changes.push(TextChange::Replace(ByteRange(target), with));
            }
        }
    }

    for change in actual_changes.into_iter() {
        match change {
            TextChange::Replace(range, with) => {
                text.replace_range(range.0, &with);
            }
        }
    }

    Ok(inserted_cursor.unwrap_or(prev_cursor))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encode_cursor(text: &str, cursor: ByteRange) -> String {
        let mut text = text.to_string();
        if cursor.is_empty() {
            text.insert_str(cursor.start, TextChange::CURSOR);
        } else {
            text.insert_str(cursor.start, TextChange::CURSOR_EDGE);
            text.insert_str(
                cursor.end + TextChange::CURSOR_EDGE.len(),
                TextChange::CURSOR_EDGE,
            );
        }
        text
    }

    #[test]
    pub fn test_splitting_list_item_via_enter() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- a{||}b".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- ab");
        assert_eq!(cursor, ByteRange(3..3));

        let structure = TextStructure::create_from(&text);

        let changes =
            on_enter_inside_list_item(&structure, &text, ByteRange(cursor.clone())).unwrap();

        let cursor = apply_text_changes(&mut text, cursor, changes).unwrap();
        assert_eq!(encode_cursor(&text, cursor), "- a\n- {||}b");
    }

    #[test]
    pub fn test_splitting_list_item_via_enter_with_selection() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- {|}a{|}b".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- ab");
        assert_eq!(cursor, ByteRange(2..3));

        let structure = TextStructure::create_from(&text);

        let changes =
            on_enter_inside_list_item(&structure, &text, ByteRange(cursor.clone())).unwrap();

        let cursor = apply_text_changes(&mut text, cursor, changes).unwrap();
        assert_eq!(encode_cursor(&text, cursor), "- \n- {||}b");
    }

    #[test]
    pub fn test_removing_emty_list_item_on_enter() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- {||}\n- a".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- \n- a");

        let structure = TextStructure::create_from(&text);

        let changes =
            on_enter_inside_list_item(&structure, &text, ByteRange(cursor.clone())).unwrap();

        let cursor = apply_text_changes(&mut text, cursor, changes).unwrap();
        assert_eq!(encode_cursor(&text, cursor), "{||}\n- a");
    }

    // --------- Text changes cursor tests --------

    #[test]
    pub fn test_cursor_extraction_from_string() {
        let (text, cursor) = TextChange::try_extract_cursor("- a{||}b".to_string());
        assert_eq!(text, "- ab");
        assert_eq!(cursor, Some(ByteRange(3..3)));

        let (text, cursor) = TextChange::try_extract_cursor("- {|}a{|}b".to_string());
        assert_eq!(text, "- ab");
        assert_eq!(cursor, Some(ByteRange(2..3)));

        let (text, cursor) = TextChange::try_extract_cursor("- a{|}b".to_string());
        assert_eq!(text, "- a{|}b");
        assert_eq!(cursor, None);
    }

    // --------- Apply changes tests --------
    #[test]
    pub fn test_several_text_changes_in_order() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            TextChange::Replace(ByteRange(a_pos..a_pos + 1), "hello".into()),
            TextChange::Replace(ByteRange(b_pos..b_pos + 1), "world".into()),
            TextChange::Replace(ByteRange(b_pos + 1..b_pos + 1), "!".into()),
        ];

        apply_text_changes(&mut text, ByteRange(0..0), changes).unwrap();
        assert_eq!(text, "hello world!");
    }

    #[test]
    pub fn test_several_text_changes_out_of_order() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            TextChange::Replace(ByteRange(b_pos + 1..b_pos + 1), "!".into()),
            TextChange::Replace(ByteRange(b_pos..b_pos + 1), "world".into()),
            TextChange::Replace(ByteRange(a_pos..a_pos + 1), "hello".into()),
        ];

        apply_text_changes(&mut text, ByteRange(0..0), changes).unwrap();
        assert_eq!(text, "hello world!");
    }

    #[test]
    pub fn test_overlapping_text_changes_are_not_allowed() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            // captures "a b"
            TextChange::Replace(ByteRange(a_pos..b_pos + 1), "hello".into()),
            // captures "b"
            TextChange::Replace(ByteRange(b_pos..b_pos + 1), "world".into()),
        ];

        let cursor = apply_text_changes(&mut text, ByteRange(0..0), changes);
        assert!(matches!(cursor, Err(TextChangeError::OverlappingChanges)));
        assert_eq!(text, "a b");
    }
}

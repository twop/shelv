use eframe::egui::{Context, Id, KeyboardShortcut, Modifiers, OpenUrl};

use smallvec::SmallVec;

use crate::{
    app_state::{AppState, UnsavedChange},
    byte_span::{ByteSpan, RangeRelation, UnOrderedByteSpan},
    commands::EditorCommand,
    text_structure::{ListDesc, SpanKind, SpanMeta, TextStructure},
};

#[derive(Debug)]
pub enum TextChange {
    // Delete(ByteRange),
    Replace(ByteSpan, String),
    // Insert { insertion: String, byte_pos: usize },
}

impl TextChange {
    pub const CURSOR_EDGE: &'static str = "{|}";
    pub const CURSOR: &'static str = "{||}";

    pub fn try_extract_cursor(mut text: String) -> (String, Option<ByteSpan>) {
        // let mut text = text.to_string();
        if let Some(start) = text.find(TextChange::CURSOR) {
            text.replace_range(start..(start + TextChange::CURSOR.len()), "");
            (text, Some(ByteSpan::new(start, start)))
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
            (text, Some(ByteSpan::new(start, end)))
        }
    }

    pub fn encode_cursor(text: &str, cursor: UnOrderedByteSpan) -> String {
        let mut text = text.to_string();
        let cursor = cursor.ordered();
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
}

pub enum AppAction {
    SwitchToNote { index: u32, via_shortcut: bool },
    // HideApp,
    // ShowApp,
    OpenLink(String),
    IncreaseFontSize,
    DecreaseFontSize,
    SetWindowPinned(bool),
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
        AppAction::IncreaseFontSize => {
            state.font_scale += 1;
        }
        AppAction::DecreaseFontSize => {
            state.font_scale -= 1;
        }

        AppAction::SetWindowPinned(is_pinned) => {
            state.is_pinned = is_pinned;
        }
    }
}

pub struct TabInsideListCommand;
pub struct ShiftTabInsideListCommand;
pub struct EnterInsideListCommand;
pub struct SpaceAfterTaskMarkersCommand;

impl EditorCommand for SpaceAfterTaskMarkersCommand {
    fn name(&self) -> &str {
        "SpaceAfterTaskMarker"
    }

    fn shortcut(&self) -> KeyboardShortcut {
        KeyboardShortcut::new(Modifiers::NONE, eframe::egui::Key::Space)
    }

    fn try_handle(
        &self,
        text_structure: &TextStructure,
        text: &str,
        byte_cursor: ByteSpan,
    ) -> Option<Vec<TextChange>> {
        on_space_after_task_markers(text_structure, text, byte_cursor)
    }
}

impl EditorCommand for TabInsideListCommand {
    fn name(&self) -> &str {
        "TabInsideList"
    }

    fn shortcut(&self) -> KeyboardShortcut {
        KeyboardShortcut::new(Modifiers::NONE, eframe::egui::Key::Tab)
    }

    fn try_handle(
        &self,
        text_structure: &TextStructure,
        text: &str,
        byte_cursor: ByteSpan,
    ) -> Option<Vec<TextChange>> {
        on_tab_inside_list(text_structure, text, byte_cursor)
    }
}

impl EditorCommand for ShiftTabInsideListCommand {
    fn name(&self) -> &str {
        "ShiftTabInsideList"
    }

    fn shortcut(&self) -> KeyboardShortcut {
        KeyboardShortcut::new(Modifiers::SHIFT, eframe::egui::Key::Tab)
    }

    fn try_handle(
        &self,
        text_structure: &TextStructure,
        text: &str,
        byte_cursor: ByteSpan,
    ) -> Option<Vec<TextChange>> {
        on_shift_tab_inside_list(text_structure, text, byte_cursor)
    }
}

impl EditorCommand for EnterInsideListCommand {
    fn name(&self) -> &str {
        "EnterInsideList"
    }

    fn shortcut(&self) -> KeyboardShortcut {
        KeyboardShortcut::new(Modifiers::NONE, eframe::egui::Key::Enter)
    }

    fn try_handle(
        &self,
        text_structure: &TextStructure,
        text: &str,
        byte_cursor: ByteSpan,
    ) -> Option<Vec<TextChange>> {
        on_enter_inside_list_item(text_structure, text, byte_cursor)
    }
}

fn select_unordered_list_marker(depth: usize) -> &'static str {
    match depth {
        0 => "-",
        _ => "*",
    }
}

// handler on ENTER
fn on_enter_inside_list_item(
    structure: &TextStructure,
    text: &str,
    cursor: ByteSpan,
) -> Option<Vec<TextChange>> {
    let (span_range, item_index) = structure.find_span_at(SpanKind::ListItem, cursor.clone())?;

    // TODO actually check if the cursor inside a symbol
    // like `{||}-` or `1{||}2.`, note that the latter will likely break
    if span_range.start == cursor.start {
        // it means that we are right in the begining of the list item
        // like so `{||}- a` or `{||}1. a`, so process those normally
        // Note that Bear.app has a special handling for that:
        // it puts the item as the top list and decreses nesting if needed for the following items
        return None;
    }

    let parents: SmallVec<[_; 4]> = structure
        .iterate_parents_of(item_index)
        .filter(|(_, desc)| desc.kind == SpanKind::List)
        .filter_map(|(idx, desc)| match structure.find_meta(idx) {
            Some(SpanMeta::List(list_desc)) => Some((idx, list_desc, desc.byte_pos)),
            _ => None,
        })
        .collect();

    let depth = parents.len() - 1;

    let is_empty_list_item = structure
        .iterate_immediate_children_of(item_index)
        .filter(|(_, desc)| desc.kind != SpanKind::TaskMarker)
        .count()
        == 0;

    let (_, _, outer_parent_pos) = parents.last().unwrap();
    // first parent is the immediate parent
    match parents[0] {
        (
            parent_list_index,
            ListDesc {
                starting_index: Some(starting_index),
                ..
            },
            _,
        ) => {
            // means that is a numbered list
            let list_items: SmallVec<[_; 6]> = structure
                .iterate_immediate_children_of(parent_list_index)
                .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
                .enumerate()
                .collect();

            // "\n".trim_end_matches(|c: char| c.is_whitespace() && c != '\n');
            if is_empty_list_item {
                // means that we need to remove the current list

                // this removed any "\t" or spaces on the same line
                let parent_text_before_span = &text[outer_parent_pos.start..span_range.start];
                let trimmed_to_new_line = parent_text_before_span
                    .trim_end_matches(|c: char| c.is_whitespace() && c != '\n');

                let mut changes = vec![TextChange::Replace(
                    ByteSpan::new(
                        span_range.start + trimmed_to_new_line.len()
                            - parent_text_before_span.len(),
                        span_range.end,
                    ),
                    format!("{}", TextChange::CURSOR),
                )];

                // and then adjust the ordering for the rest
                for (index, (_, list_item)) in list_items
                    .into_iter()
                    .skip_while(|(_, (idx, _))| idx != &item_index)
                    .skip(1)
                {
                    // now for each following list item we need to set the proper index
                    // note that -1 is to take into account item we just removed
                    let intended_number = *starting_index + index as u64 - 1;

                    let item_text = &text[list_item.byte_pos.range()];
                    // println!("list_items.enumerate(): item=`{}`", item_text);

                    if let Some(dot_pos) = item_text.find(".") {
                        changes.push(TextChange::Replace(
                            ByteSpan::new(
                                list_item.byte_pos.start,
                                list_item.byte_pos.start + dot_pos,
                            ),
                            format!("{}", intended_number),
                        ))
                    }
                }

                Some(changes)
            } else {
                let (item_pos_in_list, _) = list_items
                    .iter()
                    .find(|(_, (span_index, _))| span_index == &item_index)?;
                let item_pos_in_list = *item_pos_in_list;

                // first split the first one in half
                let mut changes = vec![TextChange::Replace(
                    cursor.clone(),
                    format!(
                        "\n{dep}{n}. {cur}",
                        dep = "\t".repeat(depth),
                        n = *starting_index + (item_pos_in_list as u64) + 1,
                        cur = TextChange::CURSOR
                    ),
                )];

                // and then adjust the ordering for the rest
                for (index, (_, list_item)) in list_items.into_iter() {
                    // now for each following list item we need to set the proper index
                    let intended_number = match index > item_pos_in_list {
                        true => *starting_index + index as u64 + 1,
                        false => *starting_index + index as u64,
                    };

                    // TODO only modify items that actually need adjustments
                    let item_text = &text[list_item.byte_pos.range()];
                    if let Some(dot_pos) = item_text.find(".") {
                        changes.push(TextChange::Replace(
                            ByteSpan::new(
                                list_item.byte_pos.start,
                                list_item.byte_pos.start + dot_pos,
                            ),
                            format!("{}", intended_number),
                        ))
                    }
                }

                Some(changes)
            }
        }
        _ => {
            // means that the list is unordered

            if is_empty_list_item {
                // then we just remove the entire list item and break

                // this removed any "\t" or spaces on the same line
                let parent_text_before_span = &text[outer_parent_pos.start..span_range.start];
                let trimmed_to_new_line = parent_text_before_span
                    .trim_end_matches(|c: char| c.is_whitespace() && c != '\n');
                // println!("$$ {trimmed_to_new_line}\n##{parent_text_before_span}");

                Some(vec![TextChange::Replace(
                    ByteSpan::new(
                        span_range.start + trimmed_to_new_line.len()
                            - parent_text_before_span.len(),
                        span_range.end,
                    ),
                    format!("{}", TextChange::CURSOR),
                )])
            } else {
                let has_task_marker = structure
                    .iterate_immediate_children_of(item_index)
                    .any(|(_, desc)| desc.kind == SpanKind::TaskMarker);
                // cond ? the_true : the_false
                Some(vec![TextChange::Replace(
                    cursor.clone(),
                    "\n".to_string()
                        + &"\t".repeat(depth)
                        + select_unordered_list_marker(depth)
                        + if has_task_marker { " [ ] " } else { " " }
                        + TextChange::CURSOR,
                )])
            }
        }
    }
}

fn on_shift_tab_inside_list(
    structure: &TextStructure,
    text: &str,
    cursor: ByteSpan,
) -> Option<Vec<TextChange>> {
    let (span_range, item_index) = structure.find_span_at(SpanKind::ListItem, cursor.clone())?;

    if text.get(span_range.start..cursor.start)?.contains("\n") {
        return None;
    }

    let parents: SmallVec<[_; 4]> = structure
        .iterate_parents_of(item_index)
        .filter(|(_, desc)| desc.kind == SpanKind::List)
        .filter_map(|(idx, _)| match structure.find_meta(idx) {
            Some(SpanMeta::List(list_desc)) => Some((idx, list_desc)),
            _ => None,
        })
        .collect();

    let depth = parents.len() - 1;

    // let is_empty_list_item = structure.iterate_immediate_children_of(item_index).count() == 0;

    // first parent is the immediate parent
    match parents[0] {
        (
            parent_list_index,
            ListDesc {
                starting_index: Some(starting_index),
                ..
            },
        ) => None,
        _ => {
            // means that the list is unordered
            // move just the item by itself
            let mut changes = vec![];

            let t = &text[..span_range.start];

            if depth > 0 && t.ends_with("\t") {
                // move itself
                changes.push(TextChange::Replace(
                    ByteSpan::new(span_range.start - 1, span_range.start + 1), //this is for "-" -> "*" replacement
                    format!("{}", select_unordered_list_marker(depth - 1)),
                ));
            } else {
                return None;
            };
            Some(changes)
        }
    }
}

fn on_tab_inside_list(
    structure: &TextStructure,
    text: &str,
    cursor: ByteSpan,
) -> Option<Vec<TextChange>> {
    let (span_range, item_index) = structure.find_span_at(SpanKind::ListItem, cursor.clone())?;

    if text.get(span_range.start..cursor.start)?.contains("\n") {
        return None;
    }

    let parents: SmallVec<[_; 4]> = structure
        .iterate_parents_of(item_index)
        .filter(|(_, desc)| desc.kind == SpanKind::List)
        .filter_map(|(idx, _)| match structure.find_meta(idx) {
            Some(SpanMeta::List(list_desc)) => Some((idx, list_desc)),
            _ => None,
        })
        .collect();

    let depth = parents.len() - 1;

    // let is_empty_list_item = structure.iterate_immediate_children_of(item_index).count() == 0;

    // first parent is the immediate parent
    match parents[0] {
        (
            parent_list_index,
            ListDesc {
                starting_index: Some(starting_index),
                ..
            },
        ) => {
            // ^^ means that is a numbered list
            let list_items: SmallVec<[_; 6]> = structure
                .iterate_immediate_children_of(parent_list_index)
                .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
                .enumerate()
                .collect();

            let (item_pos_in_list, _) = list_items
                .iter()
                .find(|(_, (span_index, _))| span_index == &item_index)?;
            let item_pos_in_list = *item_pos_in_list;

            // // first split the first one in half
            let mut changes: Vec<TextChange> = vec![];

            // the numbered items after the item now will need to be adjusted by -1
            for (index, (_, list_item)) in list_items[item_pos_in_list + 1..].into_iter() {
                let intended_number = *starting_index + *index as u64 - 1;

                // TODO only modify items that actually need adjustments
                let item_text = &text[list_item.byte_pos.range()];
                if let Some(dot_pos) = item_text.find(".") {
                    changes.push(TextChange::Replace(
                        ByteSpan::new(list_item.byte_pos.start, list_item.byte_pos.start + dot_pos),
                        format!("{}", intended_number),
                    ))
                }
            }

            // move itself, note that now the index starts with "1"
            if let Some(dot_pos) = &text[span_range.range()].find(".") {
                changes.push(TextChange::Replace(
                    ByteSpan::new(span_range.start, span_range.start + dot_pos),
                    format!("\t{}", 1),
                ))
            }

            // finally increase identation of inner items
            changes.extend(increase_nesting_for_lists(structure, item_index));

            Some(changes)
        }
        _ => {
            // means that the list is unordered

            // move all inner items of the list item
            let mut changes = increase_nesting_for_lists(structure, item_index);

            // move itself
            changes.push(TextChange::Replace(
                ByteSpan::new(span_range.start, span_range.start + 1), //this is for "-" -> "*" replacement
                format!("\t{}", select_unordered_list_marker(depth + 1)),
            ));

            Some(changes)
        }
    }
}

fn increase_nesting_for_lists(
    structure: &TextStructure,
    item_index: crate::text_structure::SpanIndex,
) -> Vec<TextChange> {
    let mut changes = vec![];

    for (nested_item_index, nested_item_des) in structure
        .iterate_children_recursively_of(item_index)
        .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
    {
        let parents: SmallVec<[_; 4]> = structure
            .iterate_parents_of(nested_item_index)
            .filter(|(_, desc)| desc.kind == SpanKind::List)
            .filter_map(|(idx, _)| match structure.find_meta(idx) {
                Some(SpanMeta::List(list_desc)) => Some(list_desc),
                _ => None,
            })
            .collect();

        let nested_item_start = nested_item_des.byte_pos.start;
        changes.push(match parents[0] {
            ListDesc {
                starting_index: Some(_),
                ..
            } =>
            // numbered lists do not need modifications
            {
                TextChange::Replace(
                    ByteSpan::new(nested_item_start, nested_item_start),
                    "\t".to_string(),
                )
            }

            //unordered need "-" -> "*" replacement
            _ => TextChange::Replace(
                ByteSpan::new(nested_item_start, nested_item_start + 1),
                format!("\t{}", select_unordered_list_marker(parents.len())),
            ),
        });
    }
    changes
}

fn on_space_after_task_markers(
    structure: &TextStructure,
    text: &str,
    cursor: ByteSpan,
) -> Option<Vec<TextChange>> {
    let unexpanded_task_markers = &text[..cursor.start].ends_with("[]");

    if !unexpanded_task_markers {
        return None;
    }

    if structure
        .find_span_at(SpanKind::ListItem, cursor.clone())
        .is_some_and(|(span, _)| {
            // At the beggining of a list item with unexpanded task markers "- []{}"
            cursor.start == span.start + 4
        })
    {
        // Ie. "- []{}" -> "- [ ]{}"
        return Some(vec![TextChange::Replace(
            ByteSpan::new(cursor.start - 2, cursor.start),
            "[ ]".to_string(),
        )]);
    }

    if structure
        .find_span_at(SpanKind::Paragraph, cursor.clone())
        .is_some_and(|(_, _)| {
            // Start of the file or a new line, so we must add a new list item
            let text_before_task_markers = &text[..cursor.start - 2];
            text_before_task_markers.len() == 0 || text_before_task_markers.ends_with("\n")
        })
    {
        // Ie. "[]{}" -> "- []{}"
        return Some(vec![TextChange::Replace(
            ByteSpan::new(cursor.start - 2, cursor.start),
            select_unordered_list_marker(0).to_string() + " [ ]",
        )]);
    }

    None
}

// ----  text change handler ----
#[derive(Debug)]
pub enum TextChangeError {
    OverlappingChanges,
}

pub fn apply_text_changes(
    text: &mut String,
    prev_cursor: UnOrderedByteSpan,
    changes: impl IntoIterator<Item = TextChange>,
) -> Result<UnOrderedByteSpan, TextChangeError> {
    #[derive(Debug, Clone)]

    struct Log {
        removed: ByteSpan,
        inserted_len: usize,
    }
    type Logs = SmallVec<[Log; 4]>;

    fn append(
        range: ByteSpan,
        to_insert: usize,
        logs: &[Log],
    ) -> Result<(Logs, ByteSpan), TextChangeError> {
        let mut res: Logs = logs.iter().map(Log::clone).collect();
        res.sort_by(|a, b| a.removed.end.cmp(&b.removed.end));

        let mut actual_range = range;

        let mut split_point: Option<usize> = None;

        // find a splitting point in the insertion logs
        for (i, log) in logs.iter().enumerate() {
            let log_entry_range = log.removed;
            match log_entry_range.relative_to(actual_range) {
                // check for overlaps
                RangeRelation::StartInside
                | RangeRelation::EndInside
                | RangeRelation::Inside
                | RangeRelation::Contains => {
                    // it means that we have overlapping ranges for removal
                    // that is not allowed
                    return Err(TextChangeError::OverlappingChanges);
                }

                RangeRelation::Before => {
                    // that means that the removal happened earlier
                    // thus, we need to adjust starting position
                    let delta = log.inserted_len as isize - log.removed.range().len() as isize;
                    actual_range = ByteSpan::new(
                        (actual_range.start as isize + delta) as usize,
                        (actual_range.end as isize + delta) as usize,
                    );
                }

                RangeRelation::After => {
                    split_point = Some(i);
                }
            }
        }

        // if we need to insert somewhere in the middle we need to shift spans that come after
        if let Some(split_point) = split_point {
            // we need to move what comes after the split
            let delta: isize = to_insert as isize - actual_range.range().len() as isize;
            for log in res[split_point..].iter_mut() {
                log.removed = ByteSpan::new(
                    (log.removed.start as isize + delta) as usize,
                    (log.removed.end as isize + delta) as usize,
                );
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

    let mut inserted_cursor: Option<ByteSpan> = None;

    for change in changes.into_iter() {
        match change {
            TextChange::Replace(range, with) => {
                let (with, extracted_cursor) = TextChange::try_extract_cursor(with);
                let to_insert = with.len();
                let (new_logs, target) = append(range, to_insert, &logs)?;
                logs = new_logs;
                if let Some(extracted_cursor) = extracted_cursor {
                    inserted_cursor = Some(ByteSpan::new(
                        target.start + extracted_cursor.start,
                        target.start + extracted_cursor.end,
                    ));
                }
                actual_changes.push(TextChange::Replace(target, with));
            }
        }
    }

    let adjusted_cursor = match inserted_cursor {
        Some(cursor) => UnOrderedByteSpan::new(cursor.start, cursor.end),
        None => {
            // let mut cursor_start = prev_cursor.start;
            // let mut cursor_end = prev_cursor.end;
            let ordered = ByteSpan::new(prev_cursor.start, prev_cursor.end);
            let (cursor_start, cursor_end) = actual_changes.iter().fold(
                (ordered.start, ordered.end),
                |(cursor_start, cursor_end), change| match change {
                    TextChange::Replace(change_range, with) => {
                        let byte_delta: isize =
                            with.len() as isize - change_range.range().len() as isize;

                        match ByteSpan::new(cursor_start, cursor_end).relative_to(*change_range) {
                            RangeRelation::Before => {
                                // nothing to do here
                                (cursor_start, cursor_end)
                            }
                            RangeRelation::After => {
                                // cursor is ahead of range => move the cursor by change delta
                                (
                                    (cursor_start as isize + byte_delta) as usize,
                                    (cursor_end as isize + byte_delta) as usize,
                                )
                            }
                            RangeRelation::StartInside => {
                                // means that the left side of selection is inside the replacement
                                // example
                                // `ab{|}cd{|}e`
                                //   ^___^ => replace with "oops"
                                // `a{|}oopsd{|}e`
                                // => selecte the entire replacement, and continue to the prev end
                                (
                                    change_range.start,
                                    (prev_cursor.end as isize + byte_delta) as usize,
                                )
                            }
                            RangeRelation::EndInside => {
                                // means that the right side of selection is inside the replacement
                                // example
                                // `ab{|}cd{|}efj`
                                //        ^____^ => replace with "oops"
                                // `ab{|}coops{|}j`
                                // => selecte the entire replacement, and continue to the prev start
                                (cursor_start, change_range.start + with.len())
                            }
                            RangeRelation::Inside => {
                                // means that the cursor is inside the replacement range
                                // example
                                // `ab{||}cd`
                                //   ^____^ => replace with "oops"
                                // `a{|}oops{|}d`
                                // => selecte the entire replacement
                                (change_range.start, change_range.start + with.len())
                            }
                            RangeRelation::Contains => {
                                // means that the selection is larger than replacement
                                // example
                                // `a{|}bcde{|}f`
                                //       ^ ^ => replace with "oops"
                                // `a{|}boops{|}f`
                                // => selecte the entire replacement
                                (cursor_start, (cursor_end as isize + byte_delta) as usize)
                            }
                        }
                    }
                },
            );

            // flip the direction if it was flipped before
            // note that we get ordered results due to algorithm using ByteSpan which assumed order
            let (cursor_start, cursor_end) = if prev_cursor.start > prev_cursor.end {
                (cursor_end, cursor_start)
            } else {
                (cursor_start, cursor_end)
            };

            UnOrderedByteSpan::new(cursor_start, cursor_end)
        }
    };

    // finally apply all the precomputed changes
    for change in actual_changes.into_iter() {
        match change {
            TextChange::Replace(byte_span, with) => {
                text.replace_range(byte_span.range(), &with);
            }
        }
    }

    Ok(adjusted_cursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    // --- handling tabs inside lists ---

    #[test]
    pub fn test_tabs_cases() {
        let test_cases = [
            (
                "-- tabs in ordered lists modify numbers --",
                "1. a\n2. b{||}\n\t- c\n\t\t 1. d\n4. d",
                Some("1. a\n\t1. b{||}\n\t\t* c\n\t\t\t 1. d\n2. d"),
            ),
            (
                "-- tabbing inside nested unordered list --",
                "- a\n\t* b\n\t* c{||}\n- d \n",
                Some("- a\n\t* b\n\t\t* c{||}\n- d \n"),
            ),
            (
                "-- tabbing inside unordered list picks proper list item marker --",
                "- a\n- b{||}\n\t- c\n\t\t 1. d",
                Some("- a\n\t* b{||}\n\t\t* c\n\t\t\t 1. d"),
            ),
            (
                "-- tabbing inside list item not on the same line when it starts => goes to default beh --",
                "- a\n\ta{||}",
                None,
            ),
            //             (
            //                 "-- identing numbered lists honors nested indicies --",
            //                 r#"
            // 1. a
            // \t1. b
            // \t2. boo
            // 2. c{||}
            // 3. d"#,
            //                 r#"
            // 1. a
            // \t1. b
            // \t2. boo
            // \t3. c{||}
            // 2. d"#,
            //             ),
        ];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let changes = on_tab_inside_list(&TextStructure::new(&text), &text, cursor.clone());

            match (changes, expected_output) {
                (None, None) => (),
                (Some(changes), Some(expected_output)) => {
                    let cursor =
                        apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
                    assert_eq!(
                        TextChange::encode_cursor(&text, cursor),
                        expected_output,
                        "test case: {}",
                        desc
                    );
                }
                (changes, expected_output) => {
                    assert!(
                        false,
                        "unexpected matching, text case:{desc} \nchanges = {changes:#?}\nexpected = {expected_output:#?}",
                        // changes, expected_output
                    );
                }
            }
        }
    }

    #[test]
    pub fn test_shift_tabs_cases() {
        let test_cases = [
            (
                "-- shift left unordered list item --",
                "- a\n\t* b{||}",
                Some("- a\n- b{||}"),
            ),
            (
                "-- shift tab bails out if the list item is not on the same line as cursor --",
                "- a\n\t* b\n\t\t*{||}",
                None,
            ),
        ];

        for (desc, input, expected_output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let changes =
                on_shift_tab_inside_list(&TextStructure::new(&text), &text, cursor.clone());

            match (changes, expected_output) {
                (None, None) => (),
                (Some(changes), Some(expected_output)) => {
                    let cursor =
                        apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();

                    assert_eq!(
                        TextChange::encode_cursor(&text, cursor),
                        expected_output,
                        "test case: {}",
                        desc
                    );
                }
                (changes, expected_output) => {
                    assert!(
                        false,
                        "unexpected matching, text case:{desc} \nchanges = {changes:#?}\nexpected = {expected_output:#?}",
                        // changes, expected_output
                    );
                }
            }
        }
    }

    // --- spitting lists via enter ---

    #[test]
    pub fn test_splitting_list_item_via_enter() {
        let test_cases = [
            (
                "## Primitive case of list item splitting ##",
                "- a{||}b",
                "- a\n- {||}b",
            ),
            (
                "## List item splitting with selection ##",
                "- {|}a{|}b",
                "- \n- {||}b",
            ),
            (
                "## Enter on empty list item removes it with newline ##",
                "- {||}\n- a",
                "{||}\n- a",
            ),
            (
                "## Enter on empty list item removes it ##",
                "- a\n\t* {||}\n\t* b",
                // "- a\n\t* {||}\n\t* b",
                // "- a\n\t{||}\n\t* b"
                "- a\n{||}\n\t* b",
            ),
            (
                "## Removing empty item in a numbered list adjusts indicies ##",
                "1. a\n\t1. {||}\n\t2. c",
                "1. a\n{||}\n\t1. c",
            ),
            (
                "## Splitting a numbered list with selection ##",
                "- parent\n\t1. {|}a{|}b\n\t2. c",
                "- parent\n\t1. \n\t2. {||}b\n\t3. c",
            ),
            (
                "## Splitting an unordered nested list##",
                "- a\n\t* b{||}",
                "- a\n\t* b\n\t* {||}",
            ),
            // todo items
            (
                "## adding a todo item in case the origin has a todo marker##",
                "- [ ] item{||}",
                "- [ ] item\n- [ ] {||}",
            ),
            (
                "## removing empty todo item on enter ##",
                "- [ ] {||}",
                "{||}",
            ),
        ];

        for (desc, input, output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = on_enter_inside_list_item(&structure, &text, cursor.clone()).unwrap();

            let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
            assert_eq!(
                TextChange::encode_cursor(&text, cursor),
                output,
                "test case: {}",
                desc
            );
        }
    }

    #[test]
    pub fn test_adding_list_item_with_enter() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- item{||}".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- item");

        let structure = TextStructure::new(&text);

        let changes = on_enter_inside_list_item(&structure, &text, cursor).unwrap();

        let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "- item\n- {||}");
    }

    #[test]
    pub fn test_adding_list_item_with_enter_on_complex_list_item() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- *item*{||}".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- *item*");

        let structure = TextStructure::new(&text);

        let changes = on_enter_inside_list_item(&structure, &text, cursor.clone()).unwrap();

        let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "- *item*\n- {||}");
    }

    #[test]
    pub fn test_not_adding_list_item_on_empty_line() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- item\n{||}".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- item\n");

        let structure = TextStructure::new(&text);

        let changes = on_enter_inside_list_item(&structure, &text, cursor.clone());
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_skips_handling_enter_if_cursor_on_markup() {
        let (text, cursor) = TextChange::try_extract_cursor("{||}- a".to_string());
        let changes =
            on_enter_inside_list_item(&TextStructure::new(&text), &text, cursor.unwrap().clone());
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_skips_expanding_task_markers_when_not_start_of_line() {
        let (text, cursor) = TextChange::try_extract_cursor("a[]{||}".to_string());
        let changes =
            on_space_after_task_markers(&TextStructure::new(&text), &text, cursor.unwrap().clone());
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_skips_expanding_task_markers_when_in_code_block() {
        let (text, cursor) = TextChange::try_extract_cursor("```\n[]{||}```".to_string());
        let changes =
            on_space_after_task_markers(&TextStructure::new(&text), &text, cursor.unwrap().clone());
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_space_expands_task_markerss() {
        // Note: the [Space] press in reality adds a space after the cursor
        let test_cases = [
            (
                "## Expanding a task marker on the first empty line ##",
                "[]{||}",
                "- [ ]{||}",
            ),
            (
                "## Expanding a task marker on any empty line ##",
                "a\n[]{||}",
                "a\n- [ ]{||}",
            ),
            (
                "## Expanding a task marker on the start of a list item ##",
                "- []{||}",
                "- [ ]{||}",
            ),
            (
                "## Expanding a task marker on a nested list item ##",
                "- a\n\t* []{||}",
                "- a\n\t* [ ]{||}",
            ),
            (
                "## Expanding a task marker from the start of a non-empty line##",
                "[]{||}abc",
                "- [ ]{||}abc",
            ),
        ];

        for (desc, input, output) in test_cases {
            let (mut text, cursor) = TextChange::try_extract_cursor(input.to_string());
            let cursor = cursor.unwrap();

            let structure = TextStructure::new(&text);

            let changes = on_space_after_task_markers(&structure, &text, cursor.clone()).unwrap();

            let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
            assert_eq!(
                TextChange::encode_cursor(&text, cursor),
                output,
                "test case: {}",
                desc
            );
        }
    }

    // --------- Text changes cursor tests --------

    #[test]
    pub fn test_cursor_extraction_from_string() {
        let (text, cursor) = TextChange::try_extract_cursor("- a{||}b".to_string());
        assert_eq!(text, "- ab");
        assert_eq!(cursor, Some(ByteSpan::new(3, 3)));

        let (text, cursor) = TextChange::try_extract_cursor("- {|}a{|}b".to_string());
        assert_eq!(text, "- ab");
        assert_eq!(cursor, Some(ByteSpan::new(2, 3)));

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
            TextChange::Replace(ByteSpan::new(a_pos, a_pos + 1), "hello".into()),
            TextChange::Replace(ByteSpan::new(b_pos, b_pos + 1), "world".into()),
            TextChange::Replace(ByteSpan::new(b_pos + 1, b_pos + 1), "!".into()),
        ];

        apply_text_changes(&mut text, UnOrderedByteSpan::new(0, 0), changes).unwrap();
        assert_eq!(text, "hello world!");
    }

    #[test]
    pub fn test_several_text_changes_out_of_order() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            TextChange::Replace(ByteSpan::new(b_pos + 1, b_pos + 1), "!".into()),
            TextChange::Replace(ByteSpan::new(b_pos, b_pos + 1), "world".into()),
            TextChange::Replace(ByteSpan::new(a_pos, a_pos + 1), "hello".into()),
        ];

        apply_text_changes(&mut text, UnOrderedByteSpan::new(0, 0), changes).unwrap();
        assert_eq!(text, "hello world!");
    }

    #[test]
    pub fn test_overlapping_text_changes_are_not_allowed() {
        let mut text = "a b".to_string();

        let a_pos = text.find("a").unwrap();
        let b_pos = text.find("b").unwrap();

        let changes = [
            // captures "a b"
            TextChange::Replace(ByteSpan::new(a_pos, b_pos + 1), "hello".into()),
            // captures "b"
            TextChange::Replace(ByteSpan::new(b_pos, b_pos + 1), "world".into()),
        ];

        let cursor = apply_text_changes(&mut text, UnOrderedByteSpan::new(0, 0), changes);
        assert!(matches!(cursor, Err(TextChangeError::OverlappingChanges)));
        assert_eq!(text, "a b");
    }

    // --- automatic cursor adjacements based on text changes ---

    #[test]
    pub fn test_cursor_adjacement_cursor_inside_replacement() {
        // `ab{||}cd`
        //   ^____^ => replace with "oops"
        // `a{|}oops{|}d`
        let (mut text, cursor) = TextChange::try_extract_cursor("ab{||}cd".to_string());

        let start = text.find("b").unwrap();
        let end = text.find("d").unwrap();

        let changes = [
            TextChange::Replace(ByteSpan::new(start, end), "oops".into()),
            // delete "a", to test out cursor adjecement that are out of range
            TextChange::Replace(ByteSpan::new(0, 1), "".into()),
        ];

        let cursor = apply_text_changes(&mut text, cursor.unwrap().unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "{|}oops{|}d");
    }

    #[test]
    pub fn test_cursor_adjacement_selection_contains_replacement() {
        // means that the selection is larger than replacement
        // example
        // `a{|}bcde{|}f`
        //       ^ ^ => replace with "oops"
        // `a{|}boops{|}f`
        // => selecte the entire replacement
        let (mut text, cursor) = TextChange::try_extract_cursor("a{|}bcde{|}f".to_string());

        let changes = [
            TextChange::Replace(
                ByteSpan::new(text.find("c").unwrap(), text.find("f").unwrap()),
                "oops".into(),
            ),
            // delete "a", to test out cursor adjecement that are out of range
            TextChange::Replace(ByteSpan::new(0, 1), "".into()),
        ];

        let cursor = apply_text_changes(&mut text, cursor.unwrap().unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "{|}boops{|}f");
    }

    #[test]
    pub fn test_cursor_adjacement_selection_start_inside_replacement() {
        // means that the left side of selection is inside the replacement
        // example
        // `ab{|}cd{|}e`
        //   ^___^ => replace with "oops"
        // `a{|}oopsd{|}e`
        // => selecte the entire replacement, and continue to the prev end
        let (mut text, cursor) = TextChange::try_extract_cursor("ab{|}cd{|}e".to_string());

        let changes = [
            TextChange::Replace(
                ByteSpan::new(text.find("b").unwrap(), text.find("d").unwrap()),
                "oops".into(),
            ),
            TextChange::Replace(ByteSpan::new(text.len(), text.len()), "!".into()),
        ];

        let cursor = apply_text_changes(&mut text, cursor.unwrap().unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "a{|}oopsd{|}e!");
    }

    #[test]
    pub fn test_cursor_adjacement_selection_end_inside_replacement() {
        // means that the right side of selection is inside the replacement
        // example
        // `ab{|}cd{|}efj`
        //        ^____^ => replace with "oops"
        // `ab{|}coops{|}j`
        // => selecte the entire replacement, and continue to the prev start
        let (mut text, cursor) = TextChange::try_extract_cursor("ab{|}cd{|}efj".to_string());

        let changes = [
            TextChange::Replace(
                ByteSpan::new(text.find("d").unwrap(), text.find("j").unwrap()),
                "oops".into(),
            ),
            TextChange::Replace(ByteSpan::new(0, 1), "!!".into()),
        ];

        let cursor = apply_text_changes(&mut text, cursor.unwrap().unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "!!b{|}coops{|}j");
    }
}

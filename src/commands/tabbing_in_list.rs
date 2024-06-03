use smallvec::SmallVec;

use crate::{
    byte_span::ByteSpan,
    command::TextCommandContext,
    effects::text_change_effect::TextChange,
    text_structure::{ListDesc, SpanKind, SpanMeta, TextStructure},
};

use super::select_unordered_list_marker;

pub fn on_shift_tab_inside_list(context: TextCommandContext) -> Option<Vec<TextChange>> {
    let TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: cursor,
    } = context;

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

pub fn on_tab_inside_list(context: TextCommandContext) -> Option<Vec<TextChange>> {
    let TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: cursor,
    } = context;

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

#[cfg(test)]
mod tests {
    use crate::effects::text_change_effect::apply_text_changes;

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

            let changes = on_tab_inside_list(TextCommandContext::new(
                &TextStructure::new(&text),
                &text,
                cursor.clone(),
            ));

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

            let changes = on_shift_tab_inside_list(TextCommandContext::new(
                &TextStructure::new(&text),
                &text,
                cursor.clone(),
            ));

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
}

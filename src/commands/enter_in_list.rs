use smallvec::SmallVec;

use crate::{
    byte_span::ByteSpan,
    command::TextCommandContext,
    effects::text_change_effect::TextChange,
    text_structure::{ListDesc, SpanKind, SpanMeta},
};

use super::select_unordered_list_marker;

// handler on ENTER
pub fn on_enter_inside_list_item(context: TextCommandContext) -> Option<Vec<TextChange>> {
    let TextCommandContext {
        text_structure: structure,
        text,
        byte_cursor: cursor,
    } = context;

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

#[cfg(test)]
mod tests {
    use crate::{effects::text_change_effect::apply_text_changes, text_structure::TextStructure};

    use super::*;

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

            let changes = on_enter_inside_list_item(TextCommandContext::new(
                &structure,
                &text,
                cursor.clone(),
            ))
            .unwrap();

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

        let changes =
            on_enter_inside_list_item(TextCommandContext::new(&structure, &text, cursor)).unwrap();

        let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "- item\n- {||}");
    }

    #[test]
    pub fn test_adding_list_item_with_enter_on_complex_list_item() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- *item*{||}".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- *item*");

        let structure = TextStructure::new(&text);

        let changes =
            on_enter_inside_list_item(TextCommandContext::new(&structure, &text, cursor.clone()))
                .unwrap();

        let cursor = apply_text_changes(&mut text, cursor.unordered(), changes).unwrap();
        assert_eq!(TextChange::encode_cursor(&text, cursor), "- *item*\n- {||}");
    }

    #[test]
    pub fn test_not_adding_list_item_on_empty_line() {
        let (mut text, cursor) = TextChange::try_extract_cursor("- item\n{||}".to_string());
        let cursor = cursor.unwrap();

        assert_eq!(text, "- item\n");

        let structure = TextStructure::new(&text);

        let changes =
            on_enter_inside_list_item(TextCommandContext::new(&structure, &text, cursor.clone()));
        assert!(changes.is_none());
    }

    #[test]
    pub fn test_skips_handling_enter_if_cursor_on_markup() {
        let (text, cursor) = TextChange::try_extract_cursor("{||}- a".to_string());
        let changes = on_enter_inside_list_item(TextCommandContext::new(
            &TextStructure::new(&text),
            &text,
            cursor.unwrap().clone(),
        ));
        assert!(changes.is_none());
    }
}

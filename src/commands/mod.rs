use std::iter;

use itertools::Itertools;
use smallvec::SmallVec;

use crate::text_structure::{
    ListItemDesc, ListItemMarker, SpanIndex, SpanKind, SpanMeta, TextStructure,
};

pub mod enter_in_list;
pub mod run_llm;
pub mod space_after_task_markers;
pub mod tabbing_in_list;
pub mod toggle_code_block;
pub mod toggle_md_headings;
pub mod toggle_simple_md_annotations;

pub fn default_unordered_list_marker(depth: usize) -> &'static str {
    match depth {
        0 => "-",
        _ => "*",
    }
}

pub fn select_unordered_list_marker<'a>(
    structure: &'a TextStructure,
    item_index: SpanIndex,
    depth_delta: isize,
) -> &'a str {
    let current_item = structure
        .find_meta(item_index)
        .map(|meta| (item_index, meta));

    let parents: SmallVec<[_; 4]> = structure
        .iterate_parents_of(item_index)
        .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
        .map(|(idx, _)| structure.find_meta(idx).map(|meta| (idx, meta)))
        .collect();

    let siblings = structure.iterate_immediate_siblings_before(item_index);

    let sibling_child_list_item = siblings
        .map(|(idx, _)| {
            structure
                .iterate_children_recursively_of(idx)
                .filter(|(_, desc)| desc.kind == SpanKind::ListItem)
                .map(|(idx, _)| structure.find_meta(idx).map(|meta| (idx, meta)))
                .nth(0)
        })
        .flatten()
        .flatten()
        .last();

    let parent_depth = parents.len();

    let mut list_chain = iter::once(current_item).chain(parents);

    let desired_depth = parent_depth.saturating_add_signed(depth_delta);

    let marker_at_depth = match depth_delta {
        depth_delta if depth_delta <= 0 => {
            list_chain
                .nth(depth_delta.abs() as usize)
                .map(|list_item| match list_item {
                    Some((
                        _,
                        SpanMeta::ListItem(ListItemDesc {
                            list_item_marker: ListItemMarker::Unordered(marker),
                        }),
                    )) => Some(marker.as_str()),
                    _ => None,
                })
        }
        depth_delta if depth_delta == 1 => {
            sibling_child_list_item.map(|list_item| match list_item {
                (
                    _,
                    SpanMeta::ListItem(ListItemDesc {
                        list_item_marker: ListItemMarker::Unordered(marker),
                    }),
                ) => Some(marker.as_str()),
                _ => None,
            })
        }
        _ => None,
    }
    .flatten();

    marker_at_depth.unwrap_or(default_unordered_list_marker(desired_depth))
}

pub mod enter_in_list;
pub mod space_after_task_markers;
pub mod tabbing_in_list;

pub fn select_unordered_list_marker(depth: usize) -> &'static str {
    match depth {
        0 => "-",
        _ => "*",
    }
}

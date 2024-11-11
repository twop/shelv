pub mod enter_in_list;
pub mod inline_llm_prompt;
pub mod insert_text;
pub mod kdl_lang;
pub mod run_llm;
pub mod slash_pallete;
pub mod space_after_task_markers;
pub mod tabbing_in_list;
pub mod toggle_code_block;
pub mod toggle_md_headings;
pub mod toggle_simple_md_annotations;

pub fn select_unordered_list_marker(depth: usize) -> &'static str {
    match depth {
        0 => "-",
        _ => "*",
    }
}

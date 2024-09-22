use eframe::egui::Id;

use crate::{
    app_actions::AppAction,
    app_state::TextSelectionAddress,
    command::{try_extract_text_command_context, CommandContext, EditorCommandOutput},
};

pub fn inline_llm_prompt_command_handler(
    CommandContext { app_state }: CommandContext,
) -> Option<EditorCommandOutput> {
    let text_command_ctx = try_extract_text_command_context(app_state)?;

    Some(
        [AppAction::TriggerInlinePromptUI(TextSelectionAddress {
            span: text_command_ctx.byte_cursor,
            note_file: app_state.selected_note,
            text_version: text_command_ctx.text_structure.opaque_version(),
        })]
        .into(),
    )
}

pub fn compute_inline_prompt_text_input_id(inline_prompt_address: TextSelectionAddress) -> Id {
    Id::new(inline_prompt_address)
}

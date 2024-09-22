use crate::{
    app_actions::AppAction,
    command::{try_extract_text_command_context, CommandContext, EditorCommandOutput},
};

pub fn inline_llm_prompt_command_handler(
    CommandContext { app_state }: CommandContext,
) -> Option<EditorCommandOutput> {
    let text_command_ctx = try_extract_text_command_context(app_state)?;

    Some(
        [AppAction::TriggerInlinePrompt(
            text_command_ctx.byte_cursor,
            app_state.selected_note,
            text_command_ctx.text_structure.opaque_version(),
        )]
        .into(),
    )
}

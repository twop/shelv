use crate::{
    app_actions::{AppAction, WordJumpAction},
    app_state::NoteSignature,
    command::{try_extract_text_command_context, CommandContext, EditorCommandOutput},
};

pub fn start_jump_list_command_handler(
    CommandContext { app_state, .. }: CommandContext,
) -> Option<EditorCommandOutput> {
    let text_command_ctx = try_extract_text_command_context(app_state)?;

    Some(
        [AppAction::WordJump(WordJumpAction::SwitchToJumpingMode(
            text_command_ctx.byte_cursor,
            NoteSignature::new(
                app_state.notes.selected_note,
                text_command_ctx.text_structure.opaque_version(),
            ),
        ))]
        .into(),
    )
}

// pub fn compute_inline_prompt_text_input_id(inline_prompt_address: TextSelectionAddress) -> Id {
//     Id::new(inline_prompt_address)
// }

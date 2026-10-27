use smallvec::SmallVec;

use crate::{
    app_actions::AppAction,
    command::{CommandContext, TextCommandContext, try_extract_text_command_context},
    effects::text_change_effect::TextChange,
};

/// `action.InsertText({ text })`: replaces the selection (or inserts at the cursor). `{||}` and
/// `{|}..{|}` in the text place the cursor or select, when the text change is applied.
pub fn call_replace_text(
    text: &str,
    CommandContext { app_state, .. }: CommandContext,
) -> SmallVec<[AppAction; 1]> {
    let Some(TextCommandContext { byte_cursor, .. }) = try_extract_text_command_context(app_state)
    else {
        return SmallVec::new();
    };

    SmallVec::from([AppAction::apply_text_changes(
        app_state.selected_note,
        [TextChange::Insert(byte_cursor, text.to_string())].into(),
    )])
}

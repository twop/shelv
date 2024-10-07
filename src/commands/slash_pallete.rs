use eframe::egui::Id;
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, SlashPaletteAction},
    app_state::{AppState, SlashPalette, SlashPaletteCmd, TextSelectionAddress},
    command::{
        try_extract_text_command_context, CommandContext, EditorCommandOutput, TextCommandContext,
    },
    effects::text_change_effect::TextChange,
};

pub fn show_slash_pallete(
    CommandContext { app_state, .. }: CommandContext,
) -> Option<EditorCommandOutput> {
    let TextCommandContext { byte_cursor, .. } = try_extract_text_command_context(app_state)?;

    Some(SmallVec::from_iter(
        [
            // TODO it seems that egui will insert "/" regardless
            // AppAction::ApplyTextChanges {
            //     target: app_state.selected_note,
            //     changes: [TextChange::Insert(byte_cursor, "/".to_string())].into(),
            //     should_trigger_eval: false,
            // },
            AppAction::SlashPalette(SlashPaletteAction::Show(SlashPalette {
                note_file: app_state.selected_note,
                // TODO verify + 1 thing, seems sketchy
                // it relies that this will be done before rendering
                slash_byte_pos: byte_cursor.start + 1,
                search_term: "".to_string(),
                options: Vec::from([
                    SlashPaletteCmd {
                        font_awesome_icon: Some("\u{ed0d}".to_string()),
                        prefix: "js".to_string(),
                        description: "inserts markdown javascript code block".to_string(),
                    },
                    SlashPaletteCmd {
                        font_awesome_icon: Some("\u{ec10}".to_string()),
                        prefix: "ai".to_string(),
                        description: "inserts ai code block".to_string(),
                    },
                    SlashPaletteCmd {
                        font_awesome_icon: Some("\u{f133}".to_string()),
                        prefix: "date".to_string(),
                        description: "inserts current date".to_string(),
                    },
                ]),
                selected: 0,
            })),
        ]
        .into_iter()
        .chain(
            app_state
                .inline_llm_prompt
                .is_some()
                .then(|| AppAction::AcceptPromptSuggestion { accept: false }),
        ),
    ))
}

pub fn next_slash_cmd(
    CommandContext { app_state, .. }: CommandContext,
) -> Option<EditorCommandOutput> {
    app_state
        .slash_palette
        .is_some()
        .then(|| SmallVec::from_iter([AppAction::SlashPalette(SlashPaletteAction::NextCommand)]))
}

pub fn prev_slash_cmd(
    CommandContext { app_state, .. }: CommandContext,
) -> Option<EditorCommandOutput> {
    app_state
        .slash_palette
        .is_some()
        .then(|| SmallVec::from_iter([AppAction::SlashPalette(SlashPaletteAction::PrevCommand)]))
}

pub fn execute_slash_cmd(
    CommandContext { app_state, .. }: CommandContext,
) -> Option<EditorCommandOutput> {
    app_state.slash_palette.as_ref().map(|palette| {
        SmallVec::from_iter([AppAction::SlashPalette(SlashPaletteAction::ExecuteCommand(
            palette.selected,
        ))])
    })
}

pub fn hide_slash_pallete(
    CommandContext { app_state, .. }: CommandContext,
) -> Option<EditorCommandOutput> {
    app_state
        .slash_palette
        .is_some()
        .then(|| SmallVec::from_iter([AppAction::SlashPalette(SlashPaletteAction::Hide)]))
}

use eframe::egui::Id;
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, SlashPaletteAction},
    app_state::{AppState, SlashPalette, SlashPaletteCmd, TextSelectionAddress},
    command::{
        try_extract_text_command_context, AppFocus, AppFocusState, CommandContext,
        EditorCommandOutput, TextCommandContext,
    },
    effects::text_change_effect::TextChange,
    scripting::JS_SOURCE_LANG,
    text_structure::{SpanKind, SpanMeta},
};

pub fn show_slash_pallete(
    CommandContext {
        app_state,
        app_focus,
        ..
    }: CommandContext,
) -> Option<EditorCommandOutput> {
    let is_focused_on_editor = matches!(
        app_focus,
        AppFocusState {
            is_menu_opened: false,
            focus: Some(AppFocus::NoteEditor),
        }
    );

    if !is_focused_on_editor {
        return None;
    }

    let TextCommandContext {
        byte_cursor,
        text_structure,
        ..
    } = try_extract_text_command_context(app_state)?;

    if let Some((_, _, meta)) =
        text_structure.find_surrounding_span_with_meta(SpanKind::CodeBlock, byte_cursor)
    {
        if meta
            == (SpanMeta::CodeBlock {
                lang: JS_SOURCE_LANG.to_string(),
            })
        {
            // do not allow "/" palette in JS blocks, let's experiment of having it in other blocks for now
            return None;
        }
    }

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
                slash_byte_pos: byte_cursor.start,
                search_term: "".to_string(),
                options: app_state.slash_palette_commands.clone(),
                selected: 0,
                update_count: 0,
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

use std::{fmt::Debug, rc::Rc};

use eframe::egui::KeyboardShortcut;
use pulldown_cmark::CowStr;
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, FocusTarget},
    app_state::AppState,
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    persistent_state::NoteFile,
    settings_eval::Scripts,
    text_structure::TextStructure,
};

#[derive(Debug, Clone, Copy)]
pub struct TextCommandContext<'a> {
    pub text_structure: &'a TextStructure,
    pub text: &'a str,
    pub byte_cursor: ByteSpan,
}

#[derive(Clone, Copy, Debug)]
pub enum AppFocus {
    NoteEditor,
    InlinePropmptEditor,
}

#[derive(Clone, Copy, Debug)]
pub struct AppFocusState {
    pub is_menu_opened: bool,
    pub focus: Option<AppFocus>,
}

// #[derive(Clone, Copy)]
pub struct CommandContext<'a> {
    pub app_state: &'a AppState,
    pub app_focus: AppFocusState,
    pub scripts: &'a mut Scripts,
}

impl<'a> TextCommandContext<'a> {
    pub fn new(text_structure: &'a TextStructure, text: &'a str, byte_cursor: ByteSpan) -> Self {
        Self {
            text_structure,
            text,
            byte_cursor,
        }
    }
}

pub type EditorCommandOutput = SmallVec<[AppAction; 1]>;

#[derive(Clone)]
pub struct EditorCommand {
    pub kind: Option<BuiltInCommand>,
    pub shortcut: Option<KeyboardShortcut>,
    pub try_handle: Rc<dyn Fn(CommandContext) -> EditorCommandOutput>,
}

impl Debug for EditorCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorCommand")
            .field("kind", &self.kind)
            .field("shortcut", &self.shortcut)
            .finish()
    }
}

impl EditorCommand {
    pub fn built_in<Handler: 'static + Fn(CommandContext) -> EditorCommandOutput>(
        kind: BuiltInCommand,
        try_handle: Handler,
    ) -> Self {
        Self {
            kind: Some(kind),
            shortcut: Some(kind.default_keybinding()),
            try_handle: Rc::new(try_handle),
        }
    }

    pub fn user_defined<Handler: 'static + Fn(CommandContext) -> EditorCommandOutput>(
        shortcut: KeyboardShortcut,
        try_handle: Handler,
    ) -> Self {
        Self {
            kind: None,
            shortcut: Some(shortcut),
            try_handle: Rc::new(try_handle),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum BuiltInCommand {
    // Autocomplete/convenience
    ExpandTaskMarker,
    IndentListItem,
    UnindentListItem,
    SplitListItem,

    // Markdown
    MarkdownBold,
    MarkdownItalic,
    MarkdownStrikethrough,
    MarkdownCodeBlock,
    MarkdownH1,
    MarkdownH2,
    MarkdownH3,

    // Others
    SwitchToNote(u8),
    SwitchToSettings,
    PinWindow,
    HideApp,
    HidePrompt,

    // SlashPallete
    ShowSlashPallete,
    NextSlashPalleteCmd,
    PrevSlashPalleteCmd,
    ExecuteSlashPalleteCmd,
    HideSlashPallete,
    // Lang specific
    EnterInsideKDL,
    BracketAutoclosingInsideKDL,

    // Async Code blocks
    RunLLMBlock,
    ShowPrompt,
}

/// Commands that we promote in UI
pub const PROMOTED_COMMANDS: [BuiltInCommand; 9] = const {
    [
        BuiltInCommand::PinWindow,
        BuiltInCommand::MarkdownBold,
        BuiltInCommand::MarkdownItalic,
        BuiltInCommand::MarkdownStrikethrough,
        BuiltInCommand::MarkdownCodeBlock,
        BuiltInCommand::RunLLMBlock,
        BuiltInCommand::MarkdownH1,
        BuiltInCommand::MarkdownH2,
        BuiltInCommand::MarkdownH3,
    ]
};

impl BuiltInCommand {
    pub fn human_description(&self) -> CowStr<'static> {
        match self {
            Self::ExpandTaskMarker => "Expand Task Marker".into(),
            Self::IndentListItem => "Increase List Item identation".into(),
            Self::UnindentListItem => "Decrease List Item identation".into(),
            Self::SplitListItem => "Split List item at cursor position".into(),
            Self::MarkdownBold => "Toggle Bold".into(),
            Self::MarkdownItalic => "Toggle Italic".into(),
            Self::MarkdownStrikethrough => "Toggle Strikethrough".into(),
            Self::MarkdownCodeBlock => "Toggle Code Block".into(),
            Self::MarkdownH1 => "Heading 1".into(),
            Self::MarkdownH2 => "Heading 2".into(),
            Self::MarkdownH3 => "Heading 3".into(),
            Self::SwitchToNote(n) => {
                let note_index = *n;
                match note_index {
                    0 => "Shelf 1".into(),
                    1 => "Shelf 2".into(),
                    2 => "Shelf 3".into(),
                    3 => "Shelf 4".into(),
                    n => format!("Shelf {}", n + 1).into(),
                }
            }
            Self::SwitchToSettings => "Open Settings".into(),
            Self::PinWindow => "Toggle Always on Top".into(),
            Self::HideApp => "Hide Window".into(),
            Self::RunLLMBlock => "Execute AI Block".into(),
            BuiltInCommand::ShowPrompt => "Show AI Prompt".into(),
            // BuiltInCommand::ClosePopupMenu => "Close currently opened popup".into(),
            BuiltInCommand::HidePrompt => "Hide Prompt".into(),
            BuiltInCommand::EnterInsideKDL => "Auto indent KDL".into(),
            BuiltInCommand::BracketAutoclosingInsideKDL => "Auto closing of '{' inside KDL".into(),
            BuiltInCommand::ShowSlashPallete => "Show slash command palette".into(),
            BuiltInCommand::NextSlashPalleteCmd => "Select next command in slash palette".into(),
            BuiltInCommand::PrevSlashPalleteCmd => {
                "Select previous command in slash palette".into()
            }
            BuiltInCommand::ExecuteSlashPalleteCmd => {
                "Execute selected command in slash palette".into()
            }
            BuiltInCommand::HideSlashPallete => "Hide slash command palette".into(),
        }
    }

    pub fn default_keybinding(self) -> eframe::egui::KeyboardShortcut {
        use eframe::egui::{Key, Modifiers};
        use BuiltInCommand as C;
        let shortcut = KeyboardShortcut::new;
        match self {
            C::ExpandTaskMarker => shortcut(Modifiers::NONE, Key::Space),
            C::IndentListItem => shortcut(Modifiers::NONE, Key::Tab),
            C::UnindentListItem => shortcut(Modifiers::SHIFT, Key::Tab),
            C::SplitListItem => shortcut(Modifiers::NONE, Key::Enter),
            C::MarkdownCodeBlock => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::B),
            C::MarkdownBold => shortcut(Modifiers::COMMAND, Key::B),
            C::MarkdownItalic => shortcut(Modifiers::COMMAND, Key::I),
            C::MarkdownStrikethrough => shortcut(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::E),
            C::MarkdownH1 => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::Num1),
            C::MarkdownH2 => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::Num2),
            C::MarkdownH3 => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::Num3),
            C::SwitchToNote(0) => shortcut(Modifiers::COMMAND, Key::Num1),
            C::SwitchToNote(1) => shortcut(Modifiers::COMMAND, Key::Num2),
            C::SwitchToNote(2) => shortcut(Modifiers::COMMAND, Key::Num3),
            C::SwitchToNote(3) => shortcut(Modifiers::COMMAND, Key::Num4),
            // TODO figure out how to make it more bulletproof, option maybe?
            C::SwitchToNote(_) => shortcut(Modifiers::COMMAND, Key::Num0),
            C::SwitchToSettings => shortcut(Modifiers::COMMAND, Key::Comma),
            C::PinWindow => shortcut(Modifiers::COMMAND, Key::P),
            C::RunLLMBlock => shortcut(Modifiers::COMMAND, Key::Enter),
            C::ShowPrompt => shortcut(Modifiers::CTRL, Key::Enter),
            C::EnterInsideKDL => shortcut(Modifiers::NONE, Key::Enter),
            C::BracketAutoclosingInsideKDL => shortcut(Modifiers::SHIFT, Key::OpenBracket),
            C::HideApp | C::HidePrompt => shortcut(Modifiers::NONE, Key::Escape),
            C::ShowSlashPallete => shortcut(Modifiers::NONE, Key::Slash),
            C::NextSlashPalleteCmd => shortcut(Modifiers::NONE, Key::ArrowDown),
            C::PrevSlashPalleteCmd => shortcut(Modifiers::NONE, Key::ArrowUp),
            C::ExecuteSlashPalleteCmd => shortcut(Modifiers::NONE, Key::Enter),
            C::HideSlashPallete => shortcut(Modifiers::NONE, Key::Escape),
        }
    }
}

impl BuiltInCommand {
    pub fn name(&self) -> &'static str {
        use BuiltInCommand::*;
        match self {
            ExpandTaskMarker => "ExpandTaskMarker",
            IndentListItem => "IndentListItem",
            UnindentListItem => "UnindentListItem",
            SplitListItem => "SplitListItem",
            MarkdownBold => "MarkdownBold",
            MarkdownItalic => "MarkdownItalic",
            MarkdownStrikethrough => "MarkdownStrikethrough",
            MarkdownCodeBlock => "MarkdownCodeBlock",
            MarkdownH1 => "MarkdownH1",
            MarkdownH2 => "MarkdownH2",
            MarkdownH3 => "MarkdownH3",
            SwitchToNote(_) => "SwitchToNote",
            SwitchToSettings => "SwitchToSettings",
            PinWindow => "PinWindow",
            HideApp => "HideApp",
            RunLLMBlock => "ExecutePrompt",
            ShowPrompt => "ShowPrompt",
            HidePrompt => "HidePrompt",
            EnterInsideKDL => "EnterInsideKDL",
            BracketAutoclosingInsideKDL => "BracketAutoclosingInsideKDL",
            ShowSlashPallete => "ShowSlashPallete",
            NextSlashPalleteCmd => "NextSlashPalleteCmd",
            PrevSlashPalleteCmd => "PrevSlashPalleteCmd",
            ExecuteSlashPalleteCmd => "ExecuteSlashPalleteCmd",
            HideSlashPallete => "HideSlashPallete",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlashPaletteCmd {
    pub phosphor_icon: Option<String>,
    pub shortcut: Option<KeyboardShortcut>,
    pub prefix: String,
    pub description: String,
    pub editor_cmd: EditorCommand,
}

impl SlashPaletteCmd {
    pub fn from_editor_cmd(prefix: impl Into<String>, editor_cmd: &EditorCommand) -> Self {
        Self {
            phosphor_icon: None,
            prefix: prefix.into(),
            shortcut: editor_cmd.shortcut.clone(),
            description: editor_cmd
                .kind
                .map(|kind| kind.human_description().to_string())
                .unwrap_or_else(|| "".to_string()),
            editor_cmd: editor_cmd.clone(),
        }
    }
    pub fn icon(mut self, icon: String) -> Self {
        self.phosphor_icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

#[derive(PartialEq, Debug)]
pub enum GlobalCommandKind {
    ShowHideApp,
}

pub struct CommandList {
    editor_commands: Vec<EditorCommand>,
    available_slash_palette_commands: Vec<SlashPaletteCmd>,
    custom_slash_commands: Vec<SlashPaletteCmd>,
}

impl CommandList {
    pub fn new(list: Vec<EditorCommand>, slash_palette_commands: Vec<SlashPaletteCmd>) -> Self {
        // TODO: ensure uniqness of names
        // that is going to be even more critical with settings note
        Self {
            editor_commands: list,
            available_slash_palette_commands: slash_palette_commands,
            custom_slash_commands: vec![],
        }
    }

    pub fn available_editor_commands(&self) -> impl Iterator<Item = &EditorCommand> {
        self.editor_commands.iter()
    }

    pub fn available_slash_commands(&self) -> impl Iterator<Item = &SlashPaletteCmd> {
        self.available_slash_palette_commands
            .iter()
            .chain(self.custom_slash_commands.iter())
    }

    pub fn remove_custom_slash_commands(&mut self) {
        self.custom_slash_commands.clear();
    }

    pub fn find(&self, cmd: BuiltInCommand) -> Option<&EditorCommand> {
        self.available_editor_commands()
            .find(|c| c.kind == Some(cmd))
    }

    pub fn editor_retain_only(&mut self, filter: impl Fn(&EditorCommand) -> bool) {
        self.editor_commands.retain(filter)
    }

    pub fn add_editor_cmd(&mut self, cmd: EditorCommand) {
        self.editor_commands.push(cmd)
    }

    pub fn add_custom_slash_command(&mut self, cmd: SlashPaletteCmd) {
        self.custom_slash_commands.push(cmd);
    }

    pub fn set_or_replace_builtin_shortcut(
        &mut self,
        shortcut: KeyboardShortcut,
        cmd: BuiltInCommand,
    ) -> Option<()> {
        let cmd = self
            .editor_commands
            .iter_mut()
            .find(|c| c.kind == Some(cmd))?;
        cmd.shortcut = Some(shortcut);
        Some(())
    }

    pub fn reset_builtins_to_default_keybindings(&mut self) {
        for command in self.editor_commands.iter_mut() {
            if let Some(kind) = command.kind {
                command.shortcut = Some(kind.default_keybinding());
            }
        }
    }
}

pub fn map_text_command_to_command_handler(
    f: impl Fn(TextCommandContext) -> Option<Vec<TextChange>> + 'static,
) -> Box<dyn Fn(CommandContext) -> EditorCommandOutput> {
    Box::new(move |CommandContext { app_state, .. }| {
        let Some(text_command_context) = try_extract_text_command_context(app_state) else {
            return SmallVec::new();
        };

        f(text_command_context)
            .map(|changes| {
                SmallVec::from([AppAction::apply_text_changes(
                    app_state.selected_note,
                    changes,
                )])
            })
            .unwrap_or_default()
    })
}

pub fn try_extract_text_command_context(app_state: &AppState) -> Option<TextCommandContext<'_>> {
    let note = app_state.notes.get(&app_state.selected_note).unwrap();

    let cursor = note.cursor().or(note.last_cursor())?;

    let text_structure = app_state.text_structure.as_ref()?;

    let text_command_context =
        TextCommandContext::new(text_structure, &note.text, cursor.ordered());

    Some(text_command_context)
}

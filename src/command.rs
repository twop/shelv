use eframe::egui::KeyboardShortcut;
use pulldown_cmark::CowStr;
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, FocusTarget},
    app_state::AppState,
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    persistent_state::NoteFile,
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

#[derive(Clone, Copy)]
pub struct CommandContext<'a> {
    pub app_state: &'a AppState,
    pub app_focus: AppFocusState,
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

pub struct EditorCommand {
    pub kind: Option<BuiltInCommand>,
    pub shortcut: Option<KeyboardShortcut>,
    pub try_handle: Box<dyn Fn(CommandContext) -> EditorCommandOutput>,
}

impl EditorCommand {
    pub fn built_in<Handler: 'static + Fn(CommandContext) -> EditorCommandOutput>(
        kind: BuiltInCommand,
        try_handle: Handler,
    ) -> Self {
        Self {
            kind: Some(kind),
            shortcut: Some(kind.default_keybinding()),
            try_handle: Box::new(try_handle),
        }
    }

    pub fn user_defined<Handler: 'static + Fn(CommandContext) -> EditorCommandOutput>(
        shortcut: KeyboardShortcut,
        try_handle: Handler,
    ) -> Self {
        Self {
            kind: None,
            shortcut: Some(shortcut),
            try_handle: Box::new(try_handle),
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
    CloseInlinePrompt,

    // Async Code blocks
    RunLLMBlock,
    TriggerInlinePrompt,
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
            BuiltInCommand::TriggerInlinePrompt => "Inline Prompt".into(),
            // BuiltInCommand::ClosePopupMenu => "Close currently opened popup".into(),
            BuiltInCommand::CloseInlinePrompt => "Close Inline Prompt".into(),
        }
    }

    pub fn default_keybinding(self) -> eframe::egui::KeyboardShortcut {
        use eframe::egui::{Key, Modifiers};
        use BuiltInCommand::*;
        let shortcut = KeyboardShortcut::new;
        match self {
            ExpandTaskMarker => shortcut(Modifiers::NONE, Key::Space),
            IndentListItem => shortcut(Modifiers::NONE, Key::Tab),
            UnindentListItem => shortcut(Modifiers::SHIFT, Key::Tab),
            SplitListItem => shortcut(Modifiers::NONE, Key::Enter),
            MarkdownCodeBlock => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::B),
            MarkdownBold => shortcut(Modifiers::COMMAND, Key::B),
            MarkdownItalic => shortcut(Modifiers::COMMAND, Key::I),
            MarkdownStrikethrough => shortcut(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::E),
            MarkdownH1 => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::Num1),
            MarkdownH2 => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::Num2),
            MarkdownH3 => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::Num3),
            SwitchToNote(0) => shortcut(Modifiers::COMMAND, Key::Num1),
            SwitchToNote(1) => shortcut(Modifiers::COMMAND, Key::Num2),
            SwitchToNote(2) => shortcut(Modifiers::COMMAND, Key::Num3),
            SwitchToNote(3) => shortcut(Modifiers::COMMAND, Key::Num4),
            // TODO figure out how to make it more bulletproof, option maybe?
            SwitchToNote(_) => shortcut(Modifiers::COMMAND, Key::Num0),
            SwitchToSettings => shortcut(Modifiers::COMMAND, Key::Comma),
            PinWindow => shortcut(Modifiers::COMMAND, Key::P),
            RunLLMBlock => shortcut(Modifiers::COMMAND, Key::Enter),
            TriggerInlinePrompt => shortcut(Modifiers::CTRL, Key::Enter),
            // AppAction::RunInlineLLMPrompt
            HideApp | CloseInlinePrompt => shortcut(Modifiers::NONE, Key::Escape),
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum GlobalCommandKind {
    ShowHideApp,
}

pub struct CommandList(Vec<EditorCommand>);

impl CommandList {
    pub fn new(list: Vec<EditorCommand>) -> Self {
        // TODO: ensure uniqness of names
        // that is going to be even more critical with settings note
        Self(list)
    }

    pub fn slice(&self) -> &[EditorCommand] {
        &self.0
    }

    pub fn find(&self, cmd: BuiltInCommand) -> Option<&EditorCommand> {
        self.slice().iter().find(|c| c.kind == Some(cmd))
    }

    pub fn retain_only(&mut self, filter: impl Fn(&EditorCommand) -> bool) {
        self.0.retain(filter)
    }

    pub fn add(&mut self, cmd: EditorCommand) {
        self.0.push(cmd)
    }

    pub fn set_or_replace_builtin_shortcut(
        &mut self,
        shortcut: KeyboardShortcut,
        cmd: BuiltInCommand,
    ) -> Option<()> {
        let cmd = self.0.iter_mut().find(|c| c.kind == Some(cmd))?;
        cmd.shortcut = Some(shortcut);
        Some(())
    }

    pub fn reset_builtins_to_default_keybindings(&mut self) {
        for command in self.0.iter_mut() {
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

    let cursor = note.cursor?;

    let text_structure = app_state.text_structure.as_ref()?;

    let text_command_context =
        TextCommandContext::new(text_structure, &note.text, cursor.ordered());

    Some(text_command_context)
}

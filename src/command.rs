use std::{fmt::Debug, ops::Deref, rc::Rc};

use eframe::egui::KeyboardShortcut;
use pulldown_cmark::CowStr;
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, FocusTarget},
    app_state::AppState,
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    persistent_state::NoteFile,
    scripting::settings_eval::Scripts,
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

#[derive(Clone, PartialEq)]
pub struct CommandInstance {
    pub shortcut: Option<KeyboardShortcut>,
    pub instruction: CommandInstruction,
    // pub handler: CommandHandler,
}

impl Debug for CommandInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorCommand")
            .field("instruction", &self.instruction)
            .finish()
    }
}

impl CommandInstance {
    pub fn built_in(instruction: CommandInstruction) -> Self {
        Self {
            shortcut: instruction.default_keybinding(),
            instruction,
        }
    }

    pub fn user_defined(
        instruction: CommandInstruction,
        shortcut: Option<KeyboardShortcut>,
    ) -> Self {
        Self {
            shortcut,
            instruction,
        }
    }
}
#[derive(Debug, Clone, knus::Decode, PartialEq, Eq)]
pub struct ScriptCall {
    #[knus(argument)]
    pub func_name: String,
}

impl ScriptCall {
    pub fn new(func_name: String) -> Self {
        Self { func_name }
    }
}

#[derive(Debug, Clone, knus::Decode, PartialEq, Eq)]
pub enum TextSource {
    #[knus(name = "string")]
    Str(#[knus(argument)] String),

    #[knus(name = "call")]
    Script(ScriptCall),
}
#[derive(PartialEq, Debug, Clone)]
pub struct ForwardToChild<T>(pub T);

impl<T> Deref for ForwardToChild<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S, T> knus::Decode<S> for ForwardToChild<T>
where
    S: knus::traits::ErrorSpan,
    T: knus::Decode<S>,
{
    fn decode_node(
        node: &knus::ast::SpannedNode<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, knus::errors::DecodeError<S>> {
        let mut iter_args = node.arguments.iter();
        if let Some(val) = iter_args.next() {
            return Err(::knus::errors::DecodeError::unexpected(
                &val.literal,
                "argument",
                "unexpected argument",
            ));
        }
        if let Some((name, val)) = node.properties.iter().next() {
            let name_str = &***name;

            return Err(::knus::errors::DecodeError::unexpected(
                name,
                "property",
                format!("unexpected property `{}`", name_str.escape_default()),
            ));
        }

        let children = node.children.as_ref().map(|lst| &lst[..]).unwrap_or(&[]);

        let single = match children {
            [single] => single,
            _ => {
                return Err(::knus::errors::DecodeError::unexpected(
                    node,
                    "node",
                    "has to be exactly one child",
                ));
            }
        };
        let decoded = T::decode_node(single, ctx)?;

        Ok(ForwardToChild(decoded))
    }
}

#[derive(PartialEq, knus::Decode, Debug, Clone)]
pub enum CommandInstruction {
    // Autocomplete/convenience
    #[knus(skip)]
    ExpandTaskMarker,
    #[knus(skip)]
    IndentListItem,
    #[knus(skip)]
    UnindentListItem,
    #[knus(skip)]
    SplitListItem,

    // Markdown
    #[knus(name = "MarkdownBold")]
    MarkdownBold,

    #[knus(name = "MarkdownItalic")]
    MarkdownItalic,

    #[knus(name = "MarkdownStrikethrough")]
    MarkdownStrikethrough,

    #[knus(name = "MarkdownCodeBlock")]
    MarkdownCodeBlock(#[knus(property(name = "lang"))] Option<String>),

    #[knus(name = "MarkdownH1")]
    MarkdownH1,

    #[knus(name = "MarkdownH2")]
    MarkdownH2,

    #[knus(name = "MarkdownH3")]
    MarkdownH3,

    // Others
    #[knus(name = "SwitchToNote")]
    SwitchToNote(#[knus(argument)] u8),

    #[knus(name = "SwitchToSettings")]
    SwitchToSettings,

    #[knus(name = "PinWindow")]
    PinWindow,

    #[knus(name = "HideApp")]
    HideApp,

    #[knus(name = "HidePrompt")]
    HidePrompt,

    // SlashPallete
    #[knus(skip)]
    ShowSlashPallete,

    #[knus(skip)]
    NextSlashPalleteCmd,

    #[knus(skip)]
    PrevSlashPalleteCmd,

    #[knus(skip)]
    ExecuteSlashPalleteCmd,

    #[knus(skip)]
    HideSlashPallete,

    // Lang specific
    #[knus(skip)]
    EnterInsideKDL,

    #[knus(skip)]
    BracketAutoclosingInsideKDL,

    // Async Code blocks
    #[knus(name = "ExecutePrompt")]
    RunLLMBlock,

    #[knus(name = "ShowPrompt")]
    ShowPrompt,

    // Script API
    #[knus(name = "InsertText")]
    InsertText(ForwardToChild<TextSource>),
}

/// Commands that we promote in UI
pub const PROMOTED_COMMANDS: [CommandInstruction; 9] = const {
    [
        CommandInstruction::PinWindow,
        CommandInstruction::MarkdownBold,
        CommandInstruction::MarkdownItalic,
        CommandInstruction::MarkdownStrikethrough,
        CommandInstruction::MarkdownCodeBlock(None),
        CommandInstruction::RunLLMBlock,
        CommandInstruction::MarkdownH1,
        CommandInstruction::MarkdownH2,
        CommandInstruction::MarkdownH3,
    ]
};

impl CommandInstruction {
    pub fn human_description(&self) -> CowStr<'static> {
        match self {
            Self::ExpandTaskMarker => "Expand Task Marker".into(),
            Self::IndentListItem => "Increase List Item identation".into(),
            Self::UnindentListItem => "Decrease List Item identation".into(),
            Self::SplitListItem => "Split List item at cursor position".into(),
            Self::MarkdownBold => "Toggle Bold".into(),
            Self::MarkdownItalic => "Toggle Italic".into(),
            Self::MarkdownStrikethrough => "Toggle Strikethrough".into(),
            Self::MarkdownCodeBlock(lang) => match lang {
                Some(language) => format!("Toggle Code Block ({})", language).into(),
                None => "Toggle Code Block".into(),
            },
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
            CommandInstruction::ShowPrompt => "Show AI Prompt".into(),
            // BuiltInCommand::ClosePopupMenu => "Close currently opened popup".into(),
            CommandInstruction::HidePrompt => "Hide Prompt".into(),
            CommandInstruction::EnterInsideKDL => "Auto indent KDL".into(),
            CommandInstruction::BracketAutoclosingInsideKDL => {
                "Auto closing of '{' inside KDL".into()
            }
            CommandInstruction::ShowSlashPallete => "Show slash command palette".into(),
            CommandInstruction::NextSlashPalleteCmd => {
                "Select next command in slash palette".into()
            }
            CommandInstruction::PrevSlashPalleteCmd => {
                "Select previous command in slash palette".into()
            }
            CommandInstruction::ExecuteSlashPalleteCmd => {
                "Execute selected command in slash palette".into()
            }
            CommandInstruction::HideSlashPallete => "Hide slash command palette".into(),

            CommandInstruction::InsertText(ForwardToChild(source)) => match source {
                TextSource::Str(str) => format!("Insert: {}", str).into(),
                TextSource::Script(script_call) => {
                    format!("Insert result from: {}", script_call.func_name).into()
                }
            },
        }
    }

    pub fn default_keybinding(&self) -> Option<eframe::egui::KeyboardShortcut> {
        use eframe::egui::{Key, Modifiers};
        use CommandInstruction as C;
        let shortcut = |mods, key| Some(KeyboardShortcut::new(mods, key));
        match self {
            C::ExpandTaskMarker => shortcut(Modifiers::NONE, Key::Space),
            C::IndentListItem => shortcut(Modifiers::NONE, Key::Tab),
            C::UnindentListItem => shortcut(Modifiers::SHIFT, Key::Tab),
            C::SplitListItem => shortcut(Modifiers::NONE, Key::Enter),
            C::MarkdownCodeBlock(None) => shortcut(Modifiers::COMMAND.plus(Modifiers::ALT), Key::B),
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
            C::InsertText(_) | C::MarkdownCodeBlock(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlashPaletteCmd {
    pub phosphor_icon: Option<String>,
    pub prefix: String,
    pub description: String,
    pub instance: CommandInstance,
}

impl SlashPaletteCmd {
    pub fn from_instruction(prefix: impl Into<String>, instruction: CommandInstruction) -> Self {
        Self {
            phosphor_icon: None,
            prefix: prefix.into(),
            description: instruction.human_description().to_string(),
            instance: CommandInstance::built_in(instruction),
        }
    }
    pub fn icon(mut self, icon: String) -> Self {
        self.phosphor_icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, shortcut: Option<KeyboardShortcut>) -> Self {
        self.instance.shortcut = shortcut;
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
    execute_instruction: Box<dyn Fn(&CommandInstruction, CommandContext) -> EditorCommandOutput>,

    defaults: (Vec<CommandInstance>, Vec<SlashPaletteCmd>),

    keyboard_commands: Vec<CommandInstance>,
    slash_commands: Vec<SlashPaletteCmd>,
}

impl Debug for CommandList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandList")
            .field("defaults", &self.defaults)
            .field("keyboard_commands", &self.keyboard_commands)
            .field("slash_commands", &self.slash_commands)
            .finish()
    }
}

impl CommandList {
    pub fn new<
        Handler: 'static + Fn(&CommandInstruction, CommandContext) -> EditorCommandOutput,
    >(
        execute: Handler,
        default_keyboard_instructions: Vec<CommandInstruction>,
        slash_palette_commands: Vec<SlashPaletteCmd>,
    ) -> Self {
        let keyboard_commands: Vec<_> = default_keyboard_instructions
            .into_iter()
            .map(CommandInstance::built_in)
            .collect();

        let defaults = (keyboard_commands.clone(), slash_palette_commands.clone());
        Self {
            defaults,
            execute_instruction: Box::new(execute),
            keyboard_commands,
            slash_commands: slash_palette_commands,
        }
    }

    pub fn available_keyboard_commands(
        &self,
    ) -> impl Iterator<Item = (KeyboardShortcut, &CommandInstance)> {
        self.keyboard_commands
            .iter()
            .flat_map(|cmd| cmd.shortcut.zip(Some(cmd)))
    }

    pub fn available_slash_commands(&self) -> impl Iterator<Item = &SlashPaletteCmd> {
        self.slash_commands.iter()
    }

    pub fn find(&self, cmd: CommandInstruction) -> Option<&CommandInstance> {
        self.keyboard_commands
            .iter()
            .rev() // in reverse to surface user defined commands first
            .find(|c| c.instruction == cmd)
    }

    pub fn add_editor_cmd(&mut self, cmd: CommandInstance) {
        if let Some(shortcut) = cmd.shortcut {
            if let Some(existing_pos) = self
                .keyboard_commands
                .iter()
                .position(|x| x.shortcut == Some(shortcut))
            {
                self.keyboard_commands.remove(existing_pos);
            }
        }

        self.keyboard_commands.push(cmd);
    }

    pub fn add_slash_command(&mut self, cmd: SlashPaletteCmd) {
        // Check for existing command with same prefix
        if let Some(existing_pos) = self
            .slash_commands
            .iter()
            .position(|x| x.prefix == cmd.prefix)
        {
            println!(
                "===== Overriding existing slash command with prefix '{}'",
                cmd.prefix
            );
            self.slash_commands.remove(existing_pos);
        }
        self.slash_commands.push(cmd);
    }

    pub fn reset_to_defaults(&mut self) {
        self.keyboard_commands.clear();
        self.keyboard_commands.extend_from_slice(&self.defaults.0);

        self.slash_commands.clear();
        self.slash_commands.extend_from_slice(&self.defaults.1);
    }

    pub fn run(
        &self,
        target_instruction: &CommandInstruction,
        ctx: CommandContext,
    ) -> EditorCommandOutput {
        (self.execute_instruction)(target_instruction, ctx)
    }
}

pub fn call_with_text_ctx(
    CommandContext { app_state, .. }: CommandContext,
    f: impl FnOnce(TextCommandContext) -> Option<Vec<TextChange>>,
) -> EditorCommandOutput {
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
}

pub fn try_extract_text_command_context(app_state: &AppState) -> Option<TextCommandContext<'_>> {
    let note = app_state.notes.get(&app_state.selected_note).unwrap();

    let cursor = note.cursor().or(note.last_cursor())?;

    let text_structure = app_state.text_structure.as_ref()?;

    let text_command_context =
        TextCommandContext::new(text_structure, &note.text, cursor.ordered());

    Some(text_command_context)
}

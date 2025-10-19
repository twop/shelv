use std::{fmt::Debug, ops::Deref};

use eframe::egui::{self, Id, Key, KeyboardShortcut, Modifiers};
use itertools::Itertools;
use pulldown_cmark::CowStr;
use smallvec::SmallVec;

use crate::{
    app_actions::AppAction,
    app_state::AppState,
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    scripting::settings_eval::Scripts,
    settings_parsing::{format_mac_shortcut_with_names, format_mac_shortcut_with_symbols},
    text_structure::TextStructure,
};

#[derive(Debug, Clone, Copy)]
pub struct TextCommandContext<'a> {
    pub text_structure: &'a TextStructure,
    pub text: &'a str,
    pub byte_cursor: ByteSpan,
}

#[derive(Clone, PartialEq, Hash, Copy, Debug)]
pub enum AppFocus {
    NoteEditor,
    InlinePropmptEditor,
    Other(Id)
}

#[derive(Clone, Copy, Debug)]
pub struct AppFocusState {
    pub is_menu_opened: bool,
    pub viewport_focused: bool,
    pub internal_focus: Option<AppFocus>,
    pub focus_id: Option<egui::Id>
}

// #[derive(Clone, Copy)]
pub struct CommandContext<'a> {
    pub app_state: &'a AppState,
    // pub app_focus: AppFocusState,
    pub ui_state: UiState,
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

#[derive(Clone, Hash, PartialEq)]
pub struct CommandInstance {
    pub shortcut: Option<KeyboardShortcut>,
    pub instruction: CommandInstruction,
    pub phase: CommandPhase,
    pub cond: CommandCondition
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
    pub fn new(instruction: CommandInstruction) -> Self {
        let (phase, cond) = instruction.default_phase_and_conditions();
        Self {
            shortcut: instruction.default_keybinding(),
            instruction,
            cond,
            phase,
        }
    }

    pub fn new_with_shortcut(
        instruction: CommandInstruction,
        shortcut: Option<KeyboardShortcut>,
    ) -> Self {
        let (phase, cond) = instruction.default_phase_and_conditions();
        Self {
            shortcut,
            instruction,
            cond,
            phase,
        }
    }

    pub fn phase(self, phase: CommandPhase) -> Self {
        Self {
            phase,
             ..self
        }
    }
}

#[derive(Debug, Clone, Hash, knus::Decode, PartialEq, Eq)]
pub enum ScriptCallArgument{
    #[knus(name = "selection")]
    Selection
}

#[derive(Debug, Hash, Clone, knus::Decode, PartialEq, Eq)]
pub struct ScriptCall {
    #[knus(argument)]
    pub func_name: String,

    #[knus(children)]
    pub arguments: SmallVec<[ScriptCallArgument; 1]>,
}

// impl ScriptCall {
//     pub fn new(func_name: String) -> Self {
//         Self { func_name }
//     }
// }

#[derive(Debug, Clone, Hash, knus::Decode, PartialEq, Eq)]
pub enum TextSource {
    #[knus(name = "string")]
    Str(#[knus(argument)] String),

    #[knus(name = "callFunc")]
    Script(ScriptCall),
}
#[derive(PartialEq, Hash, Debug, Clone)]
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
        if let Some((name, _val)) = node.properties.iter().next() {
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

#[derive(PartialEq, Hash, Debug, Clone)]
pub struct UiState(SmallVec<[UiStateAttribute; 6]>);

impl UiState {
    pub fn new(attributes: impl IntoIterator<Item = UiStateAttribute>) -> Self {
        Self(SmallVec::from_iter(attributes.into_iter()))
    }

    pub fn attributes(&self) -> &[UiStateAttribute] {
        &self.0
    }
}

#[derive(PartialEq, Hash, Debug, Clone)]
pub enum UiStateAttribute {
    Idle,
    FeedbackOpened,
    JumpMode,
    SlashMenu,
    Focus(AppFocus),
}

#[derive(Debug, PartialEq, Hash, Copy, Clone)]
pub enum CommandPhase {
    InsideRender,
    RawInputHook,
}

#[derive(PartialEq, Clone, Hash, Debug)]
pub enum CommandCondition {
    Not(Box<CommandCondition>),
    All(Vec<CommandCondition>),
    Any(Vec<CommandCondition>),
    LooseMatch(UiState),
    ExactMatch(UiState)
}

impl CommandCondition {
    fn eval(&self, state: &UiState) -> bool {
        match self {
            CommandCondition::ExactMatch(required_state) => {
                let state_attrs = state.attributes();
                let required_attrs = required_state.attributes();
                
                state_attrs.len() == required_attrs.len() && 
                required_attrs.iter().all(|req_attr| state_attrs.contains(req_attr))
            }
            
            CommandCondition::LooseMatch(required_state) => {
                let state_attrs = state.attributes();
                let required_attrs = required_state.attributes();
                
                required_attrs.iter().all(|req_attr| state_attrs.contains(req_attr))
            }
            
            CommandCondition::Not(inner_condition) => {
                !inner_condition.eval(state)
            }
            
            CommandCondition::All(conditions) => {
                conditions.iter().all(|condition| condition.eval(state))
            }
            
            CommandCondition::Any(conditions) => {
                conditions.iter().any(|condition| condition.eval(state))
            }
        }
    }

    pub fn loose_match( attributes : impl IntoIterator<Item = UiStateAttribute> ) -> Self {
        Self::LooseMatch(UiState::new(attributes.into_iter()))
    }

    pub fn exact_match( attributes : impl IntoIterator<Item = UiStateAttribute> ) -> Self {
        Self::ExactMatch(UiState::new(attributes.into_iter()))
    }

    pub fn or(self, other: Self) -> Self {
        Self::Any(Vec::from([self, other]))
    }

    
}

#[derive(PartialEq, Hash, knus::Decode, Debug, Clone)]
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

    // SlashPallete
    #[knus(skip)]
    ShowSlashPallete,

    // Lang specific
    #[knus(skip)]
    EnterInsideKDL,

    #[knus(skip)]
    BracketAutoclosingInsideKDL,

    // Async Code blocks
    // #[knus(name = "ExecutePrompt")]
    // RunLLMBlock,

    #[knus(name = "ShowPrompt")]
    ShowPrompt,

    #[knus(name = "StartWordJump")]
    StartWordJump,

    // Script API
    #[knus(name = "InsertText")]
    InsertText(ForwardToChild<TextSource>),
}

/// Commands that we promote in UI
pub const PROMOTED_COMMANDS: [CommandInstruction; 10] = const {
    [
        CommandInstruction::PinWindow,
        CommandInstruction::MarkdownBold,
        CommandInstruction::MarkdownItalic,
        CommandInstruction::MarkdownStrikethrough,
        CommandInstruction::MarkdownCodeBlock(None),
        // CommandInstruction::RunLLMBlock,
        CommandInstruction::MarkdownH1,
        CommandInstruction::MarkdownH2,
        CommandInstruction::MarkdownH3,
        CommandInstruction::ShowPrompt,
        CommandInstruction::StartWordJump,
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
            // Self::RunLLMBlock => "Execute AI Block".into(),
            CommandInstruction::ShowPrompt => "Show AI Prompt".into(),
            CommandInstruction::StartWordJump => "Activates word jump mode for quick navigation to any word using 2-character sequences".into(),
            CommandInstruction::EnterInsideKDL => "Auto indent KDL".into(),
            CommandInstruction::BracketAutoclosingInsideKDL => {
                "Auto closing of '{' inside KDL".into()
            }
            CommandInstruction::ShowSlashPallete => "Show slash command palette".into(),
            CommandInstruction::InsertText(ForwardToChild(source)) => match source {
                TextSource::Str(str) => format!("Insert: {}", str).into(),
                TextSource::Script(script_call) => {
                    format!("Insert result from: {}", script_call.func_name).into()
                }
            },
        }
    }

    pub fn default_keybinding(&self) -> Option<eframe::egui::KeyboardShortcut> {
        use CommandInstruction as C;
        use eframe::egui::{Key, Modifiers};
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
            C::SwitchToNote(_) => shortcut(Modifiers::COMMAND, Key::Num0),
            C::SwitchToSettings => shortcut(Modifiers::COMMAND, Key::Comma),
            C::PinWindow => shortcut(Modifiers::COMMAND, Key::P),
            C::ShowPrompt => shortcut(Modifiers::CTRL, Key::Enter),
            C::StartWordJump => shortcut(Modifiers::COMMAND, Key::J),
            C::EnterInsideKDL => shortcut(Modifiers::NONE, Key::Enter),
            C::BracketAutoclosingInsideKDL => shortcut(Modifiers::SHIFT, Key::OpenBracket),
            C::HideApp => shortcut(Modifiers::NONE, Key::Escape),
            C::ShowSlashPallete => shortcut(Modifiers::NONE, Key::Slash),
            C::InsertText(_) | C::MarkdownCodeBlock(_) => None,
        }
    }

    pub fn default_phase_and_conditions(&self) -> (CommandPhase, CommandCondition) {
        use CommandInstruction as C;
        match self {
            // Text editing commands that require note editor focus
            C::ExpandTaskMarker
            | C::IndentListItem
            | C::UnindentListItem
            | C::SplitListItem
            | C::MarkdownCodeBlock(_)
            | C::MarkdownBold
            | C::MarkdownItalic
            | C::MarkdownStrikethrough
            | C::MarkdownH1
            | C::MarkdownH2
            | C::MarkdownH3
            | C::EnterInsideKDL
            | C::ShowPrompt
            | C::StartWordJump
            | C::ShowSlashPallete
            | C::BracketAutoclosingInsideKDL
            | C::InsertText(_) => (
                CommandPhase::InsideRender,
                CommandCondition::exact_match([
                    UiStateAttribute::Idle,
                    UiStateAttribute::Focus(AppFocus::NoteEditor),
                ])
            ),

            // Global commands that work in any context
            C::SwitchToSettings
            | C::PinWindow => (
                CommandPhase::InsideRender,
                CommandCondition::loose_match([UiStateAttribute::Idle])
            ),

            // Commands that work in editor or idle state
            // e.g. don't require focus to work
            C::HideApp | C::SwitchToNote(_)=> (
                CommandPhase::InsideRender,
                CommandCondition::exact_match([
                    UiStateAttribute::Idle,
                    UiStateAttribute::Focus(AppFocus::NoteEditor),
                ])
                .or(CommandCondition::exact_match([UiStateAttribute::Idle]))
            ),
        }
    }

    pub fn serialize_to_kdl(&self) -> Option<CowStr> {
        match self {
            Self::ExpandTaskMarker
            | Self::IndentListItem
            | Self::UnindentListItem
            | Self::SplitListItem
            | Self::ShowSlashPallete
            | Self::EnterInsideKDL
            | Self::BracketAutoclosingInsideKDL => None,

            Self::MarkdownBold => Some("MarkdownBold;".into()),
            Self::MarkdownItalic => Some("MarkdownItalic;".into()),
            Self::MarkdownStrikethrough => Some("MarkdownStrikethrough;".into()),
            Self::MarkdownCodeBlock(lang) => match lang {
                Some(lang_str) => Some(format!("MarkdownCodeBlock lang=\"{}\";", lang_str).into()),
                None => Some("MarkdownCodeBlock;".into()),
            },
            Self::MarkdownH1 => Some("MarkdownH1;".into()),
            Self::MarkdownH2 => Some("MarkdownH2;".into()),
            Self::MarkdownH3 => Some("MarkdownH3;".into()),
            Self::SwitchToNote(n) => Some(format!("SwitchToNote {};", n).into()),
            Self::SwitchToSettings => Some("SwitchToSettings;".into()),
            Self::PinWindow => Some("PinWindow;".into()),
            Self::HideApp => Some("HideApp;".into()),
            // Self::RunLLMBlock => Some("ExecutePrompt;".into()),
            Self::ShowPrompt => Some("ShowPrompt;".into()),
            Self::StartWordJump => Some("StartWordJump;".into()),
            Self::InsertText(ForwardToChild(source)) => match source {
                TextSource::Str(text) => {
                    Some(format!("InsertText {{\n\tas_is \"{}\"\n}}", text).into())
                }
                TextSource::Script(script) => {
                    Some(format!("InsertText {{\n\t callFunc \"{}\"\n}}", script.func_name).into())
                }
            },
        }
    }
}

#[derive(Debug, Hash, Clone)]
pub struct PhosphorIcon {
    name: &'static str,
    unicode_symbol: &'static str,
}

impl PhosphorIcon {
    pub fn symbol(&self) -> &'static str {
        self.unicode_symbol
    }

    pub fn canonical_name(&self) -> String {
        let kebab_name = self.name.to_lowercase().replace('_', "-");
        kebab_name
    }

    pub fn from_string(icon_name: &str) -> Option<Self> {
        // Convert kebab-case to UPPER_SNAKE_CASE
        let upper_snake = icon_name.to_uppercase().replace('-', "_");
        
        // First try to find by key (UPPER_SNAKE_CASE)
        if let Some((key, icon_char)) = egui_phosphor::light::ICONS
            .iter()
            .find(|(key, _)| *key == upper_snake)
        {
            return Some(PhosphorIcon {
                name: key,
                unicode_symbol: icon_char,
            });
        }

        // If not found by key, try to find by value (direct unicode char)
        if let Some((key, icon_char)) = egui_phosphor::light::ICONS
            .iter()
            .find(|(_, value)| *value == icon_name)
        {
            return Some(PhosphorIcon {
                name: key,
                unicode_symbol: icon_char,
            });
        }

        None
    }
}

#[derive(Debug, Hash, Clone)]
pub struct SlashPaletteCmd {
    pub phosphor_icon: Option<PhosphorIcon>,
    pub prefix: String,
    pub description: String,
    pub instance: CommandInstance,
}

impl SlashPaletteCmd {
    pub fn from_instruction(
        prefix: impl Into<String>,
        instruction: CommandInstruction,
    ) -> Self {
        Self {
            phosphor_icon: None,
            prefix: prefix.into(),
            description: instruction.human_description().to_string(),
            instance: CommandInstance::new(instruction),
        }
    }
    pub fn icon(mut self, icon: String) -> Self {
        self.phosphor_icon = PhosphorIcon::from_string(&icon);
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

#[derive(PartialEq, Debug)]
pub enum FrameHotkeyLayer {
    Normal,
    Modal,
}

pub struct FrameHotkey {
    layer: FrameHotkeyLayer,
    shortcut: KeyboardShortcut,
    pub run: Box<dyn for<'a> Fn(CommandContext<'a>) -> EditorCommandOutput>,
}

impl FrameHotkey {
    pub fn new(
        shortcut: KeyboardShortcut,
        run: impl Fn(CommandContext) -> EditorCommandOutput + 'static,
    ) -> Self {
        Self {
            layer: FrameHotkeyLayer::Normal,
            shortcut,
            run: Box::new(run),
        }
    }
}

/// Hotkeys that are only valid until the next render, that is, after the frame fills them in
/// they can be triggered at the begining of the next one, and then cleared
/// useful for stuff like modal dialog shortcuts and such
pub struct FrameHotkeys(Vec<FrameHotkey>);

impl FrameHotkeys {
    pub fn add_key(
        &mut self,
        key: Key,
        run: impl for<'a> Fn(CommandContext<'a>) -> EditorCommandOutput + 'static,
    ) {
        self.0.push(FrameHotkey::new(
            KeyboardShortcut::new(Modifiers::NONE, key),
            run,
        ));
    }

    pub fn add_key_with_modifier(
        &mut self,
        modifier: Modifiers,
        key: Key,
        run: impl for<'a> Fn(CommandContext<'a>) -> EditorCommandOutput + 'static,
    ) {
        self.0
            .push(FrameHotkey::new(KeyboardShortcut::new(modifier, key), run));
    }

    pub fn add_with_layer(&mut self, mut frame_hotkey: FrameHotkey, layer: FrameHotkeyLayer) {
        frame_hotkey.layer = layer;
        self.0.push(frame_hotkey);
    }
}

pub struct CommandList {
    execute_instruction: Box<dyn Fn(&CommandInstruction, CommandContext) -> EditorCommandOutput>,

    defaults: (Vec<CommandInstance>, Vec<SlashPaletteCmd>),

    keyboard_commands: Vec<CommandInstance>,
    slash_commands: Vec<SlashPaletteCmd>,
    frame_hotkeys: Vec<FrameHotkey>,
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

pub enum KeyboardBinding<'a> {
    CommandInstance(&'a CommandInstance),
    FrameBinding(&'a FrameHotkey),
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
            .map(|instruction | CommandInstance::new(instruction))
            .collect();

        let defaults = (keyboard_commands.clone(), slash_palette_commands.clone());
        Self {
            defaults,
            frame_hotkeys: Vec::new(),
            execute_instruction: Box::new(execute),
            keyboard_commands,
            slash_commands: slash_palette_commands,
        }
    }

    pub fn available_keyboard_commands(
        &self,
    ) -> impl Iterator<Item = (KeyboardShortcut, KeyboardBinding)> {
        self.frame_hotkeys
            .iter()
            .rev()
            .filter(|h| h.layer == FrameHotkeyLayer::Modal)
            .chain(
                self.frame_hotkeys
                    .iter()
                    .rev()
                    .filter(|h| h.layer != FrameHotkeyLayer::Modal),
            )
            .map(|hotkey| (hotkey.shortcut, KeyboardBinding::FrameBinding(hotkey)))
            .chain(self.keyboard_commands.iter().flat_map(|cmd| {
                cmd.shortcut
                    .zip(Some(KeyboardBinding::CommandInstance(cmd)))
            }))
    }
    pub fn prepare_frame_hotkeys(&mut self) -> FrameHotkeys {
        self.frame_hotkeys.clear();
        FrameHotkeys(std::mem::take(&mut self.frame_hotkeys))
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
        cond: &CommandCondition,
        ctx: CommandContext,
    ) -> EditorCommandOutput {
        if cond.eval(&ctx.ui_state){
            (self.execute_instruction)(target_instruction, ctx)
        }
        else {
            SmallVec::new()
        }
    }

    pub fn add_frame_hotkeys(&mut self, FrameHotkeys(hotkeys): FrameHotkeys) {
        self.frame_hotkeys.extend(hotkeys);
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

    let text_structure = &note.derived_state.structure;

    let text_command_context =
        TextCommandContext::new(text_structure, &note.text, cursor.ordered());

    Some(text_command_context)
}

pub fn create_ai_keybindings_documentation(cmd_list: &CommandList) -> String {
    use eframe::egui::{Key, Modifiers};

    let global_shortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::ALT), Key::S);

    let current_commands_help = cmd_list
        .available_keyboard_commands()
        .filter_map(|(shortcut, cmd)| {
            match cmd{
                KeyboardBinding::CommandInstance(cmd) => cmd.instruction.serialize_to_kdl().map(|kdl| {
                (
                    shortcut,
                    kdl,
                    &cmd.instruction,
                    cmd_list
                        .available_slash_commands()
                        .find(|scmd| scmd.instance.instruction == cmd.instruction),
                )
            }),
                KeyboardBinding::FrameBinding(_frame_hotkey) => None,
            }
            
        })
        .map(|(shortcut, kdl_block, instruction, slash_cmd)| {
            let text = format!(
                "// ({symbols_shortcut}): {desc}\nbind \"{key_combo}\" {slash_cmd_attrs}{{ {kdl} }}",
                symbols_shortcut = format_mac_shortcut_with_symbols(shortcut),
                desc = instruction.human_description(),
                key_combo = format_mac_shortcut_with_names(shortcut),
                kdl = kdl_block,
                slash_cmd_attrs = match slash_cmd {
                    Some(cmd) => {
                        let mut attrs = String::new();
                        if let Some(phosphor_icon) = cmd.phosphor_icon.as_ref() {
                            attrs.push_str(&format!("icon=\"{}\" ", phosphor_icon.canonical_name()));
                        }

                        attrs.push_str(&format!("alias=\"{}\" description=\"{}\" ",
                            cmd.prefix, cmd.description));
                        attrs
                    },
                    None => String::new()
                }
            );
            text
        })
        .join("\n\n");

    current_commands_help
}
#[test]
fn test_keybindings_documentation_generation() {
    use eframe::egui::{Key, Modifiers};

    // Create two commands - one with slash command and one without
    let kbd_shortcut1 = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);

    let cmd_list = CommandList::new(
        |_, _| SmallVec::new(),
        vec![
            CommandInstruction::MarkdownBold,
            CommandInstruction::MarkdownItalic,
        ],
        vec![
            SlashPaletteCmd::from_instruction(
                "bold",
                CommandInstruction::MarkdownBold,
            )
            .icon(egui_phosphor::light::USER_CIRCLE_GEAR.to_string())
            .shortcut(Some(kbd_shortcut1))
            .description("Make text bold"),
        ],
    );

    // let t = "\u{E10A}";
    // let v: Vec<u32> = t.chars().map(|c| c as u32).collect();
    // assert_eq!(v.as_slice(), &[1]);

    let docs = create_ai_keybindings_documentation(&cmd_list);

    let expected_docs = r#"// (⌘ B): Toggle Bold
bind "Cmd B" icon="user-circle-gear" alias="bold" description="Make text bold" { MarkdownBold; }

// (⌘ I): Toggle Italic
bind "Cmd I" { MarkdownItalic; }"#;

    assert_eq!(docs, expected_docs);
}

#[cfg(test)]
mod command_condition_tests {
    use super::*;
    use UiStateAttribute as A;
    use CommandCondition as C;

    #[test]
    fn test_exact_match() {
        let state = UiState::new([A::Idle, A::Focus(AppFocus::NoteEditor)]);
        let condition = C::exact_match([A::Idle, A::Focus(AppFocus::NoteEditor)]);
        assert!(condition.eval(&state));

        // different order should still match
        let different_order = C::exact_match([A::Focus(AppFocus::NoteEditor), A::Idle]);
        assert!(different_order.eval(&state));

        let just_idle = C::exact_match([A::Idle]);
        assert!(!just_idle.eval(&state));

        // Test exact match with missing attribute in state (should not match)
        let extra_requirements = C::exact_match([A::Idle, A::Focus(AppFocus::NoteEditor), A::JumpMode]);
        assert!(!extra_requirements.eval(&state));

        let empty_state = UiState::new([]);
        let empty_condition = C::exact_match([]);
        assert!(empty_condition.eval(&empty_state));
    }

    #[test]
    fn test_loose_match() {
        let state = UiState::new([A::Idle, A::Focus(AppFocus::NoteEditor), A::JumpMode]);

        let smaller_condition = C::loose_match([A::Idle, A::Focus(AppFocus::NoteEditor), A::JumpMode]);
        assert!(smaller_condition.eval(&state));

        let smaller_condition = C::loose_match([A::Idle, A::Focus(AppFocus::NoteEditor)]);
        assert!(smaller_condition.eval(&state));

        let empty_condition = C::loose_match([]);
        assert!(empty_condition.eval(&state));

        let empty_state = UiState::new([]);
        let non_empty_condition = C::loose_match([A::Idle]);
        assert!(!non_empty_condition.eval(&empty_state));
    }

    #[test]
    fn test_not_condition() {
        let state = UiState::new([A::Idle]);
        
        let matching = C::exact_match([A::Idle]);
        let not_matching = C::Not(Box::new(matching));
        assert!(!not_matching.eval(&state));

        let double_negative = C::Not(Box::new(not_matching));
        assert!(!double_negative.eval(&state));
    }

    #[test]
    fn test_all_condition() {
        let state = UiState::new([A::Idle, A::Focus(AppFocus::NoteEditor)]);

        let all_true = C::All(vec![
            C::loose_match([A::Idle]),
            C::loose_match([A::Focus(AppFocus::NoteEditor)]),
        ]);
        assert!(all_true.eval(&state));

        let one_false = C::All(vec![
            C::loose_match([A::Idle]),
            C::loose_match([A::JumpMode]),
        ]);
        assert!(!one_false.eval(&state));

        let empty_all = C::All(vec![]);
        assert!(empty_all.eval(&state));
    }

    #[test]
    fn test_any_condition() {
        let state = UiState::new([A::Idle]);

        let any_true = C::Any(vec![
            C::loose_match([A::JumpMode]),
            C::loose_match([A::Idle]),
        ]);
        assert!(any_true.eval(&state));

        let all_false = C::Any(vec![
            C::loose_match([A::JumpMode]),
            C::loose_match([A::SlashMenu]),
        ]);
        assert!(!all_false.eval(&state));

        let empty_any = C::Any(vec![]);
        assert!(!empty_any.eval(&state));
    }

    #[test]
    fn test_complex_nested_conditions() {
        let state = UiState::new([A::JumpMode, A::Focus(AppFocus::NoteEditor)]);

        let complex_condition = C::Any(vec![
            C::Any(vec![
                C::loose_match([A::Idle]),
                C::loose_match([A::Focus(AppFocus::NoteEditor)]),
            ]),
            C::loose_match([A::JumpMode]),
        ]);
        assert!(complex_condition.eval(&state));
    }
}

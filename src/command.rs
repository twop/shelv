use std::fmt::Debug;

use eframe::egui::{self, Id, Key, KeyboardShortcut, Modifiers};
use itertools::Itertools;
use smallvec::SmallVec;

use crate::{
    app_actions::AppAction,
    app_state::AppState,
    byte_span::ByteSpan,
    effects::text_change_effect::TextChange,
    scripting::{settings_decode::UserCommandId, settings_eval::Scripts},
    shortcuts::{format_mac_shortcut_with_names, format_mac_shortcut_with_symbols},
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
    Other(Id),
}

#[derive(Clone, Copy, Debug)]
pub struct AppFocusState {
    pub is_menu_opened: bool,
    pub viewport_focused: bool,
    pub internal_focus: Option<AppFocus>,
    pub focus_id: Option<egui::Id>,
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
    pub cond: CommandCondition,
    /// The settings command this instance comes from (alias and description written in JS).
    pub label: Option<CommandLabel>,
}

/// A settings command's name and text, shown in the palette, hints and run-time errors.
#[derive(Clone, Hash, PartialEq, Debug)]
pub struct CommandLabel {
    pub alias: String,
    pub description: String,
}

impl Debug for CommandInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorCommand")
            .field("instruction", &self.instruction)
            .field("label", &self.label)
            .finish()
    }
}

impl CommandInstance {
    pub fn phase(self, phase: CommandPhase) -> Self {
        Self { phase, ..self }
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
    ExactMatch(UiState),
}

impl CommandCondition {
    fn eval(&self, state: &UiState) -> bool {
        match self {
            CommandCondition::ExactMatch(required_state) => {
                let state_attrs = state.attributes();
                let required_attrs = required_state.attributes();

                state_attrs.len() == required_attrs.len()
                    && required_attrs
                        .iter()
                        .all(|req_attr| state_attrs.contains(req_attr))
            }

            CommandCondition::LooseMatch(required_state) => {
                let state_attrs = state.attributes();
                let required_attrs = required_state.attributes();

                required_attrs
                    .iter()
                    .all(|req_attr| state_attrs.contains(req_attr))
            }

            CommandCondition::Not(inner_condition) => !inner_condition.eval(state),

            CommandCondition::All(conditions) => {
                conditions.iter().all(|condition| condition.eval(state))
            }

            CommandCondition::Any(conditions) => {
                conditions.iter().any(|condition| condition.eval(state))
            }
        }
    }

    pub fn loose_match(attributes: impl IntoIterator<Item = UiStateAttribute>) -> Self {
        Self::LooseMatch(UiState::new(attributes.into_iter()))
    }

    pub fn exact_match(attributes: impl IntoIterator<Item = UiStateAttribute>) -> Self {
        Self::ExactMatch(UiState::new(attributes.into_iter()))
    }

    pub fn or(self, other: Self) -> Self {
        Self::Any(Vec::from([self, other]))
    }
}

/// Everything a key, a palette entry or a settings command can do. The variants except
/// `RunUserCommand` are also the JS `action.*` of system.ts: settings decode them with serde from the
/// `{ builtin, ...params }` objects system.ts builds (scripting::actions), and a test keeps the two
/// lists equal. Names and parameter names match the JS ones.
#[derive(PartialEq, Hash, Debug, Clone, serde::Deserialize, strum::VariantNames)]
#[serde(tag = "builtin", deny_unknown_fields)]
pub enum CommandInstruction {
    // text and markdown
    /// Replaces the selection; `{||}` / `{|}..{|}` in the text place the cursor or select.
    InsertText {
        text: String,
    },
    MarkdownBold,
    MarkdownItalic,
    MarkdownStrikethrough,
    /// `None`: a code block without language.
    MarkdownCodeBlock {
        #[serde(default)]
        lang: Option<String>,
    },
    MarkdownH1,
    MarkdownH2,
    MarkdownH3,

    // lists
    IndentListItem,
    UnindentListItem,
    SplitListItem,
    ExpandTaskMarker,

    // structural editing
    ExpandSelection,
    ShrinkSelection,
    MoveLinesUp,
    MoveLinesDown,

    // navigation and palette
    StartWordJump,
    ShowSlashPalette,
    ShowPrompt,

    // notes and window
    /// `index` is 0-based here; JS's `action.SwitchToNote({ index })` is 1-based (the shelf labels).
    SwitchToNote {
        #[serde(deserialize_with = "shelf_index_from_one_based")]
        index: u8,
    },
    SwitchToSettings,
    OpenFileDialog,
    CloseCurrentNote,
    PinWindow,
    HideApp,
    ToggleDebugTools,

    /// Global only: the whole `run` of a command bound in `global` (a system-wide hotkey).
    ShowHideApp,

    /// A settings command whose `run` is a function or several actions (see scripting::settings_eval).
    #[serde(skip)]
    RunUserCommand(UserCommandId),
}

/// JS shelf numbers are 1-4 (and may arrive as floats, `2.0`); the app's are 0-3.
fn shelf_index_from_one_based<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    use serde::{Deserialize, de::Error};
    let n = f64::deserialize(deserializer)?;
    if n.fract() == 0.0 && (1.0..=4.0).contains(&n) {
        Ok(n as u8 - 1)
    } else {
        Err(D::Error::custom(format!(
            "index must be one of 1, 2, 3, 4, got {n}"
        )))
    }
}

/// Commands that we promote in UI
pub const PROMOTED_COMMANDS: [CommandInstruction; 11] = const {
    [
        CommandInstruction::PinWindow,
        CommandInstruction::MarkdownBold,
        CommandInstruction::MarkdownItalic,
        CommandInstruction::MarkdownStrikethrough,
        CommandInstruction::MarkdownCodeBlock { lang: None },
        // CommandInstruction::RunLLMBlock,
        CommandInstruction::MarkdownH1,
        CommandInstruction::MarkdownH2,
        CommandInstruction::MarkdownH3,
        CommandInstruction::ShowPrompt,
        CommandInstruction::StartWordJump,
        CommandInstruction::OpenFileDialog,
    ]
};

impl CommandInstruction {
    pub fn default_phase_and_conditions(&self) -> (CommandPhase, CommandCondition) {
        use CommandInstruction as C;
        match self {
            // Text editing commands that require note editor focus
            C::ExpandTaskMarker
            | C::IndentListItem
            | C::UnindentListItem
            | C::SplitListItem
            | C::MarkdownCodeBlock { .. }
            | C::MarkdownBold
            | C::MarkdownItalic
            | C::MarkdownStrikethrough
            | C::MarkdownH1
            | C::MarkdownH2
            | C::MarkdownH3
            | C::ShowPrompt
            | C::StartWordJump
            | C::ExpandSelection
            | C::ShrinkSelection
            | C::MoveLinesUp
            | C::MoveLinesDown
            | C::ShowSlashPalette
            | C::InsertText { .. }
            | C::RunUserCommand(_) => (
                CommandPhase::InsideRender,
                CommandCondition::exact_match([
                    UiStateAttribute::Idle,
                    UiStateAttribute::Focus(AppFocus::NoteEditor),
                ]),
            ),

            // Global commands that work in any context
            C::SwitchToSettings
            | C::PinWindow
            | C::ToggleDebugTools
            | C::OpenFileDialog
            | C::CloseCurrentNote => (
                CommandPhase::InsideRender,
                CommandCondition::loose_match([UiStateAttribute::Idle]),
            ),

            // Commands that work in editor or idle state
            // e.g. don't require focus to work
            C::HideApp | C::ShowHideApp | C::SwitchToNote { .. } => (
                CommandPhase::InsideRender,
                CommandCondition::exact_match([
                    UiStateAttribute::Idle,
                    UiStateAttribute::Focus(AppFocus::NoteEditor),
                ])
                .or(CommandCondition::exact_match([UiStateAttribute::Idle])),
            ),
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

pub struct InnerHotkey(KeyboardShortcut);

impl From<Key> for InnerHotkey {
    fn from(logical_key: Key) -> Self {
        InnerHotkey(KeyboardShortcut::new(Modifiers::NONE, logical_key))
    }
}

impl From<(Modifiers, Key)> for InnerHotkey {
    fn from((modifiers, logical_key): (Modifiers, Key)) -> Self {
        InnerHotkey(KeyboardShortcut::new(modifiers, logical_key))
    }
}

pub struct InnerFrameHotkeyHandler(Box<dyn for<'a> Fn(CommandContext<'a>) -> EditorCommandOutput>);

impl From<AppAction> for InnerFrameHotkeyHandler {
    fn from(value: AppAction) -> Self {
        let as_collection = EditorCommandOutput::from_iter([value]);
        Self(Box::new(move |_ctx| as_collection.clone()))
    }
}

impl<const N: usize> From<[AppAction; N]> for InnerFrameHotkeyHandler {
    fn from(value: [AppAction; N]) -> Self {
        let as_collection = EditorCommandOutput::from_iter(value);
        Self(Box::new(move |_ctx| as_collection.clone()))
    }
}

impl<T> From<T> for InnerFrameHotkeyHandler
where
    T: Fn(CommandContext) -> EditorCommandOutput + 'static,
{
    fn from(value: T) -> Self {
        Self(Box::new(value))
    }
}

pub struct FrameHotkey {
    phase: CommandPhase,
    condition: CommandCondition,
    shortcut: KeyboardShortcut,
    handler: InnerFrameHotkeyHandler,
}

impl FrameHotkey {
    pub fn new(
        shortcut: impl Into<InnerHotkey>,
        handler: impl Into<InnerFrameHotkeyHandler>,
        // condition: CommandCondition,
    ) -> Self {
        let InnerHotkey(shortcut) = shortcut.into();
        Self {
            phase: CommandPhase::InsideRender,
            condition: CommandCondition::loose_match([]),
            shortcut,
            handler: handler.into(),
        }
    }

    pub fn raw_input(
        shortcut: impl Into<InnerHotkey>,
        // condition: CommandCondition,
        handler: impl Into<InnerFrameHotkeyHandler>,
    ) -> Self {
        Self {
            phase: CommandPhase::RawInputHook,
            ..Self::new(shortcut, handler)
        }
    }

    // pub fn phase(self, phase: CommandPhase) -> Self {
    //     Self {
    //         phase,
    //         ..self
    //     }
    // }

    pub fn run<'a>(&self, ctx: CommandContext<'a>) -> EditorCommandOutput {
        (self.handler.0)(ctx)
    }
}

/// Hotkeys that are only valid until the next render, that is, after the frame fills them in
/// they can be triggered at the begining of the next one, and then cleared
/// useful for stuff like modal dialog shortcuts and such
pub struct FrameHotkeys(Vec<FrameHotkey>);

impl FrameHotkeys {
    pub fn add_key(&mut self, frame_hotkey: FrameHotkey) {
        self.0.push(frame_hotkey);
    }
}

pub struct CommandList {
    execute_instruction: Box<dyn Fn(&CommandInstruction, CommandContext) -> EditorCommandOutput>,

    keyboard_commands: Vec<CommandInstance>,
    slash_commands: Vec<SlashPaletteCmd>,
    frame_hotkeys: Vec<FrameHotkey>,
}

impl Debug for CommandList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandList")
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
    ) -> Self {
        // empty until the settings (the JS base layer, then the settings note) are applied
        Self {
            frame_hotkeys: Vec::new(),
            execute_instruction: Box::new(execute),
            keyboard_commands: Vec::new(),
            slash_commands: Vec::new(),
        }
    }

    /// Replaces every keyboard command and palette entry (the result of evaluating the settings).
    pub fn set_commands(
        &mut self,
        keyboard_commands: Vec<CommandInstance>,
        slash_commands: Vec<SlashPaletteCmd>,
    ) {
        self.keyboard_commands = keyboard_commands;
        self.slash_commands = slash_commands;
    }

    pub fn keyboard_commands(&self) -> &[CommandInstance] {
        &self.keyboard_commands
    }

    pub fn available_keyboard_commands_for_phase(
        &self,
        phase: CommandPhase,
    ) -> impl Iterator<Item = (KeyboardShortcut, KeyboardBinding)> {
        self.frame_hotkeys
            .iter()
            .rev()
            .filter(move |h| h.phase == phase)
            .map(|hotkey| (hotkey.shortcut, KeyboardBinding::FrameBinding(hotkey)))
            .chain(
                self.keyboard_commands
                    .iter()
                    .filter(move |cmd| cmd.phase == phase)
                    .flat_map(|cmd| {
                        cmd.shortcut
                            .zip(Some(KeyboardBinding::CommandInstance(cmd)))
                    }),
            )
    }

    pub fn has_binding_for(&self, shortcut: KeyboardShortcut) -> bool {
        self.frame_hotkeys
            .iter()
            .any(|hotkey| hotkey.shortcut == shortcut)
            || self
                .keyboard_commands
                .iter()
                .any(|cmd| cmd.shortcut == Some(shortcut))
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

    pub fn run(
        &self,
        target_instruction: &CommandInstruction,
        cond: &CommandCondition,
        ctx: CommandContext,
    ) -> EditorCommandOutput {
        if cond.eval(&ctx.ui_state) {
            (self.execute_instruction)(target_instruction, ctx)
        } else {
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

/// The current keyboard shortcuts, one per line, for the AI system prompt (`{{current_keybindings}}`).
pub fn create_ai_keybindings_documentation(cmd_list: &CommandList) -> String {
    cmd_list
        .keyboard_commands()
        .iter()
        .filter_map(|cmd| Some((cmd.shortcut?, cmd.label.as_ref()?)))
        .map(|(shortcut, label)| {
            let in_palette = cmd_list
                .available_slash_commands()
                .any(|slash| slash.prefix == label.alias);
            format!(
                "- `{names}` ({symbols}): `{alias}`, {description}{palette}",
                names = format_mac_shortcut_with_names(shortcut),
                symbols = format_mac_shortcut_with_symbols(shortcut),
                alias = label.alias,
                description = label.description,
                palette = if in_palette {
                    format!(" (slash palette: /{})", label.alias)
                } else {
                    String::new()
                },
            )
        })
        .join("\n")
}

#[cfg(test)]
mod command_condition_tests {
    use super::*;
    use CommandCondition as C;
    use UiStateAttribute as A;

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
        let extra_requirements =
            C::exact_match([A::Idle, A::Focus(AppFocus::NoteEditor), A::JumpMode]);
        assert!(!extra_requirements.eval(&state));

        let empty_state = UiState::new([]);
        let empty_condition = C::exact_match([]);
        assert!(empty_condition.eval(&empty_state));
    }

    #[test]
    fn test_loose_match() {
        let state = UiState::new([A::Idle, A::Focus(AppFocus::NoteEditor), A::JumpMode]);

        let smaller_condition =
            C::loose_match([A::Idle, A::Focus(AppFocus::NoteEditor), A::JumpMode]);
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
        assert_eq!(double_negative.eval(&state), true);
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

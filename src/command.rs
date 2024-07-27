use eframe::egui::KeyboardShortcut;
use pulldown_cmark::CowStr;
use smallvec::SmallVec;

use crate::{
    app_actions::AppAction, app_state::AppState, byte_span::ByteSpan,
    effects::text_change_effect::TextChange, text_structure::TextStructure,
};

#[derive(Debug, Clone, Copy)]
pub struct TextCommandContext<'a> {
    pub text_structure: &'a TextStructure,
    pub text: &'a str,
    pub byte_cursor: ByteSpan,
}

#[derive(Clone, Copy)]
pub struct CommandContext<'a> {
    pub app_state: &'a AppState,
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
    pub is_built_in: bool,
    pub name: String,
    pub shortcut: Option<KeyboardShortcut>,
    pub try_handle: Box<dyn Fn(CommandContext) -> EditorCommandOutput>,
}

impl EditorCommand {
    pub fn built_in<Handler: 'static + Fn(CommandContext) -> EditorCommandOutput>(
        name: impl Into<String>,
        shortcut: KeyboardShortcut,
        try_handle: Handler,
    ) -> Self {
        Self {
            is_built_in: true,
            name: name.into(),
            shortcut: Some(shortcut),
            try_handle: Box::new(try_handle),
        }
    }

    pub fn custom<Handler: 'static + Fn(CommandContext) -> EditorCommandOutput>(
        name: impl Into<String>,
        shortcut: KeyboardShortcut,
        try_handle: Handler,
    ) -> Self {
        Self {
            is_built_in: false,
            name: name.into(),
            shortcut: Some(shortcut),
            try_handle: Box::new(try_handle),
        }
    }
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

    pub fn find_by_name(&self, name: &str) -> Option<&EditorCommand> {
        self.slice().iter().find(|c| c.name == name)
    }

    pub fn retain_only(&mut self, filter: impl Fn(&EditorCommand) -> bool) {
        self.0.retain(filter)
    }

    pub fn add(&mut self, cmd: EditorCommand) {
        self.0.push(cmd)
    }

    pub fn set_or_replace_shortcut(
        &mut self,
        shortcut: KeyboardShortcut,
        cmd_name: &str,
    ) -> Option<()> {
        let cmd = self.0.iter_mut().find(|c| c.name == cmd_name)?;
        cmd.shortcut = Some(shortcut);
        Some(())
    }

    // autocomplete/convinience
    pub const EXPAND_TASK_MARKER: &'static str = "Expand Task Marker";
    pub const INDENT_LIST_ITEM: &'static str = "Increase List Item identation";
    pub const UNINDENT_LIST_ITEM: &'static str = "Decrease List Item identation";
    pub const SPLIT_LIST_ITEM: &'static str = "Split List item at cursor position";

    // markdown
    pub const MARKDOWN_BOLD: &'static str = "ToggleBold";
    pub const MARKDOWN_ITALIC: &'static str = "Toggle Italic";
    pub const MARKDOWN_STRIKETHROUGH: &'static str = "Toggle Strikethrough";
    pub const MARKDOWN_CODEBLOCK: &'static str = "Toggle Code Block";
    pub const MARKDOWN_H1: &'static str = "Heading 1";
    pub const MARKDOWN_H2: &'static str = "Heading 2";
    pub const MARKDOWN_H3: &'static str = "Heading 3";

    // others
    pub fn switch_to_note(note_index: u8) -> CowStr<'static> {
        match note_index {
            0 => "Shelf 1".into(),
            1 => "Shelf 2".into(),
            2 => "Shelf 3".into(),
            3 => "Shelf 4".into(),
            n => format!("Shelf {}", n + 1).into(),
        }
    }

    pub const OPEN_SETTIGS: &'static str = "Open Settings";

    pub const PIN_WINDOW: &'static str = "Pin/Unpin Window";

    pub const HIDE_WINDOW: &'static str = "Hide Window";
}

pub fn map_text_command_to_command_handler(
    f: impl Fn(TextCommandContext) -> Option<Vec<TextChange>> + 'static,
) -> Box<dyn Fn(CommandContext) -> EditorCommandOutput> {
    Box::new(move |CommandContext { app_state }| {
        let note = app_state.notes.get(&app_state.selected_note).unwrap();

        let Some(cursor) = note.cursor else {
            return SmallVec::new();
        };

        let Some(text_structure) = app_state.text_structure.as_ref() else {
            return SmallVec::new();
        };

        f(TextCommandContext::new(
            text_structure,
            &note.text,
            cursor.ordered(),
        ))
        .map(|changes| {
            SmallVec::from([AppAction::apply_text_changes(
                app_state.selected_note,
                changes,
            )])
        })
        .unwrap_or_default()
    })
}

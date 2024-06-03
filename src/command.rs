use eframe::egui::KeyboardShortcut;
use pulldown_cmark::CowStr;
use smallvec::SmallVec;

use crate::{
    app_actions::AppAction, app_state::AppState, byte_span::ByteSpan, text_structure::TextStructure,
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
    pub name: String,
    pub shortcut: Option<KeyboardShortcut>,
    pub try_handle: Box<dyn Fn(CommandContext) -> EditorCommandOutput>,
}

pub struct BuiltinCommands;
impl BuiltinCommands {
    // autocomplete/convinience
    pub const EXPAND_TASK_MARKER: &'static str = "Expand Task Marker";
    pub const INDENT_LIST_ITEM: &'static str = "Increase List Item identation";
    pub const UNINDENT_LIST_ITEM: &'static str = "Decrease List Item identation";
    pub const SPLIT_LIST_ITEM: &'static str = "Split List item at cursor position";

    // markdown
    pub const MARKDOWN_BOLD: &'static str = "Toggle Bold";
    pub const MARKDOWN_ITALIC: &'static str = "Toggle Italic";
    pub const MARKDOWN_STRIKETHROUGH: &'static str = "Toggle Strikethrough";
    pub const MARKDOWN_CODEBLOCK: &'static str = "Toggle Code Block";
    pub const MARKDOWN_H1: &'static str = "Heading 1";
    pub const MARKDOWN_H2: &'static str = "Heading 2";
    pub const MARKDOWN_H3: &'static str = "Heading 3";

    // others
    pub fn switch_to_note(note_index: u8) -> CowStr<'static> {
        match note_index {
            0 => "Shelv 1".into(),
            1 => "Shelv 2".into(),
            2 => "Shelv 3".into(),
            3 => "Shelv 4".into(),
            n => format!("Shelv {}", n + 1).into(),
        }
    }

    pub const INCREASE_FONT_SIZE: &'static str = "Increase Font Size";
    pub const DECREASE_FONT_SIZE: &'static str = "Decrease Font Size";
}

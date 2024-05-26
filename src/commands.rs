use eframe::egui::KeyboardShortcut;

use crate::{app_actions::TextChange, byte_span::ByteSpan, text_structure::TextStructure};

#[derive(Debug, Clone, Copy)]
pub struct EditorCommandContext<'a> {
    pub text_structure: &'a TextStructure,
    pub text: &'a str,
    pub byte_cursor: ByteSpan,
}

impl<'a> EditorCommandContext<'a> {
    pub fn new(text_structure: &'a TextStructure, text: &'a str, byte_cursor: ByteSpan) -> Self {
        Self {
            text_structure,
            text,
            byte_cursor,
        }
    }
}

pub struct EditorCommand {
    pub name: String,
    pub shortcut: Option<KeyboardShortcut>,
    pub try_handle: Box<dyn Fn(EditorCommandContext) -> Option<Vec<TextChange>>>,
}

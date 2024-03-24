use eframe::egui::KeyboardShortcut;

use crate::{app_actions::TextChange, byte_span::ByteSpan, text_structure::TextStructure};

pub trait EditorCommand {
    fn name(&self) -> &str;
    fn shortcut(&self) -> KeyboardShortcut;
    fn try_handle(
        &self,
        text_structure: &TextStructure,
        text: &str,
        byte_cursor: ByteSpan,
    ) -> Option<Vec<TextChange>>;
}

use eframe::egui::KeyboardShortcut;

use crate::{
    app_actions::TextChange,
    text_structure::{ByteRange, TextStructure},
};

pub trait EditorCommand {
    fn name(&self) -> &str;
    fn shortcut(&self) -> KeyboardShortcut;
    fn try_handle(
        &self,
        text_structure: &TextStructure,
        text: &str,
        byte_cursor: ByteRange,
    ) -> Option<Vec<TextChange>>;
}

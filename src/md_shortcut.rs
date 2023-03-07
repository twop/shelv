use eframe::egui::KeyboardShortcut;

pub struct MdAnnotationShortcut {
    name: &'static str,
    annotation: &'static str,
    shortcut: KeyboardShortcut,
}

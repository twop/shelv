// #![doc = include_str!("../README.md")]
use crate::egui::{
    vec2, Align2, Event, FontId, Id, Key, Modifiers, PointerButton, Response, Rounding, Sense, Ui,
    Widget,
};
use std::hash::Hash;

mod binding;
pub use binding::*;

/// The hotkey widget.
/// # Behavior
/// Generic `B` can be anything that implementes [`HotkeyBinding`].
/// By default [`HotkeyBinding`] is implemented for
/// * [`egui::Key`]
/// * [`egui::PointerButton`]
/// * [`binding::Binding`]
/// * [`binding::BindVariant`]
/// * `Option<HotkeyBinding>`
/// These types if not wrapped in `Option` will panic when tried to clear.
/// Clearing occurs if a Hotkey is waiting for new key and user pressed `ESC`.
pub struct Hotkey<'a, B>
where
    B: HotkeyBinding,
{
    binding: &'a mut B,
    id: Id,
    tooltip: Option<bool>,
}

impl<'a, B> Hotkey<'a, B>
where
    B: HotkeyBinding,
{
    /// Creates new hotkey.
    pub fn new(key: &'a mut B) -> Self {
        Self::with_id(key, "__hotkey")
    }

    /// Changes the default id for this widget.
    pub fn with_id(binding: &'a mut B, id_source: impl Hash) -> Self {
        Self {
            binding,
            id: Id::new(id_source),
            tooltip: None,
        }
    }

    /// If `true` widget will show full keybinding if hovered over.
    /// Default: `true`
    pub fn with_tooltip(mut self, show_tooltip: bool) -> Self {
        self.tooltip = Some(show_tooltip);
        self
    }
}

impl<'a, B> Widget for Hotkey<'a, B>
where
    B: HotkeyBinding,
{
    fn ui(self, ui: &mut Ui) -> Response {
        let size = ui.spacing().interact_size;
        let (rect, mut response) = ui.allocate_exact_size(size * vec2(1.5, 1.), Sense::click());

        let mut expecting = get_expecting(ui, self.id);

        if response.clicked() {
            expecting = !expecting;
        }

        if expecting {
            if response.clicked_elsewhere() {
                expecting = false;
            } else if ui.input(|input| input.key_pressed(Key::Escape)) {
                self.binding.clear();
                expecting = false;
            } else {
                let mut keyboard = ui.input(|input| {
                    input.events.iter().find_map(|e| match e {
                        Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => Some((BindVariant::Keyboard(*key), Some(*modifiers))),
                        _ => None,
                    })
                });
                if !B::ACCEPT_KEYBOARD {
                    keyboard = None;
                }

                let mut mouse = ui.input(|input| {
                    input.events.iter().find_map(|e| match e {
                        Event::PointerButton {
                            button,
                            pressed: true,
                            modifiers,
                            ..
                        } if *button != PointerButton::Primary
                            && *button != PointerButton::Secondary =>
                        {
                            Some((BindVariant::Mouse(*button), Some(*modifiers)))
                        }
                        _ => None,
                    })
                });
                if !B::ACCEPT_MOUSE {
                    mouse = None
                }

                if let Some((key, mods)) = keyboard.or(mouse) {
                    self.binding.set(key, mods.unwrap_or(Modifiers::NONE));
                    response.mark_changed();
                    expecting = false;
                }
            }
        } else if let Some(bind) = self.binding.get() {
            if self.tooltip.unwrap_or(true) {
                response = response.on_hover_text(bind.into_long());
            }
        }

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, expecting);
            ui.painter()
                .rect_filled(rect, Rounding::same(2.), visuals.bg_fill);

            let binding = self.binding.get();

            let text = binding
                .map(|hk| hk.into_short())
                .unwrap_or_else(|| "None".into());

            ui.painter().text(
                rect.center() + vec2(0., 1.),
                Align2::CENTER_CENTER,
                text,
                FontId::default(),
                visuals.text_color(),
            );
        }

        set_expecting(ui, self.id, expecting);
        response
    }
}

fn get_expecting(ui: &Ui, id: Id) -> bool {
    ui.ctx().memory_mut(|memory| {
        let expecting = memory
            .data
            .get_temp_mut_or_default::<bool>(ui.make_persistent_id(id));
        *expecting
    })
}

fn set_expecting(ui: &Ui, id: Id, new: bool) {
    ui.ctx().memory_mut(|memory| {
        *memory
            .data
            .get_temp_mut_or_default::<bool>(ui.make_persistent_id(id)) = new;
    });
}

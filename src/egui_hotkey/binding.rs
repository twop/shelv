use crate::egui::{Event, InputState, Key, Modifiers, PointerButton};
use std::{
    fmt::{Debug, Display, Formatter, Result},
    ops::Deref,
};

/// Variant for binding. This can be either [`egui::PointerButton`] or [`egui::Key`].
/// # As [`HotkeyBinding`]
/// Will panic if cleared. To avoid this consider using `Option<BindVariant>`
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindVariant {
    Mouse(PointerButton),
    Keyboard(Key),
}

impl BindVariant {
    /// Returns true if the variant was pressed.
    pub fn pressed(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match self {
            BindVariant::Mouse(mb) => input_state.events.iter().any(|e| {
                matches!(e, Event::PointerButton {
                    button,
                    pressed: true,
                    ..
                } if mb == button)
            }),
            BindVariant::Keyboard(kb) => input_state.key_pressed(*kb),
        }
    }

    /// Returns true if the variant was released.
    pub fn released(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match self {
            BindVariant::Mouse(mb) => {
                input_state.events.iter().any(|e| {
                    matches!(e, Event::PointerButton {
                    button,
                    pressed: false,
                    ..
                } if mb == button)
                });
                false
            }
            BindVariant::Keyboard(kb) => input_state.key_released(*kb),
        }
    }

    /// Returns true if the variant is down.
    pub fn down(&self, input_state: impl Deref<Target = InputState>) -> bool {
        match self {
            BindVariant::Mouse(mb) => input_state.pointer.button_down(*mb),
            BindVariant::Keyboard(kb) => input_state.key_down(*kb),
        }
    }
}

impl From<PointerButton> for BindVariant {
    fn from(pb: PointerButton) -> Self {
        Self::Mouse(pb)
    }
}

impl From<Key> for BindVariant {
    fn from(key: Key) -> Self {
        Self::Keyboard(key)
    }
}

impl Display for BindVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            BindVariant::Mouse(pb) => f.write_str(match pb {
                PointerButton::Primary => "M1",
                PointerButton::Secondary => "M2",
                PointerButton::Middle => "M3",
                PointerButton::Extra1 => "M4",
                PointerButton::Extra2 => "M5",
            }),
            BindVariant::Keyboard(kb) => write!(f, "{:?}", kb),
        }
    }
}

/// Binding to a variant that also stores the [`egui::Modifiers`].
/// # As [`HotkeyBinding`]
/// Will panic if cleared. To avoid this consider using `Option<Binding>`
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Binding {
    pub variant: BindVariant,
    pub modifiers: Modifiers,
}

impl Binding {
    pub fn into_short(self) -> String {
        let mut items = Vec::new();
        if self.modifiers.command {
            items.push("^");
        }
        if self.modifiers.alt {
            items.push("¬");
        }
        if self.modifiers.shift {
            items.push("⬆");
        }
        if self.modifiers.mac_cmd {
            items.push("⌘");
        }

        let merged = items.join("");
        if merged.is_empty() {
            self.variant.to_string()
        } else {
            format!("{merged}{}", self.variant)
        }
    }

    pub fn into_long(self) -> String {
        let mut items = Vec::new();
        if self.modifiers.ctrl {
            items.push("Ctrl");
        }
        if self.modifiers.alt {
            items.push("Alt");
        }
        if self.modifiers.shift {
            items.push("Shift");
        }
        if self.modifiers.mac_cmd {
            items.push("Cmd");
        }

        let merged = items.join(" + ");
        if merged.is_empty() {
            self.variant.to_string()
        } else {
            format!("{merged} + {}", self.variant)
        }
    }

    /// Returns true if the variant was pressed and input modifiers are matching.
    pub fn pressed(&self, input_state: impl Deref<Target = InputState>) -> bool {
        input_state.modifiers.matches(self.modifiers) && self.variant.pressed(input_state)
    }

    /// Returns true if the variant was released and input modifiers are matching.
    pub fn released(&self, input_state: impl Deref<Target = InputState>) -> bool {
        input_state.modifiers.matches(self.modifiers) && self.variant.released(input_state)
    }

    /// Returns true if the variant is down and input modifiers are matching.
    pub fn down(&self, input_state: impl Deref<Target = InputState>) -> bool {
        input_state.modifiers.matches(self.modifiers) && self.variant.down(input_state)
    }
}

/// This Trait defines types that can be used as hotkey's target.
pub trait HotkeyBinding {
    const ACCEPT_MOUSE: bool;
    const ACCEPT_KEYBOARD: bool;

    fn new(variant: BindVariant, modifiers: Modifiers) -> Self;
    fn get(&self) -> Option<Binding>;
    fn set(&mut self, variant: BindVariant, modifiers: Modifiers);
    fn clear(&mut self);
}

impl HotkeyBinding for Key {
    const ACCEPT_MOUSE: bool = false;
    const ACCEPT_KEYBOARD: bool = true;

    fn get(&self) -> Option<Binding> {
        Some(Binding {
            variant: BindVariant::Keyboard(*self),
            modifiers: Modifiers::NONE,
        })
    }

    fn set(&mut self, key: BindVariant, _: Modifiers) {
        match key {
            BindVariant::Keyboard(kb) => *self = kb,
            BindVariant::Mouse(_) => unreachable!(),
        }
    }

    fn clear(&mut self) {
        panic!("Cannot clear the `Key`. Consider using: `Option<Key>`");
    }

    fn new(variant: BindVariant, _: Modifiers) -> Self {
        match variant {
            BindVariant::Keyboard(kb) => kb,
            BindVariant::Mouse(_) => unreachable!(),
        }
    }
}

impl HotkeyBinding for Binding {
    const ACCEPT_MOUSE: bool = true;
    const ACCEPT_KEYBOARD: bool = true;

    fn get(&self) -> Option<Binding> {
        Some(*self)
    }

    fn set(&mut self, variant: BindVariant, modifiers: Modifiers) {
        self.variant = variant;
        self.modifiers = modifiers;
    }

    fn clear(&mut self) {
        panic!("Cannot clear the `Binding`. Consider using: `Option<Binding>`");
    }

    fn new(variant: BindVariant, modifiers: Modifiers) -> Self {
        Binding { variant, modifiers }
    }
}

impl HotkeyBinding for BindVariant {
    const ACCEPT_MOUSE: bool = true;
    const ACCEPT_KEYBOARD: bool = true;

    fn get(&self) -> Option<Binding> {
        Some(Binding {
            variant: *self,
            modifiers: Modifiers::NONE,
        })
    }

    fn set(&mut self, variant: BindVariant, _: Modifiers) {
        *self = variant;
    }

    fn clear(&mut self) {
        panic!("Cannot clear the `BindVariant`. Consider using: `Option<BindVariant>`");
    }

    fn new(variant: BindVariant, _: Modifiers) -> Self {
        variant
    }
}

impl HotkeyBinding for PointerButton {
    const ACCEPT_MOUSE: bool = true;
    const ACCEPT_KEYBOARD: bool = false;

    fn get(&self) -> Option<Binding> {
        Some(Binding {
            variant: BindVariant::Mouse(*self),
            modifiers: Modifiers::NONE,
        })
    }

    fn set(&mut self, variant: BindVariant, _: Modifiers) {
        match variant {
            BindVariant::Mouse(mb) => *self = mb,
            BindVariant::Keyboard(_) => unreachable!(),
        }
    }

    fn clear(&mut self) {
        panic!("Cannot clear the `PointerButton`. Consider using: `Option<BindVariant>`");
    }

    fn new(variant: BindVariant, _: Modifiers) -> Self {
        match variant {
            BindVariant::Mouse(mb) => mb,
            BindVariant::Keyboard(_) => unreachable!(),
        }
    }
}

impl<B> HotkeyBinding for Option<B>
where
    B: HotkeyBinding,
{
    const ACCEPT_MOUSE: bool = B::ACCEPT_MOUSE;
    const ACCEPT_KEYBOARD: bool = B::ACCEPT_KEYBOARD;

    fn get(&self) -> Option<Binding> {
        self.as_ref()?.get()
    }

    fn set(&mut self, variant: BindVariant, modifiers: Modifiers) {
        if let Some(this) = self {
            this.set(variant, modifiers);
        } else {
            *self = Self::new(variant, modifiers);
        }
    }

    fn clear(&mut self) {
        *self = None;
    }

    fn new(variant: BindVariant, modifiers: Modifiers) -> Self {
        Some(B::new(variant, modifiers))
    }
}

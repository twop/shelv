//! Keyboard shortcuts as written in settings (`"Cmd Shift B"`) and as shown to users.

use eframe::egui::{Key, KeyboardShortcut, ModifierNames, Modifiers};

fn try_parse_modifier(mod_str: &str) -> Option<Modifiers> {
    match mod_str {
        s if s == ModifierNames::NAMES.alt => Some(Modifiers::ALT),
        s if s == ModifierNames::NAMES.ctrl => Some(Modifiers::CTRL),
        s if s == ModifierNames::NAMES.mac_cmd => Some(Modifiers::MAC_CMD),
        s if s == ModifierNames::NAMES.mac_alt => Some(Modifiers::ALT),
        s if s == ModifierNames::NAMES.shift => Some(Modifiers::SHIFT),
        _ => None,
    }
}

/// `"Cmd Shift B"`: space-separated modifiers (`Cmd`, `Ctrl`, `Option`/`Alt`, `Shift`), then exactly
/// one key (egui key names: `A`, `1`, `Enter`, `Comma`, `/`, ...).
pub fn parse_keyboard_shortcut(attr: &str) -> Result<KeyboardShortcut, String> {
    let parts: Vec<_> = attr.split(' ').filter(|part| !part.is_empty()).collect();

    let modifiers = parts
        .iter()
        .flat_map(|s| try_parse_modifier(s))
        .fold(Modifiers::NONE, |modifiers, modifier| modifiers | modifier);

    let keys: Vec<_> = parts
        .iter()
        .filter(|k| try_parse_modifier(k).is_none())
        .collect();

    let [key] = keys.as_slice() else {
        return Err(if keys.is_empty() {
            "no key, only modifiers".to_string()
        } else {
            format!(
                "exactly one key expected after the modifiers (Cmd, Ctrl, Option, Shift), got {}",
                keys.iter().map(|k| format!("\"{k}\"")).collect::<Vec<_>>().join(", ")
            )
        });
    };

    let Some(key) = Key::from_name(key) else {
        return Err(format!(
            "\"{key}\" is not a key or modifier (keys: A-Z, 0-9, Enter, Space, Tab, Escape, Comma, Slash, F1, ...)"
        ));
    };

    Ok(KeyboardShortcut::new(modifiers, key))
}

pub fn format_mac_shortcut_with_symbols(shortcut: KeyboardShortcut) -> String {
    const SPACED_NAMES: ModifierNames = ModifierNames {
        concat: " ",
        ..ModifierNames::SYMBOLS
    };

    shortcut.format(&SPACED_NAMES, true)
}

pub fn format_mac_shortcut_with_names(shortcut: KeyboardShortcut) -> String {
    const SPACED_NAMES: ModifierNames = ModifierNames {
        concat: " ",
        ..ModifierNames::NAMES
    };

    shortcut.format(&SPACED_NAMES, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats() {
        let shortcut = parse_keyboard_shortcut("Cmd Shift B").unwrap();
        assert_eq!(shortcut, KeyboardShortcut::new(Modifiers::MAC_CMD | Modifiers::SHIFT, Key::B));
        assert_eq!(format_mac_shortcut_with_names(shortcut), "Shift Cmd B");
        assert_eq!(
            parse_keyboard_shortcut("Option /").unwrap(),
            KeyboardShortcut::new(Modifiers::ALT, Key::Slash)
        );
    }

    #[test]
    fn explains_errors() {
        assert!(parse_keyboard_shortcut("Cmd Shift").unwrap_err().contains("no key"));
        let two = parse_keyboard_shortcut("Cmd Shit B").unwrap_err();
        assert!(two.contains("\"Shit\", \"B\""), "{two}");
        let unknown = parse_keyboard_shortcut("Cmd Bee").unwrap_err();
        assert!(unknown.starts_with("\"Bee\" is not a key"), "{unknown}");
    }
}

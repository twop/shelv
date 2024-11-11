use eframe::egui::{Key, KeyboardShortcut, ModifierNames, Modifiers};
use knus::{ast::Literal, errors::DecodeError, span::Spanned, traits::ErrorSpan, DecodeScalar};
use smallvec::SmallVec;

use crate::command::CommandInstruction;

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedShortcut(KeyboardShortcut);
impl ParsedShortcut {
    pub fn value(&self) -> KeyboardShortcut {
        self.0
    }
}

impl<S: ErrorSpan> DecodeScalar<S> for ParsedShortcut {
    fn raw_decode(
        val: &Spanned<Literal, S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<ParsedShortcut, DecodeError<S>> {
        match &**val {
            Literal::String(ref s) => parse_keyboard_shortcut(s)
                .map_err(|err| DecodeError::conversion(val, err))
                .map(ParsedShortcut),
            _ => {
                ctx.emit_error(DecodeError::scalar_kind(knus::decode::Kind::String, val));
                Ok(ParsedShortcut(KeyboardShortcut::new(
                    Modifiers::NONE,
                    Key::Escape,
                )))
            }
        }
    }
    fn type_check(
        type_name: &Option<Spanned<knus::ast::TypeName, S>>,
        ctx: &mut knus::decode::Context<S>,
    ) {
        if let Some(typ) = type_name {
            ctx.emit_error(DecodeError::TypeName {
                span: typ.span().clone(),
                found: None,
                expected: knus::errors::ExpectedType::no_type(),
                rust_type: "ParsedShortcut",
            });
        }
    }
}

#[derive(Debug, knus::Decode, Clone, PartialEq, Eq)]
pub struct LlmSettings {
    #[knus(child(name = "model"), unwrap(argument))]
    pub model: String,

    #[knus(child(name = "systemPrompt"), unwrap(argument))]
    pub system_prompt: Option<String>,

    #[knus(child(name = "useShelvSystemPrompt"), unwrap(argument), default = true)]
    pub use_shelv_system_prompt: bool,
}

#[derive(Debug, knus::Decode, PartialEq)]
pub struct TopLevelKdlSettings {
    #[knus(children(name = "bind"))]
    pub bindings: Vec<LocalBinding>,

    #[knus(children(name = "global"))]
    pub global_bindings: Vec<GlobalBinding>,

    #[knus(child(name = "ai"))]
    pub llm_settings: Option<LlmSettings>,
}

#[derive(Debug, knus::Decode, PartialEq)]
pub enum GlobalCommand {
    #[knus(name = "ShowHideApp")]
    ShowHideApp,
}

#[derive(Debug, knus::Decode, PartialEq)]
pub struct LocalBinding {
    #[knus(argument)]
    pub shortcut: Option<ParsedShortcut>,

    #[knus(children)]
    pub instructions: SmallVec<[CommandInstruction; 1]>,

    #[knus(property(name = "alias"))]
    pub slash_alias: Option<String>,

    #[knus(property(name = "icon"))]
    pub phosphor_icon: Option<String>,

    #[knus(property(name = "description"))]
    pub description: Option<String>,
}

#[derive(Debug, knus::Decode, PartialEq)]
pub struct GlobalBinding {
    #[knus(argument)]
    pub shortcut: ParsedShortcut,

    #[knus(children)]
    pub global_commands: SmallVec<[GlobalCommand; 1]>,
}

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

fn parse_keyboard_shortcut(attr: &str) -> Result<KeyboardShortcut, String> {
    let parts: Vec<_> = attr.split(' ').collect();

    let modifiers = parts
        .iter()
        .flat_map(|s| try_parse_modifier(*s))
        .fold(Modifiers::NONE, |modifiers, modifier| modifiers | modifier);

    let non_modifiers: Vec<_> = parts
        .iter()
        .filter(|k| try_parse_modifier(k).is_none())
        .collect();

    if non_modifiers.len() != 1 {
        return Err("There has to be exactly one keyboard key".to_string());
    }

    let key = non_modifiers[0];
    let Some(key) = Key::from_name(key) else {
        return Err(format!(
            "{key} is not a valid keyboard key or modifier look at TODO url for the complete list"
        ));
    };

    Ok(KeyboardShortcut::new(modifiers, key))
}

pub fn parse_top_level_settings_block(block_str: &str) -> Result<TopLevelKdlSettings, knus::Error> {
    knus::parse::<TopLevelKdlSettings>("block", block_str)
}

pub fn format_mac_shortcut(shortcut: KeyboardShortcut) -> String {
    const SPACED_NAMES: ModifierNames = ModifierNames {
        concat: " ",
        ..ModifierNames::SYMBOLS
    };

    shortcut.format(&SPACED_NAMES, true)
}

#[cfg(test)]
mod tests {

    use crate::command::{ForwardToChild, ScriptCall, TextSource};

    use super::*;

    #[test]
    pub fn test_bind_predefined_cmd_parsing() {
        let doc_str = r#"
        bind "Cmd A" { HideApp;}
        "#;

        let keybindings = parse_top_level_settings_block(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [LocalBinding {
                    shortcut: Some(ParsedShortcut(KeyboardShortcut::new(
                        Modifiers::MAC_CMD,
                        Key::A
                    ))),
                    instructions: [CommandInstruction::HideApp].into(),
                    slash_alias: None,
                    description: None,
                    phosphor_icon: None
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: None
            }
        );
    }

    #[test]
    pub fn test_insert_text_cmd_parsing() {
        let doc_str = r#"
        bind "Cmd J" alias="some_alias" icon="some icon" description="some description" {
            InsertText {
                string "something else"
            }
        }
        "#;

        let keybindings = parse_top_level_settings_block(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [LocalBinding {
                    shortcut: Some(ParsedShortcut(KeyboardShortcut::new(
                        Modifiers::MAC_CMD,
                        Key::J
                    ))),
                    instructions: [CommandInstruction::InsertText(ForwardToChild(
                        TextSource::Str("something else".to_string())
                    ))]
                    .into(),
                    slash_alias: Some("some_alias".to_string()),
                    description: Some("some description".to_string()),
                    phosphor_icon: Some("some icon".to_string())
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: None
            }
        );
    }

    #[test]
    pub fn test_insert_text_cmd_parsing_with_script() {
        let doc_str = r#"
        bind "Cmd K" {
            InsertText {
                script "my_script_function"
            }
        }
        "#;

        let keybindings = parse_top_level_settings_block(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [LocalBinding {
                    shortcut: Some(ParsedShortcut(KeyboardShortcut::new(
                        Modifiers::MAC_CMD,
                        Key::K
                    ))),
                    instructions: [CommandInstruction::InsertText(ForwardToChild(
                        TextSource::Script(ScriptCall {
                            func_name: "my_script_function".to_string()
                        })
                    ))]
                    .into(),
                    slash_alias: None,
                    description: None,
                    phosphor_icon: None
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: None
            }
        );
    }
}

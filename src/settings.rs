use std::str::FromStr;

use eframe::egui::{Key, KeyboardShortcut, ModifierNames, Modifiers};
use itertools::Itertools;
use kdl::{KdlDocument, KdlError, KdlNode};
use miette::SourceSpan;
use smallvec::SmallVec;

use crate::text_structure::{SpanKind, TextStructure};

#[derive(Debug, PartialEq, Eq)]
pub struct TopLevelKdlSettings {
    pub bindings: Vec<Binding>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Binding {
    pub shortcut: KeyboardShortcut,
    pub command: String,
}

fn try_parse_modifier(mod_str: &str) -> Option<Modifiers> {
    match mod_str {
        s if s == ModifierNames::NAMES.alt => Some(Modifiers::ALT),
        s if s == ModifierNames::NAMES.ctrl => Some(Modifiers::CTRL),
        s if s == ModifierNames::NAMES.mac_cmd => Some(Modifiers::MAC_CMD),
        s if s == ModifierNames::NAMES.mac_alt => Some(Modifiers::ALT),
        _ => None,
    }
}

#[derive(Debug)]
pub enum SettingsParseError {
    UnexpectedNode(SourceSpan, &'static str),
    MismatchedArgsCoung(SourceSpan, usize),
    MismatchedType {
        span: SourceSpan,
        expected: &'static str,
    },
    CouldntParseShortCut(SourceSpan, String),
    MismatchedChildren(SourceSpan, String),
    CoulndntParseCommand(SourceSpan, String),
    ParseKdlErro(KdlError),
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
        return Err(format!("There has to be exectly one keyboard key"));
    }

    let key = non_modifiers[0];
    let Some(key) = Key::from_name(key) else {
        return Err(format!(
            "{key} is not a valid keyboard key or modifier look at TODO url for the complete list"
        ));
    };

    Ok(KeyboardShortcut::new(modifiers, key))
}

fn parse_command(node: &KdlNode) -> Result<String, String> {
    Ok(node.name().value().to_string())
}

fn parse_binding_node(node: &KdlNode) -> Result<Binding, SettingsParseError> {
    if node.entries().len() != 1 {
        return Err(SettingsParseError::MismatchedArgsCoung(
            node.name().span().clone(),
            1,
        ));
    }

    let kdl_entry = &node.entries()[0];
    let Some(str_attr) = kdl_entry.value().as_string() else {
        return Err(SettingsParseError::MismatchedType {
            span: node.name().span().clone(),
            expected: "string",
        });
    };

    let shortcut = parse_keyboard_shortcut(str_attr)
        .map_err(|err| SettingsParseError::CouldntParseShortCut(kdl_entry.span().clone(), err))?;

    let Some(children) = node.children() else {
        return Err(SettingsParseError::MismatchedChildren(
            node.span().clone(),
            r#"Needs to have exactly one command, like '{ DoSomething;}'"#.to_string(),
        ));
    };

    if children.nodes().len() != 1 {
        return Err(SettingsParseError::MismatchedChildren(
            children.span().clone(),
            r#"Needs to have exactly one command, like '{ DoSomething;}'"#.to_string(),
        ));
    }

    let command_node = &children.nodes()[0];
    let command = parse_command(&command_node).map_err(|err| {
        SettingsParseError::CoulndntParseCommand(command_node.span().clone(), err)
    })?;

    Ok(Binding { shortcut, command })
}

pub fn parse_top_level(block_str: &str) -> Result<TopLevelKdlSettings, SettingsParseError> {
    let doc = KdlDocument::from_str(block_str).map_err(SettingsParseError::ParseKdlErro)?;

    let bindings: Result<Vec<_>, SettingsParseError> = doc
        .nodes()
        .iter()
        .map(|node| match node.name().value() {
            "bind" => parse_binding_node(node),
            _ => Err(SettingsParseError::UnexpectedNode(
                node.name().span().clone(),
                "bind",
            )),
        })
        .collect();

    Ok(TopLevelKdlSettings {
        bindings: bindings?,
    })
}

pub fn execute_settings_note(
    text_structure: &TextStructure,
    text: &str,
) -> Result<TopLevelKdlSettings, SettingsParseError> {
    let script_blocks: SmallVec<[_; 8]> = text_structure
        .iter()
        .filter_map(|(index, desc)| match desc.kind {
            SpanKind::CodeBlock => text_structure.find_meta(index).and_then(|meta| match meta {
                crate::text_structure::SpanMeta::CodeBlock { lang } => {
                    // let byte_range = desc.byte_pos.clone();

                    let (_, code_desc) = text_structure
                        .iterate_immediate_children_of(index)
                        .find(|(_, desc)| desc.kind == SpanKind::Text)?;

                    let code = &text[code_desc.byte_pos.range()];

                    match lang.as_str() {
                        "settings" => Some(code),

                        _ => None,
                    }
                }

                _ => None,
            }),
            _ => None,
        })
        .collect();

    let block_count = script_blocks.len();
    let all_found_bindings = script_blocks
        .into_iter()
        .map(parse_top_level)
        .map_ok(|doc| doc.bindings)
        .fold_ok(vec![], |mut all, bindings| {
            all.extend(bindings);
            all
        })?;
    let bindings_count = all_found_bindings.len();
    println!("processing settings block, found {block_count} blocks with {bindings_count} bindigns total" );

    Ok(TopLevelKdlSettings {
        bindings: all_found_bindings,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn test_settings_parsing() {
        let doc_str = r#"
        bind "Cmd A" { Test;}
        "#;

        let keybindings = parse_top_level(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [Binding {
                    shortcut: KeyboardShortcut::new(Modifiers::MAC_CMD, Key::A),
                    command: "Test".to_string()
                }]
                .into()
            }
        );
    }
}

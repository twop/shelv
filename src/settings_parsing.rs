use std::{fmt::format, str::FromStr, time::Instant};

use eframe::egui::{util::undoer::Settings, Key, KeyboardShortcut, ModifierNames, Modifiers};
use itertools::{Either, Itertools};
use kdl::{KdlDocument, KdlEntry, KdlError, KdlNode, KdlValue};
use miette::SourceSpan;
use smallvec::SmallVec;

use crate::{
    app_actions::AppIO,
    app_state::MsgToApp,
    command::{
        map_text_command_to_command_handler, BuiltInCommand, CommandList, EditorCommand,
        TextCommandContext, PROMOTED_COMMANDS,
    },
    effects::text_change_effect::TextChange,
    scripting::{execute_code_blocks, BlockEvalResult, CodeBlockKind, NoteEvalContext, SourceHash},
    text_structure::{SpanKind, TextStructure},
};

#[derive(Debug, PartialEq, Eq)]
pub struct LlmSettings {
    pub model: String,
    pub system_prompt: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TopLevelKdlSettings {
    pub bindings: Vec<LocalBinding>,
    pub global_bindings: Vec<GlobalBinding>,
    pub llm_settings: Vec<LlmSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertTextTarget {
    Selection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptCall {
    pub func_name: String,
}

impl ScriptCall {
    pub fn new(func_name: String) -> Self {
        Self { func_name }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSource {
    Inline(String),
    Script(ScriptCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCmdInsertText {
    pub target: InsertTextTarget,
    pub text: TextSource,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParsedCommand {
    Predefined(BuiltInCommand),
    InsertText(ParsedCmdInsertText),
}

#[derive(Debug, PartialEq, Eq)]
pub enum GlobalCommand {
    ShowHideApp,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LocalBinding {
    pub shortcut: KeyboardShortcut,
    pub command: ParsedCommand,
    pub slash_alias: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct GlobalBinding {
    pub shortcut: KeyboardShortcut,
    pub command: GlobalCommand,
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

#[derive(Debug)]
pub enum SettingsParseError {
    UnexpectedNode(SourceSpan, &'static str),
    MismatchedArgsCount(SourceSpan, usize),
    MismatchedType {
        span: SourceSpan,
        expected: &'static str,
    },
    CouldntParseShortCut(SourceSpan, String),
    MismatchedChildren(SourceSpan, String),
    CoulndntParseCommand(SourceSpan, String),
    MissingNode {
        span: SourceSpan,
        node: String,
    },
    ParseKdlErro(KdlError),
    UnknownCommand(String),
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

fn parse_command(node: &KdlNode) -> Result<ParsedCommand, SettingsParseError> {
    match node.name().value() {
        "InsertText" => parse_replace_text_command(node).map(ParsedCommand::InsertText),
        name => match try_parse_builtin_command(name, node.entries()) {
            Some(cmd) => Ok(ParsedCommand::Predefined(cmd)),
            None => Err(SettingsParseError::UnknownCommand(node.name().to_string())),
        },
    }
}

fn try_parse_builtin_command(name: &str, entries: &[KdlEntry]) -> Option<BuiltInCommand> {
    use BuiltInCommand as B;

    match name {
        name if name == B::ExpandTaskMarker.name() => Some(B::ExpandTaskMarker),
        name if name == B::IndentListItem.name() => Some(B::IndentListItem),
        name if name == B::UnindentListItem.name() => Some(B::UnindentListItem),
        name if name == B::SplitListItem.name() => Some(B::SplitListItem),
        name if name == B::MarkdownBold.name() => Some(B::MarkdownBold),
        name if name == B::MarkdownItalic.name() => Some(B::MarkdownItalic),
        name if name == B::MarkdownStrikethrough.name() => Some(B::MarkdownStrikethrough),
        name if name == B::MarkdownCodeBlock.name() => Some(B::MarkdownCodeBlock),
        name if name == B::MarkdownH1.name() => Some(B::MarkdownH1),
        name if name == B::MarkdownH2.name() => Some(B::MarkdownH2),
        name if name == B::MarkdownH3.name() => Some(B::MarkdownH3),
        name if name == B::SwitchToNote(0).name() => match entries {
            [entry] => match entry.value() {
                KdlValue::Base10(note) if *note > 0 => Some(B::SwitchToNote((note - 1) as u8)),
                _ => None,
            },
            _ => None, // TODO wrong number of arguments
        },
        name if name == B::SwitchToSettings.name() => Some(B::SwitchToSettings),
        name if name == B::PinWindow.name() => Some(B::PinWindow),
        name if name == B::HideApp.name() => Some(B::HideApp),

        _ => None,
    }
}

fn parse_global_command(node: &KdlNode) -> Result<GlobalCommand, SettingsParseError> {
    match node.name().value() {
        "ToggleAppVisibility" | "ShowHideApp" => Ok(GlobalCommand::ShowHideApp),
        name => Err(SettingsParseError::UnknownCommand(name.to_string())),
    }
}

fn parse_replace_text_command(node: &KdlNode) -> Result<ParsedCmdInsertText, SettingsParseError> {
    use SettingsParseError as PE;
    if node.entries().len() > 0 {
        Err(PE::MismatchedArgsCount(node.span().clone(), 0))
    } else {
        let children = node.children().ok_or_else(|| {
            PE::MismatchedChildren(
                node.span().clone(),
                r#"InsertText needs to have 'target' and 'text' nodes
For example:
InsertText {
    target "selection"
    text "this is before {{selection}} and this is after"
}
"#
                .to_string(),
            )
        })?;

        let target = children
            .get("target")
            .map(parse_replace_text_target)
            .unwrap_or(Ok(InsertTextTarget::Selection))?;

        let text = children
            .get("text")
            .ok_or_else(|| PE::MissingNode {
                span: children.span().clone(),
                node: "text".to_string(),
            })
            .and_then(parse_text_source)?;

        Ok(ParsedCmdInsertText { target, text })
    }
}

fn parse_text_source(source_node: &KdlNode) -> Result<TextSource, SettingsParseError> {
    use SettingsParseError as PE;

    match source_node.entries() {
        [entry] => {
            // Parse inline text
            match &entry.value() {
                KdlValue::String(s) | KdlValue::RawString(s) => Ok(TextSource::Inline(s.clone())),
                _ => Err(PE::MismatchedType {
                    span: entry.span().clone(),
                    expected: "String",
                }),
            }
        }
        [] => {
            // Parse script call
            let children = source_node.children().ok_or_else(|| {
                PE::MismatchedChildren(
                    source_node.span().clone(),
                    "Expected either a string or a 'call' node".to_string(),
                )
            })?;

            let call_node = children.get("call").ok_or_else(|| PE::MissingNode {
                span: children.span().clone(),
                node: "call".to_string(),
            })?;

            match call_node.entries() {
                [entry] => match &entry.value() {
                    KdlValue::String(s) | KdlValue::RawString(s) => {
                        Ok(TextSource::Script(ScriptCall::new(s.clone())))
                    }

                    _ => Err(PE::MismatchedType {
                        span: entry.span().clone(),
                        expected: "String",
                    }),
                },
                _ => Err(PE::MismatchedArgsCount(call_node.span().clone(), 1)),
            }
        }
        _ => Err(PE::MismatchedArgsCount(source_node.span().clone(), 1)),
    }
}

fn parse_replace_text_target(target: &KdlNode) -> Result<InsertTextTarget, SettingsParseError> {
    use SettingsParseError as PE;

    if target.entries().len() != 1 {
        return Err(PE::MismatchedArgsCount(target.span().clone(), 1));
    }

    let target_entry = &target.entries()[0];
    if target_entry.name().is_some() {
        return Err(PE::CoulndntParseCommand(
            target_entry.span().clone(),
            r#"'target' accept a single unnamed string that can only be "selection""#.to_string(),
        ));
    }
    let target = match target_entry.value() {
        kdl::KdlValue::RawString(s) | kdl::KdlValue::String(s) => s.as_str(),
        _ => {
            return Err(PE::MismatchedType {
                span: target_entry.span().clone(),
                expected: "String",
            })
        }
    };

    if target != "selection" {
        return Err(PE::CoulndntParseCommand(
            target_entry.span().clone(),
            r#"only "selection" is supported for 'target'"#.to_string(),
        ));
    }

    Ok(InsertTextTarget::Selection)
}

fn parse_replace_text_with(node: &KdlNode) -> Result<String, SettingsParseError> {
    use SettingsParseError as PE;

    if node.entries().len() != 1 {
        return Err(PE::MismatchedArgsCount(node.span().clone(), 1));
    }

    let entry = &node.entries()[0];
    if entry.name().is_some() {
        return Err(PE::CoulndntParseCommand(
            entry.span().clone(),
            r#"'with' accept a single unnamed string"#.to_string(),
        ));
    }

    match entry.value() {
        kdl::KdlValue::RawString(s) | kdl::KdlValue::String(s) => Ok(s.clone()),
        _ => Err(PE::MismatchedType {
            span: entry.span().clone(),
            expected: "String",
        }),
    }
}

fn parse_binding<Cmd>(
    node: &KdlNode,
    parse: impl Fn(&KdlNode) -> Result<Cmd, SettingsParseError>,
) -> Result<(KeyboardShortcut, Option<String>, Option<String>, Cmd), SettingsParseError> {
    if !(1..4).contains(&node.entries().len()) {
        return Err(SettingsParseError::MismatchedArgsCount(
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

    // Try parsing alias
    let alias = node.entries().iter().find_map(|entry| {
        if entry.name().map_or(false, |name| name.value() == "alias") {
            entry.value().as_string().map(String::from)
        } else {
            None
        }
    });

    // Try parsing description
    let description = node.entries().iter().find_map(|entry| {
        if entry
            .name()
            .map_or(false, |name| name.value() == "description")
        {
            entry.value().as_string().map(String::from)
        } else {
            None
        }
    });

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
    let command = parse(&command_node)?;

    // .map_err(|err| {
    //     SettingsParseError::CoulndntParseCommand(command_node.span().clone(), err)
    // })?;

    Ok((shortcut, alias, description, command))
}

pub fn parse_top_level_settings_block(
    block_str: &str,
) -> Result<TopLevelKdlSettings, SettingsParseError> {
    let doc = KdlDocument::from_str(block_str).map_err(SettingsParseError::ParseKdlErro)?;

    let bindings: Result<Vec<LocalBinding>, SettingsParseError> = doc
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "bind")
        .map(|node| {
            parse_binding(node, parse_command).map(
                |(shortcut, slash_alias, description, command)| LocalBinding {
                    shortcut,
                    slash_alias,
                    command,
                    description,
                },
            )
        })
        .collect();

    let global_bindings: Result<Vec<GlobalBinding>, SettingsParseError> = doc
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "global")
        .map(|node| {
            parse_binding(node, parse_global_command)
                .map(|(shortcut, _, _, command)| GlobalBinding { shortcut, command })
        })
        .collect();

    let llm_settings: Result<Vec<LlmSettings>, SettingsParseError> = doc
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "ai")
        .map(|node| parse_llm_block(node))
        .collect();

    let llm_settings = llm_settings?;

    let bindings = bindings?;
    let global_bindings = global_bindings?;

    Ok(TopLevelKdlSettings {
        bindings,
        global_bindings,
        llm_settings,
    })
}

fn parse_llm_block(node: &KdlNode) -> Result<LlmSettings, SettingsParseError> {
    let children = node.children().ok_or_else(|| {
        SettingsParseError::MismatchedChildren(
            node.span().clone(),
            "ai node should have children".to_string(),
        )
    })?;

    let model = children
        .get("model")
        .and_then(|model_node| model_node.entries().first())
        .and_then(|entry| entry.value().as_string())
        .ok_or_else(|| SettingsParseError::MissingNode {
            span: children.span().clone(),
            node: "model".to_string(),
        })?
        .to_string();

    let system_prompt = children
        .get("systemPrompt")
        .and_then(|prompt_node| prompt_node.entries().first())
        .and_then(|entry| entry.value().as_string())
        .map(|s| s.to_string());

    Ok(LlmSettings {
        model,
        system_prompt,
    })
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
                    shortcut: KeyboardShortcut::new(Modifiers::MAC_CMD, Key::A),
                    command: ParsedCommand::Predefined(BuiltInCommand::HideApp),
                    slash_alias: None,
                    description: None
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: vec![]
            }
        );
    }

    #[test]
    pub fn test_insert_text_cmd_parsing() {
        let doc_str = r#"
        bind "Cmd J" alias="some_alias" description="some description" {
            InsertText {
                target "selection"
                text "something else"
            }
        }
        "#;

        let keybindings = parse_top_level_settings_block(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [LocalBinding {
                    shortcut: KeyboardShortcut::new(Modifiers::MAC_CMD, Key::J),
                    command: ParsedCommand::InsertText(ParsedCmdInsertText {
                        target: InsertTextTarget::Selection,
                        text: TextSource::Inline("something else".to_string())
                    }),
                    slash_alias: Some("some_alias".to_string()),
                    description: Some("some description".to_string())
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: vec![]
            }
        );
    }

    #[test]
    pub fn test_insert_text_cmd_parsing_with_script() {
        let doc_str = r#"
        bind "Cmd K" {
            InsertText {
                text {
                    call "my_script_function"
                }
            }
        }
        "#;

        let keybindings = parse_top_level_settings_block(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [LocalBinding {
                    shortcut: KeyboardShortcut::new(Modifiers::MAC_CMD, Key::K),
                    command: ParsedCommand::InsertText(ParsedCmdInsertText {
                        target: InsertTextTarget::Selection,
                        text: TextSource::Script(ScriptCall::new("my_script_function".to_string()))
                    }),
                    slash_alias: None,
                    description: None
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: vec![]
            }
        );
    }
}

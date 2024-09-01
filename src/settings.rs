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
        TextCommandContext,
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
struct TopLevelKdlSettings {
    bindings: Vec<Binding>,
    global_bindings: Vec<GlobalBinding>,
    llm_settings: Vec<LlmSettings>,
}

#[derive(Debug, PartialEq, Eq)]
enum ReplaceTextTarget {
    Selection,
}

#[derive(Debug, PartialEq, Eq)]
struct ReplaceText {
    target: ReplaceTextTarget,
    with: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Predefined(BuiltInCommand),
    ReplaceText(ReplaceText),
}

#[derive(Debug, PartialEq, Eq)]
enum GlobalCommand {
    ToggleAppVisibility,
}

#[derive(Debug, PartialEq, Eq)]
struct Binding {
    shortcut: KeyboardShortcut,
    command: Command,
}

#[derive(Debug, PartialEq, Eq)]
struct GlobalBinding {
    shortcut: KeyboardShortcut,
    command: GlobalCommand,
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
enum SettingsParseError {
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

fn parse_command(node: &KdlNode) -> Result<Command, SettingsParseError> {
    match node.name().value() {
        "ReplaceText" => parse_replace_text_command(node).map(Command::ReplaceText),
        name => match try_parse_builtin_command(name, node.entries()) {
            Some(cmd) => Ok(Command::Predefined(cmd)),
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
        "ToggleAppVisibility" => Ok(GlobalCommand::ToggleAppVisibility),
        name => Err(SettingsParseError::UnknownCommand(name.to_string())),
    }
}

fn parse_replace_text_command(node: &KdlNode) -> Result<ReplaceText, SettingsParseError> {
    use SettingsParseError as PE;
    if node.entries().len() > 0 {
        Err(PE::MismatchedArgsCount(node.span().clone(), 0))
    } else {
        let children = node.children().ok_or_else(|| {
            PE::MismatchedChildren(
                node.span().clone(),
                r#"ReplaceText needs to have 'target' and 'with' nodes
For example:
ReplaceText {
    target "selection"
    with "this is before {{selection}} and this is after"
}
"#
                .to_string(),
            )
        })?;

        let target = children
            .get("target")
            .ok_or_else(|| PE::MissingNode {
                span: children.span().clone(),
                node: "target".to_string(),
            })
            .and_then(parse_replace_text_target)?;

        let with = children
            .get("with")
            .ok_or_else(|| PE::MissingNode {
                span: children.span().clone(),
                node: "with".to_string(),
            })
            .and_then(parse_replace_text_with)?;

        Ok(ReplaceText { target, with })
    }
}

fn parse_replace_text_target(target: &KdlNode) -> Result<ReplaceTextTarget, SettingsParseError> {
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
            return (Err(PE::MismatchedType {
                span: target_entry.span().clone(),
                expected: "String",
            }))
        }
    };

    if target != "selection" {
        return Err(PE::CoulndntParseCommand(
            target_entry.span().clone(),
            r#"only "selection" is supported for 'target'"#.to_string(),
        ));
    }

    Ok(ReplaceTextTarget::Selection)
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
) -> Result<(KeyboardShortcut, Cmd), SettingsParseError> {
    if node.entries().len() != 1 {
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

    Ok((shortcut, command))
}

fn parse_top_level(block_str: &str) -> Result<TopLevelKdlSettings, SettingsParseError> {
    let doc = KdlDocument::from_str(block_str).map_err(SettingsParseError::ParseKdlErro)?;

    let bindings: Result<Vec<Binding>, SettingsParseError> = doc
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "bind")
        .map(|node| {
            parse_binding(node, parse_command)
                .map(|(shortcut, command)| Binding { shortcut, command })
        })
        .collect();

    let global_bindings: Result<Vec<GlobalBinding>, SettingsParseError> = doc
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "global")
        .map(|node| {
            parse_binding(node, parse_global_command)
                .map(|(shortcut, command)| GlobalBinding { shortcut, command })
        })
        .collect();

    let llm_settings: Result<Vec<LlmSettings>, SettingsParseError> = doc
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "llm")
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
            "llm node should have children".to_string(),
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

pub struct SettingsNoteEvalContext<'cx, IO: AppIO> {
    // parsed_bindings: Vec<Result<TopLevelKdlSettings, SettingsParseError>>,
    pub cmd_list: &'cx mut CommandList,
    pub should_force_eval: bool,
    pub app_io: &'cx mut IO,
    pub llm_settings: &'cx mut Option<LlmSettings>,
}

impl<'cx, IO: AppIO> NoteEvalContext for SettingsNoteEvalContext<'cx, IO> {
    fn begin(&mut self) {
        println!("##### STARTING settings eval");

        self.cmd_list.retain_only(|cmd| cmd.kind.is_some());
        self.cmd_list.reset_builtins_to_default_keybindings();

        // TODO handle error case
        self.app_io.cleanup_all_global_hotkeys().unwrap();
    }

    fn try_parse_block_lang(lang: &str) -> Option<CodeBlockKind> {
        match lang {
            "settings" => Some(CodeBlockKind::Source),

            output if output.starts_with("settings#") => {
                let hex_str = &output.strip_prefix("settings#")?;
                Some(CodeBlockKind::Output(SourceHash::parse(hex_str)))
            }

            _ => None,
        }
    }

    fn eval_block(&mut self, body: &str, hash: SourceHash) -> BlockEvalResult {
        let result = parse_top_level(body);

        let mut body = match &result {
            Ok(res) => format!("applied"),
            // Ok(res) => format!("Applied at {:#?}", Instant::now()),
            Err(err) => format!("error: {:#?}", err),
        };

        // TODO report if applying bindings failed

        if let Ok(mut settings) = result {
            for GlobalBinding { shortcut, command } in settings.global_bindings {
                println!("applying global {shortcut:?} to {command:?}");
                match command {
                    GlobalCommand::ToggleAppVisibility => {
                        match self
                            .app_io
                            .bind_global_hotkey(shortcut, Box::new(|| MsgToApp::ToggleVisibility))
                        {
                            Ok(_) => {
                                println!("registered global {shortcut:?} to show/hide Shelv");
                            }

                            Err(err) => {
                                println!("error registering global {shortcut:?} to show/hide Shelv, err = {err:?}");
                                body = err;
                            }
                        }
                    }
                }
            }

            for Binding { shortcut, command } in settings.bindings {
                println!("applying {shortcut:?} to {command:?}");
                match command {
                    Command::Predefined(kind) => {
                        self.cmd_list
                            .set_or_replace_builtin_shortcut(shortcut, kind);
                    }

                    Command::ReplaceText(cmd) => self.cmd_list.add(EditorCommand::user_defined(
                        // "replace text", // TODO figure out the name
                        shortcut,
                        map_text_command_to_command_handler(move |ctx| {
                            run_replace_text_cmd(ctx, &cmd)
                        }),
                    )),
                }
            }

            if let Some(last_llm_settings) = settings.llm_settings.pop() {
                *self.llm_settings = Some(last_llm_settings);
            }
        }

        BlockEvalResult {
            body,
            output_lang: format!("settings#{}", hash.to_string()),
        }
    }

    fn should_force_eval(&self) -> bool {
        self.should_force_eval
    }
}

fn run_replace_text_cmd(
    context: TextCommandContext,
    ReplaceText { target, with }: &ReplaceText,
) -> Option<Vec<TextChange>> {
    let TextCommandContext {
        text_structure: _,
        text,
        byte_cursor,
    } = context;

    let replacement = if with.contains("{{selection}}") {
        with.replace("{{selection}}", &text[byte_cursor.range()])
    } else {
        with.clone()
    };

    match target {
        ReplaceTextTarget::Selection => {
            Some([TextChange::Replace(byte_cursor, replacement)].into())
        }
    }
}

impl BuiltInCommand {
    fn name(&self) -> &'static str {
        use BuiltInCommand::*;
        match self {
            ExpandTaskMarker => "ExpandTaskMarker",
            IndentListItem => "IndentListItem",
            UnindentListItem => "UnindentListItem",
            SplitListItem => "SplitListItem",
            MarkdownBold => "MarkdownBold",
            MarkdownItalic => "MarkdownItalic",
            MarkdownStrikethrough => "MarkdownStrikethrough",
            MarkdownCodeBlock => "MarkdownCodeBlock",
            MarkdownH1 => "MarkdownH1",
            MarkdownH2 => "MarkdownH2",
            MarkdownH3 => "MarkdownH3",
            SwitchToNote(_) => "SwitchToNote",
            SwitchToSettings => "SwitchToSettings",
            PinWindow => "PinWindow",
            HideApp => "HideApp",
            RunLLMBlock => "RunLLMBLock",
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn test_bind_predefined_cmd_parsing() {
        let doc_str = r#"
        bind "Cmd A" { HideApp;}
        "#;

        let keybindings = parse_top_level(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [Binding {
                    shortcut: KeyboardShortcut::new(Modifiers::MAC_CMD, Key::A),
                    command: Command::Predefined(BuiltInCommand::HideApp),
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: vec![]
            }
        );
    }

    #[test]
    pub fn test_replace_text_cmd_parsing() {
        let doc_str = r#"
        bind "Cmd J" {
            ReplaceText {
                target "selection"
                with "something else"
            }
        }
        "#;

        let keybindings = parse_top_level(doc_str).unwrap();

        assert_eq!(
            keybindings,
            TopLevelKdlSettings {
                bindings: [Binding {
                    shortcut: KeyboardShortcut::new(Modifiers::MAC_CMD, Key::J),
                    command: Command::ReplaceText(ReplaceText {
                        target: ReplaceTextTarget::Selection,
                        with: "something else".to_string()
                    })
                }]
                .into(),
                global_bindings: vec![],
                llm_settings: vec![]
            }
        );
    }
}

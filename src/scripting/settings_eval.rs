//! Evaluates the settings: the base layer (`default-settings.ts`, also the module "shelv/defaults"),
//! then every ```js / ```ts block of the settings note, in one Boa `Context`. Each block's
//! `export default` is decoded into a [`Contribution`]; [`merge_settings`] turns them into the
//! command list. See docs/settings-js/codegen.md.

use std::{
    collections::{BTreeSet, HashSet},
    path::Path,
    rc::Rc,
    sync::OnceLock,
};

use boa_engine::{
    Context, JsError, JsString, JsValue, Module, Source, builtins::promise::PromiseState,
    js_string, property::PropertyKey,
};
use eframe::egui::KeyboardShortcut;
use smallvec::SmallVec;

use crate::{
    app_state::CodeBlockAnnotation,
    command::{
        CommandCondition, CommandInstance, CommandInstruction, CommandLabel, PhosphorIcon,
        SlashPaletteCmd,
    },
    scripting::typescript::transpile_typescript_to_javascript,
    text_structure::{SpanIndex, SpanKind, TextStructure},
};

use super::{
    js_error_message,
    js_module_loader::InMemoryModuleLoader,
    note_eval::HostWithLocalTimezone,
    settings_decode::{
        AiSettings, BindingIntent, CommandTable, Contribution, Decoder, GlobalAction, Need,
        UserCommand, UserCommandId, UserRun,
    },
};

/// The JS of an embedded .ts source, stripped once per process.
fn stripped(cell: &'static OnceLock<String>, name: &str, source: &str) -> &'static str {
    cell.get_or_init(|| {
        transpile_typescript_to_javascript(source, Some(true))
            .unwrap_or_else(|errors| panic!("{name} is not erasable TypeScript: {errors:?}"))
    })
}

fn system_javascript_file_js_content() -> &'static str {
    static CELL: OnceLock<String> = OnceLock::new();
    stripped(&CELL, "system.ts", include_str!("system.ts"))
}

fn default_settings_file_js_content() -> &'static str {
    static CELL: OnceLock<String> = OnceLock::new();
    stripped(
        &CELL,
        "default-settings.ts",
        include_str!("default-settings.ts"),
    )
}

/// The settings `Context` and everything its commands need at keypress.
pub struct Scripts {
    pub js_cx: Context,
    pub module_loader: Rc<InMemoryModuleLoader>,
    pub commands: CommandTable,
}

impl std::fmt::Debug for Scripts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scripts")
            .field("commands", &self.commands.len())
            .finish()
    }
}

/// One settings block's outcome.
#[derive(Debug)]
pub struct BlockResult {
    pub span: Option<SpanIndex>,
    pub result: Result<Contribution, String>,
    /// Exports shared with later blocks.
    pub named_exports: Vec<String>,
}

impl BlockResult {
    pub fn annotation(&self) -> CodeBlockAnnotation {
        match &self.result {
            Ok(contribution) => {
                let contributes = contribution.summary();
                let mut parts = Vec::new();
                if !contributes.is_empty() {
                    parts.push(format!("Contributes: {contributes}"));
                }
                if !self.named_exports.is_empty() {
                    parts.push(format!("Shares: {}", self.named_exports.join(", ")));
                }
                CodeBlockAnnotation::Applied {
                    message: if parts.is_empty() {
                        "Evaluated (no settings, no exports)".to_string()
                    } else {
                        parts.join(" · ")
                    },
                }
            }
            Err(message) => CodeBlockAnnotation::Error {
                title: "Settings not applied".to_string(),
                message: message.clone(),
            },
        }
    }
}

pub struct SettingsEval {
    pub scripts: Scripts,
    pub base: Contribution,
    pub blocks: Vec<BlockResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockLang {
    Js,
    Ts,
}

impl BlockLang {
    pub fn parse(lang: &str) -> Option<Self> {
        match lang {
            "js" => Some(Self::Js),
            "ts" => Some(Self::Ts),
            _ => None,
        }
    }
}

impl Scripts {
    /// A `Context` with "shelv" and "shelv/defaults" loaded, plus the decoded base layer.
    fn new_with_base() -> (Self, Contribution) {
        let loader = Rc::new(InMemoryModuleLoader::new());
        let mut js_cx = Context::builder()
            .module_loader(loader.clone())
            .host_hooks(Rc::new(HostWithLocalTimezone))
            .build()
            .expect("a default context builds");
        // `run` functions execute on keypress: an endless loop must not hang the app
        js_cx
            .runtime_limits_mut()
            .set_loop_iteration_limit(10_000_000);

        let mut scripts = Self {
            js_cx,
            module_loader: loader,
            commands: CommandTable::default(),
        };

        scripts
            .eval_module("shelv", "shelv:system", system_javascript_file_js_content())
            .unwrap_or_else(|err| panic!("system.ts failed to evaluate: {err}"));
        let defaults = scripts
            .eval_module(
                "shelv/defaults",
                "shelv:defaults",
                default_settings_file_js_content(),
            )
            .unwrap_or_else(|err| panic!("default-settings.ts failed to evaluate: {err}"));
        let base = scripts
            .decode_default_export(&defaults)
            .unwrap_or_else(|err| panic!("default-settings.ts is invalid: {err}"));

        (scripts, base)
    }

    pub fn new() -> Self {
        Self::new_with_base().0
    }

    fn eval_module(&mut self, specifier: &str, path: &str, source: &str) -> Result<Module, String> {
        let cx = &mut self.js_cx;
        let module = Module::parse(
            Source::from_bytes(source).with_path(Path::new(path)),
            None,
            cx,
        )
        .map_err(|err| js_error_message(&err, cx))?;
        self.module_loader
            .insert(specifier.to_string(), module.clone());

        let promise = module.load_link_evaluate(cx);
        cx.run_jobs().map_err(|err| js_error_message(&err, cx))?;
        match promise.state() {
            PromiseState::Fulfilled(_) => Ok(module),
            PromiseState::Rejected(err) => Err(js_error_message(&JsError::from_opaque(err), cx)),
            PromiseState::Pending => Err("the block did not finish evaluating".to_string()),
        }
    }

    fn decode_default_export(&mut self, module: &Module) -> Result<Contribution, String> {
        let namespace = module.namespace(&mut self.js_cx);
        let default = namespace
            .get(js_string!("default"), &mut self.js_cx)
            .map_err(|err| js_error_message(&err, &mut self.js_cx))?;
        Decoder {
            cx: &mut self.js_cx,
            table: &mut self.commands,
        }
        .settings(&default)
        .map_err(|err| err.to_string())
    }

    pub fn command(&self, id: UserCommandId) -> Option<&UserCommand> {
        self.commands.get(id)
    }
}

/// Runtime exports of a module, except `default`.
fn get_named_exports(js_cx: &mut Context, module: &Module) -> Vec<String> {
    let namespace = module.namespace(js_cx);
    namespace
        .own_property_keys(js_cx)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::String(name) => Some(name.to_std_string_escaped()),
            _ => None,
        })
        .filter(|name| name != "default")
        .collect()
}

/// Evaluates the base layer alone (startup, before the settings note is read).
pub fn eval_base_settings() -> SettingsEval {
    eval_settings_blocks(std::iter::empty())
}

/// Evaluates the ```js / ```ts blocks of the settings note on top of the base layer.
pub fn eval_settings_note(text: &str, text_structure: &TextStructure) -> SettingsEval {
    let blocks: SmallVec<[_; 8]> = text_structure
        .filter_map_codeblocks(|lang| BlockLang::parse(lang))
        .filter_map(|(index, _, _, lang)| {
            let (_, code_desc) = text_structure
                .iterate_immediate_children_of(index)
                .find(|(_, desc)| desc.kind == SpanKind::Text)?;
            Some((Some(index), &text[code_desc.byte_pos.range()], lang))
        })
        .collect();

    eval_settings_blocks(blocks)
}

pub fn eval_settings_blocks<'a>(
    blocks: impl IntoIterator<Item = (Option<SpanIndex>, &'a str, BlockLang)>,
) -> SettingsEval {
    let (mut scripts, base) = Scripts::new_with_base();
    // (export name, module specifier) of every earlier block that evaluated
    let mut shared: Vec<(String, String)> = Vec::new();
    let mut results = Vec::new();

    for (n, (span, code, lang)) in blocks.into_iter().enumerate() {
        let specifier = format!("settings:block-{}", n + 1);
        let result = eval_settings_block(&mut scripts, &shared, &specifier, code, lang);
        let block = match result {
            Ok((module, contribution)) => {
                let shares = get_named_exports(&mut scripts.js_cx, &module);
                shared.extend(shares.iter().map(|name| (name.clone(), specifier.clone())));
                BlockResult {
                    span,
                    result: Ok(contribution),
                    named_exports: shares,
                }
            }
            Err(message) => BlockResult {
                span,
                result: Err(message),
                named_exports: Vec::new(),
            },
        };
        results.push(block);
    }

    SettingsEval {
        scripts,
        base,
        blocks: results,
    }
}

fn eval_settings_block(
    scripts: &mut Scripts,
    shared: &[(String, String)],
    specifier: &str,
    code: &str,
    lang: BlockLang,
) -> Result<(Module, Contribution), String> {
    let code = match lang {
        BlockLang::Js => code.to_string(),
        BlockLang::Ts => {
            transpile_typescript_to_javascript(code, Some(true)).map_err(|errors| {
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            })?
        }
    };

    // Earlier blocks' exports, imported at the *end* so line numbers in errors stay the block's own.
    let mut source = code;
    for (name, from) in shared {
        source.push_str(&format!("\n;import {{ {name} }} from \"{from}\";"));
    }

    let path = format!("settings.md#{specifier}");
    let module = scripts.eval_module(specifier, &path, &source)?;
    let contribution = scripts.decode_default_export(&module)?;
    Ok((module, contribution))
}

// ------- merging into the command list -------

/// What the settings evaluate to, ready to install.
#[derive(Debug, Default)]
pub struct MergedSettings {
    pub keyboard: Vec<CommandInstance>,
    pub palette: Vec<SlashPaletteCmd>,
    /// Global hotkeys and the block that bound them (`None`: the base layer).
    pub global: Vec<(KeyboardShortcut, GlobalAction, Option<SpanIndex>)>,
    pub ai: Option<AiSettings>,
}

/// Materialized binding from the scripts
#[derive(Debug, Clone, Copy)]
struct Binding {
    key: KeyboardShortcut,
    command: UserCommandId,
    from_base: bool,
    origin: Option<SpanIndex>,
}

/// Applies a block's bindings: a bound key is replaced; a base-layer command bound
/// in the note moves off its base-layer keys (once); `command: null` frees the key.
fn apply_bindings(
    list: &mut Vec<Binding>,
    new: &[BindingIntent],
    base_commands: &BTreeSet<UserCommandId>,
    moved: &mut HashSet<UserCommandId>,
    origin: Option<SpanIndex>,
) {
    for binding in new {
        list.retain(|bound| bound.key != binding.key);
        if let Some(command) = binding.command {
            if base_commands.contains(&command) && moved.insert(command) {
                list.retain(|bound| !(bound.from_base && bound.command == command));
            }
            list.push(Binding {
                key: binding.key,
                command,
                from_base: false,
                origin,
            });
        }
    }
}

pub fn merge_settings(eval: &SettingsEval) -> MergedSettings {
    let table = &eval.scripts.commands;
    let block_contributions: Vec<(Option<SpanIndex>, &Contribution)> = eval
        .blocks
        .iter()
        .filter_map(|block| block.result.as_ref().ok().map(|c| (block.span, c)))
        .collect();

    // Note that even if one code block exports {useDefaults: false} then we disable the default entirely for eval
    let use_defaults_settings = block_contributions
        .iter()
        .rev()
        .find_map(|(_, c)| c.use_defaults)
        .unwrap_or(true);

    // The commands can come from 3 places:
    //   slash palette -> `.commands`
    //   bindings -> `.bindings`
    //   globals -> `.global`
    // and likely overlap, hence using a set is appropriete
    let base_commands_ids: BTreeSet<UserCommandId> = eval
        .base
        .bindings
        .iter()
        .chain(&eval.base.global)
        .filter_map(|b| b.command)
        .chain(eval.base.commands.iter().copied())
        .collect();

    let mut bindings: Vec<Binding> = Vec::new();
    let mut global: Vec<Binding> = Vec::new();
    let mut palette: Vec<UserCommandId> = Vec::new();
    let mut ai = None;
    let mut moved = HashSet::new();

    if use_defaults_settings {
        let base_bound = |list: &[BindingIntent]| -> Vec<Binding> {
            list.iter()
                .filter_map(|b| {
                    Some(Binding {
                        key: b.key,
                        command: b.command?,
                        from_base: true,
                        origin: None,
                    })
                })
                .collect()
        };

        bindings = base_bound(&eval.base.bindings);
        global = base_bound(&eval.base.global);
        palette = eval.base.commands.clone();
    }

    for (span, contribution) in &block_contributions {
        apply_bindings(
            &mut bindings,
            &contribution.bindings,
            &base_commands_ids,
            &mut moved,
            *span,
        );
        apply_bindings(
            &mut global,
            &contribution.global,
            &base_commands_ids,
            &mut moved,
            *span,
        );
        for id in &contribution.commands {
            let alias = table.get(*id).map(|c| c.alias.as_str());
            match palette
                .iter()
                .position(|existing| table.get(*existing).map(|c| c.alias.as_str()) == alias)
            {
                Some(pos) => palette[pos] = *id,
                None => palette.push(*id),
            }
        }
        if contribution.ai.is_some() {
            ai = contribution.ai.clone();
        }
    }

    let keyboard = bindings
        .iter()
        .filter_map(|bound| {
            Some(command_instance(
                bound.command,
                table.get(bound.command)?,
                Some(bound.key),
            ))
        })
        .collect();

    let palette = palette
        .iter()
        .filter_map(|id| {
            let command = table.get(*id)?;
            let shortcut = bindings
                .iter()
                .rev()
                .find(|b| b.command == *id)
                .map(|b| b.key);
            Some(SlashPaletteCmd {
                phosphor_icon: PhosphorIcon::from_string(
                    command.icon.as_deref().unwrap_or("user-circle-gear"),
                ),
                prefix: command.alias.clone(),
                description: command.description.clone(),
                instance: command_instance(*id, command, shortcut),
            })
        })
        .collect();

    let global = global
        .iter()
        .filter_map(|bound| match &table.get(bound.command)?.run {
            UserRun::Global(action) => Some((bound.key, *action, bound.origin)),
            _ => None,
        })
        .collect();

    MergedSettings {
        keyboard,
        palette,
        global,
        ai,
    }
}

/// A single action keeps its own instruction (and so its focus rules and hints); anything else runs
/// through `RunUserCommand`.
fn command_instance(
    id: UserCommandId,
    command: &UserCommand,
    shortcut: Option<KeyboardShortcut>,
) -> CommandInstance {
    let instruction = match &command.run {
        UserRun::Actions(actions) if actions.len() == 1 => actions[0].clone(),
        _ => CommandInstruction::RunUserCommand(id),
    };
    let (phase, cond) = match &command.run {
        UserRun::Actions(actions) if !actions.is_empty() => {
            actions[0].default_phase_and_conditions()
        }
        _ => instruction.default_phase_and_conditions(),
    };
    CommandInstance {
        shortcut,
        instruction,
        phase,
        cond,
        label: Some(CommandLabel {
            alias: command.alias.clone(),
            description: command.description.clone(),
        }),
    }
}

// ------- running a command at keypress -------

/// The actions a user command runs now: its fixed actions, or what its `run` function returns.
/// `selection` is the selected text, for `needs: [selection]`.
pub fn resolve_user_command(
    scripts: &mut Scripts,
    id: UserCommandId,
    selection: &str,
) -> Result<SmallVec<[CommandInstruction; 1]>, String> {
    let Some(command) = scripts.commands.get(id) else {
        return Err("this command no longer exists (settings changed)".to_string());
    };
    let alias = command.alias.clone();
    match &command.run {
        UserRun::Actions(actions) => Ok(actions.clone()),
        UserRun::Global(_) => Err(format!("\"{alias}\" only works as a global hotkey")),
        UserRun::Function { func, needs } => {
            let func = func.clone();
            let args: SmallVec<[JsValue; 1]> = needs
                .iter()
                .map(|need| match need {
                    Need::Selection => JsValue::from(JsString::from(selection)),
                })
                .collect();
            let cx = &mut scripts.js_cx;
            let result = func
                .call(&JsValue::undefined(), &args, cx)
                .map_err(|err| format!("{alias}: {}", js_error_message(&err, cx)))?;
            Decoder {
                cx,
                table: &mut scripts.commands,
            }
            .run_result(&result)
            .map_err(|err| format!("{alias}: {err}"))
        }
    }
}

/// Runs every time: used for the remaining actions of a command, whose conditions were checked
/// when it started.
pub fn always() -> CommandCondition {
    CommandCondition::loose_match([])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::CommandInstruction as CI, shortcuts::parse_keyboard_shortcut};

    fn eval(blocks: &[&str]) -> SettingsEval {
        eval_settings_blocks(blocks.iter().map(|code| (None, *code, BlockLang::Js)))
    }

    fn key(text: &str) -> KeyboardShortcut {
        parse_keyboard_shortcut(text).unwrap()
    }

    fn bound(merged: &MergedSettings, text: &str) -> Option<String> {
        merged
            .keyboard
            .iter()
            .find(|c| c.shortcut == Some(key(text)))
            .and_then(|c| c.label.as_ref().map(|l| l.alias.clone()))
    }

    #[test]
    fn base_layer_evaluates() {
        let eval = eval_base_settings();
        assert_eq!(eval.base.commands.len(), 9);
        assert_eq!(eval.base.bindings.len(), 28);
        assert_eq!(eval.base.global.len(), 1);

        let merged = merge_settings(&eval);
        assert_eq!(merged.keyboard.len(), 28);
        assert_eq!(merged.palette.len(), 9);
        assert_eq!(merged.global.len(), 1);
        assert_eq!(bound(&merged, "Cmd B").as_deref(), Some("bold"));
        let bold = merged
            .keyboard
            .iter()
            .find(|c| c.shortcut == Some(key("Cmd B")))
            .unwrap();
        assert_eq!(bold.instruction, CI::MarkdownBold);
        assert_eq!(bold.label.as_ref().unwrap().description, "Toggle Bold");
        let shelf1 = merged
            .keyboard
            .iter()
            .find(|c| c.shortcut == Some(key("Cmd 1")))
            .unwrap();
        assert_eq!(
            shelf1.instruction,
            CI::SwitchToNote { index: 0 },
            "1-based in JS, 0-based in Rust"
        );
    }

    #[test]
    fn block_contributes_and_shares() {
        let eval = eval(&[
            r#"import { action, command } from "shelv";
               export const wrap = (t) => `[${t}]`;
               export const sig = command("sig", { run: action.InsertText({ text: "-- me" }) });
               export default { commands: [sig], bindings: [{ key: "Cmd Shift S", command: sig }] };"#,
            r#"import { action, command, selection } from "shelv";
               const link = command("link", { needs: [selection], run: (t) => action.InsertText({ text: wrap(t) }) });
               export default { bindings: [{ key: "Cmd K", command: link }] };"#,
        ]);
        let first = eval.blocks[0].result.as_ref().unwrap();
        assert_eq!(first.summary(), "1 binding, 1 command");
        assert_eq!(
            eval.blocks[0].named_exports,
            vec!["sig".to_string(), "wrap".to_string()]
        );
        assert!(eval.blocks[1].result.is_ok(), "{:?}", eval.blocks[1].result);

        let merged = merge_settings(&eval);
        assert_eq!(merged.palette.len(), 10);
        let link = merged
            .keyboard
            .iter()
            .find(|c| c.shortcut == Some(key("Cmd K")))
            .unwrap();
        let CI::RunUserCommand(id) = link.instruction else {
            panic!("a function run goes through RunUserCommand");
        };
        let mut scripts = eval.scripts;
        let actions = resolve_user_command(&mut scripts, id, "hi").unwrap();
        assert_eq!(
            actions.as_slice(),
            &[CI::InsertText {
                text: "[hi]".into()
            }]
        );
    }

    #[test]
    fn errors_have_paths() {
        let eval = eval(&[
            r#"import { action, command } from "shelv";
               const b = command("b", { run: action.MarkdownBold });
               export default { bindings: [{ key: "Cmd Shit B", command: b }] };"#,
            r#"import { action } from "shelv";
               export default { bindings: [{ key: "Cmd B", command: action.MarkdownBold }] };"#,
            r#"export default { bindngs: [] };"#,
            r#"throw new Error("boom");"#,
        ]);
        let errors: Vec<String> = eval
            .blocks
            .iter()
            .map(|b| b.result.as_ref().unwrap_err().clone())
            .collect();
        assert!(
            errors[0].starts_with("bindings[0].key: \"Cmd Shit B\""),
            "{}",
            errors[0]
        );
        assert!(
            errors[1].contains("expected a command, got an action"),
            "{}",
            errors[1]
        );
        assert!(
            errors[2].starts_with("bindngs: unknown field"),
            "{}",
            errors[2]
        );
        assert_eq!(errors[3], "Error: boom");
    }

    #[test]
    fn ts_blocks_strip_and_report_lines() {
        let eval = eval_settings_blocks([
            (
                None,
                "import { action, command, type Settings } from \"shelv\";\nconst b = command(\"b\", { run: action.MarkdownBold });\nexport default { bindings: [{ key: \"Cmd Shift B\", command: b }] } satisfies Settings;",
                BlockLang::Ts,
            ),
            (None, "const a = 1;\nenum E { A }", BlockLang::Ts),
        ]);
        assert!(eval.blocks[0].result.is_ok(), "{:?}", eval.blocks[0].result);
        assert!(
            eval.blocks[1]
                .result
                .as_ref()
                .unwrap_err()
                .starts_with("2:1:")
        );
    }

    #[test]
    fn defaults_move_unbind_and_turn_off() {
        let moved = eval(&[r#"import * as defaults from "shelv/defaults";
            export default { bindings: [
                { key: "Cmd Shift B", command: defaults.bold },
                { key: "Cmd P", command: null },
            ] };"#]);
        let merged = merge_settings(&moved);
        assert_eq!(bound(&merged, "Cmd Shift B").as_deref(), Some("bold"));
        assert_eq!(
            bound(&merged, "Cmd B"),
            None,
            "bold moved off its default key"
        );
        assert_eq!(bound(&merged, "Cmd P"), None, "unbound");
        assert_eq!(bound(&merged, "Cmd I").as_deref(), Some("italic"));

        let own = eval(&[r#"import { action, command } from "shelv";
            const bold = command("bold", { run: action.MarkdownBold });
            export default { bindings: [{ key: "Cmd Shift B", command: bold }] };"#]);
        let merged = merge_settings(&own);
        assert_eq!(
            bound(&merged, "Cmd B").as_deref(),
            Some("bold"),
            "a new command does not move the default"
        );

        let off = eval(&[r#"import * as defaults from "shelv/defaults";
            export default { useDefaults: false, bindings: [{ key: "Tab", command: defaults.indent }] };"#]);
        let merged = merge_settings(&off);
        assert_eq!(merged.keyboard.len(), 1);
        assert!(merged.palette.is_empty());
        assert!(merged.global.is_empty());
    }

    /// `cargo test --release -- --ignored settings_startup --nocapture`
    #[test]
    #[ignore]
    fn settings_startup() {
        use std::time::Instant;
        let started = Instant::now();
        let (system, defaults) = (
            system_javascript_file_js_content().len(),
            default_settings_file_js_content().len(),
        );
        let strip = started.elapsed();

        let mut runs: Vec<std::time::Duration> = (0..20)
            .map(|_| {
                let started = Instant::now();
                let eval = eval_base_settings();
                let merged = merge_settings(&eval);
                assert_eq!(merged.keyboard.len(), 28);
                started.elapsed()
            })
            .collect();
        runs.sort();
        println!(
            "strip (once per process, {system} + {defaults} bytes): {strip:.2?}; base layer eval + merge: min {:.2?}, median {:.2?}",
            runs[0],
            runs[runs.len() / 2]
        );
        assert!(runs[runs.len() / 2] < std::time::Duration::from_millis(20));
    }

    #[test]
    fn new_users_settings_note_applies() {
        let text = include_str!("../default-notes/default-settings.md");
        let eval = eval_settings_note(text, &TextStructure::new(text));
        assert_eq!(eval.blocks.len(), 2);
        for block in &eval.blocks {
            assert!(block.result.is_ok(), "{:?}", block.result);
        }
        let merged = merge_settings(&eval);
        assert_eq!(bound(&merged, "Cmd K").as_deref(), Some("link"));
        assert!(merged.palette.iter().any(|p| p.prefix == "link"));
        assert_eq!(merged.global.len(), 1);
    }

    #[test]
    fn readme_examples_apply() {
        let text = include_str!("../../README.md");
        let eval = eval_settings_note(text, &TextStructure::new(text));
        assert!(
            eval.blocks.len() >= 4,
            "found {} settings blocks",
            eval.blocks.len()
        );
        for block in &eval.blocks {
            assert!(block.result.is_ok(), "{:?}", block.result);
        }
        let merged = merge_settings(&eval);
        assert_eq!(bound(&merged, "Cmd D").as_deref(), Some("day"));
        assert_eq!(bound(&merged, "Cmd Shift B").as_deref(), Some("bold"));
    }

    #[test]
    fn array_run_and_errors_at_keypress() {
        let eval = eval(&[r#"import { action, command } from "shelv";
            const both = command("both", { run: [action.MarkdownBold, action.MarkdownItalic] });
            const bad = command("bad", { run: () => "text" });
            const boom = command("boom", { run: () => { throw new TypeError("nope"); } });
            export default { bindings: [
                { key: "Cmd 7", command: both }, { key: "Cmd 8", command: bad }, { key: "Cmd 9", command: boom },
            ] };"#]);
        let merged = merge_settings(&eval);
        let id_of = |k: &str| match merged
            .keyboard
            .iter()
            .find(|c| c.shortcut == Some(key(k)))
            .unwrap()
            .instruction
        {
            CI::RunUserCommand(id) => id,
            ref other => panic!("{other:?}"),
        };
        let (both, bad, boom) = (id_of("Cmd 7"), id_of("Cmd 8"), id_of("Cmd 9"));
        let mut scripts = eval.scripts;
        assert_eq!(
            resolve_user_command(&mut scripts, both, "")
                .unwrap()
                .as_slice(),
            &[CI::MarkdownBold, CI::MarkdownItalic]
        );
        let err = resolve_user_command(&mut scripts, bad, "").unwrap_err();
        assert!(
            err.starts_with("bad: run: expected an action, got a string"),
            "{err}"
        );
        assert_eq!(
            resolve_user_command(&mut scripts, boom, "").unwrap_err(),
            "boom: TypeError: nope"
        );
    }
}

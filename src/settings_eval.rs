use std::{
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
};

use boa_engine::{
    builtins::promise::PromiseState, js_string, module::SimpleModuleLoader, property::PropertyKey,
    Context, JsError, JsNativeError, JsResult, JsValue, Module, NativeFunction,
};
use boa_parser::Source;
use itertools::Itertools;
use smallvec::SmallVec;

use crate::{
    app_actions::{AppAction, AppIO},
    app_state::MsgToApp,
    byte_span::ByteSpan,
    command::{
        map_text_command_to_command_handler, try_extract_text_command_context, CommandContext,
        CommandList, EditorCommand, SlashPaletteCmd, TextCommandContext, PROMOTED_COMMANDS,
    },
    effects::text_change_effect::TextChange,
    scripting::{BlockEvalResult, CodeBlockKind, NoteEvalContext, SourceHash},
    settings_parsing::{
        format_mac_shortcut, parse_top_level_settings_block, GlobalBinding, GlobalCommand,
        InsertTextTarget, LlmSettings, LocalBinding, ParsedCmdInsertText, ParsedCommand,
        ScriptCall, TextSource,
    },
    text_structure::{SpanDesc, SpanIndex, TextStructure},
};

pub const SETTINGS_BLOCK_LANG: &str = "settings";
pub const SETTINGS_BLOCK_LANG_OUTPUT: &str = "settings#";

pub const SETTINGS_SCRIPT_BLOCK_LANG: &str = "js";
pub const SETTINGS_SCRIPT_BLOCK_LANG_OUTPUT: &str = "js#";

#[derive(Debug, Clone, Copy)]
pub enum ScriptExportType {
    Str,
    Func,
    Unknown,
}

#[derive(Debug)]
pub struct Scripts {
    js_cx: Context,
    module_loader: Rc<SimpleModuleLoader>,
    script_blocks: Vec<SriptBlock>,
}

impl Scripts {
    fn find_exports(
        &self,
        name: &str,
    ) -> SmallVec<[(SourceHash, PropertyKey, ScriptExportType); 1]> {
        self.script_blocks
            .iter()
            .flat_map(|block| {
                block
                    .exports
                    .iter()
                    .map(|export| (block.source_hash, export))
            })
            .filter_map(|(block_hash, export)| {
                (name == export.name.as_str())
                    .then(|| (block_hash, export.key.clone(), export.export_type))
            })
            .collect()
    }
}

#[derive(Debug)]
struct ModuleExport {
    name: String,
    key: PropertyKey,
    export_type: ScriptExportType,
}

#[derive(Debug)]
pub struct SriptBlock {
    name: Option<String>,
    // module: Module,
    source_hash: SourceHash,
    //span: (SpanIndex, SpanDesc),
    // imports will come later
    exports: Vec<ModuleExport>,
}

impl Scripts {
    pub fn new() -> Self {
        // TODO get away from using SimpleModuleLoader
        let loader = Rc::new(SimpleModuleLoader::new("./").expect("why should it ever fail?"));

        // Just need to cast to a `ModuleLoader` before passing it to the builder.
        let context = Context::builder()
            .module_loader(loader.clone())
            .build()
            .expect("same here, why should it ever fail?");

        Self {
            module_loader: loader,
            js_cx: context,
            script_blocks: vec![],
        }
    }
}

// // ------- scripting settings eval -------
// pub struct ScriptingEvalContext {}

// pub struct ScriptingState {
//     sources: Vec<(String, SourceHash)>,
// }

impl NoteEvalContext for Scripts {
    type State = ();

    fn begin(&mut self) -> Self::State {
        ()
    }

    fn try_parse_block_lang(lang: &str) -> Option<CodeBlockKind> {
        match lang {
            SETTINGS_SCRIPT_BLOCK_LANG => Some(CodeBlockKind::Source),

            output if output.starts_with(SETTINGS_SCRIPT_BLOCK_LANG_OUTPUT) => {
                let hex_str = &output[SETTINGS_SCRIPT_BLOCK_LANG_OUTPUT.len()..];
                Some(CodeBlockKind::Output(SourceHash::parse(hex_str)))
            }

            _ => None,
        }
    }

    fn eval_block(
        &mut self,
        body: &str,
        hash: SourceHash,
        state: &mut Self::State,
    ) -> BlockEvalResult {
        let (exports, module) = match parse_and_eval_settings_script_block(body, &mut self.js_cx) {
            Ok((exports, module)) => (exports, module),
            Err(err) => {
                return BlockEvalResult {
                    body: format!("Error during evaluating the module\n{:#}", err),
                    output_lang: format!(
                        "{}{}",
                        SETTINGS_SCRIPT_BLOCK_LANG_OUTPUT,
                        hash.to_string()
                    ),
                };
            }
        };

        self.module_loader
            .insert(PathBuf::from_str(&hash.to_string()).unwrap(), module);

        let body = match exports.as_slice() {
            [] => "Block was evaluated by no exports were found".to_string(),
            exports => ["Evaluated, registered exports:"]
                .into_iter()
                .chain(exports.iter().map(|export| export.name.as_str()))
                .join("\n\t"),
        };

        self.script_blocks.push(SriptBlock {
            name: None,
            // module,
            source_hash: hash,
            // span: todo!(),
            exports,
        });

        BlockEvalResult {
            body,
            output_lang: format!("{}{}", SETTINGS_SCRIPT_BLOCK_LANG_OUTPUT, hash.to_string()),
        }
    }

    fn should_force_eval(&self) -> bool {
        true
    }
}

// ------- KDL settings eval -------
pub struct SettingsNoteEvalContext<'cx, IO: AppIO> {
    // parsed_bindings: Vec<Result<TopLevelKdlSettings, SettingsParseError>>,
    pub cmd_list: &'cx mut CommandList,
    pub scripts: &'cx Scripts,
    pub should_force_eval: bool,
    pub app_io: &'cx mut IO,
    pub llm_settings: &'cx mut Option<LlmSettings>,
}

impl<'cx, IO: AppIO> NoteEvalContext for SettingsNoteEvalContext<'cx, IO> {
    type State = ();

    fn begin(&mut self) {
        println!("##### STARTING settings eval");

        self.cmd_list.editor_retain_only(|cmd| cmd.kind.is_some());
        self.cmd_list.reset_builtins_to_default_keybindings();
        self.cmd_list.remove_custom_slash_commands();

        // TODO handle error case
        self.app_io.cleanup_all_global_hotkeys().unwrap();
    }

    fn try_parse_block_lang(lang: &str) -> Option<CodeBlockKind> {
        match lang {
            SETTINGS_BLOCK_LANG => Some(CodeBlockKind::Source),

            output if output.starts_with(SETTINGS_BLOCK_LANG_OUTPUT) => {
                let hex_str = &output.strip_prefix(SETTINGS_BLOCK_LANG_OUTPUT)?;
                Some(CodeBlockKind::Output(SourceHash::parse(hex_str)))
            }

            _ => None,
        }
    }

    fn eval_block(&mut self, body: &str, hash: SourceHash, _: &mut Self::State) -> BlockEvalResult {
        let result = parse_top_level_settings_block(body);

        // TODO report if applying bindings failed

        let output_lang = format!("{}{}", SETTINGS_BLOCK_LANG_OUTPUT, hash.to_string());

        match result {
            Ok(mut settings) => {
                let has_any_bindings =
                    !(settings.global_bindings.is_empty() && settings.bindings.is_empty());

                for GlobalBinding { shortcut, command } in settings.global_bindings.iter() {
                    println!("applying global {shortcut:?} to {command:?}");
                    match command {
                        GlobalCommand::ShowHideApp => {
                            match self.app_io.bind_global_hotkey(
                                *shortcut,
                                Box::new(|| MsgToApp::ToggleVisibility),
                            ) {
                                Ok(_) => {
                                    println!("registered global {shortcut:?} to show/hide Shelv");
                                }

                                Err(err) => {
                                    println!("error registering global {shortcut:?} to show/hide Shelv, err = {err:?}");

                                    return BlockEvalResult {
                                        body: format!("error: {:#?}", err),
                                        output_lang,
                                    };
                                }
                            }
                        }
                    }
                }

                for LocalBinding {
                    shortcut,
                    command,
                    slash_alias,
                } in settings.bindings
                {
                    println!("applying {shortcut:?} to {command:?}");
                    match command {
                        ParsedCommand::Predefined(kind) => {
                            self.cmd_list
                                .set_or_replace_builtin_shortcut(shortcut, kind);
                        }

                        ParsedCommand::InsertText(cmd) => {
                            if let TextSource::Script(ScriptCall { func_name: name }) = &cmd.text {
                                match self.scripts.find_exports(name).as_slice() {
                                    [] => return BlockEvalResult {
                                        body: format!("No matching exports for '{name}' were found in js blocks"),
                                        output_lang,
                                    },
                                    [_] => (),
                                    _ => return BlockEvalResult {
                                        body: format!("Multiple matching exports for '{name}' were found in js blocks"),
                                        output_lang,
                                    },
                                }
                            }

                            let cmd = EditorCommand::user_defined(
                                // "replace text", // TODO figure out the name
                                shortcut,
                                move |cx| call_replace_text(&cmd, cx),
                            );

                            if let Some(prefix) = slash_alias {
                                self.cmd_list.add_custom_slash_command(
                                    SlashPaletteCmd::from_editor_cmd(prefix, &cmd)
                                        .icon(egui_phosphor::light::USER_CIRCLE_GEAR.to_string()),
                                );
                            }

                            self.cmd_list.add_editor_cmd(cmd);
                        }
                    }
                }

                if let Some(last_llm_settings) = settings.llm_settings.pop() {
                    *self.llm_settings = Some(last_llm_settings);
                }

                // TODO temporarily disabled until we improve
                let has_any_bindings = false;

                let body = match has_any_bindings {
                    true => {
                        let mut body = "applied\n\nEffective bindings after the block:".to_string();
                        for (binding_name, shortcut) in
                            settings.global_bindings.into_iter().map(|binding| {
                                match binding.command {
                                    GlobalCommand::ShowHideApp => ("ShowHideApp", binding.shortcut),
                                }
                            })
                        {
                            body.push_str(&format!(
                                "\n{} -> {}",
                                binding_name,
                                format_mac_shortcut(shortcut)
                            ));
                        }

                        for (promoted_cmd, shortcut) in
                            PROMOTED_COMMANDS.into_iter().filter_map(|cmd| {
                                self.cmd_list
                                    .find(cmd)
                                    .and_then(|editor_cmd| editor_cmd.kind.zip(editor_cmd.shortcut))
                            })
                        {
                            body.push_str(&format!(
                                "\n{} -> {}",
                                promoted_cmd.name(),
                                format_mac_shortcut(shortcut)
                            ));
                        }
                        body
                    }
                    false => format!("applied"),
                };

                BlockEvalResult { body, output_lang }
            }
            Err(err) => BlockEvalResult {
                body: format!("error: {:#?}", err),
                output_lang,
            },
        }
    }

    fn should_force_eval(&self) -> bool {
        self.should_force_eval
    }
}

fn call_replace_text(
    cmd: &ParsedCmdInsertText,
    CommandContext {
        app_state, scripts, ..
    }: CommandContext,
) -> SmallVec<[AppAction; 1]> {
    let Some(TextCommandContext {
        text, byte_cursor, ..
    }) = try_extract_text_command_context(app_state)
    else {
        return SmallVec::new();
    };

    run_insert_text_cmd(text, byte_cursor, scripts, cmd)
        .map(|changes| {
            SmallVec::from([AppAction::apply_text_changes(
                app_state.selected_note,
                changes,
            )])
        })
        .unwrap_or_default()
}

// This example demonstrates how to use Boa's module API
fn run() -> Result<(), Box<dyn Error>> {
    // A simple module that we want to compile from Rust code.
    const MODULE_SRC: &str = r#"
        // import { pyth } from "./trig.mjs";
        // import * as ops from "./operations.mjs";

        export let result = 2 + 2;
        export function mix(a, b) {
            return a + b;
        }
    "#;

    // This can be overriden with any custom implementation of `ModuleLoader`.
    let loader = Rc::new(SimpleModuleLoader::new("./")?);

    // Just need to cast to a `ModuleLoader` before passing it to the builder.
    let mut context = &mut Context::builder().module_loader(loader.clone()).build()?;
    let source = Source::from_reader(MODULE_SRC.as_bytes(), Some(Path::new("./settings.js")));

    // Can also pass a `Some(realm)` if you need to execute the module in another realm.
    let module = Module::parse(source, None, context)?;

    // Don't forget to insert the parsed module into the loader itself, since the root module
    // is not automatically inserted by the `ModuleLoader::load_imported_module` impl.
    //
    // Simulate as if the "fake" module is located in the modules root, just to ensure that
    // the loader won't double load in case someone tries to import "./main.mjs".
    // loader.insert(
    //     Path::new("./").canonicalize()?.join("setting.js"),
    //     module.clone(),
    // );

    // The lifecycle of the module is tracked using promises which can be a bit cumbersome to use.
    // If you just want to directly execute a module, you can use the `Module::load_link_evaluate`
    // method to skip all the boilerplate.
    // This does the full version for demonstration purposes.
    //
    // parse -> load -> link -> evaluate
    let promise_result = module.load_link_evaluate(context);

    // let promise_result = module
    //     // Initial load that recursively loads the module's dependencies.
    //     // This returns a `JsPromise` that will be resolved when loading finishes,
    //     // which allows async loads and async fetches.
    //     .load(context)
    //     .then(
    //         Some(
    //             NativeFunction::from_copy_closure_with_captures(
    //                 |_, _, module, context| {
    //                     // After loading, link all modules by resolving the imports
    //                     // and exports on the full module graph, initializing module
    //                     // environments. This returns a plain `Err` since all modules
    //                     // must link at the same time.
    //                     module.link(context)?;
    //                     Ok(JsValue::undefined())
    //                 },
    //                 module.clone(),
    //             )
    //             .to_js_function(context.realm()),
    //         ),
    //         None,
    //         context,
    //     )
    //     .then(
    //         Some(
    //             NativeFunction::from_copy_closure_with_captures(
    //                 // Finally, evaluate the root module.
    //                 // This returns a `JsPromise` since a module could have
    //                 // top-level await statements, which defers module execution to the
    //                 // job queue.
    //                 |_, _, module, context| Ok(module.evaluate(context).into()),
    //                 module.clone(),
    //             )
    //             .to_js_function(context.realm()),
    //         ),
    //         None,
    //         context,
    //     );

    // Very important to push forward the job queue after queueing promises.
    context.run_jobs();

    // Checking if the final promise didn't return an error.
    match promise_result.state() {
        PromiseState::Pending => return Err("module didn't execute!".into()),
        PromiseState::Fulfilled(v) => {
            assert_eq!(v, JsValue::undefined());
        }
        PromiseState::Rejected(err) => {
            return Err(JsError::from_opaque(err).try_native(context)?.into())
        }
    }

    // We can access the full namespace of the module with all its exports.
    let namespace = module.namespace(context);
    let result = namespace.get(js_string!("result"), context)?;

    println!("result = {}", result.display());

    assert_eq!(
        namespace.get(js_string!("result"), context)?,
        JsValue::from(4)
    );

    let mix = namespace
        .get(js_string!("mix"), context)?
        .as_callable()
        .cloned()
        .ok_or_else(|| JsNativeError::typ().with_message("mix export wasn't a function!"))?;
    let result = mix.call(&JsValue::undefined(), &[5.into(), 10.into()], context)?;

    println!("mix(5, 10) = {}", result.display());

    for prop_key in namespace.own_property_keys(context)? {
        println!("Property key: {prop_key:#?}");

        let prop = namespace.get(prop_key, context)?;
        println!("Property: {prop:#?}");
        let func = prop.as_callable();
        println!("AsCallable: {func:#?}");
    }

    assert_eq!(result, 35.into());

    Ok(())
}

pub fn parse_and_eval_settings_script_block(
    block_script: &str,
    context: &mut Context,
) -> Result<(Vec<ModuleExport>, Module), Box<dyn Error>> {
    let source = Source::from_bytes(block_script);

    // Can also pass a `Some(realm)` if you need to execute the module in another realm.
    let module = Module::parse(source, None, context)?;

    // parse -> load -> link -> evaluate
    let promise_result = module.load_link_evaluate(context);

    // Very important to push forward the job queue after queueing promises.
    context.run_jobs();

    // Checking if the final promise didn't return an error.
    match promise_result.state() {
        PromiseState::Pending => return Err("module didn't execute!".into()),
        PromiseState::Fulfilled(v) => {
            assert_eq!(v, JsValue::undefined());
        }
        PromiseState::Rejected(err) => {
            return Err(JsError::from_opaque(err).try_native(context)?.into())
        }
    }

    // We can access the full namespace of the module with all its exports.
    let namespace = module.namespace(context);

    let exports: Vec<ModuleExport> = namespace
        .own_property_keys(context)?
        .into_iter()
        .filter_map(|key| match key.clone() {
            PropertyKey::String(js_string) => js_string.to_std_string().ok().zip(Some(key)),
            _ => None,
        })
        .map(|(prop_name, prop_key)| {
            // println!("Property key: {prop_key:#?}");

            let prop = namespace.get(prop_key.clone(), context).unwrap();
            // println!("Property: {prop:#?}");

            let export_type = if prop.as_callable().is_some() {
                ScriptExportType::Func
            } else if prop.as_string().is_some() {
                ScriptExportType::Str
            } else {
                ScriptExportType::Unknown
            };

            ModuleExport {
                name: prop_name,
                key: prop_key,
                export_type,
            }
        })
        .collect();

    Ok((exports, module))
}

fn run_insert_text_cmd(
    note_text: &str,
    byte_cursor: ByteSpan,
    scripts: &mut Scripts,
    ParsedCmdInsertText { target, text }: &ParsedCmdInsertText,
) -> Option<Vec<TextChange>> {
    // dbg!(&scripts.exports);

    let text = match text {
        TextSource::Inline(text) => text.clone(),
        TextSource::Script(script_call) => {
            // just grab the first one available
            let (block_hash, key, prop_type) = scripts
                .find_exports(&script_call.func_name)
                .into_iter()
                .nth(0)?;

            let namespace = scripts
                .module_loader
                .get(&PathBuf::from(block_hash.to_string()))?
                .namespace(&mut scripts.js_cx);

            // TODO proper error handling
            // probably with toast like UIs
            match prop_type {
                ScriptExportType::Str => namespace
                    .get(key, &mut scripts.js_cx)
                    .ok()?
                    .as_string()?
                    .to_std_string()
                    .ok()?,

                ScriptExportType::Func => namespace
                    .get(key, &mut scripts.js_cx)
                    .ok()?
                    .as_callable()?
                    .call(&JsValue::undefined(), &[], &mut scripts.js_cx)
                    .ok()?
                    .as_string()?
                    .to_std_string()
                    .ok()?,

                ScriptExportType::Unknown => "Unsupported export type".to_string(),
            }

            //todo!()
        }
    };

    let replacement = if text.contains("{{selection}}") {
        text.replace("{{selection}}", &note_text[byte_cursor.range()])
    } else {
        text
    };

    match target {
        InsertTextTarget::Selection => Some([TextChange::Insert(byte_cursor, replacement)].into()),
    }
}

#[test]
fn test_run() {
    let code = r#"
        const monthNames = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

        export function getCurrentDate(note) {
	const now = new Date();
	const year = now.getFullYear();
	const month = monthNames[now.getMonth()];
	// Ensures the day is 2 digits, adding leading zero if needed(2, '0');
	const day = String(now.getDate()).padStart(2, '0');
	return `${year}/${month}/${day}`;
        }
"#;

    let mut js_context = Context::builder().build().unwrap();
    let result = parse_and_eval_settings_script_block(code, &mut js_context);

    println!("result = {:#?}", result.as_ref().map(|r| &r.0));
    assert!(result.is_ok(), "run() should execute successfully");
}

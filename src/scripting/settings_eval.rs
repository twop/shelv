use std::{error::Error, path::PathBuf, rc::Rc, str::FromStr};

use boa_engine::{
    builtins::promise::PromiseState, module::SimpleModuleLoader, property::PropertyKey, Context,
    JsError, JsValue, Module,
};
use boa_parser::Source;
use itertools::Itertools;
use smallvec::SmallVec;

use crate::{
    app_actions::AppIO,
    app_state::MsgToApp,
    command::{
        AppFocus, CommandInstance, CommandInstruction, CommandList, CommandScope, ForwardToChild,
        ScriptCall, SlashPaletteCmd, TextSource,
    },
    settings_parsing::{
        parse_top_level_settings_block, GlobalBinding, GlobalCommand, LlmSettings, LocalBinding,
    },
};

use super::{
    js_module_loader::InMemoryModuleLoader,
    note_eval::HostWithLocalTimezone,
    note_eval_context::{BlockEvalResult, CodeBlockKind, NoteEvalContext, SourceHash},
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
    pub js_cx: Context,
    pub module_loader: Rc<InMemoryModuleLoader>,
    pub script_blocks: Vec<SriptBlock>,
}

impl Scripts {
    pub fn find_exports(
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
pub struct ModuleExport {
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
        let loader = Rc::new(InMemoryModuleLoader::new());

        // Just need to cast to a `ModuleLoader` before passing it to the builder.
        let context = Context::builder()
            .module_loader(loader.clone())
            .host_hooks(&HostWithLocalTimezone)
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
        // Update `body` to include all exports from current blocks
        let existing_exports: String = self
            .script_blocks
            .iter()
            .flat_map(|block| {
                block.exports.iter().map(|export| {
                    format!(
                        "import {{ {} }} from '{}';",
                        export.name,
                        block.source_hash.to_string()
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        let augmented_body = if !existing_exports.is_empty() {
            format!("{}\n\n{}", existing_exports, body)
        } else {
            body.to_string()
        };

        let body = augmented_body;

        let (exports, module) = match parse_and_eval_settings_script_block(&body, &mut self.js_cx) {
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
            .insert(format!("{}", hash.to_string()), module);

        let body = match exports.as_slice() {
            [] => "Block was evaluated by no exports were found".to_string(),
            exports => ["Registered exports:".to_string()]
                .into_iter()
                .chain(
                    exports
                        .iter()
                        .map(|export| format!("\"{}\"", export.name.as_str())),
                )
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

        self.cmd_list.reset_to_defaults();

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

    fn eval_block(&mut self, body: &str, hash: SourceHash, _: &mut ()) -> BlockEvalResult {
        // TODO report if applying bindings failed

        let output_lang = format!("{}{}", SETTINGS_BLOCK_LANG_OUTPUT, hash.to_string());

        let settings = match parse_top_level_settings_block(body) {
            Ok(settings) => settings,
            Err(err) => {
                let report = miette::Report::new(err);
                let body = format!("{report:? }");
                let plain_bytes = strip_ansi_escapes::strip(body.as_bytes());
                return BlockEvalResult {
                    body: String::from_utf8(plain_bytes).unwrap(),
                    output_lang,
                };
            }
        };

        // let has_any_bindings =
        //     !(settings.global_bindings.is_empty() && settings.bindings.is_empty());

        for GlobalBinding {
            shortcut,
            global_commands,
        } in settings.global_bindings.iter()
        {
            let shortcut = shortcut.value();
            let [command] = global_commands.as_slice() else {
                return BlockEvalResult {
                    body: "Currently only 1 command per binding is supported".to_string(),
                    output_lang,
                };
            };

            println!("applying global {shortcut:?} to {command:?}");
            match command {
                GlobalCommand::ShowHideApp => {
                    match self
                        .app_io
                        .bind_global_hotkey(shortcut, Box::new(|| MsgToApp::ToggleVisibility))
                    {
                        Ok(_) => {
                            println!("registered global {shortcut:?} to show/hide Shelv");
                        }

                        Err(err) => {
                            println!(
                                "error registering global {shortcut:?} to show/hide Shelv, err = {err:?}"
                            );

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
            instructions: instruction,
            slash_alias,
            description,
            phosphor_icon,
        } in settings.bindings
        {
            let [instruction] = instruction.as_slice() else {
                return BlockEvalResult {
                    body: "Currently only 1 command per binding is supported".to_string(),
                    output_lang,
                };
            };

            println!("applying {shortcut:?} to {instruction:?}");
            let validated_instruction = match instruction {
                CommandInstruction::InsertText(ForwardToChild(source)) => {
                    if let TextSource::Script(ScriptCall { func_name: name }) = &source {
                        match self.scripts.find_exports(name).as_slice() {
                            [] => {
                                return BlockEvalResult {
                                    body: format!(
                                        "No matching exports for '{name}' were found in js blocks"
                                    ),
                                    output_lang,
                                };
                            }
                            [_] => (),
                            _ => {
                                return BlockEvalResult {
                                    body: format!(
                                        "Multiple matching exports for '{name}' were found in js blocks"
                                    ),
                                    output_lang,
                                };
                            }
                        }
                    }

                    instruction
                }
                instruction => instruction,
            };

            if let Some(prefix) = slash_alias {
                let cmd = SlashPaletteCmd::from_instruction(
                    prefix,
                    validated_instruction.clone(),
                    CommandScope::Focus(AppFocus::NoteEditor),
                )
                .icon(
                    phosphor_icon
                        .unwrap_or_else(|| egui_phosphor::light::USER_CIRCLE_GEAR.to_string()),
                )
                .description(
                    description
                        .unwrap_or_else(|| validated_instruction.human_description().to_string()),
                )
                .shortcut(shortcut.as_ref().map(|v| v.value()));

                self.cmd_list.add_slash_command(cmd);
            }

            self.cmd_list.add_editor_cmd(CommandInstance::user_defined(
                validated_instruction.clone(),
                shortcut.map(|s| s.value()),
                CommandScope::Focus(AppFocus::NoteEditor),
            ));
        }

        if let Some(last_llm_settings) = settings.llm_settings {
            *self.llm_settings = Some(last_llm_settings);
        }

        let body = "applied".to_string();
        BlockEvalResult { body, output_lang }
    }

    fn should_force_eval(&self) -> bool {
        self.should_force_eval
    }
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
            return Err(JsError::from_opaque(err).try_native(context)?.into());
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

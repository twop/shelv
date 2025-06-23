use std::{error::Error, rc::Rc};

use boa_engine::{
    Context, JsError, JsValue, Module, builtins::promise::PromiseState, property::PropertyKey,
};
use boa_parser::Source;
use itertools::Itertools;
use smallvec::SmallVec;

use crate::{
    app_actions::AppIO,
    app_state::{CodeBlockAnnotation, MsgToApp},
    command::{
        AppFocus, CommandInstance, CommandInstruction, CommandList, CommandScope, ForwardToChild,
        ScriptCall, SlashPaletteCmd, TextSource,
    },
    settings_parsing::{
        GlobalBinding, GlobalCommand, LlmSettings, LocalBinding, parse_top_level_settings_block,
    },
    text_structure::{SpanIndex, SpanKind, TextStructure},
    theme::{AppTheme, FontTheme},
};

use super::{
    js_module_loader::InMemoryModuleLoader,
    note_eval::HostWithLocalTimezone,
    note_eval_context::{BlockEvalResult, SourceHash},
};

pub const SETTINGS_BLOCK_LANG: &str = "kdl";
pub const SETTINGS_SCRIPT_BLOCK_LANG: &str = "js";

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
pub fn eval_js_scripts_in_settings_note(
    text: &str,
    text_structure: &TextStructure,
) -> (Scripts, Vec<(SpanIndex, CodeBlockAnnotation)>) {
    let mut scripts = Scripts::new();
    let mut annotations: Vec<(SpanIndex, CodeBlockAnnotation)> = Vec::new();

    let code_blocks: SmallVec<[_; 8]> = text_structure
        .filter_map_codeblocks(|lang| (lang == SETTINGS_SCRIPT_BLOCK_LANG).then_some(0))
        .filter_map(|(index, _, _, _)| {
            let (_, code_desc) = text_structure
                .iterate_immediate_children_of(index)
                .find(|(_, desc)| desc.kind == SpanKind::Text)?;

            let code = &text[code_desc.byte_pos.range()];

            Some((index, code))
        })
        .collect();

    for (index, code) in code_blocks {
        annotations.push((index, eval_script_block(code, &mut scripts)));
    }

    (scripts, annotations)
}

fn eval_script_block(
    block_body: &str,
    scripts: &mut Scripts,
    // theme: &AppTheme,
) -> CodeBlockAnnotation {
    // Update `body` to include all exports from current blocks
    let existing_exports: String = scripts
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

    let body = if !existing_exports.is_empty() {
        format!("{}\n\n{}", existing_exports, block_body)
    } else {
        block_body.to_string()
    };

    let (exports, module) = match parse_and_eval_settings_script_block(&body, &mut scripts.js_cx) {
        Ok((exports, module)) => (exports, module),
        Err(err) => {
            // let mut job = LayoutJob::default();
            // let code_font_id = FontId {
            //     size: theme.fonts.size.normal,
            //     family: theme.fonts.family.code.clone(),
            // };
            // job.append(
            //     &err.to_string(),
            //     0.,
            //     TextFormat::simple(code_font_id, theme.colors.normal_text_color),
            // );
            return CodeBlockAnnotation::Error {
                title: "Error during evaluating the module".to_string(),
                message: err.to_string(),
            };
        }
    };

    let hash = SourceHash::from(block_body);
    scripts
        .module_loader
        .insert(format!("{}", hash.to_string()), module);

    let body = match exports.as_slice() {
        [] => "Block was evaluated by no exports were found".to_string(),
        exports => ["Registered exports:".to_string()]
            .into_iter()
            .chain(
                exports
                    .iter()
                    .map(|export| format!("{}", export.name.as_str())),
            )
            .join("\n\t"),
    };

    scripts.script_blocks.push(SriptBlock {
        name: None,
        // module,
        source_hash: hash,
        // span: todo!(),
        exports,
    });

    CodeBlockAnnotation::Applied { message: body }
}

pub fn eval_kdl_in_settings_note<IO: AppIO>(
    text: &str,
    text_structure: &TextStructure,
    mut eval_ctx: SettingsNoteEvalContext<IO>,
) -> Vec<(SpanIndex, CodeBlockAnnotation)> {
    let mut annotations: Vec<(SpanIndex, CodeBlockAnnotation)> = Vec::new();

    let code_blocks: SmallVec<[_; 8]> = text_structure
        .filter_map_codeblocks(|lang| (lang == SETTINGS_BLOCK_LANG).then_some(0))
        .filter_map(|(index, _, _, _)| {
            let (_, code_desc) = text_structure
                .iterate_immediate_children_of(index)
                .find(|(_, desc)| desc.kind == SpanKind::Text)?;

            let code = &text[code_desc.byte_pos.range()];

            Some((index, code))
        })
        .collect();

    for (index, code) in code_blocks {
        annotations.push((index, eval_settings_block(&mut eval_ctx, code)));
    }

    annotations
}

fn eval_settings_block<IO: AppIO>(
    eval_ctx: &mut SettingsNoteEvalContext<IO>,
    block_body: &str,
    // theme: &AppTheme,
) -> CodeBlockAnnotation {
    // TODO report if applying bindings failed

    let settings = match parse_top_level_settings_block(block_body) {
        Ok(settings) => settings,
        Err(err) => {
            let report = miette::Report::new(err);
            let body = format!("{report:? }");
            let plain_bytes = strip_ansi_escapes::strip(body.as_bytes());
            return CodeBlockAnnotation::Error {
                title: "Hm, that doesn't look right".to_string(),
                message: String::from_utf8(plain_bytes).unwrap(),
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
            return CodeBlockAnnotation::Error {
                title: "Oops".to_string(),
                message: "Currently only 1 command per binding is supported".to_string(),
            };
        };

        println!("applying global {shortcut:?} to {command:?}");
        match command {
            GlobalCommand::ShowHideApp => {
                match eval_ctx
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

                        return CodeBlockAnnotation::Error {
                            title: "OS refused to register shortcut".to_string(),
                            message: err,
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
            return CodeBlockAnnotation::Error {
                title: "Syntax error".to_string(),
                message: "Currently only 1 command per binding is supported".to_string(),
            };
        };

        println!("applying {shortcut:?} to {instruction:?}");
        let validated_instruction = match instruction {
            CommandInstruction::InsertText(ForwardToChild(source)) => {
                if let TextSource::Script(ScriptCall { func_name: name }) = &source {
                    match eval_ctx.scripts.find_exports(name).as_slice() {
                        [] => {
                            return CodeBlockAnnotation::Error {
                                title: "Oops".to_string(),
                                message: format!(
                                    "No matching exports for '{name}' were found in js blocks"
                                ),
                            };
                        }
                        [_] => (),
                        _ => {
                            return CodeBlockAnnotation::Error {
                                title: "Oops".to_string(),
                                message: format!(
                                    "Multiple matching exports for '{name}' were found in js blocks"
                                ),
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
                phosphor_icon.unwrap_or_else(|| egui_phosphor::light::USER_CIRCLE_GEAR.to_string()),
            )
            .description(
                description
                    .unwrap_or_else(|| validated_instruction.human_description().to_string()),
            )
            .shortcut(shortcut.as_ref().map(|v| v.value()));

            eval_ctx.cmd_list.add_slash_command(cmd);
        }

        eval_ctx
            .cmd_list
            .add_editor_cmd(CommandInstance::user_defined(
                validated_instruction.clone(),
                shortcut.map(|s| s.value()),
                CommandScope::Focus(AppFocus::NoteEditor),
            ));
    }

    if let Some(last_llm_settings) = settings.llm_settings {
        *eval_ctx.llm_settings = Some(last_llm_settings);
    }

    CodeBlockAnnotation::Applied {
        message: "Applied".to_string(),
    }
}

// ------- KDL settings eval -------
pub struct SettingsNoteEvalContext<'cx, IO: AppIO> {
    // parsed_bindings: Vec<Result<TopLevelKdlSettings, SettingsParseError>>,
    pub cmd_list: &'cx mut CommandList,
    pub scripts: &'cx Scripts,
    pub app_io: &'cx mut IO,
    pub llm_settings: &'cx mut Option<LlmSettings>,
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

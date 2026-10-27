//! Reads the values built by `system.ts` (frozen objects tagged with `__shelv`) back into Rust:
//! a block's `export default` into a [`Contribution`], `command(...)` values into [`UserCommand`]s, and
//! the value a `run` function returns into actions. Errors carry a path into the value
//! (`bindings[2].command.icon: unknown icon "calender"`). See docs/settings-js/codegen.md.

use std::fmt;

use boa_engine::{
    Context, JsObject, JsValue, js_string, object::builtins::JsArray, property::PropertyKey,
};
use eframe::egui::KeyboardShortcut;
use smallvec::SmallVec;

use crate::{
    command::{CommandInstruction, PhosphorIcon},
    shortcuts::parse_keyboard_shortcut,
};

use super::actions::{capability_from_js, instruction_from_js};

/// Index into [`CommandTable`]; stable for one settings evaluation.
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Hash, PartialEq, Eq)]
pub struct UserCommandId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Editor,
    Global,
}

/// What a global command does (v1: only `action.ShowHideApp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalAction {
    ShowHideApp,
}

/// An action as written in JS: either an editor action or a global one.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedAction {
    Editor(CommandInstruction),
    Global(GlobalAction),
}

/// A capability for `needs`: what a function `run` receives, entry i → argument i. The variants are
/// system.ts's capability exports (`selection`), decoded with serde from `{ capability: "selection" }`
/// and kept equal to the exports by `actions::tests::system_ts_capabilities_match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, strum::VariantNames)]
#[serde(tag = "capability", rename_all = "camelCase", deny_unknown_fields)]
#[strum(serialize_all = "camelCase")]
pub enum Need {
    /// The selected text in the current note, `""` when nothing is selected.
    Selection,
}

#[derive(Debug, Clone)]
pub enum UserRun {
    /// `run: action.X` or `run: [action.X, action.Y]` (in order; empty = no-op).
    Actions(SmallVec<[CommandInstruction; 1]>),
    /// `run: action.ShowHideApp` on a global command.
    Global(GlobalAction),
    /// `run: (...inputs) => ...`, called at keypress with the resolved `needs`.
    Function {
        func: JsObject,
        needs: SmallVec<[Need; 1]>,
    },
}

#[derive(Debug, Clone)]
pub struct UserCommand {
    /// The `id` that `command()` stamped: the command's identity across blocks.
    pub js_id: u32,
    pub alias: String,
    /// A valid Phosphor icon name, when one was given.
    pub icon: Option<String>,
    /// Palette and hint text (defaults to the alias).
    pub description: String,
    pub scope: Scope,
    pub run: UserRun,
}

/// All commands seen in one settings evaluation, deduplicated by their JS identity.
#[derive(Debug, Default)]
pub struct CommandTable {
    list: Vec<UserCommand>,
}

impl CommandTable {
    pub fn get(&self, id: UserCommandId) -> Option<&UserCommand> {
        self.list.get(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    fn insert(&mut self, command: UserCommand) -> UserCommandId {
        if let Some(id) = self.find_js(command.js_id) {
            return id;
        }

        let id = UserCommandId(self.list.len() as u32);
        self.list.push(command);
        id
    }

    fn find_js(&self, js_id: u32) -> Option<UserCommandId> {
        self.list
            .iter()
            .enumerate()
            .find_map(|(index, cmd)| (cmd.js_id == js_id).then(|| index as u32))
            .map(UserCommandId)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiToken {
    /// `secret("name")`: looked up in the macOS keychain when a request is made.
    Secret { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSettings {
    pub model: String,
    pub system_prompt: Option<String>,
    pub token: Option<AiToken>,
    pub use_shelv_system_prompt: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct BindingIntent {
    pub key: KeyboardShortcut,
    /// `None`: `command: null`, frees the key.
    pub command: Option<UserCommandId>,
}

/// One block's `export default`.
#[derive(Debug, Clone, Default)]
pub struct Contribution {
    pub use_defaults: Option<bool>,
    pub commands: Vec<UserCommandId>,
    pub global: Vec<BindingIntent>,
    pub bindings: Vec<BindingIntent>,
    pub ai: Option<AiSettings>,
}

impl Contribution {
    /// `"2 bindings, 1 global, 3 commands"`, for the block's annotation.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        let mut count = |n: usize, one: &str, many: &str| {
            if n > 0 {
                parts.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        };
        count(self.bindings.len(), "binding", "bindings");
        count(self.global.len(), "global", "global");
        count(self.commands.len(), "command", "commands");
        if self.ai.is_some() {
            parts.push("ai".to_string());
        }
        if self.use_defaults == Some(false) {
            parts.push("defaults off".to_string());
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

type DecodeResult<T> = Result<T, DecodeError>;

fn error<T>(path: &str, message: impl Into<String>) -> DecodeResult<T> {
    Err(DecodeError {
        path: path.to_string(),
        message: message.into(),
    })
}

fn join(path: &str, field: &str) -> String {
    if path.is_empty() {
        field.to_string()
    } else {
        format!("{path}.{field}")
    }
}

fn describe(value: &JsValue) -> &'static str {
    if value.is_undefined() {
        "undefined"
    } else if value.is_null() {
        "null"
    } else if value.as_string().is_some() {
        "a string"
    } else if value.as_number().is_some() {
        "a number"
    } else if value.as_boolean().is_some() {
        "a boolean"
    } else if value.as_callable().is_some() {
        "a function"
    } else if value.as_object().is_some_and(|o| o.is_array()) {
        "an array"
    } else {
        "an object"
    }
}

/// Reads JS values with a `Context`; commands go into `table`.
pub struct Decoder<'a> {
    pub cx: &'a mut Context,
    pub table: &'a mut CommandTable,
}

impl Decoder<'_> {
    fn get(&mut self, object: &JsObject, field: &str, path: &str) -> DecodeResult<JsValue> {
        object
            .get(js_string!(field), self.cx)
            .or_else(|err| error(&join(path, field), super::js_error_message(&err, self.cx)))
    }

    fn tag(&mut self, value: &JsValue) -> Option<String> {
        let object = value.as_object()?;
        object
            .get(js_string!("__shelv"), self.cx)
            .ok()?
            .as_string()
            .map(|s| s.to_std_string_escaped())
    }

    fn string(&mut self, value: &JsValue, path: &str) -> DecodeResult<String> {
        match value.as_string() {
            Some(s) => Ok(s.to_std_string_escaped()),
            None => error(path, format!("expected a string, got {}", describe(value))),
        }
    }

    fn optional_string(&mut self, value: &JsValue, path: &str) -> DecodeResult<Option<String>> {
        if value.is_undefined() {
            Ok(None)
        } else {
            self.string(value, path).map(Some)
        }
    }

    fn array(&mut self, value: &JsValue, path: &str) -> DecodeResult<Vec<JsValue>> {
        let Some(array) = value
            .as_object()
            .filter(|o| o.is_array())
            .and_then(|o| JsArray::from_object(o).ok())
        else {
            return error(path, format!("expected an array, got {}", describe(value)));
        };
        let length = array
            .length(self.cx)
            .or_else(|err| error(path, super::js_error_message(&err, self.cx)))?;
        (0..length)
            .map(|i| {
                array
                    .get(i, self.cx)
                    .or_else(|err| error(path, super::js_error_message(&err, self.cx)))
            })
            .collect()
    }

    /// Rejects keys outside `allowed`, so a typo fails instead of doing nothing.
    fn check_keys(&mut self, object: &JsObject, allowed: &[&str], path: &str) -> DecodeResult<()> {
        let keys = object
            .own_property_keys(self.cx)
            .or_else(|err| error(path, super::js_error_message(&err, self.cx)))?;
        for key in keys {
            if let PropertyKey::String(key) = key {
                let key = key.to_std_string_escaped();
                if !allowed.contains(&key.as_str()) {
                    let expected = allowed
                        .iter()
                        .map(|k| format!("\"{k}\""))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return error(
                        &join(path, &key),
                        format!("unknown field (expected {expected})"),
                    );
                }
            }
        }
        Ok(())
    }

    /// A block's `export default`; `undefined` (no default export) contributes nothing.
    pub fn settings(&mut self, value: &JsValue) -> DecodeResult<Contribution> {
        if value.is_undefined() {
            return Ok(Contribution::default());
        }
        let Some(object) = value
            .as_object()
            .filter(|o| !o.is_array() && !o.is_callable())
        else {
            return error(
                "export default",
                format!("expected a settings object, got {}", describe(value)),
            );
        };
        self.check_keys(
            &object,
            &["useDefaults", "commands", "global", "bindings", "ai"],
            "",
        )?;

        let mut contribution = Contribution::default();

        let use_defaults = self.get(&object, "useDefaults", "")?;
        if !use_defaults.is_undefined() {
            match use_defaults.as_boolean() {
                Some(b) => contribution.use_defaults = Some(b),
                None => {
                    return error(
                        "useDefaults",
                        format!("expected true or false, got {}", describe(&use_defaults)),
                    );
                }
            }
        }

        let commands = self.get(&object, "commands", "")?;
        if !commands.is_undefined() {
            for (i, entry) in self.array(&commands, "commands")?.iter().enumerate() {
                let path = format!("commands[{i}]");
                let id = self.try_decode_and_register_command(entry, &path)?;
                if self.table.get(id).is_some_and(|c| c.scope == Scope::Global) {
                    return error(&path, "a global command cannot be in the palette");
                }
                contribution.commands.push(id);
            }
        }

        for (field, scope) in [("bindings", Scope::Editor), ("global", Scope::Global)] {
            let list = self.get(&object, field, "")?;
            if list.is_undefined() {
                continue;
            }
            for (i, entry) in self.array(&list, field)?.iter().enumerate() {
                let binding = self.binding(entry, &format!("{field}[{i}]"), scope)?;
                match scope {
                    Scope::Editor => contribution.bindings.push(binding),
                    Scope::Global => contribution.global.push(binding),
                }
            }
        }

        let ai = self.get(&object, "ai", "")?;
        if !ai.is_undefined() {
            contribution.ai = Some(self.ai(&ai)?);
        }

        Ok(contribution)
    }

    fn binding(
        &mut self,
        value: &JsValue,
        path: &str,
        scope: Scope,
    ) -> DecodeResult<BindingIntent> {
        let Some(object) = value.as_object().filter(|o| !o.is_array()) else {
            return error(
                path,
                format!("expected {{ key, command }}, got {}", describe(value)),
            );
        };
        self.check_keys(&object, &["key", "command"], path)?;

        let key_value = self.get(&object, "key", path)?;
        if key_value.is_undefined() {
            return error(&join(path, "key"), "required");
        }
        let key_path = join(path, "key");
        let key_text = self.string(&key_value, &key_path)?;
        let key = parse_keyboard_shortcut(&key_text)
            .or_else(|reason| error(&key_path, format!("\"{key_text}\": {reason}")))?;

        let command_path = join(path, "command");
        let command_value = self.get(&object, "command", path)?;
        if command_value.is_null() {
            return Ok(BindingIntent { key, command: None });
        }
        if command_value.is_undefined() {
            return error(&command_path, "required (use null to free the key)");
        }
        let id = self.try_decode_and_register_command(&command_value, &command_path)?;
        let command_scope = self.table.get(id).map(|c| c.scope);
        match (scope, command_scope) {
            (Scope::Editor, Some(Scope::Global)) => {
                let alias = self
                    .table
                    .get(id)
                    .map(|c| c.alias.clone())
                    .unwrap_or_default();
                error(
                    &command_path,
                    format!("\"{alias}\" is a global command, bind it in global"),
                )
            }
            (Scope::Global, Some(Scope::Editor)) => {
                let alias = self
                    .table
                    .get(id)
                    .map(|c| c.alias.clone())
                    .unwrap_or_default();
                error(
                    &command_path,
                    format!("expected a global command (run: action.ShowHideApp), got \"{alias}\""),
                )
            }
            _ => Ok(BindingIntent {
                key,
                command: Some(id),
            }),
        }
    }

    /// A `command(...)` value. The same value decodes to the same id.
    pub fn try_decode_and_register_command(
        &mut self,
        value: &JsValue,
        path: &str,
    ) -> DecodeResult<UserCommandId> {
        match self.tag(value).as_deref() {
            Some("command") => {}
            Some("builtin") | Some("builtin-constructor") => {
                let name = value
                    .as_object()
                    .and_then(|o| o.get(js_string!("builtin"), self.cx).ok())
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default();
                return error(
                    path,
                    format!(
                        "expected a command, got an action: wrap it, command(\"name\", {{ run: action.{name} }})"
                    ),
                );
            }
            _ => {
                return error(path, format!("expected a command, got {}", describe(value)));
            }
        }
        let object = value.as_object().expect("tagged values are objects");

        let js_id = self
            .get(&object, "id", path)?
            .as_number()
            .map(|n| n as u32)
            .unwrap_or(u32::MAX);
        if let Some(id) = self.table.find_js(js_id) {
            return Ok(id);
        }

        let alias = {
            let v = self.get(&object, "alias", path)?;
            self.string(&v, &join(path, "alias"))?
        };
        let icon = {
            let v = self.get(&object, "icon", path)?;
            self.optional_string(&v, &join(path, "icon"))?
        };
        if let Some(name) = &icon {
            if PhosphorIcon::from_string(name).is_none() {
                return error(&join(path, "icon"), format!("unknown icon \"{name}\""));
            }
        }
        let description = {
            let v = self.get(&object, "description", path)?;
            self.optional_string(&v, &join(path, "description"))?
        }
        .unwrap_or_else(|| alias.clone());
        let scope = match self.get(&object, "scope", path)?.as_string() {
            Some(s) if s.to_std_string_escaped() == "global" => Scope::Global,
            _ => Scope::Editor,
        };

        let run_path = join(path, "run");
        let run_value = self.get(&object, "run", path)?;
        let run = if self.is_user_function(&run_value) {
            let needs_value = self.get(&object, "needs", path)?;
            let mut needs = SmallVec::new();
            for (i, need) in self
                .array(&needs_value, &join(path, "needs"))?
                .iter()
                .enumerate()
            {
                needs.push(self.need(need, &format!("{}[{i}]", join(path, "needs")))?);
            }
            UserRun::Function {
                func: run_value.as_object().expect("functions are objects"),
                needs,
            }
        } else {
            match self.run_value(&run_value, &run_path)? {
                RunValue::Global(action) => UserRun::Global(action),
                RunValue::Actions(list) => UserRun::Actions(list),
            }
        };

        Ok(self.table.insert(UserCommand {
            js_id,
            alias,
            icon,
            description,
            scope,
            run,
        }))
    }

    fn need(&mut self, value: &JsValue, path: &str) -> DecodeResult<Need> {
        if self.tag(value).as_deref() == Some("capability") {
            let object = value.as_object().expect("tagged values are objects");
            return capability_from_js(&object, self.cx)
                .or_else(|(_, message)| error(path, message));
        }
        error(
            path,
            format!(
                "expected a capability (such as selection), got {}",
                describe(value)
            ),
        )
    }

    fn is_user_function(&mut self, value: &JsValue) -> bool {
        value.as_callable().is_some() && self.tag(value).is_none()
    }

    fn run_value(&mut self, value: &JsValue, path: &str) -> DecodeResult<RunValue> {
        if value.as_object().is_some_and(|o| o.is_array()) {
            let mut list = SmallVec::new();
            for (i, entry) in self.array(value, path)?.iter().enumerate() {
                match self.action(entry, &format!("{path}[{i}]"))? {
                    DecodedAction::Editor(instruction) => list.push(instruction),
                    DecodedAction::Global(_) => {
                        return error(
                            &format!("{path}[{i}]"),
                            "action.ShowHideApp can only be the whole run of a command (for global bindings)",
                        );
                    }
                }
            }
            return Ok(RunValue::Actions(list));
        }
        Ok(match self.action(value, path)? {
            DecodedAction::Editor(instruction) => {
                RunValue::Actions(SmallVec::from_elem(instruction, 1))
            }
            DecodedAction::Global(action) => RunValue::Global(action),
        })
    }

    /// The value a `run` function returned: an action, an array of actions, or nothing.
    pub fn run_result(
        &mut self,
        value: &JsValue,
    ) -> DecodeResult<SmallVec<[CommandInstruction; 1]>> {
        if value.is_null_or_undefined() {
            return Ok(SmallVec::new());
        }
        if let Some(object) = value.as_object() {
            if object
                .get(js_string!("then"), self.cx)
                .ok()
                .is_some_and(|t| t.as_callable().is_some())
            {
                return error(
                    "run",
                    "returned a Promise: async commands are not supported yet (flow)",
                );
            }
        }
        if self.tag(value).as_deref() == Some("command") {
            return error(
                "run",
                "returned a command: commands cannot run commands in v1, return actions",
            );
        }
        match self.run_value(value, "run")? {
            RunValue::Actions(list) => Ok(list),
            RunValue::Global(_) => error(
                "run",
                "returned action.ShowHideApp, which only works as a global command",
            ),
        }
    }

    pub fn action(&mut self, value: &JsValue, path: &str) -> DecodeResult<DecodedAction> {
        let tag = self.tag(value);
        let object = match (tag.as_deref(), value.as_object()) {
            (Some("builtin"), Some(object)) => object,
            (Some("builtin-constructor"), Some(object)) => {
                let name = self.get(&object, "builtin", path)?;
                let name = self.string(&name, path)?;
                return error(
                    path,
                    format!(
                        "action.{name} has required parameters, call it: action.{name}({{ ... }})"
                    ),
                );
            }
            (Some("command"), _) => {
                return error(
                    path,
                    "expected an action, got a command (commands cannot run commands in v1)",
                );
            }
            _ => {
                return error(
                    path,
                    format!(
                        "expected an action, got {}{}",
                        describe(value),
                        if value.as_string().is_some() {
                            " (text is action.InsertText({ text }))"
                        } else {
                            ""
                        }
                    ),
                );
            }
        };

        instruction_from_js(&object, self.cx)
            .map(|instruction| match instruction {
                CommandInstruction::ShowHideApp => DecodedAction::Global(GlobalAction::ShowHideApp),
                instruction => DecodedAction::Editor(instruction),
            })
            .or_else(|(inner, message)| match inner.as_str() {
                "" | "." => error(path, message),
                inner => error(&format!("{path}.{inner}"), message),
            })
    }

    fn ai(&mut self, value: &JsValue) -> DecodeResult<AiSettings> {
        let path = "ai";
        let Some(object) = value.as_object().filter(|o| !o.is_array()) else {
            return error(path, format!("expected an object, got {}", describe(value)));
        };
        self.check_keys(
            &object,
            &["model", "systemPrompt", "token", "useShelvSystemPrompt"],
            path,
        )?;
        let model = self.get(&object, "model", path)?;
        if model.is_undefined() {
            return error("ai.model", "required");
        }
        let model = self.string(&model, "ai.model")?;
        let system_prompt = {
            let v = self.get(&object, "systemPrompt", path)?;
            self.optional_string(&v, "ai.systemPrompt")?
        };
        let token_value = self.get(&object, "token", path)?;
        let token = if token_value.is_undefined() {
            None
        } else if self.tag(&token_value).as_deref() == Some("secret") {
            let object = token_value.as_object().expect("tagged values are objects");
            let name = self.get(&object, "name", "ai.token")?;
            Some(AiToken::Secret {
                name: self.string(&name, "ai.token.name")?,
            })
        } else {
            return error(
                "ai.token",
                format!(
                    "expected a string or secret(\"name\"), got {}",
                    describe(&token_value)
                ),
            );
        };
        let use_shelv_system_prompt = {
            let v = self.get(&object, "useShelvSystemPrompt", path)?;
            if v.is_undefined() {
                None
            } else {
                match v.as_boolean() {
                    Some(b) => Some(b),
                    None => {
                        return error(
                            "ai.useShelvSystemPrompt",
                            format!("expected true or false, got {}", describe(&v)),
                        );
                    }
                }
            }
        };
        Ok(AiSettings {
            model,
            system_prompt,
            token,
            use_shelv_system_prompt,
        })
    }
}

enum RunValue {
    Actions(SmallVec<[CommandInstruction; 1]>),
    Global(GlobalAction),
}

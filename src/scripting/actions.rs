//! Decoding `action.*` values of system.ts into [`CommandInstruction`]s, and its capabilities
//! (`selection`) into [`Need`]s, with serde.
//!
//! system.ts builds actions as `{ __shelv: "builtin", builtin, params }` (a frozen object, or a
//! function object for actions usable both bare and called). serde reads `{ builtin, ...params }`:
//! `CommandInstruction` is internally tagged by `builtin`, its variant and field names are the JS
//! ones, and `tests::system_ts_actions_match` keeps `action.*` and the variants equal.

use boa_engine::{Context, JsObject, JsString};

use crate::command::CommandInstruction;

use super::settings_decode::Need;

/// Reads an action value. serde buffers an internally tagged enum before picking the variant, so
/// errors have no path inside the action; the messages name the field (`unknown field \`txt\``,
/// `missing field \`text\``, `index must be one of ...`). system.ts checks types at the call already.
pub fn instruction_from_js(object: &JsObject, cx: &mut Context) -> Result<CommandInstruction, (String, String)> {
    let field = |name: &'static str, cx: &mut Context| {
        object
            .get(JsString::from(name), cx)
            .and_then(|value| value.to_json(cx))
            .map_err(|err| (name.to_string(), super::js_error_message(&err, cx)))
    };
    // only these two fields: a function object's own `length` / `name` must not reach serde
    let builtin = field("builtin", cx)?;
    let mut json = match field("params", cx)? {
        Some(params @ serde_json::Value::Object(_)) => params,
        _ => serde_json::json!({}),
    };
    json["builtin"] = builtin.unwrap_or_default();

    serde_path_to_error::deserialize(json).map_err(|err| (err.path().to_string(), err.inner().to_string()))
}

/// Reads a capability value (`{ __shelv: "capability", capability: "selection" }`) into a [`Need`].
pub fn capability_from_js(object: &JsObject, cx: &mut Context) -> Result<Need, (String, String)> {
    let name = object
        .get(JsString::from("capability"), cx)
        .and_then(|value| value.to_json(cx))
        .map_err(|err| ("capability".to_string(), super::js_error_message(&err, cx)))?;
    let json = serde_json::json!({ "capability": name.unwrap_or_default() });
    serde_path_to_error::deserialize(json).map_err(|err| (err.path().to_string(), err.inner().to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use boa_engine::{JsValue, js_string, property::PropertyKey};
    use strum::VariantNames;

    use super::*;
    use crate::scripting::settings_eval::Scripts;

    /// `RunUserCommand` is internal: not an `action.*` (and not deserializable).
    fn action_names() -> BTreeSet<String> {
        CommandInstruction::VARIANTS
            .iter()
            .filter(|name| **name != "RunUserCommand")
            .map(|s| s.to_string())
            .collect()
    }

    /// Parameters for the actions that must be called (`builtin-constructor` in system.ts).
    fn sample_params(name: &str) -> Option<&'static str> {
        match name {
            "InsertText" => Some(r#"{ text: "x{||}" }"#),
            "SwitchToNote" => Some("{ index: 2 }"),
            _ => None,
        }
    }

    fn eval(scripts: &mut Scripts, code: &str) -> JsValue {
        scripts
            .js_cx
            .eval(boa_engine::Source::from_bytes(code))
            .unwrap_or_else(|err| panic!("{code}: {err}"))
    }

    /// system.ts's `action` object, exposed to scripts as a global for the test.
    fn actions(scripts: &mut Scripts) -> JsObject {
        let module = scripts.module_loader.get("shelv").expect("shelv is loaded");
        let namespace = module.namespace(&mut scripts.js_cx);
        let action = namespace.get(js_string!("action"), &mut scripts.js_cx).unwrap();
        scripts
            .js_cx
            .register_global_property(js_string!("action"), action.clone(), Default::default())
            .unwrap();
        action.as_object().unwrap()
    }

    #[test]
    fn system_ts_actions_match() {
        let mut scripts = Scripts::new();
        let action = actions(&mut scripts);

        let js_names: BTreeSet<String> = action
            .own_property_keys(&mut scripts.js_cx)
            .unwrap()
            .into_iter()
            .filter_map(|key| match key {
                PropertyKey::String(s) => Some(s.to_std_string_escaped()),
                _ => None,
            })
            .collect();
        assert_eq!(js_names, action_names(), "action.* in system.ts vs CommandInstruction variants");

        for name in &js_names {
            let tag = eval(&mut scripts, &format!("action.{name}.__shelv"))
                .as_string()
                .unwrap()
                .to_std_string_escaped();
            let code = match (tag.as_str(), sample_params(name)) {
                ("builtin", _) => format!("action.{name}"),
                ("builtin-constructor", Some(params)) => format!("action.{name}({params})"),
                ("builtin-constructor", None) => panic!("add sample params for action.{name}"),
                (other, _) => panic!("action.{name} has tag {other}"),
            };
            let value = eval(&mut scripts, &code).as_object().unwrap();
            if let Err((path, message)) = instruction_from_js(&value, &mut scripts.js_cx) {
                panic!("{code} does not decode: {path}: {message}");
            }
        }
    }

    /// The `//#region icons` of system.ts: every name `PhosphorIcon::from_string` accepts, kebab-case.
    /// `just update-icons` rewrites it after an egui-phosphor update (then regenerates system.d.ts).
    #[test]
    fn system_ts_icons_match() {
        const START: &str = "//#region icons\n";
        const END: &str = "//#endregion icons";

        let names: Vec<String> = egui_phosphor::light::ICONS
            .iter()
            .map(|(name, _)| name.to_lowercase().replace('_', "-"))
            .collect();
        // every generated name resolves in Rust (kebab-case → UPPER_SNAKE lookup)
        assert!(names.iter().all(|name| crate::command::PhosphorIcon::from_string(name).is_some()));

        let mut region = format!(
            "// GENERATED from egui_phosphor::light::ICONS by `system_ts_icons_match` (src/scripting/actions.rs).\n\
             // Do not edit by hand: run `just update-icons`.\n\
             // {} Phosphor icon names, as accepted by `Icon.phosphor(...)` and `PhosphorIcon::from_string`.\n\
             export type PhosphorIconName =\n",
            names.len()
        );
        for (i, name) in names.iter().enumerate() {
            let end = if i + 1 == names.len() { ";" } else { "" };
            region.push_str(&format!("\t| \"{name}\"{end}\n"));
        }

        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/scripting/system.ts");
        let source = std::fs::read_to_string(path).unwrap();
        let start = source.find(START).expect("system.ts has //#region icons") + START.len();
        let end = source[start..].find(END).expect("system.ts has //#endregion icons") + start;

        if source[start..end] == region {
            return;
        }
        if std::env::var_os("UPDATE_ICONS").is_some() {
            std::fs::write(path, format!("{}{region}{}", &source[..start], &source[end..])).unwrap();
            println!("rewrote the icons region of system.ts; now run `just build-system`");
        } else {
            panic!("the icons region of system.ts is out of date (egui-phosphor changed?): run `just update-icons`");
        }

    }

    /// Every capability system.ts exports (a value tagged `"capability"`) is a `Need` variant, and back.
    #[test]
    fn system_ts_capabilities_match() {
        let mut scripts = Scripts::new();
        let module = scripts.module_loader.get("shelv").expect("shelv is loaded");
        let namespace = module.namespace(&mut scripts.js_cx);

        let mut js_names = BTreeSet::new();
        for key in namespace.own_property_keys(&mut scripts.js_cx).unwrap() {
            let value = namespace.get(key.clone(), &mut scripts.js_cx).unwrap();
            let Some(object) = value.as_object() else { continue };
            let tag = object.get(js_string!("__shelv"), &mut scripts.js_cx).unwrap();
            if tag.as_string().is_some_and(|t| t.to_std_string_escaped() == "capability") {
                capability_from_js(&object, &mut scripts.js_cx)
                    .unwrap_or_else(|(_, message)| panic!("{key:?} does not decode: {message}"));
                let name = object
                    .get(js_string!("capability"), &mut scripts.js_cx)
                    .unwrap()
                    .as_string()
                    .unwrap()
                    .to_std_string_escaped();
                assert_eq!(format!("{key}"), name, "the export is named like its capability");
                js_names.insert(name);
            }
        }
        let rust_names: BTreeSet<String> = Need::VARIANTS.iter().map(|s| s.to_string()).collect();
        assert_eq!(js_names, rust_names, "capability exports of system.ts vs Need variants");
    }

    #[test]
    fn decodes_params_and_reports_paths() {
        let mut scripts = Scripts::new();
        actions(&mut scripts);
        let mut decode = |code: &str| {
            let value = eval(&mut scripts, code).as_object().unwrap();
            instruction_from_js(&value, &mut scripts.js_cx)
        };
        use CommandInstruction as CI;

        assert_eq!(decode("action.MarkdownBold"), Ok(CI::MarkdownBold));
        assert_eq!(decode("action.MarkdownCodeBlock"), Ok(CI::MarkdownCodeBlock { lang: None }));
        assert_eq!(
            decode(r#"action.MarkdownCodeBlock({ lang: "js" })"#),
            Ok(CI::MarkdownCodeBlock { lang: Some("js".into()) })
        );
        assert_eq!(decode("action.SwitchToNote({ index: 4 })"), Ok(CI::SwitchToNote { index: 3 }));
        assert_eq!(decode("action.ShowHideApp"), Ok(CI::ShowHideApp));

        // values system.ts would reject at the call; the Rust side checks them too
        let (path, message) =
            decode(r#"({ __shelv: "builtin", builtin: "SwitchToNote", params: { index: 7 } })"#).unwrap_err();
        assert_eq!((path.as_str(), message.as_str()), (".", "index must be one of 1, 2, 3, 4, got 7"));
        let (path, message) =
            decode(r#"({ __shelv: "builtin", builtin: "InsertText", params: { txt: "x" } })"#).unwrap_err();
        assert_eq!(path, ".");
        assert!(message.starts_with("unknown field `txt`"), "{message}");
        let (_, message) = decode(r#"({ __shelv: "builtin", builtin: "Nope", params: {} })"#).unwrap_err();
        assert!(message.starts_with("unknown variant `Nope`"), "{message}");
    }
}

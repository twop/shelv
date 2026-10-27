pub mod actions;
pub mod js_console_logger;
pub mod js_module_loader;
pub mod note_eval;
pub mod note_eval_context;
pub mod settings_decode;
pub mod settings_eval;
pub mod typescript;

use boa_engine::{Context, JsError};

/// `"TypeError: message"`, without the source position and stack that Boa's `Display` appends
/// (since 0.22); a thrown non-error value is shown as is.
pub fn js_error_message(err: &JsError, cx: &mut Context) -> String {
    if let Some(value) = err.as_opaque() {
        if !value.is_object() {
            return value.display().to_string();
        }
    }
    match err.try_native(cx) {
        Ok(native) if native.message().trim().is_empty() => native.kind().to_string(),
        Ok(native) => format!("{}: {}", native.kind(), native.message().trim()),
        Err(_) => err.to_string(),
    }
}

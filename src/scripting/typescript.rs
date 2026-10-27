//! TypeScript support: Boa runs JavaScript only, so `.ts` sources (`system.ts`, `default-settings.ts`,
//! ```ts blocks) have their types stripped first. Strip-only mode replaces type syntax with whitespace,
//! so line and column numbers in Boa's errors still point at the TypeScript as written.
//! Only erasable syntax is accepted (enum, namespace with values, parameter properties are errors),
//! matching `erasableSyntaxOnly` in src/scripting/tsconfig.json.

use std::sync::{Arc, Mutex};

use swc_common::{
    BytePos, GLOBALS, Globals, SourceMap, Spanned,
    errors::{DiagnosticBuilder, Emitter, Handler},
    sync::Lrc,
};
use swc_ts_fast_strip::{Mode, Options};

/// A positioned error from stripping (1-based line and column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsDiagnostic {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for TsDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

pub fn transpile_typescript_to_javascript(
    src: &str,
    module: Option<bool>,
) -> Result<String, Vec<TsDiagnostic>> {
    let collected: Arc<Mutex<Vec<(String, Option<BytePos>)>>> = Default::default();
    let cm: Lrc<SourceMap> = Default::default();

    let result = GLOBALS.set(&Globals::new(), || {
        let handler =
            Handler::with_emitter(false, false, Box::new(CollectingEmitter(collected.clone())));
        swc_ts_fast_strip::operate(
            &cm,
            &handler,
            src.to_string(),
            Options {
                module,
                mode: Mode::StripOnly,
                ..Default::default()
            },
        )
    });

    let diagnostics: Vec<TsDiagnostic> = collected
        .lock()
        .map(|list| {
            list.iter()
                .map(|(message, pos)| {
                    let (line, col) = pos
                        .map(|pos| {
                            let loc = cm.lookup_char_pos(pos);
                            (loc.line, loc.col.0 + 1)
                        })
                        .unwrap_or((1, 1));
                    TsDiagnostic {
                        line,
                        col,
                        message: message.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    match result {
        Ok(output) if diagnostics.is_empty() => Ok(output.code),
        Ok(_) => Err(diagnostics),
        Err(err) if diagnostics.is_empty() => Err(vec![TsDiagnostic {
            line: 1,
            col: 1,
            message: err.message,
        }]),
        Err(_) => Err(diagnostics),
    }
}

struct CollectingEmitter(Arc<Mutex<Vec<(String, Option<BytePos>)>>>);

impl Emitter for CollectingEmitter {
    fn emit(&mut self, db: &mut DiagnosticBuilder<'_>) {
        // The primary message, plus the label text when there is one ("enum is not supported ...").
        let mut message = db.message();
        for label in db.span.span_labels() {
            if let Some(text) = label.label {
                if !message.contains(&text) {
                    message = format!("{message}: {text}");
                }
            }
        }
        let pos = db.span.primary_span().map(|span| span.span_lo());
        if let Ok(mut list) = self.0.lock() {
            list.push((message, pos));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_types_and_keeps_positions() {
        let src = "const x: number = 1;\nexport const f = (a: string): string => a;\nthrow new Error(\"line 3\");\n";
        let js = strip_types(src).unwrap();
        assert_eq!(js.len(), src.len(), "stripping must not move code");
        assert!(!js.contains("number") && !js.contains("string"));
        assert_eq!(js.lines().nth(2), Some("throw new Error(\"line 3\");"));
    }

    #[test]
    fn rejects_non_erasable_syntax_with_position() {
        let errors = strip_types("const a = 1;\nenum E { A }\n").unwrap_err();
        assert_eq!(errors[0].line, 2, "{errors:?}");
        assert!(
            errors[0].message.to_lowercase().contains("enum"),
            "{errors:?}"
        );
    }

    #[test]
    fn reports_syntax_errors_with_position() {
        let errors = strip_types("const a = 1;\nconst b: = 2;\n").unwrap_err();
        assert_eq!(errors[0].line, 2, "{errors:?}");
    }

    #[test]
    fn embedded_sources_strip() {
        for (name, src) in [
            ("system.ts", include_str!("system.ts")),
            ("default-settings.ts", include_str!("default-settings.ts")),
        ] {
            if let Err(errors) = strip_types(src) {
                panic!("{name}: {errors:?}");
            }
        }
    }
}

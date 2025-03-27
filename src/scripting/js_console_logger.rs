use boa_engine::{js_str, Context, JsError, JsResult};
use boa_gc::{Finalize, Gc, GcRefCell, Trace};
use boa_runtime::{ConsoleState, Logger};

use std::fmt::{self, Display};

/// A logger that records all log messages.
#[derive(Clone, Debug, Trace, Finalize)]
pub struct JsLogCollector {
    log: Gc<GcRefCell<String>>,
}

/// An error returned by [`GcCell::try_borrow_mut`](struct.GcCell.html#method.try_borrow_mut).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default, Hash)]
pub struct BorrowMutError;

impl Display for BorrowMutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt("GcCell<T> already borrowed", f)
    }
}

impl JsLogCollector {
    pub fn new() -> Self {
        Self {
            log: Gc::new(GcRefCell::new("".to_string())),
        }
    }
    pub fn flush(&self) -> Result<String, BorrowMutError> {
        let mut log_ref_cell = self.log.try_borrow_mut().map_err(|_| BorrowMutError)?;

        let logged = log_ref_cell.split_off(0);
        Ok(logged)
    }
}

impl Logger for JsLogCollector {
    fn log(&self, msg: String, state: &ConsoleState, _: &mut Context) -> JsResult<()> {
        let indent = state.indent();
        let mut logged = self
            .log
            .try_borrow_mut()
            .map_err(|_| JsError::from_opaque(js_str!("cannot derefernce logger").into()))?;

        logged.push_str(&format!("{msg:>indent$}\n"));

        Ok(())
        // writeln!(self.log.borrow_mut(), "{msg:>indent$}").map_err(JsError::from_rust)
    }

    fn info(&self, msg: String, state: &ConsoleState, context: &mut Context) -> JsResult<()> {
        self.log(msg, state, context)
    }

    fn warn(&self, msg: String, state: &ConsoleState, context: &mut Context) -> JsResult<()> {
        self.log(msg, state, context)
    }

    fn error(&self, msg: String, state: &ConsoleState, context: &mut Context) -> JsResult<()> {
        self.log(msg, state, context)
    }
}

// impl Trace for JsLogCollector {
//     unsafe fn trace(&self, tracer: &mut boa_engine::gc::Tracer) {
//         todo!()
//     }

//     unsafe fn trace_non_roots(&self) {
//         todo!()
//     }

//     fn run_finalizer(&self) {
//         todo!()
//     }
// }

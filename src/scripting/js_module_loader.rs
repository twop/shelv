use std::{cell::RefCell, rc::Rc};

use boa_engine::{
    Context, JsNativeError, JsResult, Module,
    gc::GcRefCell,
    module::{ModuleLoader, ModuleRequest, Referrer},
};
use fxhash::FxHashMap;

#[derive(Debug)]
pub struct InMemoryModuleLoader {
    module_map: GcRefCell<FxHashMap<String, Module>>,
}

impl InMemoryModuleLoader {
    /// Creates a new `InMemoryModuleLoader`.
    pub fn new() -> Self {
        Self {
            module_map: GcRefCell::default(),
        }
    }

    /// Inserts a new module into the module map.
    #[inline]
    pub fn insert(&self, name: String, module: Module) {
        self.module_map.borrow_mut().insert(name, module);
    }

    /// Gets a module by its name.
    #[inline]
    pub fn get(&self, name: &str) -> Option<Module> {
        self.module_map.borrow().get(name).cloned()
    }
}

impl ModuleLoader for InMemoryModuleLoader {
    async fn load_imported_module(
        self: Rc<Self>,
        _referrer: Referrer,
        request: ModuleRequest,
        _context: &RefCell<&mut Context>,
    ) -> JsResult<Module> {
        let specifier = request.specifier().to_std_string_escaped();
        self.get(&specifier).ok_or_else(|| {
            JsNativeError::typ()
                .with_message(format!("Module '{specifier}' not found"))
                .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boa_engine::{Context, Source, js_str};

    #[test]
    fn test_missing_module() {
        let loader = Rc::new(InMemoryModuleLoader::new());
        let mut context = Context::builder().module_loader(loader).build().unwrap();

        let source = r#"
            import { foo } from 'missing.js';
            foo();
        "#;

        let result = context.eval(Source::from_bytes(source));
        assert!(result.is_err());
    }

    #[test]
    fn test_successful_module_import() {
        let loader = Rc::new(InMemoryModuleLoader::new());
        let mut context = Context::builder()
            .module_loader(loader.clone())
            .build()
            .unwrap();

        // Register module
        let module_code = r#"
            export function add(a, b) {
                return a + b;
            }
        "#;

        let module = Module::parse(Source::from_bytes(module_code), None, &mut context).unwrap();
        loader.insert("math.js".to_string(), module);

        // Import and use module
        let source = r#"
            import { add } from 'math.js';
            export const call = ()=> add(2, 3);
        "#;

        let module = Module::parse(Source::from_bytes(source), None, &mut context).unwrap();
        let _promise = module.load_link_evaluate(&mut context);
        context.run_jobs().unwrap();
        let namespace = module.namespace(&mut context);
        let call_fn = namespace.get(js_str!("call"), &mut context).unwrap();
        let result = call_fn
            .as_callable()
            .unwrap()
            .call(&call_fn, &[], &mut context)
            .unwrap();
        assert_eq!(result.as_number().unwrap(), 5.0);
    }
}

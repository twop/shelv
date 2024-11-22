use boa_engine::{
    gc::GcRefCell,
    module::{ModuleLoader, Referrer},
    Context, JsNativeError, JsResult, JsString, Module,
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
    fn load_imported_module(
        &self,
        _referrer: Referrer,
        specifier: JsString,
        finish_load: Box<dyn FnOnce(JsResult<Module>, &mut Context)>,
        context: &mut Context,
    ) {
        let result = if let Some(module) = self.get(&specifier.to_std_string_escaped()) {
            Ok(module)
        } else {
            Err(JsNativeError::typ()
                .with_message(format!(
                    "Module '{}' not found",
                    specifier.to_std_string_escaped()
                ))
                .into())
        };

        finish_load(result, context);
    }

    fn register_module(&self, specifier: JsString, module: Module) {
        self.insert(specifier.to_std_string_escaped(), module);
    }

    fn get_module(&self, specifier: JsString) -> Option<Module> {
        self.get(&specifier.to_std_string_escaped())
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use boa_engine::{js_str, js_string, Context, Source};

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
        loader.register_module(js_string!("math.js"), module);

        // Import and use module
        let source = r#"
            import { add } from 'math.js';
            export const call = ()=> add(2, 3);
        "#;

        let module = Module::parse(Source::from_bytes(source), None, &mut context).unwrap();
        let _promise = module.load_link_evaluate(&mut context);
        context.run_jobs();
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

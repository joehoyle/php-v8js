use ext_php_rs::binary::Binary;
use ext_php_rs::builders::ClassBuilder;
use ext_php_rs::convert::{FromZval, IntoZval};
use ext_php_rs::flags::DataType;
use ext_php_rs::types::{ZendHashTable, ZendObject, Zval};
use ext_php_rs::zend::{ClassEntry, ModuleEntry};
use ext_php_rs::{exception::PhpException, zend::ce};
use ext_php_rs::{info_table_end, info_table_row, info_table_start, prelude::*};

use std::collections::HashMap;

mod runtime;

pub use crate::runtime::JSRuntime;
pub use crate::runtime::Error as RuntimeError;

static mut V8JS_TIME_LIMIT_EXCEPTION: Option<&'static ClassEntry> = None;
static mut V8JS_MEMORY_LIMIT_EXCEPTION: Option<&'static ClassEntry> = None;
static mut V8JS_SCRIPT_EXCEPTION: Option<&'static ClassEntry> = None;

pub fn zval_from_jsvalue(result: v8::Local<v8::Value>, scope: &mut v8::HandleScope) -> Zval {
    if result.is_string() {
        return result.to_rust_string_lossy(scope).try_into().unwrap();
    }
    if result.is_null_or_undefined() {
        let mut zval = Zval::new();
        zval.set_null();
        return zval;
    }
    if result.is_boolean() {
        return result.boolean_value(scope).into();
    }
    if result.is_int32() {
        return result.integer_value(scope).unwrap().try_into().unwrap();
    }
    if result.is_number() {
        return result.number_value(scope).unwrap().into();
    }
    if result.is_array() {
        let array = v8::Local::<v8::Array>::try_from(result).unwrap();
        let mut zend_array = ZendHashTable::new();
        for index in 0..array.length() {
            let _result = zend_array.push(zval_from_jsvalue(
                array.get_index(scope, index).unwrap(),
                scope,
            ));
        }
        let mut zval = Zval::new();
        zval.set_hashtable(zend_array);
        return zval;
    }
    if result.is_function() {
        return "Function".try_into().unwrap();
    }
    if result.is_object() {
        let object = v8::Local::<v8::Object>::try_from(result).unwrap();
        let properties = object.get_own_property_names(scope).unwrap();
        let class_entry = ClassEntry::try_find("V8Object").unwrap();
        let mut zend_object = ZendObject::new(class_entry);
        for index in 0..properties.length() {
            let key = properties.get_index(scope, index).unwrap();
            let value = object.get(scope, key).unwrap();

            zend_object
                .set_property(
                    key.to_rust_string_lossy(scope).as_str(),
                    zval_from_jsvalue(value, scope),
                )
                .unwrap();
        }
        return zend_object.into_zval(false).unwrap();
    }
    result.to_rust_string_lossy(scope).try_into().unwrap()
}

#[php_class]
#[extends(ce::exception())]
#[derive(Default)]
pub struct V8JsScriptException;

pub fn js_value_from_zval<'a>(
    scope: &mut v8::HandleScope<'a>,
    zval: &'_ Zval,
) -> v8::Local<'a, v8::Value> {
    if zval.is_string() {
        return v8::String::new(scope, zval.str().unwrap()).unwrap().into();
    }
    if zval.is_long() || zval.is_double() {
        return v8::Number::new(scope, zval.double().unwrap()).into();
    }
    if zval.is_bool() {
        return v8::Boolean::new(scope, zval.bool().unwrap()).into();
    }
    if zval.is_true() {
        return v8::Boolean::new(scope, true).into();
    }
    if zval.is_false() {
        return v8::Boolean::new(scope, false).into();
    }
    if zval.is_null() {
        return v8::null(scope).into();
    }
    if zval.is_array() {
        let zend_array = zval.array().unwrap();
        let mut values: Vec<v8::Local<'_, v8::Value>> = Vec::new();
        let mut keys: Vec<v8::Local<'_, v8::Name>> = Vec::new();
        let mut has_string_keys = false;
        for (index, key, elem) in zend_array.iter() {
            let key = match key {
                Some(key) => {
                    has_string_keys = true;
                    key
                }
                None => index.to_string(),
            };
            keys.push(v8::String::new(scope, key.as_str()).unwrap().into());
            values.push(js_value_from_zval(scope, elem));
        }

        if has_string_keys {
            let null: v8::Local<v8::Value> = v8::null(scope).into();
            return v8::Object::with_prototype_and_properties(scope, null, &keys[..], &values[..])
                .into();
        } else {
            return v8::Array::new_with_elements(scope, &values[..]).into();
        }
    }
    // Todo: is_object
    v8::null(scope).into()
}

#[php_class(modifier = "v8js_modifier")]
pub struct V8Js {
    global_name: String,
    runtime: JSRuntime,
    user_properties: HashMap<String, Zval>,
}

fn v8js_modifier(class: ClassBuilder) -> ext_php_rs::error::Result<ClassBuilder> {
    class.constant("V8_VERSION", v8::V8::get_version())
}

#[php_impl(rename_methods = "camelCase")]
impl V8Js {
    pub fn __construct(
        object_name: Option<String>,
        _variables: Option<HashMap<String, String>>,
        _extensions: Option<HashMap<String, String>>,
        _report_uncaight_exceptions: Option<bool>,
        snapshot_blob: Option<Binary<u8>>,
    ) -> Self {
        let global_name = match object_name {
            Some(name) => name,
            None => String::from("PHP"),
        };
        let snapshot_blob = match snapshot_blob {
            Some(snapshot_blob) => Some(snapshot_blob.as_slice().to_vec()),
            None => None,
        };
        let mut runtime = JSRuntime::new(snapshot_blob);
        let object: v8::Global<v8::Value>;
        {
            let scope = &mut runtime.handle_scope();
            let o: v8::Local<v8::Value> = v8::Object::new(scope).into();
            object = v8::Global::new(scope, o);
        }
        runtime.add_global(global_name.as_str(), object);
        runtime.add_global_function("var_dump", php_callback_var_dump);
        runtime.add_global_function("print", php_callback_var_dump);
        runtime.add_global_function("exit", php_callback_exit);
        runtime.add_global_function("sleep", php_callback_sleep);
        runtime.add_global_function("require", php_callback_require);
        V8Js {
            runtime,
            global_name,
            user_properties: HashMap::new(),
        }
    }

    pub fn set_module_loader(&mut self, callable: &Zval) {
        let state = self.runtime.get_state();
        let mut state = state.borrow_mut();
        state.commonjs_module_loader = Some(callable.shallow_clone());
    }

    pub fn execute_string(
        &mut self,
        string: String,
        identifier: Option<String>,
        _flags: Option<String>,
        time_limit: Option<u64>,
        memory_limit: Option<u64>,
    ) -> Result<Zval, PhpException> {
        let result = self.runtime.execute_string(
            string.as_str(),
            identifier,
            _flags,
            time_limit,
            memory_limit,
        );

        match result {
            Ok(result) => match result {
                Some(result) => {
                    let mut scope = &mut self.runtime.handle_scope();
                    let local = v8::Local::new(scope, result);
                    Ok(zval_from_jsvalue(local, &mut scope))
                }
                None => {
                    let mut zval = Zval::new();
                    zval.set_null();
                    Ok(zval)
                }
            },
            Err(e) => {
                match e {
                    RuntimeError::ExecutionTimeout => {
                        Err(PhpException::new("".into(), 0, unsafe{ V8JS_TIME_LIMIT_EXCEPTION.unwrap() } ))
                    },
                    RuntimeError::MemoryLimitExceeded => {
                        Err(PhpException::new("".into(), 0, unsafe{ V8JS_MEMORY_LIMIT_EXCEPTION.unwrap() } ))
                    },
                    RuntimeError::ScriptExecutionError(error) => {
                        Err(PhpException::new(error.message.into(), 0, unsafe{ V8JS_SCRIPT_EXCEPTION.unwrap() } ))
                    }
                    _ => Err(PhpException::default(String::from("Unknown error.")))
                }
            }
        }
    }

    pub fn __set(&mut self, property: &str, value: &Zval) {
        {
            let global = self.runtime.get_global(self.global_name.as_str());
            let global = match global {
                Some(global) => global,
                None => return (),
            };
            let mut scope = self.runtime.handle_scope();
            let global = v8::Local::new(&mut scope, global);
            let global: v8::Local<v8::Object> = v8::Local::<v8::Object>::try_from(global).unwrap();
            let property_name = v8::String::new(&mut scope, property).unwrap();

            let js_value;
            if value.is_callable() {
                let function_builder: v8::FunctionBuilder<v8::Function> =
                    v8::FunctionBuilder::new(php_callback);
                let function_builder = function_builder.data(property_name.into());
                let function: v8::Local<v8::Value> =
                    function_builder.build(&mut scope).unwrap().into();
                js_value = function;
            } else {
                js_value = js_value_from_zval(&mut scope, value);
            }
            global.set(&mut scope, property_name.into(), js_value);
        }
        if value.is_callable() {
            let value = value.shallow_clone();
            self.runtime.add_callback(property, value);
        }
        self.user_properties
            .insert(property.into(), value.shallow_clone());
    }

    pub fn __get(&mut self, property: &str) -> Option<Zval> {
        match self.user_properties.get(property.into()) {
            Some(zval) => Some(zval.shallow_clone()),
            None => None,
        }
    }

    pub fn create_snapshot(source: String) -> Option<Zval> {
        let snapshot = JSRuntime::create_snapshot(source)?;
        let mut zval = Zval::new();
        zval.set_binary(snapshot);
        Some(zval)
    }
}

// Zval doesn't implement Clone, which means that Zval's can not
// be passed to `ZendCallable.try_call()`, so we have to wrap it
// in a Cloneable wrapper.
#[derive(Debug)]
struct CloneableZval(Zval);

impl FromZval<'_> for CloneableZval {
    const TYPE: DataType = DataType::Mixed;
    fn from_zval(zval: &'_ Zval) -> Option<Self> {
        Some(Self(zval.shallow_clone()))
    }
}

impl IntoZval for CloneableZval {
    const TYPE: DataType = DataType::Mixed;
    fn set_zval(self, zv: &mut Zval, _: bool) -> ext_php_rs::error::Result<()> {
        *zv = self.0;
        Ok(())
    }
    fn into_zval(self, _persistent: bool) -> ext_php_rs::error::Result<Zval> {
        Ok(self.0)
    }
}

impl Clone for CloneableZval {
    fn clone(&self) -> Self {
        Self(self.0.shallow_clone())
    }
}

pub fn php_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let isolate: &mut v8::Isolate = scope.as_mut();
    let state = JSRuntime::state(isolate);
    let state = state.borrow_mut();
    let callback_name = args.data().unwrap().to_rust_string_lossy(scope);
    let callback = state.callbacks.get(&callback_name);
    if callback.is_none() {
        println!("callback not found {:#?}", callback_name);
        return;
    }
    let callback = callback.unwrap();

    if callback.is_callable() == false {
        println!("callback not callable {:#?}", callback);
        return;
    }

    let mut php_args: Vec<CloneableZval> = Vec::new();
    let mut php_args_refs: Vec<&dyn ext_php_rs::convert::IntoZvalDyn> = Vec::new();
    for index in 0..args.length() {
        let v = zval_from_jsvalue(args.get(index), scope);
        let clonable_zval = CloneableZval::from_zval(&v).unwrap();
        php_args.push(clonable_zval);
    }
    for index in 0..php_args.len() {
        php_args_refs.push(php_args.get(index).unwrap());
    }
    let return_value = callback.try_call(php_args_refs).unwrap();
    let return_value_js = js_value_from_zval(scope, &return_value);
    rv.set(return_value_js)
}

pub fn php_callback_sleep(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let sleep = ext_php_rs::types::ZendCallable::try_from_name("sleep").unwrap();
    let arg = CloneableZval::from_zval(&zval_from_jsvalue(args.get(0), scope));
    let result = sleep.try_call(vec![&arg]);
    let result = match result {
        Ok(result) => result,
        Err(_) => Zval::new(), // todo: JS error objects?
    };
    let result = js_value_from_zval(scope, &result);
    rv.set(result);
}

pub fn php_callback_var_dump(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let var_dump = ext_php_rs::types::ZendCallable::try_from_name("var_dump").unwrap();
    let arg = CloneableZval::from_zval(&zval_from_jsvalue(args.get(0), scope));
    let result = var_dump.try_call(vec![&arg]);
    let result = match result {
        Ok(result) => result,
        Err(_) => Zval::new(), // todo: JS error objects?
    };
    let result = js_value_from_zval(scope, &result);
    rv.set(result);
}

pub fn php_callback_exit(
    scope: &mut v8::HandleScope,
    _args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue,
) {
    if scope.is_execution_terminating() {
        return ();
    }

    // There's no way to immediately terminate execution in V8 so
    // we have to spin it's wheels with an inf. loop until it terminates.
    let script;
    {
        let code = v8::String::new(scope, "for(;;);").unwrap();
        script = v8::Script::compile(scope, code, None).unwrap();
    }
    scope.terminate_execution();
    script.run(scope);
}

pub fn php_callback_require(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let module_name = args.get(0).to_rust_string_lossy(scope);
    let isolate: &mut v8::Isolate = scope.as_mut();
    let state = JSRuntime::state(isolate);
    let mut state = state.borrow_mut();

    let module = match state.commonjs_modules.get(&module_name) {
        Some(module) => module.clone(),
        None => {
            let commonjs_module_loader = &state.commonjs_module_loader;
            if commonjs_module_loader.is_none() {
                return ();
            }

            let module_code = commonjs_module_loader
                .as_ref()
                .unwrap()
                .try_call(vec![&module_name]);

            if module_code.is_err() {
                return (); // todo
            }
            let module_code = module_code.unwrap().string().unwrap();
            let module_code = format!(
                "{}{}{}",
                "(function (exports, module) {", module_code, "\n});"
            );
            let module_code = v8::String::new(scope, module_code.as_str()).unwrap();
            let script = v8::Script::compile(scope, module_code, None).unwrap();
            let result: v8::Local<v8::Function> = script.run(scope).unwrap().try_into().unwrap(); // todo
            let module = v8::Object::new(scope);
            let exports = v8::Object::new(scope);
            let name: v8::Local<v8::Value> = v8::String::new(scope, "exports").unwrap().into();
            module.set(scope, name, exports.into());
            result.call(scope, module.into(), &[exports.into(), module.into()]);

            let exports = module.get(scope, name);
            if exports.is_none() {
                return (); // todo
            }
            let exports = exports.unwrap();
            let module: v8::Global<v8::Value> = v8::Global::new(scope, exports);
            state
                .commonjs_modules
                .insert(module_name.to_string(), module.clone());
            module
        }
    };

    let local = v8::Local::new(scope, module);
    rv.set(local);
}

#[php_class]
pub struct V8Object {}

/// Used by the `phpinfo()` function and when you run `php -i`.
/// This will probably be simplified with another macro eventually!
pub extern "C" fn php_module_info(_module: *mut ModuleEntry) {
    info_table_start!();
    info_table_row!("V8 Javascript Engine", "enabled");
    info_table_row!("Version", env!("CARGO_PKG_VERSION"));
    info_table_row!("V8 Version", v8::V8::get_version());
    info_table_end!();
}


#[php_startup]
pub fn startup() {
    let ce = ClassBuilder::new("V8JsTimeLimitException")
        .extends(ce::exception())
        .build()
        .expect("Failed to build V8JsTimeLimitException");
    unsafe { V8JS_TIME_LIMIT_EXCEPTION.replace(ce) };

    let ce = ClassBuilder::new("V8JsMemoryLimitException")
        .extends(ce::exception())
        .build()
        .expect("Failed to build V8JsMemoryLimitException");
    unsafe { V8JS_MEMORY_LIMIT_EXCEPTION.replace(ce) };

    let ce = ClassBuilder::new("V8JsScriptException")
        .extends(ce::exception())
        .build()
        .expect("Failed to build V8JsScriptException");
    unsafe { V8JS_SCRIPT_EXCEPTION.replace(ce) };
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module.info_function(php_module_info)
}

#[cfg(test)]
mod integration {
    use std::process::Command;
    use std::sync::Once;

    static BUILD: Once = Once::new();

    fn setup() {
        BUILD.call_once(|| {
            assert!(Command::new("cargo")
                .arg("build")
                .output()
                .expect("failed to build extension")
                .status
                .success());
        });
    }

    pub fn run_php(file: &str) -> bool {
        setup();
        let output = Command::new("php")
            .arg(format!(
                "-dextension=target/debug/libv8js.{}",
                std::env::consts::DLL_EXTENSION
            ))
            .arg("-n")
            .arg(format!("tests/{}", file))
            .output()
            .expect("failed to run php file");
        if output.status.success() {
            true
        } else {
            panic!(
                "
                status: {}
                stdout: {}
                stderr: {}
                ",
                output.status,
                String::from_utf8(output.stdout).unwrap(),
                String::from_utf8(output.stderr).unwrap()
            );
        }
    }
    #[test]
    fn snapshot() {
        run_php("snapshot.php");
    }

    #[test]
    fn execute_string() {
        run_php("execute_string.php");
    }

    #[test]
    fn php_bridge() {
        run_php("php_bridge.php");
    }

    #[test]
    fn global_functions() {
        run_php("global_functions.php");
    }

    #[test]
    fn js_bridge() {
        run_php("js_bridge.php");
    }

    #[test]
    fn commonjs_modules() {
        run_php("commonjs_modules.php");
    }

    #[test]
    fn time_limit() {
        run_php("time_limit.php");
    }

    #[test]
    fn memory_limit() {
        run_php("memory_limit.php");
    }

    #[test]
    fn v8js_version() {
        run_php("version.php");
    }
}

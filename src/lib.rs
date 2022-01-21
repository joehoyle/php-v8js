use ext_php_rs::binary::Binary;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;
use ext_php_rs::{exception::PhpException, zend::ce};
use std::collections::HashMap;

#[derive(ZvalConvert, Debug, Clone)]
pub enum PHPValue {
    String(String),
    None,
    Boolean(bool),
    Float(f64),
    Integer(i64),
    Array(Vec<PHPValue>),
    Object(HashMap<String, PHPValue>),
}

impl PHPValue {
    pub fn new(result: v8::Local<'_, v8::Value>, scope: &mut v8::HandleScope) -> Self {
        if result.is_string() {
            return PHPValue::String(result.to_rust_string_lossy(scope));
        }
        if result.is_null_or_undefined() {
            return PHPValue::None;
        }
        if result.is_boolean() {
            return PHPValue::Boolean(result.boolean_value(scope));
        }
        if result.is_int32() {
            return PHPValue::Integer(result.integer_value(scope).unwrap());
        }
        if result.is_number() {
            return PHPValue::Float(result.number_value(scope).unwrap());
        }
        if result.is_array() {
            let array = v8::Local::<v8::Array>::try_from(result).unwrap();
            let mut vec: Vec<PHPValue> = Vec::new();
            for index in 0..array.length() {
                vec.push(PHPValue::new(array.get_index(scope, index).unwrap(), scope));
            }
            return PHPValue::Array(vec);
        }
        if result.is_function() {
            return PHPValue::String(String::from("Function"));
        }
        if result.is_object() {
            let object = v8::Local::<v8::Object>::try_from(result).unwrap();
            let properties = object.get_own_property_names(scope).unwrap();
            let mut hashmap: HashMap<String, PHPValue> = HashMap::new();
            for index in 0..properties.length() {
                let key = properties.get_index(scope, index).unwrap();
                let value = object.get(scope, key).unwrap();
                hashmap.insert(key.to_rust_string_lossy(scope), PHPValue::new(value, scope));
            }
            return PHPValue::Object(hashmap);
        }
        PHPValue::String(result.to_rust_string_lossy(scope))
    }
}

#[php_class]
pub struct V8Js {
    global_name: String,
    runtime: JSRuntime,
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
        println!("{:?}", zval);
        return v8::Boolean::new(scope, zval.bool().unwrap()).into();
    }
    if zval.is_true() {
        println!("is true");
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
    if zval.is_callable() {
        let ptr = zval as *const _;
        let external = v8::External::new(scope, ptr as *mut std::ffi::c_void);
        let function_builder: v8::FunctionBuilder<v8::Function> =
            v8::FunctionBuilder::new(php_callback);
        let function_builder = function_builder.data(external.into());
        let function = function_builder.build(scope).unwrap();
        return function.into();
    }
    v8::null(scope).into()
}

pub struct JSRuntime {
    pub isolate: v8::OwnedIsolate,
    pub context: v8::Global<v8::Context>,
}

impl JSRuntime {
    pub fn new(snapshot_blob: Option<Binary<u8>>) -> Self {
        // The V8 Platform should only ever be intitialized once.
        static START: std::sync::Once = std::sync::Once::new();
        START.call_once(|| {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });

        let mut create_params = v8::CreateParams::default();
        // Restore the snapshot if one was provided. We have to map
        // ext_php_rs Binary data to u8 slices.
        if snapshot_blob.is_some() {
            let vec_data: Vec<u8> = snapshot_blob.unwrap().into();
            create_params = create_params.snapshot_blob(vec_data);
        }
        let mut isolate = v8::Isolate::new(create_params);
        let global_context;
        {
            let scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Context::new(scope);
            global_context = v8::Global::new(scope, context);
        }
        JSRuntime {
            isolate,
            context: global_context,
        }
    }

    pub fn add_global(&mut self, name: &str) {
        let scope = &mut v8::HandleScope::new(&mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let global = v8::String::new(scope, name).unwrap();
        let global_object = v8::Object::new(scope);
        let global_scope = context.global(scope);
        global_scope.set(scope, global.into(), global_object.into());
    }

    pub fn add_global_function(
        &mut self,
        name: &str,
        callback: impl v8::MapFnTo<v8::FunctionCallback>,
    ) {
        let scope = &mut v8::HandleScope::new(&mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let global = v8::String::new(scope, name).unwrap();
        let global_scope = context.global(scope);
        let mut function_builder: v8::FunctionBuilder<v8::Function> =
            v8::FunctionBuilder::new(callback);
        let function = function_builder.build(scope).unwrap();
        global_scope.set(scope, global.into(), function.into());
    }

    pub fn execute_string(
        &mut self,
        code: &str,
        identifier: Option<String>,
        _flags: Option<String>,
        time_limit: Option<u64>,
        memory_limit: Option<u64>,
    ) -> Result<PHPValue, PhpException> {
        let isolate_handle = self.isolate.thread_safe_handle();
        let scope = &mut v8::HandleScope::new(&mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let code = v8::String::new(scope, code).ok_or(PhpException::default(
            "Unable to allocate code string.".to_string(),
        ))?;

        let resource_name = v8::String::new(
            scope,
            identifier.unwrap_or("V8Js::executeString".into()).as_str(),
        )
        .unwrap();
        let source_map_url = v8::String::new(scope, "source_map_url").unwrap();
        let script_origin = v8::ScriptOrigin::new(
            scope,
            resource_name.into(),
            0,
            0,
            false,
            123,
            source_map_url.into(),
            false,
            false,
            false,
        );
        let script = v8::Script::compile(scope, code, Some(&script_origin))
            .ok_or(PhpException::default("Unable to compile code.".to_string()))?;
        let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let time_limit_hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let memory_limit_hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        // If we have time / memory limits, we have to spawn a monitoring thread
        // to periodically check the run time / memory usage of V8.
        if memory_limit.is_some() || time_limit.is_some() {
            std::thread::spawn({
                let should_i_stop = stop_flag.clone();
                let time_limit_hit = time_limit_hit.clone();
                let memory_limit_hit = memory_limit_hit.clone();
                let start = std::time::Instant::now();
                let time_limit = std::time::Duration::from_millis(time_limit.unwrap_or(0));
                let memory_limit = memory_limit.unwrap_or(0);
                static MEMORY_LIMIT_HIT_CALLBACK: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                move || {
                    // Callbacl function that is passed to V8. This is not able to catpure
                    // anythign locally, so we use a static to flag whether the memory limit is
                    // hit. The c_void pointer called in to the callback is used to pass the
                    // memory limit reference.
                    extern "C" fn callback(isolate: &mut v8::Isolate, data: *mut std::ffi::c_void) {
                        let mut statistics = v8::HeapStatistics::default();
                        isolate.get_heap_statistics(&mut statistics);
                        let memory_limit: &mut usize = unsafe { &mut *(data as *mut usize) };
                        if statistics.used_heap_size() > *memory_limit {
                            MEMORY_LIMIT_HIT_CALLBACK
                                .store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                    while !should_i_stop.load(std::sync::atomic::Ordering::SeqCst) {
                        if time_limit.as_millis() > 0 {
                            if start.elapsed() > time_limit
                                && !isolate_handle.is_execution_terminating()
                            {
                                time_limit_hit.store(true, std::sync::atomic::Ordering::SeqCst);
                                isolate_handle.terminate_execution();
                                break;
                            }
                        }
                        if memory_limit > 0 {
                            if MEMORY_LIMIT_HIT_CALLBACK.load(std::sync::atomic::Ordering::SeqCst) {
                                memory_limit_hit.store(true, std::sync::atomic::Ordering::SeqCst);
                                isolate_handle.terminate_execution();
                                break;
                            } else {
                                let ptr = &memory_limit as *const _ as *mut std::ffi::c_void;
                                isolate_handle.request_interrupt(callback, ptr);
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            });
        }
        let result = script.run(scope);
        stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        let result = match result {
            Some(result) => Ok(result),
            None => {
                let time_limit_hit = time_limit_hit.load(std::sync::atomic::Ordering::SeqCst);
                let memory_limit_hit = memory_limit_hit.load(std::sync::atomic::Ordering::SeqCst);
                if time_limit_hit {
                    Err(PhpException::default("Time limit exceeded.".to_string()))
                } else if memory_limit_hit {
                    Err(PhpException::default("Memory limit exceeded.".to_string()))
                } else {
                    Err(PhpException::default("Unable to run code.".to_string()))
                }
            }
        }?;

        stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(PHPValue::new(result, scope))
    }
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
        let mut runtime = JSRuntime::new(snapshot_blob);
        let print = |scope: &mut v8::HandleScope,
                     args: v8::FunctionCallbackArguments,
                     _rv: v8::ReturnValue| {
            let php_print = ext_php_rs::types::ZendCallable::try_from_name("var_dump").unwrap();
            let arg = PHPValue::new(args.get(0), scope);
            let mut php_arg_refs: Vec<&dyn ext_php_rs::convert::IntoZvalDyn> = Vec::new();
            php_arg_refs.push(&arg);
            let result = php_print.try_call(php_arg_refs);
        };

        runtime.add_global(global_name.as_str());
        runtime.add_global_function("print", print);

        let var_dump = |scope: &mut v8::HandleScope,
                        args: v8::FunctionCallbackArguments,
                        _rv: v8::ReturnValue| {
            let var_dump = ext_php_rs::types::ZendCallable::try_from_name("var_dump").unwrap();
            let arg = PHPValue::new(args.get(0), scope);
            let mut php_arg_refs: Vec<&dyn ext_php_rs::convert::IntoZvalDyn> = Vec::new();
            php_arg_refs.push(&arg);
            let result = var_dump.try_call(php_arg_refs);
        };

        runtime.add_global_function("var_dump", var_dump);

        let exit = |scope: &mut v8::HandleScope,
                    _args: v8::FunctionCallbackArguments,
                    _rv: v8::ReturnValue| {
            scope.terminate_execution();
        };
        runtime.add_global_function("exit", exit);

        V8Js {
            runtime,
            global_name,
        }
    }
    pub fn execute_string(
        &mut self,
        string: String,
        identifier: Option<String>,
        _flags: Option<String>,
        time_limit: Option<u64>,
        memory_limit: Option<u64>,
    ) -> Result<PHPValue, PhpException> {
        self.runtime.execute_string(
            string.as_str(),
            identifier,
            _flags,
            time_limit,
            memory_limit,
        )
    }

    pub fn __set(&mut self, property: &str, value: &Zval) {
        let scope = &mut v8::HandleScope::new(&mut self.runtime.isolate);
        let context = v8::Local::new(scope, &self.runtime.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let global_name = v8::String::new(scope, self.global_name.as_str()).unwrap();
        let global_object = context
            .global(scope)
            .get(scope, global_name.into())
            .unwrap();
        let property_name = v8::String::new(scope, property).unwrap();
        let global_object = v8::Local::<v8::Object>::try_from(global_object).unwrap();

        let js_value: v8::Local<'_, v8::Value>;
        js_value = js_value_from_zval(scope, value);
        global_object.set(scope, property_name.into(), js_value);
    }

    pub fn create_snapshot(source: String) -> Option<Zval> {
        let mut snapshot_creator = v8::SnapshotCreator::new(None);
        let mut isolate = unsafe { snapshot_creator.get_owned_isolate() };
        {
            let scope = &mut v8::HandleScope::new(&mut isolate);
            let c = v8::Context::new(scope);
            let cg = v8::Local::new(scope, c);
            let context = v8::Global::new(scope, cg);
            let context = v8::Local::new(scope, context);
            let scope = &mut v8::ContextScope::new(scope, context);
            let code = match v8::String::new(scope, source.as_str()) {
                Some(s) => s,
                None => return None,
            };

            let script = v8::Script::compile(scope, code, None);
            let script = match script {
                Some(s) => s,
                None => return None,
            };

            script.run(scope);
            snapshot_creator.set_default_context(context);
        }
        // The isolate must be dropped, else PHP will segfault.
        std::mem::forget(isolate);
        let blob = snapshot_creator.create_blob(v8::FunctionCodeHandling::Clear);
        let startup_data = match blob {
            Some(data) => data,
            None => return None,
        };
        let snapshot_slice: &[u8] = &*startup_data;

        let mut zval = Zval::new();
        zval.set_binary(snapshot_slice.into());
        Some(zval)
    }
}
#[derive(Debug)]
struct StartupData {
    data: *const char,
    raw_size: std::os::raw::c_int,
}

pub fn php_callback(
    scope: &mut v8::HandleScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue,
) {
    let external = args.data().unwrap();
    let external = unsafe { v8::Local::<v8::External>::cast(external) };
    let ptr = external.value();
    let zval: &Zval = unsafe { std::mem::transmute(ptr) };
    let mut php_args: Vec<PHPValue> = Vec::new();
    let mut php_arg_refs: Vec<&dyn ext_php_rs::convert::IntoZvalDyn> = Vec::new();

    for index in 0..args.length() {
        let v = PHPValue::new(args.get(index), scope);
        php_args.push(v);
    }
    for index in &php_args {
        php_arg_refs.push(index);
    }
    let return_value = zval.callable().unwrap().try_call(php_arg_refs).unwrap();
    let return_value_js = js_value_from_zval(scope, &return_value);
    rv.set(return_value_js)
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
}

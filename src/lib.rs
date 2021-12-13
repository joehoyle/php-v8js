use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;
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
    isolate: v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
    global_name: String,
}

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
        // let zval = zval.shallow_clone();
        return function.into();
    }
    v8::null(scope).into()
}

#[php_impl(rename_methods = "camelCase")]
impl V8Js {
    pub fn __construct(object_name: Option<String>) -> Self {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
        let mut isolate = v8::Isolate::new(Default::default());
        let global_context;
        let global_name;
        {
            let scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Context::new(scope);
            global_context = v8::Global::new(scope, context);
        }
        {
            let scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Local::new(scope, &global_context);
            let scope = &mut v8::ContextScope::new(scope, context);
            global_name = match object_name {
                Some(name) => name,
                None => String::from("PHP"),
            };
            let global = v8::String::new(scope, global_name.as_str()).unwrap();
            let global_object = v8::Object::new(scope);
            context
                .global(scope)
                .set(scope, global.into(), global_object.into());
        }
        V8Js {
            isolate,
            context: global_context,
            global_name,
        }
    }
    pub fn execute_string(&mut self, string: String) -> PHPValue {
        let scope = &mut v8::HandleScope::new(&mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let code = v8::String::new(scope, string.as_str()).unwrap();
        let script = v8::Script::compile(scope, code, None).unwrap();
        let result = script.run(scope).unwrap();
        PHPValue::new(result, scope)
    }
    pub fn __set(&mut self, property: &str, value: &Zval) {
        let scope = &mut v8::HandleScope::new(&mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
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

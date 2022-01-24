use ext_php_rs::types::Zval;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct JSRuntime {
    isolate: v8::OwnedIsolate,
}

pub struct JsRuntimeState {
    pub global_context: v8::Global<v8::Context>,
    pub callbacks: HashMap<String, Zval>,
}
#[derive(Debug)]
pub enum Error {
    JSRuntimeError,
    ExecutionTimeout,
    MemoryLimitExceeded,
    V8Error,
}

fn init_v8() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
}
impl JSRuntime {
    pub fn new(snapshot_blob: Option<Vec<u8>>) -> Self {
        // The V8 Platform should only ever be intitialized once.
        static START: std::sync::Once = std::sync::Once::new();
        START.call_once(init_v8);

        let mut create_params = v8::CreateParams::default();
        // Restore the snapshot if one was provided. We have to map
        // ext_php_rs Binary data to u8 slices.
        if snapshot_blob.is_some() {
            let vec_data: Vec<u8> = snapshot_blob.unwrap();
            create_params = create_params.snapshot_blob(vec_data);
        }
        let mut isolate = v8::Isolate::new(create_params);

        let global_context;
        {
            let scope = &mut v8::HandleScope::new(&mut isolate);
            let context = v8::Context::new(scope);
            global_context = v8::Global::new(scope, context);
        }

        isolate.set_slot(Rc::new(RefCell::new(JsRuntimeState {
            global_context: global_context,
            callbacks: HashMap::new(),
        })));

        JSRuntime { isolate }
    }

    pub fn state(isolate: &v8::Isolate) -> Rc<RefCell<JsRuntimeState>> {
        let s = isolate.get_slot::<Rc<RefCell<JsRuntimeState>>>().unwrap();
        s.clone()
    }

    pub fn global_context(&self) -> v8::Global<v8::Context> {
        let state = Self::state(&self.isolate);
        let state = state.borrow();
        state.global_context.clone()
    }

    pub fn handle_scope(&mut self) -> v8::HandleScope {
        let context = self.global_context();
        v8::HandleScope::with_context(&mut self.isolate, context)
    }

    pub fn add_global(&mut self, name: &str, value: v8::Global<v8::Value>) {
        let mut scope = self.handle_scope();
        let context = scope.get_current_context();
        let global = context.global(&mut scope);

        let global_name = v8::String::new(&mut scope, name).unwrap();
        let global_value = v8::Local::new(&mut scope, value);

        global.set(&mut scope, global_name.into(), global_value.into());
    }

    pub fn get_global(&mut self, name: &str) -> Option<v8::Global<v8::Value>> {
        let mut scope = self.handle_scope();
        let context = scope.get_current_context();
        let global = context.global(&mut scope);

        let global_name = v8::String::new(&mut scope, name).unwrap();

        let var = global.get(&mut scope, global_name.into())?;
        Some(v8::Global::new(&mut scope, var))
    }

    pub fn add_callback(&mut self, name: &str, callback: Zval) {
        let state = Self::state(&self.isolate);
        let mut state = state.borrow_mut();
        state.callbacks.insert(name.into(), callback);
    }
    pub fn add_global_function(
        &mut self,
        name: &str,
        callback: impl v8::MapFnTo<v8::FunctionCallback>,
    ) {
        // let scope = &mut v8::HandleScope::new(&mut self.isolate);
        // let context = v8::Local::new(scope, &self.global_context());
        // let scope = &mut v8::ContextScope::new(scope, context);
        // let global = v8::String::new(scope, name).unwrap();
        // let global_scope = context.global(scope);
        let function: v8::Global<v8::Value>;
        {
            let mut scope = self.handle_scope();
            let function_builder: v8::FunctionBuilder<v8::Function> =
                v8::FunctionBuilder::new(callback);
            let f: v8::Local<v8::Value> = function_builder.build(&mut scope).unwrap().into();
            function = v8::Global::new(&mut scope, f).into();
        }
        self.add_global(name, function.into());
    }

    pub fn execute_string(
        &mut self,
        code: &str,
        identifier: Option<String>,
        _flags: Option<String>,
        time_limit: Option<u64>,
        memory_limit: Option<u64>,
    ) -> Result<Option<v8::Global<v8::Value>>, Error> {
        let isolate_handle = self.isolate.thread_safe_handle();
        let scope = &mut self.handle_scope();
        let code = v8::String::new(scope, code).ok_or(Error::V8Error)?;

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
        let script =
            v8::Script::compile(scope, code, Some(&script_origin)).ok_or(Error::JSRuntimeError)?;
        let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let time_limit_hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let memory_limit_hit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        // If we have time / memory limits, we have to spawn a monitoring thread
        // to periodically check the run time / memory usage of V8.
        if memory_limit.is_some() || time_limit.is_some() {
            println!("checking limit");
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
                    println!("{:#?}", time_limit);
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
                        println!("{:#?}", time_limit);
                        if time_limit.as_millis() > 0 {
                            println!("checlig limimt");
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
        let time_limit_hit = time_limit_hit.load(std::sync::atomic::Ordering::SeqCst);
        let memory_limit_hit = memory_limit_hit.load(std::sync::atomic::Ordering::SeqCst);

        let result = match result {
            Some(result) => Ok(Some(result)),
            None => {
                if time_limit_hit {
                    Err(Error::ExecutionTimeout)
                } else if memory_limit_hit {
                    Err(Error::MemoryLimitExceeded)
                } else {
                    Ok(None)
                }
            }
        }?;

        stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        let result = match result {
            Some(result) => Ok(Some(v8::Global::new(scope, result))),
            None => Ok(None),
        };
        result
    }

    pub fn create_snapshot(source: String) -> Option<Vec<u8>> {
        // Make sure platform is initted.
        JSRuntime::new(None);
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
        Some(snapshot_slice.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_termiante() {
        init_v8();
        {
            let isolate = &mut v8::Isolate::new(Default::default());
            let handle = isolate.thread_safe_handle();

            let scope = &mut v8::HandleScope::new(isolate);
            let context = v8::Context::new(scope);
            let scope = &mut v8::ContextScope::new(scope, context);

            let _t = std::thread::spawn(move || {
                // allow deno to boot and run
                std::thread::sleep(std::time::Duration::from_millis(300));
                handle.terminate_execution();
                // allow shutdown
                std::thread::sleep(std::time::Duration::from_millis(200));
            });

            let source = v8::String::new(scope, "for(;;) {}").unwrap();
            let r = v8::Script::compile(scope, source, None);
            let script = r.unwrap();
            let _result = script.run(scope);
        }
        {
            let isolate = &mut v8::Isolate::new(Default::default());

            let scope = &mut v8::HandleScope::new(isolate);
            let context = v8::Context::new(scope);
            let scope = &mut v8::ContextScope::new(scope, context);
            let source = v8::String::new(scope, "'hello'").unwrap();
            let r = v8::Script::compile(scope, source, None);
            let script = r.unwrap();
            let _result = script.run(scope);
        }
    }
    #[test]
    fn execute_string() {
        let mut runtime = JSRuntime::new(None);
        let result = runtime
            .execute_string("true", None, None, None, None)
            .unwrap().unwrap();
        let scope = &mut runtime.handle_scope();
        let local = v8::Local::new(scope, result);
        assert_eq!(local.is_true(), true);
    }
    // #[test]
    // fn add_global() {
    //     let string;
    //     let mut runtime = JSRuntime::new(None);
    //     {
    //         let scope = &mut runtime.handle_scope();
    //         let s: v8::Local<v8::Value> = v8::String::new(scope, "bar").unwrap().into();
    //         string = v8::Global::new(scope, s);
    //     }
    //     runtime.add_global("foo", string.into());
    //     let result = runtime
    //         .execute_string("foo", None, None, None, None)
    //         .unwrap();
    //     let scope = &mut runtime.handle_scope();
    //     let local = v8::Local::new(scope, result);
    //     assert_eq!(local.to_rust_string_lossy(scope).as_str(), "bar");
    // }
    // #[test]
    // fn add_global_function() {
    //     let mut runtime = JSRuntime::new(None);
    //     runtime.add_global_function(
    //         "return_42",
    //         |scope: &mut v8::HandleScope,
    //          _args: v8::FunctionCallbackArguments,
    //          mut rv: v8::ReturnValue| {
    //             let value = v8::Number::new(scope, 42.0);
    //             rv.set(value.into());
    //         },
    //     );

    //     let result = runtime
    //         .execute_string("return_42()", None, None, None, None)
    //         .unwrap();
    //     let scope = &mut runtime.handle_scope();
    //     let local = v8::Local::new(scope, result);
    //     assert_eq!(local.integer_value(scope).unwrap(), 42);
    // }

    // #[test]
    // fn create_snapshot() {
    //     JSRuntime::new(None);
    //     let snapshot =
    //         JSRuntime::create_snapshot("function return_41() { return 41 }".into()).unwrap();

    //     let mut runtime = JSRuntime::new(Some(snapshot));
    //     let result = runtime
    //         .execute_string("return_41()", None, None, None, None)
    //         .unwrap();
    //     let scope = &mut runtime.handle_scope();
    //     let local = v8::Local::new(scope, result);
    //     assert_eq!(local.integer_value(scope).unwrap(), 41);
    // }
    // #[test]
    // fn create_snapshot_var() {
    //     JSRuntime::new(None);
    //     let snapshot = JSRuntime::create_snapshot(
    //         "var fibonacci = n => n < 3 ? 1 : fibonacci(n - 1) + fibonacci(n - 2)".into(),
    //     )
    //     .unwrap();

    //     let mut runtime = JSRuntime::new(Some(snapshot));
    //     let result = runtime
    //         .execute_string("fibonacci(10)", None, None, None, None)
    //         .unwrap();
    //     let scope = &mut runtime.handle_scope();
    //     let local = v8::Local::new(scope, result);
    //     assert_eq!(local.integer_value(scope).unwrap(), 55);
    // }
}

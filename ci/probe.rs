fn main() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    let isolate = &mut v8::Isolate::new(Default::default());
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let wasm = wat::parse_str(
        r#"(module
          (type $box (struct (field i32)))
          (func (export "run") (result i32)
            i32.const 42 struct.new $box struct.get $box 0))"#,
    )
    .unwrap();
    let module = v8::WasmModuleObject::compile(scope, &wasm).unwrap();
    let key = v8::String::new(scope, "module").unwrap();
    context
        .global(scope)
        .set(scope, key.into(), module.into())
        .unwrap();
    let source = v8::String::new(
        scope,
        "typeof Intl === 'undefined' && new WebAssembly.Instance(module).exports.run() === 42",
    )
    .unwrap();
    let script = v8::Script::compile(scope, source, None).unwrap();
    assert!(
        script.run(scope).unwrap().is_true(),
        "ICU must be absent and WasmGC must execute"
    );
    println!("ICU is absent; WasmGC execution passed");
}

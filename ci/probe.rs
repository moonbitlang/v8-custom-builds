fn main() {
    // Run unsupported APIs in child processes: they must fail explicitly,
    // without preventing ordinary users of the stock Rust crate from linking.
    if let Some(mode) = std::env::args().nth(1) {
        match mode.as_str() {
            "--icu-get-locale" => {
                v8::icu::get_language_tag();
            }
            "--icu-set-locale" => v8::icu::set_default_locale("en-US"),
            _ => panic!("unknown probe mode: {mode}"),
        }
        return;
    }

    let executable = std::env::current_exe().unwrap();
    for (mode, symbol) in [
        ("--icu-get-locale", "icu_get_default_locale"),
        ("--icu-set-locale", "icu_set_default_locale"),
    ] {
        let output = std::process::Command::new(&executable)
            .arg(mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = format!("rusty_v8: {symbol} requires ICU support");
        assert!(
            !output.status.success() && stderr.lines().any(|line| line == expected),
            "{symbol} must terminate with an ICU-disabled diagnostic: {}\n{stderr}",
            output.status
        );
    }
    println!("Unsupported ICU locale APIs fail explicitly");

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

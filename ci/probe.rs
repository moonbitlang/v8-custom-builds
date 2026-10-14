fn evaluate<'s>(scope: &mut v8::HandleScope<'s>, source: &str) -> v8::Local<'s, v8::Value> {
    let source = v8::String::new(scope, source).unwrap();
    v8::Script::compile(scope, source, None)
        .unwrap()
        .run(scope)
        .unwrap()
}

fn main() {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        Some("--icu-get-locale") => {
            v8::icu::get_language_tag();
            return;
        }
        Some("--icu-set-locale") => {
            v8::icu::set_default_locale("en-US");
            return;
        }
        None => {
            // Keep unsupported APIs and V8's process-global tier flags isolated.
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
            for (mode, compiler) in [("liftoff", "Liftoff"), ("optimized", "TurboFan")] {
                let output = std::process::Command::new(&executable)
                    .arg(mode)
                    .output()
                    .unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                print!("{stdout}");
                assert!(
                    output.status.success(),
                    "{mode}: {}\n{stderr}",
                    output.status
                );
                assert!(
                    stdout.contains(&format!("using {compiler}")),
                    "missing compilation trace for {mode}"
                );
            }
            return;
        }
        Some("liftoff") => v8::V8::set_flags_from_string(
            "--liftoff-only --no-wasm-lazy-compilation --trace-wasm-compilation-times",
        ),
        Some("optimized") => v8::V8::set_flags_from_string(
            "--no-liftoff --no-wasm-lazy-compilation --trace-wasm-compilation-times",
        ),
        Some(mode) => panic!("unknown probe mode: {mode}"),
    }
    v8::V8::set_flags_from_string("--allow-natives-syntax");
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    let isolate = &mut v8::Isolate::new(Default::default());
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    assert!(
        evaluate(
            scope,
            "!%IsTurbofanEnabled() && !%IsMaglevEnabled() && !%IsSparkplugEnabled()"
        )
        .is_true(),
        "JavaScript optimizing and baseline JIT tiers must be disabled"
    );
    assert!(evaluate(scope, "typeof Intl === 'undefined'").is_true());
    assert_eq!(evaluate(scope, "40 + 2").int32_value(scope), Some(42));
    // Valid asm.js must retain its JavaScript behavior without specialization.
    assert!(
        evaluate(
            scope,
            r#"
            function asmModule() {
                "use asm";
                function add(a, b) {
                    a = a | 0;
                    b = b | 0;
                    return (a + b) | 0;
                }
                return {add: add};
            }
            asmModule().add(20, 22) === 42 && !%IsAsmWasmCode(asmModule)
            "#
        )
        .is_true(),
        "asm.js must execute as ordinary JavaScript"
    );

    let collect = v8::Function::new(
        scope,
        |scope: &mut v8::HandleScope, _: v8::FunctionCallbackArguments, _: v8::ReturnValue| {
            scope.low_memory_notification();
        },
    )
    .unwrap();
    let key = v8::String::new(scope, "collect").unwrap();
    context
        .global(scope)
        .set(scope, key.into(), collect.into())
        .unwrap();
    for source in [
        r#"(module (func (export "run") (result i32) i32.const 42))"#,
        // Keep GC references live across a native host call requesting collection.
        r#"(module
            (type $cell (struct (field (mut i32))))
            (type $cells (array (mut (ref null $cell))))
            (import "env" "collect" (func $collect))
            (func (export "run") (result i32)
                (local $keep (ref null $cells))
                i32.const 42 struct.new $cell
                i32.const 1 array.new $cells local.set $keep
                call $collect
                local.get $keep i32.const 0 array.get $cells
                struct.get $cell 0))"#,
    ] {
        let wasm = wat::parse_str(source).unwrap();
        let module = v8::WasmModuleObject::compile(scope, &wasm).unwrap();
        let key = v8::String::new(scope, "module").unwrap();
        context
            .global(scope)
            .set(scope, key.into(), module.into())
            .unwrap();
        assert_eq!(
            evaluate(
                scope,
                "new WebAssembly.Instance(module, {env: {collect}}).exports.run()"
            )
            .int32_value(scope),
            Some(42)
        );
    }
    println!(
        "JS JIT tiers, asm.js translation and ICU are disabled; numeric Wasm and WasmGC passed ({})",
        mode.unwrap()
    );
}

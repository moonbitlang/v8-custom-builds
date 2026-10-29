use wee8::{CompileOptions, Store, StringRef, Value};

#[test]
fn shared_code_outlives_source_store_and_runs_on_independent_threads() {
    let bytes = wat::parse_str(
        r#"(module
            (import "host" "start" (func $host_start))
            (func $start call $host_start)
            (start $start)
            (memory (export "memory") 1 2)
            (func (export "increment") (result i32)
                i32.const 0 i32.const 0 i32.load i32.const 1 i32.add i32.store
                i32.const 0 i32.load)
            (func (export "trap") unreachable))"#,
    )
    .unwrap();
    let shared = {
        let store = Store::new(()).unwrap();
        store.compile(&bytes).unwrap().share().unwrap()
    };
    std::thread::scope(|threads| {
        for _ in 0..8 {
            let shared = &shared;
            threads.spawn(move || {
                let store = Store::new(()).unwrap();
                let module = store.obtain(shared).unwrap();
                let starts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let count = starts.clone();
                let host = store
                    .host_func(&[], &[], move |_, _| {
                        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        Ok(vec![])
                    })
                    .unwrap();
                // Native code is shared; start execution and linear memory are
                // per instance, including instances in the same destination.
                for _ in 0..2 {
                    let mut instance = module.instantiate(&[&host]).unwrap();
                    assert_eq!(instance.call("increment", &[]).unwrap(), [Value::I32(1)]);
                    assert!(instance.call("trap", &[]).is_err());
                    assert_eq!(instance.call("increment", &[]).unwrap(), [Value::I32(2)]);
                }
                assert_eq!(starts.load(std::sync::atomic::Ordering::Relaxed), 2);
            });
        }
    });
    // Exercise moving and finally destroying the shared handle on another
    // thread, while an obtained module continues to own the native code.
    std::thread::spawn(move || {
        let store = Store::new(()).unwrap();
        let module = store.obtain(&shared).unwrap();
        drop(shared);
        let host = store.host_func(&[], &[], |_, _| Ok(vec![])).unwrap();
        let mut instance = module.instantiate(&[&host]).unwrap();
        assert_eq!(instance.call("increment", &[]).unwrap(), [Value::I32(1)]);
    })
    .join()
    .unwrap();
}

#[test]
fn native_string_options_survive_sharing_and_explicit_serialization() {
    for namespace in ["", "_", "literal\0strings"] {
        let bytes = wat::parse_str(format!(
            r#"(module
                (import "{}" "😄" (global $literal (ref extern)))
                (import "wasm:js-string" "length" (func $length (param externref) (result i32)))
                (func (export "length") (result i32) global.get $literal call $length)
                (func (export "get") (result externref) global.get $literal))"#,
            namespace.replace('\0', "\\00")
        ))
        .unwrap();
        let shared = {
            let store = Store::new(()).unwrap();
            let options = CompileOptions {
                js_string_builtins: true,
                imported_string_constants: Some(namespace.into()),
            };
            store
                .compile_with_options(&bytes, &options)
                .unwrap()
                .share()
                .unwrap()
        };
        let store = Store::new(()).unwrap();
        let module = store.obtain(&shared).unwrap();
        drop(shared);
        // Existing serialization remains explicit and keeps compile options
        // even when its input module was obtained via native sharing.
        let serialized = module.serialize();
        for module in [module, store.module(&serialized).unwrap()] {
            assert!(module.imports().unwrap().is_empty());
            let mut instance = module.instantiate(&[]).unwrap();
            assert_eq!(instance.call("length", &[]).unwrap(), [Value::I32(2)]);
            let Value::ExternRef(Some(value)) = instance.call("get", &[]).unwrap().remove(0) else {
                panic!("missing literal")
            };
            assert_eq!(StringRef::try_from(value).unwrap().to_utf8().unwrap(), "😄");
        }
    }
}

#[test]
fn native_sharing_fixture_matches_readable_wat() {
    assert_eq!(
        wat::parse_str(include_str!(
            "../../../custom_wee8/tests/module-sharing.wat"
        ))
        .unwrap(),
        include_bytes!("../../../custom_wee8/tests/module-sharing.wasm")
    );
}

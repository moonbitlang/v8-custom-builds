use std::cell::Cell;
use wee8::{Store, Value};

#[test]
fn native_starts_run_once_after_compilation_and_deserialization() {
    for source in [
        r#"(module
            (import "host" "start" (func $host))
            (func $start call $host)
            (start $start))"#,
        r#"(module
            (import "host" "start" (func $host))
            (start $host)
            (export "run" (func $host)))"#,
    ] {
        let store = Store::new(Cell::new(0)).unwrap();
        let module = store.compile(&wat::parse_str(source).unwrap()).unwrap();
        let compiled = module.serialize();
        let host = store
            .host_func(&[], &[], |caller, _| {
                caller.with_data(|calls| calls.set(calls.get() + 1));
                Ok(vec![])
            })
            .unwrap();
        for module in [module, store.module(&compiled).unwrap()] {
            let before = store.as_context().with_data(Cell::get);
            let mut instance = module.instantiate(&[&host]).unwrap();
            assert_eq!(store.as_context().with_data(Cell::get), before + 1);
            // Also cover the ordinary C entry for a re-exported host import.
            if instance.has_export("run") {
                assert!(instance.call("run", &[]).unwrap().is_empty());
                assert_eq!(store.as_context().with_data(Cell::get), before + 2);
            }
        }
    }
}

#[test]
fn start_traps_and_host_errors_leave_the_store_usable() {
    let store = Store::new(()).unwrap();
    let good = store
        .compile(
            &wat::parse_str(r#"(module (func (export "run") (result i32) i32.const 42))"#).unwrap(),
        )
        .unwrap();
    for source in [
        "(module (func $start unreachable) (start $start))",
        "(module (func $start call $start) (start $start))",
    ] {
        let module = store.compile(&wat::parse_str(source).unwrap()).unwrap();
        assert!(module.instantiate(&[]).is_err());
        assert_eq!(
            good.instantiate(&[]).unwrap().call("run", &[]).unwrap(),
            [Value::I32(42)]
        );
    }
    for source in [
        r#"(module (import "host" "fail" (func $fail)) (start $fail))"#,
        r#"(module (import "host" "fail" (func $fail))
            (func $start call $fail) (start $start))"#,
    ] {
        let module = store.compile(&wat::parse_str(source).unwrap()).unwrap();
        let fail = store
            .host_func(&[], &[], |_, _| Err("start callback failed".into()))
            .unwrap();
        let error = module.instantiate(&[&fail]).err().expect("start must fail");
        assert!(error.to_string().contains("start callback failed"));
        assert_eq!(
            good.instantiate(&[]).unwrap().call("run", &[]).unwrap(),
            [Value::I32(42)]
        );
    }
}

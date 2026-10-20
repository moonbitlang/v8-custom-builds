#![cfg(feature = "exceptions")]

//! Exception behavior is tested through the public Rust API and real Wasm:
//! - Tags match by identity; guest catches and rethrows preserve the payload.
//! - Payload roots stay alive while owned and are released during store teardown.
//! - Invalid or foreign handles fail safely before reaching native code.
//! - Explicit throws are catchable; host failures and panics remain host errors.
//!
//! The packaged C consumer only checks the new header/archive ABI; it does not
//! repeat these exception scenarios.

use std::{cell::Cell, rc::Rc};
use wee8::{Error, Exception, Extern, ExternRef, ExternType, Kind, Store, Tag, Value};

#[test]
fn catches_by_identity_and_preserves_uncaught_and_rethrown_payloads() {
    let store = Store::new(()).unwrap();
    let tag = Tag::new(store.as_context(), &[Kind::I32]).unwrap();
    let other = Tag::new(store.as_context(), &[Kind::I32]).unwrap();
    assert!(!tag.same(store.as_context(), other).unwrap());
    let host = store
        .host_func(&[], &[], move |caller, _| {
            let exception = Exception::new(caller.as_context(), tag, &[Value::I32(42)])?;
            Err(caller.throw(&exception))
        })
        .unwrap();
    let module = store
        .compile(&wat::parse_str(include_str!("fixtures/exceptions.wat")).unwrap())
        .unwrap();
    let imports = module.import_types().unwrap();
    assert_eq!(
        imports[0].ty,
        ExternType::Tag {
            params: vec![Kind::I32]
        }
    );
    assert!(module.imports().is_err()); // Function-only compatibility interface.
    let compiled = module.serialize();
    for module in [module, store.module(&compiled).unwrap()] {
        let mut instance = module
            .instantiate_with_imports(&[Extern::Tag(tag), Extern::Tag(other), Extern::Func(&host)])
            .unwrap();
        assert!(
            instance
                .tag("tag")
                .unwrap()
                .same(store.as_context(), tag)
                .unwrap()
        );
        assert_eq!(instance.call("caught", &[]).unwrap(), [Value::I32(42)]);
        assert!(store.take_exception().is_none());
        for (name, value) in [("wrong_tag", 42), ("guest_throw", 17), ("rethrow", 42)] {
            assert!(matches!(
                instance.call(name, &[]),
                Err(Error::ThrownException)
            ));
            let exception = store.take_exception().unwrap();
            assert!(exception.is(tag).unwrap());
            assert!(!exception.is(other).unwrap());
            assert_eq!(exception.args(tag).unwrap(), [Value::I32(value)]);
            assert!(exception.args(other).is_err());
        }
        instance.call("catch_all", &[]).unwrap();
        assert_eq!(instance.call("alive", &[]).unwrap(), [Value::I32(7)]);
    }
}

#[test]
fn payloads_preserve_bits_and_native_reference_roots() {
    struct Payload(Rc<Cell<usize>>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    {
        let store = Store::new(()).unwrap();
        let tag = Tag::new(
            store.as_context(),
            &[Kind::I32, Kind::I64, Kind::F32, Kind::F64, Kind::ExternRef],
        )
        .unwrap();
        let reference = ExternRef::new(store.as_context(), Payload(drops.clone())).unwrap();
        let values = vec![
            Value::I32(-1),
            Value::I64(i64::MIN),
            Value::F32(-0.0),
            Value::F64(f64::from_bits(0x7ff8000000000042)),
            Value::ExternRef(Some(reference.clone())),
        ];
        let exception = Exception::new(store.as_context(), tag, &values).unwrap();
        let args = exception.args(tag).unwrap();
        assert_eq!(args[0], Value::I32(-1));
        assert_eq!(args[1], Value::I64(i64::MIN));
        assert!(matches!(args[2], Value::F32(f) if f.to_bits() == (-0.0f32).to_bits()));
        assert!(matches!(args[3], Value::F64(f) if f.to_bits() == 0x7ff8000000000042));
        assert_eq!(args[4], Value::ExternRef(Some(reference)));
        drop(values);
        drop(exception);
        assert!(matches!(&args[4], Value::ExternRef(Some(r)) if r.data::<Payload>().is_some()));
        assert_eq!(drops.get(), 0);
    }
    assert_eq!(drops.get(), 1);
}

#[test]
fn guest_payload_is_rooted_after_the_call_and_can_be_externalized() {
    let store = Store::new(()).unwrap();
    let tag = Tag::new(store.as_context(), &[Kind::ExternRef]).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "e" "tag" (tag $tag (param externref)))
        (func (export "throw") (param externref) local.get 0 throw $tag))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut instance = module
        .instantiate_with_imports(&[Extern::Tag(tag)])
        .unwrap();
    let reference = store.extern_ref(String::from("payload")).unwrap();
    assert!(matches!(
        instance.call("throw", &[Value::ExternRef(Some(reference))]),
        Err(Error::ThrownException)
    ));
    let exception = store.take_exception().unwrap();
    let values = exception.args(tag).unwrap();
    assert!(
        matches!(&values[0], Value::ExternRef(Some(r)) if r.data::<String>().unwrap() == "payload")
    );
    assert!(matches!(
        instance.call("throw", &[Value::ExternRef(None)]),
        Err(Error::ThrownException)
    ));
    assert_eq!(
        store.take_exception().unwrap().args(tag).unwrap(),
        [Value::ExternRef(None)]
    );
}

#[test]
fn rejects_cross_store_handles_and_bad_signatures() {
    let stale = {
        let store = Store::new(()).unwrap();
        Tag::new(store.as_context(), &[]).unwrap()
    };
    let store = Store::new(()).unwrap();
    let other = Store::new(()).unwrap();
    let tag = Tag::new(store.as_context(), &[Kind::I32]).unwrap();
    assert!(Exception::new(store.as_context(), stale, &[]).is_err());
    assert!(Exception::new(other.as_context(), tag, &[Value::I32(1)]).is_err());
    assert!(Exception::new(store.as_context(), tag, &[]).is_err());
    assert!(Exception::new(store.as_context(), tag, &[Value::F64(1.0)]).is_err());
    let ref_tag = Tag::new(store.as_context(), &[Kind::ExternRef]).unwrap();
    let foreign = other.extern_ref(1).unwrap();
    assert!(
        Exception::new(
            store.as_context(),
            ref_tag,
            &[Value::ExternRef(Some(foreign))]
        )
        .is_err()
    );
    let module = store
        .compile(&wat::parse_str(r#"(module (import "e" "tag" (tag)))"#).unwrap())
        .unwrap();
    assert!(
        module
            .instantiate_with_imports(&[Extern::Tag(stale)])
            .is_err()
    );
    assert!(module.instantiate_with_imports(&[]).is_err());
    // Same store, wrong native signature: an instantiation error, not a crash.
    assert!(
        module
            .instantiate_with_imports(&[Extern::Tag(tag)])
            .is_err()
    );
}

#[test]
fn start_exceptions_and_untaken_exceptions_do_not_poison_next_execution() {
    let store = Store::new(()).unwrap();
    let tag = Tag::new(store.as_context(), &[]).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "e" "tag" (tag $tag))
        (func $start throw $tag) (start $start))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        module.instantiate_with_imports(&[Extern::Tag(tag)]),
        Err(Error::ThrownException)
    ));
    assert!(store.take_exception().unwrap().is(tag).unwrap());
    assert!(matches!(
        module.instantiate_with_imports(&[Extern::Tag(tag)]),
        Err(Error::ThrownException)
    ));
    let good = store.compile(&wat::parse_str("(module)").unwrap()).unwrap();
    good.instantiate(&[]).unwrap();
    assert!(store.take_exception().is_none());

    let host = store
        .host_func(&[], &[], move |caller, _| {
            let exception = Exception::new(caller.as_context(), tag, &[])?;
            Err(caller.throw(&exception))
        })
        .unwrap();
    let start = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "e" "throw" (func $throw))
        (func $start call $throw) (start $start))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        start.instantiate(&[&host]),
        Err(Error::ThrownException)
    ));
    assert!(store.take_exception().unwrap().is(tag).unwrap());
}

#[test]
fn explicit_throw_is_distinct_from_host_errors_panics_and_ignored_markers() {
    let store = Store::new(Cell::new(0)).unwrap();
    let tag = Tag::new(store.as_context(), &[]).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "e" "host" (func $host))
        (func (export "run")
            (block $caught (try_table (catch_all $caught) call $host))))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let host = store
        .host_func(&[], &[], move |caller, _| {
            let mode = caller.with_data(Cell::get);
            if mode == 4 {
                return Err(Error::ThrownException);
            }
            let exception = Exception::new(caller.as_context(), tag, &[])?;
            let thrown = caller.throw(&exception);
            match mode {
                0 => Err(thrown),
                1 => Ok(vec![]), // Ignored marker discards the pending exception.
                2 => Err("host failure".into()),
                3 => panic!("host panic"),
                _ => unreachable!(),
            }
        })
        .unwrap();
    let mut instance = module.instantiate(&[&host]).unwrap();
    for mode in 0..5 {
        store.as_context().with_data(|value| value.set(mode));
        let result = instance.call("run", &[]);
        if mode < 2 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(Error::Host(_))));
        }
        assert!(store.take_exception().is_none());
    }
    store.as_context().with_data(|value| value.set(0));
    instance.call("run", &[]).unwrap();
}

#[test]
fn exported_tags_can_be_reimported_and_unsupported_tag_payloads_fail_validation() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile(&wat::parse_str(r#"(module (tag (export "tag") (param i32)))"#).unwrap())
        .unwrap();
    let instance = module.instantiate(&[]).unwrap();
    let first = instance.tag("tag").unwrap();
    let second = instance.tag("tag").unwrap();
    assert!(first.same(store.as_context(), second).unwrap());
    let consumer = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "e" "tag" (tag $tag (param i32)))
        (export "tag" (tag $tag)))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let imported = consumer
        .instantiate_with_imports(&[Extern::Tag(first)])
        .unwrap();
    assert!(
        imported
            .tag("tag")
            .unwrap()
            .same(store.as_context(), first)
            .unwrap()
    );
    for source in [
        r#"(module (import "e" "tag" (tag (param v128))))"#,
        r#"(module (type $s (struct)) (tag (export "tag") (param (ref null $s))))"#,
        r#"(module (global (export "exception") exnref (ref.null exn)))"#,
    ] {
        assert!(store.compile(&wat::parse_str(source).unwrap()).is_err());
    }
}

#[test]
fn rejects_funcref_tag_imports_and_exports() {
    let store = Store::new(()).unwrap();
    for (source, boundary) in [
        (
            r#"(module (import "e" "tag" (tag (param funcref))))"#,
            "import `e::tag`",
        ),
        (
            r#"(module (tag (export "tag") (param funcref)))"#,
            "export `tag`",
        ),
    ] {
        let result = store.compile(&wat::parse_str(source).unwrap());
        assert!(
            matches!(result, Err(Error::Interface(ref message))
                if message.contains("funcref") && message.contains(boundary)),
            "must reject funcref payloads at {boundary} before native introspection"
        );
    }
}

#[test]
fn guest_internal_funcref_tag_payloads_can_be_caught() {
    let store = Store::new(()).unwrap();
    // The tag and its non-null function reference stay inside Wasm. Only the
    // scalar result crosses the host boundary.
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (tag $tag (param funcref))
        (func $payload)
        (elem declare func $payload)
        (func (export "run") (result i32)
            (block $caught (result funcref)
                (try_table (catch $tag $caught)
                    ref.func $payload
                    throw $tag)
                unreachable)
            ref.is_null))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    assert_eq!(instance.call("run", &[]).unwrap(), [Value::I32(0)]);
}

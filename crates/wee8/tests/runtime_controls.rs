use std::{cell::RefCell, rc::Rc};
use wee8::{Error, Exception, Kind, Store, StringRef, Tag, Value};

#[test]
fn externalized_i31_and_gc_values_keep_store_owned_roots() {
    let source = r#"(module
      (type $cell (struct (field i32)))
      (type $array (array i32))
      (import "h" "echo" (func $echo (param externref) (result externref)))
      (global $g externref (extern.convert_any (ref.i31 (i32.const 0))))
      (table $t 2 externref (extern.convert_any (ref.i31 (i32.const -1))))
      (elem (i32.const 1) externref (extern.convert_any (ref.i31 (i32.const 7))))
      (func (export "i31") (param i32) (result externref)
        local.get 0 ref.i31 extern.convert_any call $echo)
      (func (export "read_i31") (param externref) (result i32)
        local.get 0 any.convert_extern ref.cast (ref i31) i31.get_s)
      (func (export "cell") (param i32) (result externref)
        local.get 0 struct.new $cell extern.convert_any call $echo)
      (func (export "read_cell") (param externref) (result i32)
        local.get 0 any.convert_extern ref.cast (ref $cell) struct.get $cell 0)
      (func (export "array") (param i32) (result externref)
        local.get 0 i32.const 1 array.new $array extern.convert_any call $echo)
      (func (export "read_array") (param externref) (result i32)
        local.get 0 any.convert_extern ref.cast (ref $array) i32.const 0 array.get $array)
      (func (export "constant") (result externref) global.get $g)
      (func (export "table") (param i32) (result externref) local.get 0 table.get $t)
      (func (export "null") (result externref) ref.null any extern.convert_any))"#;
    let bytes = wat::parse_str(source).unwrap();
    let store = Store::new(()).unwrap();
    let compiled = store.compile(&bytes).unwrap().serialize();
    let module = store.module(&compiled).unwrap();
    let echo = store
        .host_func(&[Kind::ExternRef], &[Kind::ExternRef], |_, values| {
            let Value::ExternRef(Some(reference)) = &values[0] else {
                panic!("missing root")
            };
            assert!(reference.data::<i32>().is_none());
            assert!(StringRef::try_from(reference.clone()).is_err());
            Ok(vec![values[0].clone()])
        })
        .unwrap();
    let mut instance = module.instantiate(&[&echo]).unwrap();
    let tag = Tag::new(store.as_context(), &[Kind::ExternRef]).unwrap();
    for (make, read) in [
        ("i31", "read_i31"),
        ("cell", "read_cell"),
        ("array", "read_array"),
    ] {
        for number in [0, -1, (1 << 30) - 1, -(1 << 30), i32::MIN, i32::MAX] {
            let value = instance
                .call(make, &[Value::I32(number)])
                .unwrap()
                .remove(0);
            assert!(matches!(value, Value::ExternRef(Some(_))));
            let exception =
                Exception::new(store.as_context(), tag, std::slice::from_ref(&value)).unwrap();
            let payload = exception.args(tag).unwrap();
            assert_eq!(payload.as_slice(), std::slice::from_ref(&value));
            drop(value);
            let expected = if make == "i31" {
                (number << 1) >> 1
            } else {
                number
            };
            assert_eq!(
                instance.call(read, &payload).unwrap(),
                [Value::I32(expected)]
            );
        }
    }
    for (name, arguments, expected) in [
        ("constant", vec![], 0),
        ("table", vec![Value::I32(0)], -1),
        ("table", vec![Value::I32(1)], 7),
    ] {
        let root = instance.call(name, &arguments).unwrap();
        assert!(matches!(root[0], Value::ExternRef(Some(_))));
        assert_eq!(
            instance.call("read_i31", &root).unwrap(),
            [Value::I32(expected)]
        );
    }
    let zero = instance.call("i31", &[Value::I32(0)]).unwrap();
    let other_store = Store::new(()).unwrap();
    let other_module = other_store.module(&compiled).unwrap();
    let other_echo = other_store
        .host_func(&[Kind::ExternRef], &[Kind::ExternRef], |_, values| {
            Ok(vec![values[0].clone()])
        })
        .unwrap();
    let mut other_instance = other_module.instantiate(&[&other_echo]).unwrap();
    let other_zero = other_instance.call("i31", &[Value::I32(0)]).unwrap();
    assert_ne!(zero, other_zero);
    assert!(instance.call("read_i31", &other_zero).is_err());
    assert!(Exception::new(store.as_context(), tag, &other_zero).is_err());
    assert_eq!(
        instance.call("null", &[]).unwrap(),
        [Value::ExternRef(None)]
    );
}

#[test]
fn native_exception_trace_preserves_construction_frames_after_rethrow() {
    let store = Store::new(()).unwrap();
    let tag = Tag::new(store.as_context(), &[]).unwrap();
    assert!(
        Exception::new(store.as_context(), tag, &[])
            .unwrap()
            .trace()
            .is_empty()
    );
    let captured = Rc::new(RefCell::new(Vec::new()));
    let during_callback = captured.clone();
    let host = store
        .host_func(&[], &[], move |caller, _| {
            let exception = Exception::new(caller.as_context(), tag, &[])?;
            *during_callback.borrow_mut() = exception.trace();
            Err(caller.throw(&exception))
        })
        .unwrap();
    let bytes = wat::parse_str(
        r#"(module
      (import "h" "throw" (func $throw))
      (func $inner call $throw)
      (func $outer call $inner)
      (func (export "run")
        block $caught (result exnref)
          try_table (catch_all_ref $caught) call $outer end
          unreachable
        end
        throw_ref))"#,
    )
    .unwrap();
    let module = store.compile(&bytes).unwrap();
    let mut instance = module.instantiate(&[&host]).unwrap();
    assert!(matches!(
        instance.call("run", &[]),
        Err(Error::ThrownException)
    ));
    let exception = store.take_exception().unwrap();
    let trace = exception.trace();
    assert!(trace.len() >= 3);
    assert_eq!(trace, *captured.borrow());
    assert!(trace.iter().all(|frame| frame.module_offset > 0));
    assert_eq!(trace[0].function_index, 1);
}

#[test]
fn custom_stack_limit_traps_and_can_be_restored() {
    let store = Store::new(()).unwrap();
    // Zero is rejected before changing the native guard.
    assert!(unsafe { store.set_stack_limit(0) }.is_err());
    let bytes = wat::parse_str(
        r#"(module
      (func $recurse (export "run") (param i32) (result i32)
        local.get 0
        if (result i32)
          local.get 0 i32.const 1 i32.sub call $recurse i32.const 1 i32.add
        else i32.const 0 end))"#,
    )
    .unwrap();
    let module = store.compile(&bytes).unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    let marker = 0u8;
    let limit = (&marker as *const u8 as usize) - 128 * 1024;
    // The test thread has an allocated native stack larger than this region;
    // tighten the boundary for one call, then restore it before leaving scope.
    let previous = unsafe { store.set_stack_limit(limit) }.unwrap();
    let result = instance.call("run", &[Value::I32(100000)]);
    unsafe { store.set_stack_limit(previous) }.unwrap();
    assert!(matches!(result, Err(Error::Trap(_))));
    assert_eq!(
        instance.call("run", &[Value::I32(5)]).unwrap(),
        [Value::I32(5)]
    );
}

#[test]
fn ordinary_traps_and_callback_capture_retain_wasm_frames() {
    let store = Store::new(()).unwrap();
    let callback = store
        .host_func(&[], &[], |caller, _| {
            let frames = caller.capture_stack()?;
            assert_eq!(frames[0].function_index, 1);
            assert!(frames.iter().all(|frame| frame.module_offset > 0));
            Ok(vec![])
        })
        .unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "h" "capture" (func $capture))
        (func $inner call $capture unreachable)
        (func (export "run") call $inner))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut instance = module.instantiate(&[&callback]).unwrap();
    let Err(Error::Trap(trap)) = instance.call("run", &[]) else {
        panic!("expected trap")
    };
    assert!(trap.message.contains("unreachable"));
    assert_eq!(trap.frames[0].function_index, 1);
    assert!(trap.frames.iter().all(|frame| frame.module_offset > 0));
}

#[test]
fn host_errors_keep_frames_without_becoming_guest_traps() {
    let store = Store::new(()).unwrap();
    let host = store
        .host_func(&[], &[], |_, _| Err(Error::Host("host failure".into())))
        .unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "h" "fail" (func $fail))
        (func (export "run") call $fail))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut instance = module.instantiate(&[&host]).unwrap();
    let Err(Error::Host(error)) = instance.call("run", &[]) else {
        panic!("expected host error")
    };
    assert_eq!(error.message, "host failure");
    assert_eq!(error.frames[0].function_index, 1);
    assert!(error.frames[0].module_offset > 0);
}

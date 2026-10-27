// moon: The build system and package manager for MoonBit.
// Copyright (C) 2026 International Digital Economy Academy
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND,
// either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// For inquiries, you can contact us via e-mail at jichuruanjian@idea.edu.cn.

use wee8::{Store, Value};

#[test]
fn values_match_signature_types_without_erasing_nullability() {
    use wee8::{HeapType, RefType, ValType};

    let nullable = ValType::Ref(RefType::new(true, HeapType::Extern));
    let non_null = ValType::Ref(RefType::new(false, HeapType::Extern));
    assert_eq!(nullable, ValType::EXTERNREF);
    assert!(non_null.matches(&nullable));
    assert!(!nullable.matches(&non_null));
    assert_ne!(nullable, non_null);

    let store = Store::new(()).unwrap();
    let types = [
        ValType::I32,
        ValType::I64,
        ValType::F32,
        ValType::F64,
        nullable,
        non_null,
    ];
    let cases = [
        (Value::I32(42), [true, false, false, false, false, false]),
        (Value::I64(42), [false, true, false, false, false, false]),
        (Value::F32(42.0), [false, false, true, false, false, false]),
        (Value::F64(42.0), [false, false, false, true, false, false]),
        (
            Value::ExternRef(None),
            [false, false, false, false, true, false],
        ),
        (
            Value::ExternRef(Some(store.extern_ref(42).unwrap())),
            [false, false, false, false, true, true],
        ),
    ];
    for (value, expected) in cases {
        for (ty, expected) in types.iter().zip(expected) {
            assert_eq!(value.matches_ty(ty), expected, "{value:?} against {ty:?}");
        }
    }
}

#[test]
fn store_and_caller_share_reference_creation_through_context() {
    use wee8::{ExternRef, StoreContext, ValType};

    fn make_reference(context: StoreContext<'_, u32>) -> wee8::Result<ExternRef<'_>> {
        let value = context.with_data(|value| *value);
        ExternRef::new(context, value)
    }

    let store = Store::new(42_u32).unwrap();
    // The root borrows the store, not the temporary StoreContext value.
    let host_reference = make_reference(store.as_context()).unwrap();
    assert_eq!(host_reference.data::<u32>(), Some(&42));
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "host" "new" (func $new (result externref)))
        (func (export "run") (result externref) call $new))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let create = store
        .host_func(&[], &[ValType::EXTERNREF], |caller, _| {
            Ok(vec![Value::ExternRef(Some(make_reference(
                caller.as_context(),
            )?))])
        })
        .unwrap();
    let mut instance = module.instantiate(&[&create]).unwrap();
    let result = instance.call("run", &[]).unwrap();
    let Value::ExternRef(Some(reference)) = &result[0] else {
        panic!("missing reference")
    };
    assert_eq!(reference.data::<u32>(), host_reference.data::<u32>());
    assert_ne!(reference, &host_reference);
}

#[test]
fn gc_values_stay_internal_across_calls_and_serialization() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (type $box (struct (field (mut i32))))
        (type $items (array (mut (ref null $box))))
        (global $saved (mut (ref null $items)) (ref.null $items))
        (func (export "save")
            i32.const 42 struct.new $box
            i32.const 2 array.new $items global.set $saved)
        (func (export "read") (result i32)
            global.get $saved i32.const 1 array.get $items struct.get $box 0))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let compiled = module.serialize();
    let second = Store::new(()).unwrap();
    let module = second.module(&compiled).unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    instance.call("save", &[]).unwrap();
    assert_eq!(instance.call("read", &[]).unwrap(), [Value::I32(42)]);
}

#[test]
fn externrefs_cross_callbacks_and_guest_roots_without_losing_host_data() {
    use std::{cell::Cell, rc::Rc};
    use wee8::ValType;
    struct Payload(Rc<Cell<usize>>, String);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let dropped = Rc::new(Cell::new(0));
    {
        let store = Store::new(()).unwrap();
        let module = store
            .compile(
                &wat::parse_str(
                    r#"(module
            (import "host" "echo" (func $echo (param externref) (result externref)))
            (import "host" "new" (func $new (result externref)))
            (type $cell (struct (field externref)))
            (type $garbage (array (mut i64)))
            (global $scratch (mut (ref null $garbage)) (ref.null $garbage))
            (func (export "allocate") (local $i i32)
                loop $again
                    i32.const 256 array.new_default $garbage global.set $scratch
                    local.get $i i32.const 1 i32.add local.tee $i
                    i32.const 20000 i32.lt_u br_if $again
                end)
            (global $held (mut (ref null $cell)) (ref.null $cell))
            (func (export "save") (param externref) local.get 0 struct.new $cell global.set $held)
            (func (export "read") (result externref) global.get $held struct.get $cell 0 call $echo)
            (func (export "new") (result externref) call $new)
            (func (export "null") (result externref) ref.null extern call $echo))"#,
                )
                .unwrap(),
            )
            .unwrap();
        let echo = store
            .host_func(&[ValType::EXTERNREF], &[ValType::EXTERNREF], |_, values| {
                if let Value::ExternRef(Some(reference)) = &values[0] {
                    assert_eq!(reference.data::<Payload>().unwrap().1, "hello 🦀");
                    assert!(reference.data::<i32>().is_none());
                }
                Ok(vec![values[0].clone()])
            })
            .unwrap();
        let create = store
            .host_func(&[], &[ValType::EXTERNREF], |caller, _| {
                Ok(vec![Value::ExternRef(Some(
                    caller.extern_ref(String::from("from callback"))?,
                ))])
            })
            .unwrap();
        let mut instance = module.instantiate(&[&echo, &create]).unwrap();
        {
            let reference = store
                .extern_ref(Payload(Rc::clone(&dropped), "hello 🦀".into()))
                .unwrap();
            let value = Value::ExternRef(Some(reference));
            instance.call("save", std::slice::from_ref(&value)).unwrap();
            assert_eq!(instance.call("read", &[]).unwrap(), [value]);
        }
        // A GC struct retained by the guest is now the only owner of Payload.
        instance.call("allocate", &[]).unwrap();
        assert_eq!(dropped.get(), 0);
        let result = instance.call("read", &[]).unwrap();
        let Value::ExternRef(Some(reference)) = &result[0] else {
            panic!("missing reference")
        };
        assert_eq!(reference.data::<Payload>().unwrap().1, "hello 🦀");
        assert_eq!(
            instance.call("null", &[]).unwrap(),
            [Value::ExternRef(None)]
        );
        let result = instance.call("new", &[]).unwrap();
        let Value::ExternRef(Some(reference)) = &result[0] else {
            panic!("missing reference")
        };
        assert_eq!(reference.data::<String>().unwrap(), "from callback");
    }
    assert_eq!(dropped.get(), 1);
}

#[test]
fn cross_store_references_are_rejected_before_native_calls() {
    let first = Store::new(()).unwrap();
    let second = Store::new(()).unwrap();
    let module = second
        .compile(
            &wat::parse_str(
                r#"(module
        (func (export "echo") (param externref) (result externref) local.get 0))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    let value = Value::ExternRef(Some(first.extern_ref(42).unwrap()));
    let error = instance.call("echo", &[value]).unwrap_err();
    assert!(error.to_string().contains("different store"));
    assert_eq!(
        instance.call("echo", &[Value::ExternRef(None)]).unwrap(),
        [Value::ExternRef(None)]
    );
}

#[test]
#[cfg(not(wee8_native_modules))]
fn unsupported_reference_boundary_types_are_rejected() {
    let store = Store::new(()).unwrap();
    for source in [
        r#"(module (type $s (struct)) (import "h" "f" (func (param (ref null $s)))))"#,
        r#"(module (type $s (struct)) (func (export "f") (result (ref $s)) struct.new $s))"#,
        r#"(module (type $a (array i32)) (global (export "g") (ref null $a) (ref.null $a)))"#,
        r#"(module (type $s (struct)) (table (export "t") 0 (ref null $s)))"#,
    ] {
        let error = match store.compile(&wat::parse_str(source).unwrap()) {
            Ok(_) => panic!("accepted {source}"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("C API"), "{source}: {error}");
    }
}

#[test]
fn typed_function_references_stay_inside_the_guest() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (type $f (func (result i32)))
        (func $answer (type $f) i32.const 42)
        (elem declare func $answer)
        (func (export "run") (result i32) ref.func $answer call_ref $f))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        module.instantiate(&[]).unwrap().call("run", &[]).unwrap(),
        [Value::I32(42)]
    );
}

#[test]
fn references_remain_usable_after_traps_and_callback_errors() {
    use wee8::{Error, ValType};
    let store = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "h" "fail" (func $fail (param externref) (result externref)))
        (func (export "trap") (param externref) (result externref) unreachable)
        (func (export "fail") (param externref) (result externref) local.get 0 call $fail)
        (func (export "echo") (param externref) (result externref) local.get 0))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let fail = store
        .host_func(&[ValType::EXTERNREF], &[ValType::EXTERNREF], |_, values| {
            let Value::ExternRef(Some(reference)) = &values[0] else {
                panic!("missing reference")
            };
            assert_eq!(reference.data::<String>().unwrap(), "still rooted");
            panic!("contained callback panic");
        })
        .unwrap();
    let mut instance = module.instantiate(&[&fail]).unwrap();
    let value = Value::ExternRef(Some(
        store.extern_ref(String::from("still rooted")).unwrap(),
    ));
    assert!(matches!(
        instance.call("trap", std::slice::from_ref(&value)),
        Err(Error::Trap(_))
    ));
    assert!(matches!(
        instance.call("fail", std::slice::from_ref(&value)),
        Err(Error::Host(_))
    ));
    let result = instance.call("echo", std::slice::from_ref(&value)).unwrap();
    assert_eq!(result, [value]);
    let Value::ExternRef(Some(reference)) = &result[0] else {
        panic!("missing reference")
    };
    assert_eq!(reference.data::<String>().unwrap(), "still rooted");
}

#[test]
fn nonnull_function_signatures_survive_introspection_and_serialization() {
    use wee8::{Error, HeapType, RefType, StringRef, ValType, Value};
    let store = Store::new(()).unwrap();
    let bytes = wat::parse_str(
        r#"(module
      (import "h" "echo" (func $echo (param (ref extern)) (result (ref extern))))
      (func (export "run") (param (ref extern)) (result (ref extern))
        local.get 0 call $echo))"#,
    )
    .unwrap();
    let compiled = store.compile(&bytes).unwrap().serialize();
    let module = store.module(&compiled).unwrap();
    let imports = module.imports().unwrap();
    let non_null = ValType::Ref(RefType::new(false, HeapType::Extern));
    assert_eq!(imports[0].params, [non_null]);
    assert_eq!(imports[0].results, [non_null]);
    let nullable = store
        .host_func(&[ValType::EXTERNREF], &[ValType::EXTERNREF], |_, args| {
            Ok(args.to_vec())
        })
        .unwrap();
    assert!(matches!(
        module.instantiate(&[&nullable]),
        Err(Error::Trap(_))
    ));
    let echo = store
        .host_func(&imports[0].params, &imports[0].results, |_, args| {
            Ok(args.to_vec())
        })
        .unwrap();
    let mut instance = module.instantiate(&[&echo]).unwrap();
    let string = StringRef::from_utf8(store.as_context(), "native")
        .unwrap()
        .into_extern_ref();
    let value = Value::ExternRef(Some(string));
    assert!(value.matches_ty(&ValType::EXTERNREF));
    assert!(value.matches_ty(&imports[0].params[0]));
    assert_eq!(
        instance.call("run", std::slice::from_ref(&value)).unwrap(),
        [value]
    );
    assert!(matches!(
        instance.call("run", &[Value::ExternRef(None)]),
        Err(Error::Interface(_))
    ));
    let null = store
        .host_func(&imports[0].params, &imports[0].results, |_, _| {
            Ok(vec![Value::ExternRef(None)])
        })
        .unwrap();
    let mut instance = module.instantiate(&[&null]).unwrap();
    let value = Value::ExternRef(Some(store.extern_ref(42).unwrap()));
    assert!(matches!(
        instance.call("run", &[value]),
        Err(Error::Host(_))
    ));
}

#[test]
fn nonnull_reference_extension_does_not_widen_other_external_types() {
    use wee8::{HeapType, RefType, Tag, ValType};
    let store = Store::new(()).unwrap();
    assert!(
        Tag::new(
            store.as_context(),
            &[ValType::Ref(RefType::new(false, HeapType::Extern))]
        )
        .is_err()
    );
    #[cfg(not(wee8_native_modules))]
    for source in [
        r#"(module (import "h" "tag" (tag (param (ref extern)))))"#,
        r#"(module (import "h" "global" (global (ref extern))))"#,
        r#"(module (import "h" "table" (table 1 (ref extern))))"#,
    ] {
        let error = store
            .compile(&wat::parse_str(source).unwrap())
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("unsupported in C API"),
            "{error}"
        );
    }
}

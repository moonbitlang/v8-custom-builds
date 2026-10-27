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

use wee8::{CompileOptions, Error, Store, StringRef, ValType, Value};

fn strings() -> CompileOptions {
    CompileOptions {
        js_string_builtins: true,
        imported_string_constants: Some("_".into()),
    }
}

#[test]
fn native_builtins_and_literals_survive_serialization() {
    let bytes = wat::parse_str(include_str!("../../../ci/c-api/strings.wat")).unwrap();
    let compiled = {
        let store = Store::new(()).unwrap();
        let module = store.compile_with_options(&bytes, &strings()).unwrap();
        assert!(module.imports().unwrap().is_empty());
        module
            .instantiate(&[])
            .unwrap()
            .call("_start", &[])
            .unwrap();
        module.serialize()
    };
    let store = Store::new(()).unwrap();
    let module = store.module(&compiled).unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    instance.call("_start", &[]).unwrap();
    let Value::ExternRef(Some(literal)) = instance.call("literal", &[]).unwrap().remove(0) else {
        panic!("expected native literal")
    };
    assert!(literal.data::<String>().is_none());
    assert_eq!(
        StringRef::try_from(literal).unwrap().to_utf8().unwrap(),
        "hello"
    );
    let units = [0x61, 0xd800, 0x62, 0xdc00, 0xd83d, 0xde04];
    let source = StringRef::from_utf16(store.as_context(), &units).unwrap();
    let args = [Value::ExternRef(Some(source.into_extern_ref()))];
    let Value::ExternRef(Some(result)) = instance.call("roundtrip", &args).unwrap().remove(0)
    else {
        panic!("expected native string")
    };
    let string = StringRef::try_from(result).unwrap();
    assert_eq!(string.len(), 6);
    assert_eq!(string.to_utf16(), units);
    assert!(string.to_utf8().is_err());
    assert_eq!(string.to_string_lossy(), "a\u{fffd}b\u{fffd}😄");
}

#[test]
fn native_string_callbacks_preserve_roots_and_utf16() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "host" "echo" (func $echo (param externref) (result externref)))
        (func (export "echo") (param externref) (result externref) local.get 0 call $echo))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let echo = store
        .host_func(
            &[ValType::EXTERNREF],
            &[ValType::EXTERNREF],
            |caller, args| {
                let Value::ExternRef(Some(reference)) = &args[0] else {
                    panic!("missing string")
                };
                assert!(reference.is_string());
                let string = StringRef::try_from(reference.clone())?;
                let result = StringRef::from_utf16(caller.as_context(), &string.to_utf16())?;
                Ok(vec![Value::ExternRef(Some(result.into_extern_ref()))])
            },
        )
        .unwrap();
    let mut instance = module.instantiate(&[&echo]).unwrap();
    for text in ["", "hello", "a\0b", "中文😄"] {
        let value = StringRef::from_utf8(store.as_context(), text).unwrap();
        assert_eq!(value.is_empty(), text.is_empty());
        let args = [Value::ExternRef(Some(value.into_extern_ref()))];
        let Value::ExternRef(Some(result)) = instance.call("echo", &args).unwrap().remove(0) else {
            panic!("missing string")
        };
        assert_eq!(
            StringRef::try_from(result).unwrap().to_utf8().unwrap(),
            text
        );
    }
    let foreign = store
        .extern_ref(String::from("not a native string"))
        .unwrap();
    assert!(!foreign.is_string());
    assert!(StringRef::try_from(foreign).is_err());
}

#[test]
fn compilation_options_do_not_leak_between_modules_or_stores() {
    let bytes = wat::parse_str(
        r#"(module
        (import "wasm:js-string" "test" (func $test (param externref) (result i32)))
        (func (export "run") (param externref) (result i32) local.get 0 call $test))"#,
    )
    .unwrap();
    let store = Store::new(()).unwrap();
    let native = store.compile_with_options(&bytes, &strings()).unwrap();
    assert!(native.imports().unwrap().is_empty());
    let ordinary = store.compile(&bytes).unwrap();
    assert_eq!(ordinary.imports().unwrap().len(), 1);
    let host = store
        .host_func(&[ValType::EXTERNREF], &[ValType::I32], |_, _| {
            Ok(vec![Value::I32(42)])
        })
        .unwrap();
    let mut instance = ordinary.instantiate(&[&host]).unwrap();
    assert_eq!(
        instance.call("run", &[Value::ExternRef(None)]).unwrap(),
        [Value::I32(42)]
    );
    let second = Store::new(()).unwrap();
    let native_copy = second.module(&native.serialize()).unwrap();
    assert!(native_copy.imports().unwrap().is_empty());
    let ordinary_copy = second.module(&ordinary.serialize()).unwrap();
    assert_eq!(ordinary_copy.imports().unwrap().len(), 1);
    assert_eq!(
        native_copy
            .instantiate(&[])
            .unwrap()
            .call("run", &[Value::ExternRef(None)])
            .unwrap(),
        [Value::I32(0)]
    );
}

#[test]
fn options_only_exempt_validated_native_imports_from_boundary_checks() {
    let store = Store::new(()).unwrap();
    for wat in [
        r#"(module (import "wasm:js-string" "length" (func (param i32) (result i32))))"#,
        r#"(module (import "_" "bad" (global (mut externref))))"#,
        r#"(module (type $a (array (mut i8))) (import "wasm:js-string" "fromCharCodeArray" (func (param (ref null $a) i32 i32) (result (ref extern)))))"#,
    ] {
        assert!(
            store
                .compile_with_options(&wat::parse_str(wat).unwrap(), &strings())
                .is_err(),
            "{wat}"
        );
    }
    // Unknown ordinary imports remain host imports, even in the builtin namespace.
    let bytes = wat::parse_str(r#"(module (import "wasm:js-string" "unknown" (func)))"#).unwrap();
    assert_eq!(
        store
            .compile_with_options(&bytes, &strings())
            .unwrap()
            .imports()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn literal_namespace_is_explicit_and_owned_by_compiled_module() {
    let store = Store::new(()).unwrap();
    for namespace in ["", "_", "literal\0strings"] {
        let escaped = namespace.replace('\0', "\\00");
        let bytes = wat::parse_str(format!(
            r#"(module
          (import "{escaped}" "😄" (global $s (ref extern)))
          (func (export "get") (result externref) global.get $s))"#
        ))
        .unwrap();
        let compiled = {
            let options = CompileOptions {
                js_string_builtins: false,
                imported_string_constants: Some(namespace.into()),
            };
            store
                .compile_with_options(&bytes, &options)
                .unwrap()
                .serialize()
        };
        let module = store.module(&compiled).unwrap();
        let Value::ExternRef(Some(result)) = module
            .instantiate(&[])
            .unwrap()
            .call("get", &[])
            .unwrap()
            .remove(0)
        else {
            panic!("missing string")
        };
        assert_eq!(
            StringRef::try_from(result).unwrap().to_utf8().unwrap(),
            "😄"
        );
    }
    let invalid = CompileOptions {
        js_string_builtins: false,
        imported_string_constants: Some("非ASCII".into()),
    };
    assert!(
        store
            .compile_with_options(&wat::parse_str("(module)").unwrap(), &invalid)
            .is_err()
    );
}

#[test]
fn native_string_references_cannot_cross_stores() {
    let first = Store::new(()).unwrap();
    let second = Store::new(()).unwrap();
    let value = StringRef::from_utf8(first.as_context(), "hello").unwrap();
    let module = second.compile(&wat::parse_str("(module (func (export \"echo\") (param externref) (result externref) local.get 0))").unwrap()).unwrap();
    let error = module
        .instantiate(&[])
        .unwrap()
        .call("echo", &[Value::ExternRef(Some(value.into_extern_ref()))])
        .unwrap_err();
    assert!(matches!(error, Error::Interface(_)));
}

#[test]
fn filtered_imports_preserve_declaration_order_with_duplicate_host_names() {
    let store = Store::new(()).unwrap();
    let bytes = wat::parse_str(
        r#"(module
        (import "host" "integer" (func $integer (result i32)))
        (import "_" "hello" (global $hello (ref extern)))
        (import "wasm:js-string" "length" (func $length (param externref) (result i32)))
        (import "host" "value" (func $string (param externref) (result externref)))
        (import "host" "value" (func $again (param externref) (result externref)))
        (func (export "run") (result i32)
            global.get $hello call $string call $again call $length call $integer i32.add))"#,
    )
    .unwrap();
    let module = store.compile_with_options(&bytes, &strings()).unwrap();
    let imports = module.imports().unwrap();
    assert_eq!(imports.len(), 3);
    assert_eq!(imports[0].results, [ValType::I32]);
    assert_eq!(imports[1].params, [ValType::EXTERNREF]);
    let integer = store
        .host_func(&[], &[ValType::I32], |_, _| Ok(vec![Value::I32(37)]))
        .unwrap();
    let echo = store
        .host_func(&[ValType::EXTERNREF], &[ValType::EXTERNREF], |_, args| {
            Ok(args.to_vec())
        })
        .unwrap();
    assert_eq!(
        module
            .instantiate(&[&integer, &echo, &echo])
            .unwrap()
            .call("run", &[])
            .unwrap(),
        [Value::I32(42)]
    );
}

#[test]
fn native_strings_survive_exception_payload_transport() {
    use wee8::{Exception, Extern, Tag};
    let store = Store::new(()).unwrap();
    let tag = Tag::new(store.as_context(), &[ValType::EXTERNREF]).unwrap();
    let throw = store
        .host_func(&[], &[], move |caller, _| {
            let string = StringRef::from_utf16(caller.as_context(), &[0xd800, 0, 65])?;
            let exception = Exception::new(
                caller.as_context(),
                tag,
                &[Value::ExternRef(Some(string.into_extern_ref()))],
            )?;
            Err(caller.throw(&exception))
        })
        .unwrap();
    let module = store
        .compile_with_options(
            &wat::parse_str(
                r#"(module
        (import "host" "tag" (tag $tag (param externref)))
        (import "wasm:js-string" "length" (func $length (param externref) (result i32)))
        (import "host" "throw" (func $throw))
        (func (export "caught") (result i32)
            block $caught (result externref)
                try_table (catch $tag $caught) call $throw end
                unreachable
            end
            call $length)
        (func (export "uncaught") call $throw))"#,
            )
            .unwrap(),
            &strings(),
        )
        .unwrap();
    let mut instance = module
        .instantiate_with_imports(&[Extern::Tag(tag), Extern::Func(&throw)])
        .unwrap();
    assert_eq!(instance.call("caught", &[]).unwrap(), [Value::I32(3)]);
    assert!(matches!(
        instance.call("uncaught", &[]),
        Err(Error::ThrownException)
    ));
    let exception = store.take_exception().unwrap();
    let Value::ExternRef(Some(value)) = exception.args(tag).unwrap().remove(0) else {
        panic!("missing string")
    };
    drop(exception);
    assert_eq!(
        StringRef::try_from(value).unwrap().to_utf16(),
        [0xd800, 0, 65]
    );
}

#[test]
fn native_string_builtin_errors_are_guest_traps() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile_with_options(
            &wat::parse_str(
                r#"(module
        (import "_" "a" (global $a (ref extern)))
        (import "wasm:js-string" "cast" (func $cast (param externref) (result (ref extern))))
        (import "wasm:js-string" "charCodeAt" (func $char (param externref i32) (result i32)))
        (func (export "cast") (param externref) local.get 0 call $cast drop)
        (func (export "bounds") global.get $a i32.const 1 call $char drop))"#,
            )
            .unwrap(),
            &strings(),
        )
        .unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    let foreign = store.extern_ref("host object").unwrap();
    for args in [[Value::ExternRef(None)], [Value::ExternRef(Some(foreign))]] {
        assert!(matches!(instance.call("cast", &args), Err(Error::Trap(_))));
    }
    assert!(matches!(instance.call("bounds", &[]), Err(Error::Trap(_))));
}

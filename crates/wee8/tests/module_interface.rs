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

use wee8::Store;

#[test]
fn unsupported_host_exports_do_not_reject_the_module() {
    let store = Store::new(()).unwrap();
    let bytes = wat::parse_str(
        r#"(module
        (func (export "vector") (result v128) (v128.const i32x4 1 2 3 4))
        (global (export "vector_global") v128 (v128.const i32x4 1 2 3 4))
        (func (export "run") (result i32) i32.const 42))"#,
    )
    .unwrap();
    let compiled = store.compile(&bytes).unwrap().serialize();
    let second = Store::new(()).unwrap();
    let module = second.module(&compiled).unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    assert!(instance.has_export("vector_global"));
    assert!(instance.get_func("vector").is_err());
    assert_eq!(instance.call("run", &[]).unwrap(), [wee8::Value::I32(42)]);
}

#[test]
fn opaque_functions_link_vectors_and_gc_references_without_host_conversion() {
    for (ty, value, consume) in [
        ("v128", "v128.const i32x4 42 0 0 0", "i32x4.extract_lane 0"),
        ("(ref $s)", "i32.const 42 struct.new $s", "struct.get $s 0"),
    ] {
        let store = Store::new(()).unwrap();
        let producer = store
            .compile(
                &wat::parse_str(format!(
                    r#"(module
            (type $s (struct (field i32)))
            (func (export "value") (result {ty}) {value}))"#
                ))
                .unwrap(),
            )
            .unwrap();
        let instance = producer.instantiate(&[]).unwrap();
        assert!(instance.get_func("value").is_err());
        let export = instance.get_export("value").unwrap();
        let consumer = store
            .compile(
                &wat::parse_str(format!(
                    r#"(module
            (type $s (struct (field i32)))
            (import "p" "value" (func $value (result {ty})))
            (func (export "run") (result i32) call $value {consume}))"#
                ))
                .unwrap(),
            )
            .unwrap();
        assert!(consumer.imports().is_err());
        let mut linked = consumer
            .instantiate_with_imports(&[wee8::Extern::Export(&export)])
            .unwrap();
        assert_eq!(linked.call("run", &[]).unwrap(), [wee8::Value::I32(42)]);
    }
}

#[test]
fn opaque_links_preserve_native_type_checks_store_identity_and_roots() {
    let store = Store::new(()).unwrap();
    let consumer_bytes = wat::parse_str(
        r#"(module
        (import "p" "value" (func $value (result i32)))
        (func (export "run") (result i32) call $value))"#,
    )
    .unwrap();
    let consumer = store.compile(&consumer_bytes).unwrap();
    let producer = store
        .compile(
            &wat::parse_str(
                r#"(module
        (func (export "value") (result i32) i32.const 42)
        (func (export "wrong") (result i64) i64.const 42))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut linked = {
        let instance = producer.instantiate(&[]).unwrap();
        let wrong = instance.get_export("wrong").unwrap();
        assert!(
            consumer
                .instantiate_with_imports(&[wee8::Extern::Export(&wrong)])
                .is_err()
        );
        let export = instance.get_export("value").unwrap();
        let second = Store::new(()).unwrap();
        let foreign = second.compile(&consumer_bytes).unwrap();
        assert!(
            foreign
                .instantiate_with_imports(&[wee8::Extern::Export(&export)])
                .is_err()
        );
        consumer
            .instantiate_with_imports(&[wee8::Extern::Export(&export)])
            .unwrap()
    };
    // The importing instance, not the borrowed export handle, now roots the code.
    drop(producer);
    assert_eq!(linked.call("run", &[]).unwrap(), [wee8::Value::I32(42)]);
}

#[test]
fn opaque_globals_tables_and_tags_keep_their_native_types() {
    let store = Store::new(()).unwrap();
    let producer = store
        .compile(
            &wat::parse_str(
                r#"(module
        (type $s (struct (field i32)))
        (global (export "vector") v128 (v128.const i32x4 42 0 0 0))
        (table (export "table") 1 (ref null $s))
        (tag (export "tag") (param v128))
        (global (export "exception") exnref (ref.null exn)))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let instance = producer.instantiate(&[]).unwrap();
    let exports =
        ["vector", "table", "tag", "exception"].map(|name| instance.get_export(name).unwrap());
    let consumer = store
        .compile(
            &wat::parse_str(
                r#"(module
        (type $s (struct (field i32)))
        (import "p" "vector" (global $vector v128))
        (import "p" "table" (table $table 1 (ref null $s)))
        (import "p" "tag" (tag $tag (param v128)))
        (import "p" "exception" (global exnref))
        (func (export "run") (result i32)
            i32.const 0 i32.const 42 struct.new $s table.set $table
            (block $caught (result v128)
                (try_table (catch $tag $caught)
                    global.get $vector throw $tag)
                unreachable)
            i32x4.extract_lane 0))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(consumer.import_types().is_err());
    let mut linked = consumer
        .instantiate_with_imports(&exports.iter().map(wee8::Extern::Export).collect::<Vec<_>>())
        .unwrap();
    assert_eq!(linked.call("run", &[]).unwrap(), [wee8::Value::I32(42)]);
}

#[test]
fn shared_memory_links_and_executes_but_cannot_be_borrowed_exclusively() {
    let store = Store::new(()).unwrap();
    let producer = store
        .compile(&wat::parse_str(r#"(module (memory (export "memory") 1 1 shared))"#).unwrap())
        .unwrap();
    let instance = producer.instantiate(&[]).unwrap();
    let export = instance.get_export("memory").unwrap();
    let consumer = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "p" "memory" (memory 1 1 shared))
        (import "h" "check" (func $check))
        (export "memory" (memory 0))
        (func (export "run") (result i32)
            i32.const 0 i32.const 42 i32.atomic.store
            call $check
            i32.const 0 i32.atomic.load))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(consumer.import_types().is_err());
    let check = store
        .host_func(&[], &[], |caller, _| {
            assert!(
                caller
                    .with_memory(|_, _| ())
                    .unwrap_err()
                    .to_string()
                    .contains("shared memory")
            );
            Ok(vec![])
        })
        .unwrap();
    let mut linked = consumer
        .instantiate_with_imports(&[wee8::Extern::Export(&export), wee8::Extern::Func(&check)])
        .unwrap();
    assert_eq!(linked.call("run", &[]).unwrap(), [wee8::Value::I32(42)]);
}

#[test]
fn shared_memory_survives_producer_drop_and_growth_across_instances() {
    use wee8::{Extern, Value::I32};

    let store = Store::new(()).unwrap();
    let consumer = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "p" "memory" (memory 1 3 shared))
        (func (export "size") (result i32) memory.size)
        (func (export "grow") (param i32) (result i32) local.get 0 memory.grow)
        (func (export "read") (param i32) (result i32) local.get 0 i32.atomic.load)
        (func (export "write") (param i32 i32)
            local.get 0 local.get 1 i32.atomic.store))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let (mut first, mut second) = {
        let producer = store
            .compile(&wat::parse_str(r#"(module (memory (export "memory") 1 3 shared))"#).unwrap())
            .unwrap();
        let instance = producer.instantiate(&[]).unwrap();
        let memory = instance.get_export("memory").unwrap();
        (
            consumer
                .instantiate_with_imports(&[Extern::Export(&memory)])
                .unwrap(),
            consumer
                .instantiate_with_imports(&[Extern::Export(&memory)])
                .unwrap(),
        )
    };
    // The two importing instances retain the memory after all producer handles
    // have gone. Growth must update both instances without losing existing data.
    drop(consumer);
    first.call("write", &[I32(0), I32(17)]).unwrap();
    assert_eq!(second.call("read", &[I32(0)]).unwrap(), [I32(17)]);
    assert_eq!(first.call("grow", &[I32(1)]).unwrap(), [I32(1)]);
    for instance in [&mut first, &mut second] {
        assert_eq!(instance.call("size", &[]).unwrap(), [I32(2)]);
        assert_eq!(instance.call("read", &[I32(0)]).unwrap(), [I32(17)]);
        assert_eq!(instance.call("read", &[I32(65536)]).unwrap(), [I32(0)]);
    }
    second.call("write", &[I32(65536), I32(42)]).unwrap();
    assert_eq!(first.call("read", &[I32(65536)]).unwrap(), [I32(42)]);
    drop(first);
    assert_eq!(second.call("grow", &[I32(1)]).unwrap(), [I32(2)]);
    assert_eq!(second.call("read", &[I32(65536)]).unwrap(), [I32(42)]);
    assert_eq!(second.call("read", &[I32(131072)]).unwrap(), [I32(0)]);
    assert_eq!(second.call("grow", &[I32(1)]).unwrap(), [I32(-1)]);
    assert_eq!(second.call("size", &[]).unwrap(), [I32(3)]);
    assert_eq!(second.call("read", &[I32(65536)]).unwrap(), [I32(42)]);
}

#[test]
fn unknown_builtin_names_with_gc_signatures_remain_ordinary_imports() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile_with_options(
            &wat::parse_str(
                r#"(module
        (type $a (array (mut i16)))
        (import "wasm:js-string" "unknown" (func (param (ref null $a)))))"#,
            )
            .unwrap(),
            &wee8::CompileOptions {
                js_string_builtins: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(module.imports().is_err());
    assert!(module.instantiate(&[]).is_err());
}

#[test]
fn unsupported_exception_payloads_remain_opaque_and_do_not_poison_calls() {
    for (ty, value) in [
        ("v128", "v128.const i32x4 42 0 0 0"),
        ("funcref", "ref.null func"),
        ("(ref $s)", "struct.new $s"),
    ] {
        let store = Store::new(()).unwrap();
        let module = store
            .compile(
                &wat::parse_str(format!(
                    r#"(module
            (type $s (struct))
            (tag $tag (export "tag") (param {ty}))
            (func (export "throw") {value} throw $tag)
            (func (export "run") (result i32) i32.const 42))"#
                ))
                .unwrap(),
            )
            .unwrap();
        let mut instance = module.instantiate(&[]).unwrap();
        let tag = instance.tag("tag").unwrap();
        assert!(matches!(
            instance.call("throw", &[]),
            Err(wee8::Error::ThrownException)
        ));
        let exception = store.take_exception().unwrap();
        assert!(exception.is(tag).unwrap());
        assert!(
            exception
                .args(tag)
                .unwrap_err()
                .to_string()
                .contains("unsupported payload")
        );
        assert!(wee8::Exception::new(store.as_context(), tag, &[wee8::Value::I32(42)]).is_err());
        assert_eq!(instance.call("run", &[]).unwrap(), [wee8::Value::I32(42)]);
        assert!(store.take_exception().is_none());
    }
}

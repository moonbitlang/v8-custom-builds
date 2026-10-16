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

use wee8::{Error, Kind, Store, Value};

#[test]
fn internal_simd_survives_serialization_and_scalar_host_calls() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "host" "check" (func $check (param i32) (result i32)))
        (memory (export "memory") 1)
        (global $vector v128 (v128.const i32x4 20 21 22 23))
        (func $twice (param v128) (result v128)
            local.get 0 local.get 0 i32x4.add)
        (func (export "run") (result i32)
            i32.const 0 global.get $vector call $twice v128.store
            i32.const 0 v128.load i32x4.extract_lane 1 call $check))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let compiled = module.serialize();
    let second = Store::new(()).unwrap();
    let module = second.module(&compiled).unwrap();
    assert_eq!(module.imports().unwrap().len(), 1);
    let check = second
        .host_func(&[Kind::I32], &[Kind::I32], |caller, values| {
            caller.with_memory(|_, bytes| {
                assert_eq!(i32::from_le_bytes(bytes[0..4].try_into().unwrap()), 40);
            })?;
            assert_eq!(values, [Value::I32(42)]);
            Ok(vec![values[0].clone()])
        })
        .unwrap();
    let mut instance = module.instantiate(&[&check]).unwrap();
    assert_eq!(instance.call("run", &[]).unwrap(), [Value::I32(42)]);
}

#[test]
fn vector_imports_and_exports_are_rejected_before_native_introspection() {
    let store = Store::new(()).unwrap();
    for source in [
        r#"(module (import "host" "f" (func (param v128))))"#,
        r#"(module (import "host" "f" (func (result v128))))"#,
        // Duplicate import names must not hide the unsupported signature.
        r#"(module (import "host" "f" (func (param i32)))
                   (import "host" "f" (func (param v128))))"#,
        r#"(module (import "host" "g" (global v128)))"#,
        r#"(module (import "host" "g" (global (mut v128))))"#,
        r#"(module (func (export "f") (param v128)))"#,
        r#"(module (func (export "f") (result v128) (v128.const i32x4 0 0 0 0)))"#,
        r#"(module (global (export "g") v128 (v128.const i32x4 0 0 0 0)))"#,
        r#"(module (global (export "g") (mut v128) (v128.const i32x4 0 0 0 0)))"#,
    ] {
        let result = store.compile(&wat::parse_str(source).unwrap());
        assert!(
            matches!(result, Err(Error::Interface(ref message))
            if message.contains("v128") && message.contains("C API")),
            "{source}"
        );
    }
}

#[test]
fn simd_does_not_enable_other_proposals() {
    let store = Store::new(()).unwrap();
    for source in [
        "(module (memory 1 1 shared))",
        "(module (tag (param i32)))",
        "(module (func (param v128) (result v128) local.get 0 local.get 0 i8x16.relaxed_swizzle))",
    ] {
        assert!(
            matches!(
                store.compile(&wat::parse_str(source).unwrap()),
                Err(Error::Interface(_))
            ),
            "{source}"
        );
    }
}

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

use wee8::{Error, Store, Value};

// Exercise the additional proposals through scalar exports and serialization,
// using the published engine rather than testing only wasmparser acceptance.
#[test]
fn released_v8_features_execute_through_scalar_exports() {
    for (proposal, source) in [
        (
            "sign extension",
            r#"(module
            (func (export "run") (result i64)
                i32.const 255 i32.extend8_s i64.extend_i32_s
                i32.const 65535 i32.extend16_s i64.extend_i32_s i64.add
                i64.const 255 i64.extend8_s i64.add
                i64.const 65535 i64.extend16_s i64.add
                i64.const 4294967295 i64.extend32_s i64.add
                i64.const 47 i64.add))"#,
        ),
        (
            "tail calls",
            r#"(module
            (func $count (param i32) (result i64)
                local.get 0 i32.eqz
                if (result i64) i64.const 42
                else local.get 0 i32.const 1 i32.sub return_call $count end)
            (func (export "run") (result i64) i32.const 100000 return_call $count))"#,
        ),
        (
            "extended constants",
            r#"(module
            (global $answer i64 (i64.add (i64.const 20) (i64.const 22)))
            (func (export "run") (result i64) global.get $answer))"#,
        ),
        (
            "relaxed SIMD",
            r#"(module
            (func (export "run") (result i64)
                v128.const i32x4 42 0 0 0
                v128.const i8x16 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15
                i8x16.relaxed_swizzle i32x4.extract_lane 0 i64.extend_i32_u))"#,
        ),
        (
            "multiple memories",
            r#"(module
            (memory $first 1) (memory $second 1)
            (data (memory $second) (i32.const 0) "\2a")
            (func (export "run") (result i64)
                i32.const 0 i32.const 0 i32.const 1 memory.copy $first $second
                i32.const 0 i64.load8_u $first))"#,
        ),
        (
            "internal shared memory and atomics",
            r#"(module
            (memory 1 1 shared)
            (func (export "run") (result i64)
                i32.const 0 i64.const 42 i64.atomic.store
                i32.const 0 i64.atomic.load))"#,
        ),
    ] {
        let bytes = wat::parse_str(source).unwrap();
        let store = Store::new(()).unwrap();
        let module = store
            .compile(&bytes)
            .unwrap_or_else(|e| panic!("{proposal}: {e}"));
        let compiled = module.serialize();
        let next = Store::new(()).unwrap();
        let restored = next.module(&compiled).unwrap();
        for module in [&module, &restored] {
            let mut instance = module.instantiate(&[]).unwrap();
            assert_eq!(
                instance.call("run", &[]).unwrap(),
                [Value::I64(42)],
                "{proposal}"
            );
        }
    }
}

#[test]
fn shared_memory_is_rejected_at_the_host_boundary() {
    let store = Store::new(()).unwrap();
    for source in [
        r#"(module (import "host" "memory" (memory 1 1 shared)))"#,
        r#"(module (memory (export "memory") 1 1 shared))"#,
        // Any export could be inspected through the native C API, not only the
        // conventionally named memory borrowed by Caller::with_memory.
        r#"(module (memory (export "other") 1 1 shared))"#,
    ] {
        assert!(
            matches!(store.compile(&wat::parse_str(source).unwrap()),
            Err(Error::Interface(message)) if message.contains("shared memory is unsupported in C API")),
            "{source}"
        );
    }
}

#[test]
fn proposals_unavailable_in_the_release_are_rejected() {
    let store = Store::new(()).unwrap();
    for source in [
        // The .4 native release deliberately disables legacy exception opcodes.
        r#"(module
            (tag $answer (param i64))
            (func (export "run") (result i64)
                try (result i64)
                    i64.const 42 throw $answer
                catch $answer end))"#,
        "(module (memory i64 1))",
        "(module (memory 1 (pagesize 1)))",
        "(component)",
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

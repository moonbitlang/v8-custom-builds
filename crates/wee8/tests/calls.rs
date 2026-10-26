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

use wee8::{Error, Extern, Memory, MemoryType, Store, ValType, Value};

#[test]
fn cached_and_typed_calls_share_signatures_and_trap_recovery() {
    let store = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
      (memory (export "memory") 1)
      (func (export "id") (param i32) (result i32) local.get 0)
      (func (export "mix") (param i32 i64 f32 f64) (result f64) local.get 3)
      (func (export "i64") (param i64) (result i64) local.get 0)
      (func (export "f32") (param f32) (result f32) local.get 0)
      (func (export "unit"))
      (func (export "trap") unreachable))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let mut instance = module.instantiate(&[]).unwrap();
    drop(module);
    assert!(instance.get_func("missing").is_err());
    assert!(instance.get_func("memory").is_err());
    let id = instance.get_func("id").unwrap();
    assert_eq!(id.params(), &[ValType::I32]);
    assert_eq!(id.results(), &[ValType::I32]);
    assert!(id.typed::<i64, i32>().is_err());
    assert!(id.typed::<i32, f64>().is_err());
    assert!(id.typed::<(), i32>().is_err());
    assert!(id.call(&[Value::I64(42)]).is_err());
    assert!(id.call(&[]).is_err());
    let typed = id.typed::<i32, i32>().unwrap();
    assert_eq!(typed.call(42).unwrap(), 42);
    assert_eq!(id.call(&[Value::I32(-1)]).unwrap(), [Value::I32(-1)]);
    assert!(matches!(
        instance
            .get_func("trap")
            .unwrap()
            .typed::<(), ()>()
            .unwrap()
            .call(()),
        Err(Error::Trap(_))
    ));
    assert_eq!(typed.call(17).unwrap(), 17);
    instance
        .get_func("unit")
        .unwrap()
        .typed::<(), ()>()
        .unwrap()
        .call(())
        .unwrap();
    let mix = instance
        .get_func("mix")
        .unwrap()
        .typed::<(i32, i64, f32, f64), f64>()
        .unwrap();
    assert_eq!(
        mix.call((1, i64::MAX, 2.0, -0.0)).unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        instance
            .get_func("i64")
            .unwrap()
            .typed::<(i64,), i64>()
            .unwrap()
            .call((i64::MIN,))
            .unwrap(),
        i64::MIN
    );
    let bits = 0x7fc0_1234;
    assert_eq!(
        instance
            .get_func("f32")
            .unwrap()
            .typed::<f32, f32>()
            .unwrap()
            .call(f32::from_bits(bits))
            .unwrap()
            .to_bits(),
        bits
    );
    assert_eq!(
        instance.call("id", &[Value::I32(3)]).unwrap(),
        [Value::I32(3)]
    );
}

#[test]
fn cached_reference_calls_keep_roots_nullability_and_store_checks() {
    let store = Store::new(()).unwrap();
    let other = Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
      (type $cell (struct (field i32)))
      (func (export "make") (param i32) (result externref)
        local.get 0 struct.new $cell extern.convert_any)
      (func (export "read") (param (ref extern)) (result i32)
        local.get 0 any.convert_extern ref.cast (ref $cell) struct.get $cell 0)
      (func (export "echo") (param externref) (result externref) local.get 0)
      (func (export "trap") (param externref) (result externref) unreachable))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let instance = module.instantiate(&[]).unwrap();
    let make = instance.get_func("make").unwrap();
    let read = instance.get_func("read").unwrap();
    let echo = instance.get_func("echo").unwrap();
    assert!(echo.typed::<i64, i64>().is_err());
    let root = make.call(&[Value::I32(42)]).unwrap();
    for i in 0..1000 {
        make.call(&[Value::I32(i)]).unwrap();
    }
    assert_eq!(read.call(&root).unwrap(), [Value::I32(42)]);
    assert!(read.call(&[Value::ExternRef(None)]).is_err());
    assert!(
        echo.call(&[Value::ExternRef(Some(other.extern_ref(7).unwrap()))])
            .is_err()
    );
    assert!(matches!(
        instance.get_func("trap").unwrap().call(&root),
        Err(Error::Trap(_))
    ));
    let retained = echo.call(&root).unwrap();
    drop(root);
    assert_eq!(read.call(&retained).unwrap(), [Value::I32(42)]);
    assert_eq!(
        echo.call(&[Value::ExternRef(None)]).unwrap(),
        [Value::ExternRef(None)]
    );
}

#[test]
fn cached_calls_refresh_memory_views_and_preserve_borrow_guards() {
    let store = Store::new(()).unwrap();
    let memory = Memory::new(
        store.as_context(),
        MemoryType {
            minimum: 1,
            maximum: Some(3),
        },
    )
    .unwrap();
    let check = store
        .host_func(&[], &[ValType::I32], |caller, _| {
            let length = caller.with_memory(|_, bytes| bytes.len())?;
            Ok(vec![Value::I32(length as i32)])
        })
        .unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
      (import "h" "memory" (memory 1 3))
      (import "h" "check" (func $check (result i32)))
      (export "memory" (memory 0))
      (func (export "size") (result i32) call $check)
      (func (export "grow") (result i32) i32.const 1 memory.grow))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let instance = module
        .instantiate_with_imports(&[Extern::Memory(memory), Extern::Func(&check)])
        .unwrap();
    let size = instance
        .get_func("size")
        .unwrap()
        .typed::<(), i32>()
        .unwrap();
    let grow = instance
        .get_func("grow")
        .unwrap()
        .typed::<(), i32>()
        .unwrap();
    assert_eq!(size.call(()).unwrap(), 65536);
    memory
        .with_data(store.as_context(), |_| {
            assert!(size.call(()).is_err());
            assert!(grow.call(()).is_err());
        })
        .unwrap();
    assert_eq!(grow.call(()).unwrap(), 1);
    assert_eq!(size.call(()).unwrap(), 2 * 65536);
    assert_eq!(
        instance.get_func("size").unwrap().call(&[]).unwrap(),
        [Value::I32(2 * 65536)]
    );

    let other_memory = Memory::new(
        store.as_context(),
        MemoryType {
            minimum: 3,
            maximum: Some(3),
        },
    )
    .unwrap();
    let other = module
        .instantiate_with_imports(&[Extern::Memory(other_memory), Extern::Func(&check)])
        .unwrap();
    let other_size = other.get_func("size").unwrap().typed::<(), i32>().unwrap();
    assert_eq!(other_size.call(()).unwrap(), 3 * 65536);
    assert_eq!(size.call(()).unwrap(), 2 * 65536);
}

#[test]
fn typed_calls_recover_from_host_errors_and_bad_callback_results() {
    let store = Store::new(()).unwrap();
    let host = store
        .host_func(&[ValType::I32], &[ValType::I32], |_, args| match args {
            [Value::I32(0)] => Err("host failure".into()),
            [Value::I32(1)] => Ok(vec![Value::I64(1)]),
            [Value::I32(value)] => Ok(vec![Value::I32(*value)]),
            _ => unreachable!(),
        })
        .unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
      (import "h" "run" (func $run (param i32) (result i32)))
      (func (export "run") (param i32) (result i32) local.get 0 call $run))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let instance = module.instantiate(&[&host]).unwrap();
    let run = instance
        .get_func("run")
        .unwrap()
        .typed::<i32, i32>()
        .unwrap();
    drop(host);
    drop(module);
    for value in [0, 1] {
        assert!(matches!(run.call(value), Err(Error::Host(_))));
        assert_eq!(run.call(42).unwrap(), 42);
    }
}

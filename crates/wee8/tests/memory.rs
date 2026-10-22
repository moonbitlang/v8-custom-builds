use wee8::{Extern, ExternType, Memory, MemoryType, Store, Value};

#[test]
fn imported_memory_is_shared_with_guest_and_checked_during_host_borrows() {
    let store = Store::new(()).unwrap();
    let memory = Memory::new(
        store.as_context(),
        MemoryType {
            minimum: 1,
            maximum: Some(3),
        },
    )
    .unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
        (import "h" "memory" (memory 1 3))
        (import "h" "check" (func $check))
        (export "memory" (memory 0))
        (func (export "run") (result i32)
          i32.const 0 i32.const 42 i32.store8
          call $check
          i32.const 1 memory.grow drop
          i32.const 65536 i32.const 7 i32.store8
          i32.const 0 i32.load8_u))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        module.import_types().unwrap()[0].ty,
        ExternType::Memory(MemoryType {
            minimum: 1,
            maximum: Some(3)
        })
    );
    let callback = store
        .host_func(&[], &[], move |caller, _| {
            memory.with_data(caller.as_context(), |bytes| {
                assert_eq!(bytes[0], 42);
                assert!(memory.grow(caller.as_context(), 1).is_err());
                assert!(caller.with_memory(|_, _| ()).is_err());
            })?;
            Ok(vec![])
        })
        .unwrap();
    let mut instance = module
        .instantiate_with_imports(&[Extern::Memory(memory), Extern::Func(&callback)])
        .unwrap();
    assert_eq!(instance.call("run", &[]).unwrap(), [Value::I32(42)]);
    memory
        .with_data(store.as_context(), |bytes| {
            assert_eq!(bytes.len(), 2 * 65536);
            assert_eq!(bytes[65536], 7);
            assert!(instance.call("run", &[]).is_err());
            assert!(
                module
                    .instantiate_with_imports(&[Extern::Memory(memory), Extern::Func(&callback)])
                    .is_err()
            );
        })
        .unwrap();
    assert_eq!(memory.grow(store.as_context(), 1).unwrap(), 2);
    assert!(memory.grow(store.as_context(), 1).is_err());
    let other = Store::new(()).unwrap();
    assert!(memory.with_data(other.as_context(), |_| ()).is_err());
    assert!(memory.grow(other.as_context(), 0).is_err());
    drop(instance);
    drop(callback);
    drop(module);
    drop(store);
    assert!(memory.with_data(other.as_context(), |_| ()).is_err());
}

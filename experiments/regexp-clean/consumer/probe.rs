use wee8::{Kind, Store, Value};

fn main() {
    let bytes = wat::parse_str(
        r#"(module
            (import "host" "i32" (func $i32 (param i32) (result i32)))
            (import "host" "i64" (func $i64 (param i64) (result i64)))
            (import "host" "f32" (func $f32 (param f32) (result f32)))
            (import "host" "f64" (func $f64 (param f64) (result f64)))
            (memory (export "memory") 1 2)
            (func $start i64.const 1099511627776 call $i64 drop)
            (start $start)
            (func (export "i32") (param i32) (result i32) local.get 0 call $i32)
            (func (export "i64") (param i64) (result i64) local.get 0 call $i64)
            (func (export "f32") (param f32) (result f32) local.get 0 call $f32)
            (func (export "f64") (param f64) (result f64) local.get 0 call $f64)
            (func (export "grow") (result i32) i32.const 1 memory.grow)
            (func (export "fresh") (result i32) i32.const 65536 i32.load)
            (func (export "trap") unreachable))"#,
    )
    .unwrap();
    let compiled = {
        let store = Store::new(()).unwrap();
        assert!(store.compile(b"not wasm").is_err());
        let module = store.compile(&bytes).unwrap();
        module.serialize()
    };

    // Exercise deserialization and start/callback execution after the original
    // store is gone, then repeat with independent stores and import handles.
    for _ in 0..3 {
        let store = Store::new(()).unwrap();
        let module = store.module(&compiled).unwrap();
        let hosts = [Kind::I32, Kind::I64, Kind::F32, Kind::F64].map(|kind| {
            store
                .host_func(&[kind], &[kind], |_, args| Ok(args.to_vec()))
                .unwrap()
        });
        let imports = hosts.iter().collect::<Vec<_>>();
        let mut instance = module.instantiate(&imports).unwrap();
        for (name, value) in [
            ("i32", Value::I32(-123)),
            ("i64", Value::I64(1 << 40)),
            ("f32", Value::F32(1.25)),
            ("f64", Value::F64(-3.5)),
        ] {
            assert_eq!(instance.call(name, &[value]).unwrap(), [value]);
        }
        assert_eq!(instance.call("grow", &[]).unwrap(), [Value::I32(1)]);
        assert_eq!(instance.call("fresh", &[]).unwrap(), [Value::I32(0)]);
        assert!(instance.call("trap", &[]).is_err());
        assert_eq!(
            instance.call("i64", &[Value::I64(42)]).unwrap(),
            [Value::I64(42)]
        );
    }
    println!(
        "All numeric kinds, serialization, fresh stores, starts, growth and trap recovery passed"
    );
}

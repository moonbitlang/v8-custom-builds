unsafe extern "C" {
    fn wee8_probe(bytes: *const u8, len: usize, start_bytes: *const u8, start_len: usize);
}

fn main() {
    if std::env::args().nth(1).as_deref() != Some("--child") {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--child")
            .output()
            .unwrap();
        assert!(output.status.success(), "probe failed: {output:?}");
        assert!(
            output.stdout.is_empty(),
            "unexpected native stdout: {output:?}"
        );
        assert!(
            output.stderr.is_empty(),
            "unexpected native stderr: {output:?}"
        );
        println!("Wasm C API calls, callbacks, ownership, traps and memory passed");
        return;
    }

    let bytes = wat::parse_str(
        r#"(module
            (import "host" "sum" (func $sum (param i32 i32) (result i32)))
            (memory 1 2)
            (func (export "run") (param i32) (result i32)
                local.get 0 i32.const 1 call $sum)
            (export "memory" (memory 0))
            (func (export "trap") unreachable))"#,
    )
    .unwrap();
    let start = wat::parse_str("(module (func $start unreachable) (start $start))").unwrap();
    unsafe { wee8_probe(bytes.as_ptr(), bytes.len(), start.as_ptr(), start.len()) };
}

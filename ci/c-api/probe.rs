unsafe extern "C" {
    fn wee8_string_probe(
        engine: *mut std::ffi::c_void,
        store: *mut std::ffi::c_void,
        bytes: *const u8,
        len: usize,
        echo: *const u8,
        echo_len: usize,
    );
    fn wee8_probe(bytes: *const u8, len: usize, start_bytes: *const u8, start_len: usize);
}

fn main() {
    // Capture native output to enforce silent failure reporting. Exception
    // behavior is tested directly under cargo test, without a subprocess.
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
        println!(
            "Packaged C API compatibility, calls, callbacks, ownership, strings, traps and memory passed"
        );
        return;
    }

    // Keep the readable native-boundary fixtures paired with the bytes used
    // by the GN probes, without requiring another tool in the native matrix.
    for (source, binary) in [
        (
            include_str!("../../custom_wee8/tests/runtime-boundaries.wat"),
            include_bytes!("../../custom_wee8/tests/runtime-boundaries.wasm").as_slice(),
        ),
        (
            include_str!("../../custom_wee8/tests/runtime-shared-memory.wat"),
            include_bytes!("../../custom_wee8/tests/runtime-shared-memory.wasm").as_slice(),
        ),
    ] {
        assert_eq!(wat::parse_str(source).unwrap(), binary);
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

#[unsafe(no_mangle)]
unsafe extern "C" fn test_strings(engine: *mut std::ffi::c_void, store: *mut std::ffi::c_void) {
    let bytes = wat::parse_str(include_str!("strings.wat")).unwrap();
    let echo = wat::parse_str(
        r#"(module
        (import "host" "echo" (func $echo (param externref) (result externref)))
        (func (export "echo") (param externref) (result externref) local.get 0 call $echo))"#,
    )
    .unwrap();
    unsafe {
        wee8_string_probe(
            engine,
            store,
            bytes.as_ptr(),
            bytes.len(),
            echo.as_ptr(),
            echo.len(),
        );
    }
}

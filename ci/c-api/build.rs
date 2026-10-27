use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=WEE8_DIR");
    println!("cargo:rerun-if-changed=probe.cc");
    println!("cargo:rerun-if-changed=strings.cc");
    println!("cargo:rerun-if-changed=stack-limit.cc");
    println!("cargo:rerun-if-changed=exception-traces.cc");
    println!("cargo:rerun-if-changed=externref.cc");
    println!("cargo:rerun-if-changed=modules.cc");
    let dir = PathBuf::from(env::var_os("WEE8_DIR").expect("set WEE8_DIR"));
    println!("cargo:rerun-if-changed={}", dir.display());
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .define("LIBWASM_STATIC", None)
        .include(&dir)
        .file("probe.cc")
        .file("strings.cc")
        .file("stack-limit.cc")
        .file("exception-traces.cc")
        .file("externref.cc")
        .file("modules.cc")
        .compile("wee8_probe");
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=wee8");
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        // Match the system libraries required by the pinned rusty_v8 build.
        println!("cargo:rustc-link-lib=winmm");
        println!("cargo:rustc-link-lib=dbghelp");
        let static_crt = env::var("CARGO_CFG_TARGET_FEATURE")
            .unwrap_or_default()
            .split(',')
            .any(|feature| feature == "crt-static");
        println!(
            "cargo:rustc-link-lib={}",
            if static_crt { "libcpmt" } else { "msvcprt" }
        );
    }
}

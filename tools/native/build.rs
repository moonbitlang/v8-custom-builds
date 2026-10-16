// Reuse upstream platform/toolchain setup without building its Rust adapter.
#[allow(dead_code)]
mod upstream {
    include!("../../rusty_v8/build.rs");

    pub fn build_wee8() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../rusty_v8")
            .canonicalize()
            .unwrap();
        env::set_current_dir(&root).unwrap();
        env::set_var("CARGO_MANIFEST_DIR", &root);
        env::set_var("PYTHONDONTWRITEBYTECODE", "1");
        for name in [
            "GN",
            "NINJA",
            "GN_ARGS",
            "CLANG_BASE_PATH",
            "CXXSTDLIB",
            "PYTHON",
            "DISABLE_CLANG",
            "EXTRA_GN_ARGS",
            "PRINT_GN_ARGS",
            "V8_FORCE_DEBUG",
            "SCCACHE",
            "CCACHE",
        ] {
            println!("cargo:rerun-if-env-changed={name}");
        }
        let _lock = acquire_lock();
        build_v8(false, "wee8-build");
    }
}

fn main() {
    upstream::build_wee8();
}

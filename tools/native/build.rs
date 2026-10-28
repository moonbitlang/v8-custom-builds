// Reuse upstream platform/toolchain setup without building its Rust adapter.
#[allow(dead_code)]
mod upstream {
    include!("../../rusty_v8/build.rs");

    pub fn build_wee8() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../rusty_v8")
            .canonicalize()
            .unwrap();
        // GN cannot rebase Windows verbatim prefixes returned by canonicalize.
        let root = root.to_str().unwrap();
        let root = if let Some(unc) = root.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{unc}"))
        } else {
            PathBuf::from(root.strip_prefix(r"\\?\").unwrap_or(root))
        };
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
            "WEE8_BUILD_STAGE",
        ] {
            println!("cargo:rerun-if-env-changed={name}");
        }
        let _lock = acquire_lock();
        let target = env::var("TARGET").unwrap();
        let base = root
            .parent()
            .unwrap()
            .join("target/wee8-base")
            .join(&target);
        env::set_var("EXTRA_GN_ARGS", format!("wee8_base_dir={:?}", base));
        let out = prepare_v8(false);
        let cache = root.join("../tools/base_cache.py");
        println!("cargo:rerun-if-changed={}", cache.display());
        println!(
            "cargo:rerun-if-changed={}",
            root.join("../custom_wee8").display()
        );
        let run_cache = |command| {
            assert!(Command::new(python())
                .arg(&cache)
                .arg(command)
                .arg("--target")
                .arg(&target)
                .arg("--out")
                .arg(&out)
                .arg("--gn")
                .arg(gn())
                .status()
                .unwrap()
                .success());
        };
        match env::var("WEE8_BUILD_STAGE").as_deref() {
            Ok("base") => {
                run_cache("prepare");
                if !base.join("base.json").exists() {
                    build("wee8-base-build", None);
                    run_cache("save");
                }
            }
            Ok("ext") => {
                run_cache("verify");
                build("wee8-ext-build", None);
                run_cache("merge");
            }
            _ => panic!("WEE8_BUILD_STAGE must be base or ext"),
        }
    }
}

fn main() {
    upstream::build_wee8();
}

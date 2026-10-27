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

//! Acquisition and linking of the wee8 native archive. Consumer builds do not
//! invoke the V8 source bootstrap.

#[path = "build/checksums.rs"]
mod checksums;

use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    fs::{self, File},
    io::{self, BufReader},
    path::{Path, PathBuf},
    time::Duration,
};

const RELEASE: &str = "v0.106.0-moonbit.7";
const RELEASE_HEADER_SHA256: &str =
    "5d46b03807b147b6f31a1f6ea3bae6ceb534afb5eba87dc8d5730c4ee4a4b9a3";

const WASM_HEADER_SHA256: &str = "7b6414f525d503a27609f0dfc3a33fa135f5cdf22d8b03f9e20a948ee8e62301";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=WEE8_ARCHIVE");
    println!("cargo:rerun-if-env-changed=WEE8_HEADER");
    println!("cargo:rerun-if-env-changed=WEE8_MIRROR");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let (archive, header) = match (env::var_os("WEE8_ARCHIVE"), env::var_os("WEE8_HEADER")) {
        (Some(archive), Some(header)) => {
            (dunce::canonicalize(archive)?, dunce::canonicalize(header)?)
        }
        (None, None) => release_files(&out, &env::var("TARGET")?)?,
        _ => {
            return Err(
                "set both WEE8_ARCHIVE and WEE8_HEADER, or neither to download the release".into(),
            );
        }
    };
    // One pinned native contract. Header hashes reject old overrides before
    // binding generation; the required symbols enforce the archive contract.
    let upstream = header.with_file_name("wasm.h");
    if sha256(&header)? != RELEASE_HEADER_SHA256 || sha256(&upstream)? != WASM_HEADER_SHA256 {
        return Err(
            "WEE8_HEADER must match the pinned native API, with wasm.h alongside it".into(),
        );
    }
    println!("cargo:rerun-if-changed={}", upstream.display());
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rerun-if-changed={}", header.display());
    let msvc = env::var("CARGO_CFG_TARGET_ENV")? == "msvc";
    // Stage the exact static archive: an adjacent SDK dynamic library must not
    // be selected by another link mode. This backend never uses rusty_v8.
    fs::copy(
        &archive,
        out.join(if msvc { "wee8.lib" } else { "libwee8.a" }),
    )?;
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=wee8");
    match env::var("CARGO_CFG_TARGET_OS")?.as_str() {
        "macos" => println!("cargo:rustc-link-lib=c++"),
        "linux" => println!("cargo:rustc-link-lib=stdc++"),
        "windows" => {
            // Follow the system libraries used by the pinned rusty_v8 recipe.
            println!("cargo:rustc-link-lib=winmm");
            println!("cargo:rustc-link-lib=dbghelp");
            let static_crt = env::var("CARGO_CFG_TARGET_FEATURE")?
                .split(',')
                .any(|feature| feature == "crt-static");
            println!(
                "cargo:rustc-link-lib={}",
                if static_crt { "libcpmt" } else { "msvcprt" }
            );
        }
        _ => {}
    }
    bindgen::Builder::default()
        .rust_edition(bindgen::RustEdition::Edition2024)
        .header(header.to_string_lossy())
        .clang_args(["-xc++", "-std=c++17", "-DLIBWASM_STATIC"])
        .allowlist_type("(wasm|wee8)_.*")
        .allowlist_function("(wasm|wee8)_.*")
        .allowlist_var("WASM_.*")
        .layout_tests(false)
        .generate_comments(false)
        .generate()?
        .write_to_file(out.join("wee8_bindings.rs"))?;
    Ok(())
}

/// Cache within Cargo's private OUT_DIR: Cargo serializes this build script, so
/// concurrent targets/profiles never share partial files. Include the download
/// origin in the cache identity so changing mirrors cannot reuse another origin.
fn release_files(out: &Path, target: &str) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let archive_name = match target {
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu"
        | "x86_64-apple-darwin" | "aarch64-apple-darwin" => format!("libwee8_release_{target}.a.gz"),
        "x86_64-pc-windows-msvc" => format!("wee8_release_{target}.lib.gz"),
        _ => return Err(format!("no wee8 release for {target}; provide WEE8_ARCHIVE and WEE8_HEADER from a matching source build").into()),
    };
    let base = env::var("WEE8_MIRROR").unwrap_or_else(|_| {
        "https://github.com/moonbitlang/v8-custom-builds/releases/download".into()
    });
    let url = format!("{}/{}", base.trim_end_matches('/'), RELEASE);
    let cache = out.join(format!(
        "wee8-{target}-{:x}",
        Sha256::digest(url.as_bytes())
    ));
    fs::create_dir_all(&cache)?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(300))
        .build()?;
    let manifest = cache.join(format!("SHA256SUMS_{target}"));
    if !manifest.exists() {
        download(
            &client,
            &format!("{url}/SHA256SUMS_{target}"),
            &manifest,
            None,
        )?;
    }
    // The pinned release's manifest covers the archive and both public headers.
    // Independent header hashes enforce our known ABI before binding generation.
    let checksums = fs::read_to_string(&manifest)?;
    let header_name = format!("wee8_c_api_{target}.h");
    let upstream_name = format!("wasm_c_api_{target}.h");
    for name in [&archive_name, &header_name, &upstream_name] {
        let checksum = checksums::checksum(&checksums, name)?;
        let path = cache.join(name);
        if !path.exists() || !sha256(&path)?.eq_ignore_ascii_case(checksum) {
            download(&client, &format!("{url}/{name}"), &path, Some(checksum))?;
        }
    }
    if sha256(&cache.join(&header_name))? != RELEASE_HEADER_SHA256
        || sha256(&cache.join(&upstream_name))? != WASM_HEADER_SHA256
    {
        return Err("downloaded wee8 headers have an incompatible ABI".into());
    }
    // wee8.h includes wasm.h by name. Stage the verified pair together under
    // their public include names, independent of platform-specific asset names.
    let header = cache.join("wee8.h");
    fs::copy(cache.join(header_name), &header)?;
    fs::copy(cache.join(upstream_name), cache.join("wasm.h"))?;
    let archive = cache.join(if target.ends_with("-msvc") {
        "wee8.lib"
    } else {
        "libwee8.a"
    });
    // Always regenerate from the verified compressed bytes on a build-script
    // rerun. An interrupted extraction must never leave a usable partial archive.
    let temporary = archive.with_extension("part");
    let mut input = GzDecoder::new(BufReader::new(File::open(cache.join(archive_name))?));
    let mut output = File::create(&temporary)?;
    io::copy(&mut input, &mut output)?;
    drop(output);
    fs::rename(temporary, &archive)?;
    Ok((archive, header))
}

fn download(
    client: &Client,
    url: &str,
    path: &Path,
    checksum: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let temporary = path.with_extension("part");
    let mut response = client.get(url).send()?.error_for_status()?;
    let mut output = File::create(&temporary)?;
    io::copy(&mut response, &mut output)?;
    drop(output);
    if let Some(expected) = checksum
        && !sha256(&temporary)?.eq_ignore_ascii_case(expected)
    {
        return Err(format!(
            "wee8 checksum mismatch for {}",
            path.file_name().unwrap().to_string_lossy()
        )
        .into());
    }
    fs::rename(temporary, path)?;
    Ok(())
}

fn sha256(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

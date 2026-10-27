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

use wee8::{Error, Store};

#[test]
fn invalid_uncalled_bodies_fail_compilation_with_useful_diagnostics() {
    let store = Store::new(()).unwrap();
    // None of these invalid functions is exported or called. Deferring body
    // validation until invocation would incorrectly accept the whole module.
    for (source, diagnostic) in [
        ("(module (func local.get 0 drop))", "unknown local"),
        ("(module (func (result i32) f64.const 0))", "type mismatch"),
        ("(module (func i32.add drop))", "type mismatch"),
        ("(module (func br 1))", "unknown label"),
    ] {
        let bytes = wat::parse_str(source).unwrap();
        let Err(Error::Interface(error)) = store.compile(&bytes) else {
            panic!("accepted invalid function body: {source}");
        };
        if cfg!(wee8_native_modules) {
            assert!(
                error.contains("compilation") && error.contains("@+"),
                "{source}: {error}"
            );
        } else {
            assert!(error.contains(diagnostic), "{source}: {error}");
        }
    }
}

#[test]
fn malformed_section_and_body_framing_is_rejected() {
    let store = Store::new(()).unwrap();
    let bytes = wat::parse_str("(module (func))").unwrap();
    for end in 8..bytes.len() {
        // Ending at a complete prefix section can form a different valid
        // module; compare framing acceptance with the full validator.
        if wasmparser::Validator::new()
            .validate_all(&bytes[..end])
            .is_err()
        {
            assert!(store.compile(&bytes[..end]).is_err(), "prefix {end}");
        }
    }
    let mut missing_end = bytes;
    *missing_end.last_mut().unwrap() = 0x01; // nop instead of the function end
    assert!(store.compile(&missing_end).is_err());
}

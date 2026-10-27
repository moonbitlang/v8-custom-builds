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
    for source in [
        "(module (func local.get 0 drop))",
        "(module (func (result i32) f64.const 0))",
        "(module (func i32.add drop))",
        "(module (func br 1))",
    ] {
        let bytes = wat::parse_str(source).unwrap();
        let Err(Error::Interface(error)) = store.compile(&bytes) else {
            panic!("accepted invalid function body: {source}");
        };
        assert!(
            error.contains("compilation") && error.contains("@+"),
            "{source}: {error}"
        );
    }
}

#[test]
fn malformed_section_and_body_framing_is_rejected() {
    let store = Store::new(()).unwrap();
    let bytes = [
        0, 97, 115, 109, 1, 0, 0, 0, // header
        1, 4, 1, 96, 0, 0, // one () -> () type
        3, 2, 1, 0, // one function of type 0
        10, 4, 1, 2, 0, 11, // body: no locals, end
    ];
    assert!(store.compile(&bytes).is_ok());
    for end in 0..bytes.len() {
        // Only the header and complete type section form valid prefixes. A
        // function section without its matching code section is invalid.
        if end == 8 || end == 14 {
            assert!(store.compile(&bytes[..end]).is_ok(), "prefix {end}");
        } else {
            assert!(store.compile(&bytes[..end]).is_err(), "prefix {end}");
        }
    }
    let mut missing_end = bytes;
    *missing_end.last_mut().unwrap() = 0x01; // nop instead of the function end
    assert!(store.compile(&missing_end).is_err());
}

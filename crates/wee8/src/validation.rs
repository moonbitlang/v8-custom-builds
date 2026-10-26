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

//! Keep boundary type information in Rust, with body validation delegated only
//! to an archive that explicitly guarantees eager rejection during compilation.

use crate::{Error, Result};
#[cfg(wee8_eager_validation)]
use wasmparser::{Parser, ValidPayload};
use wasmparser::{Validator, WasmFeatures, types::Types};

// Match the pinned engine's proposal policy, not wasmparser's newer defaults.
const FEATURES: WasmFeatures = WasmFeatures::WASM2
    .union(WasmFeatures::TAIL_CALL)
    .union(WasmFeatures::EXTENDED_CONST)
    .union(WasmFeatures::RELAXED_SIMD)
    .union(WasmFeatures::MULTI_MEMORY)
    .union(WasmFeatures::THREADS)
    .union(WasmFeatures::GC)
    .union(WasmFeatures::FUNCTION_REFERENCES)
    .union(WasmFeatures::EXCEPTIONS);

fn invalid_module(error: wasmparser::BinaryReaderError) -> Error {
    format!("outside wee8's supported Wasm subset or invalid Wasm: {error}").into()
}

pub(super) fn module_types(bytes: &[u8]) -> Result<Types> {
    #[cfg(wee8_eager_validation)]
    if unsafe { crate::ffi::wee8_module_validation_is_eager() } {
        return metadata(bytes);
    }
    Validator::new_with_features(FEATURES)
        .validate_all(bytes)
        .map_err(invalid_module)
}

#[cfg(wee8_eager_validation)]
fn metadata(bytes: &[u8]) -> Result<Types> {
    let mut validator = Validator::new_with_features(FEATURES);
    let mut parser = Parser::new(0);
    parser.set_features(FEATURES);
    let mut types = None;
    for payload in parser.parse_all(bytes) {
        match validator
            .payload(&payload.map_err(invalid_module)?)
            .map_err(invalid_module)?
        {
            ValidPayload::End(module_types) => types = Some(module_types),
            // Still pass every payload through Validator, including code/body
            // framing and function counts. Only the returned function validator
            // is omitted: native compilation validates its instruction body.
            ValidPayload::Func(..) | ValidPayload::Ok => {}
            ValidPayload::Parser(_) => return Err("component modules are unsupported".into()),
        }
    }
    types.ok_or_else(|| "missing module type information".into())
}

pub(super) fn compilation_error(bytes: &[u8]) -> Error {
    // The C API discards V8's diagnostic. Preserve detailed Wasm errors on the
    // failure path without paying for duplicate validation on successful input.
    match Validator::new_with_features(FEATURES).validate_all(bytes) {
        Err(error) => invalid_module(error),
        Ok(_) => "module validation or compilation failed".into(),
    }
}

#[cfg(all(test, wee8_eager_validation))]
mod tests {
    use super::*;

    #[test]
    fn native_validation_handles_bodies_omitted_by_metadata_processing() {
        let store = crate::Store::new(()).unwrap();
        assert!(unsafe { crate::ffi::wee8_module_validation_is_eager() });
        let invalid = wat::parse_str("(module (func local.get 0 drop))").unwrap();
        assert!(metadata(&invalid).is_ok());
        assert!(store.compile(&invalid).is_err());
    }
}

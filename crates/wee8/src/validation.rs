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

use crate::{CompileOptions, Error, Module, Result, Store, ffi};
use std::ptr::{self, NonNull};
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

// Older archives abort on unsupported metadata conversions and discard native
// diagnostics. Keep their validation fallback until the minimum release changes.
pub(super) fn compile_with_options<'store, T: 'static>(
    store: &'store Store<T>,
    bytes: &[u8],
    options: &CompileOptions,
) -> Result<Module<'store, T>> {
    if options
        .imported_string_constants
        .as_ref()
        .is_some_and(|name| !name.is_ascii())
    {
        return Err("imported string constant namespace must be ASCII in this V8 version".into());
    }
    // Match V8's always-enabled proposals plus the custom release's
    // optional-feature policy. Keep this explicit: wasmparser defaults
    // include proposals this engine does not implement. C API representation
    // and memory-borrow restrictions are checked separately below.
    let types = module_types(bytes)?;
    let types = types.as_ref();
    let binary = ffi::wasm_byte_vec_t {
        size: bytes.len(),
        data: bytes.as_ptr().cast_mut().cast(),
    };
    let constants = options
        .imported_string_constants
        .as_ref()
        .map(|name| ffi::wasm_name_t {
            size: name.len(),
            data: name.as_ptr().cast_mut().cast(),
        });
    let raw_options = ffi::wee8_compile_options_t {
        js_string_builtins: options.js_string_builtins,
        imported_string_constants: constants.as_ref().map_or(ptr::null(), |name| name),
    };
    let raw = NonNull::new(unsafe {
        ffi::wee8_module_new(store.state.inner.raw.as_ptr(), &binary, &raw_options)
    })
    .ok_or_else(|| compilation_error(bytes))?;
    let module = Module {
        raw,
        store,
        options: options.clone(),
    };
    // Compilation can handle internal GC types; metadata introspection
    // cannot. V8's validated classification is authoritative for builtins.
    // Walk the original declarations: core_imports groups duplicate names
    // and therefore cannot supply native declaration indices.
    let mut imports = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::ImportSection(section) = payload? {
            for (index, import) in section.into_imports().enumerate() {
                let import = import?;
                if !unsafe { ffi::wee8_module_import_is_builtin(raw.as_ptr(), index) } {
                    imports.push((
                        format!("import `{}::{}`", import.module, import.name),
                        types
                            .entity_type_from_import(&import)
                            .ok_or("missing validated import type")?,
                    ));
                }
            }
        }
    }
    let exports = types
        .core_exports()
        .unwrap()
        .map(|(name, ty)| (format!("export `{name}`"), ty));
    // The .5 extension preserves externref nullability in function types.
    // Other native external types still require nullable references. Reject
    // unsupported types before C API introspection, which may abort in V8.
    let scalar_or_externref = |ty: wasmparser::ValType| {
        matches!(
            ty,
            wasmparser::ValType::I32
                | wasmparser::ValType::I64
                | wasmparser::ValType::F32
                | wasmparser::ValType::F64
        ) || ty == wasmparser::ValType::EXTERNREF
    };
    let supported = |ty| scalar_or_externref(ty) || ty == wasmparser::ValType::FUNCREF;
    let function_type = |ty| {
        supported(ty)
            || matches!(ty, wasmparser::ValType::Ref(r) if r.heap_type() == wasmparser::HeapType::EXTERN)
    };
    for (name, ty) in imports.into_iter().chain(exports) {
        let unsupported = match ty {
            wasmparser::types::EntityType::Func(id) => {
                let function = types[id].unwrap_func();
                function
                    .params()
                    .iter()
                    .chain(function.results())
                    .copied()
                    .find(|ty| !function_type(*ty))
            }
            wasmparser::types::EntityType::Tag(id) => {
                // Tag payloads must also pass through Value and the native
                // exception encoder/decoder, which do not support funcref.
                types[id]
                    .unwrap_func()
                    .params()
                    .iter()
                    .copied()
                    .find(|ty| !scalar_or_externref(*ty))
            }
            wasmparser::types::EntityType::Memory(memory) if memory.shared => {
                // Caller exposes an exclusive, non-atomic slice. Shared
                // memories may execute internally but cannot cross this API.
                return Err(format!("shared memory is unsupported in C API {name}").into());
            }
            wasmparser::types::EntityType::Global(global) => {
                (!supported(global.content_type)).then_some(global.content_type)
            }
            wasmparser::types::EntityType::Table(table) => {
                let ty = wasmparser::ValType::Ref(table.element_type);
                (!supported(ty)).then_some(ty)
            }
            _ => None,
        };
        if let Some(ty) = unsupported {
            return Err(format!("{ty} is unsupported in C API {name}").into());
        }
    }
    Ok(module)
}

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

//! Signature types are separate from runtime values and the C ABI's value tags.

use crate::{Result, ffi};

/// A value type supported by the Rust host interface.
/// Equality compares exact types; [`Self::matches`] checks subtyping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
    Ref(RefType),
}

/// A reference's nullability and heap type, following Wasm's `(ref null? heap)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefType {
    is_nullable: bool,
    heap_type: HeapType,
}

/// Heap types supported by the Rust host interface. Guest-only reference types
/// are intentionally not exposed as constructible host types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapType {
    Extern,
}

impl RefType {
    /// The nullable `(ref null extern)` type.
    pub const EXTERNREF: Self = Self::new(true, HeapType::Extern);

    pub const fn new(is_nullable: bool, heap_type: HeapType) -> Self {
        Self {
            is_nullable,
            heap_type,
        }
    }

    pub const fn is_nullable(&self) -> bool {
        self.is_nullable
    }

    pub const fn heap_type(&self) -> &HeapType {
        &self.heap_type
    }
}

impl ValType {
    /// The nullable `(ref null extern)` type, also spelled `externref` in WAT.
    pub const EXTERNREF: Self = Self::Ref(RefType::EXTERNREF);

    /// Whether `self` is a subtype of `other`. A non-null reference matches the
    /// corresponding nullable type, but not vice versa. This is value-type
    /// matching, not function import matching: V8 requires exact host signatures.
    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ref(actual), Self::Ref(expected)) => {
                actual.heap_type == expected.heap_type
                    && (!actual.is_nullable || expected.is_nullable)
            }
            _ => self == other,
        }
    }

    pub(crate) fn raw_kind(self) -> u8 {
        (match self {
            Self::I32 => ffi::wasm_valkind_enum_WASM_I32,
            Self::I64 => ffi::wasm_valkind_enum_WASM_I64,
            Self::F32 => ffi::wasm_valkind_enum_WASM_F32,
            Self::F64 => ffi::wasm_valkind_enum_WASM_F64,
            Self::Ref(reference) => match reference.heap_type {
                HeapType::Extern => ffi::wasm_valkind_enum_WASM_EXTERNREF,
            },
        }) as u8
    }

    pub(crate) fn new_type(self) -> *mut ffi::wasm_valtype_t {
        unsafe {
            match self {
                Self::Ref(reference) => match reference.heap_type {
                    HeapType::Extern => ffi::wee8_valtype_new_externref(reference.is_nullable),
                },
                _ => ffi::wasm_valtype_new(self.raw_kind()),
            }
        }
    }

    pub(crate) unsafe fn from_type(ty: *const ffi::wasm_valtype_t) -> Result<Self> {
        unsafe {
            match u32::from(ffi::wasm_valtype_kind(ty)) {
                ffi::wasm_valkind_enum_WASM_I32 => Ok(Self::I32),
                ffi::wasm_valkind_enum_WASM_I64 => Ok(Self::I64),
                ffi::wasm_valkind_enum_WASM_F32 => Ok(Self::F32),
                ffi::wasm_valkind_enum_WASM_F64 => Ok(Self::F64),
                ffi::wasm_valkind_enum_WASM_EXTERNREF => Ok(Self::Ref(RefType::new(
                    ffi::wee8_valtype_is_nullable(ty),
                    HeapType::Extern,
                ))),
                _ => Err("unsupported host value type".into()),
            }
        }
    }
}

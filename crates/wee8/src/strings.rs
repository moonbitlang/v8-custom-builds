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

//! Store-bound native strings. UTF-16 is lossless, including lone surrogates.

use crate::{Error, ExternRef, Result, StoreContext, ffi};
use std::ptr::NonNull;

/// An owned root for a V8 string. Guest builtins operate on the same native
/// representation; host conversion copies only at the FFI boundary. Cloning
/// retains the root without copying the string. No heap view escapes V8.
#[derive(Clone, Debug)]
pub struct StringRef<'store>(ExternRef<'store>);

impl<'store> StringRef<'store> {
    /// Copy valid Rust UTF-8 into a native string owned by this store.
    pub fn from_utf8<T>(context: StoreContext<'store, T>, text: &str) -> Result<Self> {
        let store = &context.0.inner;
        let raw = NonNull::new(unsafe {
            ffi::wee8_string_new_utf8(store.raw.as_ptr(), text.as_ptr().cast(), text.len())
        })
        .ok_or("failed to create native string: allocation or engine length limit")?;
        Ok(Self(ExternRef { raw, store }))
    }

    /// Copy UTF-16 code units exactly, preserving unpaired surrogates.
    pub fn from_utf16<T>(context: StoreContext<'store, T>, units: &[u16]) -> Result<Self> {
        let store = &context.0.inner;
        let raw = NonNull::new(unsafe {
            ffi::wee8_string_new_utf16(store.raw.as_ptr(), units.as_ptr(), units.len())
        })
        .ok_or("failed to create native string: allocation or engine length limit")?;
        Ok(Self(ExternRef { raw, store }))
    }

    /// Transfer the root into an externref for guest arguments or host results.
    pub fn into_extern_ref(self) -> ExternRef<'store> {
        self.0
    }

    /// Number of UTF-16 code units, not Unicode scalar values or UTF-8 bytes.
    pub fn len(&self) -> usize {
        // Construction checks the native type and strings are immutable.
        unsafe { ffi::wee8_string_length(self.0.raw.as_ptr()) as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copy all code units without replacing unpaired surrogates.
    pub fn to_utf16(&self) -> Vec<u16> {
        let mut units = vec![0; self.len()];
        assert!(
            unsafe {
                ffi::wee8_string_copy_utf16(self.0.raw.as_ptr(), units.as_mut_ptr(), units.len())
            },
            "rooted immutable string changed type or length"
        );
        units
    }

    /// Convert to Rust UTF-8, rejecting unpaired surrogates.
    pub fn to_utf8(&self) -> std::result::Result<String, std::string::FromUtf16Error> {
        String::from_utf16(&self.to_utf16())
    }

    /// Convert to Rust UTF-8, replacing unpaired surrogates with U+FFFD.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.to_utf16())
    }
}

impl<'store> TryFrom<ExternRef<'store>> for StringRef<'store> {
    type Error = Error;

    fn try_from(reference: ExternRef<'store>) -> Result<Self> {
        if !reference.is_string() {
            return Err("externref is not a native string".into());
        }
        Ok(Self(reference))
    }
}

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

//! Store-owned linear memory. All memory views share a store-wide borrow guard:
//! aliased imports/exports cannot overlap a view, growth, or guest execution.
use crate::{Result, StoreContext, StoreOpaque, ffi};
use std::ptr::NonNull;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryType {
    pub minimum: u32,
    pub maximum: Option<u32>,
}

/// A copyable store-owned memory identity, suitable for host callback captures.
/// Pages are 64 KiB. Shared and 64-bit memories are not supported at this boundary.
#[derive(Clone, Copy, Debug)]
pub struct Memory {
    store_id: u64,
    index: usize,
}
impl Memory {
    pub fn new<T>(context: StoreContext<'_, T>, ty: MemoryType) -> Result<Self> {
        if ty.minimum > 65536
            || ty
                .maximum
                .is_some_and(|max| max < ty.minimum || max > 65536)
        {
            return Err("invalid wasm32 memory limits".into());
        }
        let store = &context.0.inner;
        let _access = store
            .memory_access
            .try_borrow_mut()
            .map_err(|_| "cannot allocate memory while guest memory is borrowed")?;
        let limits = ffi::wasm_limits_t {
            min: ty.minimum,
            max: ty.maximum.unwrap_or(u32::MAX),
        };
        let raw = unsafe {
            let ty = ffi::wasm_memorytype_new(&limits);
            let raw = ffi::wasm_memory_new(store.raw.as_ptr(), ty);
            ffi::wasm_memorytype_delete(ty);
            NonNull::new(raw).ok_or("failed to allocate memory")?
        };
        let mut memories = store.memories.borrow_mut();
        let index = memories.len();
        memories.push(raw);
        Ok(Self {
            store_id: store.id,
            index,
        })
    }

    pub(crate) fn raw(self, store: &StoreOpaque) -> Result<NonNull<ffi::wasm_memory_t>> {
        if self.store_id != store.id {
            return Err("memory belongs to a different store".into());
        }
        Ok(store.memories.borrow()[self.index])
    }

    /// Borrow the current backing store for one operation. The view cannot
    /// escape or overlap another view, memory growth, or guest execution.
    pub fn with_data<T, R>(
        self,
        context: StoreContext<'_, T>,
        operation: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R> {
        let store = &context.0.inner;
        let raw = self.raw(store)?;
        let _access = store
            .memory_access
            .try_borrow_mut()
            .map_err(|_| "nested guest memory borrow")?;
        let size = unsafe { ffi::wasm_memory_data_size(raw.as_ptr()) };
        let bytes = if size == 0 {
            &mut []
        } else {
            unsafe {
                std::slice::from_raw_parts_mut(ffi::wasm_memory_data(raw.as_ptr()).cast(), size)
            }
        };
        Ok(operation(bytes))
    }

    /// Grow by `pages`, returning the previous size in pages. Failure preserves
    /// the existing memory. No view may be live, including through an export.
    pub fn grow<T>(self, context: StoreContext<'_, T>, pages: u32) -> Result<u32> {
        let store = &context.0.inner;
        let raw = self.raw(store)?;
        let _access = store
            .memory_access
            .try_borrow_mut()
            .map_err(|_| "cannot grow borrowed guest memory")?;
        let previous = unsafe { ffi::wasm_memory_size(raw.as_ptr()) };
        if !unsafe { ffi::wasm_memory_grow(raw.as_ptr(), pages) } {
            return Err("failed to grow memory".into());
        }
        Ok(previous)
    }
}

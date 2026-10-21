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

//! Native roots and host payloads. Every root borrows the store that owns it.

use crate::{Result, StoreContext, StoreOpaque, ffi};
use std::{
    any::Any,
    ffi::c_void,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
};

/// An owned native root tied to one store. Cloning retains the same guest object;
/// dropping a root releases it, while guest references may keep its payload alive.
///
/// ```compile_fail
/// let reference = {
///     let store = wee8::Store::new(()).unwrap();
///     store.extern_ref(String::from("hello")).unwrap()
/// };
/// reference.data::<String>();
/// ```
///
/// Creating through a store context preserves the same lifetime constraint:
///
/// ```compile_fail
/// let reference = {
///     let store = wee8::Store::new(()).unwrap();
///     wee8::ExternRef::new(store.as_context(), String::from("hello")).unwrap()
/// };
/// reference.data::<String>();
/// ```
pub struct ExternRef<'store> {
    pub(crate) raw: NonNull<ffi::wasm_ref_t>,
    // Borrow the non-generic store state to enforce lifetime and store identity,
    // without tying the reference type to the embedder's host-data type T.
    pub(crate) store: &'store StoreOpaque,
}

impl<'store> ExternRef<'store> {
    /// Root a host value using a context from either a store or a host callback.
    /// The root borrows the store, not the temporary context value. V8 owns the
    /// object and invokes its payload finalizer when it is no longer retained.
    pub fn new<T>(context: StoreContext<'store, T>, data: impl Any) -> Result<Self> {
        let store = &context.0.inner;
        let foreign = NonNull::new(unsafe { ffi::wasm_foreign_new(store.raw.as_ptr()) })
            .ok_or("failed to create externref")?;
        let raw = unsafe { NonNull::new_unchecked(ffi::wasm_foreign_as_ref(foreign.as_ptr())) };
        // Only this crate installs host_info. The extra Box makes the Any trait
        // object a sized allocation that can cross the opaque C pointer boundary.
        let data = Box::into_raw(Box::new(Box::new(data) as Box<dyn Any>));
        unsafe {
            ffi::wasm_ref_set_host_info_with_finalizer(raw.as_ptr(), data.cast(), Some(finalize))
        };
        Ok(Self { raw, store })
    }
}

unsafe extern "C" fn finalize(data: *mut c_void) {
    if catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(data.cast::<Box<dyn Any>>()));
    }))
    .is_err()
    {
        std::process::abort();
    }
}

impl ExternRef<'_> {
    /// Whether this reference is a native V8 string, independent of host payloads.
    pub fn is_string(&self) -> bool {
        unsafe { ffi::wee8_ref_is_string(self.raw.as_ptr()) }
    }

    /// Borrow this object's host payload if it has the requested Rust type.
    pub fn data<D: Any>(&self) -> Option<&D> {
        let data = unsafe { ffi::wasm_ref_get_host_info(self.raw.as_ptr()) };
        if data.is_null() {
            return None;
        }
        // The native root keeps the object and its host-info finalizer alive.
        unsafe { (&*data.cast::<Box<dyn Any>>()).downcast_ref() }
    }
}

impl Clone for ExternRef<'_> {
    fn clone(&self) -> Self {
        Self {
            raw: NonNull::new(unsafe { ffi::wasm_ref_copy(self.raw.as_ptr()) })
                .expect("failed to copy externref root"),
            store: self.store,
        }
    }
}

impl PartialEq for ExternRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.store.raw == other.store.raw
            && unsafe { ffi::wasm_ref_same(self.raw.as_ptr(), other.raw.as_ptr()) }
    }
}
impl Eq for ExternRef<'_> {}

impl fmt::Debug for ExternRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternRef").finish_non_exhaustive()
    }
}

impl Drop for ExternRef<'_> {
    fn drop(&mut self) {
        unsafe { ffi::wasm_ref_delete(self.raw.as_ptr()) }
    }
}

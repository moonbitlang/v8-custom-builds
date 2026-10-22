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

//! Store-owned tag identities and native exception roots. Like Wasmtime, a tag
//! is a copyable store handle and a thrown exception travels through a pending
//! store slot. Payload references remain rooted by V8, not by a Rust registry.

use crate::{Error, Kind, RawValues, Result, StoreContext, StoreOpaque, Value, ffi};
use std::ptr::{self, NonNull};

pub(crate) struct Exceptions {
    tags: std::cell::RefCell<Vec<NonNull<ffi::wasm_tag_t>>>,
    pub(crate) pending: std::cell::RefCell<Option<ExceptionRoot>>,
}

impl Exceptions {
    pub(crate) fn new() -> Self {
        Self {
            tags: Default::default(),
            pending: Default::default(),
        }
    }

    // Store<T> calls this before deleting the native store. Callback captures
    // can keep State alive during finalization, so State::drop would be too late.
    pub(crate) fn clear(&self) {
        self.pending.borrow_mut().take();
        for tag in self.tags.borrow_mut().drain(..) {
            unsafe { ffi::wasm_tag_delete(tag.as_ptr()) }
        }
    }

    pub(crate) fn insert(&self, store_id: u64, raw: NonNull<ffi::wasm_tag_t>) -> Tag {
        let mut tags = self.tags.borrow_mut();
        // Export lookup may return another wrapper for an existing identity.
        // Reuse its slot instead of retaining one extra root on every lookup.
        if let Some(index) = tags
            .iter()
            .position(|tag| unsafe { ffi::wasm_tag_same(tag.as_ptr(), raw.as_ptr()) })
        {
            unsafe { ffi::wasm_tag_delete(raw.as_ptr()) };
            return Tag { store_id, index };
        }
        let index = tags.len();
        tags.push(raw);
        Tag { store_id, index }
    }
}

/// A tag identity owned by one store. Copying a handle preserves that identity;
/// creating two tags with identical signatures produces different identities.
/// Handles may be captured in host callbacks. Using one with another store,
/// including after its original store was destroyed, returns an interface error.
#[derive(Clone, Copy, Debug)]
pub struct Tag {
    store_id: u64,
    index: usize,
}

impl Tag {
    /// Create a tag with scalar/externref payload parameters and no results.
    pub fn new<T>(context: StoreContext<'_, T>, params: &[Kind]) -> Result<Self> {
        if params.contains(&Kind::NonNullExternRef) {
            return Err("non-null tag payloads are unsupported".into());
        }
        unsafe {
            let raw: Vec<_> = params
                .iter()
                .map(|kind| ffi::wasm_valtype_new(kind.raw()))
                .collect();
            let mut params = ffi::wasm_valtype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            let mut results = ffi::wasm_valtype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            ffi::wasm_valtype_vec_new(&mut params, raw.len(), raw.as_ptr());
            ffi::wasm_valtype_vec_new_empty(&mut results);
            let function = ffi::wasm_functype_new(&mut params, &mut results);
            if function.is_null() {
                return Err("failed to create tag signature".into());
            }
            let ty = ffi::wasm_tagtype_new(function);
            if ty.is_null() {
                return Err("failed to create tag type".into());
            }
            let tag = ffi::wasm_tag_new(context.0.inner.raw.as_ptr(), ty);
            ffi::wasm_tagtype_delete(ty);
            let tag = NonNull::new(tag).ok_or("failed to create tag")?;
            Ok(context.0.inner.exceptions.insert(context.0.inner.id, tag))
        }
    }

    pub(crate) fn raw(self, store: &StoreOpaque) -> Result<NonNull<ffi::wasm_tag_t>> {
        if self.store_id != store.id {
            return Err("tag belongs to a different store".into());
        }
        Ok(store.exceptions.tags.borrow()[self.index])
    }

    /// Compare native tag identities, including handles obtained from exports.
    pub fn same<T>(self, context: StoreContext<'_, T>, other: Self) -> Result<bool> {
        let store = &context.0.inner;
        Ok(unsafe { ffi::wasm_tag_same(self.raw(store)?.as_ptr(), other.raw(store)?.as_ptr()) })
    }
}

pub(crate) struct ExceptionRoot(pub(crate) NonNull<ffi::wasm_trap_t>);
impl Drop for ExceptionRoot {
    fn drop(&mut self) {
        unsafe { ffi::wasm_trap_delete(self.0.as_ptr()) }
    }
}
impl ExceptionRoot {
    pub(crate) fn into_raw(self) -> *mut ffi::wasm_trap_t {
        let raw = self.0.as_ptr();
        std::mem::forget(self);
        raw
    }
}

/// One captured Wasm call site. Indices and byte offsets refer to the frame's
/// module, not native addresses or source lines. This snapshot owns no V8 roots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackFrame {
    pub function_index: u32,
    pub function_offset: usize,
    pub module_offset: usize,
}

/// A native root for a catchable Wasm exception, borrowing its store.
/// Payload references are retained by the native exception object itself.
///
/// ```compile_fail
/// let exception = {
///     let store = wee8::Store::new(()).unwrap();
///     let tag = wee8::Tag::new(store.as_context(), &[]).unwrap();
///     wee8::Exception::new(store.as_context(), tag, &[]).unwrap()
/// };
/// drop(exception);
/// ```
pub struct Exception<'store> {
    pub(crate) root: ExceptionRoot,
    pub(crate) store: &'store StoreOpaque,
}

impl<'store> Exception<'store> {
    pub fn new<T>(
        context: StoreContext<'store, T>,
        tag: Tag,
        values: &[Value<'_>],
    ) -> Result<Self> {
        let store = &context.0.inner;
        let tag = tag.raw(store)?;
        for value in values {
            value.check_store(store)?;
        }
        let mut values = RawValues(values.iter().map(Value::raw).collect());
        let args = ffi::wasm_val_vec_t {
            size: values.0.len(),
            data: values.0.as_mut_ptr(),
        };
        let raw = NonNull::new(unsafe {
            ffi::wee8_exception_new(store.raw.as_ptr(), tag.as_ptr(), &args)
        })
        .ok_or("exception payload does not match tag signature")?;
        Ok(Self {
            root: ExceptionRoot(raw),
            store,
        })
    }

    /// The Wasm stack captured when a native exception was constructed.
    /// Rethrowing preserves those frames. Construction outside guest execution
    /// yields an empty trace; guest-created exceptions may also lack a trace.
    pub fn trace(&self) -> Vec<StackFrame> {
        unsafe { trace(self.root.0.as_ptr()) }
    }

    /// Read a payload only with the matching tag. Returned references own roots.
    pub fn args(&self, tag: Tag) -> Result<Vec<Value<'store>>> {
        let tag = tag.raw(self.store)?;
        unsafe {
            let mut values = ffi::wasm_val_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            if !ffi::wee8_exception_get_args(self.root.0.as_ptr(), tag.as_ptr(), &mut values) {
                return Err("exception tag mismatch or unsupported payload".into());
            }
            let result = (0..values.size)
                .map(|index| Value::from_raw(&*values.data.add(index), self.store))
                .collect();
            ffi::wasm_val_vec_delete(&mut values);
            result
        }
    }

    pub fn is(&self, tag: Tag) -> Result<bool> {
        let tag = tag.raw(self.store)?;
        Ok(unsafe { ffi::wee8_exception_is(self.root.0.as_ptr(), tag.as_ptr()) })
    }
}

impl<T> crate::Caller<'_, T> {
    /// Return this error from the callback to throw a catchable guest exception.
    /// The store retains a native root until the callback trampoline consumes it.
    /// If the callback succeeds, returns another error, or panics, that pending
    /// root is discarded instead. Ordinary host errors stay uncatchable.
    pub fn throw(&self, exception: &Exception<'_>) -> Error {
        if !ptr::eq(exception.store, &self.0.inner) {
            return Error::Interface("exception belongs to a different store".into());
        }
        let raw = NonNull::new(unsafe { ffi::wasm_trap_copy(exception.root.0.as_ptr()) })
            .expect("failed to copy exception root");
        *self.0.inner.exceptions.pending.borrow_mut() = Some(ExceptionRoot(raw));
        Error::ThrownException
    }
}

impl<T> crate::Store<T> {
    /// Take the exception after a call/instantiation returned ThrownException.
    /// Starting another guest execution discards any untaken exception.
    pub fn take_exception(&self) -> Option<Exception<'_>> {
        self.state
            .inner
            .exceptions
            .pending
            .borrow_mut()
            .take()
            .map(|root| Exception {
                root,
                store: &self.state.inner,
            })
    }
}

impl<T> crate::Instance<'_, T> {
    /// Obtain a store-owned handle for an exported tag.
    pub fn tag(&self, name: &str) -> Result<Tag> {
        unsafe {
            let tag = ffi::wasm_extern_as_tag(self.export(name)?);
            if tag.is_null() {
                return Err("export is not a tag".into());
            }
            let raw = NonNull::new(ffi::wasm_tag_copy(tag)).ok_or("failed to copy tag root")?;
            Ok(self
                .store
                .state
                .inner
                .exceptions
                .insert(self.store.state.inner.id, raw))
        }
    }
}

// Snapshot frames before releasing their native trap or exception root.
pub(crate) unsafe fn trace(trap: *mut ffi::wasm_trap_t) -> Vec<StackFrame> {
    unsafe {
        let mut frames = ffi::wasm_frame_vec_t {
            size: 0,
            data: ptr::null_mut(),
        };
        ffi::wasm_trap_trace(trap, &mut frames);
        let result = (0..frames.size)
            .map(|index| {
                let frame = *frames.data.add(index);
                StackFrame {
                    function_index: ffi::wasm_frame_func_index(frame),
                    function_offset: ffi::wasm_frame_func_offset(frame),
                    module_offset: ffi::wasm_frame_module_offset(frame),
                }
            })
            .collect();
        ffi::wasm_frame_vec_delete(&mut frames);
        result
    }
}

impl<T> crate::Caller<'_, T> {
    /// Capture this host call's Wasm stack without throwing. The standard C API
    /// captures frames when it creates a trap; the temporary root is discarded
    /// after copying the frame metadata, without touching pending exceptions.
    pub fn capture_stack(&self) -> crate::Result<Vec<StackFrame>> {
        let message = ffi::wasm_byte_vec_t {
            size: 0,
            data: ptr::null_mut(),
        };
        let trap = NonNull::new(unsafe { ffi::wasm_trap_new(self.0.inner.raw.as_ptr(), &message) })
            .ok_or("failed to capture Wasm stack")?;
        let root = ExceptionRoot(trap);
        Ok(unsafe { trace(root.0.as_ptr()) })
    }
}

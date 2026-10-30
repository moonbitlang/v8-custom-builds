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

//! Cross-thread control delegates to V8. Stores and guest handles remain local.

use crate::{Result, Store, ffi};
use std::{ptr::NonNull, sync::Arc};

#[derive(Debug)]
struct IsolateHandleInner(NonNull<ffi::wee8_isolate_handle_t>);

// The native control block serializes requests with store disposal. It contains
// no Rust host state and never dereferences a disposed isolate. Arc keeps the
// handle itself alive for every operation, including concurrent clones.
unsafe impl Send for IsolateHandleInner {}
unsafe impl Sync for IsolateHandleInner {}

impl Drop for IsolateHandleInner {
    fn drop(&mut self) {
        unsafe { ffi::wee8_isolate_handle_delete(self.0.as_ptr()) }
    }
}

/// Cloneable cross-thread execution control that does not keep its store alive.
///
/// Termination uses V8's interrupt checks, including in pure Wasm loops. It does
/// not preempt a blocking host callback; the embedder must unblock its own I/O.
/// Store operations and all other guest handles remain on their creating thread.
#[derive(Clone, Debug)]
pub struct IsolateHandle(Arc<IsolateHandleInner>);

impl IsolateHandle {
    /// Request termination. Returns false if the store has been destroyed.
    /// An interrupted call or start function returns [`crate::Error::Terminated`].
    /// This is a request, not a guarantee that a racing call has not just finished.
    pub fn terminate_execution(&self) -> bool {
        unsafe { ffi::wee8_isolate_handle_terminate_execution(self.0.0.as_ptr()) }
    }

    /// Cancel V8's termination state/request. Returns false after store destruction.
    /// Normally let the interrupted invocation unwind before reusing the store.
    /// Cancellation racing execution may allow that invocation to continue.
    pub fn cancel_terminate_execution(&self) -> bool {
        unsafe { ffi::wee8_isolate_handle_cancel_terminate_execution(self.0.0.as_ptr()) }
    }
}

impl<T> Store<T> {
    /// Obtain control which can be moved to another thread without moving the store.
    pub fn isolate_handle(&self) -> Result<IsolateHandle> {
        let raw =
            NonNull::new(unsafe { ffi::wee8_store_isolate_handle(self.state.inner.raw.as_ptr()) })
                .ok_or("failed to create isolate handle")?;
        Ok(IsolateHandle(Arc::new(IsolateHandleInner(raw))))
    }
}

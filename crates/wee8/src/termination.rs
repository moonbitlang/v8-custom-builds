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

struct InterruptRoot(NonNull<ffi::wee8_interrupt_t>);

// The native control block serializes requests with store disposal. It contains
// no Rust host state and never dereferences a disposed isolate. Arc keeps the
// handle itself alive for every operation, including concurrent clones.
unsafe impl Send for InterruptRoot {}
unsafe impl Sync for InterruptRoot {}

impl Drop for InterruptRoot {
    fn drop(&mut self) {
        unsafe { ffi::wee8_interrupt_delete(self.0.as_ptr()) }
    }
}

/// Cloneable cross-thread execution control that does not keep its store alive.
///
/// Termination uses V8's interrupt checks, including in pure Wasm loops. It does
/// not preempt a blocking host callback; the embedder must unblock its own I/O.
/// Store operations and all other guest handles remain on their creating thread.
#[derive(Clone)]
pub struct InterruptHandle(Arc<InterruptRoot>);

impl InterruptHandle {
    /// Request termination. Returns false if the store has been destroyed.
    /// An interrupted call or start function returns [`crate::Error::Terminated`].
    /// This is a request, not a guarantee that a racing call has not just finished.
    pub fn terminate_execution(&self) -> bool {
        unsafe { ffi::wee8_interrupt_terminate(self.0.0.as_ptr()) }
    }

    /// Cancel V8's termination state/request. Returns false after store destruction.
    /// Normally let the interrupted invocation unwind before reusing the store.
    /// Cancellation racing execution may allow that invocation to continue.
    pub fn cancel_terminate_execution(&self) -> bool {
        unsafe { ffi::wee8_interrupt_cancel(self.0.0.as_ptr()) }
    }

    /// Whether V8 currently has an active termination exception. This is not a
    /// request acknowledgement or history; it is false after the store is gone.
    pub fn is_execution_terminating(&self) -> bool {
        unsafe { ffi::wee8_interrupt_is_terminating(self.0.0.as_ptr()) }
    }
}

impl<T> Store<T> {
    /// Obtain control which can be moved to another thread without moving the store.
    pub fn interrupt_handle(&self) -> Result<InterruptHandle> {
        let raw = NonNull::new(unsafe {
            ffi::wee8_store_interrupt_handle(self.state.inner.raw.as_ptr())
        })
        .ok_or("failed to create interrupt handle")?;
        Ok(InterruptHandle(Arc::new(InterruptRoot(raw))))
    }
}

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

//! Experimental Wasm interface over the wee8 archive. All C ownership rules
//! live here. Stores and their borrowed handles stay on their creating thread;
//! Rust host state is retained through callback finalization. No JS handles cross
//! this interface. Native strings are exposed as store-bound Wasm externrefs.
//!
//! Host services such as WASI belong to the embedder. This crate only compiles,
//! instantiates, and calls Wasm with numeric values and store-bound host references.
//!
//! ```
//! use wee8::{Store, ValType, Value};
//!
//! let store = Store::new(41)?;
//! let module = store.compile(&wat::parse_str(r#"(module
//!     (import "host" "answer" (func $answer (result i32)))
//!     (func (export "run") (result i32) call $answer))"#)?)?;
//! let answer = store.host_func(&[], &[ValType::I32], |caller, _| {
//!     Ok(vec![Value::I32(caller.with_data(|value| *value + 1))])
//! })?;
//! let mut instance = module.instantiate(&[&answer])?;
//! assert_eq!(instance.call("run", &[])?, [Value::I32(42)]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Handles cannot outlive their store:
//!
//! ```compile_fail
//! let module = {
//!     let store = wee8::Store::new(()).unwrap();
//!     store.compile(b"\0asm\x01\0\0\0").unwrap()
//! };
//! module.instantiate(&[]).unwrap();
//! ```

mod calls;
pub use calls::{Func, TypedFunc, WasmParams, WasmResult};
mod memory;
pub use memory::{Memory, MemoryType};
mod exceptions;
pub use exceptions::{Exception, StackFrame, Tag};
mod ffi;
mod references;
pub use references::ExternRef;
mod strings;
mod types;
use std::{
    cell::{Cell, OnceCell, RefCell},
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::{self, NonNull},
    rc::Rc,
    sync::OnceLock,
};
pub use strings::StringRef;
pub use types::{HeapType, RefType, ValType};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Interface(String),
    #[error("{0}")]
    Host(Trap),
    #[error("{0}")]
    Trap(Trap),
    /// The store holds a catchable Wasm exception. Retrieve it with take_exception.
    #[error("uncaught Wasm exception")]
    ThrownException,
}

/// An uncaught native trap with an owned snapshot of its Wasm call sites.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Trap {
    pub message: String,
    pub frames: Vec<StackFrame>,
}

impl From<String> for Trap {
    fn from(message: String) -> Self {
        Self {
            message,
            frames: Vec::new(),
        }
    }
}

impl From<&str> for Trap {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Interface(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Interface(message.into())
    }
}

impl From<wasmparser::BinaryReaderError> for Error {
    fn from(error: wasmparser::BinaryReaderError) -> Self {
        Self::Interface(error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value<'store> {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    ExternRef(Option<ExternRef<'store>>),
}

impl<'store> Value<'store> {
    /// Whether this value satisfies a signature type, including nullability.
    /// This does not check store identity; calls check that separately.
    pub fn matches_ty(&self, ty: &ValType) -> bool {
        match (self, ty) {
            (Self::I32(_), ValType::I32)
            | (Self::I64(_), ValType::I64)
            | (Self::F32(_), ValType::F32)
            | (Self::F64(_), ValType::F64) => true,
            (Self::ExternRef(value), ValType::Ref(reference)) => match reference.heap_type() {
                HeapType::Extern => reference.is_nullable() || value.is_some(),
            },
            _ => false,
        }
    }

    fn check_store(&self, store: &StoreOpaque) -> Result<()> {
        if let Self::ExternRef(Some(reference)) = self
            && reference.store.raw != store.raw
        {
            return Err("externref belongs to a different store".into());
        }
        Ok(())
    }

    // The returned C value owns a new root, consumed by the caller or RawValues.
    fn raw(&self) -> ffi::wasm_val_t {
        use ffi::wasm_val_t__bindgen_ty_1 as Payload;
        let (kind, of) = match self {
            Self::I32(value) => (ffi::wasm_valkind_enum_WASM_I32, Payload { i32_: *value }),
            Self::I64(value) => (ffi::wasm_valkind_enum_WASM_I64, Payload { i64_: *value }),
            Self::F32(value) => (ffi::wasm_valkind_enum_WASM_F32, Payload { f32_: *value }),
            Self::F64(value) => (ffi::wasm_valkind_enum_WASM_F64, Payload { f64_: *value }),
            Self::ExternRef(reference) => (
                ffi::wasm_valkind_enum_WASM_EXTERNREF,
                Payload {
                    ref_: reference
                        .as_ref()
                        .map_or(ptr::null_mut(), |reference| unsafe {
                            NonNull::new(ffi::wasm_ref_copy(reference.raw.as_ptr()))
                                .expect("failed to copy externref root")
                                .as_ptr()
                        }),
                },
            ),
        };
        ffi::wasm_val_t {
            kind: kind as u8,
            of,
        }
    }

    // C arguments are borrowed; C results are released separately. Both need
    // their own root before becoming an independently owned Rust value.
    unsafe fn from_raw(value: &ffi::wasm_val_t, store: &'store StoreOpaque) -> Result<Self> {
        unsafe {
            match ffi::wasm_valkind_enum::from(value.kind) {
                ffi::wasm_valkind_enum_WASM_I32 => Ok(Self::I32(value.of.i32_)),
                ffi::wasm_valkind_enum_WASM_I64 => Ok(Self::I64(value.of.i64_)),
                ffi::wasm_valkind_enum_WASM_F32 => Ok(Self::F32(value.of.f32_)),
                ffi::wasm_valkind_enum_WASM_F64 => Ok(Self::F64(value.of.f64_)),
                ffi::wasm_valkind_enum_WASM_EXTERNREF => {
                    Ok(Self::ExternRef(NonNull::new(value.of.ref_).map(|raw| {
                        ExternRef {
                            raw: NonNull::new(ffi::wasm_ref_copy(raw.as_ptr()))
                                .expect("failed to copy externref root"),
                            store,
                        }
                    })))
                }
                _ => Err("unsupported host value type".into()),
            }
        }
    }
}

// C value vectors own roots even when a call traps or result conversion fails.
// Callback descriptors themselves are borrowed and never wrapped here.
struct RawValues(Vec<ffi::wasm_val_t>);
impl Drop for RawValues {
    fn drop(&mut self) {
        for value in &mut self.0 {
            unsafe { ffi::wasm_val_delete(value) }
        }
    }
}

/// V8 initializes one process-wide platform. It must outlive every isolate.
/// Store creation does not mutate the engine; each Store owns a separate isolate.
struct ProcessEngine(NonNull<ffi::wasm_engine_t>);
unsafe impl Send for ProcessEngine {}
unsafe impl Sync for ProcessEngine {}

/// Initialize the process-wide engine. `Store::new` also initializes it on demand.
pub fn initialize() -> Result<()> {
    process_engine().map(|_| ())
}

fn process_engine() -> Result<&'static ProcessEngine> {
    static ENGINE: OnceLock<std::result::Result<ProcessEngine, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            NonNull::new(unsafe { ffi::wasm_engine_new() })
                .map(ProcessEngine)
                .ok_or_else(|| "failed to initialize wee8".into())
        })
        .as_ref()
        .map_err(|error| Error::Interface(error.clone()))
    // Intentionally no Drop: V8 cannot be reinitialized after engine deletion.
}

struct State<T> {
    inner: StoreOpaque,
    data: RefCell<T>,
}

// Like Wasmtime's StoreOpaque, this is the part of the store independent of T.
// It holds native identity and execution state, not a second reference store.
// Store<T> deletes the native store while callback-owned Rc<State<T>> values
// still keep this state alive for finalization. Dropping State must not delete
// the native store: native callbacks themselves retain State until teardown.
struct StoreOpaque {
    id: u64,
    raw: NonNull<ffi::wasm_store_t>,
    executing: Cell<bool>,
    memory: Cell<*mut ffi::wasm_memory_t>,
    host_error: RefCell<Option<String>>,
    exceptions: exceptions::Exceptions,
    memories: RefCell<Vec<NonNull<ffi::wasm_memory_t>>>,
    memory_access: RefCell<()>,
}

// Both instantiation (which can run a start function) and exported calls enter
// guest code. Reject re-entry through another instance in the same store too:
// it could otherwise grow a memory while a host callback holds its Rust view.
struct Execution<'a>(&'a StoreOpaque);

impl StoreOpaque {
    fn enter(&self) -> Result<Execution<'_>> {
        if self.executing.get() {
            return Err("wee8 does not support guest re-entry from a host callback yet".into());
        }
        let _memory = self
            .memory_access
            .try_borrow_mut()
            .map_err(|_| "cannot enter Wasm while guest memory is borrowed")?;
        self.executing.set(true);
        self.exceptions.pending.borrow_mut().take();
        Ok(Execution(self))
    }
}

impl Drop for Execution<'_> {
    fn drop(&mut self) {
        self.0.memory.set(ptr::null_mut());
        self.0.executing.set(false);
    }
}

/// Per-module native imports. The default leaves every import to the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompileOptions {
    /// Resolve the validated `wasm:js-string` builtins inside V8.
    pub js_string_builtins: bool,
    /// Resolve immutable string globals from this namespace inside V8.
    /// `None` disables constants; an empty string selects the empty namespace.
    /// This pinned engine supports ASCII namespace names (including NUL).
    pub imported_string_constants: Option<String>,
}

/// Native store and host state. Native handles borrow it and cannot cross threads.
pub struct Store<T> {
    state: Rc<State<T>>,
}

/// Borrowed access to one store, obtained from either [`Store::as_context`] or
/// [`Caller::as_context`]. This follows Wasmtime's store-context separation;
/// native rooting remains owned by V8, with no Rust reference registry.
///
/// A context exposes host data and creates store-bound references. Guest memory
/// access remains on [`Caller`], where the current instance is known. Unlike
/// Wasmtime's mutable context, this wrapper uses checked interior borrows and
/// rejects guest re-entry; the context does not grant exclusive store access.
pub struct StoreContext<'a, T>(&'a State<T>);

impl<T> StoreContext<'_, T> {
    /// Borrow the embedder's host state for the duration of the operation.
    pub fn with_data<R>(&self, operation: impl FnOnce(&T) -> R) -> R {
        operation(&self.0.data.borrow())
    }
}

impl<T> Store<T> {
    /// Create an independent store owning the embedder's host state.
    pub fn new(data: T) -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("exhausted wee8 store identities");
        let raw = NonNull::new(unsafe { ffi::wasm_store_new(process_engine()?.0.as_ptr()) })
            .ok_or("failed to create wee8 store")?;
        Ok(Self {
            state: Rc::new(State {
                inner: StoreOpaque {
                    id,
                    raw,
                    executing: Cell::new(false),
                    memory: Cell::new(ptr::null_mut()),
                    host_error: RefCell::new(None),
                    exceptions: exceptions::Exceptions::new(),
                    memories: RefCell::new(Vec::new()),
                    memory_access: RefCell::new(()),
                },
                data: RefCell::new(data),
            }),
        })
    }

    /// Set this store's native stack boundary, returning its previous address.
    /// This does not allocate or enlarge the operating-system thread stack.
    ///
    /// # Safety
    /// A nonzero `limit` must be an address inside the current thread's live stack,
    /// with enough space reserved for host callbacks and V8's error handling.
    /// Keep the store on that thread. Restore the returned limit before leaving
    /// the stack region whose bounds were used. An invalid boundary can allow
    /// native stack overflow; a byte count is not a stack address.
    pub unsafe fn set_stack_limit(&self, limit: usize) -> Result<usize> {
        if self.state.inner.executing.get() {
            return Err("cannot change stack limit during guest execution".into());
        }
        let previous =
            unsafe { ffi::wee8_store_set_stack_limit(self.state.inner.raw.as_ptr(), limit) };
        if previous == 0 {
            return Err("stack limit must be a nonzero native stack address".into());
        }
        Ok(previous)
    }

    /// Borrow this store through the same interface available to host callbacks.
    pub fn as_context(&self) -> StoreContext<'_, T> {
        StoreContext(&self.state)
    }

    /// Root a host value in this store. The guest may retain the object after
    /// the Rust root is dropped; its payload is finalized when no roots remain.
    pub fn extern_ref(&self, data: impl std::any::Any) -> Result<ExternRef<'_>> {
        ExternRef::new(self.as_context(), data)
    }

    fn take_trap(&self, trap: *mut ffi::wasm_trap_t) -> Error {
        if unsafe { ffi::wee8_trap_is_exception(trap) } {
            *self.state.inner.exceptions.pending.borrow_mut() =
                Some(exceptions::ExceptionRoot(NonNull::new(trap).unwrap()));
            return Error::ThrownException;
        }
        let mut trap = unsafe { take_trap(trap) };
        match self.state.inner.host_error.borrow_mut().take() {
            Some(error) => {
                trap.message = error;
                Error::Host(trap)
            }
            None => Error::Trap(trap),
        }
    }
}

impl<T> Drop for Store<T> {
    fn drop(&mut self) {
        self.state.inner.exceptions.clear();
        for memory in self.state.inner.memories.borrow_mut().drain(..) {
            unsafe { ffi::wasm_memory_delete(memory.as_ptr()) };
        }
        // Native finalizers still have their own Rc to Rust state during teardown.
        unsafe { ffi::wasm_store_delete(self.state.inner.raw.as_ptr()) }
    }
}

/// Valid only for the duration of a synchronous host call. No guest re-entry is
/// exposed yet. A memory borrow cannot escape this closure or cross guest growth.
pub struct Caller<'a, T>(&'a State<T>);

impl<'a, T> Caller<'a, T> {
    /// Borrow the store for this host call. Returned roots can outlive this
    /// temporary Caller value, but cannot escape the callback's store borrow.
    pub fn as_context(&self) -> StoreContext<'a, T> {
        StoreContext(self.0)
    }

    /// Create a host object during this callback, for example as a return value.
    pub fn extern_ref(&self, data: impl std::any::Any) -> Result<ExternRef<'a>> {
        ExternRef::new(self.as_context(), data)
    }

    /// Read the store's host state for this callback.
    pub fn with_data<R>(&self, operation: impl FnOnce(&T) -> R) -> R {
        self.as_context().with_data(operation)
    }

    /// Borrow the current instance's exported `memory` for one host call.
    /// Fails during start sections, nested memory borrows, or without that export.
    pub fn with_memory<R>(&self, operation: impl FnOnce(&T, &mut [u8]) -> R) -> Result<R> {
        let memory = self.0.inner.memory.get();
        if memory.is_null() {
            return Err("wee8 host memory requires an exported `memory` and is unavailable during module start".into());
        }
        let _access = self
            .0
            .inner
            .memory_access
            .try_borrow_mut()
            .map_err(|_| "nested guest memory borrow")?;
        // The mutable state borrow also rejects nested memory views. Boundary
        // validation excludes shared memory, and callbacks cannot re-enter Wasm.
        let data = self
            .0
            .data
            .try_borrow_mut()
            .map_err(|_| "nested guest memory borrow")?;
        let size = unsafe { ffi::wasm_memory_data_size(memory) };
        let bytes = if size == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(ffi::wasm_memory_data(memory).cast(), size) }
        };
        Ok(operation(&data, bytes))
    }
}

type Callback<T> = dyn for<'a> Fn(Caller<'a, T>, &[Value<'a>]) -> Result<Vec<Value<'a>>>;

struct CallbackEnvironment<T> {
    state: Rc<State<T>>,
    results: Vec<ValType>,
    callback: Box<Callback<T>>,
}

unsafe extern "C" fn finalize_callback<T>(env: *mut c_void) {
    // The engine owns the environment until finalization, which may happen when
    // the store is deleted, later than deletion of the host's function handle.
    // Never let user capture destructors unwind through C++.
    if catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(env.cast::<CallbackEnvironment<T>>()));
    }))
    .is_err()
    {
        std::process::abort();
    }
}

unsafe extern "C" fn call_host<T>(
    env: *mut c_void,
    args: *const ffi::wasm_val_vec_t,
    results: *mut ffi::wasm_val_vec_t,
) -> *mut ffi::wasm_trap_t {
    unsafe {
        let env = &*env.cast::<CallbackEnvironment<T>>();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let args = &*args;
            let args = if args.size == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(args.data, args.size)
            };
            let values = args
                .iter()
                .map(|value| Value::from_raw(value, &env.state.inner))
                .collect::<Result<Vec<_>>>()?;
            let values = (env.callback)(Caller(&env.state), &values)?;
            if values.len() != env.results.len()
                || values
                    .iter()
                    .zip(&env.results)
                    .any(|(value, ty)| !value.matches_ty(ty))
            {
                return Err("host callback returned the wrong result signature".into());
            }
            // Both implementations supply result slots. This descriptor is
            // borrowed; do not replace or free it, even for a zero-result call.
            if (*results).size != values.len() {
                return Err("invalid callback result capacity".into());
            }
            for value in &values {
                value.check_store(&env.state.inner)?;
            }
            for (index, value) in values.into_iter().enumerate() {
                (*results).data.add(index).write(value.raw());
            }
            Ok::<(), Error>(())
        }));
        {
            let pending = env.state.inner.exceptions.pending.borrow_mut().take();
            if matches!(&outcome, Ok(Err(Error::ThrownException)))
                && let Some(exception) = pending
            {
                return exception.into_raw();
            }
            // A bare marker without Caller::throw must never swallow failure.
        }
        let mut error = match outcome {
            Ok(Ok(())) => return ptr::null_mut(),
            Ok(Err(error)) => error.to_string(),
            Err(_) => "Rust host callback panicked".into(),
        };
        // Wasm C API trap messages include the trailing NUL in their size.
        *env.state.inner.host_error.borrow_mut() = Some(error.clone());
        error.push('\0');
        let message = ffi::wasm_byte_vec_t {
            size: error.len(),
            data: error.as_ptr().cast_mut().cast(),
        };
        ffi::wasm_trap_new(env.state.inner.raw.as_ptr(), &message)
    }
}

impl<T: 'static> Store<T> {
    /// Validate guest proposals and C API boundary types, then compile in this store.
    pub fn compile<'store>(&'store self, bytes: &[u8]) -> Result<Module<'store, T>> {
        self.compile_with_options(bytes, &CompileOptions::default())
    }

    /// Compile with explicit native imports. Host import metadata excludes only
    /// builtins whose signatures V8 has validated; ordinary boundaries retain
    /// the same checks as [`Store::compile`]. Options are owned by the module.
    pub fn compile_with_options<'store>(
        &'store self,
        bytes: &[u8],
        options: &CompileOptions,
    ) -> Result<Module<'store, T>> {
        if options
            .imported_string_constants
            .as_ref()
            .is_some_and(|name| !name.is_ascii())
        {
            return Err(
                "imported string constant namespace must be ASCII in this V8 version".into(),
            );
        }
        // Match V8's always-enabled proposals plus the custom release's
        // optional-feature policy. Keep this explicit: wasmparser defaults
        // include proposals this engine does not implement. C API representation
        // and memory-borrow restrictions are checked separately below.
        let features = wasmparser::WasmFeatures::WASM2
            | wasmparser::WasmFeatures::TAIL_CALL
            | wasmparser::WasmFeatures::EXTENDED_CONST
            | wasmparser::WasmFeatures::RELAXED_SIMD
            | wasmparser::WasmFeatures::MULTI_MEMORY
            | wasmparser::WasmFeatures::THREADS
            | wasmparser::WasmFeatures::GC
            | wasmparser::WasmFeatures::FUNCTION_REFERENCES
            | wasmparser::WasmFeatures::EXCEPTIONS;
        let types = wasmparser::Validator::new_with_features(features)
            .validate_all(bytes)
            .map_err(|error| {
                format!("outside wee8's supported Wasm subset or invalid Wasm: {error}")
            })?;
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
            ffi::wee8_module_new(self.state.inner.raw.as_ptr(), &binary, &raw_options)
        })
        .ok_or("module validation or compilation failed")?;
        let module = Module {
            raw,
            store: self,
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

    /// Create a numeric or externref host function with at most one result.
    /// Callback state is retained until native finalization, which can outlive
    /// the function handle.
    pub fn host_func<'store>(
        &'store self,
        params: &[ValType],
        results: &[ValType],
        callback: impl for<'a> Fn(Caller<'a, T>, &[Value<'a>]) -> Result<Vec<Value<'a>>> + 'static,
    ) -> Result<HostFunc<'store, T>> {
        if results.len() > 1 {
            return Err("wee8 host functions return at most one value".into());
        }
        unsafe {
            let mut params_vec = ffi::wasm_valtype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            let mut results_vec = ffi::wasm_valtype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            let params_raw: Vec<_> = params.iter().map(|ty| ty.new_type()).collect();
            let results_raw: Vec<_> = results.iter().map(|ty| ty.new_type()).collect();
            ffi::wasm_valtype_vec_new(&mut params_vec, params_raw.len(), params_raw.as_ptr());
            ffi::wasm_valtype_vec_new(&mut results_vec, results_raw.len(), results_raw.as_ptr());
            // The constructors consume elements, then vectors. Do not delete
            // consumed vectors again (V8 need not reset the descriptors to zero).
            let ty = ffi::wasm_functype_new(&mut params_vec, &mut results_vec);
            if ty.is_null() {
                return Err("failed to create host function type".into());
            }
            let env = Box::into_raw(Box::new(CallbackEnvironment {
                state: Rc::clone(&self.state),
                results: results.to_vec(),
                callback: Box::new(callback),
            }));
            let raw = ffi::wasm_func_new_with_env(
                self.state.inner.raw.as_ptr(),
                ty,
                Some(call_host::<T>),
                env.cast(),
                Some(finalize_callback::<T>),
            );
            ffi::wasm_functype_delete(ty);
            let raw = NonNull::new(raw).ok_or("failed to create host function")?;
            Ok(HostFunc { raw, store: self })
        }
    }
}

/// V8's C API shares compiled modules by serialization. Keep those bytes private:
/// deserializing arbitrary input as compiled code would be unsafe. This permits
/// Module reuse across fresh stores; it does not promise a zero-copy code cache.
#[derive(Debug)]
pub struct Compiled {
    bytes: Vec<u8>,
    options: CompileOptions,
}

impl<T> Module<'_, T> {
    /// Retain compiled code for reuse in fresh stores of this process.
    pub fn serialize(&self) -> Compiled {
        unsafe {
            let mut bytes = ffi::wasm_byte_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            ffi::wasm_module_serialize(self.raw.as_ptr(), &mut bytes);
            let result = if bytes.size == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(bytes.data.cast(), bytes.size).to_vec()
            };
            ffi::wasm_byte_vec_delete(&mut bytes);
            Compiled {
                bytes: result,
                options: self.options.clone(),
            }
        }
    }
}

impl<T> Store<T> {
    /// Obtain a module from code previously compiled through this crate.
    pub fn module(&self, compiled: &Compiled) -> Result<Module<'_, T>> {
        let bytes = ffi::wasm_byte_vec_t {
            size: compiled.bytes.len(),
            data: compiled.bytes.as_ptr().cast_mut().cast(),
        };
        let constants = compiled
            .options
            .imported_string_constants
            .as_ref()
            .map(|name| ffi::wasm_name_t {
                size: name.len(),
                data: name.as_ptr().cast_mut().cast(),
            });
        let options = ffi::wee8_compile_options_t {
            js_string_builtins: compiled.options.js_string_builtins,
            imported_string_constants: constants.as_ref().map_or(ptr::null(), |name| name),
        };
        let raw = NonNull::new(unsafe {
            ffi::wee8_module_deserialize(self.state.inner.raw.as_ptr(), &bytes, &options)
        })
        .ok_or("failed to obtain compiled module in new store")?;
        Ok(Module {
            raw,
            store: self,
            options: compiled.options.clone(),
        })
    }
}

/// One function import, in the module's declaration order.
#[derive(Debug)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

/// Import type information, preserving declaration order and native kind.
#[derive(Debug, PartialEq, Eq)]
pub enum ExternType {
    Func {
        params: Vec<ValType>,
        results: Vec<ValType>,
    },
    Tag {
        params: Vec<ValType>,
    },
    Memory(MemoryType),
}

#[derive(Debug)]
pub struct ImportType {
    pub module: String,
    pub name: String,
    pub ty: ExternType,
}

/// Borrowed function or store-owned tag/memory supplied to an instance.
pub enum Extern<'a, T> {
    Func(&'a HostFunc<'a, T>),
    Tag(Tag),
    Memory(Memory),
}

impl<T> Module<'_, T> {
    /// Describe function imports; other kinds require import_types.
    pub fn imports(&self) -> Result<Vec<Import>> {
        self.import_types()?
            .into_iter()
            .map(|import| match import.ty {
                ExternType::Func { params, results } => Ok(Import {
                    module: import.module,
                    name: import.name,
                    params,
                    results,
                }),
                ExternType::Tag { .. } | ExternType::Memory(_) => Err(format!(
                    "non-function import {}::{} requires import_types",
                    import.module, import.name
                )
                .into()),
            })
            .collect()
    }

    /// Describe function, tag and memory imports; reject other unsupported imports.
    pub fn import_types(&self) -> Result<Vec<ImportType>> {
        unsafe {
            let mut imports = ffi::wasm_importtype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            ffi::wasm_module_imports(self.raw.as_ptr(), &mut imports);
            let result = (0..imports.size)
                .map(|index| {
                    let import = *imports.data.add(index);
                    let read_name = |name: *const ffi::wasm_byte_vec_t| {
                        if (*name).size == 0 {
                            String::new()
                        } else {
                            String::from_utf8_lossy(std::slice::from_raw_parts(
                                (*name).data.cast(),
                                (*name).size,
                            ))
                            .into_owned()
                        }
                    };
                    let module = read_name(ffi::wasm_importtype_module(import));
                    let name = read_name(ffi::wasm_importtype_name(import));
                    let ty = ffi::wasm_importtype_type(import);
                    let memory = ffi::wasm_externtype_as_memorytype_const(ty);
                    if !memory.is_null() {
                        let limits = &*ffi::wasm_memorytype_limits(memory);
                        return Ok(ImportType {
                            module,
                            name,
                            ty: ExternType::Memory(MemoryType {
                                minimum: limits.min,
                                maximum: (limits.max != u32::MAX).then_some(limits.max),
                            }),
                        });
                    }
                    let function = ffi::wasm_externtype_as_functype_const(ty);
                    let function = if function.is_null() {
                        let tag = ffi::wasm_externtype_as_tagtype_const(ty);
                        if tag.is_null() {
                            ptr::null()
                        } else {
                            ffi::wasm_tagtype_functype(tag)
                        }
                    } else {
                        function
                    };
                    if function.is_null() {
                        return Err(Error::Interface(format!(
                            "unsupported import {module}::{name}"
                        )));
                    }
                    let read_types = |types: *const ffi::wasm_valtype_vec_t| {
                        (0..(*types).size)
                            .map(|i| ValType::from_type(*(*types).data.add(i)))
                            .collect::<Result<Vec<_>>>()
                    };
                    let params = read_types(ffi::wasm_functype_params(function))?;
                    let results = read_types(ffi::wasm_functype_results(function))?;
                    let kind = if ffi::wasm_externtype_kind(ty)
                        == ffi::wasm_externkind_enum_WASM_EXTERN_TAG as u8
                    {
                        ExternType::Tag { params }
                    } else {
                        ExternType::Func { params, results }
                    };
                    Ok(ImportType {
                        module,
                        name,
                        ty: kind,
                    })
                })
                .collect();
            ffi::wasm_importtype_vec_delete(&mut imports);
            result
        }
    }
}

/// Store-owned host callback handle.
pub struct HostFunc<'store, T> {
    raw: NonNull<ffi::wasm_func_t>,
    store: &'store Store<T>,
}

impl<T> Drop for HostFunc<'_, T> {
    fn drop(&mut self) {
        unsafe { ffi::wasm_func_delete(self.raw.as_ptr()) }
    }
}

/// Compiled module handle borrowed from one store.
pub struct Module<'store, T> {
    raw: NonNull<ffi::wasm_module_t>,
    store: &'store Store<T>,
    options: CompileOptions,
}

impl<T> Drop for Module<'_, T> {
    fn drop(&mut self) {
        unsafe { ffi::wasm_module_delete(self.raw.as_ptr()) }
    }
}

impl<'store, T> Module<'store, T> {
    /// Instantiate with function-only imports in module declaration order.
    pub fn instantiate(&self, imports: &[&HostFunc<'_, T>]) -> Result<Instance<'store, T>> {
        if imports.iter().any(|f| !ptr::eq(f.store, self.store)) {
            return Err("imports must belong to the module's store".into());
        }
        let raw = imports
            .iter()
            .map(|f| unsafe { ffi::wasm_func_as_extern(f.raw.as_ptr()) })
            .collect();
        self.instantiate_raw(raw)
    }

    /// Instantiate with typed imports in declaration order. Every handle is
    /// checked against this module's store before entering the native adapter.
    pub fn instantiate_with_imports(
        &self,
        imports: &[Extern<'_, T>],
    ) -> Result<Instance<'store, T>> {
        let raw = imports
            .iter()
            .map(|import| match import {
                Extern::Func(function) => {
                    if !ptr::eq(function.store, self.store) {
                        return Err("imports must belong to the module's store".into());
                    }
                    Ok(unsafe { ffi::wasm_func_as_extern(function.raw.as_ptr()) })
                }
                Extern::Memory(memory) => Ok(unsafe {
                    ffi::wasm_memory_as_extern(memory.raw(&self.store.state.inner)?.as_ptr())
                }),
                Extern::Tag(tag) => Ok(unsafe {
                    ffi::wasm_tag_as_extern(tag.raw(&self.store.state.inner)?.as_ptr())
                }),
            })
            .collect::<Result<Vec<_>>>()?;
        self.instantiate_raw(raw)
    }

    fn instantiate_raw(
        &self,
        mut raw_imports: Vec<*mut ffi::wasm_extern_t>,
    ) -> Result<Instance<'store, T>> {
        let _execution = self.store.state.inner.enter()?;
        unsafe {
            let mut types = ffi::wasm_importtype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            ffi::wasm_module_imports(self.raw.as_ptr(), &mut types);
            let count = types.size;
            ffi::wasm_importtype_vec_delete(&mut types);
            if count != raw_imports.len() {
                return Err("import count mismatch".into());
            }
            let imports = ffi::wasm_extern_vec_t {
                size: raw_imports.len(),
                data: raw_imports.as_mut_ptr(),
            };
            let mut trap = ptr::null_mut();
            let raw = ffi::wasm_instance_new(
                self.store.state.inner.raw.as_ptr(),
                self.raw.as_ptr(),
                &imports,
                &mut trap,
            );
            if !trap.is_null() {
                return Err(self.store.take_trap(trap));
            }
            let raw = NonNull::new(raw).ok_or("instantiation failed")?;
            let mut exports = ffi::wasm_extern_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            ffi::wasm_instance_exports(raw.as_ptr(), &mut exports);
            let mut types = ffi::wasm_exporttype_vec_t {
                size: 0,
                data: ptr::null_mut(),
            };
            ffi::wasm_module_exports(self.raw.as_ptr(), &mut types);
            let names: Vec<Vec<u8>> = (0..types.size)
                .map(|index| {
                    let name = &*ffi::wasm_exporttype_name(*types.data.add(index));
                    if name.size == 0 {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(name.data.cast(), name.size).to_vec()
                    }
                })
                .collect();
            ffi::wasm_exporttype_vec_delete(&mut types);
            // Export handles own the roots. Cache the memory object, whose data
            // address is queried afresh for each host borrow after memory.grow.
            let memory = names
                .iter()
                .position(|name| name == b"memory")
                .filter(|&index| index < exports.size)
                .map(|index| ffi::wasm_extern_as_memory(*exports.data.add(index)))
                .unwrap_or(ptr::null_mut());
            let functions = (0..names.len()).map(|_| OnceCell::new()).collect();
            Ok(Instance {
                raw,
                exports,
                names,
                memory,
                functions,
                store: self.store,
            })
        }
    }
}

/// Instantiated module and exports borrowed from its store.
pub struct Instance<'store, T> {
    raw: NonNull<ffi::wasm_instance_t>,
    exports: ffi::wasm_extern_vec_t,
    names: Vec<Vec<u8>>,
    memory: *mut ffi::wasm_memory_t,
    functions: Vec<OnceCell<calls::Signature>>,
    store: &'store Store<T>,
}

impl<T> Drop for Instance<'_, T> {
    fn drop(&mut self) {
        unsafe {
            ffi::wasm_extern_vec_delete(&mut self.exports);
            ffi::wasm_instance_delete(self.raw.as_ptr());
        }
    }
}

impl<'store, T> Instance<'store, T> {
    /// Test whether an export exists, regardless of its kind.
    pub fn has_export(&self, name: &str) -> bool {
        self.names.iter().any(|export| export == name.as_bytes())
    }

    fn export(&self, name: &str) -> Result<*mut ffi::wasm_extern_t> {
        let index = self
            .names
            .iter()
            .position(|n| n == name.as_bytes())
            .ok_or_else(|| format!("missing export {name}"))?;
        if index >= self.exports.size {
            return Err("export metadata mismatch".into());
        }
        Ok(unsafe { *self.exports.data.add(index) })
    }

    /// Call an exported function with numeric or externref arguments and at most
    /// one result. Guest traps and host callback failures remain distinct errors.
    pub fn call(&mut self, name: &str, args: &[Value<'_>]) -> Result<Vec<Value<'store>>> {
        self.get_func(name)?.call(args)
    }
}

unsafe fn take_trap(trap: *mut ffi::wasm_trap_t) -> Trap {
    unsafe {
        let mut message = ffi::wasm_byte_vec_t {
            size: 0,
            data: ptr::null_mut(),
        };
        ffi::wasm_trap_message(trap, &mut message);
        let text = if message.size == 0 {
            "Wasm trapped".into()
        } else {
            String::from_utf8_lossy(std::slice::from_raw_parts(
                message.data.cast(),
                message.size,
            ))
            .trim_end_matches('\0')
            .to_owned()
        };
        ffi::wasm_byte_vec_delete(&mut message);
        let frames = exceptions::trace(trap);
        ffi::wasm_trap_delete(trap);
        Trap {
            message: text,
            frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_extensions_preserve_the_single_result_host_boundary() {
        let store = Store::new(()).unwrap();
        let module = store
            .compile(
                &wat::parse_str(
                    r#"(module
                    (memory 1)
                    (func $pair (export "pair") (result i32 i32)
                        i32.const 20 i32.const 22)
                    (func (export "run") (result i32)
                        i32.const 0 i32.const 42 i32.const 1 memory.fill
                        i32.const 1 i32.const 0 i32.const 1 memory.copy
                        i32.const 1 i32.load8_u
                        call $pair i32.add i32.ne if unreachable end
                        call $pair i32.add))"#,
                )
                .unwrap(),
            )
            .unwrap();
        let mut instance = module.instantiate(&[]).unwrap();
        assert!(matches!(
            instance.call("pair", &[]),
            Err(Error::Interface(message)) if message.contains("at most one return value")
        ));
        assert_eq!(instance.call("run", &[]).unwrap(), [Value::I32(42)]);
        assert!(matches!(
            store.host_func(&[], &[ValType::I32, ValType::I32], |_, _| unreachable!()),
            Err(Error::Interface(message)) if message.contains("at most one value")
        ));
    }

    #[test]
    fn retained_callbacks_keep_rust_state_alive_and_contain_panics() {
        struct Data(Rc<Cell<usize>>);
        impl Drop for Data {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        {
            let store = Store::new(Data(Rc::clone(&drops))).unwrap();
            let module = store
                .compile(
                    &wat::parse_str(
                        r#"(module
                (import "host" "check" (func $check (param i32) (result i32)))
                (func (export "run") (param i32) (result i32) local.get 0 call $check))"#,
                    )
                    .unwrap(),
                )
                .unwrap();
            let host = store
                .host_func(&[ValType::I32], &[ValType::I32], |caller, args| {
                    caller.with_data(|data| assert_eq!(data.0.get(), 0));
                    match args {
                        [Value::I32(0)] => panic!("host panic to contain"),
                        [Value::I32(1)] => Ok(vec![Value::I64(1)]),
                        [Value::I32(value)] => Ok(vec![Value::I32(*value + 1)]),
                        _ => unreachable!(),
                    }
                })
                .unwrap();
            let mut instance = module.instantiate(&[&host]).unwrap();
            drop(host);
            drop(module);
            assert_eq!(
                instance.call("run", &[Value::I32(41)]).unwrap(),
                [Value::I32(42)]
            );
            assert!(
                matches!(instance.call("run", &[Value::I32(0)]), Err(Error::Host(message)) if message.message.contains("panicked"))
            );
            assert!(
                matches!(instance.call("run", &[Value::I32(1)]), Err(Error::Host(message)) if message.message.contains("wrong result signature"))
            );
            assert!(instance.call("run", &[Value::I64(41)]).is_err());
            assert_eq!(
                instance.call("run", &[Value::I32(41)]).unwrap(),
                [Value::I32(42)]
            );
            assert_eq!(drops.get(), 0);
        }
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn host_memory_borrow_rejects_nested_views_and_store_reentry() {
        let store = Rc::new(Store::new(()).unwrap());
        let module = store
            .compile(
                &wat::parse_str(
                    r#"(module
            (import "host" "check" (func $check))
            (memory (export "memory") 1)
            (func (export "run") call $check))"#,
                )
                .unwrap(),
            )
            .unwrap();
        let weak_store = Rc::downgrade(&store);
        let nested_bytes = wat::parse_str("(module)").unwrap();
        let host = store.host_func(&[], &[], move |caller, _| {
            caller.with_memory(|_, memory| {
                memory[0] = 42;
                assert!(caller.with_memory(|_, _| ()).is_err());
                let store = weak_store.upgrade().unwrap();
                let module = store.compile(&nested_bytes).unwrap();
                assert!(matches!(module.instantiate(&[]), Err(Error::Interface(message)) if message.contains("re-entry")));
                assert_eq!(memory[0], 42);
            })?;
            Ok(vec![])
        }).unwrap();
        let mut instance = module.instantiate(&[&host]).unwrap();
        instance.call("run", &[]).unwrap();
        // A rejected nested operation must not release the outer call's guard
        // or leave the store permanently active after that outer call returns.
        instance.call("run", &[]).unwrap();
    }
}

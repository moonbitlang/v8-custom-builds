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

//! Function handles borrow the instance's export vector. Signatures are inspected
//! once; cached numeric plans own native roots released with the instance cache.

use crate::{Instance, RawValues, Result, ValType, Value, ffi};
use std::{cell::OnceCell, marker::PhantomData, ptr::NonNull};

pub(super) struct Signature {
    raw: NonNull<ffi::wasm_func_t>,
    params: Vec<ValType>,
    results: Vec<ValType>,
    numeric: OnceCell<Option<NumericFunc>>,
}

struct NumericFunc(NonNull<ffi::wee8_numeric_func_t>);
impl Drop for NumericFunc {
    fn drop(&mut self) {
        unsafe { ffi::wee8_numeric_func_delete(self.0.as_ptr()) }
    }
}

/// A resolved function borrowing its instance, and transitively its store.
/// The instance retains both the function and the memory used by host callbacks.
///
/// ```compile_fail
/// let store = wee8::Store::new(()).unwrap();
/// let module = store.compile(&wat::parse_str(
///     "(module (func (export \"run\")))").unwrap()).unwrap();
/// let function = {
///     let instance = module.instantiate(&[]).unwrap();
///     instance.get_func("run").unwrap()
/// };
/// function.call(&[]).unwrap();
/// ```
pub struct Func<'instance, 'store, T> {
    instance: &'instance Instance<'store, T>,
    signature: &'instance Signature,
}

impl<'store, T> Instance<'store, T> {
    /// Resolve an exported function and cache its supported signature. Repeated
    /// named calls share this cache; retain the handle to also avoid name lookup.
    pub fn get_func(&self, name: &str) -> Result<Func<'_, 'store, T>> {
        let index = self
            .names
            .iter()
            .position(|n| n == name.as_bytes())
            .ok_or_else(|| format!("missing export {name}"))?;
        if index >= self.exports.size {
            return Err("export metadata mismatch".into());
        }
        let cached = &self.functions[index];
        if cached.get().is_none() {
            let store = &self.store.state.inner;
            if store.executing.get() {
                return Err("cannot inspect a function during guest execution".into());
            }
            // Inspection cannot run guest code. Protect native access without
            // clearing the pending exception or changing execution state.
            let _memory = store
                .memory_access
                .try_borrow_mut()
                .map_err(|_| "cannot inspect a function while guest memory is borrowed")?;
            let signature = unsafe {
                let raw = NonNull::new(ffi::wasm_extern_as_func(*self.exports.data.add(index)))
                    .ok_or("export is not a function")?;
                let ty = NonNull::new(ffi::wasm_func_type(raw.as_ptr()))
                    .ok_or("failed to inspect function signature")?;
                let params = &*ffi::wasm_functype_params(ty.as_ptr());
                let results = &*ffi::wasm_functype_results(ty.as_ptr());
                let signature = if results.size > 1 {
                    Err("wee8 calls support at most one return value".into())
                } else {
                    let params = (0..params.size)
                        .map(|i| ValType::from_type(*params.data.add(i)))
                        .collect::<Result<Vec<_>>>();
                    let results = (0..results.size)
                        .map(|i| ValType::from_type(*results.data.add(i)))
                        .collect::<Result<Vec<_>>>();
                    params.and_then(|params| {
                        results.map(|results| Signature {
                            raw,
                            params,
                            results,
                            numeric: OnceCell::new(),
                        })
                    })
                };
                ffi::wasm_functype_delete(ty.as_ptr());
                signature?
            };
            // Stores are thread-bound, and introspection cannot call guest code.
            let _ = cached.set(signature);
        }
        Ok(Func {
            instance: self,
            signature: cached.get().unwrap(),
        })
    }
}

impl<'instance, 'store, T> Func<'instance, 'store, T> {
    pub fn params(&self) -> &[ValType] {
        &self.signature.params
    }

    pub fn results(&self) -> &[ValType] {
        &self.signature.results
    }

    /// Call with dynamic values, checking argument types, nullability and store
    /// identity on every invocation. Results own independent reference roots.
    pub fn call(&self, args: &[Value<'_>]) -> Result<Vec<Value<'store>>> {
        let store = &self.instance.store.state.inner;
        let _execution = store.enter()?;
        for value in args {
            value.check_store(store)?;
        }
        if args.len() != self.params().len()
            || !args
                .iter()
                .zip(self.params())
                .all(|(value, ty)| value.matches_ty(ty))
        {
            return Err("argument signature mismatch".into());
        }
        let mut args = RawValues(args.iter().map(Value::raw).collect());
        let mut results = RawValues(
            self.results()
                .iter()
                .map(|ty| ffi::wasm_val_t {
                    kind: ty.raw_kind(),
                    of: ffi::wasm_val_t__bindgen_ty_1 { i64_: 0 },
                })
                .collect(),
        );
        self.invoke(&mut args.0, &mut results.0)?;
        results
            .0
            .iter()
            .map(|value| unsafe { Value::from_raw(value, store) })
            .collect()
    }

    /// Check a numeric signature once. `i32`, `i64`, `f32` and `f64` map to the
    /// corresponding Wasm types; `()` denotes no arguments or no result.
    /// Arguments also accept tuples of up to eight numeric values.
    ///
    /// ```
    /// let store = wee8::Store::new(())?;
    /// let module = store.compile(&wat::parse_str(
    ///     "(module (func (export \"id\") (param i32) (result i32) local.get 0))")?)?;
    /// let instance = module.instantiate(&[])?;
    /// let id = instance.get_func("id")?.typed::<i32, i32>()?;
    /// assert_eq!(id.call(42)?, 42);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn typed<Params: WasmParams, Returns: WasmResult>(
        &self,
    ) -> Result<TypedFunc<'instance, 'store, T, Params, Returns>> {
        if self.params() != Params::TYPES || self.results() != Returns::TYPES {
            return Err("typed function signature mismatch".into());
        }
        let store = &self.instance.store.state.inner;
        if self.signature.numeric.get().is_none() {
            if store.executing.get() {
                return Err("cannot prepare a function during guest execution".into());
            }
            // Preparation can compile a wrapper and GC, but cannot enter Wasm.
            // Preserve the pending exception just like signature introspection.
            let _memory = store
                .memory_access
                .try_borrow_mut()
                .map_err(|_| "cannot prepare a function while guest memory is borrowed")?;
            let numeric = NonNull::new(unsafe {
                ffi::wee8_func_prepare_numeric(self.signature.raw.as_ptr())
            })
            .map(NumericFunc);
            let _ = self.signature.numeric.set(numeric);
        }
        Ok(TypedFunc {
            function: Func {
                instance: self.instance,
                signature: self.signature,
            },
            signature: PhantomData,
        })
    }

    // Both callers hold the same store execution guard for the entire call and
    // conversion. It clears the current memory and pending execution on errors.
    fn invoke(&self, args: &mut [ffi::wasm_val_t], results: &mut [ffi::wasm_val_t]) -> Result<()> {
        let args = ffi::wasm_val_vec_t {
            size: args.len(),
            data: args.as_mut_ptr(),
        };
        let mut results = ffi::wasm_val_vec_t {
            size: results.len(),
            data: results.as_mut_ptr(),
        };
        self.instance
            .store
            .state
            .inner
            .memory
            .set(self.instance.memory);
        let trap = unsafe { ffi::wasm_func_call(self.signature.raw.as_ptr(), &args, &mut results) };
        if trap.is_null() {
            Ok(())
        } else {
            Err(self.instance.store.take_trap(trap))
        }
    }
}

/// A function whose numeric signature was checked at resolution. Calls keep
/// arguments/results on the stack and retain the ordinary store execution guard.
pub struct TypedFunc<'instance, 'store, T, Params, Returns> {
    function: Func<'instance, 'store, T>,
    signature: PhantomData<fn(Params) -> Returns>,
}

impl<T, Params: WasmParams, Returns: WasmResult> TypedFunc<'_, '_, T, Params, Returns> {
    pub fn call(&self, args: Params) -> Result<Returns> {
        let _execution = self.function.instance.store.state.inner.enter()?;
        if let Some(numeric) = self.function.signature.numeric.get().unwrap() {
            let mut packed = [0u8; 64];
            args.write_packed(&mut packed);
            self.function
                .instance
                .store
                .state
                .inner
                .memory
                .set(self.function.instance.memory);
            let trap = unsafe {
                ffi::wee8_numeric_func_call(numeric.0.as_ptr(), packed.as_mut_ptr().cast())
            };
            if !trap.is_null() {
                return Err(self.function.instance.store.take_trap(trap));
            }
            return Ok(Returns::read_packed(&packed));
        }
        // Direct host exports retain C callback conversion. This is a function
        // kind distinction, not compatibility with older runtime releases.
        let mut args = args.into_raw();
        let mut results = Returns::empty();
        self.function.invoke(args.as_mut(), results.as_mut())?;
        // Only sealed numeric implementations can decode these values; the
        // checked native signature excludes any reference ownership here.
        Ok(unsafe { Returns::from_raw(results) })
    }
}

/// Numeric arguments supported by [`TypedFunc`]. Sealed to keep native layouts
/// and conversion invariants inside this crate. Supports scalars, `()` and tuples
/// containing one to eight numeric scalars.
pub trait WasmParams: private::Params {}
impl<T: private::Params> WasmParams for T {}

/// A numeric scalar or `()`, matching the single-result host boundary.
pub trait WasmResult: private::Return {}
impl<T: private::Return> WasmResult for T {}

mod private {
    use crate::{ValType, ffi};

    pub trait Params {
        const TYPES: &'static [ValType];
        type Raw: AsMut<[ffi::wasm_val_t]>;
        fn into_raw(self) -> Self::Raw;
        fn write_packed(self, buffer: &mut [u8; 64]);
    }
    pub trait Return {
        const TYPES: &'static [ValType];
        type Raw: AsMut<[ffi::wasm_val_t]>;
        fn empty() -> Self::Raw;
        unsafe fn from_raw(raw: Self::Raw) -> Self;
        fn read_packed(buffer: &[u8; 64]) -> Self;
    }
    pub trait Scalar {
        const TYPE: ValType;
        fn into_value(self) -> ffi::wasm_val_t;
        const SIZE: usize;
        fn write(self, buffer: &mut [u8]);
    }

    impl Params for () {
        const TYPES: &'static [ValType] = &[];
        type Raw = [ffi::wasm_val_t; 0];
        fn into_raw(self) -> Self::Raw {
            []
        }
        fn write_packed(self, _: &mut [u8; 64]) {}
    }
    impl Return for () {
        const TYPES: &'static [ValType] = &[];
        type Raw = [ffi::wasm_val_t; 0];
        fn empty() -> Self::Raw {
            []
        }
        unsafe fn from_raw(_: Self::Raw) -> Self {}
        fn read_packed(_: &[u8; 64]) -> Self {}
    }

    macro_rules! scalar {
        ($rust:ty, $kind:ident, $field:ident) => {
            impl Scalar for $rust {
                const TYPE: ValType = ValType::$kind;
                const SIZE: usize = size_of::<Self>();
                fn write(self, buffer: &mut [u8]) {
                    buffer[..Self::SIZE].copy_from_slice(&self.to_ne_bytes());
                }
                fn into_value(self) -> ffi::wasm_val_t {
                    ffi::wasm_val_t {
                        kind: Self::TYPE.raw_kind(),
                        of: ffi::wasm_val_t__bindgen_ty_1 { $field: self },
                    }
                }
            }
            impl Params for $rust {
                const TYPES: &'static [ValType] = &[Self::TYPE];
                type Raw = [ffi::wasm_val_t; 1];
                fn into_raw(self) -> Self::Raw {
                    [self.into_value()]
                }
                fn write_packed(self, buffer: &mut [u8; 64]) {
                    self.write(buffer);
                }
            }
            impl Return for $rust {
                const TYPES: &'static [ValType] = &[Self::TYPE];
                type Raw = [ffi::wasm_val_t; 1];
                fn empty() -> Self::Raw {
                    [(0 as $rust).into_value()]
                }
                unsafe fn from_raw(raw: Self::Raw) -> Self {
                    unsafe { raw[0].of.$field }
                }
                fn read_packed(buffer: &[u8; 64]) -> Self {
                    Self::from_ne_bytes(buffer[..Self::SIZE].try_into().unwrap())
                }
            }
        };
    }
    scalar!(i32, I32, i32_);
    scalar!(i64, I64, i64_);
    scalar!(f32, F32, f32_);
    scalar!(f64, F64, f64_);

    macro_rules! tuple {
        ($length:literal; $($ty:ident:$index:tt),+) => {
            impl<$($ty: Scalar),+> Params for ($($ty,)+) {
                const TYPES: &'static [ValType] = &[$($ty::TYPE),+];
                type Raw = [ffi::wasm_val_t; $length];
                fn into_raw(self) -> Self::Raw { [$(self.$index.into_value()),+] }
                fn write_packed(self, buffer: &mut [u8; 64]) {
                    let mut rest = buffer.as_mut_slice();
                    $(self.$index.write(rest); rest = &mut rest[$ty::SIZE..];)+
                    let _ = rest;
                }
            }
        }
    }
    tuple!(1; A:0);
    tuple!(2; A:0, B:1);
    tuple!(3; A:0, B:1, C:2);
    tuple!(4; A:0, B:1, C:2, D:3);
    tuple!(5; A:0, B:1, C:2, D:3, E:4);
    tuple!(6; A:0, B:1, C:2, D:3, E:4, F:5);
    tuple!(7; A:0, B:1, C:2, D:3, E:4, F:5, G:6);
    tuple!(8; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
}

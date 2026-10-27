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

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! {
    // Only count this test's calling thread, excluding unrelated test/framework
    // work. Native allocations are outside Rust's global allocator.
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get().map(|n| n + 1)));
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get().map(|n| n + 1)));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get().map(|n| n + 1)));
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn typed_numeric_calls_allocate_no_rust_heap_storage() {
    let store = wee8::Store::new(()).unwrap();
    let module = store
        .compile(
            &wat::parse_str(
                r#"(module
      (func (export "id") (param i32) (result i32) local.get 0))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let instance = module.instantiate(&[]).unwrap();
    let id = instance
        .get_func("id")
        .unwrap()
        .typed::<i32, i32>()
        .unwrap();
    assert_eq!(id.call(42).unwrap(), 42);
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let mut checksum = 0;
    for i in 0..1000 {
        checksum += id.call(i).unwrap();
    }
    let allocations = ALLOCATIONS.with(|count| count.replace(None).unwrap());
    assert_eq!(checksum, 999 * 1000 / 2);
    assert_eq!(allocations, 0);
}

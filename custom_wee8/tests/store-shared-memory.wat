(module
  (import "host" "observe" (func $observe))
  (memory 1 2 shared)
  (func (export "run") (result i32)
    call $observe
    i32.const 1 memory.grow i32.const 1 i32.ne
    if unreachable end
    i32.const 65536 i32.const 42 i32.atomic.store
    call $observe
    i32.const 65536 i32.atomic.load))

(module
  (import "host" "arm" (func $arm))
  (memory 1 2 shared)
  (func $check (param i32)
    local.get 0 i32.eqz if unreachable end)
  (func (export "run") (param $iterations i32) (result i32)
    (local $i i32)
    i32.const 0 i32.const 7 i32.atomic.store
    i32.const 0 i32.const 2 i32.atomic.rmw.add i32.const 7 i32.eq call $check
    i32.const 0 i32.const 9 i64.const 0 memory.atomic.wait32
    i32.const 2 i32.eq call $check
    i32.const 8 i64.const 13 i64.atomic.store
    i32.const 8 i64.const 13 i64.const 0 memory.atomic.wait64
    i32.const 2 i32.eq call $check
    i32.const 0 i32.const 1 memory.atomic.notify i32.eqz call $check
    i32.const 1 memory.grow i32.const 1 i32.eq call $check
    i32.const 0 i32.atomic.load i32.const 9 i32.eq call $check
    i32.const 65536 i32.atomic.load i32.eqz call $check
    call $arm
    loop $spin
      local.get $i i32.const 1 i32.add local.tee $i
      local.get $iterations i32.lt_u br_if $spin
    end
    i32.const 42)
  (func (export "fail")
    i32.const 1 i32.atomic.load drop))

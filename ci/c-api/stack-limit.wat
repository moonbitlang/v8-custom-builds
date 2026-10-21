(module
  (import "host" "tick" (func $tick))
  (func $recurse (export "recurse") (param $n i32) (result i32)
    call $tick
    local.get $n
    if (result i32)
      local.get $n i32.const 1 i32.sub call $recurse i32.const 1 i32.add
    else
      i32.const 0
    end))

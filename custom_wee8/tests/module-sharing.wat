(module
  (import "host" "observe" (func))
  (func (export "run") (result i32) call 0 i32.const 42)
  (func (result i32) i32.const 7))

(module
  (import "host" "visit" (func $visit))
  (func (export "run") (result i32)
    call $visit
    i32.const 42))

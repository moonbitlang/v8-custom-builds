(module
  (type $cell (struct (field (mut i32))))
  (type $cells (array (mut (ref null $cell))))
  (import "env" "collect" (func $collect))
  (func (export "run") (result i32)
      (local $keep (ref null $cells))
      i32.const 42 struct.new $cell
      i32.const 1 array.new $cells local.set $keep
      call $collect
      local.get $keep i32.const 0 array.get $cells
      struct.get $cell 0))

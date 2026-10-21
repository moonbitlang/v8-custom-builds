(module
  (import "host" "throw" (func $throw))
  (func $inner call $throw)
  (func $outer call $inner)
  (func (export "raise")
    block $caught (result exnref)
      try_table (catch_all_ref $caught) call $outer end
      unreachable
    end
    throw_ref))

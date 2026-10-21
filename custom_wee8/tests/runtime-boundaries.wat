(module
  (type $answer (func (result i32)))
  (type $cell (struct (field i32)))
  (import "host" "arm" (func $arm))
  (memory 1 2)
  (table 2 6 funcref)
  (tag $problem (param i32))
  (global $last (mut (ref null $cell)) (ref.null $cell))
  (func $fortyone (result i32) i32.const 41)
  (func $fortytwo (result i32) i32.const 42)
  (elem $functions func $fortyone $fortytwo)
  (data $bytes "\2a\00\00\00")
  (func $check (param i32)
    local.get 0 i32.eqz if unreachable end)
  (func (export "run") (param $iterations i32) (result i32)
    (local $i i32)
    ref.null func i32.const 2 table.grow i32.const 2 i32.eq call $check
    i32.const 0 i32.const 0 i32.const 2 table.init $functions
    i32.const 2 i32.const 0 i32.const 2 table.copy
    i32.const 0 ref.func $fortytwo i32.const 1 table.fill
    i32.const 0 call_indirect (type $answer) i32.const 42 i32.eq call $check
    i32.const 2 call_indirect (type $answer) i32.const 41 i32.eq call $check
    i32.const 1 table.get ref.is_null i32.eqz call $check
    i32.const 2 ref.func $fortytwo table.set
    elem.drop $functions
    i32.const 0 i32.const 0 i32.const 4 memory.init $bytes
    i32.const 8 i32.const 0 i32.const 4 memory.copy
    i32.const 8 i32.load i32.const 42 i32.eq call $check
    i32.const 16 i32.const 7 i32.const 4 memory.fill
    i32.const 17 i32.load8_u i32.const 7 i32.eq call $check
    data.drop $bytes
    block $caught (result i32)
      try_table (catch $problem $caught)
        i32.const 42 throw $problem
      end
      unreachable
    end
    i32.const 42 i32.eq call $check
    i32.const 42 struct.new $cell global.set $last
    call $arm
    global.get $last struct.get $cell 0 i32.const 42 i32.eq call $check
    loop $allocate
      local.get $i struct.new $cell global.set $last
      local.get $i i32.const 1 i32.add local.tee $i
      local.get $iterations i32.lt_u br_if $allocate
    end
    global.get $last struct.get $cell 0
    local.get $iterations i32.const 1 i32.sub i32.eq call $check
    i32.const 2 call_indirect (type $answer))
  (func (export "fail")
    i32.const 99 table.get drop))

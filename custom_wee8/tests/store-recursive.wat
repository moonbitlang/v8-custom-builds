(module
  (rec
    (type $node (struct (field (ref null $nodes))))
    (type $nodes (array (mut (ref null $node)))))
  (import "host" "visit" (func $visit))
  (func (export "run") (result i32)
    call $visit
    ref.null $nodes
    struct.new $node
    struct.get $node 0
    ref.is_null
    i32.const 41
    i32.add))

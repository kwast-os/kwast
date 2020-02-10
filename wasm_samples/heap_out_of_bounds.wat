(module
  (memory 1)
  (func $test
    i32.const 0xffffffff
    i32.const 0xdeaddead
    i32.store)
  (export "test" (func $test)))

(module
  (memory 1)
  (func $test (param i32) (result i32)
    i32.const 0
    i32.const 42
    i32.store
    
    i32.const 4
    get_local 0
    i32.store
    
    i32.const 0
    i32.const 4
    i32.load
    i32.load
    i32.add)
  (func $start
    i32.const 0xCAFE
    call $test
    drop
  )
  (start $start))

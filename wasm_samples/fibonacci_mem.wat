(module
  (memory 1)
  (func $store (param i32 i32)
    get_local 0
    get_local 1
    call $recursive_fibonacci
    i32.store
  )
  (func $recursive_fibonacci (param i32) (result i32)
    get_local 0
    i32.const 2
    i32.lt_u
    if (result i32)
      i32.const 1
    else
      get_local 0
      i32.const 1
      i32.sub
      call $recursive_fibonacci
      get_local 0
      i32.const 2
      i32.sub
      call $recursive_fibonacci
      i32.add
    end
  )
  (export "recursive_fibonacci" (func $recursive_fibonacci))
)

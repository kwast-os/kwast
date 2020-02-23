(module
  (import "os" "hello" (func $hello (param i32)))
  (func $first (param i32) (result i32)
    get_local 0
    i32.const 1
    i32.add
  )
  (func $test
    (local $i i32)
    loop $L0
      get_local $i
      call $first
      tee_local $i
      call $hello
      br $L0
    end)
  (start $test))

(module
  (import "os" "hello" (func $hello (param i32)))
  (func $first (result i32)
    i32.const 1234)
  (func $test
    loop $L0
      call $first
      call $hello
      br $L0
    end)
  (start $test))

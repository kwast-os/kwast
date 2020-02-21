(module
  (import "os" "hello" (func $hello (param i32)))
  (func $first (result i32)
    i32.const 1234)
  (func $test
    call $first
    call $hello)
  (start $test))

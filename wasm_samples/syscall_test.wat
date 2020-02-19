(module
  (import "os" "hello" (func $hello (param i32)))
  (func $test
    i32.const 1234
    call $hello)
  (start $test))

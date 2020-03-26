(module
  (import "os" "hello" (func $hello (param i32)))
  (memory 1 3)
  (func $_start
    memory.size
    call $hello
    i32.const 2
    memory.grow
    call $hello
    memory.size
    call $hello
    i32.const 2
    memory.grow
    call $hello
    memory.size
    call $hello

    i32.const 1234
    i32.const 80000
    i32.store

    i32.const 1
    memory.grow
    call $hello
    memory.size
    call $hello

    i32.const 0xDEAD0000
    i32.const 4321
    i32.store
  )
  (export "_start" (func $_start))
)

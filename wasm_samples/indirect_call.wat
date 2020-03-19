(module
    (import "os" "hello" (func $hello (param i32)))
    (type $_type (func (result i32)))
    (table 2 anyfunc)
    (elem (i32.const 0) $test)
    
    (func $test (param i32)
        get_local 0
        call $hello
    )

    (func $main
        (i32.const 1234)
        (call_indirect (param i32) (i32.const 0))
    )
)

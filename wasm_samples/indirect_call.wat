(module
    (type $_type (func (result i32)))
    (table 2 anyfunc)
    (elem (i32.const 0) $test)
    
    (func $test (result i32)
        (i32.const 1234)
    )

    (func $main (result i32)
        (call_indirect (result i32) (i32.const 0))
    )
)

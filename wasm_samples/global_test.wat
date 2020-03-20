(module
    (import "os" "hello" (func $hello (param i32)))
    (global $g (mut i32) (i32.const 1048576))
    
    (func $main
        global.get $g
        i32.const 1
        i32.add
        global.set $g
    )

    (start $main)
)

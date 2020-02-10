(module
  (func $overflow (param i32) (result i32)
    get_local 0
    call $overflow
    get_local 0
    i32.add)
  (export "overflow" (func $overflow)))

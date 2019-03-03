#![allow(unused_macros)]

macro_rules! likely {
    ($e:expr) => {
        unsafe {
            core::intrinsics::likely($e)
        }
    }
}

macro_rules! unlikely {
    ($e:expr) => {
        unsafe {
            core::intrinsics::unlikely($e)
        }
    }
}

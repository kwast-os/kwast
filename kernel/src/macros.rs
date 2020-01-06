#![allow(unused_macros)]

macro_rules! likely {
    ($e:expr) => {
        core::intrinsics::likely($e)
    }
}

macro_rules! unlikely {
    ($e:expr) => {
        core::intrinsics::unlikely($e)
    }
}

/// Compare exchange, acquire ordering for success, relaxed for fail, using hardware lock elision.
#[inline(always)]
pub unsafe fn compare_exchange_acquire_relaxed_hle(
    ptr: *mut bool,
    current: bool,
    new: bool,
) -> Result<bool, bool> {
    let previous: bool;

    llvm_asm!("xacquire; lock cmpxchgb $3, $0" : "=*m" (ptr), "={eax}" (previous) : "{eax}" (current), "r" (new) : "memory" : "volatile");

    if previous == current {
        Ok(previous)
    } else {
        Err(previous)
    }
}

/// Atomic store with release ordering, using hardware lock elision.
#[inline(always)]
pub unsafe fn store_release_hle(ptr: *mut bool, val: bool) {
    llvm_asm!("xrelease; movb $1, $0" :: "*m" (ptr), "I" (val) : "memory" : "volatile");
}

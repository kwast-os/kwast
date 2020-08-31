use core::mem::size_of;

extern "C" {
    pub fn page_clear(dst: *mut u8);
}

const SIZE: usize = size_of::<usize>();

fn is_unaligned(ptr: *const u8) -> bool {
    ptr as usize & (SIZE - 1) > 0
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0usize;

    // First try to align the destination and do byte copies.
    while i < n && is_unaligned(dst.add(i)) {
        *dst.add(i) = *src.add(i);
        i += 1;
    }

    // If we end up with an aligned source now, we can do full block copies.
    if !is_unaligned(src.add(i)) {
        while i + SIZE < n {
            let src = src.add(i) as *mut usize;
            let dst = dst.add(i) as *mut usize;
            *dst = *src;
            i += SIZE;
        }
    }

    // Copy the left over parts.
    while i < n {
        *dst.add(i) = *src.add(i);
        i += 1;
    }

    dst
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dst: *mut u8, src: *const u8, mut n: usize) -> *mut u8 {
    if src as usize + n <= dst as usize {
        return memcpy(dst, src, n);
    }

    if src < dst {
        while n > 0 {
            n -= 1;
            *dst.add(n) = *src.add(n);
        }
    } else {
        let mut i = 0usize;
        while i < n {
            *dst.add(i) = *src.add(i);
            i += 1;
        }
    }

    dst
}

#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut u8, data: i32, n: usize) -> *mut u8 {
    let mut i = 0usize;
    let data = data as u8;

    // First try aligning and do byte writes.
    while i < n && is_unaligned(dst.add(i)) {
        *dst.add(i) = data;
        i += 1;
    }

    // If we end up with an aligned source now, we can do full block copies.
    if !is_unaligned(dst.add(i)) {
        let data = data as u8 as usize;
        let data = data << 8 | data;
        let data = data << 16 | data;
        let data = data << 32 | data;

        while i + SIZE < n {
            let dst = dst.add(i) as *mut usize;
            *dst = data;
            i += SIZE;
        }
    }

    // Copy the left over parts.
    while i < n {
        *dst.add(i) = data;
        i += 1;
    }

    dst
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0usize;
    while i < n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return a as i32 - b as i32;
        }
        i += 1;
    }

    0
}

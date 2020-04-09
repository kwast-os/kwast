use crate::arch::address::VirtAddr;
use crate::arch::paging::PAGE_SIZE;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem::size_of;

/// Test one allocation.
#[cfg(feature = "test-heap-one-alloc")]
pub fn test_main() {
    Box::new(42);
}

/// Test big allocation and freeing.
#[cfg(feature = "test-heap-big-alloc")]
pub fn test_main() {
    let mut vec1: Vec<i8> = Vec::new();
    vec1.reserve(8193);
    let mut vec2: Vec<i8> = Vec::new();
    vec2.reserve(8193);
    assert_ne!(vec1.as_ptr(), vec2.as_ptr());

    let test;
    {
        let mut vec3: Vec<i8> = Vec::new();
        vec3.reserve(8193);
        assert_ne!(vec1.as_ptr(), vec3.as_ptr());
        assert_ne!(vec2.as_ptr(), vec3.as_ptr());
        test = vec3.as_ptr();
    }

    let mut vec4: Vec<i8> = Vec::new();
    vec4.reserve(8193);
    assert_ne!(vec1.as_ptr(), vec4.as_ptr());
    assert_ne!(vec2.as_ptr(), vec4.as_ptr());
    assert_eq!(test, vec4.as_ptr());
}

/// Test heap by inspecting the pointers.
#[cfg(feature = "test-heap-realloc")]
pub fn test_main() {
    // Regular realloc.
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }

    assert_eq!(vec.iter().sum::<i32>(), (n - 1) * n / 2);

    // Big realloc.
    let items = 32768 / size_of::<usize>();
    let mut vec = Vec::<usize>::new();
    vec.reserve_exact(items);
    let mut vec2 = Vec::<usize>::new();
    vec2.reserve_exact(items);
    for i in 0..items {
        vec.push(i);
        vec2.push(i * 2);
    }

    vec.reserve_exact(items * 2);
    for i in 0..items {
        assert_eq!(vec[i], i);
        assert_eq!(vec2[i], i * 2);
    }
}

/// Test heap by inspecting the pointers.
#[cfg(feature = "test-heap-pointers")]
pub fn test_main() {
    let mut a: Vec<i8> = Vec::new();
    a.reserve(12);
    let mut b: Vec<i8> = Vec::new();
    b.reserve(12);
    let mut c: Vec<i8> = Vec::new();
    c.reserve(12);
    let mut d: Vec<i8> = Vec::new();
    d.reserve(126);

    // Test offset inside slab
    assert_eq!(a.as_ptr(), unsafe { b.as_ptr().offset(-32) });
    assert_eq!(b.as_ptr(), unsafe { c.as_ptr().offset(-32) });
    assert_ne!(d.as_ptr(), a.as_ptr());
    assert_ne!(d.as_ptr(), b.as_ptr());
    assert_ne!(d.as_ptr(), c.as_ptr());

    // Test reallocating
    drop(b);
    let mut b: Vec<i8> = Vec::new();
    b.reserve(20);
    assert_eq!(b.as_ptr(), unsafe { c.as_ptr().offset(-32) });

    // Test partial & free: exhaust the 512-byte cache,
    // then start a new slab, then check what the heap picks.
    drop(a);
    drop(c);
    let mut a: Vec<i8> = Vec::new();
    a.reserve(512);
    let mut b: Vec<i8> = Vec::new();
    b.reserve(512);
    let mut c: Vec<i8> = Vec::new();
    c.reserve(512);
    let mut d: Vec<i8> = Vec::new();
    d.reserve(512);
    let mut e: Vec<i8> = Vec::new();
    e.reserve(512);
    let mut f: Vec<i8> = Vec::new();
    f.reserve(512);
    let mut g: Vec<i8> = Vec::new();
    g.reserve(512);
    let mut h: Vec<i8> = Vec::new();
    h.reserve(512);
    assert_ne!(a.as_ptr(), b.as_ptr());
    assert_ne!(b.as_ptr(), c.as_ptr());
    assert_ne!(c.as_ptr(), d.as_ptr());
    assert_ne!(d.as_ptr(), e.as_ptr());
    assert_ne!(e.as_ptr(), f.as_ptr());
    assert_ne!(f.as_ptr(), g.as_ptr());
    assert_ne!(g.as_ptr(), h.as_ptr());
    let mut i: Vec<i8> = Vec::new();
    i.reserve(512);

    fn page_of(x: *const i8) -> usize {
        x as usize & !(PAGE_SIZE - 1)
    }

    assert_eq!(page_of(f.as_ptr()), page_of(g.as_ptr()));
    assert!(page_of(h.as_ptr()) - page_of(a.as_ptr()) >= PAGE_SIZE);

    // Drop h & i so that we have a free slab.
    let i_ptr = i.as_ptr();
    drop(h);
    drop(i);
    // Also drop a & b, so we can see if it prefers the partial or the free.
    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();
    drop(a);
    drop(b);
    let mut x: Vec<i8> = Vec::new();
    x.reserve(512);
    let mut y: Vec<i8> = Vec::new();
    y.reserve(512);
    assert_eq!(x.as_ptr(), b_ptr);
    assert_eq!(y.as_ptr(), a_ptr);
    // The partial is now full, should get from the free slab now.
    let mut z: Vec<i8> = Vec::new();
    z.reserve(512);
    assert_eq!(z.as_ptr(), i_ptr);
}

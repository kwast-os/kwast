use crate::arch::x86_64::{cr4_read, cr4_write, xsetbv};
use alloc::alloc::{alloc, handle_alloc_error};
use alloc::boxed::Box;
use core::alloc::Layout;
use core::ptr::{copy_nonoverlapping, null};
use core::slice;
use raw_cpuid::CpuId;

/// SIMD save routine.
static mut SIMD_SAVE_ROUTINE: unsafe fn(region: *mut u8) -> () = invalid_simd_save_routine;

/// SIMD save region size.
static mut SIMD_SAVE_SIZE: u32 = 0;

/// SIMD save region alignment.
static mut SIMD_SAVE_ALIGN: u32 = 64;

/// SIMD initial state.
static mut SIMD_INIT: *const u8 = null();

/// Sets up SIMD.
pub fn setup_simd() {
    let cpuid = CpuId::new();

    // Set OSFXSR and OSXMMEXCPT bits, at least SSE2 is available
    let mut cr4 = cr4_read();
    cr4 |= (1 << 9) | (1 << 10);

    // Check for XSAVE support etc.
    if let Some(state) = cpuid.get_extended_state_info() {
        // Enable XSAVE
        cr4 |= 1 << 18;

        // XCR0 will have x87 and SSE states for sure
        #[allow(clippy::identity_op)]
        let mut xcr0 = (1 << 0) | (1 << 1);

        if state.xcr0_supports_avx_256() {
            xcr0 |= 1 << 2;
        }

        unsafe {
            cr4_write(cr4);
            xsetbv(0, xcr0);
            SIMD_SAVE_SIZE = cpuid.get_extended_state_info().unwrap().xsave_size();
            SIMD_SAVE_ROUTINE = if state.has_xsaves_xrstors() {
                simd_save_routine_xsaves
            } else if state.has_xsaveopt() {
                simd_save_routine_xsaveopt
            } else {
                simd_save_routine_xsave
            }
        }
    } else {
        unsafe {
            cr4_write(cr4);
            SIMD_SAVE_SIZE = 512;
            SIMD_SAVE_ALIGN = 16;
            SIMD_SAVE_ROUTINE = simd_save_routine_fxsave;
        }
    }

    // Setup initial state
    unsafe {
        let region = alloc_simd_save_region();
        simd_save(region);
        SIMD_INIT = region;
    }
}

/// Save simd region.
#[inline]
pub unsafe fn simd_save(region: *mut u8) {
    SIMD_SAVE_ROUTINE(region)
}

/// Allocate SIMD save region.
pub fn alloc_simd_save_region() -> *mut u8 {
    unsafe {
        let layout =
            Layout::from_size_align(SIMD_SAVE_SIZE as usize, SIMD_SAVE_ALIGN as usize).unwrap();
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        ptr
    }
}

/// Create SIMD save region.
pub fn create_simd_save_region() -> Box<[u8]> {
    let region = alloc_simd_save_region();
    unsafe {
        copy_nonoverlapping(SIMD_INIT, region, SIMD_SAVE_SIZE as usize);
        Box::from_raw(slice::from_raw_parts_mut(region, SIMD_SAVE_SIZE as usize))
    }
}

/// Invalid SIMD save routine.
fn invalid_simd_save_routine(_region: *mut u8) {
    unreachable!("simd routine should be selected");
}

/// SIMD save routine using FXSAVE.
unsafe fn simd_save_routine_fxsave(region: *mut u8) {
    asm!("fxsave ($0)" :: "r" (region) : "memory");
}

/// SIMD save routine using XSAVE.
unsafe fn simd_save_routine_xsave(region: *mut u8) {
    asm!("xsave ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

/// SIMD save routine using XSAVEOPT.
unsafe fn simd_save_routine_xsaveopt(region: *mut u8) {
    asm!("xsaveopt ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

/// SIMD save routine using XSAVES.
unsafe fn simd_save_routine_xsaves(region: *mut u8) {
    asm!("xsaves ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

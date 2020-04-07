use crate::arch::x86_64::{cr4_read, cr4_write, xsetbv};
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::alloc::Layout;
use core::ptr::{copy_nonoverlapping, null};
use raw_cpuid::CpuId;

/// SIMD save routine.
static mut SIMD_SAVE_ROUTINE: unsafe fn(region: *mut u8) -> () = simd_invalid_routine;

/// SIMD restore routine.
static mut SIMD_RESTORE_ROUTINE: unsafe fn(region: *mut u8) -> () = simd_invalid_routine;

/// SIMD save region size.
static mut SIMD_SAVE_SIZE: u32 = 0;

/// SIMD save region alignment.
static mut SIMD_SAVE_ALIGN: u32 = 64;

/// SIMD initial state.
static mut SIMD_INIT: *const u8 = null();

/// SIMD state
#[repr(transparent)]
pub struct SimdState {
    ptr: *mut u8,
}

impl SimdState {
    /// Create SIMD save region.
    pub fn new() -> Self {
        let ptr = alloc_simd_save_region();
        unsafe {
            copy_nonoverlapping(SIMD_INIT, ptr, SIMD_SAVE_SIZE as usize);
            Self { ptr }
        }
    }

    /// Save SIMD region.
    #[inline]
    pub fn save(&self) {
        unsafe { SIMD_SAVE_ROUTINE(self.ptr) }
    }

    /// Restore SIMD region.
    #[inline]
    pub fn restore(&self) {
        unsafe { SIMD_RESTORE_ROUTINE(self.ptr) }
    }
}

impl Drop for SimdState {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr, simd_layout());
        }
    }
}

/// Sets up SIMD.
pub fn setup_simd() {
    let cpuid = CpuId::new();

    // Set OSFXSR and OSXMMEXCPT bits, at least SSE2 is available
    let mut cr4 = cr4_read();
    cr4 |= (1 << 9) | (1 << 10);

    // Check for XSAVE support etc.
    if cpuid.get_feature_info().unwrap().has_xsave() {
        let state = cpuid.get_extended_state_info().unwrap();

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
            assert!(SIMD_SAVE_SIZE > 0);
            if state.has_xsaves_xrstors() {
                SIMD_SAVE_ROUTINE = simd_routine_xsaves;
                SIMD_RESTORE_ROUTINE = simd_routine_xrstors;
                println!("Use xsaves");
            } else if state.has_xsaveopt() {
                SIMD_SAVE_ROUTINE = simd_routine_xsaveopt;
                SIMD_RESTORE_ROUTINE = simd_routine_xrstor;
                println!("Use xsaveopt");
            } else {
                SIMD_SAVE_ROUTINE = simd_routine_xsave;
                SIMD_RESTORE_ROUTINE = simd_routine_xrstor;
                println!("Use xsave");
            };
        }
    } else {
        unsafe {
            cr4_write(cr4);
            SIMD_SAVE_SIZE = 512;
            SIMD_SAVE_ALIGN = 16;
            SIMD_SAVE_ROUTINE = simd_routine_fxsave;
            SIMD_RESTORE_ROUTINE = simd_routine_fxrstor;
            println!("Use fxsave");
        }
    }

    // Setup initial state
    unsafe {
        let region = alloc_simd_save_region();
        SIMD_SAVE_ROUTINE(region);
        SIMD_INIT = region;
    }
}

/// Gets the SIMD layout.
fn simd_layout() -> Layout {
    unsafe { Layout::from_size_align(SIMD_SAVE_SIZE as usize, SIMD_SAVE_ALIGN as usize).unwrap() }
}

/// Allocate SIMD save region.
pub fn alloc_simd_save_region() -> *mut u8 {
    let layout = simd_layout();
    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        ptr
    }
}

/// Invalid SIMD save routine.
fn simd_invalid_routine(_region: *mut u8) {
    unreachable!("simd routine should be selected");
}

/// SIMD save routine using FXSAVE.
unsafe fn simd_routine_fxsave(region: *mut u8) {
    asm!("fxsave ($0)" :: "r" (region) : "memory");
}

/// SIMD save routine using XSAVE.
unsafe fn simd_routine_xsave(region: *mut u8) {
    asm!("xsave ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

/// SIMD save routine using XSAVEOPT.
unsafe fn simd_routine_xsaveopt(region: *mut u8) {
    asm!("xsaveopt ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

/// SIMD save routine using XSAVES.
unsafe fn simd_routine_xsaves(region: *mut u8) {
    asm!("xsaves ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

/// SIMD save routine using FXRSTOR.
unsafe fn simd_routine_fxrstor(region: *mut u8) {
    asm!("fxrstor ($0)" :: "r" (region) : "memory");
}

/// SIMD save routine using XRSTOR.
unsafe fn simd_routine_xrstor(region: *mut u8) {
    asm!("xrstor ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

/// SIMD save routine using XRSTORS.
unsafe fn simd_routine_xrstors(region: *mut u8) {
    asm!("xrstors ($0)" :: "r" (region), "{eax}" (0xFFFF_FFFFu32), "{edx}" (0xFFFF_FFFFu32) : "memory");
}

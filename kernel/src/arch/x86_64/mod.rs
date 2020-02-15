use core::cmp::max;

use crate::arch::address::VirtAddr;
use crate::arch::cpu_data::CpuData;
use crate::arch::x86_64::address::PhysAddr;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::mm::pmm::with_pmm;
use crate::mm::vma_allocator::with_vma_allocator;
use multiboot2::ElfSectionFlags;

#[macro_use]
pub mod macros;
pub mod address;
pub mod atomic;
pub mod interrupts;
pub mod paging;
pub mod port;
pub mod tasking;
pub mod vga_text;

// For tests
pub mod qemu;
pub mod serial;

extern "C" {
    static KERNEL_END_PTR: usize;
}

/// Per-CPU data for the bootstrap processor.
static mut PER_CPU_DATA_BSP: CpuData = CpuData::new();

/// Initializes arch-specific stuff.
#[no_mangle]
pub extern "C" fn entry(mboot_addr: usize) {
    // Not shared between cores, but we must be careful about what data we modify or read.
    unsafe {
        PER_CPU_DATA_BSP.prepare_to_set();
        set_per_cpu_data(&mut PER_CPU_DATA_BSP as *mut _);
    }

    interrupts::init();

    let kernel_end = unsafe { &KERNEL_END_PTR as *const _ as usize };
    let mboot_struct = unsafe { multiboot2::load(mboot_addr) };
    let mboot_end = mboot_struct.end_address();
    let reserved_end = max(kernel_end, mboot_end);

    // Map sections correctly
    {
        let mut mapping = ActiveMapping::get();
        let sections = mboot_struct
            .elf_sections_tag()
            .expect("no elf sections tag");
        for x in sections.sections() {
            if x.flags().is_empty()
                || x.flags() == ElfSectionFlags::WRITABLE | ElfSectionFlags::ALLOCATED
            {
                continue;
            }

            let mut paging_flags: EntryFlags = EntryFlags::PRESENT;

            if x.flags().contains(ElfSectionFlags::WRITABLE) {
                paging_flags |= EntryFlags::WRITABLE;
            }

            if !x.flags().contains(ElfSectionFlags::EXECUTABLE) {
                paging_flags |= EntryFlags::NX;
            }

            //println!("{:#x}-{:#x} {:?}", x.start_address(), x.end_address(), x.flags());

            let start = VirtAddr::new(x.start_address() as usize).align_down();
            mapping
                .change_flags_range(
                    start,
                    (x.end_address() - start.as_u64()) as usize, // No need for page alignment of size
                    paging_flags,
                )
                .unwrap();
        }
    }

    with_pmm(|pmm| pmm.init(&mboot_struct, PhysAddr::new(reserved_end)));

    let reserved_end = VirtAddr::new(reserved_end).align_up();
    crate::kernel_run(reserved_end);
}

/// Inits the VMA regions. May only be called once per VMA allocator.
pub unsafe fn init_vma_regions(start: VirtAddr) {
    with_vma_allocator(|vma| {
        vma.insert_region(start, 0x8000_00000000 - start.as_usize());
        vma.insert_region(
            VirtAddr::new(0xffff8000_00000000),
            0x8000_00000000 - 512 * 1024 * 1024 * 1024,
        );
    });
}

/// Halt instruction. Waits for interrupt.
pub fn halt() {
    unsafe {
        asm!("hlt" :::: "volatile");
    }
}

/// Returns true if the architecture supports hardware lock elision.
#[inline(always)]
pub const fn supports_hle() -> bool {
    // Instructions are backwards compatible.
    true
}

/// Sets the per-CPU data pointer.
fn set_per_cpu_data(ptr: *mut CpuData) {
    unsafe {
        wrmsr(0xC000_0101, ptr as u64);
    }
}

/// Gets the per-CPU data.
#[inline(always)]
pub fn get_per_cpu_data() -> &'static mut CpuData {
    unsafe {
        let value: *mut CpuData;
        asm!("mov %gs:0, $0" : "=r" (value));
        &mut *value
    }
}

/// Write Model Specific Register.
#[inline]
unsafe fn wrmsr(reg: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    asm!("wrmsr" :: "{ecx}" (reg), "{eax}" (lo), "{edx}" (hi) : "memory" : "volatile");
}

/// Irq flags type. Flags register for the x86 architecture.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct IrqState(u64);

/// Saves the IRQ state and stops IRQs.
pub fn irq_save_and_stop() -> IrqState {
    unsafe {
        let state: IrqState;
        asm!("pushf; pop $0; cli" : "=r" (state) : : "memory" : "volatile");
        state
    }
}

/// Restores an old IRQ state.
pub fn irq_restore(state: IrqState) {
    unsafe {
        asm!("push $0; popf" : : "r" (state) : "memory" : "volatile");
    }
}

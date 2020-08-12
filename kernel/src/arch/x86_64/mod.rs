use core::cmp::max;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::cpu_data::CpuData;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags};
use crate::arch::x86_64::simd::setup_simd;
use crate::mm::mapper::MemoryMapper;
use crate::mm::pmm::with_pmm;
use crate::util::boot_module::{BootModule, BootModuleProvider, Range};
use multiboot2::{BootInformation, ElfSectionFlags, ModuleIter};
use raw_cpuid::CpuId;

#[macro_use]
pub mod macros;
pub mod address;
pub mod interrupts;
pub mod paging;
pub mod port;
pub mod simd;
pub mod tasking;

// For tests
pub mod qemu;
pub mod serial;

const ONE_PML4_ENTRY: usize = 512 * 1024 * 1024 * 1024;

// TODO: do we also want to give the 0xffff8000_00000000-(0x8000_00000000 - ONE_PML4_ENTRY) range?
pub const USER_START: usize = ONE_PML4_ENTRY;
pub const USER_LEN: usize = 0x8000_00000000 - ONE_PML4_ENTRY - 0x1000;

extern "C" {
    static KERNEL_END_PTR: usize;
    static STACK_BOTTOM: usize;
    static INTERRUPT_STACK_BOTTOM: usize;
}

/// Per-CPU data for the bootstrap processor.
static mut PER_CPU_DATA_BSP: CpuData = CpuData::new();

struct ArchBootModuleProvider<'a> {
    module_iter: ModuleIter<'a>,
    range: Option<Range>,
}

impl<'a> ArchBootModuleProvider<'a> {
    /// Creates a new module provider for this arch.
    pub fn new(boot_info: &'a BootInformation) -> Self {
        let lowest_module = boot_info
            .module_tags()
            .min_by_key(|module| module.start_address());
        let highest_module = boot_info
            .module_tags()
            .max_by_key(|module| module.end_address());

        let range = match (lowest_module, highest_module) {
            (Some(lowest), Some(highest)) => Some(Range {
                start: VirtAddr::new(lowest.start_address() as usize),
                len: (highest.end_address() - lowest.start_address()) as usize,
            }),
            (_, _) => None,
        };

        Self {
            module_iter: boot_info.module_tags(),
            range,
        }
    }
}

impl BootModuleProvider for ArchBootModuleProvider<'_> {
    fn range(&self) -> Option<Range> {
        self.range
    }
}

impl Iterator for ArchBootModuleProvider<'_> {
    type Item = BootModule;

    fn next(&mut self) -> Option<Self::Item> {
        self.module_iter.next().map(|item| BootModule {
            range: Range {
                start: VirtAddr::new(item.start_address() as usize),
                len: (item.end_address() - item.start_address()) as usize,
            },
        })
    }
}

/// Initializes arch-specific stuff.
#[no_mangle]
pub extern "C" fn entry(mboot_addr: usize) {
    {
        let cpuid = CpuId::new();
        let use_pcid = cpuid.get_feature_info().expect("feature info").has_pcid()
            && cpuid
                .get_extended_feature_info()
                .map_or_else(|| false, |info| info.has_invpcid());
        
        unsafe {
            if use_pcid {
                cr4_write(cr4_read() | (1 << 17));
            }

            // Not shared between cores, but we must be careful about what data we modify or read.
            PER_CPU_DATA_BSP.prepare_to_set(use_pcid);
            set_per_cpu_data(&mut PER_CPU_DATA_BSP as *mut _);
        }
    }

    interrupts::init();

    let kernel_end = unsafe { &KERNEL_END_PTR as *const _ as usize };
    let mboot_struct = unsafe { multiboot2::load(mboot_addr) };
    let mboot_end = mboot_struct.end_address();

    let boot_modules = ArchBootModuleProvider::new(&mboot_struct);

    let reserved_end = {
        let mut reserved_end = max(kernel_end, mboot_end);

        if let Some(range) = boot_modules.range {
            reserved_end = max(reserved_end, range.start.as_usize() + range.len);
        }

        reserved_end
    };

    // Map sections correctly
    {
        // Safety: we are the only running thread right now, so no locking is required.
        let mut mapping = unsafe { ActiveMapping::get_unlocked() };
        let sections = mboot_struct
            .elf_sections_tag()
            .expect("no elf sections tag");
        for x in sections.sections().filter(|x| !x.flags().is_empty()) {
            let mut paging_flags: EntryFlags = EntryFlags::PRESENT | EntryFlags::GLOBAL;

            if x.flags().contains(ElfSectionFlags::WRITABLE) {
                paging_flags |= EntryFlags::WRITABLE;
            }

            if !x.flags().contains(ElfSectionFlags::EXECUTABLE) {
                paging_flags |= EntryFlags::NX;
            }

            println!(
                "{:#x}-{:#x} {:?}",
                x.start_address(),
                x.end_address(),
                x.flags()
            );

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

    // Init PMM
    with_pmm(|pmm| pmm.init(&mboot_struct, PhysAddr::new(reserved_end)));

    // Setup guard pages for stack
    unsafe {
        let stack_bottom = VirtAddr::new(&STACK_BOTTOM as *const _ as usize);
        let interrupt_stack_bottom = VirtAddr::new(&INTERRUPT_STACK_BOTTOM as *const _ as usize);
        let mut mapping = ActiveMapping::get_unlocked();
        mapping.free_and_unmap_single(stack_bottom);
        mapping.free_and_unmap_single(interrupt_stack_bottom);
    }

    // Run kernel main
    let reserved_end = VirtAddr::new(reserved_end).align_up();
    crate::kernel_run(reserved_end, boot_modules);
}

/// Late init.
pub fn late_init() {
    setup_simd();
}

/// Halt instruction. Waits for interrupt.
pub fn halt() {
    unsafe {
        llvm_asm!("hlt" :::: "volatile");
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
pub fn get_per_cpu_data() -> &'static CpuData {
    unsafe {
        let value: *mut CpuData;
        llvm_asm!("mov %gs:0, $0" : "=r" (value));
        &*value
    }
}

/// Write Model Specific Register.
unsafe fn wrmsr(reg: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    llvm_asm!("wrmsr" :: "{ecx}" (reg), "{eax}" (lo), "{edx}" (hi) : "memory" : "volatile");
}

/// Read CR4
fn cr4_read() -> u64 {
    unsafe {
        let value: u64;
        llvm_asm!("mov %cr4, $0" : "=r" (value));
        value
    }
}

/// Write new CR4
unsafe fn cr4_write(value: u64) {
    llvm_asm!("mov $0, %cr4" :: "r" (value) : "memory" : "volatile");
}

/// Write extended control register.
unsafe fn xsetbv(reg: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    llvm_asm!("xsetbv" :: "{ecx}" (reg), "{eax}" (lo), "{edx}" (hi) : "memory" : "volatile");
}

/// Enable preemption.
#[inline(always)]
pub fn preempt_enable() {
    debug_assert_eq!(CpuData::preempt_count_offset(), 8);
    unsafe {
        llvm_asm!("decl %gs:8" ::: "memory" : "volatile");
    }
}

/// Disable preemption.
#[inline(always)]
pub fn preempt_disable() {
    debug_assert_eq!(CpuData::preempt_count_offset(), 8);
    unsafe {
        llvm_asm!("incl %gs:8" ::: "memory" : "volatile");
    }
}

/// Invalid opcode.
pub fn invalid_opcode() {
    unsafe {
        llvm_asm!("ud2" :::: "volatile");
    }
}

#![allow(clippy::identity_op)]

use core::cmp::max;

use crate::arch::acpi;
use crate::arch::acpi::hpet::Hpet;
use crate::arch::acpi::RootSdt;
use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::cpu_data::CpuData;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags};
use crate::arch::x86_64::simd::setup_simd;
use crate::mm::mapper::MemoryMapper;
use crate::mm::pmm::with_pmm;
use crate::util::boot_module::{BootModule, BootModuleProvider, Range};
use crate::util::lfb_text;
use crate::util::lfb_text::LfbParameters;
use multiboot2::{BootInformation, ElfSectionFlags, FramebufferType, ModuleIter};
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
pub const TCB_START: usize = 1 * 1024 * 1024 * 1024; // 1 GiB, which is the next entry on the PML3
pub const TCB_LEN: usize = 1 * 1024 * 1024 * 1024; // 1 GiB, which means a whole PML3 will be used

extern "C" {
    static KERNEL_END_PTR: usize;
    static STACK_BOTTOM: usize;
    static INTERRUPT_STACK_BOTTOM: usize;
}

/// Per-CPU data for the bootstrap processor.
static mut PER_CPU_DATA_BSP: CpuData = CpuData::new();

/// Hpet.
static mut HPET: Option<Hpet> = None;

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
    // Constants that can't be put as const because it's not const fn.
    let hpet_addr: VirtAddr = VirtAddr::new(0x1000);

    // Safety: we are the only running thread right now, so no locking is required.
    let mut mapping = unsafe { ActiveMapping::get_unlocked() };

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

    // Put the reserved end after the kernel and modules.
    let reserved_end = {
        let mut reserved_end = max(kernel_end, mboot_end);

        if let Some(range) = boot_modules.range {
            reserved_end = max(reserved_end, range.start.as_usize() + range.len);
        }

        reserved_end
    };

    // Map sections correctly
    {
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
        mapping.free_and_unmap_single(stack_bottom);
        mapping.free_and_unmap_single(interrupt_stack_bottom);
    }

    let mut reserved_end = VirtAddr::new(reserved_end).align_up();

    // Setup kernel text output.
    if let Some(lfb_mboot_params) = mboot_struct.framebuffer_tag() {
        if matches!(lfb_mboot_params.buffer_type, FramebufferType::RGB { .. }) {
            if let Some(new_reserved_end) = lfb_text::init(
                LfbParameters {
                    address: PhysAddr::new(lfb_mboot_params.address as _),
                    width: lfb_mboot_params.width,
                    height: lfb_mboot_params.height,
                    pitch: lfb_mboot_params.pitch,
                    bpp: lfb_mboot_params.bpp,
                },
                &mut mapping,
                reserved_end,
            ) {
                reserved_end = new_reserved_end;
            }
        }
    }

    // The bootloader verifies the checksum and revision for us.
    let root_sdt = if let Some(rsdp) = mboot_struct.rsdp_v2_tag() {
        RootSdt::Xsdt(PhysAddr::new(rsdp.xsdt_address()))
    } else if let Some(rsdp) = mboot_struct.rsdp_v1_tag() {
        RootSdt::Rsdt(PhysAddr::new(rsdp.rsdt_address()))
    } else {
        panic!("No RSDP table found");
    };

    let result = acpi::parse_tables(root_sdt, reserved_end);
    unsafe {
        if let Some(hpet_data) = result.hpet {
            HPET = Some(Hpet::from(&mut mapping, hpet_addr, hpet_data));
        }
    }

    crate::kernel_run(reserved_end, boot_modules);
}

/// Gets the Hpet reference if there is one.
pub fn hpet() -> Option<&'static Hpet> {
    // Safety: read-only and only written to on bootup.
    unsafe { HPET.as_ref() }
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

/// Sets the per-CPU data pointer.
fn set_per_cpu_data(ptr: *const CpuData) {
    unsafe {
        wrmsr(0xC000_0101, ptr as u64);
    }
}

/// Gets the per-CPU data.
#[inline(always)]
pub fn get_per_cpu_data() -> &'static CpuData {
    unsafe {
        let value: *const CpuData;
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

/// Check if the "should schedule" flag is set and switch if needed.
#[inline]
pub fn check_should_schedule() {
    extern "C" {
        fn _check_should_schedule();
    }

    unsafe {
        _check_should_schedule();
    }
}

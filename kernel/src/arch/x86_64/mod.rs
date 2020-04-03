use core::cmp::max;

use crate::arch::address::VirtAddr;
use crate::arch::cpu_data::CpuData;
use crate::arch::x86_64::address::PhysAddr;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::mm::pmm::with_pmm;
use crate::mm::vma_allocator::with_vma_allocator;
use crate::util::boot_module::{BootModule, BootModuleProvider, Range};
use multiboot2::{BootInformation, ElfSectionFlags, ModuleIter};

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

/*#[repr(C, packed)]
struct TSS {
    reserved0: u32,
    rsp: [VirtAddr; 3],
    reserved1: u64,
    ist: [VirtAddr; 7],
    reserved2: u64,
    reserved3: u16,
    io_map_base: u16,
}

impl TSS {
    const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp: [VirtAddr::null(); 3],
            reserved1: 0,
            ist: [VirtAddr::null(); 7],
            reserved2: 0,
            reserved3: 0,
            io_map_base: 0,
        }
    }

    fn set_ist(&mut self, n: usize, addr: VirtAddr) {
        self.ist[n] = addr;
    }
}*/

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
    unsafe {
        // Not shared between cores, but we must be careful about what data we modify or read.
        PER_CPU_DATA_BSP.prepare_to_set();
        set_per_cpu_data(&mut PER_CPU_DATA_BSP as *mut _);
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
        let mut mapping = ActiveMapping::get();
        let sections = mboot_struct
            .elf_sections_tag()
            .expect("no elf sections tag");
        for x in sections.sections().filter(|x| {
            !x.flags().is_empty()
                && x.flags() != ElfSectionFlags::WRITABLE | ElfSectionFlags::ALLOCATED
        }) {
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
    crate::kernel_run(reserved_end, boot_modules);
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
pub fn get_per_cpu_data() -> &'static CpuData {
    unsafe {
        let value: *mut CpuData;
        asm!("mov %gs:0, $0" : "=r" (value));
        &*value
    }
}

/// Write Model Specific Register.
#[inline]
unsafe fn wrmsr(reg: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    asm!("wrmsr" :: "{ecx}" (reg), "{eax}" (lo), "{edx}" (hi) : "memory" : "volatile");
}

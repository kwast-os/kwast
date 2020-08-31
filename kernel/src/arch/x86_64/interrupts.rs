use core::mem::size_of;

use bitflags::bitflags;
use lazy_static::lazy_static;

use crate::arch::x86_64::address::VirtAddr;
use crate::arch::x86_64::paging::PageFaultError;
use crate::arch::x86_64::port::write_port8;
use crate::tasking::scheduler;

/// The stack frame pushed by the CPU for an ISR.
#[derive(Debug)]
#[repr(C)]
struct ISRStackFrame {
    /// Points to the instruction that will be executed when the handler returns.
    rip: VirtAddr,
    /// Code segment, high-order 48-bits zeros
    cs: u64,
    /// RFlags
    rflags: u64,
    /// Stack pointer at time of interrupt
    rsp: VirtAddr,
    /// Stack segment at time of interrupt
    ss: u64,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct Entry {
    offset_1: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_2: u16,
    offset_3: u32,
    zero: u32,
}

bitflags! {
    /// Flags for an entry.
    #[repr(transparent)]
    struct EntryFlags: u8 {
        /// Specifies whether this entry is present.
        const PRESENT = 1 << 7;
        /// Interrupt gate.
        const INT_GATE = 0b1110;
        /// Trap gate: same as interrupt gate, but doesn't automatically disable/re-enable interrupts.
        const TRAP_GATE = 0b1111;
    }
}

#[repr(C, packed)]
struct IDTDescriptor {
    limit: u16,
    base: u64,
}

const ENTRY_COUNT: usize = 64;

struct IDT([Entry; ENTRY_COUNT]);

/// Irq flags type. Flags register for the x86 architecture.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct IrqState(u64);

impl Entry {
    fn new(handler: usize, flags: EntryFlags, ist: u8) -> Self {
        Self {
            offset_1: handler as u16,
            selector: 0x08,
            ist,
            type_attr: flags.bits(),
            offset_2: (handler >> 16) as u16,
            offset_3: (handler >> 32) as u32,
            zero: 0,
        }
    }

    fn empty() -> Self {
        Self::new(0, EntryFlags::empty(), 0)
    }
}

impl IDT {
    fn new() -> Self {
        Self([Entry::empty(); ENTRY_COUNT])
    }

    fn lidt(&self) {
        let desc = IDTDescriptor {
            limit: (size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };

        unsafe {
            llvm_asm!("lidt ($0)" :: "r" (&desc));
        }
    }

    fn set_handler(&mut self, n: usize, handler: usize, flags: EntryFlags, ist: u8) {
        self.0[n] = Entry::new(handler, flags, ist);
    }
}

lazy_static! {
    static ref IDT_INSTANCE: IDT = {
        let exc_flags = EntryFlags::PRESENT | EntryFlags::INT_GATE;

        let mut idt = IDT::new();
        idt.set_handler(0, exc_divide_by_zero as usize, exc_flags, 0);
        idt.set_handler(1, exc_debug as usize, exc_flags, 0);
        idt.set_handler(2, exc_nmi as usize, exc_flags, 1);
        idt.set_handler(3, exc_breakpoint as usize, exc_flags, 0);
        idt.set_handler(4, exc_overflow as usize, exc_flags, 0);
        idt.set_handler(5, exc_bound_range_exceeded as usize, exc_flags, 0);
        idt.set_handler(6, exc_invalid_opcode as usize, exc_flags, 0);
        idt.set_handler(7, exc_device_not_available as usize, exc_flags, 0);
        idt.set_handler(8, exc_double_fault as usize, exc_flags, 1);
        idt.set_handler(9, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(10, exc_invalid_tss as usize, exc_flags, 0);
        idt.set_handler(11, exc_segment_not_present as usize, exc_flags, 0);
        idt.set_handler(12, exc_stack_segment as usize, exc_flags, 0);
        idt.set_handler(13, exc_gpf as usize, exc_flags, 1);
        idt.set_handler(14, exc_pf as usize, exc_flags, 1);
        idt.set_handler(15, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(16, exc_fp as usize, exc_flags, 0);
        idt.set_handler(17, exc_alignment_check as usize, exc_flags, 0);
        idt.set_handler(18, exc_machine_check as usize, exc_flags, 0);
        idt.set_handler(19, exc_simd_fp as usize, exc_flags, 0);
        idt.set_handler(20, exc_virtualization as usize, exc_flags, 0);
        idt.set_handler(21, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(22, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(23, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(24, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(25, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(26, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(27, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(28, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(29, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(30, exc_unknown as usize, exc_flags, 0);
        idt.set_handler(31, exc_unknown as usize, exc_flags, 0);

        extern "C" {
            fn irq0();
        }

        // Timer
        idt.set_handler(32, irq0 as usize, exc_flags, 0);

        for i in 1..16 {
            idt.set_handler(32 + i, irq as usize, exc_flags, 0);
        }

        idt
    };
}

pub fn init() {
    IDT_INSTANCE.lidt();

    // Remap PIC
    unsafe {
        write_port8(0x20, 0x11);
        write_port8(0xA0, 0x11);
        write_port8(0x21, 0x20);
        write_port8(0xA1, 0x28);
        write_port8(0x21, 0x04);
        write_port8(0xA1, 0x02);
        write_port8(0x21, 0x01);
        write_port8(0xA1, 0x01);
        write_port8(0x21, 0x00);
        write_port8(0xA1, 0x00);
    }
}

pub fn setup_timer() {
    // TODO: replace this with the APIC timer
    unsafe {
        // Write to command port: channel 0, access mode lo&hi, mode 3, binary
        write_port8(0x43, 0b0011_0110);
        let hz = 100;
        let divisor: i32 = 1_193_182 / hz;
        write_port8(0x40, (divisor & 0xFF) as u8);
        write_port8(0x40, (divisor >> 8) as u8);
    }
}

pub fn enable() {
    unsafe {
        llvm_asm!("sti" :::: "volatile");
    }
}

pub fn disable() {
    unsafe {
        llvm_asm!("cli" :::: "volatile");
    }
}

/// Saves the IRQ state and stops IRQs.
pub fn irq_save_and_stop() -> IrqState {
    unsafe {
        let state: IrqState;
        llvm_asm!("pushf; pop $0; cli" : "=r" (state) : : "memory" : "volatile");
        state
    }
}

/// Restores an old IRQ state.
pub fn irq_restore(state: IrqState) {
    unsafe {
        llvm_asm!("push $0; popf" : : "r" (state) : "memory" : "volatile");
    }
}

extern "x86-interrupt" fn exc_unknown(frame: &mut ISRStackFrame) {
    panic!("Unknown exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_divide_by_zero(frame: &mut ISRStackFrame) {
    panic!("Divide by zero exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_debug(frame: &mut ISRStackFrame) {
    panic!("Debug exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_nmi(frame: &mut ISRStackFrame) {
    panic!("NMI: {:#?}", frame);
}

extern "x86-interrupt" fn exc_breakpoint(frame: &mut ISRStackFrame) {
    panic!("Breakpoint exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_overflow(frame: &mut ISRStackFrame) {
    panic!("Overflow exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_bound_range_exceeded(frame: &mut ISRStackFrame) {
    panic!("Bound range exceeded exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_invalid_opcode(_frame: &mut ISRStackFrame) {
    // Can't check RIP because we do ud2 too in kernel code for userspace.
    println!("wasm trap, thread killed");
    scheduler::thread_exit(u32::MAX); // TODO: exit code
}

extern "x86-interrupt" fn exc_device_not_available(frame: &mut ISRStackFrame) {
    panic!("Device not available: {:#?}", frame);
}

extern "x86-interrupt" fn exc_double_fault(frame: &mut ISRStackFrame, _: u64) {
    panic!("Double fault: {:#?}", frame);
}

extern "x86-interrupt" fn exc_invalid_tss(frame: &mut ISRStackFrame, s: u64) {
    panic!("Invalid TSS: {:#?}, selector: {:x}", frame, s);
}

extern "x86-interrupt" fn exc_segment_not_present(frame: &mut ISRStackFrame, s: u64) {
    panic!("Segment not present: {:#?}, selector: {:x}", frame, s);
}

extern "x86-interrupt" fn exc_stack_segment(frame: &mut ISRStackFrame, err: u64) {
    panic!("Stack segment fault: {:#?}, errcode {:x}", frame, err);
}

extern "x86-interrupt" fn exc_gpf(frame: &mut ISRStackFrame, err: u64) {
    panic!("GPF: {:#?}, errcode {:x}", frame, err);
}

extern "x86-interrupt" fn exc_pf(frame: &mut ISRStackFrame, err: PageFaultError) {
    let addr: VirtAddr;
    unsafe {
        llvm_asm!("movq %cr2, $0" : "=r" (addr));
    }

    //println!("{:?} {:?}", frame, err);
    crate::mm::page_fault(
        addr,
        frame.rip,
        err.contains(PageFaultError::CAUSED_BY_WRITE),
    );
}

extern "x86-interrupt" fn exc_fp(frame: &mut ISRStackFrame) {
    panic!("x87 FP exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_alignment_check(frame: &mut ISRStackFrame, err: u64) {
    panic!("Alignment check: {:#?}, errcode {:x}", frame, err);
}

extern "x86-interrupt" fn exc_machine_check(frame: &mut ISRStackFrame) {
    panic!("Machine check: {:#?}", frame);
}

extern "x86-interrupt" fn exc_simd_fp(frame: &mut ISRStackFrame) {
    panic!("SIMD FP exception: {:#?}", frame);
}

extern "x86-interrupt" fn exc_virtualization(frame: &mut ISRStackFrame) {
    panic!("Virtualization exception: {:#?}", frame);
}

extern "x86-interrupt" fn irq(_frame: &mut ISRStackFrame) {
    //println!("IRQ: {:#?}", _frame);
    // Real EOI to (maybe both) PIC
}

#![allow(dead_code)]

pub unsafe fn read_port8(port: u16) -> u8 {
    let ret: u8;
    llvm_asm!("inb $1, $0" : "={al}" (ret) : "{dx}N" (port) :: "volatile");
    ret
}

pub unsafe fn write_port8(port: u16, val: u8) {
    llvm_asm!("outb $1, $0" :: "{dx}N" (port), "{al}" (val) :: "volatile");
}

pub unsafe fn read_port16(port: u16) -> u16 {
    let ret: u16;
    llvm_asm!("inw $1, $0" : "={ax}" (ret) : "{dx}N" (port) :: "volatile");
    ret
}

pub unsafe fn write_port16(port: u16, val: u16) {
    llvm_asm!("outw $1, $0" :: "{dx}N" (port), "{ax}" (val) :: "volatile");
}

pub unsafe fn read_port32(port: u16) -> u32 {
    let ret: u32;
    llvm_asm!("inl $1, $0" : "={eax}" (ret) : "{dx}N" (port) :: "volatile");
    ret
}

pub unsafe fn write_port32(port: u16, val: u32) {
    llvm_asm!("outl $1, $0" :: "{dx}N" (port), "{eax}" (val) :: "volatile");
}

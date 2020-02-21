#![allow(dead_code)]

pub unsafe fn read_port8(port: u16) -> u8 {
    let ret: u8;
    asm!("inb $1, $0" : "={al}" (ret) : "{dx}N" (port) :: "volatile");
    ret
}

pub unsafe fn write_port8(port: u16, val: u8) {
    asm!("outb $1, $0" :: "{dx}N" (port), "{al}" (val) :: "volatile");
}

pub unsafe fn read_port16(port: u16) -> u16 {
    let ret: u16;
    asm!("inw $1, $0" : "={ax}" (ret) : "{dx}N" (port) :: "volatile");
    ret
}

pub unsafe fn write_port16(port: u16, val: u16) {
    asm!("outw $1, $0" :: "{dx}N" (port), "{ax}" (val) :: "volatile");
}

pub unsafe fn read_port32(port: u16) -> u32 {
    let ret: u32;
    asm!("inl $1, $0" : "={eax}" (ret) : "{dx}N" (port) :: "volatile");
    ret
}

pub unsafe fn write_port32(port: u16, val: u32) {
    asm!("outl $1, $0" :: "{dx}N" (port), "{eax}" (val) :: "volatile");
}

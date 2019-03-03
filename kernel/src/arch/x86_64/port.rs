#![allow(dead_code)]

#[inline]
pub fn read_port8(port: u16) -> u8 {
    let ret: u8;
    unsafe {
        asm!("inb $1, $0" : "={al}" (ret) : "{dx}N" (port) :: "volatile");
    }
    ret
}

#[inline]
pub fn write_port8(port: u16, val: u8) {
    unsafe {
        asm!("outb $1, $0" :: "{dx}N" (port), "{al}" (val) :: "volatile");
    }
}

#[inline]
pub fn read_port16(port: u16) -> u16 {
    let ret: u16;
    unsafe {
        asm!("inw $1, $0" : "={ax}" (ret) : "{dx}N" (port) :: "volatile");
    }
    ret
}

#[inline]
pub fn write_port16(port: u16, val: u16) {
    unsafe {
        asm!("outw $1, $0" :: "{dx}N" (port), "{ax}" (val) :: "volatile");
    }
}

#[inline]
pub fn read_port32(port: u16) -> u32 {
    let ret: u32;
    unsafe {
        asm!("ins $1, $0" : "={eax}" (ret) : "{dx}N" (port) :: "volatile");
    }
    ret
}

#[inline]
pub fn write_port32(port: u16, val: u32) {
    unsafe {
        asm!("outs $1, $0" :: "{dx}N" (port), "{eax}" (val) :: "volatile");
    }
}

use crate::result::Result;
use core::arch::asm;
use core::fmt;
use core::marker::PhantomData;

pub fn hlt() {
    unsafe {
        asm!("hlt")
    }
}

pub fn busy_loop_hint() {
    unsafe {
        asm!("pause")
    }
}

pub fn read_io_port_u8(port: u16) -> u8 {
    let mut data: u8;
    unsafe {
        asm!("in al, dx", out("al") data, in("dx") port)
    }
    data
}

pub fn write_io_port_u8(port: u16, data: u8) {
    unsafe {
        asm!("out dx, al", in("al") data, in("dx") port)
    }
}


pub fn read_cr3() -> *mut RootPageTable {
    let mut cr3: *mut RootPageTable;
    unsafe {
        asm!("mov rax, cr3", out("rax") cr3);
    }
    cr3
}

pub type RootPageTable = [u8; 1024];
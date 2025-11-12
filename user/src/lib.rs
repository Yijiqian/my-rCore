#![no_std]
#![feature(linkage)]

#[macro_use]
pub mod console;
mod syscall;
mod lang_items;
use syscall::*;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub extern "C" fn _start() -> ! {
    clear_bss();
    exit(main());
    panic!("unreachable after sys_exit!");
}

fn clear_bss() {
    unsafe extern "C" {
        safe fn start_bss();
        safe fn end_bss();
    }
    (start_bss as usize..end_bss as usize).for_each(|addr| unsafe {
        (addr as *mut u8).write_volatile(0);
    });
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main() -> i32 {
    panic!("Cannot find main!");
}

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}

pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}

pub fn yield_() -> isize {
    sys_yield()
}
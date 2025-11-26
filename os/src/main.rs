//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`syscall`]: System call handling and implementation
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`batch::run_next_app()`] and for the first time go to
//! userspace.

#![deny(missing_docs)]
#![deny(warnings)]
#![no_main]
#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;  // Rust 内置的 crate

#[macro_use]
extern crate bitflags;

use log::*;

#[path = "boards/qemu.rs"]   // 告诉 Rust 编译器，不要按照默认的规则查找模块文件，而是直接使用指定的文件路径
mod board;  // 声明一个名为 board 的模块，但是这个模块的代码不在默认的 board.rs 或 board/mod.rs 文件中，而是在 boards/qemu.rs 这个特定的文件中。

#[macro_use]
mod console;
mod config;
mod lang_items;
mod sbi;
mod logging;
mod sync;
mod loader;
/// 为什么要声明成 pub ?
pub mod task;
mod timer;

mod mm;


/// 为什么要声明成 pub ?
pub mod trap;
/// 为什么要声明成 pub ?
pub mod syscall;

use core::arch::global_asm;
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

/// the rust entry-point of os
#[unsafe(no_mangle)]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    info!("[kernel] Hello, world!");
    mm::init();

    info!("[kernel] back tpo world!");
    mm::remap_test();
    task::add_initproc();
    trap::init();  // 初始化 Trap 的处理入口点为 __alltraps

    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

/// clear BSS segment
fn clear_bss() {
    unsafe extern "C" {
        safe fn sbss();
        safe fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
                        .fill(0);
    }
}

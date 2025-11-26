mod context;

use riscv::register::{
    mtvec::TrapMode, 
    scause::{self, Exception, Trap, Interrupt},
    sie, stval, stvec
};
use crate::{config::{TRAMPOLINE, TRAP_CONTEXT}, syscall::syscall, task::{current_trap_cx, current_user_token}};
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::set_next_trigger;
use core::arch::global_asm;

global_asm!(include_str!("trap.S"));

/// initialize CSR `stvec` as the entry of `__alltraps`
/// 在批处理系统初始化的时候，修改 stvec 寄存器来指向正确的 Trap 处理入口点
pub fn init() {
    unsafe extern "C" { safe fn __alltraps(); }
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

/// 定义一个从trap 返回到用户级别的函数
#[unsafe(no_mangle)]
pub fn trap_return() -> ! {
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    let user_satp = current_user_token();
    unsafe extern "C" {
        safe fn __alltraps();
        safe fn __restore();
    }
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        core::arch::asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn)
        );
    }
    // panic!("Unreachable in back_to_user!");   // 这行代码永远无法到达，所以注释掉
}

/// 当在内核触发 trap 时，这里不做处理，简单的 panic 结束程序即可
#[unsafe(no_mangle)]
pub fn trap_from_kernel() -> ! {
    panic!("a trap from kernel!");
}

/// handle an interrupt, exception, or system call from user space
#[unsafe(no_mangle)]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let mut cx = current_trap_cx();
    let scause = scause::read();
    // stval 寄存器的主要作用是为 异常处理程序提供关于陷进的附加上下文信息，
    // 帮助确定异常的具体原因和位置
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {

            
            cx.sepc += 4;
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
            cx = current_trap_cx();
            // 将父进程的返回值设置成 子进程的pid
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault) 
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) 
        | Trap::Exception(Exception::InstructionFault) 
        | Trap::Exception(Exception::InstructionPageFault) => {
            println!(
                "[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.",
                stval, cx.sepc
            );
            exit_current_and_run_next(-2);
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next(-3);
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    trap_return();
}

pub use context::TrapContext;

/// 使能时钟中断
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}
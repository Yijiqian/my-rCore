
use crate::task::suspend_current_and_run_next;   // 暂停当前应用并切换到下一个应用
use crate::task::exit_current_and_run_next; 
use crate::task::change_program_brk;

use crate::timer::get_time_us;

pub fn sys_exit(xstate: i32) -> ! {
    println!("[kernel] Application exited with code {}", xstate);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!")
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time_us() as isize
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
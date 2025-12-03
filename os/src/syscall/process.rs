use alloc::sync::Arc;
use crate::task::{suspend_current_and_run_next, exit_current_and_run_next};   // 暂停当前应用并切换到下一个应用
use crate::timer::get_time_us;
use crate::task::{add_task, current_user_token, current_task};
use crate::mm::{translated_refmut, translated_str};
use crate::fs::{OpenFlags, open_file};

pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!")
}

pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_get_time() -> isize {
    get_time_us() as isize
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;

    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // 将子进程的 fork() 返回值设置成 0
    trap_cx.x[10] = 0;
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    /*
      有了文件系统支持之后，应用的 ELF 文件格式的数据就不再需要通过应用加载器从内核的
      数据段获取，而是从文件系统中获取，这样内核与应用的代码/数据就解耦了
    */
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();

    let mut inner = task.inner_exclusive_access();
    if inner.children
        .iter()
        .find(|p| {
            pid == -1 || pid as usize == p.getpid()
        })
        .is_none() {
            return -1;
        }
    let pair = inner.children
        .iter()
        .enumerate()
        .find(|(_, p)| {
            p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();

        let exit_code = child.inner_exclusive_access().exit_code;
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
}
mod context;
mod switch;
mod task;
mod manager;
mod processor;
mod pid;

use alloc::sync::Arc;
use lazy_static::*;
use task::{TaskControlBlock, TaskStatus};
use context::TaskContext;
use crate::loader::get_app_data_by_name;
pub use manager::{add_task, fetch_task};
pub use processor::{take_current_task, schedule, run_tasks, current_user_token, current_trap_cx, current_task};
use switch::__switch;

lazy_static! {
    /// 内核运行的第一个程序
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(
        TaskControlBlock::new(get_app_data_by_name("initproc").unwrap())
    );
}

/// 将第一个运行的程序加入到任务队列中
pub fn add_initproc() {
    add_task(INITPROC.clone());
}



/// 挂起当前任务，运行下一个任务
pub fn suspend_current_and_run_next() {
    let task = take_current_task().unwrap();

    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;

    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    add_task(task);

    schedule(task_cx_ptr);
}

/// 终止当前任务，运行下一任务
pub fn exit_current_and_run_next(exit_code: i32) {
    // mark_current_exited();
    // run_next_task();
    let task = take_current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;

    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }

    inner.children.clear();
    inner.memory_set.recycle_data_pages();
    drop(inner);
    drop(task);
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}


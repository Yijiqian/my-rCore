mod context;
mod switch;
mod task;

use lazy_static::*;
use crate::sync::UPSafeCell;
use task::{TaskControlBlock, TaskStatus};
use crate::trap::TrapContext;
use context::TaskContext;
use switch::__switch;
use crate::loader::{get_app_data, get_num_app};
use crate::sbi::shutdown;

use alloc::vec::Vec;

/// 任务管理器，用于声明一个全局静态的任务管理器，以统一管理所有任务
pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

struct TaskManagerInner {
    tasks: Vec<TaskControlBlock>,
    current_task: usize,
}

impl TaskManager {
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task();
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task();
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &mut inner.tasks[next].task_cx as *mut TaskContext;
            drop(inner);

            unsafe {
                __switch(
                    current_task_cx_ptr,
                    next_task_cx_ptr
                );
            }
        } else {
            println!("All applications completed!");
            shutdown(false);
        }
    }

    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current+1..current+self.num_app+1)
            .map(|id| id % self.num_app)
            .find(|id| {
                inner.tasks[*id].task_status == TaskStatus::Ready
            })
    }

    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();

        unsafe {
            __switch(
                &mut _unused as *mut TaskContext,
                next_task_cx_ptr
            );
        }
        panic!("unreachable in run_first_task!")
    }

    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_user_token()
    }

    fn get_current_trap_cx(&self) -> &mut TrapContext {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_trap_cx()
    }

    /// 改变当前任务的断点
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }
}

impl TaskManagerInner {
    fn current_task(&self) -> usize {
        self.current_task
    }
}

lazy_static! {
    /// 声明并延迟初始化一个全局的任务管理器
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();

        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(
                get_app_data(i),
                i
            ));
        }

        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                tasks,
                current_task: 0,
            })},
        }
    };
}

/// 挂起当前任务，运行下一个任务
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// 终止当前任务，运行下一任务
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// 运行第一个任务
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// 获取当前正在执行的应用的地址空间的 token
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// 获取当前应用的地址空间中的 trap 上下文
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// 改变当前正在运行任务的程序断点
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}
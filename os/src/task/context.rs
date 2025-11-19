//! Implementation of [`TaskContext`]
use crate::trap::trap_return;
/// Task Context
#[derive(Copy, Clone)]
#[repr(C)]
pub struct TaskContext {
    /// __switch 函数返回后的程序执行地址
    ra: usize,
    /// 应用程序的内核栈指针地址
    sp: usize,
    /// 被调用者保存寄存器
    s: [usize; 12],
}

impl TaskContext {
    /// 初始化任务上下文
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
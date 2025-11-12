//! Implementation of [`TaskContext`]

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

    /// 构造每个任务保存在任务控制块中的任务上下文，主要用于内核第一次执行应用程序时
    pub fn goto_restore(kstack_ptr: usize) -> Self {
        // 这样，在__switch 从这里恢复并返回之后就会直接跳转到 __restore ，此时栈顶
        // 是我们构造出来的第一次进入用户态执行的 trap 上下文
        unsafe extern "C" {
            unsafe fn __restore();
        }
        Self {
            ra: __restore as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
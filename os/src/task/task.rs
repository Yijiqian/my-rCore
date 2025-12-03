// use riscv::register::scause::Trap;

use alloc::vec;
use alloc::sync::Weak;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefMut;

use crate::sync::UPSafeCell;
use crate::{config::TRAP_CONTEXT, trap::TrapContext};

use super::TaskContext;
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::trap::trap_handler;
use super::pid::{KernelStack, PidHandle, pid_alloc};
use crate::fs::{File, Stdin, Stdout};


#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,  // 准备运行
    Running,  // 正在运行
    Zombie,  // 僵尸进程
}

pub struct TaskControlBlock {
    // immutable
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,

    // mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}
pub struct TaskControlBlockInner {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,

    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,

    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,

    // Option 用来区分一个文件描述符当前是否空闲；None 表示是空闲的，Some 表示被占用 
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,

    /// 统计了应用数据的大小
    #[allow(unused)]
    pub base_size: usize,
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    pub fn new(elf_data: &[u8]) -> Self {
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
                .translate(VirtAddr::from(TRAP_CONTEXT).into())
                .unwrap()
                .ppn();
        
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner{
                    trap_cx_ppn: trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set: memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        // 0 表示标准输入
                        Some(Arc::new(Stdin)),
                        // 1 表示标准输出
                        Some(Arc::new(Stdout)),
                        // 2 表示标准错误，这里将错误信息输出到标准输出，即 1 和 2 这两个文件描述符共享同一个文件
                        Some(Arc::new(Stdout)),
                    ]
                })
            },
        };
        
        // 在用户空间准备 trap 上下文
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(    // 初始化 trap 上下文
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        let mut parent_inner = self.inner_exclusive_access();

        let memory_set = MemorySet::from_existed_user(
            &parent_inner.memory_set
        );
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn: trap_cx_ppn,
                base_size: parent_inner.base_size,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set: memory_set,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
                fd_table: new_fd_table,
            })},
        });

        parent_inner.children.push(task_control_block.clone());
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;

        task_control_block
    }

    pub fn exec(&self, elf_data: &[u8]) {
        // 从 ELF 文件生成一个全新的地址空间
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
                            .translate(VirtAddr::from(TRAP_CONTEXT).into())
                            .unwrap()
                            .ppn();
        let mut inner = self.inner_exclusive_access();

        // 使用新生成的地址空间替换原来的地址空间，这会导致原有的地址空间生命周期结束，
        // 里面包含的全部物理页帧都会被回收
        inner.memory_set = memory_set;
        inner.trap_cx_ppn = trap_cx_ppn;

        let trap_cx = inner.get_trap_cx();

        // 修改新的地址空间中的 Trap 上下文，将解析得到的应用入口点、用户栈位置以及一些内核的
        // 信息进行初始化，这样才能正常实现 Trap 机制。
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        /*
            值得注意的是，这里无需对任务上下文进行处理，因为这个进程本身已经在执行了，而只有
            被暂停的应用才需要在内核栈上保留一个任务上下文。
         */
    }

}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    pub fn get_status(&self) -> TaskStatus {
        self.task_status
    }

    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }

    pub fn alloc_fd(&mut self) -> usize {
        // 这里 Some 是因为 find 方法的返回值类型是 Option
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            // 文件描述符表中存在未被占用的文件描述符
            fd
        } else {
            // 已分配的文件描述符都是被占用状态，此时需要 push 一个新的空闲的文件描述符
            self.fd_table.push(None);
            self.fd_table.len() - 1  // 返回新添加的文件描述符
        }
    }
}
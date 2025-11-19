// use riscv::register::scause::Trap;

use crate::{config::TRAP_CONTEXT, trap::TrapContext};

use super::TaskContext;
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, MapPermission, KERNEL_SPACE};
use crate::config::kernel_stack_position;
use crate::trap::trap_handler;


#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    #[allow(unused)]
    UnInit, // 未初始化
    Ready,  // 准备运行
    Running,  // 正在运行
    Exited,  // 已退出
}

pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,

    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    /// 统计了应用数据的大小
    #[allow(unused)]
    pub base_size: usize,
    pub heap_bottom: usize,
    pub program_brk: usize,
}

impl TaskControlBlock {
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        println!("******************* Create User Space of app {} *******************", app_id);
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
                .translate(VirtAddr::from(TRAP_CONTEXT).into())
                .unwrap()
                .ppn();
        let task_status = TaskStatus::Ready;

        // 在内核空间映射一个内核栈
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        
        // 在内核地址空间创建应用程序 app_id 的内核栈的逻辑块
        KERNEL_SPACE
            .exclusive_access()
            .insert_framed_area(
                kernel_stack_bottom.into(),
                kernel_stack_top.into(),
                MapPermission::R | MapPermission::W,
            );
        let task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),   // 初始化任务上下文
            memory_set,
            trap_cx_ppn,
            heap_bottom: user_sp,
            program_brk: user_sp,
            base_size: user_sp,
        };

        // 在用户空间准备 trap 上下文
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(    // 初始化 trap 上下文
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    /// 改变程序断点的位置，失败则返回 None
    pub fn change_program_brk(&mut self, size: i32) -> Option<usize> {
        let old_break = self.program_brk;
        let new_break = self.program_brk as isize + size as isize;
        if new_break < self.heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            self.memory_set
                .shrink_to(VirtAddr(self.heap_bottom), VirtAddr(new_break as usize))
        } else {
            self.memory_set
                .append_to(VirtAddr(self.heap_bottom), VirtAddr(new_break as usize))
        };
        if result {
            self.program_brk = new_break as usize;
            Some(old_break)
        } else {
            None
        }
    }
}
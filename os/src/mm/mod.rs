mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{FrameTracker, frame_alloc};
pub use memory_set::remap_test;
pub use memory_set::{KERNEL_SPACE, MapPermission, MemorySet};
use page_table::{PTEFlags, PageTable};
pub use page_table::{PageTableEntry, translated_byte_buffer};

pub fn init() {
    // 全局动态内存分配器初始化，以使用 Rust 的堆数据结构
    heap_allocator::init_heap();
    // 初始化物理页帧管理器，内包含堆数据结构 Vec<T>，使能物理页帧的分配和回收能力
    frame_allocator::init_frame_allocator();
    // 创建内核地址空间并让 CPU 开启分页模式。
    KERNEL_SPACE.exclusive_access().activate();
    println!("mm init success!");
}
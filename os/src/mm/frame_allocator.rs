use alloc::vec::Vec;
use crate::sync::UPSafeCell;
use lazy_static::lazy_static;
use super::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;

type FrameAllocatorImp = StackFrameAllocator;

lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImp> = unsafe {
        UPSafeCell::new(FrameAllocatorImp::new())
    };
}

pub fn init_frame_allocator() {
    unsafe extern "C" {
        safe fn ekernel();
    }
    FRAME_ALLOCATOR.exclusive_access()
                   .init(PhysAddr::from(ekernel as usize).ceil(),    // [ekernel, MEMORY_END] = [0x806d6000, 0x80800000]
                         PhysAddr::from(MEMORY_END).floor()
                        );
}

trait FrameAllocator {
    fn new() -> Self;
    fn alloc(&mut self) -> Option<PhysPageNum>;
    fn dealloc(&mut self, ppn: PhysPageNum);
}

pub struct StackFrameAllocator {
    current: usize,          // 空闲内存的起始物理页号
    end: usize,              // 空闲内存的结束物理页号
    recycled: Vec<usize>,    // 已分配且回收了的物理页号
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else {
            if self.current == self.end { 
                println!("All the frames haved beem used up!");
                None 
            }
            else {
                self.current += 1;
                Some((self.current - 1).into())
            }
        }
    }

    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        if ppn >= self.current || self.recycled
                                    .iter()
                                    .find(|&v| *v == ppn)
                                    .is_some()
        {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }

        self.recycled.push(ppn);
    }
}

impl StackFrameAllocator {
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}

pub fn frame_alloc() -> Option<FrameTracker> {
    // 返回一个 FrameTracker 实例，是为了将 物理页帧 的生命周期绑定在一个变量上
    FRAME_ALLOCATOR.exclusive_access()
                   .alloc()
                   .map(|ppn| FrameTracker::new(ppn))
}

pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access()
                   .dealloc(ppn);
}

pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // 清除页帧内容
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}

use core::fmt::{self, Debug, Formatter};
impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

impl Drop for FrameTracker {
    /// 当一个 FrameTracker 生命周期结束被编译器回收时，需要将它控制的物理页帧回收到 FRAME_ALLOCATOR 中
    /// 实现这个 Drop 特征后，就不必手动回收物理页帧，这在编译期就解决了很多潜在的问题
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

#[allow(unused)]
pub fn frame_allocator_test() {
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }

    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    println!("frame_allocator_test passed!");
}
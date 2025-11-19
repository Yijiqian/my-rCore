use bitflags::*;
use super::{PhysPageNum, VirtPageNum, VirtAddr, FrameTracker, frame_alloc, StepByOne};
use alloc::vec::Vec;
use alloc::vec;


bitflags!{
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }

    pub fn empty() -> Self {
        PageTableEntry {
            bits: 0,
        }
    }

    /// 从页表项取出物理页面号, 56位物理地址
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ( (1usize << 44) - 1)).into()  // usize 转 PhysPageNum
    }

    /// 从页表项取出 页表项标志位
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }

    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }

    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }

    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }

    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

pub struct PageTable {
    // 页表根节点
    root_ppn: PhysPageNum,   

    // 这个字段的作用到底是什么？收集根页表的下级页表节点，方便后续的自动回收
    frames: Vec<FrameTracker>,   
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        // 实际上就是设置第三级页表 在第三级索引处的页表项
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is unmapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }

    /// 根据虚拟页号在页表中查找 存储物理页号的页表项。
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();  // 获取在各级页表中索引
        let mut ppn = self.root_ppn;   // 根页表
        let mut result: Option<&mut PageTableEntry> = None;
        for i in 0..3 {
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i==2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                // 1. 分配一个空白帧，类型是 Frame_Tracker
                let frame = frame_alloc().unwrap();   
                // 2. 现在已经有了下一张页表，则再为这张页表创建在上一级页表的页表项
                *pte = PageTableEntry::new( frame.ppn, PTEFlags::V);   
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for i in 0..3 {
            let pte = &mut ppn.get_pte_array()[idxs[i]];
            if i==2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 临时创建一个专用于手动查页表的 PageTable。注意它的 frames 字段为空，因此不实际控制任何资源
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    /// 使用 translate 方法查找临时页表
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn)
            .map(|pte| {pte.clone()})
    }

    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

pub fn translated_byte_buffer(
    token: usize,
    ptr: *const u8,
    len: usize
) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table
            .translate(vpn)
            .unwrap()
            .ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}
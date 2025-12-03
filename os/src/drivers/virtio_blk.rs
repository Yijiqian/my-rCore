
use virtio_drivers::{VirtIOBlk, VirtIOHeader, Hal};
use lazy_static::lazy_static;
use crate::sync::UPSafeCell;
use easy_fs::BlockDevice;
use alloc::vec::Vec;
use crate::mm::{FrameTracker, PageTable, PhysAddr, PhysPageNum, StepByOne, VirtAddr, frame_alloc, frame_dealloc};
use crate::mm::kernel_token;

const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock(UPSafeCell<VirtIOBlk<'static, VirtioHal>>);

impl VirtIOBlock {
    pub fn new() -> Self {
        Self( unsafe {
            UPSafeCell::new(VirtIOBlk::new(
             &mut *(VIRTIO0 as *mut VirtIOHeader) 
        ).unwrap())
        })
    }
}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.0.exclusive_access().read_block(block_id, buf).expect("Error when reading VirtIOBlk");
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0.exclusive_access().write_block(block_id, buf).expect("Error when writing VirtIOBlk");
    }
}

/*
    VirtIO 设备需要占用部分内存作为一个公共区域从而更好的和 CPU 进行合作。这就像 MMU
    需要在内存中保存多级页表才能和 CPU 共同实现分页机制一样。在 VirtIO 架构下，需要在
    公共区域中放置一种叫做 VirtQueue 的环形队列，CPU 可以向此环形队列中向 VirtIO 设备
    提交请求，也可以从队列中取得请求的结果。
    对于 VirtQueue 的使用涉及到物理内存的分配和回收，但这并不在 VirtIO 驱动 Virttio-drivers
    的职责范围之内，因此它声明了数个相关的接口，需要库的使用者自己来实现。
*/
lazy_static! {
    static ref QUEUE_FRAMES: UPSafeCell<Vec<FrameTracker>> = unsafe { UPSafeCell::new(Vec::new()) };
}

pub struct VirtioHal;

impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> usize {
        let mut ppn_base = PhysPageNum(0);
        for i in 0..pages {
            let frame = frame_alloc().unwrap();
            if i==0 { ppn_base = frame.ppn; }
            assert_eq!(frame.ppn.0, ppn_base.0 + i);
            QUEUE_FRAMES.exclusive_access().push(frame);
        }
        let pa: PhysAddr = ppn_base.into();
        pa.0
    }

    fn dma_dealloc(pa: usize, pages: usize) -> i32 {
        let pa = PhysAddr::from(pa);
        let mut ppn_base: PhysPageNum = pa.into();
        for _ in 0..pages {
            frame_dealloc(ppn_base);
            ppn_base.step();
        }
        0
    }

    fn phys_to_virt(paddr: usize) -> usize {
        paddr   // 恒等映射
    }

    fn virt_to_phys(vaddr: usize) -> usize {
        PageTable::from_token(kernel_token())
            .translate_va(VirtAddr::from(vaddr))
            .unwrap()
            .0

    }
}
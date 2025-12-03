use spin::Mutex;
use alloc::sync::Arc;

use super::BLOCK_SZ;
use crate::block_dev::BlockDevice;
use super::Bitmap;
use super::{DiskInode, DataBlock, SuperBlock, DiskInodeType};
use crate::block_cache::get_block_cache;
use super::Inode;

/// 简易文件系统的抽象
pub struct EasyFileSystem {
    /// 块设备
    pub block_device: Arc<dyn BlockDevice>,
    /// 索引节点位图
    pub inode_bitmap: Bitmap,
    /// 数据块的位图
    pub data_bitmap: Bitmap,
    inode_area_start_block: u32,
    data_area_start_block: u32,
}

impl EasyFileSystem {
    /// 创建一个 简易文件系统 的实例
    pub fn create(
        block_device: Arc<dyn BlockDevice>,
        total_blocks: u32,        // 管理的磁盘块总数量
        inode_bitmap_blocks: u32,    // 索引节点位图所需的磁盘块总数量
    ) -> Arc<Mutex<Self>> {
        let inode_bitmap = Bitmap::new(1, inode_bitmap_blocks as usize);
        let inode_num = inode_bitmap.maximum();
        let inode_area_blocks = ((inode_num * core::mem::size_of::<DiskInode>() + BLOCK_SZ - 1) / BLOCK_SZ) as u32;
        let inode_total_blocks = inode_bitmap_blocks + inode_area_blocks;
        let data_total_blocks = total_blocks - 1 - inode_total_blocks;
        let data_bitmap_blocks = (data_total_blocks + 4096) / 4097;
        let data_area_blocks = data_total_blocks - data_bitmap_blocks;
        let data_bitmap = Bitmap::new(
            (1 + inode_bitmap_blocks + inode_area_blocks) as usize,
            data_bitmap_blocks as usize,
        );

        let mut efs = Self {
            block_device: Arc::clone(&block_device),
            inode_bitmap,
            data_bitmap,
            inode_area_start_block: 1 + inode_bitmap_blocks,
            data_area_start_block: 1 + inode_total_blocks + data_bitmap_blocks,
        };

        for i in 0..total_blocks {
            get_block_cache(
                i as usize,
                Arc::clone(&block_device)
            )
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                for byte in data_block.iter_mut() {
                    *byte = 0;
                }
            });
        }
        get_block_cache(0, Arc::clone(&block_device))
        .lock()
        .modify(0, |super_block: &mut SuperBlock| {
            super_block.initialize(
                total_blocks,
                inode_bitmap_blocks,
                inode_area_blocks,
                data_bitmap_blocks,
                data_area_blocks,
            );
        });

        assert_eq!(efs.alloc_inode(), 0);  // 分配一个编号为 0 的索引节点
        let (root_inode_block_id, root_inode_offset) = efs.get_disk_inode_pos(0);
        get_block_cache(
            root_inode_block_id as usize,
            Arc::clone(&block_device)
        )
        .lock()
        .modify(root_inode_offset, |disk_inode: &mut DiskInode| {
            disk_inode.initialize(DiskInodeType::Directory);
        });
        Arc::new(Mutex::new(efs))
    }

    /// 从一个已写入了 easy-fs 镜像的块设备上打开 easy-fs
    pub fn open(block_device: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        get_block_cache(0, Arc::clone(&block_device))
            .lock()
            .read(0, |super_block: &SuperBlock| {
                assert!(super_block.is_vaild(), "Error loading EFS!");
                let inode_total_blocks = 
                    super_block.inode_bitmap_blocks + super_block.inode_area_blocks;
                let efs = Self {
                    block_device,
                    inode_bitmap: Bitmap::new(
                        1,
                        super_block.inode_bitmap_blocks as usize
                    ),
                    data_bitmap: Bitmap::new(
                        (1 + inode_total_blocks) as usize,
                        super_block.data_bitmap_blocks as usize
                    ),
                    inode_area_start_block: 1 + super_block.inode_bitmap_blocks,
                    data_area_start_block: 1 + inode_total_blocks + super_block.data_bitmap_blocks,
                };
                Arc::new(Mutex::new(efs))
            })
    }

    /// 这里inode_id表示索引编号，对应着在索引节点位图中的第几个bit位
    /// 返回值：索引节点所在的块编号，以及在块内的偏移（因为一个块可以存储多个索引节点）
    pub fn get_disk_inode_pos(&self, inode_id: u32) -> (u32, usize) {
        let inode_size = core::mem::size_of::<DiskInode>();
        let inodes_per_lock = (BLOCK_SZ / inode_size) as u32;
        let block_id = self.inode_area_start_block + inode_id / inodes_per_lock;
        (block_id, (inode_id % inodes_per_lock) as usize * inode_size)
    }

    /// 得到数据块的真实编号
    pub fn get_data_block_id(&self, data_block_id: u32) -> u32 {
        self.data_area_start_block + data_block_id
    }

    /// 创建一个根目录节点（内存视角）
    pub fn root_inode(efs: &Arc<Mutex<Self>>) -> Inode {
        let block_device = Arc::clone(&efs.lock().block_device);

        let (block_id, block_offset) = efs.lock().get_disk_inode_pos(0);

        Inode::new(
            block_id,
            block_offset,
            Arc::clone(efs),
            block_device,
        )
    }

    /// 分配一个索引节点编号，而不是直接分配一个索引节点（磁盘节点）
    pub fn alloc_inode(&mut self) -> u32 {
        self.inode_bitmap.alloc(&self.block_device).unwrap() as u32
    }

    /// 分配一个数据块编号，这个编号是在整个磁盘块中的编号
    pub fn alloc_data(&mut self) -> u32 {
        self.data_bitmap.alloc(&self.block_device) .unwrap() as u32 + self.data_area_start_block
    }

    /// 释放指定编号的数据块的数据，以及释放数据块位图中的 bit 位
    pub fn dealloc_data(&mut self, block_id: u32) {
        get_block_cache(
            block_id as usize,
            Arc::clone(&self.block_device)
        )
        .lock()
        .modify(0, |data_block: &mut DataBlock| {
            data_block.iter_mut().for_each(|p| { *p = 0; })
        });
        self.data_bitmap.dealloc(
            &self.block_device,
            (block_id - self.data_area_start_block) as usize
        )
    }
}
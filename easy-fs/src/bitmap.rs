use crate::BLOCK_SZ;
use crate::block_dev::BlockDevice;
use crate::block_cache::get_block_cache;

use alloc::sync::Arc;

// 本质是一个大小为64的 u64 类型的数组。
// 一个 u64 的数据有64个bit，因而一个 BitmapBlock 一共有4096 个bit;
// 一个磁盘块可以存储 512 个字节，即存储 512*8 = 4096 个 bit;
type BitmapBlock = [u64; 64];
const BLOCK_BITS: usize = BLOCK_SZ * 8;   // BLOCK_SZ = 512

pub struct Bitmap {
    // 注意：Bitmap 自身是驻留在内存中的，其能表示索引节点/数据块区域中的
    //       那些磁盘块的分配情况。

    start_block_id: usize,
    blocks: usize,    // 位图块的数量
}

impl Bitmap {
    pub fn new(start_block_id: usize, blocks: usize) -> Self {
        Self {
            start_block_id,
            blocks,
        }
    }

    pub fn alloc(&self, block_device: &Arc<dyn BlockDevice>) -> Option<usize> {
        for block_id in 0..self.blocks {
            let pos = get_block_cache(
                block_id + self.start_block_id as usize,
                Arc::clone(block_device),
            )
            .lock()
            .modify(0, |bitmap_block: &mut BitmapBlock| {
                if let Some((bits64_pos, inner_pos)) = bitmap_block
                    .iter()
                    .enumerate()
                    .find(|(_, bits64)| **bits64 != u64::MAX)
                    .map(|(bits64_pos, bits64)| {
                        // u64::trailing_ones 的作用是找到最低的一个 0 并置1.
                        (bits64_pos, bits64.trailing_ones() as usize)
                    }) {
                        bitmap_block[bits64_pos] |= 1u64 << inner_pos;
                        Some(block_id * BLOCK_BITS + bits64_pos * 64 + inner_pos as usize)
                    } else {
                        None
                    }
            });
            if pos.is_some() {
                return pos;
            }
        }
        None
    }

    pub fn dealloc(&self, block_device: &Arc<dyn BlockDevice>, bit: usize) {
        let (block_pos, bits64_pos, inner_pos) = decomposition(bit);
        get_block_cache(
            block_pos + self.start_block_id,
            Arc::clone(block_device)
        ).lock().modify(0, |bitmap_block: &mut BitmapBlock| {
            assert!(bitmap_block[bits64_pos] & (1u64 << inner_pos) > 0);
            bitmap_block[bits64_pos] -= 1u64 << inner_pos;
        });
    }

    // 获取可分配块的最大编号
    pub fn maximum(&self) -> usize {
        self.blocks * BLOCK_BITS
    }
}

fn decomposition(mut bit: usize) -> (usize, usize, usize) {
    let block_pos = bit / BLOCK_BITS;
    bit = bit % BLOCK_BITS;
    (block_pos, bit/64, bit%64)
}
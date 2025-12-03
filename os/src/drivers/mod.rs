
mod virtio_blk;

use lazy_static::lazy_static;
use alloc::sync::Arc;
use easy_fs::BlockDevice;


type BlockDeviceImpl = virtio_blk::VirtIOBlock;

lazy_static! {
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}
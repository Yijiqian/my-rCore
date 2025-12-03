//! 一个与内核隔离的简易文件系统

#![no_std]
#![deny(missing_docs)]    // 强制要求编写文档注释

extern crate alloc;
mod bitmap;
mod block_cache;
mod block_dev;
mod efs;
mod vfs;
mod layout;

/// 一个磁盘块的大小为 512 个字节
pub const BLOCK_SZ: usize = 512;
use bitmap::Bitmap;
pub use block_dev::BlockDevice;
pub use efs::EasyFileSystem;
pub use vfs::Inode;
use block_cache::{get_block_cache};
use layout::*;
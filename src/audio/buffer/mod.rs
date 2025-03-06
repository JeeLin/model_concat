//! 音频缓冲区管理模块
//!
//! 本模块提供高效的音频数据缓冲区管理机制，主要包括以下功能：
//!
//! - 基本缓冲区池：通过预分配和重用缓冲区来减少内存分配开销
//! - 优化缓冲区池：根据缓冲区大小分类和使用模式进行优化，提高内存利用率
//! - 零拷贝缓冲区：通过引用计数和智能指针实现零拷贝数据传输，减少数据复制
//! - 全局缓冲区池：提供全局单例的缓冲区池，方便跨模块复用缓冲区资源
//!
//! # 示例
//!
//! ```rust
//! use crate::audio::buffer::{BufferPool, global};
//!
//! // 使用全局缓冲区池
//! let buffer = global::get_buffer(1024);
//! // 处理完后归还缓冲区
//! global::return_buffer(buffer);
//!
//! // 或者创建独立的缓冲区池
//! let pool = BufferPool::new(32, 16384); // 32个缓冲区，每个16KB
//! let buffer = pool.get_buffer(1024);
//! pool.return_buffer(buffer);
//! ```

mod pool;
mod pool_opt;
mod zero_copy;

pub use pool::SharedBufferPool;
pub use zero_copy::{SharedZeroCopyBuffer, ZeroCopyBuffer};

/// 全局缓冲区池访问
pub mod global {
    use super::*;
    use std::sync::OnceLock;

    static GLOBAL_POOL: OnceLock<SharedBufferPool> = OnceLock::new();

    /// 获取全局缓冲区池
    ///
    /// 返回全局单例的共享缓冲区池实例。首次调用时会初始化池。
    pub fn pool() -> &'static SharedBufferPool {
        GLOBAL_POOL.get_or_init(|| {
            SharedBufferPool::new(32, 16384) // 默认32个缓冲区，每个16KB
        })
    }

    /// 从全局池获取缓冲区
    ///
    /// # 参数
    /// * `min_size` - 所需的最小缓冲区大小（字节）
    pub fn get_buffer(min_size: usize) -> bytes::BytesMut {
        pool().get_buffer(min_size)
    }

    /// 将缓冲区归还全局池
    ///
    /// # 参数
    /// * `buffer` - 要归还的缓冲区
    pub fn return_buffer(buffer: bytes::BytesMut) {
        pool().return_buffer(buffer);
    }
}

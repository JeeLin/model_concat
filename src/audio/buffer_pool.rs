//! 音频缓冲区池模块
//!
//! 提供音频缓冲区的高效管理和复用功能，包括缓冲区的分配、回收和状态管理。
//! 主要包括缓冲区池（BufferPool）和缓冲区状态（BufferState）的实现。

use bytes::BytesMut;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

/// 缓冲区状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferState {
    /// 空闲状态
    Idle,
    /// 使用中
    InUse,
    /// 已满
    Full,
}

/// 音频缓冲区池
///
/// 管理音频缓冲区的分配和回收，提供缓冲区复用功能。
/// 使用互斥锁确保并发安全。
pub struct BufferPool {
    /// 缓冲区队列
    buffers: Arc<Mutex<VecDeque<BytesMut>>>,
    /// 缓冲区大小（字节）
    buffer_size: usize,
    /// 缓冲区容量
    capacity: usize,
}

impl BufferPool {
    /// 创建新的缓冲区池
    ///
    /// # 参数
    ///
    /// * `buffer_size` - 每个缓冲区的大小（字节）
    /// * `capacity` - 池的容量（缓冲区数量）
    pub fn new(buffer_size: usize, capacity: usize) -> Self {
        let mut buffers = VecDeque::with_capacity(capacity);
        for _ in 0..capacity {
            buffers.push_back(BytesMut::with_capacity(buffer_size));
        }

        Self {
            buffers: Arc::new(Mutex::new(buffers)),
            buffer_size,
            capacity,
        }
    }

    /// 获取空闲缓冲区
    ///
    /// 如果没有空闲缓冲区，则创建新的缓冲区
    pub fn get_buffer(&self) -> BytesMut {
        if let Ok(mut buffers) = self.buffers.lock() {
            if let Some(buffer) = buffers.pop_front() {
                buffer
            } else {
                BytesMut::with_capacity(self.buffer_size)
            }
        } else {
            // 如果获取锁失败，创建新的缓冲区
            BytesMut::with_capacity(self.buffer_size)
        }
    }

    /// 回收缓冲区
    ///
    /// # 参数
    ///
    /// * `buffer` - 要回收的缓冲区
    pub fn recycle_buffer(&self, mut buffer: BytesMut) {
        // 清空缓冲区数据
        buffer.clear();

        if let Ok(mut buffers) = self.buffers.lock() {
            if buffers.len() < self.capacity {
                buffers.push_back(buffer);
            }
        }
    }

    /// 获取当前空闲缓冲区数量
    pub fn available_buffers(&self) -> usize {
        self.buffers.lock().map(|buffers| buffers.len()).unwrap_or(0)
    }

    /// 获取缓冲区大小
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// 获取池容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(1024, 5);

        // 测试获取缓冲区
        let buffer1 = pool.get_buffer();
        assert_eq!(pool.available_buffers(), 4);

        let buffer2 = pool.get_buffer();
        assert_eq!(pool.available_buffers(), 3);

        // 测试回收缓冲区
        pool.recycle_buffer(buffer1);
        assert_eq!(pool.available_buffers(), 4);

        pool.recycle_buffer(buffer2);
        assert_eq!(pool.available_buffers(), 5);
    }
}
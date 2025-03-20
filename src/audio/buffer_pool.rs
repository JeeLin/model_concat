//! 音频缓冲区池
//!
//! 提供音频缓冲区的复用和管理功能，通过池化技术提高内存使用效率。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::buffer::{BufferConfig, RingBuffer};

/// 缓冲区池配置
#[derive(Debug, Clone)]
pub struct BufferPoolConfig {
    /// 池中缓冲区的数量
    pub pool_size: usize,
    /// 每个缓冲区的配置
    pub buffer_config: BufferConfig,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            pool_size: 8,
            buffer_config: BufferConfig::default(),
        }
    }
}

/// 音频缓冲区池
///
/// 管理一组音频缓冲区，支持缓冲区的获取和归还操作。
#[derive(Debug, Clone)]
pub struct BufferPool {
    /// 空闲缓冲区队列
    free_buffers: Arc<Mutex<VecDeque<RingBuffer>>>,
    /// 池配置
    config: BufferPoolConfig,
}

impl BufferPool {
    /// 创建新的缓冲区池
    pub fn new(config: BufferPoolConfig) -> Self {
        let mut free_buffers = VecDeque::with_capacity(config.pool_size);
        for _ in 0..config.pool_size {
            free_buffers.push_back(RingBuffer::new(config.buffer_config.clone()));
        }

        Self {
            free_buffers: Arc::new(Mutex::new(free_buffers)),
            config,
        }
    }

    /// 获取一个缓冲区
    pub fn acquire(&self) -> Option<RingBuffer> {
        let mut buffers = self.free_buffers.lock().unwrap();
        buffers.pop_front()
    }

    /// 归还一个缓冲区
    pub fn release(&self, mut buffer: RingBuffer) {
        buffer.clear();
        let mut buffers = self.free_buffers.lock().unwrap();
        if buffers.len() < self.config.pool_size {
            buffers.push_back(buffer);
        }
    }

    /// 获取池配置
    pub fn config(&self) -> &BufferPoolConfig {
        &self.config
    }

    /// 获取当前空闲缓冲区数量
    pub fn available_buffers(&self) -> usize {
        self.free_buffers.lock().unwrap().len()
    }
}
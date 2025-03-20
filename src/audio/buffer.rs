//! 音频缓冲区模块
//!
//! 提供音频数据的缓冲区管理功能，包括环形缓冲区的实现和缓冲区池的管理。
//! 主要用于临时存储解码后的PCM数据和编码前的数据，确保音频处理的连续性。

use bytes::{Bytes, BytesMut};
use std::sync::{Arc, Mutex};

/// 音频缓冲区配置
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// 缓冲区大小（字节）
    pub capacity: usize,
    /// 是否允许覆盖
    pub allow_overwrite: bool,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            capacity: 1024 * 1024, // 1MB
            allow_overwrite: false,
        }
    }
}

/// 音频环形缓冲区
///
/// 提供线程安全的环形缓冲区实现，支持音频数据的读写操作。
#[derive(Debug)]
pub struct RingBuffer {
    /// 内部数据
    buffer: Arc<Mutex<BytesMut>>,
    /// 读指针位置
    read_pos: Arc<Mutex<usize>>,
    /// 写指针位置
    write_pos: Arc<Mutex<usize>>,
    /// 缓冲区配置
    config: BufferConfig,
}

impl RingBuffer {
    /// 创建新的环形缓冲区
    pub fn new(config: BufferConfig) -> Self {
        let buffer = BytesMut::with_capacity(config.capacity);
        Self {
            buffer: Arc::new(Mutex::new(buffer)),
            read_pos: Arc::new(Mutex::new(0)),
            write_pos: Arc::new(Mutex::new(0)),
            config,
        }
    }

    /// 写入数据
    ///
    /// 返回实际写入的字节数
    pub fn write(&self, data: &[u8]) -> usize {
        let mut buffer = self.buffer.lock().unwrap();
        let mut write_pos = self.write_pos.lock().unwrap();
        let mut read_pos = self.read_pos.lock().unwrap();

        let available = if *write_pos >= *read_pos {
            self.config.capacity - (*write_pos - *read_pos)
        } else {
            *read_pos - *write_pos
        };

        let write_len = data.len().min(available);
        if write_len == 0 {
            return 0;
        }

        // 写入数据
        for i in 0..write_len {
            let pos = (*write_pos + i) % self.config.capacity;
            if pos >= buffer.len() {
                buffer.extend_from_slice(&[0]);
            }
            buffer[pos] = data[i];
        }

        *write_pos = (*write_pos + write_len) % self.config.capacity;
        write_len
    }

    /// 读取数据
    ///
    /// 返回读取的数据
    pub fn read(&self, size: usize) -> Bytes {
        let buffer = self.buffer.lock().unwrap();
        let mut read_pos = self.read_pos.lock().unwrap();
        let write_pos = self.write_pos.lock().unwrap();

        let available = if *write_pos >= *read_pos {
            *write_pos - *read_pos
        } else {
            self.config.capacity - (*read_pos - *write_pos)
        };

        let read_len = size.min(available);
        let mut result = BytesMut::with_capacity(read_len);

        // 读取数据
        for i in 0..read_len {
            let pos = (*read_pos + i) % self.config.capacity;
            result.extend_from_slice(&[buffer[pos]]);
        }

        *read_pos = (*read_pos + read_len) % self.config.capacity;
        result.freeze()
    }

    /// 获取可用数据大小
    pub fn available(&self) -> usize {
        let read_pos = *self.read_pos.lock().unwrap();
        let write_pos = *self.write_pos.lock().unwrap();

        if write_pos >= read_pos {
            write_pos - read_pos
        } else {
            self.config.capacity - (read_pos - write_pos)
        }
    }

    /// 清空缓冲区
    pub fn clear(&self) {
        let mut read_pos = self.read_pos.lock().unwrap();
        let mut write_pos = self.write_pos.lock().unwrap();
        *read_pos = 0;
        *write_pos = 0;
    }
}
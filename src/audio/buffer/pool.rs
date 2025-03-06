use bytes::BytesMut;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 音频缓冲区池
///
/// 用于管理和重用音频处理过程中的缓冲区，减少内存分配和复制。
/// 主要特点：
/// - 缓冲区重用：通过池化技术减少内存分配
/// - 自动扩容：根据需求动态调整缓冲区大小
/// - 容量控制：限制池中缓冲区数量，防止内存泄漏
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::BufferPool;
///
/// let mut pool = BufferPool::new(32, 4096); // 最多32个缓冲区，默认4KB
///
/// // 获取缓冲区
/// let buffer = pool.get_buffer(2048);
/// // 使用缓冲区...
///
/// // 归还缓冲区到池中
/// pool.return_buffer(buffer);
/// ```
pub struct BufferPool {
    /// 可用的缓冲区列表
    buffers: VecDeque<BytesMut>,
    /// 池的最大容量
    max_size: usize,
    /// 默认缓冲区大小
    default_capacity: usize,
}

impl BufferPool {
    /// 创建新的缓冲区池
    ///
    /// # 参数
    /// * `max_size` - 池中允许的最大缓冲区数量
    /// * `default_capacity` - 默认的缓冲区容量（字节）
    pub fn new(max_size: usize, default_capacity: usize) -> Self {
        Self {
            buffers: VecDeque::with_capacity(max_size),
            max_size,
            default_capacity,
        }
    }

    /// 从池中获取缓冲区
    ///
    /// # 参数
    /// * `min_size` - 所需的最小缓冲区大小（字节）
    ///
    /// # 返回值
    /// 返回一个至少具有指定大小的缓冲区
    pub fn get_buffer(&mut self, min_size: usize) -> BytesMut {
        // 尝试从池中获取合适大小的缓冲区
        for i in 0..self.buffers.len() {
            if self.buffers[i].capacity() >= min_size {
                return self.buffers.remove(i).unwrap();
            }
        }

        // 如果没有合适的缓冲区，创建一个新的
        let capacity = min_size.max(self.default_capacity);
        BytesMut::with_capacity(capacity)
    }

    /// 将缓冲区返回池中
    ///
    /// # 参数
    /// * `buffer` - 要归还的缓冲区
    pub fn return_buffer(&mut self, mut buffer: BytesMut) {
        // 清空缓冲区内容但保留容量
        buffer.clear();

        // 如果池未满，将缓冲区添加到池中
        if self.buffers.len() < self.max_size {
            self.buffers.push_back(buffer);
        }
        // 否则缓冲区将被丢弃并由Rust的内存管理系统回收
    }

    /// 清空缓冲区池
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    /// 获取当前池中缓冲区数量
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// 检查池是否为空
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

/// 线程安全的缓冲区池
///
/// 提供线程安全的缓冲区池访问，适用于多线程音频处理场景。
/// 主要特点：
/// - 线程安全：使用互斥锁保护内部状态
/// - 高效共享：通过Arc实现高效的跨线程共享
/// - 内存复用：继承底层BufferPool的缓冲区重用特性
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::SharedBufferPool;
/// use std::sync::Arc;
/// use std::thread;
///
/// let pool = SharedBufferPool::new(32, 4096);
/// let pool_clone = pool.clone();
///
/// // 在不同线程中安全使用
/// let handle = thread::spawn(move || {
///     let buffer = pool_clone.get_buffer(2048);
///     // 处理音频数据...
///     pool_clone.return_buffer(buffer);
/// });
///
/// // 主线程中使用
/// let buffer = pool.get_buffer(1024);
/// // 处理音频数据...
/// pool.return_buffer(buffer);
///
/// handle.join().unwrap();
/// ```
pub struct SharedBufferPool {
    /// 内部缓冲区池
    inner: Arc<Mutex<BufferPool>>,
}

impl SharedBufferPool {
    /// 创建新的线程安全缓冲区池
    pub fn new(max_size: usize, default_capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BufferPool::new(max_size, default_capacity))),
        }
    }

    /// 从池中获取缓冲区
    pub fn get_buffer(&self, min_size: usize) -> BytesMut {
        let mut pool = self.inner.lock().unwrap();
        pool.get_buffer(min_size)
    }

    /// 将缓冲区返回池中
    pub fn return_buffer(&self, buffer: BytesMut) {
        let mut pool = self.inner.lock().unwrap();
        pool.return_buffer(buffer);
    }

    /// 清空缓冲区池
    pub fn clear(&self) {
        let mut pool = self.inner.lock().unwrap();
        pool.clear();
    }

    /// 获取当前池中缓冲区数量
    pub fn len(&self) -> usize {
        let pool = self.inner.lock().unwrap();
        pool.len()
    }

    /// 检查池是否为空
    pub fn is_empty(&self) -> bool {
        let pool = self.inner.lock().unwrap();
        pool.is_empty()
    }

    /// 创建池的克隆
    pub fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_basic_operations() {
        let mut pool = BufferPool::new(2, 8);

        // 测试获取缓冲区
        let buffer1 = pool.get_buffer(4);
        assert!(buffer1.capacity() >= 4);
        assert_eq!(pool.len(), 0);

        // 测试返回缓冲区
        pool.return_buffer(buffer1);
        assert_eq!(pool.len(), 1);

        // 测试获取特定大小的缓冲区
        let buffer2 = pool.get_buffer(16);
        assert!(buffer2.capacity() >= 16);
    }

    #[test]
    fn test_buffer_pool_capacity_management() {
        let mut pool = BufferPool::new(2, 8);

        // 测试池容量限制
        let buffer1 = pool.get_buffer(4);
        let buffer2 = pool.get_buffer(4);
        let buffer3 = pool.get_buffer(4);

        pool.return_buffer(buffer1);
        pool.return_buffer(buffer2);
        pool.return_buffer(buffer3);

        assert_eq!(pool.len(), 2); // 不应超过最大容量
    }

    #[test]
    fn test_shared_buffer_pool() {
        let pool = SharedBufferPool::new(2, 8);

        // 测试基本操作
        let buffer = pool.get_buffer(4);
        pool.return_buffer(buffer);
        assert_eq!(pool.len(), 1);

        // 测试多线程安全性
        let pool2 = pool.clone();
        let handle = std::thread::spawn(move || {
            let buffer = pool2.get_buffer(4);
            assert!(pool2.is_empty()); // 缓冲区已被取出
            pool2.return_buffer(buffer);
            assert_eq!(pool2.len(), 1);
        });

        handle.join().unwrap();
    }

    #[test]
    fn test_buffer_reuse() {
        let mut pool = BufferPool::new(1, 8);

        // 测试缓冲区重用
        let buffer1 = pool.get_buffer(4);
        let cap1 = buffer1.capacity();
        pool.return_buffer(buffer1);

        let buffer2 = pool.get_buffer(4);
        assert_eq!(buffer2.capacity(), cap1); // 应该获得相同的缓冲区
    }
}

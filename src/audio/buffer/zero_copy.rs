use bytes::{Bytes, BytesMut};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

/// 零拷贝缓冲区管理器
///
/// 提供高效的缓冲区管理，减少内存分配和数据复制，
/// 特别适用于流式音频处理场景。通过引用计数和智能指针实现
/// 零拷贝数据传输，显著提升性能。
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::ZeroCopyBuffer;
///
/// let mut buffer = ZeroCopyBuffer::new(4096);
/// buffer.write(b"audio data");
/// if let Some(data) = buffer.read(1024) {
///     // 处理音频数据...
/// }
/// ```
pub struct ZeroCopyBuffer {
    /// 内部缓冲区
    buffer: BytesMut,
    /// 当前读取位置
    read_pos: usize,
    /// 当前写入位置
    write_pos: usize,
    /// 默认容量
    default_capacity: usize,
}

impl ZeroCopyBuffer {
    /// 创建新的零拷贝缓冲区
    ///
    /// # 参数
    /// * `capacity` - 初始缓冲区容量（字节）
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
            read_pos: 0,
            write_pos: 0,
            default_capacity: capacity,
        }
    }

    /// 写入数据到缓冲区
    ///
    /// 将数据写入缓冲区，如果空间不足会自动扩容。
    ///
    /// # 参数
    /// * `data` - 要写入的字节数据
    ///
    /// # 返回值
    /// 返回实际写入的字节数
    pub fn write(&mut self, data: &[u8]) -> usize {
        // 确保有足够的空间
        self.ensure_capacity(data.len());

        // 写入数据
        self.buffer.extend_from_slice(data);
        let written = data.len();
        self.write_pos += written;

        written
    }

    /// 从缓冲区读取数据
    ///
    /// 读取指定长度的数据，如果数据不足则返回None。
    /// 读取后数据会从缓冲区中移除。
    ///
    /// # 参数
    /// * `len` - 要读取的字节数
    ///
    /// # 返回值
    /// 返回读取的数据，如果没有足够数据则返回None
    pub fn read(&mut self, len: usize) -> Option<Bytes> {
        let available = self.available_data();
        if available == 0 {
            return None;
        }

        let read_len = std::cmp::min(len, available);
        if read_len == 0 {
            return None;
        }

        // 使用零拷贝方式获取数据片段
        let data = self.buffer.split_to(read_len).freeze();
        self.read_pos += read_len;

        // 如果已经读完所有数据，重置位置
        if self.read_pos == self.write_pos {
            self.read_pos = 0;
            self.write_pos = 0;
        }

        Some(data)
    }

    /// 查看缓冲区中的数据但不消费
    pub fn peek(&self, len: usize) -> Option<Bytes> {
        let available = self.available_data();
        if available == 0 {
            return None;
        }

        let peek_len = std::cmp::min(len, available);
        if peek_len == 0 {
            return None;
        }

        // 创建一个数据的视图而不移动读取位置
        Some(self.buffer.slice(0, peek_len).freeze())
    }

    /// 确保缓冲区有足够的容量
    fn ensure_capacity(&mut self, additional: usize) {
        let required = self.write_pos + additional;
        if self.buffer.capacity() < required {
            // 分配更大的缓冲区
            let new_capacity =
                std::cmp::max(self.buffer.capacity() * 2, required + self.default_capacity);

            // 创建新的缓冲区并复制数据
            let mut new_buffer = BytesMut::with_capacity(new_capacity);
            new_buffer.extend_from_slice(&self.buffer);
            self.buffer = new_buffer;
        }
    }

    /// 获取可用数据量
    pub fn available_data(&self) -> usize {
        self.write_pos - self.read_pos
    }

    /// 获取剩余容量
    pub fn remaining_capacity(&self) -> usize {
        self.buffer.capacity() - self.write_pos
    }

    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.read_pos = 0;
        self.write_pos = 0;
    }

    /// 压缩缓冲区，移除已读取的数据
    pub fn compact(&mut self) {
        if self.read_pos > 0 {
            // 移动未读取的数据到缓冲区开始位置
            if self.read_pos < self.write_pos {
                let remaining = self.write_pos - self.read_pos;
                self.buffer.copy_within(self.read_pos..self.write_pos, 0);
                self.buffer.truncate(remaining);
            } else {
                self.buffer.clear();
            }

            // 重置位置
            self.write_pos -= self.read_pos;
            self.read_pos = 0;
        }
    }
}

/// 共享的零拷贝缓冲区，线程安全
pub struct SharedZeroCopyBuffer {
    /// 内部缓冲区
    inner: Arc<Mutex<ZeroCopyBuffer>>,
}

impl SharedZeroCopyBuffer {
    /// 创建新的共享零拷贝缓冲区
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ZeroCopyBuffer::new(capacity))),
        }
    }

    /// 写入数据到缓冲区
    pub fn write(&self, data: &[u8]) -> usize {
        let mut buffer = self.inner.lock();
        buffer.write(data)
    }

    /// 从缓冲区读取数据
    pub fn read(&self, len: usize) -> Option<Bytes> {
        let mut buffer = self.inner.lock();
        buffer.read(len)
    }

    /// 查看缓冲区中的数据但不消费
    pub fn peek(&self, len: usize) -> Option<Bytes> {
        let buffer = self.inner.lock();
        buffer.peek(len)
    }

    /// 获取可用数据量
    pub fn available_data(&self) -> usize {
        let buffer = self.inner.lock();
        buffer.available_data()
    }

    /// 获取剩余容量
    pub fn remaining_capacity(&self) -> usize {
        let buffer = self.inner.lock();
        buffer.remaining_capacity()
    }

    /// 清空缓冲区
    pub fn clear(&self) {
        let mut buffer = self.inner.lock();
        buffer.clear();
    }

    /// 压缩缓冲区
    pub fn compact(&self) {
        let mut buffer = self.inner.lock();
        buffer.compact();
    }

    /// 创建缓冲区的克隆
    pub fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// 零拷贝缓冲区池
///
/// 提供高效的零拷贝缓冲区管理，通过预分配和重用缓冲区来减少内存分配开销。
/// 主要特点：
/// - 自动扩缩容：根据需求动态调整缓冲区大小
/// - 内存复用：重用已分配的缓冲区，减少GC压力
/// - 容量控制：限制池中缓冲区数量，防止内存泄漏
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::ZeroCopyBufferPool;
///
/// let mut pool = ZeroCopyBufferPool::new(32, 4096); // 最多32个缓冲区，默认4KB
///
/// // 获取缓冲区
/// let buffer = pool.get_buffer(2048);
/// // 使用缓冲区...
///
/// // 归还缓冲区到池中
/// pool.return_buffer(buffer);
/// ```
pub struct ZeroCopyBufferPool {
    /// 可用的缓冲区列表
    buffers: VecDeque<ZeroCopyBuffer>,
    /// 池的最大容量
    max_size: usize,
    /// 默认缓冲区容量
    default_capacity: usize,
}

impl ZeroCopyBufferPool {
    /// 创建新的零拷贝缓冲区池
    pub fn new(max_size: usize, default_capacity: usize) -> Self {
        Self {
            buffers: VecDeque::with_capacity(max_size),
            max_size,
            default_capacity,
        }
    }

    /// 从池中获取缓冲区
    pub fn get_buffer(&mut self, min_capacity: usize) -> ZeroCopyBuffer {
        // 尝试从池中获取合适大小的缓冲区
        for i in 0..self.buffers.len() {
            if self.buffers[i].remaining_capacity() >= min_capacity {
                return self.buffers.remove(i).unwrap();
            }
        }

        // 如果没有合适的缓冲区，创建一个新的
        let capacity = std::cmp::max(min_capacity, self.default_capacity);
        ZeroCopyBuffer::new(capacity)
    }

    /// 将缓冲区返回池中
    pub fn return_buffer(&mut self, mut buffer: ZeroCopyBuffer) {
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

/// 共享的零拷贝缓冲区池，线程安全
///
/// 提供线程安全的零拷贝缓冲区池访问，适用于多线程音频处理场景。
/// 主要特点：
/// - 线程安全：使用互斥锁保护内部状态
/// - 高效共享：通过Arc实现高效的跨线程共享
/// - 零拷贝：继承底层ZeroCopyBufferPool的零拷贝特性
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::SharedZeroCopyBufferPool;
/// use std::sync::Arc;
/// use std::thread;
///
/// let pool = SharedZeroCopyBufferPool::new(32, 4096);
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
pub struct SharedZeroCopyBufferPool {
    /// 内部缓冲区池
    inner: Arc<Mutex<ZeroCopyBufferPool>>,
}

impl SharedZeroCopyBufferPool {
    /// 创建新的共享零拷贝缓冲区池
    pub fn new(max_size: usize, default_capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ZeroCopyBufferPool::new(
                max_size,
                default_capacity,
            ))),
        }
    }

    /// 从池中获取缓冲区
    pub fn get_buffer(&self, min_capacity: usize) -> ZeroCopyBuffer {
        let mut pool = self.inner.lock();
        pool.get_buffer(min_capacity)
    }

    /// 将缓冲区返回池中
    pub fn return_buffer(&self, buffer: ZeroCopyBuffer) {
        let mut pool = self.inner.lock();
        pool.return_buffer(buffer);
    }

    /// 清空缓冲区池
    pub fn clear(&self) {
        let mut pool = self.inner.lock();
        pool.clear();
    }

    /// 获取当前池中缓冲区数量
    pub fn len(&self) -> usize {
        let pool = self.inner.lock();
        pool.len()
    }

    /// 检查池是否为空
    pub fn is_empty(&self) -> bool {
        let pool = self.inner.lock();
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
    fn test_zero_copy_buffer_basic_operations() {
        let mut buffer = ZeroCopyBuffer::new(8);

        // 测试写入操作
        let written = buffer.write(b"hello");
        assert_eq!(written, 5);
        assert_eq!(buffer.available_data(), 5);

        // 测试读取操作
        let data = buffer.read(3).unwrap();
        assert_eq!(&data[..], b"hel");
        assert_eq!(buffer.available_data(), 2);

        // 测试peek操作
        let peeked = buffer.peek(2).unwrap();
        assert_eq!(&peeked[..], b"lo");
        assert_eq!(buffer.available_data(), 2); // peek不应改变可用数据量
    }

    #[test]
    fn test_zero_copy_buffer_capacity_management() {
        let mut buffer = ZeroCopyBuffer::new(4);

        // 测试自动扩容
        buffer.write(b"hello world");
        assert!(buffer.remaining_capacity() >= 11);

        // 测试压缩操作
        buffer.read(6).unwrap(); // 读取"hello "
        buffer.compact();
        assert_eq!(buffer.available_data(), 5); // 剩余"world"
    }

    #[test]
    fn test_shared_zero_copy_buffer() {
        let buffer = SharedZeroCopyBuffer::new(8);

        // 测试并发写入和读取
        buffer.write(b"test");
        assert_eq!(buffer.available_data(), 4);

        let data = buffer.read(2).unwrap();
        assert_eq!(&data[..], b"te");

        // 测试克隆和共享
        let buffer2 = buffer.clone();
        buffer2.write(b"shared");
        assert_eq!(buffer.available_data(), 8); // 2(剩余) + 6(新写入)
    }

    #[test]
    fn test_zero_copy_buffer_pool() {
        let mut pool = ZeroCopyBufferPool::new(2, 8);

        // 测试获取和返回缓冲区
        let buffer1 = pool.get_buffer(4);
        let buffer2 = pool.get_buffer(4);
        assert_eq!(pool.len(), 0);

        pool.return_buffer(buffer1);
        assert_eq!(pool.len(), 1);

        // 测试池容量限制
        let buffer3 = pool.get_buffer(4);
        pool.return_buffer(buffer2);
        pool.return_buffer(buffer3);
        assert_eq!(pool.len(), 2); // 不应超过最大容量
    }

    #[test]
    fn test_shared_zero_copy_buffer_pool() {
        let pool = SharedZeroCopyBufferPool::new(2, 8);

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
}

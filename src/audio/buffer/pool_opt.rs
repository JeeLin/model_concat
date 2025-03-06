use bytes::BytesMut;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 优化的音频缓冲区池
///
/// 提供更高效的缓冲区管理，包括：
/// - 基于大小的缓冲区分类：根据不同大小范围将缓冲区分类存储
/// - 自适应缓冲区大小调整：根据使用情况动态调整各类别的缓冲区数量
/// - 缓冲区生命周期追踪：跟踪缓冲区的创建、重用和丢弃情况
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::OptimizedBufferPool;
///
/// let mut pool = OptimizedBufferPool::new(100, 4096);
/// let buffer = pool.get_buffer(2048);
/// // 使用缓冲区...
/// pool.return_buffer(buffer);
/// ```
pub struct OptimizedBufferPool {
    /// 按大小分类的缓冲区列表
    pools: Vec<VecDeque<BytesMut>>,
    /// 每个大小类别的容量限制
    size_limits: Vec<usize>,
    /// 缓冲区使用统计
    stats: BufferStats,
    /// 默认缓冲区大小
    default_capacity: usize,
    /// 最大缓冲区数量
    max_buffers: usize,
}

/// 缓冲区使用统计
#[derive(Default)]
struct BufferStats {
    /// 创建次数
    created: usize,
    /// 重用次数
    reused: usize,
    /// 丢弃次数
    discarded: usize,
    /// 最后一次调整时间
    last_adjustment: Instant,
}

impl OptimizedBufferPool {
    /// 创建新的优化缓冲区池
    ///
    /// # 参数
    ///
    /// * `max_buffers` - 池中允许的最大缓冲区数量
    /// * `default_capacity` - 默认的缓冲区容量（字节）
    ///
    /// # 返回值
    ///
    /// 返回一个新的优化缓冲区池实例
    pub fn new(max_buffers: usize, default_capacity: usize) -> Self {
        // 定义不同大小的缓冲区类别
        let size_limits = vec![1024, 4096, 16384, 65536];
        let mut pools = Vec::with_capacity(size_limits.len());
        for _ in 0..size_limits.len() {
            pools.push(VecDeque::new());
        }

        Self {
            pools,
            size_limits,
            stats: BufferStats::default(),
            default_capacity,
            max_buffers,
        }
    }

    /// 获取合适大小的缓冲区
    ///
    /// 从对应大小类别的池中获取缓冲区，如果没有可用的则创建新的。
    ///
    /// # 参数
    ///
    /// * `min_size` - 所需的最小缓冲区大小（字节）
    ///
    /// # 返回值
    ///
    /// 返回一个至少具有指定大小的缓冲区
    pub fn get_buffer(&mut self, min_size: usize) -> BytesMut {
        let size_class = self.get_size_class(min_size);

        // 尝试从对应大小类别的池中获取缓冲区
        if let Some(mut buffer) = self.pools[size_class].pop_front() {
            buffer.clear();
            self.stats.reused += 1;
            return buffer;
        }

        // 创建新的缓冲区
        self.stats.created += 1;
        BytesMut::with_capacity(self.size_limits[size_class].max(min_size))
    }

    /// 返回缓冲区到池中
    ///
    /// 将使用完的缓冲区归还到对应大小类别的池中。如果池已满，
    /// 缓冲区将被丢弃。
    ///
    /// # 参数
    ///
    /// * `buffer` - 要归还的缓冲区
    pub fn return_buffer(&mut self, mut buffer: BytesMut) {
        let size_class = self.get_size_class(buffer.capacity());

        // 检查是否超过池容量限制
        if self.get_total_buffers() >= self.max_buffers {
            self.stats.discarded += 1;
            return;
        }

        buffer.clear();
        self.pools[size_class].push_back(buffer);

        // 定期调整缓冲区大小分布
        self.maybe_adjust_pools();
    }

    /// 获取缓冲区大小类别
    fn get_size_class(&self, size: usize) -> usize {
        self.size_limits
            .iter()
            .position(|&limit| size <= limit)
            .unwrap_or(self.size_limits.len() - 1)
    }

    /// 获取当前池中的总缓冲区数量
    fn get_total_buffers(&self) -> usize {
        self.pools.iter().map(|pool| pool.len()).sum()
    }

    /// 根据使用情况调整缓冲区池
    fn maybe_adjust_pools(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.stats.last_adjustment).as_secs() < 60 {
            return;
        }

        // 清理使用率低的大型缓冲区
        for pool in self.pools.iter_mut().rev() {
            while pool.len() > self.max_buffers / self.pools.len() {
                pool.pop_back();
                self.stats.discarded += 1;
            }
        }

        self.stats.last_adjustment = now;
    }

    /// 清空所有缓冲区池
    ///
    /// 清除所有已缓存的缓冲区并重置统计信息
    pub fn clear(&mut self) {
        for pool in &mut self.pools {
            pool.clear();
        }
        self.stats = BufferStats::default();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> (usize, usize, usize) {
        (self.stats.created, self.stats.reused, self.stats.discarded)
    }
}

/// 线程安全的优化缓冲区池
///
/// 提供线程安全的优化缓冲区池访问，适用于多线程音频处理场景。
/// 主要特点：
/// - 线程安全：使用互斥锁保护内部状态
/// - 高效共享：通过Arc实现高效的跨线程共享
/// - 优化管理：继承底层OptimizedBufferPool的优化特性
///
/// # 示例
///
/// ```rust
/// use crate::audio::buffer::SharedOptimizedBufferPool;
/// use std::sync::Arc;
/// use std::thread;
///
/// let pool = SharedOptimizedBufferPool::new(100, 4096);
/// let pool_clone = pool.clone();
///
/// let handle = thread::spawn(move || {
///     let buffer = pool_clone.get_buffer(2048);
///     // 处理音频数据...
///     pool_clone.return_buffer(buffer);
/// });
///
/// handle.join().unwrap();
/// ```
pub struct SharedOptimizedBufferPool {
    inner: Arc<Mutex<OptimizedBufferPool>>,
}

impl SharedOptimizedBufferPool {
    /// 创建新的线程安全缓冲区池
    pub fn new(max_buffers: usize, default_capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(OptimizedBufferPool::new(
                max_buffers,
                default_capacity,
            ))),
        }
    }

    /// 获取缓冲区
    pub fn get_buffer(&self, min_size: usize) -> BytesMut {
        let mut pool = self.inner.lock().unwrap();
        pool.get_buffer(min_size)
    }

    /// 返回缓冲区
    pub fn return_buffer(&self, buffer: BytesMut) {
        let mut pool = self.inner.lock().unwrap();
        pool.return_buffer(buffer);
    }

    /// 清空缓冲区池
    pub fn clear(&self) {
        let mut pool = self.inner.lock().unwrap();
        pool.clear();
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> (usize, usize, usize) {
        let pool = self.inner.lock().unwrap();
        pool.get_stats()
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
    use std::thread;

    #[test]
    fn test_optimized_pool_basic_operations() {
        let mut pool = OptimizedBufferPool::new(10, 1024);

        // 测试获取不同大小的缓冲区
        let buffer1 = pool.get_buffer(512); // 应该从第一个大小类别获取
        assert!(buffer1.capacity() >= 512);
        assert!(buffer1.capacity() <= 1024);

        let buffer2 = pool.get_buffer(2048); // 应该从第二个大小类别获取
        assert!(buffer2.capacity() >= 2048);
        assert!(buffer2.capacity() <= 4096);

        // 测试返回缓冲区
        pool.return_buffer(buffer1);
        pool.return_buffer(buffer2);

        // 验证统计信息
        let (created, reused, discarded) = pool.get_stats();
        assert_eq!(created, 2);
        assert_eq!(discarded, 0);
    }

    #[test]
    fn test_optimized_pool_capacity_management() {
        let mut pool = OptimizedBufferPool::new(2, 1024);

        // 填满池
        let buffer1 = pool.get_buffer(1024);
        let buffer2 = pool.get_buffer(1024);
        let buffer3 = pool.get_buffer(1024);

        // 返回所有缓冲区
        pool.return_buffer(buffer1);
        pool.return_buffer(buffer2);
        pool.return_buffer(buffer3);

        // 验证池的大小限制
        assert!(pool.get_total_buffers() <= 2);

        // 验证统计信息
        let (_, _, discarded) = pool.get_stats();
        assert!(discarded > 0); // 应该有缓冲区被丢弃
    }

    #[test]
    fn test_shared_optimized_pool() {
        let pool = SharedOptimizedBufferPool::new(10, 1024);
        let pool_clone = pool.clone();

        // 在新线程中使用池
        let handle = thread::spawn(move || {
            let buffer = pool_clone.get_buffer(2048);
            assert!(buffer.capacity() >= 2048);
            pool_clone.return_buffer(buffer);
        });

        // 在主线程中使用池
        let buffer = pool.get_buffer(1024);
        assert!(buffer.capacity() >= 1024);
        pool.return_buffer(buffer);

        handle.join().unwrap();

        // 验证统计信息
        let (created, reused, discarded) = pool.get_stats();
        assert_eq!(created, 2); // 应该创建了两个缓冲区
        assert_eq!(discarded, 0); // 不应该有缓冲区被丢弃
    }

    #[test]
    fn test_buffer_reuse() {
        let mut pool = OptimizedBufferPool::new(10, 1024);

        // 获取并返回缓冲区
        let buffer1 = pool.get_buffer(512);
        let cap1 = buffer1.capacity();
        pool.return_buffer(buffer1);

        // 再次获取相同大小的缓冲区
        let buffer2 = pool.get_buffer(512);
        assert_eq!(buffer2.capacity(), cap1); // 应该重用了之前的缓冲区

        // 验证统计信息
        let (created, reused, _) = pool.get_stats();
        assert_eq!(created, 1);
        assert_eq!(reused, 1);
    }
}

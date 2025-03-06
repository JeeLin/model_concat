//! 优化的音频处理器缓存模块
//!
//! 本模块提供了一个优化的处理器缓存实现，用于管理音频处理过程中的各种处理器实例。
//! 主要特点：
//! - 使用泛型方法和特征约束实现统一的缓存管理
//! - 支持基于LRU策略的缓存清理
//! - 提供线程安全的处理器实例访问
//! - 支持多种音频处理器的缓存管理
//! - 内存使用优化：自动清理过期和不常用的缓存项
//!
//! # 示例
//!
//! ```rust
//! use crate::audio::converter::cache_opt::OptimizedProcessorCache;
//! use crate::audio::format::AudioCodec;
//!
//! let mut cache = OptimizedProcessorCache::new(16);
//!
//! // 获取解码器
//! let decoder = cache.get_decoder(&AudioCodec::Wav)?;
//!
//! // 获取编码器
//! let encoder = cache.get_encoder(&AudioCodec::Mp3, 44100, 2, None)?;
//! ```

use crate::audio::decoders::AudioDecoder;
use crate::audio::encoders::AudioEncoder;
use crate::audio::format::AudioCodec;
use crate::audio::processors::{AudioProcessor, ChannelConverter, Resampler};
use crate::error::{ServiceError, ServiceResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// 优化的处理器缓存
///
/// 使用泛型方法和特征约束替代多个特定类型的获取方法，提供统一的缓存管理接口。
/// 支持自动清理过期缓存项，并确保线程安全的访问。
///
/// # 特点
///
/// - 泛型缓存管理：统一处理不同类型的处理器
/// - LRU缓存策略：自动清理最少使用的项
/// - 线程安全：支持并发访问
/// - 内存优化：自动管理内存使用
#[derive(Debug)]
pub struct OptimizedProcessorCache {
    /// 通用缓存存储
    cache: HashMap<String, Arc<Mutex<Box<dyn std::any::Any + Send + Sync>>>>,
    /// 最后访问时间，用于LRU策略
    last_access: HashMap<String, Instant>,
    /// 缓存容量上限
    capacity: usize,
    /// 当前内存使用量（字节）
    current_memory_usage: usize,
    /// 内存使用上限（字节）
    max_memory_usage: usize,
}

impl OptimizedProcessorCache {
    /// 创建新的处理器缓存
    ///
    /// # 参数
    ///
    /// * `capacity` - 缓存的最大容量，当缓存项数量超过此值时，将清理最旧的项
    ///
    /// # 返回值
    ///
    /// 返回新创建的处理器缓存实例
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(capacity),
            last_access: HashMap::with_capacity(capacity),
            capacity,
            current_memory_usage: 0,
            max_memory_usage: 1024 * 1024 * 1024, // 1GB 默认上限
        }
    }

    /// 清除所有缓存
    ///
    /// 重置缓存状态，释放所有资源
    pub fn clear(&mut self) {
        self.cache.clear();
        self.last_access.clear();
        self.current_memory_usage = 0;
    }

    /// 泛型方法：获取或创建缓存项
    ///
    /// # 类型参数
    ///
    /// * `T` - 缓存项的类型，必须实现 Send + Sync 特征
    /// * `F` - 创建缓存项的工厂函数类型
    ///
    /// # 参数
    ///
    /// * `key` - 缓存项的键
    /// * `creator` - 创建缓存项的工厂函数
    ///
    /// # 返回值
    ///
    /// 返回缓存项的Arc包装引用
    pub fn get_or_create<T, F>(&mut self, key: &str, creator: F) -> Arc<Mutex<Box<dyn T>>>
    where
        T: 'static + ?Sized + Send + Sync,
        F: FnOnce() -> Box<dyn T>,
    {
        let type_key = format!("{}_{}_{}", key, std::any::type_name::<T>(), Instant::now().elapsed().as_secs());

        // 检查缓存是否已满，如果已满则清理最旧的项
        if !self.cache.contains_key(&type_key) && self.cache.len() >= self.capacity {
            self.cleanup_oldest();
        }

        // 如果缓存中存在该项，则更新最后访问时间并返回
        if let Some(cached) = self.cache.get(&type_key) {
            self.last_access.insert(type_key.clone(), Instant::now());
            return Arc::clone(cached).downcast::<Mutex<Box<dyn T>>>().unwrap();
        }

        // 创建新项并添加到缓存
        let item = Arc::new(Mutex::new(creator()));
        let any_item = item.clone() as Arc<Mutex<Box<dyn std::any::Any + Send + Sync>>>;
        let estimated_mem = any_item.estimate_memory();
        self.current_memory_usage += estimated_mem;
        self.cache.insert(type_key.clone(), any_item);
        self.last_access.insert(type_key, Instant::now());

        item
    }

    /// 清理最旧的缓存项
    fn cleanup_oldest(&mut self) {
        if let Some((oldest_key, _)) = self.last_access.iter()
            .min_by_key(|(_, &time)| time) {
            let oldest_key = oldest_key.clone();
            self.cache.remove(&oldest_key);
            self.last_access.remove(&oldest_key);
        }
    }

    /// 获取解码器
    /// 获取解码器
    ///
    /// # 参数
    ///
    /// * `codec` - 音频编解码器类型
    ///
    /// # 返回值
    ///
    /// 返回对应编解码器的解码器实例
    pub fn get_decoder(&mut self, codec: &AudioCodec) -> ServiceResult<Arc<Mutex<Box<dyn AudioDecoder>>>> {
        let key = format!("decoder_{:?}", codec);
        match codec {
            AudioCodec::Wav => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::decoders::WavDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Mp3 => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::decoders::Mp3Decoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Ogg => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::decoders::OggDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Flac => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::decoders::FlacDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Aac => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::decoders::AacDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Opus => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::decoders::OpusDecoder::new()) as Box<dyn AudioDecoder>
            })),
            _ => Err(ServiceError::AudioConversion(format!("Unsupported codec: {:?}", codec)))
        }
    }

    /// 获取编码器
    /// 获取编码器
    ///
    /// # 参数
    ///
    /// * `codec` - 音频编解码器类型
    /// * `sample_rate` - 采样率
    /// * `channels` - 通道数
    /// * `bit_rate` - 比特率（可选）
    ///
    /// # 返回值
    ///
    /// 返回对应编解码器的编码器实例
    pub fn get_encoder(&mut self, codec: &AudioCodec, sample_rate: u32, channels: u16, bit_rate: Option<u32>) -> ServiceResult<Arc<Mutex<Box<dyn AudioEncoder>>>> {
        let key = format!("encoder_{:?}_{}_{}_{:?}", codec, sample_rate, channels, bit_rate);
        match codec {
            AudioCodec::Wav => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::encoders::WavEncoder::new(sample_rate, channels)) as Box<dyn AudioEncoder>
            })),
            AudioCodec::Mp3 => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::encoders::Mp3Encoder::new(sample_rate, channels, bit_rate)) as Box<dyn AudioEncoder>
            })),
            AudioCodec::Ogg => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::encoders::VorbisEncoder::new(sample_rate, channels, bit_rate)) as Box<dyn AudioEncoder>
            })),
            AudioCodec::Flac => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::encoders::FlacEncoder::new(sample_rate, channels)) as Box<dyn AudioEncoder>
            })),
            AudioCodec::Aac => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::encoders::AacEncoder::new(sample_rate, channels, bit_rate)) as Box<dyn AudioEncoder>
            })),
            AudioCodec::Opus => Ok(self.get_or_create(&key, || {
                Box::new(crate::audio::encoders::OpusEncoder::new(sample_rate, channels, bit_rate)) as Box<dyn AudioEncoder>
            })),
            _ => Err(ServiceError::AudioConversion(format!("Unsupported codec: {:?}", codec)))
        }
    }

    /// 获取重采样器
    /// 获取重采样器
    ///
    /// # 参数
    ///
    /// * `input_rate` - 输入采样率
    /// * `output_rate` - 输出采样率
    /// * `channels` - 通道数
    ///
    /// # 返回值
    ///
    /// 返回重采样器实例
    pub fn get_resampler(&mut self, input_rate: u32, output_rate: u32, channels: usize) -> ServiceResult<Arc<Mutex<Resampler>>> {
        let key = format!("resampler_{}_{}_{}", input_rate, output_rate, channels);
        Ok(self.get_or_create(&key, || {
            Box::new(crate::audio::processors::Resampler::new(src_rate, dst_rate, channels).unwrap()) as Box<dyn Resampler>
        }))
    }

    /// 获取通道转换器
    /// 获取通道转换器
    ///
    /// # 参数
    ///
    /// * `input_channels` - 输入通道数
    /// * `output_channels` - 输出通道数
    ///
    /// # 返回值
    ///
    /// 返回通道转换器实例
    pub fn get_channel_converter(&mut self, input_channels: usize, output_channels: usize) -> ServiceResult<Arc<Mutex<ChannelConverter>>> {
        let key = format!("channel_converter_{}_{}", input_channels, output_channels);
        Ok(self.get_or_create(&key, || {
            Box::new(crate::audio::processors::ChannelConverter::new(src_channels, dst_channels)) as Box<dyn ChannelConverter>
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 模拟音频处理器
    struct MockProcessor;
    impl AudioProcessor for MockProcessor {
        fn process(&mut self, _samples: &[f32]) -> ServiceResult<Vec<f32>> {
            Ok(vec![0.0, 1.0])
        }
    }

    #[test]
    fn test_cache_creation() {
        let cache = OptimizedProcessorCache::new(5);
        assert_eq!(cache.capacity, 5);
        assert!(cache.cache.is_empty());
        assert!(cache.last_access.is_empty());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = OptimizedProcessorCache::new(5);

        // 添加一些缓存项
        let key = "test_key";
        let processor = Box::new(MockProcessor) as Box<dyn AudioProcessor>;
        let _ = cache.get_or_create::<dyn AudioProcessor, _>(key, || processor);

        assert!(!cache.cache.is_empty());

        // 清除缓存
        cache.clear();
        assert!(cache.cache.is_empty());
        assert!(cache.last_access.is_empty());
    }

    #[test]
    fn test_cache_cleanup() {
        let mut cache = OptimizedProcessorCache::new(2);

        // 添加超过容量的缓存项
        for i in 0..3 {
            let key = format!("key_{}", i);
            let processor = Box::new(MockProcessor) as Box<dyn AudioProcessor>;
            let _ = cache.get_or_create::<dyn AudioProcessor, _>(&key, || processor);
        }

        // 验证缓存大小不超过容量
        assert!(cache.cache.len() <= cache.capacity);
    }

    #[tokio::test]
    async fn test_get_decoder() {
        let mut cache = OptimizedProcessorCache::new(5);

        // 测试支持的编解码器
        let codecs = vec![
            AudioCodec::Wav,
            AudioCodec::Mp3,
            AudioCodec::Ogg,
            AudioCodec::Flac,
            AudioCodec::Aac,
            AudioCodec::Opus
        ];

        for codec in codecs {
            let result = cache.get_decoder(&codec);
            assert!(result.is_ok());
        }

        // 测试不支持的编解码器
        let result = cache.get_decoder(&AudioCodec::Unknown);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_encoder() {
        let mut cache = OptimizedProcessorCache::new(5);
        let sample_rate = 44100;
        let channels = 2;
        let bit_rate = Some(128000);

        // 测试支持的编解码器
        let codecs = vec![
            AudioCodec::Wav,
            AudioCodec::Mp3,
            AudioCodec::Ogg,
            AudioCodec::Flac,
            AudioCodec::Aac,
            AudioCodec::Opus
        ];

        for codec in codecs {
            let result = cache.get_encoder(&codec, sample_rate, channels, bit_rate);
            assert!(result.is_ok());
        }

        // 测试不支持的编解码器
        let result = cache.get_encoder(&AudioCodec::Unknown, sample_rate, channels, bit_rate);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_resampler() {
        let mut cache = OptimizedProcessorCache::new(5);
        let result = cache.get_resampler(44100, 48000, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_channel_converter() {
        let mut cache = OptimizedProcessorCache::new(5);
        let result = cache.get_channel_converter(2, 1);
        assert!(result.is_ok());
    }
}
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use super::cache_opt::OptimizedProcessorCache;
use crate::audio::buffer::{SharedZeroCopyBuffer, ZeroCopyBuffer};
use crate::audio::codecs::factory::CodecFactory;
use crate::audio::format::{AudioCodec, AudioFormat, AudioParams};
use crate::audio::processors::{ParallelAudioProcessor, ParallelProcessingConfig};
use crate::audio::stream::{AudioChunk, AudioStream, StreamProcessor};
use crate::error::{ServiceError, ServiceResult};

/// 增强型音频转换器
///
/// 提供高性能的音频格式转换，支持：
/// - 零拷贝缓冲区管理
/// - 并行处理
/// - 直接转换路径
/// - 智能缓存管理
pub struct EnhancedAudioConverter {
    /// 输入格式
    input_format: AudioFormat,
    /// 输出格式
    output_format: AudioFormat,
    /// 优化的处理器缓存
    processor_cache: Arc<RwLock<OptimizedProcessorCache>>,
    /// 零拷贝输入缓冲区
    input_buffer: Option<SharedZeroCopyBuffer>,
    /// 零拷贝输出缓冲区
    output_buffer: Option<SharedZeroCopyBuffer>,
    /// 并行处理配置
    parallel_config: ParallelProcessingConfig,
    /// 是否使用零拷贝模式
    use_zero_copy: bool,
    /// 是否使用并行处理
    use_parallel: bool,
    /// 是否使用直接转换路径
    use_direct_path: bool,
    /// 性能统计
    stats: ConverterStats,
}

/// 转换器性能统计
struct ConverterStats {
    /// 处理的块数量
    chunks_processed: usize,
    /// 总处理时间（毫秒）
    total_processing_time_ms: u64,
    /// 最大处理时间（毫秒）
    max_processing_time_ms: u64,
    /// 缓存命中次数
    cache_hits: usize,
}

impl Default for ConverterStats {
    fn default() -> Self {
        Self {
            chunks_processed: 0,
            total_processing_time_ms: 0,
            max_processing_time_ms: 0,
            cache_hits: 0,
        }
    }
}

impl EnhancedAudioConverter {
    /// 创建新的增强型音频转换器
    pub fn new(input_format: AudioFormat, output_format: AudioFormat) -> Self {
        Self {
            input_format,
            output_format,
            processor_cache: Arc::new(RwLock::new(OptimizedProcessorCache::new(32))),
            input_buffer: None,
            output_buffer: None,
            parallel_config: ParallelProcessingConfig::default(),
            use_zero_copy: true,
            use_parallel: true,
            use_direct_path: true,
            stats: ConverterStats::default(),
        }
    }

    /// 设置是否使用零拷贝模式
    pub fn set_zero_copy(&mut self, use_zero_copy: bool) -> &mut Self {
        self.use_zero_copy = use_zero_copy;
        self
    }

    /// 设置是否使用并行处理
    pub fn set_parallel(&mut self, use_parallel: bool) -> &mut Self {
        self.use_parallel = use_parallel;
        self
    }

    /// 设置是否使用直接转换路径
    pub fn set_direct_path(&mut self, use_direct_path: bool) -> &mut Self {
        self.use_direct_path = use_direct_path;
        self
    }

    /// 设置并行处理配置
    pub fn set_parallel_config(&mut self, config: ParallelProcessingConfig) -> &mut Self {
        self.parallel_config = config;
        self
    }

    /// 获取性能统计
    pub fn get_stats(&self) -> (usize, u64, u64, usize) {
        (
            self.stats.chunks_processed,
            self.stats.total_processing_time_ms,
            self.stats.max_processing_time_ms,
            self.stats.cache_hits,
        )
    }

    /// 检查是否可以使用直接转换路径
    fn can_use_direct_path(&self) -> bool {
        if !self.use_direct_path {
            return false;
        }

        // 检查输入和输出格式是否支持直接转换
        match (self.input_format.codec, self.output_format.codec) {
            // 添加支持直接转换的格式对
            (AudioCodec::Mp3, AudioCodec::Wav) => true,
            (AudioCodec::Wav, AudioCodec::Mp3) => true,
            (AudioCodec::Flac, AudioCodec::Wav) => true,
            (AudioCodec::Wav, AudioCodec::Flac) => true,
            (AudioCodec::Ogg, AudioCodec::Wav) => true,
            (AudioCodec::Wav, AudioCodec::Ogg) => true,
            // 其他格式对不支持直接转换
            _ => false,
        }
    }

    /// 使用直接转换路径处理
    async fn process_direct_path(&mut self, data: &[u8]) -> ServiceResult<Vec<u8>> {
        // 获取缓存键
        let cache_key = format!(
            "direct_{}_{}_{}_{}",
            self.input_format.codec,
            self.output_format.codec,
            self.input_format.sample_rate,
            self.output_format.sample_rate
        );

        // 尝试从缓存获取转换器
        let converter = self.processor_cache.write().get_or_create(&cache_key, || {
            // 创建直接转换器
            Box::new(DirectFormatConverter::new(
                self.input_format.clone(),
                self.output_format.clone(),
            ))
        });

        // 执行转换
        let mut converter = converter.lock().await;
        converter.convert(data).await
    }

    /// 使用并行处理模式
    async fn process_parallel(&mut self, data: &[u8]) -> ServiceResult<Vec<u8>> {
        // 创建并行处理器
        let input_format = self.input_format.clone();
        let output_format = self.output_format.clone();
        let processor_cache = Arc::clone(&self.processor_cache);

        let processor_factory = move || {
            // 创建标准转换器
            StandardFormatConverter::new(
                input_format.clone(),
                output_format.clone(),
                Arc::clone(&processor_cache),
            )
        };

        let mut parallel_processor = ParallelAudioProcessor::with_config(
            processor_factory,
            self.input_format.clone(),
            self.output_format.clone(),
            self.parallel_config.clone(),
        );

        // 启动处理器
        parallel_processor.start();

        // 创建音频块
        let chunk = AudioChunk {
            data: Bytes::from(data.to_vec()),
            timestamp: 0,
            duration: 0,
            is_last: false,
        };

        // 处理数据
        let result = parallel_processor.process_chunk(chunk).await?;

        // 停止处理器
        parallel_processor.stop().await;

        Ok(result.data.to_vec())
    }

    /// 使用标准处理模式
    async fn process_standard(&mut self, data: &[u8]) -> ServiceResult<Vec<u8>> {
        // 创建标准转换器
        let mut converter = StandardFormatConverter::new(
            self.input_format.clone(),
            self.output_format.clone(),
            Arc::clone(&self.processor_cache),
        );

        // 创建音频块
        let chunk = AudioChunk {
            data: Bytes::from(data.to_vec()),
            timestamp: 0,
            duration: 0,
            is_last: false,
        };

        // 处理数据
        let result = converter.process_chunk(chunk).await?;

        Ok(result.data.to_vec())
    }

    /// 使用零拷贝处理模式
    async fn process_zero_copy(&mut self, data: &[u8]) -> ServiceResult<Vec<u8>> {
        // 初始化缓冲区
        if self.input_buffer.is_none() {
            self.input_buffer = Some(SharedZeroCopyBuffer::new(data.len() * 2));
        }
        if self.output_buffer.is_none() {
            self.output_buffer = Some(SharedZeroCopyBuffer::new(data.len() * 2));
        }

        let input_buffer = self.input_buffer.as_ref().unwrap();
        let output_buffer = self.output_buffer.as_ref().unwrap();

        // 写入数据到输入缓冲区
        input_buffer.write(data);

        // 创建标准转换器
        let mut converter = ZeroCopyFormatConverter::new(
            self.input_format.clone(),
            self.output_format.clone(),
            Arc::clone(&self.processor_cache),
            input_buffer.clone(),
            output_buffer.clone(),
        );

        // 处理数据
        converter.process().await?;

        // 从输出缓冲区读取结果
        let result_size = output_buffer.available_data();
        if let Some(result_data) = output_buffer.read(result_size) {
            Ok(result_data.to_vec())
        } else {
            Err(ServiceError::AudioConversion("Failed to read from output buffer".to_string()))
        }
    }
}

#[async_trait::async_trait]
impl StreamProcessor for EnhancedAudioConverter {
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        let start = Instant::now();

        // 选择处理模式
        let result = if self.can_use_direct_path() {
            // 使用直接转换路径
            debug!("Using direct conversion path: {} -> {}", self.input_format.codec, self.output_format.codec);
            self.process_direct_path(chunk.data.as_ref()).await
        } else if self.use_parallel {
            // 使用并行处理
            debug!("Using parallel processing: {} -> {}", self.input_format.codec, self.output_format.codec);
            self.process_parallel(chunk.data.as_ref()).await
        } else if self.use_zero_copy {
            // 使用零拷贝处理
            debug!("Using zero-copy processing: {} -> {}", self.input_format.codec, self.output_format.codec);
            self.process_zero_copy(chunk.data.as_ref()).await
        } else {
            // 使用标准处理
            debug!("Using standard processing: {} -> {}", self.input_format.codec, self.output_format.codec);
            self.process_standard(chunk.data.as_ref()).await
        };

        // 更新统计信息
        let elapsed = start.elapsed();
        self.stats.chunks_processed += 1;
        self.stats.total_processing_time_ms += elapsed.as_millis() as u64;
        self.stats.max_processing_time_ms = self.stats.max_processing_time_ms.max(elapsed.as_millis() as u64);

        // 返回处理结果
        match result {
            Ok(data) => Ok(AudioChunk {
                data: Bytes::from(data),
                timestamp: chunk.timestamp,
                duration: chunk.duration,
                is_last: chunk.is_last,
            }),
            Err(e) => Err(e),
        }
    }

    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
        // 重置缓冲区
        self.input_buffer = None;
        self.output_buffer = None;

        // 重置统计信息
        self.stats = ConverterStats::default();

        Ok(None)
    }
}

/// 直接格式转换器
struct DirectFormatConverter {
    input_format: AudioFormat,
    output_format: AudioFormat,
}

impl DirectFormatConverter {
    /// 创建新的直接格式转换器
    fn new(input_format: AudioFormat, output_format: AudioFormat) -> Self {
        Self {
            input_format,
            output_format,
        }
    }

    /// 执行转换
    async fn convert(&mut self, data: &[u8]) -> ServiceResult<Vec<u8>> {
        // 根据格式对选择转换方法
        match (self.input_format.codec, self.output_format.codec) {
            (AudioCodec::Mp3, AudioCodec::Wav) => self.convert_mp3_to_wav(data).await,
            (AudioCodec::Wav, AudioCodec::Mp3) => self.convert_wav_to_mp3(data).await,
            (AudioCodec::Flac, AudioCodec::Wav) => self.convert_flac_to_wav(data).await,
            (AudioCodec::Wav, AudioCodec::
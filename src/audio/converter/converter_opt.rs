use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use super::cache_opt::OptimizedProcessorCache;
use crate::audio::buffer::{SharedZeroCopyBuffer, ZeroCopyBuffer};
use crate::audio::format::{AudioCodec, AudioFormat, AudioParams};
use crate::audio::processors::{EnhancedStreamProcessor, ParallelProcessorConfig};
use crate::audio::stream::{AudioChunk, AudioStream, StreamProcessor};
use crate::error::{ServiceError, ServiceResult};

/// 优化的音频转换器
///
/// 提供高性能的音频格式转换，支持：
/// - 零拷贝缓冲区管理：通过共享内存减少数据复制
/// - 并行处理：利用多线程提高处理效率
/// - 增量编解码：支持流式处理
/// - 统一的缓存管理：复用处理器实例
///
/// # 示例
///
/// ```rust
/// use crate::audio::converter::OptimizedAudioConverter;
/// use crate::audio::format::AudioFormat;
///
/// // 创建转换器
/// let converter = OptimizedAudioConverter::new(input_format, output_format);
///
/// // 处理音频数据
/// let result = converter.process_chunk(chunk).await?;
/// ```
pub struct OptimizedAudioConverter {
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
    parallel_config: ParallelProcessorConfig,
    /// 是否使用零拷贝模式
    use_zero_copy: bool,
}

#[async_trait::async_trait]
impl StreamProcessor for OptimizedAudioConverter {
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        let start = Instant::now();

        // 使用零拷贝模式处理
        if self.use_zero_copy {
            // 将数据写入输入缓冲区
            if self.input_buffer.is_none() {
                self.input_buffer = Some(
                    self.processor_cache
                        .write()
                        .get_zero_copy_buffer(chunk.data.len() * 2),
                );
            }

            let input_buffer = self.input_buffer.as_ref().unwrap();
            input_buffer.write(chunk.data.as_ref());

            // 解码并处理音频数据
            let processed_samples = self.decode_and_process_zero_copy().await?;

            // 编码为目标格式
            let encoded_data = self.encode_samples_zero_copy(&processed_samples).await?;

            debug!(
                "Audio chunk conversion (zero-copy): {} -> {} ({}ms)",
                self.input_format.codec,
                self.output_format.codec,
                start.elapsed().as_millis()
            );

            Ok(AudioChunk {
                data: encoded_data.unwrap_or_default(),
                timestamp: chunk.timestamp,
                duration: chunk.duration,
                is_last: chunk.is_last,
            })
        } else {
            // 传统处理方式
            // 解码并处理音频数据
            let processed_samples = self.decode_and_process(chunk.data.as_ref()).await?;

            // 编码为目标格式
            let encoded_data = self.encode_samples(&processed_samples).await?;

            debug!(
                "Audio chunk conversion: {} -> {} ({}ms)",
                self.input_format.codec,
                self.output_format.codec,
                start.elapsed().as_millis()
            );

            Ok(AudioChunk {
                data: Bytes::from(encoded_data.unwrap_or_default()),
                timestamp: chunk.timestamp,
                duration: chunk.duration,
                is_last: chunk.is_last,
            })
        }
    }

    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
        // 获取解码器
        let decoder = self.get_decoder_for_codec(&self.input_format.codec).await?;

        // 刷新解码器
        let mut decoder = decoder.lock().await;
        if let Some(pcm_samples) = decoder.flush().await? {
            // 处理最后的样本
            let processed_samples = self.process_samples(&pcm_samples).await?;

            // 编码
            if let Some(encoded_data) = self.encode_samples(&processed_samples).await? {
                return Ok(Some(AudioChunk {
                    data: Bytes::from(encoded_data),
                    timestamp: 0,
                    duration: 0,
                    is_last: true,
                }));
            }
        }

        Ok(None)
    }

    fn reset(&mut self) {
        // 重置所有缓存的处理器
        self.processor_cache.write().clear();

        // 清空缓冲区
        if let Some(buffer) = &self.input_buffer {
            buffer.clear();
        }

        if let Some(buffer) = &self.output_buffer {
            buffer.clear();
        }
    }
}

impl OptimizedAudioConverter {
    /// 创建新的优化音频转换器
    ///
    /// # 参数
    ///
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    ///
    /// # 返回值
    ///
    /// 返回配置为零拷贝模式的转换器实例
    pub fn new(input_format: AudioFormat, output_format: AudioFormat) -> Self {
        Self {
            input_format,
            output_format,
            processor_cache: Arc::new(RwLock::new(OptimizedProcessorCache::new(32))),
            input_buffer: None,
            output_buffer: None,
            parallel_config: ParallelProcessorConfig::default(),
            use_zero_copy: true,
        }
    }

    /// 使用自定义配置创建转换器
    ///
    /// # 参数
    ///
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    /// * `use_zero_copy` - 是否启用零拷贝模式
    ///
    /// # 返回值
    ///
    /// 返回根据指定配置创建的转换器实例
    pub fn with_config(
        input_format: AudioFormat,
        output_format: AudioFormat,
        use_zero_copy: bool,
    ) -> Self {
        Self {
            input_format,
            output_format,
            processor_cache: Arc::new(RwLock::new(OptimizedProcessorCache::new_for_streaming(32))),
            input_buffer: None,
            output_buffer: None,
            parallel_config: ParallelProcessorConfig::default(),
            use_zero_copy,
        }
    }

    /// 获取指定编解码器的解码器
    ///
    /// # 参数
    ///
    /// * `codec` - 音频编解码器类型
    ///
    /// # 返回值
    ///
    /// 返回对应编解码器的解码器实例
    async fn get_decoder_for_codec(
        &self,
        codec: &AudioCodec,
    ) -> ServiceResult<Arc<Mutex<Box<dyn crate::audio::decoders::AudioDecoder>>>> {
        self.processor_cache.write().get_decoder(codec)
    }

    /// 解码并处理音频数据
    ///
    /// # 参数
    ///
    /// * `data` - 待处理的音频数据
    ///
    /// # 返回值
    ///
    /// 返回处理后的PCM样本数据
    async fn decode_and_process(&self, data: &[u8]) -> ServiceResult<Vec<f32>> {
        // 获取解码器
        let decoder = self.get_decoder_for_codec(&self.input_format.codec).await?;

        // 解码为PCM样本
        let mut decoder = decoder.lock().await;
        let pcm_samples = decoder.decode_chunk(data).await?;

        // 处理样本
        self.process_samples(&pcm_samples).await
    }

    /// 使用零拷贝模式解码并处理音频数据
    ///
    /// 从共享内存缓冲区读取数据并进行处理，减少数据复制
    ///
    /// # 返回值
    ///
    /// 返回处理后的PCM样本数据
    async fn decode_and_process_zero_copy(&self) -> ServiceResult<Vec<f32>> {
        // 获取解码器
        let decoder = self.get_decoder_for_codec(&self.input_format.codec).await?;

        // 从输入缓冲区读取数据
        let input_buffer = self.input_buffer.as_ref().unwrap();
        let data = match input_buffer.read(self.parallel_config.max_chunk_size) {
            Some(data) => data,
            None => return Ok(Vec::new()), // 没有数据可处理
        };

        // 解码为PCM样本
        let mut decoder = decoder.lock().await;
        let pcm_samples = decoder.decode_chunk(&data).await?;

        // 处理样本
        self.process_samples(&pcm_samples).await
    }

    /// 处理音频样本
    ///
    /// 执行重采样和通道转换等处理
    ///
    /// # 参数
    ///
    /// * `samples` - 待处理的PCM样本数据
    ///
    /// # 返回值
    ///
    /// 返回处理后的PCM样本数据
    async fn process_samples(&self, samples: &[f32]) -> ServiceResult<Vec<f32>> {
        let mut processed_samples = samples.to_vec();

        // 如果需要重采样
        if self.input_format.sample_rate != self.output_format.sample_rate {
            let resampler_result = self.processor_cache.write().get_resampler(
                self.input_format.sample_rate,
                self.output_format.sample_rate,
                self.input_format.channels as usize,
            );

            match resampler_result {
                Ok(resampler) => {
                    processed_samples = resampler.lock().await.resample(&processed_samples)?;
                }
                Err(e) => return Err(e),
            }
        }

        // 如果需要调整通道数
        if self.input_format.channels != self.output_format.channels {
            let channel_converter_result = self.processor_cache.write().get_channel_converter(
                self.input_format.channels as usize,
                self.output_format.channels as usize,
            );

            match channel_converter_result {
                Ok(channel_converter) => {
                    processed_samples = channel_converter
                        .lock()
                        .await
                        .convert_channels(&processed_samples)?;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(processed_samples)
    }

    /// 编码音频样本为目标格式
    ///
    /// # 参数
    ///
    /// * `samples` - 待编码的PCM样本数据
    ///
    /// # 返回值
    ///
    /// 返回编码后的音频数据
    async fn encode_samples(&self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 从缓存中获取编码器
        let encoder_result = self.processor_cache.write().get_encoder(
            &self.output_format.codec,
            self.output_format.sample_rate,
            self.output_format.channels,
        );

        match encoder_result {
            Ok(encoder) => {
                // 编码样本
                let mut encoder = encoder.lock().await;
                Ok(encoder.encode_samples(samples)?)
            }
            Err(e) => Err(e),
        }
    }

    /// 使用零拷贝模式编码音频样本
    ///
    /// # 参数
    ///
    /// * `samples` - 待编码的PCM样本数据
    ///
    /// # 返回值
    ///
    /// 返回编码后的音频数据
    async fn encode_samples_zero_copy(&self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 从缓存中获取编码器
        let encoder_result = self.processor_cache.write().get_encoder(
            &self.output_format.codec,
            self.output_format.sample_rate,
            self.output_format.channels,
        );

        match encoder_result {
            Ok(encoder) => {
                // 编码样本
                let mut encoder = encoder.lock().await;
                let encoded = encoder.encode_samples(samples)?;

                // 如果有编码后的数据，写入输出缓冲区
                if let Some(data) = encoded {
                    if self.output_buffer.is_none() {
                        self.output_buffer = Some(
                            self.processor_cache
                                .write()
                                .get_zero_copy_buffer(data.len() * 2),
                        );
                    }

                    let output_buffer = self.output_buffer.as_ref().unwrap();
                    output_buffer.write(&data);

                    // 返回数据
                    Ok(Some(data))
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// 创建流式处理管道
    ///
    /// # 参数
    ///
    /// * `input_stream` - 输入音频流
    ///
    /// # 返回值
    ///
    /// 返回处理后的音频流
    pub fn create_stream_pipeline(&self, input_stream: AudioStream) -> ServiceResult<AudioStream> {
        // 创建增强型流处理器，使用当前配置
        let mut config = ParallelProcessorConfig::default();
        config.use_zero_copy = self.use_zero_copy;
        config.worker_threads = num_cpus::get().max(2); // 确保至少有2个工作线程

        let processor = EnhancedStreamProcessor::with_config(
            Self::with_config(
                self.input_format.clone(),
                self.output_format.clone(),
                self.use_zero_copy,
            ),
            self.input_format.clone(),
            self.output_format.clone(),
            config,
        );

        // 创建处理管道
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async { input_stream.create_pipeline(processor).await })
    }

    /// 异步创建流式处理管道
    ///
    /// # 参数
    ///
    /// * `input_stream` - 输入音频流
    ///
    /// # 返回值
    ///
    /// 返回处理后的音频流
    pub async fn create_stream_pipeline_async(
        &self,
        input_stream: AudioStream,
    ) -> ServiceResult<AudioStream> {
        // 创建增强型流处理器，使用当前配置
        let mut config = ParallelProcessorConfig::default();
        config.use_zero_copy = self.use_zero_copy;
        config.worker_threads = num_cpus::get().max(2); // 确保至少有2个工作线程

        let processor = EnhancedStreamProcessor::with_config(
            Self::with_config(
                self.input_format.clone(),
                self.output_format.clone(),
                self.use_zero_copy,
            ),
            self.input_format.clone(),
            self.output_format.clone(),
            config,
        );

        // 创建处理管道
        input_stream.create_pipeline(processor).await
    }

    /// 在不同音频格式之间进行转换
    ///
    /// # 参数
    ///
    /// * `data` - 待转换的音频数据
    /// * `input_format` - 输入音频格式
    /// * `params` - 转换参数
    ///
    /// # 返回值
    ///
    /// 返回转换后的音频数据和格式
    pub async fn convert_format(
        &self,
        data: &[u8],
        input_format: &AudioFormat,
        params: &AudioParams,
    ) -> ServiceResult<(Vec<u8>, AudioFormat)> {
        // 创建目标格式
        let target_format = AudioFormat {
            codec: params.codec.clone().unwrap_or(input_format.codec.clone()),
            sample_rate: params.sample_rate.unwrap_or(input_format.sample_rate),
            channels: params.channels.unwrap_or(input_format.channels),
            bit_rate: params.bit_rate.or(input_format.bit_rate),
            bits_per_sample: params.bits_per_sample.or(input_format.bits_per_sample),
        };

        // 只记录关键信息
        info!("Converting audio format");
        let start = Instant::now();

        // 创建临时转换器
        let mut converter = Self::new(input_format.clone(), target_format.clone());

        // 解码并处理音频数据
        let processed_samples = converter.decode_and_process(data).await?;

        // 编码为目标格式
        let encoded_data = converter.encode_samples(&processed_samples).await?;

        // 记录转换时间
        debug!(
            "Audio format conversion completed in {}ms",
            start.elapsed().as_millis()
        );

        // 返回结果
        match encoded_data {
            Some(data) => Ok((data.to_vec(), target_format)),
            None => Err(ServiceError::AudioConversion(
                "Failed to encode audio data".to_string(),
            )),
        }
    }
}

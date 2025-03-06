use crate::error::{ServiceError, ServiceResult};
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use linked_hash_map::LinkedHashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use super::format::{AudioCodec, AudioFormat, AudioParams};
use super::processors::AudioProcessor;
use super::stream::{AudioChunk, AudioStream, StreamProcessor};

mod cache;
mod cache_opt;
mod fmt_strategy;
mod format;

pub use cache::ProcessorCache;
pub use cache_opt::OptimizedProcessorCache;
pub use fmt_strategy::{
    FormatConversionStrategy, FormatConverterFactory, GenericConversionStrategy,
};
pub use format::SampleFormatConverter;

/// 音频转换器
pub struct AudioConverter {
    /// 输入格式
    input_format: AudioFormat,
    /// 输出格式
    output_format: AudioFormat,
    /// 处理器缓存
    processor_cache: Arc<RwLock<ProcessorCache>>,
    /// 最小缓冲区大小
    min_buffer_size: usize,
}

#[async_trait::async_trait]
impl StreamProcessor for AudioConverter {
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        let start = Instant::now();

        // 解码并处理音频数据
        let processed_samples = self.decode_and_process(chunk.data.as_ref()).await?;

        // 编码为目标格式
        let encoded_data = self.encode_samples(&processed_samples).await?;

        // 只在调试级别记录详细信息
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
    }
}

impl AudioConverter {
    pub fn new(input_format: AudioFormat, output_format: AudioFormat) -> Self {
        Self {
            input_format,
            output_format,
            processor_cache: Arc::new(RwLock::new(ProcessorCache::new(16))),
            min_buffer_size: 4096,
        }
    }

    /// 获取指定编解码器的解码器
    async fn get_decoder_for_codec(
        &self,
        codec: &AudioCodec,
    ) -> ServiceResult<Arc<Mutex<Box<dyn AudioDecoder>>>> {
        match codec {
            AudioCodec::Wav => Ok(self.processor_cache.read().get_wav_decoder()),
            AudioCodec::Mp3 => Ok(self.processor_cache.read().get_mp3_decoder()),
            AudioCodec::Ogg => Ok(self.processor_cache.read().get_ogg_decoder()),
            AudioCodec::Flac => Ok(self.processor_cache.read().get_flac_decoder()),
            AudioCodec::Aac => Ok(self.processor_cache.read().get_aac_decoder()),
            AudioCodec::Opus => Ok(self.processor_cache.read().get_opus_decoder()),
            _ => Err(ServiceError::AudioConversion(format!(
                "Unsupported codec: {:?}",
                codec
            ))),
        }
    }

    /// 解码并处理音频数据
    async fn decode_and_process(&self, data: &[u8]) -> ServiceResult<Vec<f32>> {
        // 获取解码器
        let decoder = self.get_decoder_for_codec(&self.input_format.codec).await?;

        // 解码为PCM样本
        let mut decoder = decoder.lock().await;
        let pcm_samples = decoder.decode_chunk(data).await?;

        // 处理样本
        self.process_samples(&pcm_samples).await
    }

    /// 处理音频样本（重采样、通道转换等）
    async fn process_samples(&self, samples: &[f32]) -> ServiceResult<Vec<f32>> {
        let mut processed_samples = samples.to_vec();

        // 如果需要重采样
        if self.input_format.sample_rate != self.output_format.sample_rate {
            let resampler_result = self.processor_cache.read().get_resampler(
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
            let channel_converter = self.processor_cache.read().get_channel_converter(
                self.input_format.channels as usize,
                self.output_format.channels as usize,
            );
            processed_samples = channel_converter
                .lock()
                .await
                .convert_channels(&processed_samples)?;
        }

        Ok(processed_samples)
    }

    /// 编码音频样本为目标格式
    async fn encode_samples(&self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 从缓存中获取编码器
        let encoder = self.processor_cache.read().get_encoder(
            &self.output_format.codec,
            self.output_format.sample_rate,
            self.output_format.channels,
        );

        // 编码样本
        let mut encoder = encoder.lock().await;
        Ok(encoder.encode_samples(samples)?)
    }

    /// 在不同音频格式之间进行转换
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

        // 如果输入和输出格式相同，直接返回原始数据
        if input_format.codec == target_format.codec
            && input_format.sample_rate == target_format.sample_rate
            && input_format.channels == target_format.channels
        {
            debug!("Skipping conversion (identical formats)");
            return Ok((data.to_vec(), target_format));
        }

        // 解码并处理
        let pcm_samples = self.decode_and_process(data).await?;

        // 编码
        let encoded_data = self
            .encode_samples(&pcm_samples)
            .await?
            .ok_or_else(|| ServiceError::AudioConversion("Failed to encode audio".into()))?;

        // 只记录关键信息
        debug!(
            "Audio conversion completed in {}ms",
            start.elapsed().as_millis()
        );

        Ok((encoded_data.to_vec(), target_format))
    }

    pub async fn process_stream(&self, mut stream: AudioStream) -> ServiceResult<AudioStream> {
        // 创建输出流
        let (sender, receiver) = tokio::sync::mpsc::channel(100);
        let output_stream = AudioStream::from_receiver(receiver, stream.format.clone());

        // 创建处理器的克隆
        let converter = Arc::new(Mutex::new(AudioConverter::new(
            self.input_format.clone(),
            self.output_format.clone(),
        )));

        // 使用并行处理模型
        tokio::spawn(async move {
            let mut last_chunk: Option<AudioChunk> = None;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // 保存最后一块用于flush
                        let is_last = chunk.is_last;

                        // 处理当前块
                        let converter_clone = converter.clone();
                        let sender_clone = sender.clone();

                        // 使用任务处理当前块
                        let process_result = {
                            let mut conv = converter_clone.lock().await;
                            conv.process_chunk(chunk.clone()).await
                        };

                        match process_result {
                            Ok(processed) => {
                                if let Err(e) = sender_clone.send(Ok(processed)).await {
                                    error!("Failed to send processed chunk: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                if let Err(send_err) = sender_clone.send(Err(e)).await {
                                    error!("Failed to send error: {}", send_err);
                                }
                                break;
                            }
                        }

                        // 如果是最后一块，保存它用于flush
                        if is_last {
                            last_chunk = Some(chunk);
                            break;
                        }
                    }
                    Err(e) => {
                        if let Err(send_err) = sender.send(Err(e)).await {
                            error!("Failed to send error: {}", send_err);
                        }
                        break;
                    }
                }
            }

            // 如果有最后一块，执行flush
            if last_chunk.is_some() {
                let mut conv = converter.lock().await;
                match conv.flush().await {
                    Ok(Some(final_chunk)) => {
                        if let Err(e) = sender.send(Ok(final_chunk)).await {
                            error!("Failed to send final chunk: {}", e);
                        }
                    }
                    Ok(None) => {
                        // 没有最终块，不需要处理
                    }
                    Err(e) => {
                        if let Err(send_err) = sender.send(Err(e)).await {
                            error!("Failed to send flush error: {}", send_err);
                        }
                    }
                }
            }
        });

        Ok(output_stream)
    }

    /// 创建适用于流式处理的转换器
    pub fn new_for_streaming(input_format: AudioFormat, output_format: AudioFormat) -> Self {
        let cache = ProcessorCache::new_for_streaming(8);
        let cache = Arc::new(RwLock::new(cache));

        // 设置为流式模式
        cache.write().set_streaming_mode(true);

        Self {
            input_format,
            output_format,
            processor_cache: cache,
            min_buffer_size: 4096,
        }
    }

    /// 流式处理音频数据
    pub async fn process_streaming(&self, data: &[u8], is_last: bool) -> ServiceResult<Bytes> {
        // 创建音频块
        let chunk = AudioChunk {
            data: Bytes::from(data.to_vec()),
            timestamp: 0, // 在流式处理中，时间戳通常由上层管理
            duration: 0,  // 同上
            is_last,
        };

        // 处理音频块
        let processed = self.process_chunk(chunk).await?;

        Ok(processed.data)
    }
}

impl AudioConverter {
    // 获取处理器缓存
    pub fn get_processor_cache(&self) -> Arc<RwLock<ProcessorCache>> {
        self.processor_cache.clone()
    }
}

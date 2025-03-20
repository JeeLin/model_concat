//! 音频转换器
//!
//! 提供音频格式之间的转换功能，支持一对多的格式转换。
//! 实现了流式和非流式处理接口。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use tracing::{debug, error};

use super::buffer::{BufferConfig, RingBuffer};
use super::format::{AudioCodec, AudioFormat};
use super::processors::{AudioProcessor, AudioProcessorParams};
use super::stream::{AudioChunk, AudioStream, StreamProcessor};
use crate::error::{ServiceError, ServiceResult};

/// 音频转换配置
#[derive(Debug, Clone)]
pub struct ConverterConfig {
    /// 输入格式
    pub input_format: AudioFormat,
    /// 输出格式列表
    pub output_formats: Vec<AudioFormat>,
    /// 缓冲区配置
    pub buffer_config: BufferConfig,
}

/// 音频转换器
///
/// 支持将一种音频格式转换为多种目标格式，可以同时输出多个转换结果。
pub struct AudioConverter {
    /// 转换器配置
    config: ConverterConfig,
    /// 输入缓冲区
    input_buffer: RingBuffer,
    /// 输出缓冲区映射（格式 -> 缓冲区）
    output_buffers: HashMap<AudioFormat, RingBuffer>,
    /// 格式转换处理器
    format_processor: Box<dyn AudioProcessor>,
    /// 效果处理器列表
    effect_processors: Vec<Box<dyn AudioProcessor>>,
}

impl AudioConverter {
    /// 创建新的音频转换器
    pub fn new(config: ConverterConfig, format_processor: Box<dyn AudioProcessor>) -> ServiceResult<Self> {
        let input_buffer = RingBuffer::new(config.buffer_config.clone());
        let mut output_buffers = HashMap::new();

        // 为每个输出格式创建缓冲区
        for format in &config.output_formats {
            output_buffers.insert(
                format.clone(),
                RingBuffer::new(config.buffer_config.clone()),
            );
        }

        let processor_params = AudioProcessorParams {
            sample_rate: config.input_format.sample_rate,
            channels: config.input_format.channels,
            bits_per_sample: config.input_format.bits_per_sample,
        };

        // 创建效果处理器列表
        let mut effect_processors = Vec::new();
        let process_config = &config.input_format.process_config;

        // 添加音量处理器
        if process_config.volume != 1.0 || process_config.normalize_volume {
            effect_processors.push(Box::new(super::processors::volume::VolumeProcessor::new(
                process_config.volume,
                process_config.normalize_volume,
                processor_params.clone(),
            )));
        }

        // 添加通道处理器
        if process_config.merge_channels {
            effect_processors.push(Box::new(super::processors::channel::ChannelProcessor::new(
                processor_params.clone(),
                true,
            )));
        }

        Ok(Self {
            config,
            input_buffer,
            output_buffers,
            format_processor,
            effect_processors,
        })
    }

    /// 添加效果处理器
    pub fn add_effect_processor(&mut self, processor: Box<dyn AudioProcessor>) {
        self.effect_processors.push(processor);
    }

    /// 处理音频数据
    pub async fn process(&mut self, data: Bytes) -> ServiceResult<HashMap<AudioFormat, Bytes>> {
        // 写入输入缓冲区
        let written = self.input_buffer.write(&data);
        if written < data.len() {
            error!("输入缓冲区已满，部分数据丢失");
        }

        // 处理音频数据
        let mut processed = self.format_processor.process(data).await?;

        // 执行效果处理
        for processor in &mut self.effect_processors {
            processed = processor.process(processed).await?;
        }

        // 写入输出缓冲区
        let mut results = HashMap::new();
        for (format, buffer) in &self.output_buffers {
            buffer.write(&processed);
            results.insert(format.clone(), buffer.read(processed.len()));
        }

        Ok(results)
    }

    /// 获取指定格式的输出流
    pub fn get_output_stream(&self, format: &AudioFormat) -> Option<AudioStream> {
        self.output_buffers.get(format).map(|buffer| {
            let buffer = buffer.clone();
            AudioStream::new(stream::unfold(buffer, |buffer| async move {
                let data = buffer.read(1024);
                if data.is_empty() {
                    None
                } else {
                    Some((Ok(AudioChunk::new(data, 0, 0, false)), buffer))
                }
            }))
        })
    }

    /// 清空所有缓冲区
    pub fn clear(&mut self) {
        self.input_buffer.clear();
        for buffer in self.output_buffers.values() {
            buffer.clear();
        }
    }
}

#[async_trait]
impl StreamProcessor for AudioConverter {
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        let results = self.process(chunk.data).await?;

        // 返回第一个输出格式的结果
        if let Some(format) = self.config.output_formats.first() {
            if let Some(data) = results.get(format) {
                return Ok(AudioChunk::new(
                    data.clone(),
                    chunk.timestamp,
                    chunk.duration,
                    chunk.is_last,
                ));
            }
        }

        Ok(AudioChunk::empty())
    }
}
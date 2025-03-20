//! 声道处理器
//!
//! 提供音频声道处理功能，包括声道数转换、音量调节等。支持单声道与立体声之间的转换，
//! 以及音量增益调节，适用于音频流的实时处理。
//!
//! # 特性
//! - 支持单声道/立体声转换
//! - 音量增益调节
//! - 支持多种采样率和位深度
//!
//! # 示例
//! ```no_run
//! use crate::audio::processors::{ChannelProcessor, ChannelProcessorParams};
//!
//! // 创建一个立体声到单声道的转换处理器
//! let params = ChannelProcessorParams {
//!     input_channels: 2,
//!     output_channels: 1,
//!     sample_rate: 44100,
//!     bits_per_sample: 16,
//!     volume: 1.0,
//! };
//! let processor = ChannelProcessor::new(params);
//! ```

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};

use super::{AudioProcessor, AudioProcessorParams};
use crate::error::ServiceResult;

/// 声道处理器
///
/// 实现音频声道数转换和音量调节功能。
/// 支持以下功能：
/// - 单声道转立体声：将单个声道的音频复制到两个声道
/// - 立体声转单声道：将两个声道的音频混合为一个声道
/// - 音量调节：对音频信号进行增益调节参数
///
/// 配置声道处理器的参数，包括输入输出声道数、采样率、位深度和音量增益。
/// 通过调整这些参数可以实现不同的声道转换和音量调节效果。
#[derive(Debug, Clone)]
pub struct ChannelProcessorParams {
    /// 输入声道数
    pub input_channels: u8,
    /// 输出声道数
    pub output_channels: u8,
    /// 采样率
    pub sample_rate: u32,
    /// 采样位数
    pub bits_per_sample: u16,
    /// 音量增益（默认为1.0）
    pub volume: f32,
}

impl Default for ChannelProcessorParams {
    fn default() -> Self {
        Self {
            input_channels: 1,
            output_channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            volume: 1.0,
        }
    }
}

/// 声道处理器
///
/// 实现音频声道数转换和音量调节功能。
/// 支持以下功能：
/// - 单声道转立体声：将单个声道的音频复制到两个声道
/// - 立体声转单声道：将两个声道的音频混合为一个声道
/// - 音量调节：对音频信号进行增益调节
pub struct ChannelProcessor {
    /// 处理器参数
    params: ChannelProcessorParams,
}

impl ChannelProcessor {
    /// 创建新的声道处理器
    ///
    /// # 参数
    /// * `params` - 声道处理器的配置参数，包括声道数、采样率等
    ///
    /// # 示例
    /// ```no_run
    /// let params = ChannelProcessorParams::default();
    /// let processor = ChannelProcessor::new(params);
    /// ```
    pub fn new(params: ChannelProcessorParams) -> Self {
        Self { params }
    }

    /// 处理单个采样
    fn process_sample(&self, sample: f32) -> f32 {
        sample * self.params.volume
    }

    /// 将单声道转换为多声道
    fn mono_to_multi(&self, sample: f32) -> Vec<f32> {
        vec![sample; self.params.output_channels as usize]
    }

    /// 将多声道混音为单声道
    fn multi_to_mono(&self, samples: &[f32]) -> f32 {
        samples.iter().sum::<f32>() / samples.len() as f32
    }
}

#[async_trait]
impl AudioProcessor for ChannelProcessor {
    fn name(&self) -> &str {
        "channel_processor"
    }

    async fn process(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        let sample_size = (self.params.bits_per_sample / 8) as usize;
        let frame_size = sample_size * self.params.input_channels as usize;
        let mut output = BytesMut::with_capacity(data.len());

        // 处理每一帧数据
        for frame in data.chunks(frame_size) {
            let mut samples = Vec::with_capacity(self.params.input_channels as usize);

            // 将字节数据转换为浮点数
            for chunk in frame.chunks(sample_size) {
                let sample = match sample_size {
                    2 => i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0,
                    4 => f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                    _ => return Err(crate::error::ServiceError::InvalidFormat(
                        "不支持的采样位数".to_string())),
                };
                samples.push(sample);
            }

            // 处理声道转换
            let processed = match (self.params.input_channels, self.params.output_channels) {
                (1, 1) => vec![self.process_sample(samples[0])],
                (1, _) => self.mono_to_multi(self.process_sample(samples[0])),
                (_, 1) => vec![self.process_sample(self.multi_to_mono(&samples))],
                (i, o) if i == o => samples.iter()
                    .map(|&s| self.process_sample(s))
                    .collect(),
                (_, o) => {
                    let mono = self.multi_to_mono(&samples);
                    self.mono_to_multi(self.process_sample(mono))
                }
            };

            // 将处理后的浮点数转回字节
            for sample in processed {
                match sample_size {
                    2 => {
                        let value = (sample * 32768.0) as i16;
                        output.extend_from_slice(&value.to_le_bytes());
                    }
                    4 => {
                        output.extend_from_slice(&sample.to_le_bytes());
                    }
                    _ => unreachable!(),
                }
            }
        }

        Ok(output.freeze())
    }

    fn reset(&mut self) {
        // 声道处理器不需要维护状态
    }

    fn parameters(&self) -> AudioProcessorParams {
        AudioProcessorParams {
            sample_rate: self.params.sample_rate,
            channels: self.params.output_channels,
            bits_per_sample: self.params.bits_per_sample,
        }
    }
}
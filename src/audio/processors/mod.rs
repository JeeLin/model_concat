//! 音频处理器模块
//!
//! 本模块提供了一系列音频信号处理器，用于对音频数据进行各种处理和转换，主要包括：
//!
//! - 增益处理：通过`GainProcessor`调整音频信号的音量
//! - 音频标准化：使用`NormalizationProcessor`对音频信号进行标准化处理
//! - 流式处理：支持`EnhancedStreamProcessor`进行实时流式音频处理
//! - 并行处理：通过`ParallelProcessorConfig`配置并行处理策略
//! - 重采样：支持音频采样率转换
//! - 通道转换：支持音频通道数的转换
//!
//! # 示例
//!
//! ```rust
//! use crate::audio::processors::{AudioProcessor, GainProcessor};
//!
//! // 创建增益处理器
//! let mut processor = GainProcessor::new(1.5); // 1.5倍增益
//!
//! // 处理音频样本
//! let mut samples = vec![0.1, 0.2, 0.3];
//! processor.process_samples(&mut samples);
//! ```
use crate::audio::format::AudioFormat;
use crate::error::ServiceResult;
use async_trait;
use rubato::{Resampler as RubatoResampler, SincFixedIn};
pub mod gain;
pub mod normalizer;
pub mod stream_processor;

pub use normalizer::NormalizationProcessor;

/// 音频处理器特征
///
/// 定义了音频处理器的基本接口，所有具体的处理器都需要实现这个特征
pub trait AudioProcessor: Send + Sync {
    /// 处理音频样本
    ///
    /// # 参数
    /// * `samples` - 待处理的音频样本数组，处理结果直接修改原数组
    fn process_samples(&mut self, samples: &mut [f32]);

    /// 重置处理器状态
    fn reset(&mut self);

    /// 获取处理器元数据
    fn metadata(&self) -> ProcessorMeta {
        ProcessorMeta::default()
    }

    /// 设置音频格式参数
    fn set_format(&mut self, format: &AudioFormat);
}

/// 处理器元数据
#[derive(Debug, Clone, Default)]
pub struct ProcessorMeta {
    /// 所需的音频格式
    pub required_format: Option<AudioFormat>,
    /// 处理块大小
    pub block_size: Option<usize>,
}

/// 重采样器特征
#[async_trait::async_trait]
pub trait Resampler: Send + Sync {
    /// 重采样音频数据
    ///
    /// # 参数
    /// * `samples` - 输入音频样本
    ///
    /// # 返回
    /// 返回重采样后的音频样本
    async fn resample(&mut self, samples: &[f32]) -> ServiceResult<Vec<f32>>;
}

/// 通道转换器特征
#[async_trait::async_trait]
pub trait ChannelConverter: Send + Sync {
    /// 转换通道数
    ///
    /// # 参数
    /// * `samples` - 输入音频样本
    ///
    /// # 返回
    /// 返回通道转换后的音频样本
    async fn convert_channels(&mut self, samples: &[f32]) -> ServiceResult<Vec<f32>>;
}

/// 重采样器
pub struct Resampler {
    resampler: SincFixedIn<f32>,
    channels: usize,
}

#[async_trait::async_trait]
impl Resampler for Resampler {
    async fn resample(&mut self, samples: &[f32]) -> ServiceResult<Vec<f32>> {
        let frames = samples.len() / self.channels;
        let mut input: Vec<Vec<f32>> = vec![Vec::with_capacity(frames); self.channels];

        // 将交错的样本分离到每个通道
        for (i, sample) in samples.iter().enumerate() {
            input[i % self.channels].push(*sample);
        }

        // 执行重采样
        let output = self.resampler.process(&input, None)
            .map_err(|e| crate::error::ServiceError::AudioConversion(e.to_string()))?;

        // 将通道数据重新交错
        let mut result = Vec::with_capacity(output[0].len() * self.channels);
        for frame in 0..output[0].len() {
            for channel in 0..self.channels {
                result.push(output[channel][frame]);
            }
        }

        Ok(result)
    }
}

impl Resampler {
    pub fn new(src_rate: u32, dst_rate: u32, channels: usize) -> ServiceResult<Self> {
        let resampler = SincFixedIn::new(
            dst_rate as f64 / src_rate as f64,
            2.0,
            rubato::SincInterpolationType::Linear,
            channels,
            4096,
        ).map_err(|e| crate::error::ServiceError::AudioConversion(e.to_string()))?;

        Ok(Self {
            resampler,
            channels,
        })
    }
}

/// 通道转换器
pub struct ChannelConverter {
    src_channels: usize,
    dst_channels: usize,
}

#[async_trait::async_trait]
impl ChannelConverter for ChannelConverter {
    async fn convert_channels(&mut self, samples: &[f32]) -> ServiceResult<Vec<f32>> {
        let frames = samples.len() / self.src_channels;
        let mut output = Vec::with_capacity(frames * self.dst_channels);

        match (self.src_channels, self.dst_channels) {
            // 单声道转立体声
            (1, 2) => {
                for sample in samples {
                    output.push(*sample); // 左声道
                    output.push(*sample); // 右声道
                }
            }
            // 立体声转单声道
            (2, 1) => {
                for chunk in samples.chunks_exact(2) {
                    output.push((chunk[0] + chunk[1]) * 0.5); // 取平均值
                }
            }
            // 其他情况（暂时只支持简单的截断或重复）
            _ => {
                if self.dst_channels > self.src_channels {
                    // 通过重复通道扩展
                    for frame in 0..frames {
                        for dst_ch in 0..self.dst_channels {
                            let src_ch = dst_ch % self.src_channels;
                            output.push(samples[frame * self.src_channels + src_ch]);
                        }
                    }
                } else {
                    // 通过截断减少通道
                    for frame in 0..frames {
                        for dst_ch in 0..self.dst_channels {
                            output.push(samples[frame * self.src_channels + dst_ch]);
                        }
                    }
                }
            }
        }

        Ok(output)
    }
}

impl ChannelConverter {
    pub fn new(src_channels: usize, dst_channels: usize) -> Self {
        Self {
            src_channels,
            dst_channels,
        }
    }
}
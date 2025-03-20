//! 音频均衡器处理器
//!
//! 提供音频信号的均衡处理功能，支持调整不同频段的增益。均衡器可以用于调整音频的音色，
//! 增强或减弱特定频率范围的声音，适用于音频后期处理和实时音效处理。
//!
//! # 特性
//! - 支持多个频段独立调节
//! - 使用BiQuad滤波器实现高质量均衡
//! - 支持实时处理
//!
//! # 示例
//! ```no_run
//! use std::collections::HashMap;
//! use crate::audio::processors::{Equalizer, EqualizerParams};
//!
//! // 创建一个三段式均衡器
//! let mut bands = HashMap::new();
//! bands.insert(100.0, -3.0);  // 降低低频
//! bands.insert(1000.0, 0.0);  // 中频保持不变
//! bands.insert(8000.0, 6.0);  // 提升高频
//!
//! let params = EqualizerParams {
//!     bands,
//!     sample_rate: 44100,
//!     channels: 2,
//!     bits_per_sample: 16,
//! };
//! let equalizer = Equalizer::new(params);
//! ```

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::collections::HashMap;

use super::{AudioProcessor, AudioProcessorParams};
use crate::error::ServiceResult;

/// 均衡器参数
///
/// 配置均衡器的参数，包括频段增益设置、采样率和通道数。
/// 每个频段可以独立设置增益值，正值表示增强，负值表示衰减。
#[derive(Debug, Clone)]
pub struct EqualizerParams {
    /// 频段增益设置（频率 -> 增益）
    pub bands: HashMap<f32, f32>,
    /// 采样率
    pub sample_rate: u32,
    /// 通道数
    pub channels: u8,
    /// 采样位数
    pub bits_per_sample: u16,
}

/// 音频均衡器
///
/// 实现多频段音频均衡功能，使用BiQuad滤波器对不同频段进行增益调节。
/// 每个频段使用一个独立的滤波器，可以精确控制频率响应。
pub struct Equalizer {
    /// 均衡器参数
///
/// 配置均衡器的参数，包括频段增益设置、采样率和通道数。
/// 每个频段可以独立设置增益值，正值表示增强，负值表示衰减。
    params: EqualizerParams,
    /// 滤波器状态
    filters: Vec<BiQuadFilter>,
}

impl Equalizer {
    /// 创建新的均衡器
    ///
    /// # 参数
    /// * `params` - 均衡器配置参数，包括频段设置和音频格式
    ///
    /// # 示例
    /// ```no_run
    /// let mut bands = HashMap::new();
    /// bands.insert(1000.0, 3.0);  // 1kHz增强3dB
    /// let params = EqualizerParams {
    ///     bands,
    ///     sample_rate: 44100,
    ///     channels: 2,
    ///     bits_per_sample: 16,
    /// };
    /// let equalizer = Equalizer::new(params);
    /// ```
    pub fn new(params: EqualizerParams) -> Self {
        let mut filters = Vec::new();
        for (freq, gain) in &params.bands {
            filters.push(BiQuadFilter::new_peaking_eq(
                *freq,
                1.0,  // Q因子
                *gain,
                params.sample_rate as f32,
            ));
        }

        Self { params, filters }
    }

    /// 处理单个采样
    ///
    /// 对输入的音频采样应用所有频段的滤波器，实现均衡效果。
    ///
    /// # 参数
    /// * `sample` - 输入的音频采样值
    ///
    /// # 返回
    /// 处理后的音频采样值
    fn process_sample(&mut self, sample: f32) -> f32 {
        let mut output = sample;
        for filter in &mut self.filters {
            output = filter.process(output);
        }
        output
    }
}

#[async_trait]
impl AudioProcessor for Equalizer {
    fn name(&self) -> &str {
        "equalizer"
    }

    async fn process(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        let sample_size = (self.params.bits_per_sample / 8) as usize;
        let mut output = BytesMut::with_capacity(data.len());

        // 将字节数据转换为浮点数进行处理
        for chunk in data.chunks(sample_size) {
            let sample = match sample_size {
                2 => i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0,
                4 => f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                _ => return Err(crate::error::ServiceError::InvalidFormat(
                    "不支持的采样位数".to_string())),
            };

            let processed = self.process_sample(sample);

            // 将处理后的浮点数转回字节
            match sample_size {
                2 => {
                    let value = (processed * 32768.0) as i16;
                    output.extend_from_slice(&value.to_le_bytes());
                }
                4 => {
                    output.extend_from_slice(&processed.to_le_bytes());
                }
                _ => unreachable!(),
            }
        }

        Ok(output.freeze())
    }

    fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }

    fn parameters(&self) -> AudioProcessorParams {
        AudioProcessorParams {
            sample_rate: self.params.sample_rate,
            channels: self.params.channels,
            bits_per_sample: self.params.bits_per_sample,
        }
    }
}

/// 双二阶滤波器
#[derive(Debug)]
struct BiQuadFilter {
    /// 滤波器系数
    a0: f32,
    a1: f32,
    a2: f32,
    b1: f32,
    b2: f32,
    /// 延迟线
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiQuadFilter {
    /// 创建新的峰值均衡滤波器
    fn new_peaking_eq(freq: f32, q: f32, gain_db: f32, sample_rate: f32) -> Self {
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let alpha = omega.sin() / (2.0 * q);
        let a = 10.0f32.powf(gain_db / 40.0);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * omega.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * omega.cos();
        let a2 = 1.0 - alpha / a;

        Self {
            a0: b0 / a0,
            a1: b1 / a0,
            a2: b2 / a0,
            b1: a1 / a0,
            b2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// 处理单个采样
    ///
    /// 对输入的音频采样应用所有频段的滤波器，实现均衡效果。
    ///
    /// # 参数
    /// * `sample` - 输入的音频采样值
    ///
    /// # 返回
    /// 处理后的音频采样值
    fn process(&mut self, input: f32) -> f32 {
        let output = self.a0 * input + self.a1 * self.x1 + self.a2 * self.x2
            - self.b1 * self.y1 - self.b2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// 重置滤波器状态
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}
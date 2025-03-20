//! 音量处理器
//!
//! 提供音量增益和归一化功能。

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};

use super::{AudioProcessor, AudioProcessorParams};
use crate::error::ServiceResult;

/// 音量处理器
pub struct VolumeProcessor {
    /// 音量增益
    gain: f32,
    /// 是否启用音量归一化
    normalize: bool,
    /// 处理器参数
    params: AudioProcessorParams,
}

impl VolumeProcessor {
    /// 创建新的音量处理器
    pub fn new(gain: f32, normalize: bool, params: AudioProcessorParams) -> Self {
        Self {
            gain,
            normalize,
            params,
        }
    }

    /// 应用音量增益
    fn apply_gain(&self, samples: &mut [i16]) {
        for sample in samples.iter_mut() {
            let value = *sample as f32 * self.gain;
            *sample = value.min(32767.0).max(-32768.0) as i16;
        }
    }

    /// 应用音量归一化
    fn normalize(&self, samples: &mut [i16]) {
        if samples.is_empty() {
            return;
        }

        // 找到最大振幅
        let max_amplitude = samples
            .iter()
            .map(|&x| x.abs() as f32)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        if max_amplitude > 0.0 {
            // 计算归一化因子
            let normalize_factor = 32767.0 / max_amplitude;
            
            // 应用归一化
            for sample in samples.iter_mut() {
                let value = *sample as f32 * normalize_factor;
                *sample = value.min(32767.0).max(-32768.0) as i16;
            }
        }
    }
}

#[async_trait]
impl AudioProcessor for VolumeProcessor {
    fn name(&self) -> &str {
        "volume"
    }

    async fn process(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        let sample_size = (self.params.bits_per_sample / 8) as usize;
        let samples_count = data.len() / sample_size;
        let mut samples = Vec::with_capacity(samples_count);

        // 将字节数据转换为采样点
        for i in (0..data.len()).step_by(sample_size) {
            let sample = i16::from_le_bytes([data[i], data[i + 1]]);
            samples.push(sample);
        }

        // 应用音量处理
        if self.normalize {
            self.normalize(&mut samples);
        }
        self.apply_gain(&mut samples);

        // 将采样点转换回字节数据
        let mut output = BytesMut::with_capacity(data.len());
        for sample in samples {
            output.extend_from_slice(&sample.to_le_bytes());
        }

        Ok(output.freeze())
    }

    fn reset(&mut self) {
        // 音量处理器不需要维护状态
    }

    fn parameters(&self) -> AudioProcessorParams {
        self.params.clone()
    }
}
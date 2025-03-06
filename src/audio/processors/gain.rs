use super::{AudioProcessor, ProcessorMeta};
use crate::audio::format::AudioFormat;

/// 音量增益处理器
///
/// 提供音频信号的增益调节功能，可以通过分贝值调整音频的音量大小。
/// 主要特点：
/// - 精确的分贝控制：支持以分贝(dB)为单位的增益调节
/// - 实时处理：支持实时音频流的音量调整
/// - 无失真处理：采用浮点运算保证信号质量
///
/// # 示例
///
/// ```rust
/// use crate::audio::processors::{AudioProcessor, GainProcessor};
///
/// // 创建一个+6dB的增益处理器
/// let mut processor = GainProcessor::new(6.0);
///
/// // 处理音频样本
/// let mut samples = vec![0.5, -0.3, 0.2];
/// processor.process_samples(&mut samples);
///
/// // 样本值会被放大约2倍(6dB = 2倍电压增益)
/// assert!((samples[0] - 1.0).abs() < 0.01);
/// ```
#[derive(Debug)]
pub struct GainProcessor {
    /// 增益系数(线性比例，非分贝值)
    gain_factor: f32,
    /// 处理器元数据
    metadata: ProcessorMeta,
}

impl GainProcessor {
    /// 创建新的增益处理器
    ///
    /// # 参数
    /// * `gain_db` - 增益值(分贝)，正值表示放大，负值表示衰减
    ///
    /// # 示例
    /// ```rust
    /// use crate::audio::processors::GainProcessor;
    ///
    /// // 创建-3dB的衰减器
    /// let processor = GainProcessor::new(-3.0);
    ///
    /// // 创建+12dB的放大器
    /// let processor = GainProcessor::new(12.0);
    /// ```
    pub fn new(gain_db: f32) -> Self {
        Self {
            gain_factor: 10.0f32.powf(gain_db / 20.0),
            metadata: ProcessorMeta::default(),
        }
    }

    /// 获取当前增益系数
    pub fn gain_factor(&self) -> f32 {
        self.gain_factor
    }
}

impl AudioProcessor for GainProcessor {
    fn process_samples(&mut self, samples: &mut [f32]) {
        for sample in samples {
            *sample *= self.gain_factor;
        }
    }

    fn set_format(&mut self, format: &AudioFormat) {
        self.metadata.required_format = Some(format.clone());
    }

    fn reset(&mut self) {
        // 增益处理器是无状态的，不需要重置
    }

    fn metadata(&self) -> ProcessorMeta {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_processor_creation() {
        let processor = GainProcessor::new(6.0);
        assert!((processor.gain_factor() - 2.0).abs() < 0.01); // 6dB ≈ 2倍增益
    }

    #[test]
    fn test_gain_processing() {
        let mut processor = GainProcessor::new(20.0); // 20dB = 10倍增益
        let mut samples = vec![0.1, -0.2, 0.3];
        processor.process_samples(&mut samples);

        assert!((samples[0] - 1.0).abs() < 0.01);
        assert!((samples[1] - (-2.0)).abs() < 0.01);
        assert!((samples[2] - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_format_setting() {
        let mut processor = GainProcessor::new(0.0);
        let format = AudioFormat {
            sample_rate: 44100,
            channels: 2,
            ..Default::default()
        };

        processor.set_format(&format);
        assert_eq!(
            processor.metadata().required_format.unwrap().sample_rate,
            44100
        );
    }
}

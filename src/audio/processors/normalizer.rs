use super::{AudioProcessor, ProcessorMeta};
use crate::audio::format::AudioFormat;

/// 音频标准化处理器
///
/// 提供音频信号的动态范围标准化功能，通过分析整个音频流的峰值
/// 来调整增益，使音频信号的最大峰值达到指定的目标值。
///
/// 主要特点：
/// - 自适应增益：根据音频内容自动调整增益
/// - 动态范围优化：保持音频的相对动态范围
/// - 过载保护：防止信号过载和失真
///
/// # 示例
///
/// ```rust
/// use crate::audio::processors::{AudioProcessor, NormalizationProcessor};
///
/// // 创建标准化处理器，目标峰值为0.95
/// let mut processor = NormalizationProcessor::new(0.95);
///
/// // 处理第一批样本
/// let mut samples1 = vec![0.5, -0.8, 0.3];
/// processor.process_samples(&mut samples1);
///
/// // 处理第二批样本
/// let mut samples2 = vec![0.2, -0.4, 0.6];
/// processor.process_samples(&mut samples2);
///
/// // 获取标准化后的结果
/// let normalized = processor.finalize();
/// ```
pub struct NormalizationProcessor {
    /// 目标峰值电平
    target_peak: f32,
    /// 当前检测到的最大峰值
    max_peak: f32,
    /// 音频样本缓冲区
    buffer: Vec<f32>,
    /// 处理器元数据
    metadata: ProcessorMeta,
}

impl NormalizationProcessor {
    /// 创建新的标准化处理器
    ///
    /// # 参数
    /// * `target_peak` - 目标峰值电平，通常设置为小于1.0的值以防止过载
    ///
    /// # 示例
    /// ```rust
    /// use crate::audio::processors::NormalizationProcessor;
    ///
    /// // 创建目标峰值为0.95的标准化处理器
    /// let processor = NormalizationProcessor::new(0.95);
    /// ```
    pub fn new(target_peak: f32) -> Self {
        Self {
            target_peak,
            max_peak: 0.0,
            buffer: Vec::new(),
            metadata: ProcessorMeta::default(),
        }
    }

    /// 分析音频样本，更新最大峰值
    ///
    /// # 参数
    /// * `samples` - 要分析的音频样本
    pub fn analyze(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.max_peak = self.max_peak.max(sample.abs());
        }
        self.buffer.extend_from_slice(samples);
    }

    /// 完成标准化处理并返回结果
    ///
    /// 根据分析的最大峰值，计算并应用增益系数，
    /// 使音频信号的峰值达到目标电平。
    ///
    /// # 返回值
    /// 返回标准化处理后的音频样本
    pub fn finalize(&mut self) -> Vec<f32> {
        let gain = if self.max_peak > 0.0 {
            self.target_peak / self.max_peak
        } else {
            1.0
        };

        for sample in &mut self.buffer {
            *sample *= gain;
        }

        std::mem::take(&mut self.buffer)
    }

    /// 获取当前检测到的最大峰值
    pub fn max_peak(&self) -> f32 {
        self.max_peak
    }

    /// 获取目标峰值电平
    pub fn target_peak(&self) -> f32 {
        self.target_peak
    }
}

impl AudioProcessor for NormalizationProcessor {
    fn process_samples(&mut self, samples: &mut [f32]) {
        self.analyze(samples);
    }

    fn reset(&mut self) {
        self.max_peak = 0.0;
        self.buffer.clear();
    }

    fn set_format(&mut self, format: &AudioFormat) {
        self.metadata.required_format = Some(format.clone());
    }

    fn metadata(&self) -> ProcessorMeta {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalization_processor_creation() {
        let processor = NormalizationProcessor::new(0.95);
        assert_eq!(processor.target_peak(), 0.95);
        assert_eq!(processor.max_peak(), 0.0);
    }

    #[test]
    fn test_peak_detection() {
        let mut processor = NormalizationProcessor::new(0.95);
        processor.analyze(&[0.5, -0.8, 0.3]);
        assert_eq!(processor.max_peak(), 0.8);
    }

    #[test]
    fn test_normalization() {
        let mut processor = NormalizationProcessor::new(0.95);
        processor.analyze(&[0.5, -0.8, 0.3]);
        let result = processor.finalize();

        // 最大峰值应该接近目标值0.95
        assert!((result[1].abs() - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_format_setting() {
        let mut processor = NormalizationProcessor::new(0.95);
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

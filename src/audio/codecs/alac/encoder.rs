use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::{ServiceError, ServiceResult};
use bytes::Bytes;

/// ALAC音频编码器
///
/// Apple Lossless Audio Codec (ALAC) 编码器的简化实现，支持以下功能：
/// - 支持单声道和立体声编码
/// - 使用16位采样深度
/// - 基于帧的音频数据处理
/// - 支持流式编码
///
/// 注意：当前实现为简化版本，仅用于演示。实际应用中应使用完整的ALAC编码库。
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::alac::AlacEncoder;
///
/// // 创建ALAC编码器
/// let mut encoder = AlacEncoder::new(44100, 2)?;
///
/// // 编码音频数据
/// let samples = vec![0.0f32; 4096 * 2]; // 4096个立体声样本
/// encoder.encode_samples(&samples)?;
///
/// // 完成编码并获取结果
/// let encoded_data = encoder.finalize()?;
/// ```
pub struct AlacEncoder {
    /// 基础编码器，提供通用的编码功能
    base: BaseEncoder,
    /// 每帧采样数
    frame_size: usize,
}

impl AlacEncoder {
    /// 创建新的ALAC编码器
    ///
    /// # 参数
    ///
    /// * `sample_rate` - 采样率（Hz），常用值：44100, 48000
    /// * `channels` - 通道数，1表示单声道，2表示立体声
    ///
    /// # 返回值
    ///
    /// 返回`ServiceResult<AlacEncoder>`，成功时包含编码器实例
    pub fn new(sample_rate: u32, channels: u16) -> ServiceResult<Self> {
        // ALAC编码器实现
        // 注意：这里使用简化实现，实际应该使用ALAC编码库

        // 每帧采样数，ALAC通常使用4096个样本
        let frame_size = 4096;

        Ok(Self {
            base: BaseEncoder::new(sample_rate, channels, None),
            frame_size,
        })
    }
}

impl AudioEncoder for AlacEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 使用BaseEncoder的process_frames方法处理帧
        self.base
            .process_frames(samples, self.frame_size, |chunk| {
                // 将浮点样本转换为16位整数
                let i16_samples = self.base.convert_to_i16(chunk);

                // 这里应该使用实际的ALAC编码库进行编码
                // 由于缺少原生ALAC编码库，这里只是简单地将样本转换为字节
                let mut encoded = Vec::with_capacity(i16_samples.len() * 2);
                for sample in i16_samples {
                    encoded.extend_from_slice(&sample.to_le_bytes());
                }

                Ok(encoded)
            })
            .map_err(|e: ServiceError| e)?;

        Ok(None)
    }

    fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
        // 如果缓冲区不为空，返回缓冲区内容
        if !self.base.buffer.is_empty() {
            Ok(Some(self.base.take_buffer()))
        } else {
            Ok(None)
        }
    }

    fn reset(&mut self) {
        self.base.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_encoder() {
        // 测试创建编码器
        let encoder = AlacEncoder::new(44100, 2);
        assert!(encoder.is_ok());

        let encoder = encoder.unwrap();
        assert_eq!(encoder.frame_size, 4096);
        assert_eq!(encoder.base.sample_rate, 44100);
        assert_eq!(encoder.base.channels, 2);
        assert_eq!(encoder.base.bit_rate, None);
    }

    #[test]
    fn test_encode_mono() {
        // 测试单声道编码
        let mut encoder = AlacEncoder::new(44100, 1).unwrap();

        // 创建一个完整帧的测试数据
        let samples = vec![0.0f32; 4096];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_stereo() {
        // 测试立体声编码
        let mut encoder = AlacEncoder::new(44100, 2).unwrap();

        // 创建一个完整帧的立体声测试数据
        let samples = vec![0.0f32; 4096 * 2];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finalize() {
        // 测试编码完成
        let mut encoder = AlacEncoder::new(44100, 2).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 4096 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 测试finalize
        let result = encoder.finalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        // 测试重置编码器
        let mut encoder = AlacEncoder::new(44100, 2).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 4096 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 重置编码器
        encoder.reset();
        assert!(encoder.base.buffer.is_empty());
        assert!(!encoder.base.finalized);
    }
}

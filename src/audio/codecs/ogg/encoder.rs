use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::{ServiceError, ServiceResult};
use bytes::Bytes;
use vorbis_encoder::{VorbisEncoder as VorbisEnc, VorbisEncoderConfig};

/// OGG音频编码器
///
/// 使用Vorbis库实现的OGG音频编码器，支持以下功能：
/// - 支持单声道和立体声编码
/// - 可配置采样率和质量级别
/// - 基于帧的音频数据处理
/// - 支持流式编码
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::ogg::OggEncoder;
///
/// // 创建OGG编码器
/// let mut encoder = OggEncoder::new(44100, 2, 0.5)?; // 质量范围0.0-1.0
///
/// // 编码音频数据
/// let samples = vec![0.0f32; 1024 * 2]; // 1024个立体声样本
/// encoder.encode_samples(&samples)?;
///
/// // 完成编码并获取结果
/// let encoded_data = encoder.finalize()?;
/// ```
pub struct OggEncoder {
    /// Vorbis编码器实例
    encoder: VorbisEnc,
    /// 基础编码器，提供通用的编码功能
    base: BaseEncoder,
    /// 每帧采样数，Vorbis通常使用1024个样本
    frame_size: usize,
}

impl OggEncoder {
    /// 创建新的OGG编码器
    ///
    /// # 参数
    ///
    /// * `sample_rate` - 采样率（Hz），常用值：44100, 48000
    /// * `channels` - 通道数，1表示单声道，2表示立体声
    /// * `quality` - 质量级别（0.0-1.0），值越大质量越好
    ///
    /// # 返回值
    ///
    /// 返回`ServiceResult<OggEncoder>`，成功时包含编码器实例，失败时包含错误信息
    ///
    /// # 错误
    ///
    /// 当Vorbis编码器初始化失败时返回错误
    pub fn new(sample_rate: u32, channels: u16, quality: f32) -> ServiceResult<Self> {
        // 创建Vorbis编码器配置
        let config = VorbisEncoderConfig::new(channels as u8, sample_rate, quality);

        // 创建Vorbis编码器
        let encoder = VorbisEnc::new(config).map_err(|e| {
            BaseEncoder::create_error("OGG", format!("Failed to create encoder: {}", e))
        })?;

        // 每帧采样数，Vorbis通常使用1024个样本
        let frame_size = 1024;

        Ok(Self {
            encoder,
            base: BaseEncoder::new(sample_rate, channels, None),
            frame_size,
        })
    }
}

impl AudioEncoder for OggEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 使用BaseEncoder的process_frames方法处理帧
        self.base
            .process_frames(samples, self.frame_size, |chunk| {
                // 编码音频数据
                let encoded = self.encoder.encode(chunk).map_err(|e| {
                    BaseEncoder::create_error("OGG", format!("Encoding failed: {}", e))
                })?;

                // 将编码数据添加到缓冲区
                Ok(encoded)
            })
            .map_err(|e: ServiceError| e)?;

        Ok(None)
    }

    fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
        // 完成编码，获取所有编码数据
        let flush_data = self
            .encoder
            .flush()
            .map_err(|e| BaseEncoder::create_error("OGG", format!("Flush failed: {}", e)))?;

        // 将刷新数据添加到缓冲区
        self.base.buffer.extend_from_slice(&flush_data);

        // 返回所有编码数据
        if !self.base.buffer.is_empty() {
            Ok(Some(self.base.take_buffer()))
        } else {
            Ok(None)
        }
    }

    fn reset(&mut self) {
        // 重置基础编码器
        self.base.reset();

        // 重新创建Vorbis编码器
        let config = VorbisEncoderConfig::new(
            self.base.channels as u8,
            self.base.sample_rate,
            0.5, // 默认质量
        );

        if let Ok(encoder) = VorbisEnc::new(config) {
            self.encoder = encoder;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_encoder() {
        // 测试创建编码器
        let encoder = OggEncoder::new(44100, 2, 0.5);
        assert!(encoder.is_ok());

        let encoder = encoder.unwrap();
        assert_eq!(encoder.frame_size, 1024);
        assert_eq!(encoder.base.sample_rate, 44100);
        assert_eq!(encoder.base.channels, 2);
        assert_eq!(encoder.base.bit_rate, None);
    }

    #[test]
    fn test_encode_mono() {
        // 测试单声道编码
        let mut encoder = OggEncoder::new(44100, 1, 0.5).unwrap();

        // 创建一个完整帧的测试数据
        let samples = vec![0.0f32; 1024];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_stereo() {
        // 测试立体声编码
        let mut encoder = OggEncoder::new(44100, 2, 0.5).unwrap();

        // 创建一个完整帧的立体声测试数据
        let samples = vec![0.0f32; 1024 * 2];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finalize() {
        // 测试编码完成和刷新
        let mut encoder = OggEncoder::new(44100, 2, 0.5).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 1024 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 测试finalize
        let result = encoder.finalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        // 测试重置编码器
        let mut encoder = OggEncoder::new(44100, 2, 0.5).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 1024 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 重置编码器
        encoder.reset();
        assert!(encoder.base.buffer.is_empty());
        assert!(!encoder.base.finalized);
    }
}

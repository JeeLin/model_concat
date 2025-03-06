use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::{ServiceError, ServiceResult};
use bytes::Bytes;
use claxon::{FlacEncoder as ClaxonEncoder, FlacEncoderSettings};
use std::io::Cursor;

/// FLAC音频编码器
///
/// 使用Claxon库实现的FLAC音频编码器，支持以下功能：
/// - 支持单声道和立体声编码
/// - 可配置压缩级别（0-8，值越大压缩率越高）
/// - 使用24位采样深度
/// - 支持流式编码
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::flac::FlacEncoder;
///
/// // 创建FLAC编码器
/// let mut encoder = FlacEncoder::new(44100, 2, 5)?;
///
/// // 编码音频数据
/// let samples = vec![0.0f32; 4096 * 2]; // 4096个立体声样本
/// encoder.encode_samples(&samples)?;
///
/// // 完成编码并获取结果
/// let encoded_data = encoder.finalize()?;
/// ```
pub struct FlacEncoder {
    /// FLAC编码器实例
    encoder: ClaxonEncoder<Cursor<Vec<u8>>>,
    /// 基础编码器，提供通用的编码功能
    base: BaseEncoder,
    /// 压缩级别（0-8）
    compression_level: u8,
    /// 每帧采样数
    frame_size: usize,
}

impl FlacEncoder {
    /// 创建新的FLAC编码器
    ///
    /// # 参数
    ///
    /// * `sample_rate` - 采样率（Hz），常用值：44100, 48000
    /// * `channels` - 通道数，1表示单声道，2表示立体声
    /// * `compression_level` - 压缩级别（0-8），值越大压缩率越高，但编码速度越慢
    ///
    /// # 返回值
    ///
    /// 返回`ServiceResult<FlacEncoder>`，成功时包含编码器实例，失败时包含错误信息
    ///
    /// # 错误
    ///
    /// 当Claxon编码器初始化失败时返回错误
    pub fn new(sample_rate: u32, channels: u16, compression_level: u8) -> ServiceResult<Self> {
        // 创建FLAC编码器设置
        let mut settings = FlacEncoderSettings::new();
        settings.set_compression_level(compression_level);

        // 创建FLAC编码器
        let cursor = Cursor::new(Vec::new());
        let encoder = ClaxonEncoder::with_settings(cursor, settings).map_err(|e| {
            BaseEncoder::create_error("FLAC", format!("Failed to create encoder: {}", e))
        })?;

        // 每帧采样数，FLAC通常使用4096个样本
        let frame_size = 4096;

        Ok(Self {
            encoder,
            base: BaseEncoder::new(sample_rate, channels, None),
            compression_level,
            frame_size,
        })
    }
}

impl AudioEncoder for FlacEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 使用BaseEncoder的process_frames方法处理帧
        self.base
            .process_frames(samples, self.frame_size, |chunk| {
                // 将浮点样本转换为24位整数
                let i24_samples = self.base.convert_to_i24(chunk);

                // 编码音频数据
                for sample in i24_samples {
                    self.encoder.write_sample(sample).map_err(|e| {
                        BaseEncoder::create_error("FLAC", format!("Encoding failed: {}", e))
                    })?;
                }

                // FLAC编码器在内部缓冲数据，不需要立即返回
                Ok(Vec::new())
            })
            .map_err(|e: ServiceError| e)?;

        Ok(None)
    }

    fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
        // 完成编码，获取所有编码数据
        self.encoder
            .flush()
            .map_err(|e| BaseEncoder::create_error("FLAC", format!("Flush failed: {}", e)))?;

        // 获取编码后的数据
        let cursor = self.encoder.into_inner();
        let encoded_data = cursor.into_inner();

        // 将编码数据添加到缓冲区
        self.base.buffer.extend_from_slice(&encoded_data);

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

        // 重新创建FLAC编码器
        let mut settings = FlacEncoderSettings::new();
        settings.set_compression_level(self.compression_level);

        let cursor = Cursor::new(Vec::new());
        if let Ok(encoder) = ClaxonEncoder::with_settings(cursor, settings) {
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
        let encoder = FlacEncoder::new(44100, 2, 5);
        assert!(encoder.is_ok());

        let encoder = encoder.unwrap();
        assert_eq!(encoder.frame_size, 4096);
        assert_eq!(encoder.base.sample_rate, 44100);
        assert_eq!(encoder.base.channels, 2);
        assert_eq!(encoder.compression_level, 5);
    }

    #[test]
    fn test_encode_mono() {
        // 测试单声道编码
        let mut encoder = FlacEncoder::new(44100, 1, 5).unwrap();

        // 创建一个完整帧的测试数据
        let samples = vec![0.0f32; 4096];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_stereo() {
        // 测试立体声编码
        let mut encoder = FlacEncoder::new(44100, 2, 5).unwrap();

        // 创建一个完整帧的立体声测试数据
        let samples = vec![0.0f32; 4096 * 2];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finalize() {
        // 测试编码完成和刷新
        let mut encoder = FlacEncoder::new(44100, 2, 5).unwrap();

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
        let mut encoder = FlacEncoder::new(44100, 2, 5).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 4096 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 重置编码器
        encoder.reset();
        assert!(encoder.base.buffer.is_empty());
        assert!(!encoder.base.finalized);
    }
}

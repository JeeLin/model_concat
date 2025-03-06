use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::{ServiceError, ServiceResult};
use bytes::Bytes;
use fdk_aac::{AacObjectType, Encoder as AacEnc, EncoderConfig as AacConfig};

/// AAC音频编码器
///
/// 使用FDK-AAC库实现的AAC音频编码器，支持以下功能：
/// - 支持单声道和立体声编码
/// - 可配置采样率和比特率
/// - 使用AAC-LC（低复杂度）配置文件
/// - 支持流式编码
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::aac::AacEncoder;
///
/// // 创建AAC编码器
/// let mut encoder = AacEncoder::new(44100, 2, 192000)?;
///
/// // 编码音频数据
/// let samples = vec![0.0f32; 1024 * 2]; // 1024个立体声样本
/// encoder.encode_samples(&samples)?;
///
/// // 完成编码并获取结果
/// let encoded_data = encoder.finalize()?;
/// ```
pub struct AacEncoder {
    /// AAC编码器实例
    encoder: AacEnc,
    /// 基础编码器，提供通用的编码功能
    base: BaseEncoder,
    /// 每帧采样数
    frame_size: usize,
}

impl AacEncoder {
    /// 创建新的AAC编码器
    ///
    /// # 参数
    ///
    /// * `sample_rate` - 采样率（Hz），常用值：44100, 48000
    /// * `channels` - 通道数，1表示单声道，2表示立体声
    /// * `bit_rate` - 比特率（bps），常用值：128000, 192000, 256000
    ///
    /// # 返回值
    ///
    /// 返回`ServiceResult<AacEncoder>`，成功时包含编码器实例，失败时包含错误信息
    ///
    /// # 错误
    ///
    /// 当FDK-AAC编码器初始化失败时返回错误
    pub fn new(sample_rate: u32, channels: u16, bit_rate: u32) -> ServiceResult<Self> {
        // 创建AAC编码器配置
        let mut config = AacConfig::new();
        config.set_aot(AacObjectType::AacLc) // 使用AAC-LC配置
            .set_bitrate(bit_rate as i32)
            .set_channels(channels as i32)
            .set_sample_rate(sample_rate as i32);

        // 创建AAC编码器
        let encoder = AacEnc::new(config).map_err(|e| {
            BaseEncoder::create_error("AAC", format!("Failed to create encoder: {}", e))
        })?;

        // 获取编码器的帧大小
        let frame_size = encoder.frame_size() as usize;

        Ok(Self {
            encoder,
            base: BaseEncoder::new(sample_rate, channels, Some(bit_rate)),
            frame_size,
        })
    }
}

impl AudioEncoder for AacEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 使用BaseEncoder的process_frames方法处理帧
        self.base
            .process_frames(samples, self.frame_size, |chunk| {
                // 预留足够的输出空间，AAC通常产生较小的数据包
                let mut output = vec![0u8; self.frame_size * 4];

                // 编码音频数据
                let encoded = self.encoder.encode(chunk, &mut output).map_err(|e| {
                    BaseEncoder::create_error("AAC", format!("Encoding failed: {}", e))
                })?;

                // 返回实际编码的数据
                if encoded > 0 {
                    Ok(output[..encoded].to_vec())
                } else {
                    Ok(Vec::new())
                }
            })
            .map_err(|e: ServiceError| e)?;

        Ok(None)
    }

    fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
        // AAC编码器不需要特殊的结束处理
        // 返回缓冲区中的所有数据
        if !self.base.buffer.is_empty() {
            Ok(Some(self.base.take_buffer()))
        } else {
            Ok(None)
        }
    }

    fn reset(&mut self) {
        // 重置基础编码器状态
        self.base.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_aac_encoder_creation() {
        // 测试有效参数
        let encoder = AacEncoder::new(44100, 2, 192000);
        assert!(encoder.is_ok());

        // 测试无效比特率
        let encoder = AacEncoder::new(44100, 2, 0);
        assert!(encoder.is_err());
    }

    #[test]
    fn test_aac_encoding_process() {
        let mut encoder = AacEncoder::new(44100, 2, 192000).unwrap();

        // 生成测试音频数据
        let samples: Vec<f32> = (0..2048)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
            .collect();

        // 测试编码过程
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_aac_encoder_performance() {
        let mut encoder = AacEncoder::new(44100, 2, 192000).unwrap();

        // 生成1秒的音频数据
        let samples: Vec<f32> = (0..44100 * 2)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
            .collect();

        let start = Instant::now();
        let result = encoder.encode_samples(&samples);
        let duration = start.elapsed();

        assert!(result.is_ok());
        println!("AAC编码性能：处理1秒音频数据耗时：{:?}", duration);
    }

    #[test]
    fn test_aac_encoder_finalize() {
        let mut encoder = AacEncoder::new(44100, 2, 192000).unwrap();

        // 编码一些数据
        let samples: Vec<f32> = (0..2048)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
            .collect();

        encoder.encode_samples(&samples).unwrap();

        // 测试finalize
        let result = encoder.finalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_aac_encoder_error_handling() {
        // 测试无效的采样率
        let result = AacEncoder::new(8000, 2, 192000);
        assert!(result.is_err());

        // 测试无效的通道数
        let result = AacEncoder::new(44100, 0, 192000);
        assert!(result.is_err());

        // 测试空输入
        let mut encoder = AacEncoder::new(44100, 2, 192000).unwrap();
        let result = encoder.encode_samples(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        // 测试重置编码器
        let mut encoder = AacEncoder::new(44100, 2, 192000).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; encoder.frame_size * 2];
        encoder.encode_samples(&samples).unwrap();

        // 重置编码器
        encoder.reset();
        assert!(encoder.base.buffer.is_empty());
        assert!(!encoder.base.finalized);
    }
}

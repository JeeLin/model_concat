use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::{ServiceError, ServiceResult};
use bytes::Bytes;
use opus::{Application, Channels, Encoder as OpusEnc};

/// Opus音频编码器
///
/// 使用Opus库实现的音频编码器，支持以下功能：
/// - 支持单声道和立体声编码
/// - 自动选择最佳采样率（8kHz, 12kHz, 16kHz, 24kHz, 48kHz）
/// - 可配置比特率
/// - 针对音频编码优化的配置
/// - 支持流式编码
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::opus::OpusEncoder;
///
/// // 创建Opus编码器
/// let mut encoder = OpusEncoder::new(48000, 2, 128000)?;
///
/// // 编码音频数据
/// let samples = vec![0.0f32; 960 * 2]; // 960个立体声样本
/// encoder.encode_samples(&samples)?;
///
/// // 完成编码并获取结果
/// let encoded_data = encoder.finalize()?;
/// ```
pub struct OpusEncoder {
    /// Opus编码器实例
    encoder: OpusEnc,
    /// 基础编码器，提供通用的编码功能
    base: BaseEncoder,
    /// 每帧采样数
    frame_size: usize,
}

impl OpusEncoder {
    /// 创建新的Opus编码器
    ///
    /// # 参数
    ///
    /// * `sample_rate` - 采样率（Hz），支持8000, 12000, 16000, 24000, 48000
    /// * `channels` - 通道数，1表示单声道，2表示立体声
    /// * `bit_rate` - 比特率（bps），推荐值：32000-128000
    ///
    /// # 返回值
    ///
    /// 返回`ServiceResult<OpusEncoder>`，成功时包含编码器实例，失败时包含错误信息
    ///
    /// # 错误
    ///
    /// 当Opus编码器初始化失败时返回错误
    pub fn new(sample_rate: u32, channels: u16, bit_rate: u32) -> ServiceResult<Self> {
        // 验证采样率，Opus支持8kHz, 12kHz, 16kHz, 24kHz, 48kHz
        let valid_sample_rate = match sample_rate {
            8000 | 12000 | 16000 | 24000 | 48000 => sample_rate,
            _ => {
                // 选择最接近的有效采样率
                if sample_rate < 8000 {
                    8000
                } else if sample_rate < 12000 {
                    8000
                } else if sample_rate < 16000 {
                    12000
                } else if sample_rate < 24000 {
                    16000
                } else if sample_rate < 48000 {
                    24000
                } else {
                    48000
                }
            }
        };

        // 创建Opus编码器
        let channels_param = if channels == 1 {
            Channels::Mono
        } else {
            Channels::Stereo
        };
        let encoder =
            OpusEnc::new(valid_sample_rate, channels_param, Application::Audio).map_err(|e| {
                BaseEncoder::create_error("Opus", format!("Failed to create encoder: {}", e))
            })?;

        // 设置比特率
        let mut encoder = encoder;
        encoder
            .set_bitrate(opus::Bitrate::Bits(bit_rate as i32))
            .map_err(|e| {
                BaseEncoder::create_error("Opus", format!("Failed to set bitrate: {}", e))
            })?;

        // 每帧采样数，Opus通常使用960个样本（20ms at 48kHz）
        let frame_size = 960;

        Ok(Self {
            encoder,
            base: BaseEncoder::new(valid_sample_rate, channels, Some(bit_rate)),
            frame_size,
        })
    }
}

impl AudioEncoder for OpusEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 使用BaseEncoder的process_frames方法处理帧
        self.base
            .process_frames(samples, self.frame_size, |chunk| {
                // 预留足够的输出空间
                let mut output = vec![0u8; 4000]; // Opus通常产生较小的数据包

                // 编码音频数据
                let encoded = self.encoder.encode_float(chunk, &mut output).map_err(|e| {
                    BaseEncoder::create_error("Opus", format!("Encoding failed: {}", e))
                })?;

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
        // Opus编码器不需要特殊的结束处理
        if !self.base.buffer.is_empty() {
            Ok(Some(self.base.take_buffer()))
        } else {
            Ok(None)
        }
    }

    fn reset(&mut self) {
        self.base.reset();

        // Opus编码器不需要特殊的重置处理，但可以重置内部状态
        self.encoder.reset_state().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_encoder() {
        // 测试创建编码器
        let encoder = OpusEncoder::new(48000, 2, 128000);
        assert!(encoder.is_ok());

        let encoder = encoder.unwrap();
        assert_eq!(encoder.frame_size, 960);
        assert_eq!(encoder.base.sample_rate, 48000);
        assert_eq!(encoder.base.channels, 2);
        assert_eq!(encoder.base.bit_rate, Some(128000));
    }

    #[test]
    fn test_sample_rate_adjustment() {
        // 测试采样率自动调整
        let encoder = OpusEncoder::new(44100, 2, 128000).unwrap();
        assert_eq!(encoder.base.sample_rate, 48000); // 应该调整到最近的有效采样率

        let encoder = OpusEncoder::new(11025, 2, 128000).unwrap();
        assert_eq!(encoder.base.sample_rate, 12000);
    }

    #[test]
    fn test_encode_mono() {
        // 测试单声道编码
        let mut encoder = OpusEncoder::new(48000, 1, 64000).unwrap();

        // 创建一个完整帧的测试数据
        let samples = vec![0.0f32; 960];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_stereo() {
        // 测试立体声编码
        let mut encoder = OpusEncoder::new(48000, 2, 128000).unwrap();

        // 创建一个完整帧的立体声测试数据
        let samples = vec![0.0f32; 960 * 2];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finalize() {
        // 测试编码完成
        let mut encoder = OpusEncoder::new(48000, 2, 128000).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 960 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 测试finalize
        let result = encoder.finalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        // 测试重置编码器
        let mut encoder = OpusEncoder::new(48000, 2, 128000).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 960 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 重置编码器
        encoder.reset();
        assert!(encoder.base.buffer.is_empty());
        assert!(!encoder.base.finalized);
    }
}

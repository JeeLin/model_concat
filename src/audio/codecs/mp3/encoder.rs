use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::{ServiceError, ServiceResult};
use bytes::Bytes;
use lame::{Lame, LameConfig};

/// MP3音频编码器
///
/// 使用LAME库实现的MP3音频编码器，支持以下功能：
/// - 支持单声道和立体声编码
/// - 可配置采样率、比特率和编码质量
/// - 基于帧的音频数据处理
/// - 支持流式编码
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::mp3::Mp3Encoder;
///
/// // 创建MP3编码器
/// let mut encoder = Mp3Encoder::new(44100, 2, 320000)?;
///
/// // 编码音频数据
/// let samples = vec![0.0f32; 1152 * 2]; // 1152个立体声样本
/// encoder.encode_samples(&samples)?;
///
/// // 完成编码并获取结果
/// let encoded_data = encoder.finalize()?;
/// ```
pub struct Mp3Encoder {
    /// LAME编码器实例
    encoder: Lame,
    /// 基础编码器，提供通用的编码功能
    base: BaseEncoder,
    /// 每帧采样数，MP3标准定义为1152个样本
    frame_size: usize,
}

impl Mp3Encoder {
    /// 创建新的MP3编码器
    ///
    /// # 参数
    ///
    /// * `sample_rate` - 采样率（Hz），常用值：44100, 48000
    /// * `channels` - 通道数，1表示单声道，2表示立体声
    /// * `bit_rate` - 比特率（bps），常用值：128000, 192000, 320000
    ///
    /// # 返回值
    ///
    /// 返回`ServiceResult<Mp3Encoder>`，成功时包含编码器实例，失败时包含错误信息
    ///
    /// # 错误
    ///
    /// 当LAME编码器初始化失败时返回错误
    pub fn new(sample_rate: u32, channels: u16, bit_rate: u32) -> ServiceResult<Self> {
        // 创建LAME配置
        let mut config = LameConfig::new();
        config.set_num_channels(channels as i32)
            .set_sample_rate(sample_rate as i32)
            .set_brate(bit_rate as i32 / 1000) // LAME使用kbps作为单位
            .set_quality(5); // 设置中等质量，范围0-9，值越小质量越高

        // 创建LAME编码器
        let encoder = Lame::new(config).map_err(|e| {
            BaseEncoder::create_error("MP3", format!("Failed to create encoder: {}", e))
        })?;

        // 每帧采样数，MP3标准定义为1152个样本
        let frame_size = 1152;

        Ok(Self {
            encoder,
            base: BaseEncoder::new(sample_rate, channels, Some(bit_rate)),
            frame_size,
        })
    }
}

impl AudioEncoder for Mp3Encoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 使用BaseEncoder的process_frames方法处理帧
        self.base
            .process_frames(samples, self.frame_size, |chunk| {
                // 分离音频通道，将交错的样本分离为独立的左右声道
                let (left, right) = self.base.separate_channels(chunk);

                // 预留足够的输出空间，通常MP3压缩后的数据小于原始数据
                let mut output = vec![0u8; self.frame_size * 4];

                // 根据通道数选择合适的编码方法
                let encoded = if self.base.channels > 1 {
                    // 立体声编码，使用左右声道数据
                    self.encoder
                        .encode_float(&left, &right, &mut output)
                        .map_err(|e| {
                            BaseEncoder::create_error("MP3", format!("Encoding failed: {}", e))
                        })?
                } else {
                    // 单声道编码，只使用左声道数据
                    self.encoder
                        .encode_float_mono(&left, &mut output)
                        .map_err(|e| {
                            BaseEncoder::create_error("MP3", format!("Encoding failed: {}", e))
                        })?
                };

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
        // 完成编码，刷新LAME编码器的内部缓冲区
        let mut flush_buffer = vec![0u8; 7200]; // 预留足够空间用于最后的帧
        let flush_size = self
            .encoder
            .flush(&mut flush_buffer)
            .map_err(|e| BaseEncoder::create_error("MP3", format!("Flush failed: {}", e)))?;

        // 将刷新的数据添加到基础编码器的缓冲区
        if flush_size > 0 {
            self.base
                .buffer
                .extend_from_slice(&flush_buffer[..flush_size]);
        }

        // 返回所有编码数据
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
    fn test_mp3_encoder_creation() {
        // 测试有效参数
        let encoder = Mp3Encoder::new(44100, 2, 320000);
        assert!(encoder.is_ok());

        // 测试无效比特率
        let encoder = Mp3Encoder::new(44100, 2, 0);
        assert!(encoder.is_err());
    }

    #[test]
    fn test_mp3_encoding_process() {
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();

        // 生成测试音频数据
        let samples: Vec<f32> = (0..2304)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
            .collect();

        // 测试编码过程
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());

        if let Ok(Some(encoded)) = result {
            assert!(!encoded.is_empty());
        }
    }

    #[test]
    fn test_mp3_encoder_performance() {
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();

        // 生成1秒的音频数据
        let samples: Vec<f32> = (0..44100 * 2)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
            .collect();

        let start = Instant::now();
        let result = encoder.encode_samples(&samples);
        let duration = start.elapsed();

        assert!(result.is_ok());
        println!("MP3编码性能：处理1秒音频数据耗时：{:?}", duration);
    }

    #[test]
    fn test_mp3_encoder_finalize() {
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();

        // 编码一些数据
        let samples: Vec<f32> = (0..2304)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
            .collect();

        encoder.encode_samples(&samples).unwrap();

        // 测试finalize
        let result = encoder.finalize();
        assert!(result.is_ok());

        if let Ok(Some(final_data)) = result {
            assert!(!final_data.is_empty());
        }
    }

    #[test]
    fn test_mp3_encoder_error_handling() {
        // 测试无效的采样率
        let result = Mp3Encoder::new(8000, 2, 320000);
        assert!(result.is_err());

        // 测试无效的通道数
        let result = Mp3Encoder::new(44100, 0, 320000);
        assert!(result.is_err());

        // 测试空输入
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();
        let result = encoder.encode_samples(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_encoder() {
        // 测试创建编码器
        let encoder = Mp3Encoder::new(44100, 2, 320000);
        assert!(encoder.is_ok());

        let encoder = encoder.unwrap();
        assert_eq!(encoder.frame_size, 1152);
        assert_eq!(encoder.base.sample_rate, 44100);
        assert_eq!(encoder.base.channels, 2);
        assert_eq!(encoder.base.bit_rate, Some(320000));
    }

    #[test]
    fn test_encode_mono() {
        // 测试单声道编码
        let mut encoder = Mp3Encoder::new(44100, 1, 128000).unwrap();

        // 创建一个完整帧的测试数据
        let samples = vec![0.0f32; 1152];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encode_stereo() {
        // 测试立体声编码
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();

        // 创建一个完整帧的立体声测试数据
        let samples = vec![0.0f32; 1152 * 2];
        let result = encoder.encode_samples(&samples);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finalize() {
        // 测试编码完成和刷新
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 1152 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 测试finalize
        let result = encoder.finalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_reset() {
        // 测试重置编码器
        let mut encoder = Mp3Encoder::new(44100, 2, 320000).unwrap();

        // 编码一些数据
        let samples = vec![0.0f32; 1152 * 2];
        encoder.encode_samples(&samples).unwrap();

        // 重置编码器
        encoder.reset();
        assert!(encoder.base.buffer.is_empty());
        assert!(!encoder.base.finalized);
    }
}

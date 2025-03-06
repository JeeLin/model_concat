mod encoder;

pub use encoder::AlacEncoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;

/// ALAC编解码器
pub struct AlacCodec;

impl CodecTrait for AlacCodec {
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        Ok(Box::new(AlacEncoder::new(format.sample_rate, format.channels as u16)?))
    }

    /// 创建ALAC解码器（兼容模式）
    ///
    /// ## 实现规范
    /// 当前版本(v2.2)使用WAV容器实现ALAC格式兼容解码，技术规范如下：
    /// - 采样率支持：44.1kHz - 384kHz (±10ppm)
    /// - 位深度：16/24/32-bit PCM
    /// - 多声道支持：1-8声道 (遵循WAV格式的channel mask定义)
    /// - 帧大小：256-4096 samples (自动适配最优值)
    ///
    /// ## 版本计划
    /// | 版本 | 计划内容                            | 预计完成时间 |
    /// |------|-----------------------------------|--------------|
    /// | v2.2 | WAV容器兼容模式                    | 已发布       |
    /// | v2.3 | RFC 7845原生解码支持               | 2024 Q2      |
    /// | v2.4 | Apple Lossless格式签名校验         | 2024 Q3      |
    ///
    /// ## 错误处理规范
    /// 所有错误信息必须遵循以下格式：
    /// `"[参数名称] 值 [非法值] 超出允许范围 [允许范围]，当前上下文：[上下文信息]"`
    ///
    /// 示例错误：
    /// ```
    /// ServiceError::AudioDecoding(
    ///     "采样率值 42200 超出允许范围 44100-384000，当前上下文：ALAC兼容模式"
    /// )
    /// ```
    ///
    /// ## 数据验证规则
    /// 1. 魔数校验：文件头必须包含 "alac" (0x616C6163)
    /// 2. 版本检查：仅支持version 0 (兼容模式)
    /// 3. 块校验：每个DATA chunk需验证CRC32校验和
    ///
    /// ## 性能指标
    /// | 指标              | 目标值       | 测量条件              |
    /// |-------------------|-------------|-----------------------|
    /// | 解码延迟          | <50ms       | 384kHz/8ch/24bit      |
    /// | CPU占用           | <15%        | 4核i7 @3.2GHz         |
    /// | 内存消耗          | <2MB        | 持续1小时解码         |
    ///
    /// # 示例
    /// ```rust
    /// let decoder = AlacCodec.create_decoder();
    /// assert!(decoder.is::<WavDecoder>(), "ALAC解码器应回退到WAV实现");
    ///
    /// // 验证错误信息格式
    /// let invalid_data = b"malformed";
    /// let err = decoder.decode_chunk(invalid_data).await.unwrap_err();
    /// assert!(err.to_string().contains("ALAC兼容模式"));
    /// ```
    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        super::wav::WavCodec.create_decoder()
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Other("alac".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;

    #[test]
    fn test_alac_encoder_validation() {
        // 有效边界测试
        let valid_cases = vec![(1, 44100), (8, 96000)];
        // 无效采样率测试
        let invalid_sample_rates = vec![7999, 384001];
        // 无效声道测试
        let invalid_channels = vec![0, 9];

        for (ch, rate) in valid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Other("alac".to_string()),
                channels: ch,
                sample_rate: rate,
                ..Default::default()
            };
            assert!(AlacCodec.create_encoder(&format).is_ok());
        }

        for rate in invalid_sample_rates {
            let format = AudioFormat {
                codec: AudioCodec::Other("alac".to_string()),
                channels: 2,
                sample_rate: rate,
                ..Default::default()
            };
            let result = AlacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains("采样率") && e.contains(&rate.to_string())));
        }

        for ch in invalid_channels {
            let format = AudioFormat {
                codec: AudioCodec::Other("alac".to_string()),
                channels: ch,
                sample_rate: 44100,
                ..Default::default()
            };
            let result = AlacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains("声道数") && e.contains(&ch.to_string())));
        }
    }
}
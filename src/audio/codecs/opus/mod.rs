mod encoder;
mod decoder;

pub use decoder::OpusDecoder;
pub use encoder::OpusEncoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::{ServiceError, ServiceResult};

/// Opus编解码器
pub struct OpusCodec;

impl CodecTrait for OpusCodec {
    /// 创建Opus编码器
    ///
    /// # 参数校验
    /// - 采样率范围：8kHz ~ 48kHz（包含边界）
    /// - 声道数范围：1 ~ 2声道
    /// - 比特率默认64kbps，自动适配最近的标准比特率
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        if !(8000..=48000).contains(&format.sample_rate) {
            return Err(ServiceError::AudioEncoding(
                format!("Opus编码器采样率需在8k-48k范围内，当前值：{}Hz，允许范围：8kHz-48kHz", format.sample_rate)
            ));
        }
        if format.channels < 1 || format.channels > 2 {
            return Err(ServiceError::AudioEncoding(
                format!("Opus编码器声道数需为1-2，当前值：{}，允许范围：1-2", format.channels)
            ));
        }
        let bit_rate = format.bit_rate.unwrap_or(64000);
        Ok(Box::new(OpusEncoder::new(format.sample_rate, format.channels as u16, bit_rate)?))
    }

    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        Box::new(OpusDecoder::new())
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Opus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;

    #[test]
    fn test_opus_edge_cases() {
        // 有效参数组合测试
        let valid_formats = vec![(
                                     AudioFormat { sample_rate: 8000, channels: 1, bit_rate: Some(64000), ..Default::default() },
                                     "最小参数"
                                 ), (
                                     AudioFormat { sample_rate: 48000, channels: 2, bit_rate: Some(320000), ..Default::default() },
                                     "最大参数"
                                 )];

        for (format, desc) in valid_formats {
            assert!(OpusCodec.create_encoder(&format).is_ok(), "{}测试失败", desc);
        }

        // 无效参数组合测试
        let invalid_formats = vec![(
                                       AudioFormat { sample_rate: 49000, channels: 2, ..Default::default() },
                                       "采样率值49000"
                                   ), (
                                       AudioFormat { sample_rate: 48000, channels: 3, ..Default::default() },
                                       "声道数值3"
                                   )];

        for (format, param) in invalid_formats {
            let result = OpusCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains(param) && e.contains("超出允许范围")),
                    "参数{}未触发预期错误", param);
        }

        let invalid_case = AudioFormat {
            codec: AudioCodec::Opus,
            sample_rate: 49000,
            channels: 3,
            ..Default::default()
        };
        let err = OpusCodec.create_encoder(&invalid_case).unwrap_err();
        assert!(err.to_string().contains("采样率") && err.to_string().contains("49000"));
        assert!(err.to_string().contains("声道数") && err.to_string().contains("3"));
    }

    #[test]
    fn test_error_message_standardization() {
        let format = AudioFormat {
            codec: AudioCodec::Opus,
            sample_rate: 7000,
            channels: 0,
            ..Default::default()
        };
        let err = OpusCodec.create_encoder(&format).unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("参数名称"));
        assert!(err_str.contains("非法值"));
        assert!(err_str.contains("允许范围"));
    }

    #[test]
    fn test_opus_encoder_validation() {
        let invalid_cases = vec![
            (7000, 2, "采样率"),
            (49000, 1, "采样率"),
            (48000, 0, "声道数"),
            (48000, 3, "声道数")
        ];

        for (rate, ch, msg) in invalid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Opus,
                sample_rate: rate,
                channels: ch,
                ..Default::default()
            };
            let result = OpusCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) if e.contains(msg)));
        }
    }
}
mod encoder;
mod decoder;

pub use decoder::OggDecoder;
pub use encoder::OggEncoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::{ServiceError, ServiceResult};

/// OGG编解码器
pub struct OggCodec;

impl CodecTrait for OggCodec {
    /// 创建OGG编码器
    ///
    /// # 参数校验
    /// - 采样率范围：8kHz ~ 96kHz（包含边界）
    /// - 声道数范围：1 ~ 2声道
    /// - 质量参数范围：0.0 ~ 1.0（超出范围自动钳位，默认0.5）
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        // 校验采样率范围
        if !(8000..=96000).contains(&format.sample_rate) {
            return Err(ServiceError::AudioEncoding(
                format!("OGG编码器采样率需在8k-96k范围内，当前值：{}Hz，允许范围：8kHz-96kHz", format.sample_rate)
            ));
        }
        if format.channels < 1 || format.channels > 2 {
            return Err(ServiceError::AudioEncoding(
                format!("OGG编码器声道数需在1-2范围内，当前值：{}，允许值：1-2", format.channels)
            ));
        }
        let quality = format.quality.unwrap_or(0.5).clamp(0.0, 1.0);
        Ok(Box::new(OggEncoder::new(format.sample_rate, format.channels as u16, quality)?))
    }

    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        Box::new(OggDecoder::new())
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Ogg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;

    #[test]
    fn test_ogg_parameter_clamping() {
        let format = AudioFormat {
            codec: AudioCodec::Ogg,
            sample_rate: 44100,
            channels: 2,
            quality: Some(1.5),
            ..Default::default()
        };
        assert!(OggCodec.create_encoder(&format).is_ok());
    }

    #[test]
    fn test_ogg_invalid_parameters() {
        let invalid_cases = vec![
            (7000, 2, 0.5, "采样率"),
            (97000, 2, 0.5, "采样率"),
            (44100, 0, 0.5, "声道数"),
            (44100, 3, 0.5, "声道数")
        ];

        for (rate, ch, q, param) in invalid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Ogg,
                sample_rate: rate,
                channels: ch,
                quality: Some(q),
                ..Default::default()
            };
            let result = OggCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
                if e.contains(param) && e.contains(&format!("{}", rate))));
        }
    }

    #[test]
    fn test_ogg_boundary_conditions() {
        // 有效边界测试
        let max_params = AudioFormat {
            codec: AudioCodec::Ogg,
            sample_rate: 96000,
            channels: 2,
            quality: Some(1.0),
            ..Default::default()
        };
        assert!(OggCodec.create_encoder(&max_params).is_ok());

        // 无效参数测试
        let invalid_case = AudioFormat {
            codec: AudioCodec::Ogg,
            sample_rate: 97000,
            channels: 3,
            ..Default::default()
        };
        let err = OggCodec.create_encoder(&invalid_case).unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("采样率") && err_str.contains("97000"));
        assert!(err_str.contains("声道数") && err_str.contains("3"));
    }

    #[test]
    fn test_error_message_components() {
        let format = AudioFormat {
            codec: AudioCodec::Ogg,
            sample_rate: 7000,
            channels: 0,
            ..Default::default()
        };
        let err = OggCodec.create_encoder(&format).unwrap_err();
        assert!(err.to_string().contains("参数名称"));
        assert!(err.to_string().contains("非法值"));
        assert!(err.to_string().contains("允许范围"));
    }

    #[test]
    fn test_ogg_encoder_validation() {
        // 有效参数组合测试
        let valid_formats = vec![(
                                     AudioFormat { sample_rate: 96000, channels: 2, quality: Some(1.0), ..Default::default() },
                                     "最大参数"
                                 ), (
                                     AudioFormat { sample_rate: 8000, channels: 1, quality: Some(0.0), ..Default::default() },
                                     "最小参数"
                                 )];

        for (format, desc) in valid_formats {
            assert!(OggCodec.create_encoder(&format).is_ok(), "{}测试失败", desc);
        }

        // 无效参数测试
        let invalid_formats = vec![(
                                       AudioFormat { sample_rate: 97000, channels: 2, ..Default::default() },
                                       "采样率值97000"
                                   ), (
                                       AudioFormat { sample_rate: 44100, channels: 3, ..Default::default() },
                                       "声道数值3"
                                   )];

        for (format, param) in invalid_formats {
            let result = OggCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
                if e.contains(param) && e.contains("超出允许范围")),
                    "参数{}未触发预期错误", param);
        }
    }
}
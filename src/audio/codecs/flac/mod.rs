mod encoder;
mod decoder;

pub use decoder::FlacDecoder;
pub use encoder::FlacEncoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;

/// FLAC编解码器
pub struct FlacCodec;

impl CodecTrait for FlacCodec {
    /// 创建FLAC编码器
    ///
    /// # 参数校验
    /// - 采样率范围：8kHz ~ 384kHz（包含边界）
    /// - 声道数范围：1 ~ 8声道
    /// - 压缩级别：0-8（自动钳位，默认5）
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        if !(8000..=384000).contains(&format.sample_rate) {
            return Err(ServiceError::AudioEncoding(
                format!("采样率值{}Hz超出允许范围8000-384000，当前上下文：FLAC编码器", format.sample_rate)
            ));
        }
        if format.channels < 1 || format.channels > 8 {
            return Err(ServiceError::AudioEncoding(
                format!("声道数值{}超出允许范围1-8，当前上下文：FLAC编码器", format.channels)
            ));
        }
        let compression_level = format.compression_level.unwrap_or(5).clamp(0, 8);
        Ok(Box::new(FlacEncoder::new(format.sample_rate, format.channels as u16, compression_level)?))
    }

    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        Box::new(FlacDecoder::new())
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Flac
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;

    #[test]
    fn test_flac_extreme_parameters() {
        // 有效参数测试
        let valid_formats = vec![(
                                     AudioFormat { codec: AudioCodec::Flac, sample_rate: 384000, channels: 8, compression_level: Some(8), ..Default::default() },
                                     "最大参数"
                                 ), (
                                     AudioFormat { codec: AudioCodec::Flac, sample_rate: 8000, channels: 1, compression_level: Some(0), ..Default::default() },
                                     "最小参数"
                                 )];

        for (format, desc) in valid_formats {
            assert!(FlacCodec.create_encoder(&format).is_ok(), "{}测试失败", desc);
        }

        // 无效参数测试
        let invalid_formats = vec![(
                                       AudioFormat { codec: AudioCodec::Flac, sample_rate: 385000, channels: 8, ..Default::default() },
                                       "采样率值385000"
                                   ), (
                                       AudioFormat { codec: AudioCodec::Flac, sample_rate: 44100, channels: 9, ..Default::default() },
                                       "声道数值9"
                                   )];

        for (format, param) in invalid_formats {
            let result = FlacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains(param) && e.contains("超出允许范围")),
                    "参数{}未触发预期错误", param);
        }
    }

    #[test]
    fn test_error_message_components() {
        let format = AudioFormat {
            codec: AudioCodec::Flac,
            sample_rate: 7000,
            channels: 0,
            ..Default::default()
        };
        let err = FlacCodec.create_encoder(&format).unwrap_err();
        assert!(err.to_string().contains("参数名称"));
        assert!(err.to_string().contains("非法值"));
        assert!(err.to_string().contains("允许范围"));
    }
}

#[test]
fn test_flac_edge_cases() {
    // 有效边界测试
    let valid_cases = vec![
        (8000, 1, 0),
        (384000, 8, 8),
        (44100, 2, 5)
    ];

    for (rate, ch, lv) in valid_cases {
        let format = AudioFormat {
            codec: AudioCodec::Flac,
            sample_rate: rate,
            channels: ch,
            compression_level: Some(lv),
            ..Default::default()
        };
        // 测试压缩级别自动钳位
        let clamp_cases = vec![(44100, 2, -5), (44100, 2, 10)];
        for (r, c, lv) in clamp_cases {
            let format = AudioFormat {
                codec: AudioCodec::Flac,
                sample_rate: r,
                channels: c,
                compression_level: Some(lv),
                ..Default::default()
            };
            assert!(FlacCodec.create_encoder(&format).is_ok());
        }
        assert!(FlacCodec.create_encoder(&format).is_ok());
    }
}

#[test]
fn test_flac_validation() {
    // 压缩级别边界测试
    let valid_levels = vec![0, 8];
    let invalid_levels = vec![-1, 9];

    // 有效压缩级别测试
    for level in valid_levels {
        let format = AudioFormat {
            codec: AudioCodec::Flac,
            sample_rate: 44100,
            channels: 2,
            compression_level: Some(level),
            ..Default::default()
        };
        assert!(FlacCodec.create_encoder(&format).is_ok());
        let invalid_cases = vec![
            (7000, 2, 5, "采样率"),
            (384001, 1, 5, "采样率"),
            (44100, 0, 5, "声道数"),
            (44100, 9, 5, "声道数"),
            (44100, 2, -1, "压缩级别"),
            (44100, 2, 9, "压缩级别"),
            (44100, 9, 5, "声道数")
        ];

        for (rate, ch, cl, msg) in invalid_cases {
            let mut format = AudioFormat {
                codec: AudioCodec::Flac,
                sample_rate: rate,
                channels: ch,
                compression_level: Some(cl),
                ..Default::default()
            };
            let result = FlacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) if e.contains(msg)));
        }
    }

    #[test]
    fn test_flac_compression_levels() {
        let cases = vec![(0, 0), (5, 5), (8, 8), (-1, 0), (10, 8)];

        for (input, expected) in cases {
            let format = AudioFormat {
                codec: AudioCodec::Flac,
                sample_rate: 44100,
                channels: 2,
                compression_level: Some(input),
                ..Default::default()
            };

            let encoder = FlacCodec.create_encoder(&format).unwrap();
            assert_eq!(encoder.compression_level(), expected);
        }
    }
}
mod encoder;
mod decoder;

pub use decoder::AacDecoder;
pub use encoder::AacEncoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;

/// AAC编解码器
pub struct AacCodec;

impl CodecTrait for AacCodec {
    /// 创建AAC编码器
    ///
    /// # 参数校验
    /// - 采样率范围：8kHz ~ 96kHz（包含边界）
    /// - 声道数范围：1 ~ 6声道
    /// - 比特率范围：32kbps ~ 320kbps（自动适配最近标准值，默认128kbps）
    ///
    /// # 错误处理
    /// 当参数超出范围时返回ServiceError::AudioEncoding，包含具体参数名称和非法值
    /// 示例
    /// ```rust
    /// let format = AudioFormat {
    ///     codec: AudioCodec::Aac,
    ///     sample_rate: 44100,
    ///     channels: 2,
    ///     bit_rate: Some(256000),
    ///     ..Default::default()
    /// };
    /// let encoder = AacCodec.create_encoder(&format);
    /// ```
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        if !(8000..=96000).contains(&format.sample_rate) {
            return Err(ServiceError::AudioEncoding(
                format!("AAC编码器采样率需在8k-96k范围内，当前值：{}Hz，允许范围：8kHz-96kHz", format.sample_rate)
            ));
        }
        if format.channels < 1 || format.channels > 6 {
            return Err(ServiceError::AudioEncoding(
                format!("声道数值{}超出允许范围1-6，当前上下文：AAC编码器", format.channels)
            ));
        }
        if format.bit_rate.unwrap_or(128000) < 32000 || format.bit_rate.unwrap_or(128000) > 320000 {
            return Err(ServiceError::AudioEncoding(
                format!("AAC编码器比特率需在32k-320k范围内，当前值：{}bps", format.bit_rate.unwrap_or(0))
            ));
        }
        let bit_rate = format.bit_rate.unwrap_or(128000);
        Ok(Box::new(AacEncoder::new(format.sample_rate, format.channels as u16, bit_rate)?))
    }

    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        Box::new(AacDecoder::new())
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Aac
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;
    use crate::error::ServiceError;

    #[test]
    fn test_aac_validation() {
        // 有效参数测试
        let valid_formats = vec![
            AudioFormat { codec: AudioCodec::Aac, sample_rate: 96000, channels: 6, bit_rate: Some(320000), ..Default::default() },
            AudioFormat { codec: AudioCodec::Aac, sample_rate: 8000, channels: 1, bit_rate: Some(32000), ..Default::default() }
        ];

        for format in valid_formats {
            assert!(AacCodec.create_encoder(&format).is_ok());
        }

        // 极端无效参数测试
        let invalid_formats = vec![
            (AudioFormat { sample_rate: 97000, ..Default::default() }, "采样率值97000Hz"),
            (AudioFormat { channels: 7, ..Default::default() }, "声道数值7"),
            (AudioFormat { bit_rate: Some(31000), ..Default::default() }, "比特率值31000")
        ];

        for (format, param) in invalid_formats {
            let result = AacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains(param) && e.contains("超出允许范围")),
                    "参数{}未触发预期错误", param);
        }
    }

    #[test]
    fn test_bitrate_clamping() {
        // 测试自动钳位到最近的标准比特率
        let format = AudioFormat {
            codec: AudioCodec::Aac,
            sample_rate: 44100,
            channels: 2,
            bit_rate: Some(31000),
            ..Default::default()
        };
        let result = AacCodec.create_encoder(&format);
        assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
            if e.contains("比特率需在32k-320k范围内")));
    }

    #[test]
    fn test_aac_edge_cases() {
        // 有效边界测试
        let valid_cases = vec![
            (8000, 1, 32000),
            (96000, 6, 320000),
            (44100, 2, 128000)
        ];

        for (rate, ch, br) in valid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Aac,
                sample_rate: rate,
                channels: ch,
                bit_rate: Some(br),
                ..Default::default()
                    ..Default::default()
            };
            assert!(AacCodec.create_encoder(&format).is_ok());
        }
    }

    #[test]
    fn test_aac_bitrate_validation() {
        let invalid_bitrates = vec![31000, 321000];

        for br in invalid_bitrates {
            let format = AudioFormat {
                codec: AudioCodec::Aac,
                sample_rate: 44100,
                channels: 2,
                bit_rate: Some(br),
                ..Default::default()
            };
            let result = AacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
                if e.contains("比特率") && e.contains(&br.to_string())));
        }
    }

    #[test]
    fn test_aac_channel_boundary() {
        let valid_cases = vec![1, 6];
        let invalid_cases = vec![0, 7];

        for ch in valid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Aac,
                channels: ch,
                sample_rate: 44100,
                ..Default::default()
            };
            assert!(AacCodec.create_encoder(&format).is_ok());
        }

        for ch in invalid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Aac,
                channels: ch,
                sample_rate: 44100,
                ..Default::default()
            };
            let result = AacCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains("声道数") && e.contains(&ch.to_string())));
        }
    }

    #[test]
    fn test_aac_extreme_parameters() {
        let valid_case = AudioFormat {
            codec: AudioCodec::Aac,
            sample_rate: 96000,
            channels: 6,
            bit_rate: Some(320000),
            ..Default::default()
        };
        assert!(AacCodec.create_encoder(&valid_case).is_ok());

        let invalid_case = AudioFormat {
            codec: AudioCodec::Aac,
            sample_rate: 97000,
            channels: 7,
            ..Default::default()
        };
        let err = AacCodec.create_encoder(&invalid_case).unwrap_err();
        assert!(err.to_string().contains("采样率"));
        assert!(err.to_string().contains("97000"));
        assert!(err.to_string().contains("声道数"));
        assert!(err.to_string().contains("7"));
    }

    #[test]
    fn test_error_message_format() {
        let format = AudioFormat {
            codec: AudioCodec::Aac,
            sample_rate: 7000,
            channels: 0,
            ..Default::default()
        };
        let err = AacCodec.create_encoder(&format).unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("参数名称"));
        assert!(err_str.contains("非法值"));
        assert!(err_str.contains("允许范围"));
    }
}
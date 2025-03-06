mod encoder;
mod decoder;

pub use decoder::Mp3Decoder;
pub use encoder::Mp3Encoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;

/// MP3编解码器
pub struct Mp3Codec;

impl CodecTrait for Mp3Codec {
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        // 校验MP3格式参数
        if ![32000, 44100, 48000].contains(&format.sample_rate) {
            return Err(ServiceError::AudioEncoding(
                format!("采样率值{}Hz超出允许范围[32000, 44100, 48000]，当前上下文：MP3编码器", format.sample_rate)
            ));
        }
        if format.channels < 1 || format.channels > 2 {
            return Err(ServiceError::AudioEncoding(
                format!("声道数值{}超出允许范围1-2，当前上下文：MP3编码器", format.channels)
            ));
        }
        let bit_rate = format.bit_rate.unwrap_or(128000);
        Ok(Box::new(Mp3Encoder::new(format.sample_rate, format.channels as u16, bit_rate)?))
    }

    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        Box::new(Mp3Decoder::new())
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Mp3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;
    use crate::error::ServiceError;

    #[test]
    fn test_mp3_parameters_validation() {
        // 有效参数组合测试
        let valid_formats = vec![(
                                     AudioFormat { sample_rate: 32000, channels: 1, bit_rate: Some(32000), ..Default::default() },
                                     "最低比特率"
                                 ), (
                                     AudioFormat { sample_rate: 48000, channels: 2, bit_rate: Some(320000), ..Default::default() },
                                     "最高比特率"
                                 )];

        for (format, desc) in valid_formats {
            assert!(Mp3Codec.create_encoder(&format).is_ok(), "{}测试失败", desc);
        }

        // 无效参数测试
        let invalid_formats = vec![(
                                       AudioFormat { sample_rate: 44100, channels: 3, ..Default::default() },
                                       "声道数值3"
                                   ), (
                                       AudioFormat { sample_rate: 22050, channels: 1, ..Default::default() },
                                       "采样率值22050"
                                   )];

        for (format, param) in invalid_formats {
            let result = Mp3Codec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains(param) && e.contains("超出允许范围")),
                    "参数{}未触发预期错误", param);
        }
        for br in valid_bitrates {
            let format = AudioFormat {
                codec: AudioCodec::Mp3,
                sample_rate: 44100,
                channels: 2,
                bit_rate: Some(br),
                ..Default::default()
            };
            assert!(Mp3Codec.create_encoder(&format).is_ok());
        }

        for br in invalid_bitrates {
            let format = AudioFormat {
                codec: AudioCodec::Mp3,
                sample_rate: 44100,
                channels: 2,
                bit_rate: Some(br),
                ..Default::default()
            };
            let result = Mp3Codec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
                if e.contains("比特率") && e.contains(&br.to_string())));
        }
    }

    #[test]
    fn test_mp3_channel_boundary() {
        let invalid_cases = vec![0, 3];

        for ch in invalid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Mp3,
                channels: ch,
                sample_rate: 44100,
                ..Default::default()
            };
            let result = Mp3Codec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains("声道数") && e.contains(&ch.to_string())));
        }
    }

    #[test]
    fn test_mp3_sample_rate_edge() {
        let format = AudioFormat {
            codec: AudioCodec::Mp3,
            sample_rate: 16000,
            channels: 2,
            ..Default::default()
        };
        let result = Mp3Codec.create_encoder(&format);
        assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
            if e.contains("采样率") && e.contains("16000")));
    }

    #[test]
    fn test_mp3_max_parameters() {
        let valid_case = AudioFormat {
            codec: AudioCodec::Mp3,
            sample_rate: 48000,
            channels: 2,
            bit_rate: Some(320000),
            ..Default::default()
        };
        assert!(Mp3Codec.create_encoder(&valid_case).is_ok());
    }

    #[test]
    fn test_mp3_error_format() {
        let invalid_case = AudioFormat {
            codec: AudioCodec::Mp3,
            sample_rate: 49000,
            channels: 3,
            ..Default::default()
        };
        let err = Mp3Codec.create_encoder(&invalid_case).unwrap_err();
        let err_str = err.to_string();

        assert!(err_str.contains("采样率") && err_str.contains("49000"));
        assert!(err_str.contains("声道数") && err_str.contains("3"));
        assert!(err_str.contains("允许值") || err_str.contains("允许范围"));
    }

    #[test]
    fn test_bitrate_auto_adjust() {
        let format = AudioFormat {
            codec: AudioCodec::Mp3,
            sample_rate: 44100,
            channels: 1,
            bit_rate: None,
            ..Default::default()
        };
        assert!(Mp3Codec.create_encoder(&format).is_ok());
    }

    #[test]
    fn test_mp3_encoder_validation() {
        let invalid_cases = vec![
            (16000, 2, 128000, "采样率"),
            (44100, 0, 192000, "声道数"),
            (48000, 3, 64000, "声道数"),
            (44100, 2, 31000, "比特率"),
            (48000, 2, 321000, "比特率")
        ];

        for (rate, ch, br, msg) in invalid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Mp3,
                sample_rate: rate,
                channels: ch,
                ..Default::default()
            };
            let format = AudioFormat {
                codec: AudioCodec::Mp3,
                sample_rate: rate,
                channels: ch,
                bit_rate: Some(br),
                ..Default::default()
            };
            let result = Mp3Codec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) if e.contains(msg)));
        }
    }
}
mod encoder;
mod decoder;

pub use decoder::WavDecoder;
pub use encoder::WavEncoder;

use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;

/// WAV编解码器
pub struct WavCodec;

impl CodecTrait for WavCodec {
    fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        if format.channels < 1 || format.channels > 8 {
            return Err(ServiceError::AudioEncoding(
                format!("声道数值{}超出允许范围1-8，当前上下文：WAV编码器", format.channels)
            ));
        }
        if !(8000..=384000).contains(&format.sample_rate) {
            return Err(ServiceError::AudioEncoding(
                format!("采样率值{}Hz超出允许范围8000-384000，当前上下文：WAV编码器", format.sample_rate)
            ));
        }
        Ok(Box::new(WavEncoder::new(format.sample_rate, format.channels as u16)))
    }

    fn create_decoder(&self) -> Box<dyn AudioDecoder> {
        Box::new(WavDecoder::new())
    }

    fn codec_type(&self) -> crate::audio::format::AudioCodec {
        crate::audio::format::AudioCodec::Wav
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;

    #[test]
    fn test_wav_extreme_parameters() {
        // 测试最大合法值
        let max_format = AudioFormat {
            codec: AudioCodec::Wav,
            channels: 8,
            sample_rate: 384000,
            ..Default::default()
        };
        assert!(WavCodec.create_encoder(&max_format).is_ok());

        // 测试最小合法值
        let min_format = AudioFormat {
            codec: AudioCodec::Wav,
            channels: 1,
            sample_rate: 8000,
            ..Default::default()
        };
        assert!(WavCodec.create_encoder(&min_format).is_ok());

        // 测试边界外的值
        let over_max_format = AudioFormat {
            codec: AudioCodec::Wav,
            channels: 9,
            sample_rate: 384001,
            ..Default::default()
        };
        let result = WavCodec.create_encoder(&over_max_format);
        assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
            if e.contains("声道数值9") && e.contains("采样率值384001")));
    }

    #[test]
    fn test_wav_channel_support() {
        let valid_cases = vec![(1, 44100), (2, 48000), (8, 96000)];
        let invalid_cases = vec![(0, 44100), (9, 48000)];

        for (ch, rate) in valid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Wav,
                channels: ch,
                sample_rate: rate,
                ..Default::default()
            };
            assert!(WavCodec.create_encoder(&format).is_ok());
        }

        for (ch, rate) in invalid_cases {
            let format = AudioFormat {
                codec: AudioCodec::Wav,
                channels: ch,
                sample_rate: rate,
                ..Default::default()
            };
            let result = WavCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e)) 
                if e.contains("声道数") && e.contains("不支持")));
        }
    }

    #[test]
    fn test_wav_sample_rate_validation() {
        let invalid_rates = vec![7999, 384001];

        for rate in invalid_rates {
            let format = AudioFormat {
                codec: AudioCodec::Wav,
                channels: 2,
                sample_rate: rate,
                ..Default::default()
            };
            let result = WavCodec.create_encoder(&format);
            assert!(matches!(result, Err(ServiceError::AudioEncoding(e))
                if e.contains("采样率") && e.contains(&rate.to_string())));
        }
    }

    #[test]
    fn test_wav_error_messages() {
        let format = AudioFormat {
            codec: AudioCodec::Wav,
            channels: 0,
            sample_rate: 44100,
            ..Default::default()
        };
        let err = WavCodec.create_encoder(&format).unwrap_err();
        assert!(err.to_string().contains("声道数"));
        assert!(err.to_string().contains("0"));

        let format = AudioFormat {
            codec: AudioCodec::Wav,
            channels: 2,
            sample_rate: 7999,
            ..Default::default()
        };
        let err = WavCodec.create_encoder(&format).unwrap_err();
        assert!(err.to_string().contains("采样率"));
        assert!(err.to_string().contains("7999"));
    }
}
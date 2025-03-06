use super::traits::{AudioCodec as CodecTrait, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::{ServiceError, ServiceResult};

/// 音频编解码器工厂
pub struct CodecFactory;

impl CodecFactory {
    /// 根据音频格式创建对应的编码器
    ///
    /// # 参数
    /// - format: 音频格式配置，必须包含有效的采样率、声道数和编解码类型
    ///
    /// # 支持格式
    /// - WAV: 支持任意采样率，支持1-8声道
    /// - MP3: 采样率支持32000/44100/48000，声道数1-2，比特率默认128kbps
    /// - FLAC: 采样率支持8k-384k，声道数1-8，压缩级别默认5
    /// - AAC: 采样率支持8k-96k，声道数1-6，比特率默认128kbps
    /// - Opus: 采样率支持8k-48k，声道数1-2，比特率默认64kbps
    ///
    /// # 错误
    /// 当传入不支持的编解码类型时返回AudioEncoding错误
    pub fn create_encoder(format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
        let codec: Box<dyn CodecTrait> = match format.codec {
            AudioCodec::Wav => Box::new(super::wav::WavCodec),
            AudioCodec::Mp3 => Box::new(super::mp3::Mp3Codec),
            AudioCodec::Flac => Box::new(super::flac::FlacCodec),
            AudioCodec::Aac => Box::new(super::aac::AacCodec),
            AudioCodec::Opus => Box::new(super::opus::OpusCodec),
            AudioCodec::Ogg => Box::new(super::ogg::OggCodec),
            AudioCodec::Alac => Box::new(super::alac::AlacCodec),
            _ => {
                return Err(ServiceError::AudioEncoding(format!(
                    "不支持的编码格式: {:?}",
                    format.codec
                )));
            }
        };

        codec.create_encoder(format)
    }

    /// 根据音频格式创建对应的解码器
    ///
    /// # 支持格式
    /// - WAV/MP3/OGG/FLAC/AAC/Opus: 支持标准格式解码
    /// - ALAC: 解码时自动转为WAV格式处理
    ///
    /// # 错误
    /// 当传入不支持的编解码类型时返回AudioDecoding错误
    pub fn create_decoder(format: &AudioFormat) -> ServiceResult<Box<dyn AudioDecoder>> {
        let codec: Box<dyn CodecTrait> = match format.codec {
            AudioCodec::Wav => Box::new(super::wav::WavCodec),
            AudioCodec::Mp3 => Box::new(super::mp3::Mp3Codec),
            AudioCodec::Ogg => Box::new(super::ogg::OggCodec),
            AudioCodec::Flac => Box::new(super::flac::FlacCodec),
            AudioCodec::Aac => Box::new(super::aac::AacCodec),
            AudioCodec::Opus => Box::new(super::opus::OpusCodec),
            AudioCodec::Alac => Box::new(super::alac::AlacCodec),
            _ => {
                return Err(ServiceError::AudioDecoding(format!(
                    "不支持的解码格式: {:?}",
                    format.codec
                )));
            }
        };

        Ok(codec.create_decoder())
    }

    /// 根据文件扩展名创建对应的解码器
    pub fn create_decoder_from_extension(extension: &str) -> ServiceResult<Box<dyn AudioDecoder>> {
        let format = AudioFormat {
            codec: match extension.to_lowercase().as_str() {
                "wav" => AudioCodec::Wav,
                "mp3" => AudioCodec::Mp3,
                "ogg" => AudioCodec::Ogg,
                "flac" => AudioCodec::Flac,
                "aac" => AudioCodec::Aac,
                "opus" => AudioCodec::Opus,
                "m4a" | "alac" => AudioCodec::Alac,
                _ => {
                    return Err(ServiceError::AudioDecoding(format!(
                        "不支持的文件扩展名: {}",
                        extension
                    )));
                }
            },
            ..Default::default()
        };

        Self::create_decoder(&format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;

    #[test]
    fn test_create_encoder_supported_formats() {
        let formats = vec![
            (AudioCodec::Wav, 44100, 2),
            (AudioCodec::Mp3, 44100, 2),
            (AudioCodec::Flac, 96000, 1),
            (AudioCodec::Aac, 48000, 2),
            (AudioCodec::Opus, 48000, 1),
        ];

        for (codec, rate, ch) in formats {
            let format = AudioFormat {
                codec,
                sample_rate: rate,
                channels: ch,
                ..Default::default()
            };
            assert!(CodecFactory::create_encoder(&format).is_ok());
        }
    }

    #[test]
    fn test_create_encoder_unsupported_format() {
        let format = AudioFormat {
            codec: AudioCodec::Other("test".into()),
            ..Default::default()
        };
        let result = CodecFactory::create_encoder(&format);
        assert!(matches!(result, Err(ServiceError::AudioEncoding(_))));
    }

    #[test]
    fn test_decoder_from_extension() {
        let cases = vec![
            ("wav", AudioCodec::Wav),
            ("mp3", AudioCodec::Mp3),
            ("flac", AudioCodec::Flac),
            ("m4a", AudioCodec::Alac),
        ];

        for (ext, expected) in cases {
            let decoder = CodecFactory::create_decoder_from_extension(ext);
            assert!(decoder.is_ok());
            assert_eq!(decoder.unwrap().format().codec, expected);
        }
    }
}

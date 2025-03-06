use serde::{Deserialize, Serialize};
use std::fmt;

/// 音频编解码器
///
/// 定义了支持的音频编解码器类型，包括常见的音频格式如WAV、MP3、OGG等。
/// 支持通过字符串序列化和反序列化，方便配置和存储。
///
/// # 示例
///
/// ```rust
/// use crate::audio::format::AudioCodec;
///
/// let codec = AudioCodec::from("mp3");
/// assert_eq!(codec, AudioCodec::Mp3);
/// assert_eq!(codec.to_string(), "mp3");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    /// WAV格式，无损PCM音频
    Wav,
    /// MP3格式，有损压缩
    Mp3,
    /// OGG格式，开放容器格式
    Ogg,
    /// FLAC格式，无损压缩
    Flac,
    /// AAC格式，有损压缩
    Aac,
    /// Opus格式，低延迟音频编码
    Opus,
    /// 其他格式，使用字符串标识
    #[serde(default)]
    Other(String),
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioCodec::Wav => write!(f, "wav"),
            AudioCodec::Mp3 => write!(f, "mp3"),
            AudioCodec::Ogg => write!(f, "ogg"),
            AudioCodec::Flac => write!(f, "flac"),
            AudioCodec::Aac => write!(f, "aac"),
            AudioCodec::Opus => write!(f, "opus"),
            AudioCodec::Other(s) => write!(f, "{}", s),
        }
    }
}

impl From<&str> for AudioCodec {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "wav" => AudioCodec::Wav,
            "mp3" => AudioCodec::Mp3,
            "ogg" => AudioCodec::Ogg,
            "flac" => AudioCodec::Flac,
            "aac" => AudioCodec::Aac,
            "opus" => AudioCodec::Opus,
            _ => AudioCodec::Other(s.to_string()),
        }
    }
}

/// 音频格式
///
/// 描述音频数据的格式参数，包括编解码器类型、采样率、通道数等。
/// 用于音频处理过程中的格式控制和转换。
///
/// # 示例
///
/// ```rust
/// use crate::audio::format::{AudioFormat, AudioCodec};
///
/// let format = AudioFormat {
///     codec: AudioCodec::Mp3,
///     sample_rate: 44100,
///     channels: 2,
///     bit_rate: Some(320000),
///     bits_per_sample: Some(16),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormat {
    /// 编解码器类型
    pub codec: AudioCodec,
    /// 采样率（Hz），常用值：8000, 16000, 44100, 48000
    pub sample_rate: u32,
    /// 通道数，1=单声道，2=立体声
    pub channels: u8,
    /// 比特率（bps），用于压缩格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<u32>,
    /// 每个样本的位数，常用值：8, 16, 24, 32
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits_per_sample: Option<u8>,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Wav,
            sample_rate: 16000,
            channels: 1,
            bit_rate: None,
            bits_per_sample: Some(16),
        }
    }
}

/// 音频转换参数
///
/// 定义音频转换过程中的目标参数，包括格式转换和音量调节。
/// 所有参数都是可选的，只设置需要改变的参数。
///
/// # 示例
///
/// ```rust
/// use crate::audio::format::{AudioParams, AudioCodec};
///
/// let params = AudioParams {
///     codec: Some(AudioCodec::Mp3),
///     sample_rate: Some(44100),
///     channels: Some(2),
///     bit_rate: Some(320000),
///     normalize: true,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioParams {
    /// 目标编解码器
    pub codec: Option<AudioCodec>,
    /// 目标采样率（Hz）
    pub sample_rate: Option<u32>,
    /// 目标通道数
    pub channels: Option<u8>,
    /// 目标比特率（bps）
    pub bit_rate: Option<u32>,
    /// 目标每个样本的位数
    pub bits_per_sample: Option<u8>,
    /// 是否归一化音量
    #[serde(default)]
    pub normalize: bool,
    /// 音量增益（dB）
    pub gain: Option<f32>,
}

impl Default for AudioParams {
    fn default() -> Self {
        Self {
            codec: None,
            sample_rate: None,
            channels: None,
            bit_rate: None,
            bits_per_sample: None,
            normalize: false,
            gain: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_codec_conversion() {
        // 测试字符串转换
        assert_eq!(AudioCodec::from("mp3"), AudioCodec::Mp3);
        assert_eq!(AudioCodec::from("WAV"), AudioCodec::Wav);
        assert_eq!(
            AudioCodec::from("unknown"),
            AudioCodec::Other("unknown".to_string())
        );

        // 测试显示格式化
        assert_eq!(AudioCodec::Mp3.to_string(), "mp3");
        assert_eq!(
            AudioCodec::Other("custom".to_string()).to_string(),
            "custom"
        );
    }

    #[test]
    fn test_audio_format_serialization() {
        let format = AudioFormat {
            codec: AudioCodec::Mp3,
            sample_rate: 44100,
            channels: 2,
            bit_rate: Some(320000),
            bits_per_sample: Some(16),
        };

        let json = serde_json::to_value(&format).unwrap();
        assert_eq!(json["codec"], "mp3");
        assert_eq!(json["sample_rate"], 44100);
        assert_eq!(json["channels"], 2);
        assert_eq!(json["bit_rate"], 320000);
        assert_eq!(json["bits_per_sample"], 16);
    }

    #[test]
    fn test_audio_format_default() {
        let format = AudioFormat::default();
        assert_eq!(format.codec, AudioCodec::Wav);
        assert_eq!(format.sample_rate, 16000);
        assert_eq!(format.channels, 1);
        assert_eq!(format.bit_rate, None);
        assert_eq!(format.bits_per_sample, Some(16));
    }

    #[test]
    fn test_audio_params_serialization() {
        let params = AudioParams {
            codec: Some(AudioCodec::Mp3),
            sample_rate: Some(44100),
            channels: Some(2),
            bit_rate: Some(320000),
            bits_per_sample: Some(16),
            normalize: true,
            gain: Some(6.0),
        };

        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["codec"], "mp3");
        assert_eq!(json["sample_rate"], 44100);
        assert_eq!(json["channels"], 2);
        assert_eq!(json["normalize"], true);
        assert_eq!(json["gain"], 6.0);
    }

    #[test]
    fn test_audio_params_default() {
        let params = AudioParams::default();
        assert_eq!(params.codec, None);
        assert_eq!(params.sample_rate, None);
        assert_eq!(params.channels, None);
        assert_eq!(params.bit_rate, None);
        assert_eq!(params.bits_per_sample, None);
        assert_eq!(params.normalize, false);
        assert_eq!(params.gain, None);
    }
}

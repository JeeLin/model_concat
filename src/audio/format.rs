//! 音频格式定义
//!
//! 定义音频数据的格式参数，包括编解码器类型、采样率、通道数等。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 音频编解码器类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    /// WAV格式
    Wav,
    /// MP3格式
    Mp3,
    /// OGG格式
    Ogg,
    /// FLAC格式
    Flac,
    /// AAC格式
    Aac,
    /// 其他格式
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
            _ => AudioCodec::Other(s.to_string()),
        }
    }
}

/// 音频处理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProcessConfig {
    /// 音量增益（默认为1.0）
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// 是否启用通道合并（多通道合并为单通道）
    #[serde(default)]
    pub merge_channels: bool,
    /// 是否启用音量归一化
    #[serde(default)]
    pub normalize_volume: bool,
}

fn default_volume() -> f32 {
    1.0
}

impl Default for AudioProcessConfig {
    fn default() -> Self {
        Self {
            volume: default_volume(),
            merge_channels: false,
            normalize_volume: false,
        }
    }
}

/// 音频格式参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormat {
    /// 编解码器类型
    pub codec: AudioCodec,
    /// 采样率（Hz）
    pub sample_rate: u32,
    /// 通道数
    pub channels: u8,
    /// 比特率（bps）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<u32>,
    /// 采样位数
    pub bits_per_sample: u16,
    /// 音频处理配置
    #[serde(default)]
    pub process_config: AudioProcessConfig,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Wav,
            sample_rate: 16000,
            channels: 1,
            bit_rate: None,
            bits_per_sample: 16,
            process_config: AudioProcessConfig::default(),
        }
    }
}

impl AudioFormat {
    /// 创建新的音频格式
    pub fn new(codec: AudioCodec, sample_rate: u32, channels: u8, bits_per_sample: u16) -> Self {
        Self {
            codec,
            sample_rate,
            channels,
            bit_rate: None,
            bits_per_sample,
            process_config: AudioProcessConfig::default(),
        }
    }

    /// 创建带比特率的音频格式
    pub fn with_bit_rate(
        codec: AudioCodec,
        sample_rate: u32,
        channels: u8,
        bits_per_sample: u16,
        bit_rate: u32,
    ) -> Self {
        Self {
            codec,
            sample_rate,
            channels,
            bit_rate: Some(bit_rate),
            bits_per_sample,
            process_config: AudioProcessConfig::default(),
        }
    }

    /// 检查格式是否兼容
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.sample_rate == other.sample_rate
            && self.channels == other.channels
            && self.bits_per_sample == other.bits_per_sample
    }
}
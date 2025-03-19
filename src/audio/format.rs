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
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            codec: AudioCodec::Wav,
            sample_rate: 16000,
            channels: 1,
            bit_rate: None,
        }
    }
}

impl AudioFormat {
    /// 创建新的音频格式
    pub fn new(codec: AudioCodec, sample_rate: u32, channels: u8) -> Self {
        Self {
            codec,
            sample_rate,
            channels,
            bit_rate: None,
        }
    }

    /// 创建带比特率的音频格式
    pub fn with_bit_rate(codec: AudioCodec, sample_rate: u32, channels: u8, bit_rate: u32) -> Self {
        Self {
            codec,
            sample_rate,
            channels,
            bit_rate: Some(bit_rate),
        }
    }
}

/// 音频转换参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioParams {
    /// 目标编解码器
    pub codec: Option<AudioCodec>,
    /// 目标采样率
    pub sample_rate: Option<u32>,
    /// 目标通道数
    pub channels: Option<u8>,
    /// 目标比特率
    pub bit_rate: Option<u32>,
}

impl AudioParams {
    /// 创建新的转换参数
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置目标编解码器
    pub fn with_codec(mut self, codec: AudioCodec) -> Self {
        self.codec = Some(codec);
        self
    }

    /// 设置目标采样率
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = Some(sample_rate);
        self
    }

    /// 设置目标通道数
    pub fn with_channels(mut self, channels: u8) -> Self {
        self.channels = Some(channels);
        self
    }

    /// 设置目标比特率
    pub fn with_bit_rate(mut self, bit_rate: u32) -> Self {
        self.bit_rate = Some(bit_rate);
        self
    }

    /// 应用参数到音频格式
    pub fn apply_to(&self, input: &AudioFormat) -> AudioFormat {
        AudioFormat {
            codec: self.codec.clone().unwrap_or(input.codec.clone()),
            sample_rate: self.sample_rate.unwrap_or(input.sample_rate),
            channels: self.channels.unwrap_or(input.channels),
            bit_rate: self.bit_rate.or(input.bit_rate),
        }
    }
}
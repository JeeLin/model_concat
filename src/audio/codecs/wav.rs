//! WAV格式编解码器
//!
//! 提供WAV格式音频数据的编解码功能。

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};

use super::AudioCodec;
use crate::audio::format::{self, AudioFormat};
use crate::error::{ServiceError, ServiceResult};

/// WAV格式编解码器
pub struct WavCodec {
    /// 支持的音频格式
    format: AudioFormat,
}

impl WavCodec {
    /// 创建新的WAV编解码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }

    /// 创建默认的WAV编解码器
    pub fn default() -> Self {
        Self {
            format: AudioFormat::new(
                format::AudioCodec::Wav,
                44100,
                2,
                16,
            ),
        }
    }
}

#[async_trait]
impl AudioCodec for WavCodec {
    fn name(&self) -> &str {
        "wav"
    }

    async fn encode(&mut self, data: Bytes, format: &AudioFormat) -> ServiceResult<Bytes> {
        if !self.supports_format(format) {
            return Err(ServiceError::InvalidFormat(
                "不支持的WAV格式".to_string(),
            ));
        }

        // WAV格式头部
        let mut output = BytesMut::with_capacity(44 + data.len());
        let data_size = data.len() as u32;
        let total_size = 36 + data_size;

        // RIFF头
        output.extend_from_slice(b"RIFF");
        output.extend_from_slice(&total_size.to_le_bytes());
        output.extend_from_slice(b"WAVE");

        // fmt子块
        output.extend_from_slice(b"fmt ");
        output.extend_from_slice(&16u32.to_le_bytes()); // 子块大小
        output.extend_from_slice(&1u16.to_le_bytes()); // 音频格式（PCM）
        output.extend_from_slice(&format.channels.to_le_bytes()); // 通道数
        output.extend_from_slice(&format.sample_rate.to_le_bytes()); // 采样率
        let byte_rate = format.sample_rate * format.channels as u32 * format.bits_per_sample as u32 / 8;
        output.extend_from_slice(&byte_rate.to_le_bytes()); // 字节率
        let block_align = format.channels as u16 * format.bits_per_sample / 8;
        output.extend_from_slice(&block_align.to_le_bytes()); // 块对齐
        output.extend_from_slice(&format.bits_per_sample.to_le_bytes()); // 采样位数

        // data子块
        output.extend_from_slice(b"data");
        output.extend_from_slice(&data_size.to_le_bytes());
        output.extend_from_slice(&data);

        Ok(output.freeze())
    }

    async fn decode(&mut self, data: Bytes, format: &AudioFormat) -> ServiceResult<Bytes> {
        if !self.supports_format(format) {
            return Err(ServiceError::InvalidFormat(
                "不支持的WAV格式".to_string(),
            ));
        }

        // 检查WAV头部
        if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(ServiceError::InvalidFormat(
                "无效的WAV文件头".to_string(),
            ));
        }

        // 跳过头部，直接返回音频数据
        Ok(data.slice(44..))
    }

    fn reset(&mut self) {
        // WAV编解码器不需要维护状态
    }
}
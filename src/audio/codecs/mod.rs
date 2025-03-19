//! 音频编解码器
//!
//! 提供音频编解码功能，支持不同音频格式之间的转换。
//! 主要包括：
//!
//! - 音频解码器：将音频数据解码为PCM格式
//! - 音频编码器：将PCM数据编码为指定格式

use crate::audio::format::{AudioCodec, AudioFormat};
use crate::audio::stream::{AudioChunk, StreamProcessor};
use crate::error::{ServiceError, ServiceResult};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};

mod wav;
mod mp3;
mod ogg;
mod flac;
mod aac;

pub use wav::{WavDecoder, WavEncoder};
pub use mp3::{Mp3Decoder, Mp3Encoder};
pub use ogg::{OggDecoder, OggEncoder};
pub use flac::{FlacDecoder, FlacEncoder};
pub use aac::{AacDecoder, AacEncoder};

/// 音频解码器
///
/// 将音频数据解码为PCM格式。
#[async_trait]
pub trait AudioDecoder: Send + Sync {
    /// 解码音频数据
    async fn decode(&mut self, data: Bytes) -> ServiceResult<Bytes>;

    /// 获取输出格式
    fn output_format(&self) -> AudioFormat;

    /// 刷新解码器
    async fn flush(&mut self) -> ServiceResult<Option<Bytes>> {
        Ok(None)
    }
}

/// 音频编码器
///
/// 将PCM数据编码为指定格式。
#[async_trait]
pub trait AudioEncoder: Send + Sync {
    /// 编码音频数据
    async fn encode(&mut self, data: Bytes) -> ServiceResult<Bytes>;

    /// 获取输出格式
    fn output_format(&self) -> AudioFormat;

    /// 刷新编码器
    async fn flush(&mut self) -> ServiceResult<Option<Bytes>> {
        Ok(None)
    }
}

/// 创建音频解码器
pub fn create_decoder(format: &AudioFormat) -> ServiceResult<Box<dyn AudioDecoder>> {
    match format.codec {
        AudioCodec::Wav => Ok(Box::new(WavDecoder::new(format.clone()))),
        AudioCodec::Mp3 => Ok(Box::new(Mp3Decoder::new(format.clone()))),
        AudioCodec::Ogg => Ok(Box::new(OggDecoder::new(format.clone()))),
        AudioCodec::Flac => Ok(Box::new(FlacDecoder::new(format.clone()))),
        AudioCodec::Aac => Ok(Box::new(AacDecoder::new(format.clone()))),
        AudioCodec::Other(ref s) => Err(ServiceError::InvalidFormat(format!("不支持的音频格式: {}", s))),
    }
}

/// 创建音频编码器
pub fn create_encoder(format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
    match format.codec {
        AudioCodec::Wav => Ok(Box::new(WavEncoder::new(format.clone()))),
        AudioCodec::Mp3 => Ok(Box::new(Mp3Encoder::new(format.clone()))),
        AudioCodec::Ogg => Ok(Box::new(OggEncoder::new(format.clone()))),
        AudioCodec::Flac => Ok(Box::new(FlacEncoder::new(format.clone()))),
        AudioCodec::Aac => Ok(Box::new(AacEncoder::new(format.clone()))),
        AudioCodec::Other(ref s) => Err(ServiceError::InvalidFormat(format!("不支持的音频格式: {}", s))),
    }
}

/// 编解码处理器
///
/// 实现StreamProcessor trait，用于在音频流中进行格式转换。
pub struct CodecProcessor {
    decoder: Box<dyn AudioDecoder>,
    encoder: Box<dyn AudioEncoder>,
}

impl CodecProcessor {
    /// 创建新的编解码处理器
    pub fn new(decoder: Box<dyn AudioDecoder>, encoder: Box<dyn AudioEncoder>) -> Self {
        Self { decoder, encoder }
    }

    /// 从输入和输出格式创建处理器
    pub fn from_formats(
        input_format: &AudioFormat,
        output_format: &AudioFormat,
    ) -> ServiceResult<Self> {
        let decoder = create_decoder(input_format)?;
        let encoder = create_encoder(output_format)?;
        Ok(Self::new(decoder, encoder))
    }
}

#[async_trait]
impl StreamProcessor for CodecProcessor {
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        // 解码
        let pcm = self.decoder.decode(chunk.data).await?;

        // 编码
        let encoded = self.encoder.encode(pcm).await?;

        Ok(AudioChunk::new(
            encoded,
            chunk.timestamp,
            chunk.duration,
            chunk.is_last,
        ))
    }

    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
        // 刷新解码器
        if let Some(pcm) = self.decoder.flush().await? {
            let encoded = self.encoder.encode(pcm).await?;
            return Ok(Some(AudioChunk::new(encoded, 0, 0, true)));
        }

        // 刷新编码器
        if let Some(encoded) = self.encoder.flush().await? {
            return Ok(Some(AudioChunk::new(encoded, 0, 0, true)));
        }

        Ok(None)
    }
}
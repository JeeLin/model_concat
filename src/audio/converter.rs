//! 音频流转换器
//!
//! 提供音频格式之间的实时转换功能，支持WAV、MP3等常见格式。
//! 实现了流式处理接口，可以进行实时音频转换。

use crate::audio::codecs::{self, AudioDecoder, AudioEncoder};
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::audio::stream::{AudioChunk, StreamProcessor};
use crate::error::{ServiceError, ServiceResult};
use async_trait::async_trait;
use bytes::Bytes;
use tracing::{debug, error};

/// 音频流转换器
///
/// 支持不同音频格式之间的实时转换，实现了StreamProcessor trait。
pub struct AudioStreamConverter {
    /// 输入音频格式
    input_format: AudioFormat,
    /// 输出音频格式
    output_format: AudioFormat,
    /// 音频解码器
    decoder: Box<dyn AudioDecoder>,
    /// 音频编码器
    encoder: Box<dyn AudioEncoder>,
}

impl AudioStreamConverter {
    /// 创建新的音频流转换器
    ///
    /// # 参数
    ///
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    pub fn new(input_format: AudioFormat, output_format: AudioFormat) -> ServiceResult<Self> {
        // 创建解码器
        let decoder: Box<dyn AudioDecoder> = match input_format.codec {
            AudioCodec::Wav => Box::new(codecs::WavDecoder::new(input_format.clone())),
            AudioCodec::Mp3 => Box::new(codecs::Mp3Decoder::new(input_format.clone())),
            _ => return Err(ServiceError::InvalidFormat(format!(
                "不支持的输入格式: {}", input_format.codec
            ))),
        };

        // 创建编码器
        let encoder: Box<dyn AudioEncoder> = match output_format.codec {
            AudioCodec::Wav => Box::new(codecs::WavEncoder::new(output_format.clone())),
            AudioCodec::Mp3 => Box::new(codecs::Mp3Encoder::new(output_format.clone())),
            _ => return Err(ServiceError::InvalidFormat(format!(
                "不支持的输出格式: {}", output_format.codec
            ))),
        };

        Ok(Self {
            input_format,
            output_format,
            decoder,
            encoder,
        })
    }

    /// 获取输入格式
    pub fn input_format(&self) -> &AudioFormat {
        &self.input_format
    }

    /// 获取输出格式
    pub fn output_format(&self) -> &AudioFormat {
        &self.output_format
    }
}

#[async_trait]
impl StreamProcessor for AudioStreamConverter {
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        debug!("处理音频块: {} 字节", chunk.data.len());

        // 解码为PCM
        let pcm = self.decoder.decode(chunk.data).await?;

        // 编码为目标格式
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_wav_to_mp3() {
        // 创建测试数据
        let samples: Vec<f32> = vec![-0.5, 0.0, 0.5, 1.0];
        let input_data = Bytes::from(
            samples
                .iter()
                .flat_map(|&s| s.to_le_bytes().to_vec())
                .collect::<Vec<u8>>(),
        );

        // 创建转换器
        let converter = AudioStreamConverter::new(
            AudioFormat::new(AudioCodec::Wav, 44100, 1),
            AudioFormat::with_bit_rate(AudioCodec::Mp3, 44100, 1, 192000),
        )
        .unwrap();

        // 创建音频块
        let chunk = AudioChunk::new(input_data, 0, 1000, true);

        // 执行转换
        let result = converter.process_chunk(chunk).await.unwrap();

        // 验证结果
        assert!(!result.data.is_empty());
    }
}
//! OGG格式编解码器实现
//!
//! 提供OGG格式音频的编码和解码功能。使用Symphonia库进行编解码。

use crate::audio::format::AudioFormat;
use crate::error::{ServiceError, ServiceResult};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::io::Cursor;

/// OGG解码器
pub struct OggDecoder {
    format: AudioFormat,
}

impl OggDecoder {
    /// 创建新的OGG解码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }
}

#[async_trait]
impl super::AudioDecoder for OggDecoder {
    async fn decode(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        // 创建媒体源
        let cursor = Cursor::new(data);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        // 创建格式探测器
        let mut hint = Hint::new();
        hint.with_extension("ogg");

        // 探测格式
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| ServiceError::InvalidFormat(format!("无法识别OGG格式: {}", e)))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| ServiceError::InvalidFormat("无法获取音频轨道".to_string()))?;

        // 创建解码器
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| ServiceError::InvalidFormat(format!("无法创建解码器: {}", e)))?;

        let mut samples = Vec::new();

        // 解码音频帧
        while let Ok(packet) = format.next_packet() {
            let decoded = decoder
                .decode(&packet)
                .map_err(|e| ServiceError::InvalidFormat(format!("解码失败: {}", e)))?;

            let mut sample_buf = SampleBuffer::new(decoded.capacity() as u64, *decoded.spec());
            sample_buf.copy_interleaved_ref(decoded);

            // 将采样点转换为f32
            samples.extend(
                sample_buf
                    .samples()
                    .iter()
                    .map(|&s| s as f32 / 32768.0),
            );
        }

        // 转换为字节
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|&s| s.to_le_bytes().to_vec())
            .collect();

        Ok(Bytes::from(bytes))
    }

    fn output_format(&self) -> AudioFormat {
        self.format.clone()
    }
}

/// OGG编码器
pub struct OggEncoder {
    format: AudioFormat,
}

impl OggEncoder {
    /// 创建新的OGG编码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }
}

#[async_trait]
impl super::AudioEncoder for OggEncoder {
    async fn encode(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        // 将字节转换为采样点
        let samples: Vec<f32> = data
            .chunks(4)
            .map(|chunk| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(chunk);
                f32::from_le_bytes(bytes)
            })
            .collect();

        // TODO: 实现OGG编码
        // 由于Rust生态中缺乏成熟的OGG编码库，这里需要集成第三方库或实现编码逻辑
        Err(ServiceError::Unimplemented("OGG编码器尚未实现".to_string()))
    }

    fn output_format(&self) -> AudioFormat {
        self.format.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioCodec;

    #[tokio::test]
    async fn test_ogg_decoder() {
        // 创建测试数据（这里需要一个有效的OGG文件数据）
        let format = AudioFormat::new(AudioCodec::Ogg, 44100, 2);
        let decoder = OggDecoder::new(format);

        // TODO: 添加解码器测试用例
    }
}
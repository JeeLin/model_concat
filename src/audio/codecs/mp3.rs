//! MP3格式编解码器实现
//!
//! 提供MP3格式音频的编码和解码功能。使用LAME库进行编解码。

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

/// MP3解码器
pub struct Mp3Decoder {
    format: AudioFormat,
}

impl Mp3Decoder {
    /// 创建新的MP3解码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }
}

#[async_trait]
impl super::AudioDecoder for Mp3Decoder {
    async fn decode(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        // 创建媒体源
        let cursor = Cursor::new(data);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        // 创建格式探测器
        let mut hint = Hint::new();
        hint.with_extension("mp3");

        // 探测格式
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| ServiceError::InvalidFormat(format!("无法识别MP3格式: {}", e)))?;

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

/// MP3编码器
pub struct Mp3Encoder {
    format: AudioFormat,
}

impl Mp3Encoder {
    /// 创建新的MP3编码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }
}

#[async_trait]
impl super::AudioEncoder for Mp3Encoder {
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

        // 创建LAME编码器
        let mut lame = lame::Lame::new().expect("无法创建LAME编码器");
        lame.set_num_channels(self.format.channels as u8)
            .expect("设置通道数失败");
        lame.set_sample_rate(self.format.sample_rate as u32)
            .expect("设置采样率失败");
        if let Some(bit_rate) = self.format.bit_rate {
            lame.set_brate((bit_rate / 1000) as u16)
                .expect("设置比特率失败");
        }
        lame.init_params().expect("初始化参数失败");

        let mut output = BytesMut::new();
        let mut mp3_buffer = vec![0u8; (samples.len() * 2) + 7200]; // 预留足够空间

        // 将采样点转换为PCM并编码
        let pcm: Vec<i16> = samples
            .iter()
            .map(|&s| (s * 32768.0) as i16)
            .collect();

        let encoded = if self.format.channels == 1 {
            lame.encode(&pcm, &pcm, &mut mp3_buffer)
                .expect("编码失败")
        } else {
            let (left, right): (Vec<_>, Vec<_>) = pcm.chunks(2).map(|c| (c[0], c[1])).unzip();
            lame.encode(&left, &right, &mut mp3_buffer)
                .expect("编码失败")
        };

        output.extend_from_slice(&mp3_buffer[..encoded]);

        // 写入MP3尾帧
        let flush_size = lame.flush(&mut mp3_buffer).expect("刷新编码器失败");
        output.extend_from_slice(&mp3_buffer[..flush_size]);

        Ok(output.freeze())
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
    async fn test_mp3_codec() {
        // 创建测试数据
        let samples: Vec<f32> = vec![-0.5, 0.0, 0.5, 1.0];
        let input_data = Bytes::from(
            samples
                .iter()
                .flat_map(|&s| s.to_le_bytes().to_vec())
                .collect::<Vec<u8>>(),
        );

        // 创建编码器和解码器
        let format = AudioFormat::with_bit_rate(AudioCodec::Mp3, 44100, 2, 320000);
        let mut encoder = Mp3Encoder::new(format.clone());
        let mut decoder = Mp3Decoder::new(format);

        // 编码
        let encoded = encoder.encode(input_data.clone()).await.unwrap();

        // 解码
        let decoded = decoder.decode(encoded).await.unwrap();

        // 验证结果（由于MP3是有损压缩，这里只检查基本的解码功能）
        assert!(!decoded.is_empty());
    }
}
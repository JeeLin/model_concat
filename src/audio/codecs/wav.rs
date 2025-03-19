//! WAV格式编解码器实现
//!
//! 提供WAV格式音频的编码和解码功能。支持不同的采样率、通道数和位深度。

use crate::audio::format::AudioFormat;
use crate::error::{ServiceError, ServiceResult};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use hound::{SampleFormat, WavReader, WavWriter};
use std::io::Cursor;

/// WAV解码器
pub struct WavDecoder {
    format: AudioFormat,
}

impl WavDecoder {
    /// 创建新的WAV解码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }
}

#[async_trait]
impl super::AudioDecoder for WavDecoder {
    async fn decode(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        let cursor = Cursor::new(data);
        let mut reader = WavReader::new(cursor).map_err(|e| {
            ServiceError::InvalidFormat(format!("无法读取WAV格式: {}", e))
        })?;

        let spec = reader.spec();
        let mut samples = Vec::new();

        // 根据采样格式读取样本
        match (spec.sample_format, spec.bits_per_sample) {
            (SampleFormat::Float, 32) => {
                samples.extend(reader.samples::<f32>().filter_map(Result::ok));
            }
            (SampleFormat::Int, 16) => {
                samples.extend(
                    reader
                        .samples::<i16>()
                        .filter_map(Result::ok)
                        .map(|s| s as f32 / 32768.0),
                );
            }
            (SampleFormat::Int, 24) => {
                samples.extend(
                    reader
                        .samples::<i32>()
                        .filter_map(Result::ok)
                        .map(|s| s as f32 / 8388608.0),
                );
            }
            (SampleFormat::Int, 32) => {
                samples.extend(
                    reader
                        .samples::<i32>()
                        .filter_map(Result::ok)
                        .map(|s| s as f32 / 2147483648.0),
                );
            }
            _ => {
                return Err(ServiceError::InvalidFormat(
                    "不支持的WAV采样格式".to_string(),
                ));
            }
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

/// WAV编码器
pub struct WavEncoder {
    format: AudioFormat,
}

impl WavEncoder {
    /// 创建新的WAV编码器
    pub fn new(format: AudioFormat) -> Self {
        Self { format }
    }
}

#[async_trait]
impl super::AudioEncoder for WavEncoder {
    async fn encode(&mut self, data: Bytes) -> ServiceResult<Bytes> {
        let mut buffer = BytesMut::new();
        let cursor = Cursor::new(&mut buffer);

        let spec = hound::WavSpec {
            channels: self.format.channels as u16,
            sample_rate: self.format.sample_rate,
            bits_per_sample: self.format.bits_per_sample.unwrap_or(32),
            sample_format: SampleFormat::Float,
        };

        let mut writer = WavWriter::new(cursor, spec).map_err(|e| {
            ServiceError::InvalidFormat(format!("无法创建WAV编码器: {}", e))
        })?;

        // 将字节转换为采样点
        let samples: Vec<f32> = data
            .chunks(4)
            .map(|chunk| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(chunk);
                f32::from_le_bytes(bytes)
            })
            .collect();

        // 写入采样点
        for sample in samples {
            writer.write_sample(sample).map_err(|e| {
                ServiceError::InvalidFormat(format!("WAV编码失败: {}", e))
            })?;
        }

        writer.finalize().map_err(|e| {
            ServiceError::InvalidFormat(format!("WAV编码完成失败: {}", e))
        })?;

        Ok(buffer.freeze())
    }

    fn output_format(&self) -> AudioFormat {
        self.format.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::codecs::{AudioDecoder, AudioEncoder};
    use crate::audio::format::AudioCodec;

    #[tokio::test]
    async fn test_wav_codec() {
        // 创建测试数据
        let samples: Vec<f32> = vec![-0.5, 0.0, 0.5, 1.0];
        let input_data = Bytes::from(
            samples
                .iter()
                .flat_map(|&s| s.to_le_bytes().to_vec())
                .collect::<Vec<u8>>(),
        );

        // 创建编码器和解码器
        let format = AudioFormat::new(AudioCodec::Wav, 44100, 1);
        let mut encoder = WavEncoder::new(format.clone());
        let mut decoder = WavDecoder::new(format);

        // 编码
        let encoded = encoder.encode(input_data.clone()).await.unwrap();

        // 解码
        let decoded = decoder.decode(encoded).await.unwrap();

        // 验证结果
        let result_samples: Vec<f32> = decoded
            .chunks(4)
            .map(|chunk| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(chunk);
                f32::from_le_bytes(bytes)
            })
            .collect();

        // 检查解码后的样本是否与原始样本匹配
        assert_eq!(samples.len(), result_samples.len());
        for (original, result) in samples.iter().zip(result_samples.iter()) {
            assert!((original - result).abs() < 1e-6);
        }
    }
}
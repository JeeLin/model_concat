//! 音频格式转换策略模块
//!
//! 本模块提供了音频格式转换的策略模式实现，包括：
//! - 通用格式转换策略：支持任意音频格式之间的转换
//! - 优化的采样格式转换：使用SIMD加速的采样格式转换
//! - 策略工厂：管理和创建转换策略
//!
//! # 示例
//!
//! ```rust
//! use crate::audio::converter::fmt_strategy::{FormatConverterFactory, GenericConversionStrategy};
//! use crate::audio::format::AudioFormat;
//!
//! // 创建工厂
//! let mut factory = FormatConverterFactory::new();
//!
//! // 注册转换策略
//! factory.register_strategy(Box::new(strategy));
//!
//! // 转换音频格式
//! let output = factory.convert_format(&input_data, &input_format, &output_format).await?;
//! ```

use crate::audio::format::{AudioCodec, AudioFormat};
use crate::audio::processors::{ChannelConverter, Resampler};
use crate::error::{ServiceError, ServiceResult};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 格式转换策略特征
/// 实现不同音频格式之间的转换逻辑
pub trait FormatConversionStrategy: Send + Sync {
    /// 转换音频数据
    async fn convert(&self, input: &[u8], input_format: &AudioFormat, output_format: &AudioFormat) -> ServiceResult<Vec<u8>>;

    /// 获取策略名称
    fn name(&self) -> &str;

    /// 获取源格式
    fn source_codec(&self) -> AudioCodec;

    /// 获取目标格式
    fn target_codec(&self) -> AudioCodec;
}

/// 通用格式转换策略
/// 使用解码器和编码器进行任意格式间的转换
pub struct GenericConversionStrategy {
    /// 源编解码器
    source_codec: AudioCodec,
    /// 目标编解码器
    target_codec: AudioCodec,
    /// 策略名称
    strategy_name: String,
    /// 解码器
    decoder: Arc<Mutex<Box<dyn AudioDecoder>>>,
    /// 编码器工厂函数
    encoder_factory: Box<dyn Fn(u32, u16, Option<u32>) -> ServiceResult<Box<dyn AudioEncoder>> + Send + Sync>,
}

impl GenericConversionStrategy {
    /// 创建新的通用转换策略
    pub fn new(
        source_codec: AudioCodec,
        target_codec: AudioCodec,
        decoder: Arc<Mutex<Box<dyn AudioDecoder>>>,
        encoder_factory: Box<dyn Fn(u32, u16, Option<u32>) -> ServiceResult<Box<dyn AudioEncoder>> + Send + Sync>,
    ) -> Self {
        let strategy_name = format!("{:?} to {:?}", source_codec, target_codec);
        Self {
            source_codec,
            target_codec,
            strategy_name,
            decoder,
            encoder_factory,
        }
    }
}

#[async_trait::async_trait]
impl FormatConversionStrategy for GenericConversionStrategy {
    async fn convert(&self, input: &[u8], input_format: &AudioFormat, output_format: &AudioFormat) -> ServiceResult<Vec<u8>> {
        // 解码为PCM样本
        let mut decoder = self.decoder.lock().await;
        let pcm_samples = decoder.decode_chunk(input).await?;

        // 如果需要，进行重采样
        let mut processed_samples = pcm_samples;
        if input_format.sample_rate != output_format.sample_rate {
            let resampler = Resampler::new(
                input_format.sample_rate,
                output_format.sample_rate,
                input_format.channels as usize,
            )?;
            processed_samples = resampler.resample(&processed_samples)?;
        }

        // 如果需要，进行通道转换
        if input_format.channels != output_format.channels {
            let converter = ChannelConverter::new(
                input_format.channels as usize,
                output_format.channels as usize,
            );
            processed_samples = converter.convert_channels(&processed_samples)?;
        }

        // 编码为目标格式
        let mut encoder = (self.encoder_factory)(
            output_format.sample_rate,
            output_format.channels as u16,
            output_format.bit_rate,
        )?;

        let mut result = Vec::new();
        if let Some(data) = encoder.encode_samples(&processed_samples)? {
            result.extend_from_slice(&data);
        }

        if let Some(data) = encoder.finalize()? {
            result.extend_from_slice(&data);
        }

        Ok(result)
    }

    fn name(&self) -> &str {
        &self.strategy_name
    }

    fn source_codec(&self) -> AudioCodec {
        self.source_codec.clone()
    }

    fn target_codec(&self) -> AudioCodec {
        self.target_codec.clone()
    }
}

/// 优化的采样格式转换函数
/// 使用通用的转换逻辑处理不同的采样格式
pub fn convert_sample_format<T, U>(input: &[T], scale_factor: f32, offset: f32) -> Vec<U>
where
    T: Copy + Into<f32>,
    U: From<f32> + Copy,
{
    let mut output = Vec::with_capacity(input.len());

    // 使用SIMD优化的批量处理
    if std::simd::Simd::<f32, 4>::valid() {
        let chunks = input.chunks_exact(4);
        let remainder = chunks.remainder();

        for chunk in chunks {
            let simd_samples = std::simd::f32x4::from_array([
                chunk[0].into(),
                chunk[1].into(),
                chunk[2].into(),
                chunk[3].into()
            ]);

            let result = (simd_samples / std::simd::f32x4::splat(scale_factor)) - std::simd::f32x4::splat(offset);
            let result_array = result.to_array();

            for &sample in &result_array {
                output.push(U::from(sample));
            }
        }

        // 处理剩余样本
        for &sample in remainder {
            let float_sample = sample.into();
            let converted = (float_sample / scale_factor) - offset;
            output.push(U::from(converted));
        }
    } else {
        // 回退到非SIMD实现
        for &sample in input {
            let float_sample = sample.into();
            let converted = (float_sample / scale_factor) - offset;
            output.push(U::from(converted));
        }
    }

    output
}

/// 格式转换策略工厂
pub struct FormatConverterFactory {
    strategies: std::collections::HashMap<(AudioCodec, AudioCodec), Box<dyn FormatConversionStrategy>>,
}

impl FormatConverterFactory {
    /// 创建新的格式转换策略工厂
    pub fn new() -> Self {
        Self {
            strategies: std::collections::HashMap::new(),
        }
    }

    /// 注册转换策略
    pub fn register_strategy(&mut self, strategy: Box<dyn FormatConversionStrategy>) -> &mut Self {
        let key = (strategy.source_codec(), strategy.target_codec());
        self.strategies.insert(key, strategy);
        self
    }

    /// 获取转换策略
    pub fn get_strategy(&self, source_codec: &AudioCodec, target_codec: &AudioCodec) -> Option<&Box<dyn FormatConversionStrategy>> {
        self.strategies.get(&(source_codec.clone(), target_codec.clone()))
    }

    /// 转换音频格式
    pub async fn convert_format(
        &self,
        data: &[u8],
        input_format: &AudioFormat,
        output_format: &AudioFormat,
    ) -> ServiceResult<Vec<u8>> {
        // 如果格式相同，直接返回
        if input_format.codec == output_format.codec
            && input_format.sample_rate == output_format.sample_rate
            && input_format.channels == output_format.channels {
            return Ok(data.to_vec());
        }

        // 获取转换策略
        if let Some(strategy) = self.get_strategy(&input_format.codec, &output_format.codec) {
            strategy.convert(data, input_format, output_format).await
        } else {
            Err(ServiceError::AudioConversion(
                format!("No conversion strategy found for {:?} to {:?}",
                        input_format.codec, output_format.codec)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // 模拟解码器
    struct MockDecoder;

    #[async_trait::async_trait]
    impl AudioDecoder for MockDecoder {
        async fn decode_chunk(&mut self, _data: &[u8]) -> ServiceResult<Vec<f32>> {
            Ok(vec![0.1, 0.2, 0.3])
        }

        async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
            Ok(None)
        }

        fn reset(&mut self) {}
    }

    // 模拟编码器
    struct MockEncoder;

    impl AudioEncoder for MockEncoder {
        fn encode_samples(&mut self, _samples: &[f32]) -> ServiceResult<Option<bytes::Bytes>> {
            Ok(Some(bytes::Bytes::from(vec![1, 2, 3])))
        }

        fn finalize(&mut self) -> ServiceResult<Option<bytes::Bytes>> {
            Ok(None)
        }

        fn reset(&mut self) {}
    }

    #[tokio::test]
    async fn test_generic_conversion_strategy() {
        let decoder = Arc::new(Mutex::new(Box::new(MockDecoder) as Box<dyn AudioDecoder>));
        let encoder_factory = Box::new(|_sr: u32, _ch: u16, _br: Option<u32>| -> ServiceResult<Box<dyn AudioEncoder>> {
            Ok(Box::new(MockEncoder))
        });

        let strategy = GenericConversionStrategy::new(
            AudioCodec::Wav,
            AudioCodec::Mp3,
            decoder,
            encoder_factory,
        );

        let input_format = AudioFormat {
            codec: AudioCodec::Wav,
            sample_rate: 44100,
            channels: 2,
            ..Default::default()
        };

        let output_format = AudioFormat {
            codec: AudioCodec::Mp3,
            sample_rate: 44100,
            channels: 2,
            ..Default::default()
        };

        let result = strategy.convert(&[0u8; 10], &input_format, &output_format).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_sample_format() {
        let input: Vec<i16> = vec![16384, -16384, 32767]; // 16-bit PCM samples
        let scale = 32768.0;
        let offset = 0.0;

        let output: Vec<f32> = convert_sample_format(&input, scale, offset);

        assert!((output[0] - 0.5).abs() < 0.01);
        assert!((output[1] - (-0.5)).abs() < 0.01);
        assert!((output[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_format_converter_factory() {
        let mut factory = FormatConverterFactory::new();
        let decoder = Arc::new(Mutex::new(Box::new(MockDecoder) as Box<dyn AudioDecoder>));
        let encoder_factory = Box::new(|_sr: u32, _ch: u16, _br: Option<u32>| -> ServiceResult<Box<dyn AudioEncoder>> {
            Ok(Box::new(MockEncoder))
        });

        let strategy = Box::new(GenericConversionStrategy::new(
            AudioCodec::Wav,
            AudioCodec::Mp3,
            decoder,
            encoder_factory,
        ));

        factory.register_strategy(strategy);

        let found_strategy = factory.get_strategy(&AudioCodec::Wav, &AudioCodec::Mp3);
        assert!(found_strategy.is_some());

        let not_found = factory.get_strategy(&AudioCodec::Mp3, &AudioCodec::Wav);
        assert!(not_found.is_none());
    }
}
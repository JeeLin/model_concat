use crate::audio::decoders::AudioDecoder;
use crate::audio::encoders::AudioEncoder;
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::audio::processors::{ChannelConverter, Resampler};
use crate::error::{ServiceError, ServiceResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 优化的格式转换策略特征
///
/// 提供更灵活的音频格式转换机制
pub trait OptimizedFormatConversionStrategy: Send + Sync {
    /// 转换音频数据
    async fn convert(&self, input: &[u8], input_format: &AudioFormat, output_format: &AudioFormat) -> ServiceResult<Vec<u8>>;

    /// 获取策略名称
    fn name(&self) -> &str;

    /// 获取源格式
    fn source_codec(&self) -> AudioCodec;

    /// 获取目标格式
    fn target_codec(&self) -> AudioCodec;

    /// 检查策略是否适用于给定的格式转换
    fn is_applicable(&self, source: &AudioCodec, target: &AudioCodec) -> bool {
        *source == self.source_codec() && *target == self.target_codec()
    }
}

/// 优化的格式转换工厂
///
/// 管理和创建适合特定转换需求的策略
pub struct OptimizedFormatConverterFactory {
    /// 注册的转换策略
    strategies: HashMap<(AudioCodec, AudioCodec), Box<dyn OptimizedFormatConversionStrategy>>,
    /// 通用转换策略构建器
    generic_builder: Box<dyn Fn(AudioCodec, AudioCodec) -> ServiceResult<Box<dyn OptimizedFormatConversionStrategy>> + Send + Sync>,
}

impl OptimizedFormatConverterFactory {
    /// 创建新的格式转换工厂
    pub fn new<F>(generic_builder: F) -> Self
    where
        F: Fn(AudioCodec, AudioCodec) -> ServiceResult<Box<dyn OptimizedFormatConversionStrategy>> + Send + Sync + 'static,
    {
        Self {
            strategies: HashMap::new(),
            generic_builder: Box::new(generic_builder),
        }
    }

    /// 注册转换策略
    pub fn register_strategy(&mut self, strategy: Box<dyn OptimizedFormatConversionStrategy>) {
        let key = (strategy.source_codec(), strategy.target_codec());
        self.strategies.insert(key, strategy);
    }

    /// 获取适合给定转换的策略
    pub fn get_strategy(&self, source: AudioCodec, target: AudioCodec) -> ServiceResult<&dyn OptimizedFormatConversionStrategy> {
        // 尝试获取专用策略
        if let Some(strategy) = self.strategies.get(&(source, target)) {
            return Ok(strategy.as_ref());
        }

        // 如果没有专用策略，创建通用策略
        Err(ServiceError::AudioConversion(format!("No conversion strategy found for {:?} to {:?}", source, target)))
    }

    /// 创建通用转换策略
    pub fn create_generic_strategy(&self, source: AudioCodec, target: AudioCodec) -> ServiceResult<Box<dyn OptimizedFormatConversionStrategy>> {
        (self.generic_builder)(source, target)
    }
}

/// 优化的通用格式转换策略
pub struct OptimizedGenericConversionStrategy {
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

impl OptimizedGenericConversionStrategy {
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

//! 优化的音频格式转换策略模块
//!
//! 本模块提供了优化的音频格式转换实现，包括：
//! - 优化的格式转换策略：支持更灵活的音频格式转换
//! - 优化的工厂模式：支持动态创建和管理转换策略
//! - 通用转换策略：提供默认的转换实现
//!
//! # 示例
//!
//! ```rust
//! use crate::audio::converter::format_strategy_opt::{OptimizedFormatConverterFactory, OptimizedGenericConversionStrategy};
//! use crate::audio::format::AudioFormat;
//!
//! // 创建工厂
//! let mut factory = OptimizedFormatConverterFactory::new(|source, target| {
//!     // 创建通用转换策略
//!     Ok(Box::new(strategy))
//! });
//!
//! // 注册专用转换策略
//! factory.register_strategy(Box::new(strategy));
//!
//! // 获取转换策略
//! let strategy = factory.get_strategy(source_codec, target_codec)?;
//! ```

#[async_trait::async_trait]
impl OptimizedFormatConversionStrategy for OptimizedGenericConversionStrategy {
    async fn convert(&self, input: &[u8], input_format: &AudioFormat, output_format: &AudioFormat) -> ServiceResult<Vec<u8>> {
        // 解码为PCM样本
        let mut decoder = self.decoder.lock().await;
        let pcm_samples = decoder.decode_chunk(input).await?;

        // 如果需要，进行重采样
        let mut processed_samples = pcm_samples;
        if input_format.sample_rate != output_format.sample_rate {
            let mut resampler = Resampler::new(
                input_format.sample_rate,
                output_format.sample_rate,
                input_format.channels as usize,
            )?;
            processed_samples = resampler.resample(&processed_samples).await?;
        }

        // 如果需要，进行通道转换
        if input_format.channels != output_format.channels {
            let mut converter = ChannelConverter::new(
                input_format.channels as usize,
                output_format.channels as usize,
            );
            processed_samples = converter.convert_channels(&processed_samples).await?;
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
        self.source_codec
    }

    fn target_codec(&self) -> AudioCodec {
        self.target_codec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
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
        fn encode_samples(&mut self, _samples: &[f32]) -> ServiceResult<Option<Bytes>> {
            Ok(Some(Bytes::from(vec![1, 2, 3])))
        }

        fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
            Ok(None)
        }

        fn reset(&mut self) {}
    }

    #[tokio::test]
    async fn test_optimized_generic_strategy() {
        let decoder = Arc::new(Mutex::new(Box::new(MockDecoder) as Box<dyn AudioDecoder>));
        let encoder_factory = Box::new(|_sr: u32, _ch: u16, _br: Option<u32>| -> ServiceResult<Box<dyn AudioEncoder>> {
            Ok(Box::new(MockEncoder))
        });

        let strategy = OptimizedGenericConversionStrategy::new(
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
            sample_rate: 48000, // 不同的采样率，测试重采样
            channels: 1, // 不同的通道数，测试通道转换
            ..Default::default()
        };

        let result = strategy.convert(&[0u8; 10], &input_format, &output_format).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimized_factory() {
        let generic_builder = |source: AudioCodec, target: AudioCodec| -> ServiceResult<Box<dyn OptimizedFormatConversionStrategy>> {
            let decoder = Arc::new(Mutex::new(Box::new(MockDecoder) as Box<dyn AudioDecoder>));
            let encoder_factory = Box::new(|_sr: u32, _ch: u16, _br: Option<u32>| -> ServiceResult<Box<dyn AudioEncoder>> {
                Ok(Box::new(MockEncoder))
            });

            Ok(Box::new(OptimizedGenericConversionStrategy::new(
                source,
                target,
                decoder,
                encoder_factory,
            )))
        };

        let mut factory = OptimizedFormatConverterFactory::new(generic_builder);

        // 测试获取不存在的策略
        let result = factory.get_strategy(AudioCodec::Wav, AudioCodec::Mp3);
        assert!(result.is_err());

        // 测试注册和获取策略
        let decoder = Arc::new(Mutex::new(Box::new(MockDecoder) as Box<dyn AudioDecoder>));
        let encoder_factory = Box::new(|_sr: u32, _ch: u16, _br: Option<u32>| -> ServiceResult<Box<dyn AudioEncoder>> {
            Ok(Box::new(MockEncoder))
        });

        let strategy = Box::new(OptimizedGenericConversionStrategy::new(
            AudioCodec::Wav,
            AudioCodec::Mp3,
            decoder,
            encoder_factory,
        ));

        factory.register_strategy(strategy);

        let found_strategy = factory.get_strategy(AudioCodec::Wav, AudioCodec::Mp3);
        assert!(found_strategy.is_ok());
    }

    #[test]
    fn test_strategy_applicability() {
        let decoder = Arc::new(Mutex::new(Box::new(MockDecoder) as Box<dyn AudioDecoder>));
        let encoder_factory = Box::new(|_sr: u32, _ch: u16, _br: Option<u32>| -> ServiceResult<Box<dyn AudioEncoder>> {
            Ok(Box::new(MockEncoder))
        });

        let strategy = OptimizedGenericConversionStrategy::new(
            AudioCodec::Wav,
            AudioCodec::Mp3,
            decoder,
            encoder_factory,
        );

        assert!(strategy.is_applicable(&AudioCodec::Wav, &AudioCodec::Mp3));
        assert!(!strategy.is_applicable(&AudioCodec::Mp3, &AudioCodec::Wav));
    }
}
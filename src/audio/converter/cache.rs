use crate::audio::codecs::{AudioDecoder, AudioEncoder};
use crate::audio::decoders::AudioDecoder;
use crate::audio::encoders::AudioEncoder;
use crate::audio::processors::{AudioProcessor, ChannelConverter, Resampler};
use crate::audio::AudioCodec;
use crate::error::ServiceResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// 处理器缓存
#[derive(Debug)]
pub struct ProcessorCache {
    /// 解码器缓存
    decoders: HashMap<String, Arc<Mutex<Box<dyn AudioDecoder>>>>,
    /// 重采样器缓存
    resamplers: HashMap<(u32, u32, usize), Arc<Mutex<Box<dyn Resampler>>>>,
    /// 通道转换器缓存
    channel_converters: HashMap<(usize, usize), Arc<Mutex<Box<dyn ChannelConverter>>>>,
    /// 处理器缓存
    processors: HashMap<String, Arc<Mutex<Box<dyn AudioProcessor>>>>,
    /// 编码器缓存
    encoders: HashMap<(AudioCodec, u32, u8), Arc<Mutex<Box<dyn AudioEncoder>>>>,
    /// 最后访问时间
    last_access: HashMap<String, Instant>,
    /// 缓存容量
    capacity: usize,
}

impl ProcessorCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            decoders: HashMap::with_capacity(capacity),
            resamplers: HashMap::with_capacity(capacity),
            channel_converters: HashMap::with_capacity(capacity),
            processors: HashMap::with_capacity(capacity),
            encoders: HashMap::with_capacity(capacity),
            last_access: HashMap::with_capacity(capacity),
            capacity,
        }
    }

    /// 创建适用于流式处理的缓存
    pub fn new_for_streaming(capacity: usize) -> Self {
        Self {
            decoders: HashMap::with_capacity(4), // 流式处理通常只需要少量解码器
            resamplers: HashMap::with_capacity(2), // 通常只需要1-2个重采样器
            channel_converters: HashMap::with_capacity(2), // 通常只需要1-2个通道转换器
            processors: HashMap::with_capacity(2), // 通常只需要少量处理器
            encoders: HashMap::with_capacity(4), // 流式处理通常只需要少量编码器
            last_access: HashMap::with_capacity(capacity),
            capacity,
        }
    }

    /// 清除所有缓存
    pub fn clear(&mut self) {
        self.decoders.clear();
        self.resamplers.clear();
        self.channel_converters.clear();
        self.processors.clear();
        self.encoders.clear();
        self.last_access.clear();
    }

    /// 获取解码器
    pub fn get_decoder(
        &self,
        codec: &AudioCodec,
    ) -> ServiceResult<Arc<Mutex<Box<dyn AudioDecoder>>>> {
        let key = codec.to_string();
        match codec {
            AudioCodec::Wav => Ok(self.get_or_create_decoder(&key, || {
                Box::new(crate::audio::decoders::WavDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Mp3 => Ok(self.get_or_create_decoder(&key, || {
                Box::new(crate::audio::decoders::Mp3Decoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Ogg => Ok(self.get_or_create_decoder(&key, || {
                Box::new(crate::audio::decoders::OggDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Flac => Ok(self.get_or_create_decoder(&key, || {
                Box::new(crate::audio::decoders::FlacDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Aac => Ok(self.get_or_create_decoder(&key, || {
                Box::new(crate::audio::decoders::AacDecoder::new()) as Box<dyn AudioDecoder>
            })),
            AudioCodec::Opus => Ok(self.get_or_create_decoder(&key, || {
                Box::new(crate::audio::decoders::OpusDecoder::new()) as Box<dyn AudioDecoder>
            })),
            _ => Err(ServiceError::AudioConversion(format!(
                "Unsupported codec: {:?}",
                codec
            ))),
        }
    }

    /// 获取WAV解码器 (兼容旧API)
    pub fn get_wav_decoder(&self) -> Arc<Mutex<Box<dyn AudioDecoder>>> {
        self.get_decoder(&AudioCodec::Wav).unwrap_or_else(|_| {
            self.get_or_create_decoder("wav", || {
                Box::new(crate::audio::decoders::WavDecoder::new()) as Box<dyn AudioDecoder>
            })
        })
    }

    /// 获取MP3解码器 (兼容旧API)
    pub fn get_mp3_decoder(&self) -> Arc<Mutex<Box<dyn AudioDecoder>>> {
        self.get_decoder(&AudioCodec::Mp3).unwrap_or_else(|_| {
            self.get_or_create_decoder("mp3", || {
                Box::new(crate::audio::decoders::Mp3Decoder::new()) as Box<dyn AudioDecoder>
            })
        })
    }

    /// 获取OGG解码器 (兼容旧API)
    pub fn get_ogg_decoder(&self) -> Arc<Mutex<Box<dyn AudioDecoder>>> {
        self.get_decoder(&AudioCodec::Ogg).unwrap_or_else(|_| {
            self.get_or_create_decoder("ogg", || {
                Box::new(crate::audio::decoders::OggDecoder::new()) as Box<dyn AudioDecoder>
            })
        })
    }

    /// 获取FLAC解码器 (兼容旧API)
    pub fn get_flac_decoder(&self) -> Arc<Mutex<Box<dyn AudioDecoder>>> {
        self.get_decoder(&AudioCodec::Flac).unwrap_or_else(|_| {
            self.get_or_create_decoder("flac", || {
                Box::new(crate::audio::decoders::FlacDecoder::new()) as Box<dyn AudioDecoder>
            })
        })
    }

    /// 获取AAC解码器 (兼容旧API)
    pub fn get_aac_decoder(&self) -> Arc<Mutex<Box<dyn AudioDecoder>>> {
        self.get_decoder(&AudioCodec::Aac).unwrap_or_else(|_| {
            self.get_or_create_decoder("aac", || {
                Box::new(crate::audio::decoders::AacDecoder::new()) as Box<dyn AudioDecoder>
            })
        })
    }

    /// 获取OPUS解码器 (兼容旧API)
    pub fn get_opus_decoder(&self) -> Arc<Mutex<Box<dyn AudioDecoder>>> {
        self.get_decoder(&AudioCodec::Opus).unwrap_or_else(|_| {
            self.get_or_create_decoder("opus", || {
                Box::new(crate::audio::decoders::OpusDecoder::new()) as Box<dyn AudioDecoder>
            })
        })
    }

    /// 获取重采样器
    pub fn get_resampler(
        &self,
        src_rate: u32,
        dst_rate: u32,
        channels: usize,
    ) -> ServiceResult<Arc<Mutex<Box<dyn Resampler>>>> {
        let key = (src_rate, dst_rate, channels);
        if let Some(resampler) = self.resamplers.get(&key) {
            return Ok(resampler.clone());
        }

        // 创建新的重采样器
        let resampler_result =
            crate::audio::processors::Resampler::new(src_rate, dst_rate, channels);

        match resampler_result {
            Ok(resampler) => {
                let resampler_box = Arc::new(Mutex::new(Box::new(resampler) as Box<dyn Resampler>));

                // 更新缓存
                let mut resamplers = self.resamplers.clone();

                // 如果缓存已满，移除最旧的条目
                if resamplers.len() >= self.capacity {
                    self.evict_oldest_cache_entry();
                }

                resamplers.insert(key, resampler_box.clone());

                Ok(resampler_box)
            }
            Err(e) => Err(e),
        }
    }

    /// 获取通道转换器
    pub fn get_channel_converter(
        &self,
        src_channels: usize,
        dst_channels: usize,
    ) -> Arc<Mutex<Box<dyn ChannelConverter>>> {
        let key = (src_channels, dst_channels);
        if let Some(converter) = self.channel_converters.get(&key) {
            return converter.clone();
        }

        // 创建新的通道转换器
        let converter = Arc::new(Mutex::new(Box::new(
            crate::audio::processors::ChannelConverter::new(src_channels, dst_channels),
        ) as Box<dyn ChannelConverter>));

        // 更新缓存
        let mut channel_converters = self.channel_converters.clone();

        // 如果缓存已满，移除最旧的条目
        if channel_converters.len() >= self.capacity {
            self.evict_oldest_cache_entry();
        }

        channel_converters.insert(key, converter.clone());

        converter
    }

    /// 获取编码器
    pub fn get_encoder(
        &self,
        codec: &AudioCodec,
        sample_rate: u32,
        channels: u8,
    ) -> Arc<Mutex<Box<dyn AudioEncoder>>> {
        let key = (codec.clone(), sample_rate, channels);
        if let Some(encoder) = self.encoders.get(&key) {
            return encoder.clone();
        }

        // 创建新的编码器
        let encoder = match codec {
            AudioCodec::Wav => Arc::new(Mutex::new(Box::new(
                crate::audio::encoders::WavEncoder::new(sample_rate, channels as u16),
            ) as Box<dyn AudioEncoder>)),
            AudioCodec::Mp3 => Arc::new(Mutex::new(Box::new(
                crate::audio::encoders::Mp3Encoder::new(sample_rate, channels as u16, 128000),
            ) as Box<dyn AudioEncoder>)),
            AudioCodec::Ogg => Arc::new(Mutex::new(Box::new(
                crate::audio::encoders::VorbisEncoder::new(sample_rate, channels as u16, 128000),
            ) as Box<dyn AudioEncoder>)),
            AudioCodec::Flac => Arc::new(Mutex::new(Box::new(
                crate::audio::encoders::FlacEncoder::new(sample_rate, channels as u16, 16),
            ) as Box<dyn AudioEncoder>)),
            AudioCodec::Aac => Arc::new(Mutex::new(Box::new(
                crate::audio::encoders::AacEncoder::new(sample_rate, channels as u16, 128000),
            ) as Box<dyn AudioEncoder>)),
            AudioCodec::Opus => Arc::new(Mutex::new(Box::new(
                crate::audio::encoders::OpusEncoder::new(sample_rate, channels as u16, 128000),
            ) as Box<dyn AudioEncoder>)),
            _ => Arc::new(Mutex::new(
                Box::new(crate::audio::encoders::WavEncoder::new(
                    sample_rate,
                    channels as u16,
                )) as Box<dyn AudioEncoder>,
            )),
        };

        // 更新缓存
        let mut encoders = self.encoders.clone();

        // 如果缓存已满，移除最旧的条目
        if encoders.len() >= self.capacity {
            self.evict_oldest_cache_entry();
        }

        encoders.insert(key, encoder.clone());

        encoder
    }

    /// 获取归一化处理器
    pub fn get_normalizer(&self) -> Arc<Mutex<Box<dyn AudioProcessor>>> {
        self.get_or_create_processor("normalizer", || {
            Box::new(crate::audio::processors::NormalizationProcessor::new(0.95))
                as Box<dyn AudioProcessor>
        })
    }

    /// 获取或创建解码器
    fn get_or_create_decoder<F>(&self, key: &str, create_fn: F) -> Arc<Mutex<Box<dyn AudioDecoder>>>
    where
        F: FnOnce() -> Box<dyn AudioDecoder>,
    {
        if let Some(decoder) = self.decoders.get(key) {
            // 更新最后访问时间
            let mut last_access = self.last_access.clone();
            last_access.insert(key.to_string(), Instant::now());

            return decoder.clone();
        }

        // 创建新的解码器
        let decoder = Arc::new(Mutex::new(create_fn()));

        // 更新缓存
        let mut decoders = self.decoders.clone();
        let mut last_access = self.last_access.clone();

        // 如果缓存已满，移除最旧的条目
        if decoders.len() >= self.capacity {
            self.evict_oldest_cache_entry();
        }

        decoders.insert(key.to_string(), decoder.clone());
        last_access.insert(key.to_string(), Instant::now());

        decoder
    }

    /// 获取或创建处理器
    fn get_or_create_processor<F>(
        &self,
        key: &str,
        create_fn: F,
    ) -> Arc<Mutex<Box<dyn AudioProcessor>>>
    where
        F: FnOnce() -> Box<dyn AudioProcessor>,
    {
        if let Some(processor) = self.processors.get(key) {
            // 更新最后访问时间
            let mut last_access = self.last_access.clone();
            last_access.insert(key.to_string(), Instant::now());

            return processor.clone();
        }

        // 创建新的处理器
        let processor = Arc::new(Mutex::new(create_fn()));

        // 更新缓存
        let mut processors = self.processors.clone();
        let mut last_access = self.last_access.clone();

        // 如果缓存已满，移除最旧的条目
        if processors.len() >= self.capacity {
            self.evict_oldest_cache_entry();
        }

        processors.insert(key.to_string(), processor.clone());
        last_access.insert(key.to_string(), Instant::now());

        processor
    }

    /// 移除最旧的缓存条目
    fn evict_oldest_cache_entry(&self) {
        if let Some((oldest_key, _)) = self.last_access.iter().min_by_key(|(_, &time)| time) {
            let oldest_key = oldest_key.clone();

            // 尝试从各个缓存中移除
            let mut decoders = self.decoders.clone();
            let mut processors = self.processors.clone();
            let mut last_access = self.last_access.clone();

            decoders.remove(&oldest_key);
            processors.remove(&oldest_key);
            last_access.remove(&oldest_key);
        }
    }

    /// 设置为流式模式（禁用缓存淘汰，优化长时间运行）
    pub fn set_streaming_mode(&mut self, streaming: bool) -> &mut Self {
        if streaming {
            // 在流式模式下，我们不需要频繁淘汰缓存
            // 因为处理器通常会在整个会话期间保持活跃
            self.capacity = usize::MAX;
        }
        self
    }

    /// 获取或创建解码器（流式优化版本）
    fn get_or_create_decoder_for_streaming<F>(
        &mut self,
        key: &str,
        create_fn: F,
    ) -> Arc<Mutex<Box<dyn AudioDecoder>>>
    where
        F: FnOnce() -> Box<dyn AudioDecoder>,
    {
        if let Some(decoder) = self.decoders.get(key) {
            // 在流式模式下，我们不需要更新最后访问时间
            // 因为我们不会淘汰缓存
            return decoder.clone();
        }

        // 创建新的解码器
        let decoder = Arc::new(Mutex::new(create_fn()));

        // 更新缓存
        self.decoders.insert(key.to_string(), decoder.clone());

        decoder
    }
}

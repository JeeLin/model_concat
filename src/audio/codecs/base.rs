use crate::audio::buffer::global as buffer_pool;
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::{ServiceError, ServiceResult};
use bytes::{BufMut, Bytes, BytesMut};
use std::io::Cursor;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::formats::FormatReader;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// 音频编码器基类
///
/// 提供音频编码器的通用功能和状态管理，包括：
/// - 音频格式参数管理（采样率、通道数、比特率）
/// - 编码缓冲区管理（使用全局缓冲区池）
/// - 通道分离和样本格式转换
/// - 编码状态跟踪
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::base::BaseEncoder;
/// use crate::audio::format::AudioFormat;
///
/// // 创建编码器
/// let format = AudioFormat {
///     sample_rate: 44100,
///     channels: 2,
///     bit_rate: Some(320000),
///     ..Default::default()
/// };
/// let mut encoder = BaseEncoder::with_format(&format);
///
/// // 编码音频样本
/// let samples = vec![0.0f32; 1024];
/// let (left, right) = encoder.separate_channels(&samples);
/// let pcm = encoder.convert_to_i16(&samples);
/// ```
#[derive(Debug)]
pub struct BaseEncoder {
    /// 输出缓冲区
    pub buffer: BytesMut,
    /// 采样率
    pub sample_rate: u32,
    /// 通道数
    pub channels: u16,
    /// 比特率（可选）
    pub bit_rate: Option<u32>,
    /// 是否已完成编码
    pub finalized: bool,
}

impl BaseEncoder {
    /// 创建新的编码器基类，使用默认帧大小
    pub fn with_format(format: &AudioFormat) -> Self {
        Self::new(format.sample_rate, format.channels as u16, format.bit_rate)
    }

    /// 创建新的编码器基类
    pub fn new(sample_rate: u32, channels: u16, bit_rate: Option<u32>) -> Self {
        Self {
            buffer: buffer_pool::get_buffer(4096), // 使用全局缓冲区池获取初始缓冲区
            sample_rate,
            channels,
            bit_rate,
            finalized: false,
        }
    }

    /// 重置编码器状态
    pub fn reset(&mut self) {
        // 返回当前缓冲区到池中并获取新的缓冲区
        let old_buffer = std::mem::replace(&mut self.buffer, buffer_pool::get_buffer(4096));
        buffer_pool::return_buffer(old_buffer);
        self.finalized = false;
    }

    /// 获取当前缓冲区内容并清空缓冲区
    pub fn take_buffer(&mut self) -> Bytes {
        // 获取当前缓冲区内容
        let bytes = self.buffer.clone().freeze();

        // 返回当前缓冲区到池中并获取新的缓冲区
        let old_buffer = std::mem::replace(&mut self.buffer, buffer_pool::get_buffer(4096));
        buffer_pool::return_buffer(old_buffer);

        bytes
    }

    /// 分离音频通道
    pub fn separate_channels<'a>(&self, samples: &'a [f32]) -> (Vec<f32>, Vec<f32>) {
        let samples_per_channel = samples.len() / self.channels as usize;
        let mut left = Vec::with_capacity(samples_per_channel);
        let mut right = Vec::with_capacity(if self.channels > 1 {
            samples_per_channel
        } else {
            0
        });

        // 分离通道
        for chunk in samples.chunks(self.channels as usize) {
            left.push(chunk[0]);
            if self.channels > 1 {
                right.push(chunk[1]);
            }
        }

        (left, right)
    }

    /// 设置音频格式
    pub fn set_format(&mut self, format: &AudioFormat) {
        self.sample_rate = format.sample_rate;
        self.channels = format.channels as u16;
        self.bit_rate = format.bit_rate;
    }

    /// 将浮点样本转换为16位整数
    pub fn convert_to_i16(&self, samples: &[f32]) -> Vec<i16> {
        samples.iter().map(|&s| (s * 32767.0) as i16).collect()
    }

    /// 将浮点样本转换为24位整数
    pub fn convert_to_i24(&self, samples: &[f32]) -> Vec<i32> {
        samples.iter().map(|&s| (s * 8388607.0) as i32).collect()
    }

    /// 将浮点样本转换为32位整数
    pub fn convert_to_i32(&self, samples: &[f32]) -> Vec<i32> {
        samples.iter().map(|&s| (s * 2147483647.0) as i32).collect()
    }

    /// 检查样本数据是否为完整帧
    pub fn is_complete_frame(&self, samples: &[f32], frame_size: usize) -> bool {
        samples.len() == frame_size * self.channels as usize
    }

    /// 处理音频帧
    pub fn process_frames<F, T>(
        &mut self,
        samples: &[f32],
        frame_size: usize,
        mut process_fn: F,
    ) -> ServiceResult<()>
    where
        F: FnMut(&[f32]) -> ServiceResult<Vec<u8>>,
        T: std::error::Error + 'static,
    {
        let channel_count = self.channels as usize;
        let samples_per_frame = frame_size * channel_count;

        // 处理完整帧
        for chunk in samples.chunks(samples_per_frame) {
            if chunk.len() == samples_per_frame {
                let encoded = process_fn(chunk)?;
                if !encoded.is_empty() {
                    self.buffer.put_slice(&encoded);
                }
            }
        }

        Ok(())
    }

    /// 创建编码错误
    pub fn create_error<S: Into<String>>(codec_name: &str, message: S) -> ServiceError {
        ServiceError::AudioEncoding(format!("{} encoder error: {}", codec_name, message.into()))
    }
}

/// 音频解码器基类
///
/// 提供音频解码器的通用功能和状态管理，包括：
/// - 音频格式参数管理和自动更新
/// - 基于Symphonia的解码器和格式读取器管理
/// - 样本缓冲区管理（支持全局缓冲区池）
/// - 流式解码支持
/// - 自动格式探测和解码器初始化
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::base::BaseDecoder;
/// use crate::audio::format::AudioFormat;
/// use symphonia::core::probe::Hint;
///
/// // 创建解码器
/// let format = AudioFormat::default();
/// let mut decoder = BaseDecoder::new(format);
///
/// // 准备解码
/// let mut hint = Hint::new();
/// hint.with_extension("mp3");
///
/// // 流式解码数据
/// let data = vec![0u8; 1024]; // 音频数据
/// decoder.decode(&data)?;
///
/// // 获取解码后的PCM数据
/// let pcm = decoder.get_decoded_data();
/// ```
#[derive(Debug)]
pub struct BaseDecoder {
    /// 解码器实例
    pub decoder: Option<Box<dyn Decoder>>,
    /// 格式读取器
    pub format_reader: Option<Box<dyn FormatReader>>,
    /// 音频格式
    pub format: AudioFormat,
    /// 未处理的数据
    pub remaining_data: Vec<u8>,
    /// 解码后的PCM数据
    pub decoded_data: Vec<f32>,
    /// 是否已完成解码
    pub finalized: bool,
}

impl BaseDecoder {
    /// 处理解码后的样本数据
    ///
    /// 将解码后的原始样本数据转换为标准化的浮点数格式。此方法处理不同的
    /// 采样格式（如i16、i24、i32等），并进行适当的归一化处理。
    ///
    /// # 参数
    /// * `sample_buffer` - 包含解码后样本数据的缓冲区
    ///
    /// # 返回值
    /// 返回转换后的32位浮点数格式的音频样本。
    pub fn process_decoded_samples(&self, sample_buffer: &SampleBuffer<f32>) -> Vec<f32> {
        sample_buffer.samples().to_vec()
    }

    /// 创建解码错误
    ///
    /// 创建标准化的音频解码错误，包含编解码器名称和具体错误信息。
    ///
    /// # 参数
    /// * `codec_name` - 编解码器名称
    /// * `message` - 错误信息
    ///
    /// # 返回值
    /// 返回格式化的ServiceError
    pub fn create_error<S: Into<String>>(codec_name: &str, message: S) -> ServiceError {
        ServiceError::AudioDecoding(format!("{} decoder error: {}", codec_name, message.into()))
    }

    /// 获取当前音频格式
    ///
    /// 返回解码器当前使用的音频格式参数，包括采样率、通道数等信息。
    ///
    /// # 返回值
    /// 返回当前的AudioFormat配置
    pub fn get_format(&self) -> &AudioFormat {
        &self.format
    }

    /// 检查解码器是否已完成初始化
    ///
    /// 验证解码器和格式读取器是否已正确初始化，可用于解码前的状态检查。
    ///
    /// # 返回值
    /// 如果解码器已正确初始化则返回true，否则返回false
    pub fn is_initialized(&self) -> bool {
        self.decoder.is_some() && self.format_reader.is_some()
    }

    /// 创建新的解码器基类
    pub fn new(format: AudioFormat) -> Self {
        Self {
            format,
            decoder: None,
            format_reader: None,
            sample_buf: None,
            remaining_data: Vec::new(),
            use_buffer_pool: true,
        }
    }

    /// 初始化解码器
    ///
    /// 使用提供的格式提示(hint)来探测音频格式并创建相应的解码器。
    /// 此方法会自动处理以下步骤：
    /// 1. 创建媒体源流
    /// 2. 探测音频格式
    /// 3. 获取默认音轨
    /// 4. 创建对应的解码器
    ///
    /// # 参数
    /// * `hint` - 格式提示，用于指导格式探测过程
    ///
    /// # 错误
    /// 在以下情况会返回错误：
    /// - 格式探测失败
    /// - 未找到默认音轨
    /// - 解码器创建失败
    ///
    /// # 示例
    /// ```rust
    /// use symphonia::core::probe::Hint;
    ///
    /// let mut hint = Hint::new();
    /// hint.with_extension("mp3");
    /// decoder.initialize_decoder(hint)?;
    /// ```
    pub fn initialize_decoder(&mut self, hint: Hint) -> ServiceResult<()> {
        // 合并剩余数据
        let cursor = Cursor::new(&self.remaining_data);
        let media_source = MediaSourceStream::new(Box::new(cursor), Default::default());

        // 探测格式
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, media_source, &format_opts, &metadata_opts)
            .map_err(|e| crate::error::ServiceError::AudioConversion(e.to_string()))?;

        let mut format = probed.format;
        let track = format.default_track().ok_or_else(|| {
            crate::error::ServiceError::AudioConversion("No default track found".into())
        })?;

        let decoder = symphonia::default::get_codecs()
            .make_decoder(track.codec_params.clone(), &decoder_opts)
            .map_err(|e| crate::error::ServiceError::AudioConversion(e.to_string()))?;

        self.decoder = Some(decoder);
        self.format_reader = Some(format);
        Ok(())
    }

    /// 流式解码音频数据
    ///
    /// 对输入的音频数据进行解码，支持流式处理。解码后的PCM数据可通过
    /// `get_decoded_data()`方法获取。
    ///
    /// # 参数
    /// * `data` - 待解码的音频数据
    /// * `hint` - 音频格式提示
    ///
    /// # 错误
    /// 在以下情况会返回错误：
    /// - 解码器未初始化
    /// - 解码过程中出现错误
    /// - 音频格式不支持
    ///
    /// # 示例
    /// ```rust
    /// // 流式解码数据
    /// let data = vec![0u8; 1024]; // 音频数据块
    /// let mut hint = Hint::new();
    /// hint.with_extension("mp3");
    /// decoder.decode_chunk(&data, hint)?;
    /// ```
    pub async fn decode_chunk(&mut self, data: &[u8], hint: Hint) -> ServiceResult<Vec<f32>> {
        // 添加新数据到缓冲区
        self.remaining_data.extend_from_slice(data);

        // 如果解码器未初始化，则初始化
        if self.decoder.is_none() {
            self.initialize_decoder(hint)?;
        }

        // 使用缓冲区池分配内存，减少内存分配
        let mut decoded_samples = if self.use_buffer_pool {
            Vec::with_capacity(data.len() * 4) // 预估解码后的样本数量
        } else {
            Vec::new()
        };

        // 解码所有可用的数据包
        if let (Some(decoder), Some(format_reader)) = (&mut self.decoder, &mut self.format_reader) {
            loop {
                // 尝试获取下一个数据包
                match format_reader.next_packet() {
                    Ok(packet) => {
                        // 解码数据包
                        match decoder.decode(&packet) {
                            Ok(decoded) => {
                                // 创建样本缓冲区（如果需要）
                                if self.sample_buf.is_none() {
                                    let spec = *decoded.spec();
                                    self.sample_buf =
                                        Some(SampleBuffer::new(decoded.capacity() as u64, spec));

                                    // 更新音频格式信息
                                    self.format.sample_rate = spec.rate;
                                    self.format.channels = spec.channels.count() as u8;
                                }

                                // 将解码后的音频数据转换为f32样本
                                if let Some(sample_buf) = &mut self.sample_buf {
                                    sample_buf.copy_interleaved_ref(decoded);
                                    decoded_samples.extend_from_slice(sample_buf.samples());
                                }
                            }
                            Err(e) => {
                                return Err(crate::error::ServiceError::AudioDecoding(format!(
                                    "解码失败: {}",
                                    e
                                )));
                            }
                        }
                    }
                    Err(_) => {
                        // 没有更多数据包可用，退出循环
                        break;
                    }
                }
            }
        }

        Ok(decoded_samples)
    }

    /// 刷新解码器，处理剩余数据
    ///
    /// 处理解码器中的剩余数据，并返回最后的PCM样本。
    /// 此方法通常在音频流结束时调用，以确保所有数据都被正确解码。
    ///
    /// # 返回值
    /// 如果还有剩余数据被解码，则返回Some(Vec<f32>)，否则返回None
    ///
    /// # 错误
    /// 在解码过程中出现错误时返回ServiceError
    ///
    /// # 示例
    /// ```rust
    /// // 完成解码后刷新
    /// if let Some(final_samples) = decoder.flush().await? {
    ///     // 处理最后的样本数据
    /// }
    /// ```
    pub async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
        if self.decoder.is_none() || self.remaining_data.is_empty() {
            return Ok(None);
        }

        // 尝试解码剩余数据
        let mut hint = Hint::new();
        match self.format.codec {
            AudioCodec::Wav => hint.with_extension("wav"),
            AudioCodec::Mp3 => hint.with_extension("mp3"),
            AudioCodec::Ogg => hint.with_extension("ogg"),
            AudioCodec::Flac => hint.with_extension("flac"),
            AudioCodec::Aac => hint.with_extension("aac"),
            AudioCodec::Opus => hint.with_extension("opus"),
            _ => hint.with_extension("wav"),
        };

        let result = self.decode_chunk(&[], hint).await?;

        // 清理资源
        self.decoder = None;
        self.format_reader = None;
        self.sample_buf = None;
        self.remaining_data.clear();

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// 重置解码器状态
    ///
    /// 清空所有缓冲区并重置解码器状态，为处理新的音频流做准备。
    /// 注意：此操作不会重置音频格式参数。
    pub fn reset(&mut self) {
        self.decoder = None;
        self.format_reader = None;
        self.sample_buf = None;
        self.remaining_data.clear();
    }
}

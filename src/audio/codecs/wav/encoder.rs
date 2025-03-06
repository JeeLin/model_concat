use crate::audio::codecs::base::BaseEncoder;
use crate::audio::codecs::traits::AudioEncoder;
use crate::error::ServiceResult;
use bytes::{BufMut, Bytes, BytesMut};
use hound::{SampleFormat, WavSpec};
use std::io::Cursor;

/// WAV编码器
pub struct WavEncoder {
    /// 基础编码器
    base: BaseEncoder,
    /// WAV规格
    spec: WavSpec,
    /// 是否已写入头部
    header_written: bool,
}

impl WavEncoder {
    /// 创建新的WAV编码器
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        Self {
            base: BaseEncoder::new(sample_rate, channels, None),
            spec,
            header_written: false,
        }
    }

    /// 写入WAV头部
    fn write_header(&mut self) -> ServiceResult<()> {
        let mut buffer = BytesMut::new();
        let mut writer =
            hound::WavWriter::new(Cursor::new(Vec::new()), self.spec).map_err(|e| {
                BaseEncoder::create_error("WAV", format!("Failed to create WAV writer: {}", e))
            })?;

        // 获取头部数据
        let header_bytes = writer.get_mut().get_ref();
        buffer.put_slice(header_bytes);

        // 更新基础编码器的缓冲区
        self.base.buffer = buffer;
        self.header_written = true;

        Ok(())
    }
}

impl AudioEncoder for WavEncoder {
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
        // 如果还没有写入头部，先写入头部
        if !self.header_written {
            self.write_header()?;
        }

        // 将浮点样本转换为16位整数
        let i16_samples = self.base.convert_to_i16(samples);

        // 将样本写入缓冲区
        for sample in i16_samples {
            self.base.buffer.put_i16_le(sample);
        }

        Ok(None)
    }

    fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
        // 如果缓冲区不为空，返回缓冲区内容
        if !self.base.buffer.is_empty() {
            Ok(Some(self.base.take_buffer()))
        } else {
            Ok(None)
        }
    }

    fn reset(&mut self) {
        self.base.reset();
        self.header_written = false;
    }
}

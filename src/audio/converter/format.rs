use super::format_strategy::convert_sample_format;
use crate::error::ServiceResult;
use symphonia::core::audio::{AudioBuffer, AudioBufferRef, SignalSpec};

/// 采样格式转换器
pub struct SampleFormatConverter {
    spec: SignalSpec,
}

impl SampleFormatConverter {
    pub fn new(spec: SignalSpec) -> Self {
        Self { spec }
    }

    pub fn convert_to_f32(&self, buffer: AudioBufferRef) -> ServiceResult<AudioBuffer<f32>> {
        let mut float_buf = AudioBuffer::new(buffer.capacity() as u64, self.spec);

        match buffer {
            AudioBufferRef::F32(buf) => {
                float_buf.copy_interleaved_ref(buf);
            }
            AudioBufferRef::U8(buf) => {
                let samples = convert_sample_format::<u8, f32>(buf.samples(), 128.0, 1.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::U16(buf) => {
                let samples = convert_sample_format::<u16, f32>(buf.samples(), 32768.0, 1.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::U24(buf) => {
                let samples = convert_sample_format::<u32, f32>(buf.samples(), 8388608.0, 1.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::U32(buf) => {
                let samples = convert_sample_format::<u32, f32>(buf.samples(), 2147483648.0, 1.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::S8(buf) => {
                let samples = convert_sample_format::<i8, f32>(buf.samples(), 128.0, 0.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::S16(buf) => {
                let samples = convert_sample_format::<i16, f32>(buf.samples(), 32768.0, 0.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::S24(buf) => {
                let samples = convert_sample_format::<i32, f32>(buf.samples(), 8388608.0, 0.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::S32(buf) => {
                let samples = convert_sample_format::<i32, f32>(buf.samples(), 2147483648.0, 0.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
            AudioBufferRef::F64(buf) => {
                let samples = convert_sample_format::<f64, f32>(buf.samples(), 1.0, 0.0);
                float_buf.samples_mut().copy_from_slice(&samples);
            }
        }

        Ok(float_buf)
    }
}

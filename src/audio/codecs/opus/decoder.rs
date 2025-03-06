use crate::audio::codecs::base::BaseDecoder;
use crate::audio::codecs::traits::AudioDecoder;
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;
use symphonia::core::probe::Hint;

/// Opus音频解码器
///
/// 用于解码Opus格式的音频数据
pub struct OpusDecoder {
    /// 基础解码器
    base: BaseDecoder,
}

impl OpusDecoder {
    /// 创建新的Opus解码器
    pub fn new() -> Self {
        let mut format = AudioFormat::default();
        format.codec = AudioCodec::Opus;

        Self {
            base: BaseDecoder::new(format),
        }
    }
}

#[async_trait::async_trait]
impl AudioDecoder for OpusDecoder {
    async fn decode_chunk(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>> {
        // 创建Opus格式提示
        let mut hint = Hint::new();
        hint.with_extension("opus");

        // 使用基础解码器进行流式解码
        self.base.decode_chunk(data, hint).await
    }

    async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
        self.base.flush().await
    }

    fn reset(&mut self) {
        let mut format = AudioFormat::default();
        format.codec = AudioCodec::Opus;

        // 创建新的基础解码器
        self.base = BaseDecoder::new(format);
    }
}

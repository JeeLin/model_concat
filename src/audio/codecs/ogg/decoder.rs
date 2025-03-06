use crate::audio::codecs::base::BaseDecoder;
use crate::audio::codecs::traits::AudioDecoder;
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;
use symphonia::core::probe::Hint;

/// OGG音频解码器
///
/// 用于解码OGG格式的音频数据
pub struct OggDecoder {
    /// 基础解码器
    base: BaseDecoder,
}

impl OggDecoder {
    /// 创建新的OGG解码器
    pub fn new() -> Self {
        let mut format = AudioFormat::default();
        format.codec = AudioCodec::Ogg;

        Self {
            base: BaseDecoder::new(format),
        }
    }
}

#[async_trait::async_trait]
impl AudioDecoder for OggDecoder {
    async fn decode_chunk(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>> {
        // 创建OGG格式提示
        let mut hint = Hint::new();
        hint.with_extension("ogg");

        // 使用基础解码器进行流式解码
        self.base.decode_chunk(data, hint).await
    }

    async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
        self.base.flush().await
    }

    fn reset(&mut self) {
        let mut format = AudioFormat::default();
        format.codec = AudioCodec::Ogg;

        // 创建新的基础解码器
        self.base = BaseDecoder::new(format);
    }
}

use crate::audio::codecs::base::BaseDecoder;
use crate::audio::codecs::traits::AudioDecoder;
use crate::audio::format::{AudioCodec, AudioFormat};
use crate::error::ServiceResult;
use symphonia::core::probe::Hint;

/// ALAC音频解码器
///
/// 使用Symphonia库实现的ALAC（Apple Lossless Audio Codec）音频解码器，支持以下功能：
/// - 支持标准ALAC格式解码
/// - 支持流式解码
/// - 自动检测音频格式参数
/// - 支持解码状态重置
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::alac::AlacDecoder;
///
/// // 创建ALAC解码器
/// let mut decoder = AlacDecoder::new();
///
/// // 解码音频数据
/// let alac_data = vec![0u8; 1024]; // ALAC编码的数据
/// let decoded = decoder.decode_chunk(&alac_data).await?;
///
/// // 完成解码并获取剩余数据
/// let final_samples = decoder.flush().await?;
/// ```
pub struct AlacDecoder {
    /// 基础解码器，提供通用的解码功能
    base: BaseDecoder,
}

impl AlacDecoder {
    /// 创建新的ALAC解码器
    ///
    /// # 返回值
    ///
    /// 返回初始化完成的ALAC解码器实例
    pub fn new() -> Self {
        let mut format = AudioFormat::default();
        format.codec = AudioCodec::Alac;

        Self {
            base: BaseDecoder::new(format),
        }
    }
}

#[async_trait::async_trait]
impl AudioDecoder for AlacDecoder {
    /// 解码一块ALAC音频数据
    ///
    /// # 参数
    ///
    /// * `data` - 待解码的ALAC数据块
    ///
    /// # 返回值
    ///
    /// 返回解码后的f32格式音频样本
    async fn decode_chunk(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>> {
        // 创建ALAC格式提示
        let mut hint = Hint::new();
        hint.with_extension("m4a");

        // 使用基础解码器进行流式解码
        self.base.decode_chunk(data, hint).await
    }

    /// 刷新解码器并获取剩余的音频数据
    ///
    /// # 返回值
    ///
    /// 返回解码器缓冲区中剩余的音频样本
    async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
        self.base.flush().await
    }

    /// 重置解码器状态
    ///
    /// 清除所有内部缓冲区并重置解码器到初始状态
    fn reset(&mut self) {
        let mut format = AudioFormat::default();
        format.codec = AudioCodec::Alac;

        // 创建新的基础解码器
        self.base = BaseDecoder::new(format);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alac_decoder_creation() {
        let decoder = AlacDecoder::new();
        assert_eq!(decoder.base.format().codec, AudioCodec::Alac);
    }

    #[tokio::test]
    async fn test_alac_decoding_empty_input() {
        let mut decoder = AlacDecoder::new();

        // 测试空输入
        let result = decoder.decode_chunk(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_alac_decoder_reset() {
        let mut decoder = AlacDecoder::new();

        // 解码一些数据
        let _ = decoder.decode_chunk(&[0u8; 1024]).await;

        // 重置解码器
        decoder.reset();

        // 验证重置后的状态
        assert_eq!(decoder.base.format().codec, AudioCodec::Alac);
    }

    #[tokio::test]
    async fn test_alac_decoder_flush() {
        let mut decoder = AlacDecoder::new();

        // 解码一些数据
        let _ = decoder.decode_chunk(&[0u8; 1024]).await;

        // 测试刷新操作
        let result = decoder.flush().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_alac_decoder_invalid_data() {
        let mut decoder = AlacDecoder::new();

        // 使用无效的ALAC数据测试解码器
        let invalid_data = vec![0xFF; 1024]; // 非法ALAC数据
        let result = decoder.decode_chunk(&invalid_data).await;

        // 解码应该成功但返回空结果
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}

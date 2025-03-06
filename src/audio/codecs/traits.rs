use crate::error::ServiceResult;
use bytes::Bytes;

/// 音频编码器特征
///
/// 定义了音频编码器的基本接口，用于将音频样本编码为特定格式的数据。
/// 实现此特征的编码器需要支持流式编码和状态管理。
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::AudioEncoder;
/// use bytes::Bytes;
///
/// struct MyEncoder;
///
/// impl AudioEncoder for MyEncoder {
///     fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
///         // 实现编码逻辑
///         Ok(None)
///     }
///     
///     fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
///         // 实现最终化逻辑
///         Ok(None)
///     }
///     
///     fn reset(&mut self) {
///         // 实现重置逻辑
///     }
/// }
/// ```
pub trait AudioEncoder: Send + Sync {
    /// 编码音频样本
    ///
    /// 将浮点格式的音频样本编码为目标格式的字节数据。
    ///
    /// # 参数
    ///
    /// * `samples` - 待编码的音频样本数组，每个样本为32位浮点数
    ///
    /// # 返回值
    ///
    /// 返回编码后的数据，如果没有足够的数据生成输出则返回None
    fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>>;

    /// 完成编码
    ///
    /// 在所有数据都已处理完毕后调用，用于获取编码器中剩余的数据。
    ///
    /// # 返回值
    ///
    /// 返回编码器中剩余的数据，如果没有剩余数据则返回None
    fn finalize(&mut self) -> ServiceResult<Option<Bytes>>;

    /// 重置编码器状态
    ///
    /// 清除编码器的内部状态，准备处理新的音频流。
    fn reset(&mut self);
}

/// 音频解码器特征
///
/// 定义了音频解码器的基本接口，用于将编码后的音频数据解码为PCM样本。
/// 实现此特征的解码器需要支持异步操作和流式解码。
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::AudioDecoder;
///
/// struct MyDecoder;
///
/// #[async_trait::async_trait]
/// impl AudioDecoder for MyDecoder {
///     async fn decode_chunk(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>> {
///         // 实现解码逻辑
///         Ok(Vec::new())
///     }
///     
///     async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
///         // 实现刷新逻辑
///         Ok(None)
///     }
///     
///     fn reset(&mut self) {
///         // 实现重置逻辑
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait AudioDecoder: Send + Sync {
    /// 解码音频数据
    ///
    /// 将编码后的音频数据解码为PCM样本。
    /// 默认实现调用decode_chunk方法。
    ///
    /// # 参数
    ///
    /// * `data` - 待解码的音频数据
    ///
    /// # 返回值
    ///
    /// 返回解码后的PCM样本数组
    async fn decode(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>> {
        // 默认实现：调用decode_chunk
        self.decode_chunk(data).await
    }

    /// 流式解码音频数据
    ///
    /// 以流式方式解码音频数据块。适用于实时解码场景。
    ///
    /// # 参数
    ///
    /// * `data` - 待解码的音频数据块
    ///
    /// # 返回值
    ///
    /// 返回解码后的PCM样本数组
    async fn decode_chunk(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>>;

    /// 完成解码（刷新缓冲区）
    ///
    /// 在所有数据都已处理完毕后调用，用于获取解码器中剩余的数据。
    ///
    /// # 返回值
    ///
    /// 返回解码器中剩余的PCM样本，如果没有剩余数据则返回None
    async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>>;

    /// 重置解码器状态
    ///
    /// 清除解码器的内部状态，准备处理新的音频流。
    fn reset(&mut self);
}

/// 音频编解码器特征
///
/// 提供统一的接口来创建编码器和解码器。
/// 实现此特征的类型需要能够创建特定格式的编解码器实例。
///
/// # 示例
///
/// ```rust
/// use crate::audio::codecs::{AudioCodec, AudioEncoder, AudioDecoder};
/// use crate::audio::format::AudioFormat;
///
/// struct MyCodec;
///
/// impl AudioCodec for MyCodec {
///     fn create_encoder(&self, format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
///         // 创建编码器实例
///         unimplemented!()
///     }
///     
///     fn create_decoder(&self) -> Box<dyn AudioDecoder> {
///         // 创建解码器实例
///         unimplemented!()
///     }
///     
///     fn codec_type(&self) -> crate::audio::format::AudioCodec {
///         // 返回编解码器类型
///         unimplemented!()
///     }
/// }
/// ```
pub trait AudioCodec: Send + Sync {
    /// 创建编码器
    ///
    /// 根据指定的音频格式创建对应的编码器实例。
    ///
    /// # 参数
    ///
    /// * `format` - 目标音频格式
    ///
    /// # 返回值
    ///
    /// 返回创建的编码器实例
    fn create_encoder(
        &self,
        format: &crate::audio::format::AudioFormat,
    ) -> ServiceResult<Box<dyn AudioEncoder>>;

    /// 创建解码器
    ///
    /// 创建当前编解码器类型对应的解码器实例。
    ///
    /// # 返回值
    ///
    /// 返回创建的解码器实例
    fn create_decoder(&self) -> Box<dyn AudioDecoder>;

    /// 获取支持的编解码器类型
    ///
    /// # 返回值
    ///
    /// 返回当前实现支持的编解码器类型
    fn codec_type(&self) -> crate::audio::format::AudioCodec;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::{AudioCodec as CodecType, AudioFormat};
    use tokio::runtime::Runtime;

    // 测试用的编码器实现
    struct TestEncoder;

    impl AudioEncoder for TestEncoder {
        fn encode_samples(&mut self, samples: &[f32]) -> ServiceResult<Option<Bytes>> {
            Ok(Some(Bytes::from(vec![0u8; samples.len() * 2])))
        }

        fn finalize(&mut self) -> ServiceResult<Option<Bytes>> {
            Ok(None)
        }

        fn reset(&mut self) {}
    }

    // 测试用的解码器实现
    struct TestDecoder;

    #[async_trait::async_trait]
    impl AudioDecoder for TestDecoder {
        async fn decode_chunk(&mut self, data: &[u8]) -> ServiceResult<Vec<f32>> {
            Ok(vec![0.0; data.len() / 2])
        }

        async fn flush(&mut self) -> ServiceResult<Option<Vec<f32>>> {
            Ok(None)
        }

        fn reset(&mut self) {}
    }

    // 测试用的编解码器实现
    struct TestCodec;

    impl AudioCodec for TestCodec {
        fn create_encoder(&self, _format: &AudioFormat) -> ServiceResult<Box<dyn AudioEncoder>> {
            Ok(Box::new(TestEncoder))
        }

        fn create_decoder(&self) -> Box<dyn AudioDecoder> {
            Box::new(TestDecoder)
        }

        fn codec_type(&self) -> CodecType {
            CodecType::Wav
        }
    }

    #[test]
    fn test_encoder() {
        let mut encoder = TestEncoder;
        let samples = vec![0.0f32; 1000];

        // 测试编码
        let result = encoder.encode_samples(&samples).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), samples.len() * 2);

        // 测试完成编码
        let final_result = encoder.finalize().unwrap();
        assert!(final_result.is_none());
    }

    #[test]
    fn test_decoder() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut decoder = TestDecoder;
            let data = vec![0u8; 1000];

            // 测试解码
            let samples = decoder.decode_chunk(&data).await.unwrap();
            assert_eq!(samples.len(), data.len() / 2);

            // 测试完成解码
            let final_samples = decoder.flush().await.unwrap();
            assert!(final_samples.is_none());
        });
    }

    #[test]
    fn test_codec() {
        let codec = TestCodec;
        let format = AudioFormat::default();

        // 测试创建编码器
        let encoder = codec.create_encoder(&format).unwrap();
        assert!(encoder.encode_samples(&[0.0; 10]).is_ok());

        // 测试创建解码器
        let decoder = codec.create_decoder();
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            assert!(decoder.decode_chunk(&[0u8; 10]).await.is_ok());
        });

        // 测试编解码器类型
        assert_eq!(codec.codec_type(), CodecType::Wav);
    }
}

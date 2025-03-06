use super::format::AudioFormat;
use crate::error::ServiceResult;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// 音频数据块
///
/// 表示音频流中的一个数据块，包含音频数据和相关的时间信息。
/// 用于在音频处理管道中传输和处理音频数据。
///
/// # 示例
///
/// ```rust
/// use crate::audio::stream::AudioChunk;
/// use bytes::Bytes;
///
/// let chunk = AudioChunk {
///     data: Bytes::from(vec![0u8; 1024]),
///     timestamp: 0,
///     duration: 100,
///     is_last: false
/// };
/// ```
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// 音频数据
    pub data: Bytes,
    /// 时间戳（毫秒）
    pub timestamp: u64,
    /// 持续时间（毫秒）
    pub duration: u64,
    /// 是否为最后一块
    pub is_last: bool,
}

/// 流式音频处理器特征
///
/// 定义了音频处理器的基本接口，用于实现各种音频处理功能。
/// 处理器可以对音频数据进行实时处理，如格式转换、音量调节等。
#[async_trait::async_trait]
pub trait StreamProcessor: Send + Sync {
    /// 处理音频块
    ///
    /// # 参数
    ///
    /// * `chunk` - 待处理的音频块
    ///
    /// # 返回值
    ///
    /// 返回处理后的音频块
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk>;

    /// 完成处理（刷新缓冲区）
    ///
    /// 在处理完最后一块数据后调用，用于处理可能残留在缓冲区中的数据
    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>>;

    /// 重置处理器状态
    ///
    /// 清除处理器的内部状态，准备处理新的音频流
    fn reset(&mut self);

    /// 获取处理器名称
    ///
    /// 返回处理器的类型名称，用于日志和调试
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

/// 音频流
///
/// 表示一个连续的音频数据流，支持异步处理和流式传输。
/// 可以通过处理管道对音频数据进行实时处理。
pub struct AudioStream {
    /// 音频格式
    pub format: AudioFormat,
    /// 接收器
    receiver: Receiver<ServiceResult<AudioChunk>>,
    /// 发送器
    sender: Sender<ServiceResult<AudioChunk>>,
    /// 缓冲区大小
    buffer_size: usize,
}

impl AudioStream {
    /// 创建新的音频流
    ///
    /// # 参数
    ///
    /// * `format` - 音频格式
    /// * `buffer_size` - 缓冲区大小（块数）
    pub fn new(format: AudioFormat, buffer_size: usize) -> Self {
        let (sender, receiver) = channel(buffer_size);
        Self {
            format,
            receiver,
            sender,
            buffer_size,
        }
    }

    /// 从接收器创建音频流
    ///
    /// # 参数
    ///
    /// * `receiver` - 音频块接收器
    /// * `format` - 音频格式
    pub fn from_receiver(
        receiver: Receiver<ServiceResult<AudioChunk>>,
        format: AudioFormat,
    ) -> Self {
        let buffer_size = 100; // 默认缓冲区大小
        let (sender, _) = channel(buffer_size);
        Self {
            format,
            receiver,
            sender,
            buffer_size,
        }
    }

    /// 获取发送器
    pub fn sender(&self) -> Sender<ServiceResult<AudioChunk>> {
        self.sender.clone()
    }

    /// 获取缓冲区大小
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// 创建处理管道
    ///
    /// 将当前音频流连接到一个处理器，创建新的处理管道。
    ///
    /// # 参数
    ///
    /// * `processor` - 实现了StreamProcessor特征的处理器
    ///
    /// # 返回值
    ///
    /// 返回新的音频流，包含处理后的数据
    pub async fn create_pipeline<P: StreamProcessor + 'static>(
        self,
        mut processor: P,
    ) -> ServiceResult<AudioStream> {
        let (new_sender, new_receiver) = channel(self.buffer_size);
        let new_stream = AudioStream {
            format: self.format.clone(),
            receiver: new_receiver,
            sender: new_sender.clone(),
            buffer_size: self.buffer_size,
        };

        // 启动处理任务
        let mut receiver = self.receiver;
        tokio::spawn(async move {
            let mut last_chunk: Option<AudioChunk> = None;

            while let Some(chunk_result) = receiver.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        // 处理当前块
                        match processor.process_chunk(chunk.clone()).await {
                            Ok(processed_chunk) => {
                                if let Err(e) = new_sender.send(Ok(processed_chunk)).await {
                                    error!("Failed to send processed chunk: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                let mut retries = 0;
                                let mut backoff = 100u64; // 初始退避100ms
                                let mut last_error = e;

                                while retries < 3 {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff))
                                        .await;
                                    match processor.process_chunk(chunk.clone()).await {
                                        Ok(processed_chunk) => {
                                            if let Err(e) =
                                                new_sender.send(Ok(processed_chunk)).await
                                            {
                                                error!(
                                                    "Failed to send processed chunk after retry: {}",
                                                    e
                                                );
                                            }
                                            break;
                                        }
                                        Err(e) => {
                                            last_error = e;
                                            backoff *= 2;
                                            retries += 1;
                                        }
                                    }
                                }

                                if retries >= 3 {
                                    if let Err(e) = new_sender.send(Err(last_error)).await {
                                        error!("Failed to send final error: {}", e);
                                    }
                                    break;
                                }
                            }
                        }

                        // 如果是最后一块，保存它用于flush
                        if chunk.is_last {
                            last_chunk = Some(chunk);
                            break;
                        }
                    }
                    Err(e) => {
                        if let Err(e) = new_sender.send(Err(e)).await {
                            error!("Failed to send error: {}", e);
                        }
                        break;
                    }
                }
            }

            // 如果有最后一块，执行flush
            if let Some(_) = last_chunk {
                match processor.flush().await {
                    Ok(Some(final_chunk)) => {
                        if let Err(e) = new_sender.send(Ok(final_chunk)).await {
                            error!("Failed to send final chunk: {}", e);
                        }
                    }
                    Ok(None) => {
                        // 没有最终块，不需要处理
                    }
                    Err(e) => {
                        if let Err(send_err) = new_sender.send(Err(e)).await {
                            error!("Failed to send flush error: {}", send_err);
                        }
                    }
                }
            }
        });

        Ok(new_stream)
    }
}

impl Stream for AudioStream {
    type Item = ServiceResult<AudioChunk>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_recv(cx)
    }
}

/// 音频流分块器
///
/// 用于将连续的音频数据分割成固定大小的块，便于流式处理。
pub struct AudioChunker {
    /// 块大小（字节）
    chunk_size: usize,
    /// 块持续时间（毫秒）
    chunk_duration: u64,
}

impl AudioChunker {
    /// 创建新的分块器
    ///
    /// # 参数
    ///
    /// * `chunk_size` - 块大小（字节）
    /// * `chunk_duration` - 块持续时间（毫秒）
    pub fn new(chunk_size: usize, chunk_duration: u64) -> Self {
        Self {
            chunk_size,
            chunk_duration,
        }
    }

    /// 将音频数据分块
    ///
    /// # 参数
    ///
    /// * `data` - 原始音频数据
    /// * `format` - 音频格式
    ///
    /// # 返回值
    ///
    /// 返回包含分块后数据的音频流
    pub async fn chunk_data(
        &self,
        data: Vec<u8>,
        format: AudioFormat,
    ) -> ServiceResult<AudioStream> {
        let stream = AudioStream::new(format, 100);
        let sender = stream.sender();

        // 计算总块数
        let total_chunks = (data.len() + self.chunk_size - 1) / self.chunk_size;

        // 分块发送
        for (i, chunk) in data.chunks(self.chunk_size).enumerate() {
            let is_last = i == total_chunks - 1;
            let timestamp = i as u64 * self.chunk_duration;

            sender
                .send(Ok(AudioChunk {
                    data: Bytes::copy_from_slice(chunk),
                    timestamp,
                    duration: self.chunk_duration,
                    is_last,
                }))
                .await
                .map_err(|e| {
                    crate::error::ServiceError::Audio(format!("Failed to send audio chunk: {}", e))
                })?;
        }

        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_audio_chunk() {
        let chunk = AudioChunk {
            data: Bytes::from(vec![0u8; 1024]),
            timestamp: 100,
            duration: 50,
            is_last: false,
        };

        assert_eq!(chunk.data.len(), 1024);
        assert_eq!(chunk.timestamp, 100);
        assert_eq!(chunk.duration, 50);
        assert!(!chunk.is_last);
    }

    #[test]
    fn test_audio_stream() {
        let format = AudioFormat::default();
        let stream = AudioStream::new(format.clone(), 100);

        assert_eq!(stream.buffer_size(), 100);
        assert_eq!(stream.format, format);
    }

    #[test]
    fn test_audio_chunker() {
        let chunker = AudioChunker::new(1024, 50);
        let data = vec![0u8; 2500]; // 2500字节的测试数据
        let format = AudioFormat::default();

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let stream = chunker.chunk_data(data.clone(), format).await.unwrap();
            assert_eq!(stream.buffer_size(), 100);

            // 验证分块结果
            let mut total_size = 0;
            while let Some(chunk_result) = stream.receiver.recv().await {
                let chunk = chunk_result.unwrap();
                total_size += chunk.data.len();
            }
            assert_eq!(total_size, data.len());
        });
    }

    #[test]
    fn test_stream_processor() {
        // 创建一个简单的处理器用于测试
        struct TestProcessor;

        #[async_trait::async_trait]
        impl StreamProcessor for TestProcessor {
            async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
                Ok(chunk)
            }

            async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
                Ok(None)
            }

            fn reset(&mut self) {}
        }

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let format = AudioFormat::default();
            let stream = AudioStream::new(format, 100);
            let processor = TestProcessor;

            let processed_stream = stream.create_pipeline(processor).await.unwrap();
            assert_eq!(processed_stream.buffer_size(), 100);
        });
    }
}

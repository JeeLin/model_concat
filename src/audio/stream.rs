//! 音频流处理
//!
//! 定义音频流的数据结构和处理接口，支持流式音频处理。
//! 主要包括音频块（AudioChunk）和音频流（AudioStream）的定义和操作。

use crate::error::ServiceResult;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

/// 音频数据块
///
/// 表示音频流中的一个数据块，包含音频数据和相关元数据。
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

impl AudioChunk {
    /// 创建新的音频数据块
    pub fn new(data: Bytes, timestamp: u64, duration: u64, is_last: bool) -> Self {
        Self {
            data,
            timestamp,
            duration,
            is_last,
        }
    }

    /// 创建空的音频数据块
    pub fn empty() -> Self {
        Self {
            data: Bytes::new(),
            timestamp: 0,
            duration: 0,
            is_last: false,
        }
    }

    /// 获取数据大小（字节数）
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 检查数据块是否为空
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// 音频流处理器接口
///
/// 定义了处理音频流的标准方法，包括处理单个数据块和刷新处理器。
#[async_trait::async_trait]
pub trait StreamProcessor: Send + Sync {
    /// 处理单个音频数据块
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk>;

    /// 刷新处理器，获取可能的剩余数据
    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
        Ok(None)
    }
}

/// 音频流
///
/// 表示一个音频数据流，实现了Stream trait，可以异步迭代音频数据块。
#[derive(Debug, Clone)]
pub struct AudioStream {
    /// 内部数据流
    inner: Pin<Box<dyn Stream<Item = ServiceResult<AudioChunk>> + Send>>,
}

impl AudioStream {
    /// 从任意Stream创建AudioStream
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = ServiceResult<AudioChunk>> + Send + 'static,
    {
        Self {
            inner: Box::pin(stream),
        }
    }

    /// 从音频数据块集合创建AudioStream
    pub fn from_chunks(chunks: Vec<AudioChunk>) -> Self {
        use futures::stream;
        Self::new(stream::iter(chunks.into_iter().map(Ok)))
    }

    /// 从单个音频数据块创建AudioStream
    pub fn from_chunk(chunk: AudioChunk) -> Self {
        Self::from_chunks(vec![chunk])
    }
}

impl Stream for AudioStream {
    type Item = ServiceResult<AudioChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}
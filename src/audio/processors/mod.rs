//! 音频处理器模块
//!
//! 提供音频信号处理的基础功能，包括均衡器、声道处理等。
//! 支持通过处理链组合多个处理器，实现复杂的音频处理流程。

use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;

use crate::error::ServiceResult;

/// 音频处理器接口
///
/// 定义了音频处理器的基本操作，所有具体的处理器都需要实现这个接口。
#[async_trait]
pub trait AudioProcessor: Send + Sync {
    /// 获取处理器名称
    fn name(&self) -> &str;

    /// 处理音频数据
    async fn process(&mut self, data: Bytes) -> ServiceResult<Bytes>;

    /// 重置处理器状态
    fn reset(&mut self);

    /// 获取处理器参数
    fn parameters(&self) -> AudioProcessorParams;
}

/// 音频处理器参数
#[derive(Debug, Clone)]
pub struct AudioProcessorParams {
    /// 采样率
    pub sample_rate: u32,
    /// 通道数
    pub channels: u8,
    /// 采样位数
    pub bits_per_sample: u16,
}
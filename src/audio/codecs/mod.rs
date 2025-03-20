//! 音频编解码器模块
//!
//! 提供各种音频格式的编解码实现。

use async_trait::async_trait;
use bytes::Bytes;

use crate::audio::format::AudioFormat;
use crate::error::ServiceResult;

/// 音频编解码器接口
#[async_trait]
pub trait AudioCodec: Send + Sync {
    /// 获取编解码器名称
    fn name(&self) -> &str;

    /// 编码音频数据
    ///
    /// # 参数
    /// * `data` - 输入的音频数据
    /// * `format` - 音频格式参数
    async fn encode(&mut self, data: Bytes, format: &AudioFormat) -> ServiceResult<Bytes>;

    /// 解码音频数据
    ///
    /// # 参数
    /// * `data` - 输入的音频数据
    /// * `format` - 音频格式参数
    async fn decode(&mut self, data: Bytes, format: &AudioFormat) -> ServiceResult<Bytes>;

    /// 重置编解码器状态
    fn reset(&mut self);
}

mod wav;
mod mp3;
mod ogg;
mod flac;
mod aac;

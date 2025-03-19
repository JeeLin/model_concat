//! 音频处理模块
//!
//! 本模块提供音频格式转换功能，支持不同音频格式之间的转换。
//! 主要功能包括：
//!
//! - 音频格式定义：支持多种编解码器、采样率和通道配置
//! - 音频格式转换：在不同格式间进行转换
//! - 音频流处理：支持流式音频处理

pub mod buffer;
pub mod buffer_pool;
pub mod codecs;
pub mod converter;
pub mod format;
pub mod stream;

pub use stream::StreamProcessor;

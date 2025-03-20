//! 音频处理模块
//!
//! 提供音频格式转换和处理功能，支持多种音频格式之间的转换。
//! 主要功能包括：
//!
//! - 音频格式定义：支持多种编解码器、采样率和通道配置
//! - 音频格式转换：支持一对多的格式转换
//! - 音频处理工具：均衡器、声道处理等
//! - 音频流处理：支持流式和非流式处理
//! - 缓冲区管理：高效的音频数据缓存

pub mod buffer;
pub mod buffer_pool;
pub mod converter;
pub mod format;
pub mod processors;
pub mod stream;
pub mod codecs;

pub use processors::AudioProcessor;
pub use stream::StreamProcessor;

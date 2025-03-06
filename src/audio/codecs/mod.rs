//! 音频编解码器模块
//!
//! 本模块提供了各种音频格式的编码器和解码器实现，支持以下功能：
//!
//! - 统一的编解码器接口：通过`AudioEncoder`和`AudioDecoder` trait定义标准接口
//! - 工厂模式创建：使用`CodecFactory`动态创建编解码器实例
//! - 多格式支持：内置支持WAV、MP3、FLAC、AAC、OGG、OPUS等常见音频格式
//! - 可扩展架构：便于添加新的音频格式支持
//!
//! # 示例
//!
//! ```rust
//! use crate::audio::codecs::{CodecFactory, AudioEncoder, AudioDecoder};
//!
//! // 创建编解码器工厂
//! let factory = CodecFactory::new();
//!
//! // 获取MP3编码器
//! let encoder = factory.create_encoder("mp3", params)?;
//!
//! // 获取WAV解码器
//! let decoder = factory.create_decoder("wav", params)?;
//! ```

// 导出基础组件
pub mod base;
pub mod factory;
pub mod traits;

// 导出各种音频格式的编解码器
pub mod aac;
pub mod alac;
pub mod flac;
pub mod mp3;
pub mod ogg;
pub mod opus;
pub mod wav;

pub use traits::{AudioCodec, AudioDecoder, AudioEncoder};

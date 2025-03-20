//! 模型处理模块
//!
//! 本模块提供了模型处理的核心功能，包括：
//!
//! - 模型接口：定义了模型的标准接口和行为
//! - 数据类型：支持文本和音频等不同类型的数据处理
//! - 模型工厂：负责创建和管理不同类型的模型实例
//! - 配置管理：提供了模型配置的读取和解析功能
//! - 错误处理：定义了模型处理过程中可能出现的错误类型

// 导出子模块
pub mod config;
pub mod error;
pub mod factory;
pub mod provider;
pub mod providers;

use crate::audio::format::AudioFormat;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

// 导出常用类型
pub use config::{ProviderConfig, ProvidersConfig};
pub use error::{ModelError, ModelResult};
pub use provider::{ProtocolHandler, Provider};

/// 模型参数
///
/// 用于配置模型的行为，如温度、最大token数等。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParams {
    /// 温度参数，控制生成文本的随机性
    pub temperature: f32,
    /// 最大生成token数
    pub max_tokens: u32,
    /// Top-p采样参数
    pub top_p: f32,
    /// 频率惩罚参数
    pub frequency_penalty: f32,
    /// 存在惩罚参数
    pub presence_penalty: f32,
}

impl Default for ModelParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 1024,
            top_p: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        }
    }
}

/// 流式模式
///
/// 表示数据是否以流式方式处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamMode {
    /// 非流式模式
    NonStreaming,
    /// 流式模式
    Streaming,
}

/// 数据类型
///
/// 表示模型可以处理的不同类型的数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    /// 文本数据
    Text {
        /// 文本内容
        content: String,
        /// 流式模式
        mode: StreamMode,
    },
    /// 音频数据
    Audio {
        /// 音频格式
        format: AudioFormat,
        /// 音频数据
        data: Vec<u8>,
        /// 流式模式
        mode: StreamMode,
    },
}

/// 流式数据类型
///
/// 表示模型可以处理的不同类型的流式数据。
pub enum StreamDataType {
    /// 文本流
    Text(Pin<Box<dyn Stream<Item=Result<String, ModelError>> + Send>>),
    /// 音频流
    Audio(Pin<Box<dyn Stream<Item=Result<Vec<u8>, ModelError>> + Send>>),
}

/// 支持的格式
///
/// 描述模型支持的输入或输出格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedFormat {
    /// 数据类型（"Text"或"Audio"）
    pub data_type: String,
    /// 是否支持流式处理
    pub streaming: bool,
    /// 音频格式（仅当data_type为"Audio"时有效）
    pub audio_format: Option<AudioFormat>,
}

/// 模型元数据
///
/// 描述模型的基本信息和能力。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型版本
    pub version: String,
    /// 提供商名称
    pub provider: String,
    /// 模型描述
    pub description: String,
    /// 支持的输入格式
    pub input_formats: Vec<SupportedFormat>,
    /// 支持的输出格式
    pub output_formats: Vec<SupportedFormat>,
}

/// 模型接口
#[async_trait::async_trait]
pub trait Model: Send + Sync {
    /// 处理输入数据并返回结果
    async fn process(&self, input: DataType) -> ModelResult<DataType>;

    /// 处理流式输入数据并返回流式结果
    async fn process_stream(&self, input: StreamDataType) -> ModelResult<StreamDataType> {
        Err(ModelError::UnsupportedOperation("流式处理未实现".into()))
    }

    /// 获取模型元数据
    fn metadata(&self) -> ModelMetadata;

    /// 获取模型支持的输入类型
    fn supported_input_formats(&self) -> Vec<SupportedFormat>;

    /// 获取模型支持的输出类型
    fn supported_output_formats(&self) -> Vec<SupportedFormat>;

    /// 是否支持流式处理
    fn supports_streaming(&self) -> bool {
        // 检查是否有任何输入或输出格式支持流式处理
        self.supported_input_formats().iter().any(|f| f.streaming)
            || self.supported_output_formats().iter().any(|f| f.streaming)
    }

    /// 检查模型是否支持指定的输入格式
    fn supports_input_format(&self, format: &AudioFormat) -> bool {
        for input in self.supported_input_formats() {
            if input.data_type == "Audio" && input.audio_format.is_some() {
                let fmt = input.audio_format.as_ref().unwrap();
                if fmt.codec == format.codec
                    && fmt.sample_rate == format.sample_rate
                    && fmt.channels == format.channels
                {
                    return true;
                }
            }
        }
        false
    }

    /// 获取模型支持的所有音频输入格式
    fn supported_audio_input_formats(&self) -> Vec<AudioFormat> {
        let mut formats = Vec::new();
        for input in self.supported_input_formats() {
            if input.data_type == "Audio" && input.audio_format.is_some() {
                formats.push(input.audio_format.unwrap());
            }
        }
        formats
    }

    /// 获取模型提供方名称
    fn provider(&self) -> String;

    /// 获取模型ID
    fn model_id(&self) -> String;
}
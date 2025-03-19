//! 模型处理模块
//!
//! 本模块提供了模型处理的核心功能，包括：
//!
//! - 模型接口：定义了模型的标准接口和行为
//! - 数据类型：支持文本和音频等不同类型的数据处理
//! - 模型工厂：负责创建和管理不同类型的模型实例
//! - 配置管理：提供了模型配置的读取和解析功能
//! - 错误处理：定义了模型处理过程中可能出现的错误类型
//!
//! # 示例
//!
//! ```rust
//! use crate::model::{Model, DataType, ModelFactory, ModelRequest, ModelParams};
//!
//! // 创建模型工厂
//! let factory = ModelFactory::new(config);
//!
//! // 创建模型请求
//! let request = ModelRequest {
//!     provider: "openai".to_string(),
//!     model_id: "gpt-4".to_string(),
//!     parameters: ModelParams::default(),
//! };
//!
//! // 创建模型实例
//! let model = factory.create_model(&request).await?;
//!
//! // 处理文本输入
//! let input = DataType::Text {
//!     content: "请介绍一下人工智能".to_string(),
//!     mode: StreamMode::NonStreaming,
//! };
//! let output = model.process(input).await?;
//! ```

// 导出子模块
pub mod config;
pub mod converter;
pub mod error;
pub mod factory;
pub mod pool;
pub mod provider;
pub mod providers;
pub mod service;

use crate::audio::format::AudioFormat;
use futures::Stream;
use serde::{Deserialize, Serialize};

// 导出常用类型
pub use config::{ProviderConfig, ProvidersConfig};
pub use provider::{ProtocolHandler, Provider};

use crate::audio::stream::AudioStream;
use crate::text::stream::TextStream;
use async_trait::async_trait;

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
/// 表示模型可以处理的数据类型，包括文本和音频。
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
        format: crate::audio::format::AudioFormat,
        /// 音频数据
        data: Vec<u8>,
        /// 流式模式
        mode: StreamMode,
    },
}

/// 流式数据类型
///
/// 表示可以流式处理的数据类型，包括文本流和音频流。
#[derive(Debug)]
pub enum StreamDataType {
    /// 文本流
    Text(TextStream),
    /// 音频流
    Audio(AudioStream),
}

/// 支持的格式
///
/// 表示模型支持的数据格式，包括数据类型、是否支持流式处理等信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedFormat {
    /// 数据类型名称
    pub data_type: String,
    /// 是否支持流式处理
    pub streaming: bool,
    /// 音频格式配置（仅当data_type为"Audio"时有效）
    pub audio_format: Option<crate::audio::format::AudioFormat>,
}

/// 模型元数据
///
/// 包含模型的基本信息，如名称、版本、支持的输入输出格式等。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// 模型名称
    pub name: String,
    /// 模型版本
    pub version: String,
    /// 支持的输入格式
    pub input_formats: Vec<SupportedFormat>,
    /// 支持的输出格式
    pub output_formats: Vec<SupportedFormat>,
    /// 性能指标配置
    #[serde(default)]
    pub metrics_config: config::MetricsConfig,
}

/// 模型接口
#[async_trait]
pub trait Model {
    /// 处理输入数据并返回结果
    async fn process(&self, input: DataType) -> Result<DataType, crate::error::ServiceError>;

    /// 处理流式输入数据并返回流式结果
    async fn process_stream(
        &self,
        input: StreamDataType,
    ) -> Result<StreamDataType, crate::error::ServiceError> {
        Err(crate::error::ServiceError::UnsupportedOperation(
            "流式处理未实现".into(),
        ))
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
    fn supports_input_format(&self, format: &crate::audio::format::AudioFormat) -> bool {
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
    fn supported_audio_input_formats(&self) -> Vec<crate::audio::format::AudioFormat> {
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

/// 创建非流式文本数据
pub fn create_text(content: String) -> DataType {
    DataType::Text {
        content,
        mode: StreamMode::NonStreaming,
    }
}

/// 创建流式文本数据
pub fn create_streaming_text(content: String) -> DataType {
    DataType::Text {
        content,
        mode: StreamMode::Streaming,
    }
}

/// 创建文本流
pub fn create_text_stream(stream: crate::text::stream::TextStream) -> StreamDataType {
    StreamDataType::Text(stream)
}

/// 创建非流式音频数据
pub fn create_audio(format: crate::audio::format::AudioFormat, data: Vec<u8>) -> DataType {
    DataType::Audio {
        format,
        data,
        mode: StreamMode::NonStreaming,
    }
}

/// 创建流式音频数据
pub fn create_streaming_audio(
    format: crate::audio::format::AudioFormat,
    data: Vec<u8>,
) -> DataType {
    DataType::Audio {
        format,
        data,
        mode: StreamMode::Streaming,
    }
}

/// 创建音频流
pub fn create_audio_stream(stream: crate::audio::stream::AudioStream) -> StreamDataType {
    StreamDataType::Audio(stream)
}

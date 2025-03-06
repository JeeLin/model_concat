pub mod adapter_error;
pub mod adapters;
pub mod factory;
pub mod provider_adapter;

/// 模型参数
#[derive(Debug, Clone)]
pub struct ModelParams {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
}

use crate::audio::format::AudioCodec;
use crate::audio::stream::AudioStream;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// 导出ProviderAdapter接口
pub use self::provider_adapter::ProviderAdapter;

// 适配器配置重导出
pub use self::adapters::api_provider::ApiProviderConfig;
pub use self::adapters::ollama_adapter::OllamaConfig;
pub use adapters::ollama_adapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderConfig {
    Api(ApiProviderConfig),
    Ollama(ollama_adapter::OllamaConfig),
}

// 移除重复定义的ApiProviderConfig结构体

#[derive(Debug, Serialize, Deserialize)]
pub enum DataType {
    Text(String),
    Audio { format: AudioFormat, data: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AudioFormat {
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub bit_rate: Option<u32>,
    pub channels: u8,
}

#[async_trait]
pub trait Model {
    async fn process(&self, input: DataType) -> Result<DataType, crate::error::ServiceError>;

    /// 流式处理方法（默认实现）
    async fn process_stream(
        &self,
        input_stream: AudioStream,
    ) -> Result<AudioStream, crate::error::ServiceError> {
        Err(crate::error::ServiceError::UnsupportedOperation(
            "流式处理未实现".into(),
        ))
    }

    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: "".to_string(),
            version: "1.0".to_string(),
            input_formats: self.input_type(),
            output_formats: self.output_type(),
            metrics_config: MetricsConfig::default(),
        }
    }

    fn input_type(&self) -> Vec<DataType>;
    fn output_type(&self) -> Vec<DataType>;
    fn supports_streaming(&self) -> bool;

    /// 检查模型是否支持指定的输入格式
    fn supports_input_format(&self, format: &AudioFormat) -> bool {
        for input in self.input_type() {
            if let DataType::Audio {
                format: ref fmt, ..
            } = input
            {
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

    /// 获取模型支持的所有输入格式
    fn supported_input_formats(&self) -> Vec<AudioFormat> {
        let mut formats = Vec::new();
        for input in self.input_type() {
            if let DataType::Audio { format, .. } = input {
                formats.push(format);
            }
        }
        formats
    }

    /// 获取模型提供方名称
    fn provider(&self) -> String {
        "unknown".to_string()
    }

    /// 获取模型ID
    fn model_id(&self) -> String {
        "unknown".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRuntimeConfig {
    pub protocol: crate::model::adapters::ollama_adapter::OllamaProtocol,
    pub streaming: bool,
}

pub struct ModelMetadata {
    pub name: String,
    pub version: String,
    pub input_formats: Vec<DataType>,
    pub output_formats: Vec<DataType>,
    #[serde(default)]
    pub metrics_config: MetricsConfig,
}

/// 性能指标配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsConfig {
    /// 是否启用耗时统计
    pub enable_latency_metrics: bool,
    /// 是否启用成功率统计
    pub enable_success_metrics: bool,
    /// 是否启用输入输出大小统计
    pub enable_io_metrics: bool,
}

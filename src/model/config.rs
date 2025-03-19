//! 模型配置模块
//!
//! 本模块定义了模型配置的数据结构，按提供商划分配置，包含地址、密钥、支持的模型及其输入输出特性。
//! 主要功能包括：
//!
//! - 提供商配置：按提供商划分的配置结构
//! - 模型特性配置：每种模型的输入输出支持
//! - 连接配置：API地址、密钥等连接信息
//! - 协议配置：HTTP/WebSocket等协议支持

use crate::audio::format::AudioCodec;
use crate::model::{AudioFormat, SupportedFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 模型提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// 提供商名称
    pub name: String,
    /// 基础URL
    pub base_url: String,
    /// API密钥
    pub api_key: String,
    /// 连接超时时间（秒）
    pub timeout_sec: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 是否为内部网络
    pub internal_network: bool,
    /// SSL证书路径（可选）
    pub cert_path: Option<String>,
    /// 支持的模型列表
    pub models: HashMap<String, ModelConfig>,
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型版本
    pub version: String,
    /// 协议类型(http/websocket)
    pub protocol: String,
    /// 默认参数
    pub default_params: ModelParamsConfig,
    /// 支持的输入格式
    pub input_formats: Vec<FormatConfig>,
    /// 支持的输出格式
    pub output_formats: Vec<FormatConfig>,
    /// 性能指标配置
    #[serde(default)]
    pub metrics_config: MetricsConfig,
}

/// 模型参数配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParamsConfig {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
}

impl Default for ModelParamsConfig {
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

/// 格式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatConfig {
    /// 数据类型
    pub data_type: String, // "Text" 或 "Audio"
    /// 是否支持流式处理
    pub streaming: bool,
    /// 音频格式配置（仅当data_type为"Audio"时有效）
    pub audio_format: Option<AudioFormatConfig>,
}

/// 音频格式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormatConfig {
    pub codec: AudioCodec,
    pub sample_rate: u32,
    pub bit_rate: Option<u32>,
    pub channels: u8,
}

impl AudioFormatConfig {
    pub fn to_audio_format(&self) -> AudioFormat {
        AudioFormat {
            codec: self.codec.clone(),
            sample_rate: self.sample_rate,
            bit_rate: self.bit_rate,
            channels: self.channels,
            bits_per_sample: None,
        }
    }
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

/// 将格式配置转换为支持的格式
impl FormatConfig {
    pub fn to_supported_format(&self) -> SupportedFormat {
        SupportedFormat {
            data_type: self.data_type.clone(),
            streaming: self.streaming,
            audio_format: self.audio_format.as_ref().map(|af| af.to_audio_format()),
        }
    }
}

/// 提供商配置集合
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersConfig {
    /// 按提供商名称索引的配置
    pub providers: HashMap<String, ProviderConfig>,
}

impl ProvidersConfig {
    /// 创建新的提供商配置集合
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// 添加提供商配置
    pub fn add_provider(&mut self, provider: ProviderConfig) {
        self.providers.insert(provider.name.clone(), provider);
    }

    /// 获取提供商配置
    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// 获取所有提供商名称
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// 从YAML文件加载配置
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;

        serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse YAML config: {}", e))
    }

    /// 保存配置到YAML文件
    pub fn to_yaml_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| format!("Failed to serialize config to YAML: {}", e))?;

        fs::write(path, content).map_err(|e| format!("Failed to write config file: {}", e))
    }
}

//! 模型配置模块
//!
//! 本模块定义了模型配置的数据结构，按提供商划分配置，包含地址、密钥、支持的模型及其输入输出特性。
//! 主要功能包括：
//!
//! - 提供商配置：按提供商划分的配置结构
//! - 模型特性配置：每种模型的输入输出支持
//! - 连接配置：API地址、密钥等连接信息
//! - 协议配置：HTTP/WebSocket等协议支持

use crate::model::SupportedFormat;
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
    /// 连接协议类型
    pub protocol_type: ProtocolType,
}

/// 协议类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    /// HTTP协议
    Http,
    /// WebSocket协议
    WebSocket,
    /// 同时支持HTTP和WebSocket
    Both,
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
    pub default_params: HashMap<String, serde_json::Value>,
    /// 支持的输入输出格式
    pub supported_formats: SupportedFormat,
    /// 最大输入长度
    pub max_input_length: Option<usize>,
    /// 最大输出长度
    pub max_output_length: Option<usize>,
}

/// 提供商配置集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    /// 提供商配置映射
    pub providers: HashMap<String, ProviderConfig>,
}

impl ProvidersConfig {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let content = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }

    /// 获取提供商配置
    pub fn get_provider(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// 获取模型配置
    pub fn get_model(&self, provider: &str, model_id: &str) -> Option<&ModelConfig> {
        self.providers
            .get(provider)
            .and_then(|p| p.models.get(model_id))
    }
}

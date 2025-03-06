//! 模型适配器注册表模块
//!
//! 本模块提供了一个统一的适配器注册和管理机制，用于管理不同AI模型提供商的适配器。
//! 主要功能包括：
//!
//! - 适配器注册：支持动态注册不同提供商的适配器
//! - 配置管理：统一管理各提供商的配置信息
//! - 适配器实例化：根据提供商名称创建对应的适配器实例
//! - 错误处理：提供统一的错误处理机制
//! - 连接验证：在初始化适配器时验证连接配置和可用性
//!
//! # 示例
//!
//! ```rust
//! use crate::model::adapters::registry::{AdapterRegistry, ProviderConfig};
//!
//! // 创建注册表
//! let mut registry = AdapterRegistry::new();
//!
//! // 注册OpenAI提供商
//! registry.register("openai", ProviderConfig {
//!     api_key: "sk-xxx".to_string(),
//!     base_url: "https://api.openai.com/v1".to_string(),
//!     protocol_type: "http".to_string(),
//!     internal_network: false,
//!     cert_path: None,
//! });
//!
//! // 获取适配器实例
//! let adapter = registry.get_adapter("openai")?;
//! ```

use super::ProviderAdapter;
use super::connection::ProtocolType;
use crate::error::AdapterError;

/// 模型提供商配置
///
/// 包含与模型提供商通信所需的基本配置信息
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// API密钥
    pub api_key: String,
    /// 基础URL
    pub base_url: String,
    /// 协议类型(http/websocket)
    pub protocol_type: String,
    /// 是否为内部网络
    pub internal_network: bool,
    /// SSL证书路径
    pub cert_path: Option<String>,
}

/// 适配器注册表
///
/// 用于管理所有已注册的模型提供商适配器及其配置信息
pub struct AdapterRegistry {
    /// 存储提供商名称到配置的映射
    providers: std::collections::HashMap<String, ProviderConfig>,
}

impl AdapterRegistry {
    /// 创建新的适配器注册表实例
    pub fn new() -> Self {
        Self {
            providers: std::collections::HashMap::new(),
        }
    }

    /// 注册新的模型提供商
    ///
    /// # 参数
    /// * `name` - 提供商名称
    /// * `config` - 提供商配置信息
    pub fn register(&mut self, name: &str, config: ProviderConfig) {
        self.providers.insert(name.to_string(), config);
    }

    /// 获取指定提供商的适配器实例
    ///
    /// # 参数
    /// * `name` - 提供商名称
    ///
    /// # 返回
    /// 返回对应的适配器实例，如果提供商未注册则返回错误
    pub fn get_adapter(&self, name: &str) -> Result<Box<dyn ProviderAdapter>, AdapterError> {
        let config = self
            .providers
            .get(name)
            .ok_or_else(|| AdapterError::ProviderNotFound(name.to_string()))?;
            
        // 检查配置是否有效
        if config.api_key.is_empty() {
            return Err(AdapterError::ConnectionError(format!("{}服务的API密钥不能为空", name)));
        }
        
        if config.base_url.is_empty() {
            return Err(AdapterError::ConnectionError(format!("{}服务的基础URL不能为空", name)));
        }

        // 根据提供商名称创建对应的适配器实例
        let adapter: Box<dyn ProviderAdapter> = match name {
            "anthropic" => Ok(Box::new(super::anthropic::AnthropicAdapter::new(
                config.api_key.clone(),
                config.base_url.clone(),
                &config.protocol_type,
            ))),
            "deepseek" => Ok(Box::new(super::deepseek::DeepSeekAdapter::new(
                config.api_key.clone(),
                config.base_url.clone(),
                &config.protocol_type,
            ))),
            "openai" => Ok(Box::new(super::openai::OpenAIAdapter::new(
                config.api_key.clone(),
                config.base_url.clone(),
                &config.protocol_type,
            ))),
            "self_hosted" => Ok(Box::new(super::self_hosted::SelfHostedAdapter::new(
                config.api_key.clone(),
                config.base_url.clone(),
                &config.protocol_type,
                config.internal_network,
                config.cert_path.clone(),
            ))),
            _ => return Err(AdapterError::UnsupportedProvider(name.to_string())),
        };
        
        Ok(adapter)
    }
}

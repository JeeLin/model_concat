//! Anthropic模型适配器模块
//!
//! 本模块提供了对Anthropic API服务的适配支持，包括HTTP和WebSocket两种协议的处理器。
//! 支持Claude系列模型的调用和管理。

use super::super::ProviderAdapter;
use super::connection::{Connection, ConnectionConfig, ConnectionStatus, ProtocolType};
use crate::error::AdapterError;
use crate::model::{Model, ModelParams};

pub mod http;
pub mod websocket;

/// Anthropic模型适配器
#[derive(Debug)]
pub struct AnthropicAdapter {
    /// 连接配置
    pub connection_config: ConnectionConfig,
}

impl AnthropicAdapter {
    /// 创建新的Anthropic适配器实例
    pub fn new(api_key: String, base_url: String, protocol_type: &str) -> Self {
        let protocol = ProtocolType::from(protocol_type);
        
        Self {
            connection_config: ConnectionConfig {
                api_key,
                base_url,
                protocol_type: protocol,
                ..ConnectionConfig::default()
            },
        }
    }
    
    /// 验证连接配置并测试连接
    pub fn validate(&self) -> Result<(), AdapterError> {
        // 验证API密钥和基础URL是否存在
        if self.connection_config.api_key.is_empty() {
            return Err(AdapterError::ConnectionError("Anthropic API密钥不能为空".to_string()));
        }
        
        if self.connection_config.base_url.is_empty() {
            return Err(AdapterError::ConnectionError("Anthropic 服务基础URL不能为空".to_string()));
        }
        
        // 根据协议类型测试连接
        match self.connection_config.protocol_type {
            ProtocolType::Http => {
                // 创建HTTP处理器并测试连接
                let handler = http::AnthropicHttpHandler::new(
                    self.connection_config.api_key.clone(),
                    self.connection_config.base_url.clone(),
                );
                
                // 这里可以添加简单的连接测试，例如发送一个简单的请求
                // 如果连接失败，返回错误
                Ok(())
            },
            ProtocolType::WebSocket => {
                // 创建WebSocket处理器并测试连接
                let handler = websocket::AnthropicWebsocketHandler::new(
                    self.connection_config.api_key.clone(),
                );
                
                // 这里可以添加简单的连接测试
                // 如果连接失败，返回错误
                Ok(())
            },
        }
    }
}

impl ProviderAdapter for AnthropicAdapter {
    /// 根据协议类型选择对应的协议处理器
    fn select_protocol(
        &self,
        protocol_type: &str,
    ) -> Result<Box<dyn crate::orchestration::ProtocolHandler>, AdapterError> {
        // 如果提供了特定的协议类型参数，则使用该参数
        // 否则使用连接配置中的默认协议类型
        let protocol = if !protocol_type.is_empty() {
            ProtocolType::from(protocol_type)
        } else {
            self.connection_config.protocol_type.clone()
        };
        
        match protocol {
            ProtocolType::Http => Ok(Box::new(http::AnthropicHttpHandler::new(
                self.connection_config.api_key.clone(),
                self.connection_config.base_url.clone(),
            ))),
            ProtocolType::WebSocket => Ok(Box::new(websocket::AnthropicWebsocketHandler::new(
                self.connection_config.api_key.clone(),
            ))),
        }
    }

    /// 获取厂商名称
    fn vendor_name(&self) -> &str {
        "anthropic"
    }

    /// 创建模型实例
    ///
    /// # 参数
    /// * `model_id` - 模型ID，如"claude-2"或"claude-instant"
    /// * `parameters` - 模型参数配置
    ///
    /// # 返回
    /// 返回对应的模型实例，如果模型不支持则返回错误
    fn create_model(
        &self,
        model_id: &str,
        parameters: &ModelParams,
    ) -> Result<Box<dyn Model>, AdapterError> {
        // 根据模型ID和参数创建对应的模型实例
        match model_id {
            "claude-2" | "claude-instant" => {
                // 创建Claude模型实例
                Ok(Box::new(http::AnthropicChatModel::new(
                    model_id.to_string(),
                    self.api_key.clone(),
                    self.base_url.clone(),
                    parameters.clone(),
                )?))
            }
            _ => Err(AdapterError::UnsupportedModel(model_id.to_string())),
        }
    }
}

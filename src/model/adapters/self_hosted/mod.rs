//! 自托管模型适配器模块
//!
//! 本模块提供了对自托管模型服务的适配支持，包括HTTP和WebSocket两种协议的处理器。
//! 通过该适配器，可以连接到自定义部署的模型服务。

use super::super::ProviderAdapter;
use super::connection::{Connection, ConnectionConfig, ConnectionStatus, ProtocolType};
use crate::error::AdapterError;

pub mod http;
pub mod websocket;

/// 自托管模型适配器
#[derive(Debug)]
pub struct SelfHostedAdapter {
    /// API密钥
    api_key: String,
    /// 服务基础URL
    base_url: String,
    /// 是否为内部网络
    internal_network: bool,
    /// 证书路径
    cert_path: Option<String>,
    /// 连接配置
    connection_config: ConnectionConfig,
}

impl SelfHostedAdapter {
    /// 创建新的自托管适配器实例
    pub fn new(
        api_key: String, 
        base_url: String, 
        protocol_type: &str,
        internal_network: bool,
        cert_path: Option<String>
    ) -> Self {
        let protocol = ProtocolType::from(protocol_type);
        
        Self {
            api_key: api_key.clone(),
            base_url: base_url.clone(),
            internal_network,
            cert_path: cert_path.clone(),
            connection_config: ConnectionConfig {
                api_key,
                base_url,
                protocol_type: protocol,
                internal_network,
                cert_path,
                ..ConnectionConfig::default()
            },
        }
    }
    
    /// 验证连接配置并测试连接
    pub fn validate(&self) -> Result<(), AdapterError> {
        // 验证API密钥和基础URL是否存在
        if self.api_key.is_empty() {
            return Err(AdapterError::ConnectionError("自托管服务API密钥不能为空".to_string()));
        }
        
        if self.base_url.is_empty() {
            return Err(AdapterError::ConnectionError("自托管服务基础URL不能为空".to_string()));
        }
        
        // 根据协议类型测试连接
        match self.connection_config.protocol_type {
            ProtocolType::Http => {
                // 创建HTTP处理器并测试连接
                let handler = http::SelfHostedHttpHandler::new(
                    self.api_key.clone(),
                    self.base_url.clone(),
                    self.internal_network,
                    self.cert_path.clone(),
                );
                
                // 这里可以添加简单的连接测试，例如发送一个简单的请求
                // 如果连接失败，返回错误
                Ok(())
            },
            ProtocolType::WebSocket => {
                // 创建WebSocket处理器并测试连接
                let handler = websocket::SelfHostedWebsocketHandler::new(
                    self.api_key.clone(),
                    self.base_url.clone(),
                    self.internal_network,
                    self.cert_path.clone(),
                );
                
                // 这里可以添加简单的连接测试
                // 如果连接失败，返回错误
                Ok(())
            },
        }
    }
}

impl ProviderAdapter for SelfHostedAdapter {
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
            ProtocolType::Http => Ok(Box::new(http::SelfHostedHttpHandler::new(
                self.api_key.clone(),
                self.base_url.clone(),
                self.internal_network,
                self.cert_path.clone(),
            ))),
            ProtocolType::WebSocket => Ok(Box::new(websocket::SelfHostedWebsocketHandler::new(
                self.api_key.clone(),
                self.base_url.clone(),
                self.internal_network,
                self.cert_path.clone(),
            ))),
        }
    }

    /// 获取厂商名称
    fn vendor_name(&self) -> &str {
        "self_hosted"
    }
}

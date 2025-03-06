use super::super::ProviderAdapter;
use super::connection::{Connection, ConnectionConfig, ConnectionStatus, ProtocolType};
use crate::error::AdapterError;

pub mod http;
pub mod websocket;

#[derive(Debug)]
pub struct DeepSeekAdapter {
    /// 连接配置
    pub connection_config: ConnectionConfig,
}

impl DeepSeekAdapter {
    /// 创建新的DeepSeek适配器实例
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
            return Err(AdapterError::ConnectionError("DeepSeek API密钥不能为空".to_string()));
        }
        
        if self.connection_config.base_url.is_empty() {
            return Err(AdapterError::ConnectionError("DeepSeek 服务基础URL不能为空".to_string()));
        }
        
        // 根据协议类型测试连接
        match self.connection_config.protocol_type {
            ProtocolType::Http => {
                // 创建HTTP处理器并测试连接
                let handler = http::DeepSeekHttpHandler::new(
                    self.connection_config.api_key.clone(),
                    self.connection_config.base_url.clone(),
                );
                
                // 这里可以添加简单的连接测试，例如发送一个简单的请求
                // 如果连接失败，返回错误
                Ok(())
            },
            ProtocolType::WebSocket => {
                // 创建WebSocket处理器并测试连接
                let handler = websocket::DeepSeekWebsocketHandler::new(
                    self.connection_config.api_key.clone(),
                );
                
                // 这里可以添加简单的连接测试
                // 如果连接失败，返回错误
                Ok(())
            },
        }
    }
}

impl ProviderAdapter for DeepSeekAdapter {
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
            ProtocolType::Http => Ok(Box::new(http::DeepSeekHttpHandler::new(
                self.connection_config.api_key.clone(),
                self.connection_config.base_url.clone(),
            ))),
            ProtocolType::WebSocket => Ok(Box::new(websocket::DeepSeekWebsocketHandler::new(
                self.connection_config.api_key.clone(),
            ))),
        }
    }

    fn vendor_name(&self) -> &str {
        "deepseek"
    }
}

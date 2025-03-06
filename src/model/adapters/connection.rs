//! 模型连接基础模块
//!
//! 本模块提供了模型连接的基础接口和实现，用于统一HTTP和WebSocket的连接方法。
//! 包括连接配置、连接状态管理、错误处理和重试机制等功能。

use crate::error::AdapterError;
use std::time::Duration;

/// 连接配置
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// API密钥
    pub api_key: String,
    /// 服务基础URL
    pub base_url: String,
    /// 连接超时时间（秒）
    pub timeout_sec: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试延迟（毫秒）
    pub retry_delay_ms: u64,
    /// 是否为内部网络
    pub internal_network: bool,
    /// SSL证书路径
    pub cert_path: Option<String>,
    /// 协议类型
    pub protocol_type: ProtocolType,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            timeout_sec: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
            internal_network: false,
            cert_path: None,
            protocol_type: ProtocolType::Http,
        }
    }
}

/// 协议类型
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolType {
    /// HTTP协议
    Http,
    /// WebSocket协议
    WebSocket,
}

impl From<&str> for ProtocolType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "websocket" | "ws" => ProtocolType::WebSocket,
            _ => ProtocolType::Http,
        }
    }
}

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// 未连接
    Disconnected,
    /// 正在连接
    Connecting,
    /// 已连接
    Connected,
    /// 连接错误
    Error(String),
}

/// 连接接口
pub trait Connection: std::fmt::Debug {
    /// 根据协议类型选择对应的协议处理器
    fn select_protocol(
        &self,
        protocol_type: &str,
        api_key: String,
        base_url: String,
        internal_network: bool,
        cert_path: Option<String>,
    ) -> Result<Box<dyn crate::orchestration::ProtocolHandler>, AdapterError> {
        // 如果提供了特定的协议类型参数，则使用该参数
        // 否则使用连接配置中的默认协议类型
        let protocol = if !protocol_type.is_empty() {
            ProtocolType::from(protocol_type)
        } else {
            self.get_config().protocol_type.clone()
        };
        
        // 根据协议类型创建对应的处理器
        match protocol {
            ProtocolType::Http => self.create_http_handler(api_key, base_url, internal_network, cert_path),
            ProtocolType::WebSocket => self.create_websocket_handler(api_key, base_url, internal_network, cert_path),
        }
    }
    
    /// 创建HTTP协议处理器
    fn create_http_handler(
        &self,
        api_key: String,
        base_url: String,
        internal_network: bool,
        cert_path: Option<String>,
    ) -> Result<Box<dyn crate::orchestration::ProtocolHandler>, AdapterError>;
    
    /// 创建WebSocket协议处理器
    fn create_websocket_handler(
        &self,
        api_key: String,
        base_url: String,
        internal_network: bool,
        cert_path: Option<String>,
    ) -> Result<Box<dyn crate::orchestration::ProtocolHandler>, AdapterError>;
    
    /// 初始化连接
    /// 
    /// 验证配置并准备连接资源
    fn initialize(&mut self) -> Result<(), AdapterError>;
    
    /// 建立连接
    fn connect(&mut self) -> Result<(), AdapterError>;
    
    /// 关闭连接
    fn disconnect(&mut self) -> Result<(), AdapterError>;
    
    /// 获取连接状态
    fn status(&self) -> ConnectionStatus;
    
    /// 验证连接配置
    /// 
    /// 检查必要的配置项是否存在，如API密钥和基础URL
    fn validate_config(&self) -> Result<(), AdapterError> {
        let config = self.get_config();
        
        // 验证API密钥
        if config.api_key.is_empty() {
            return Err(AdapterError::ConnectionError("API密钥不能为空".to_string()));
        }
        
        // 验证基础URL
        if config.base_url.is_empty() {
            return Err(AdapterError::ConnectionError("服务基础URL不能为空".to_string()));
        }
        
        Ok(())
    }
    
    /// 获取连接配置
    fn get_config(&self) -> &ConnectionConfig;
    
    /// 设置连接配置
    fn set_config(&mut self, config: ConnectionConfig);
    
    /// 执行带有重试机制的操作
    fn with_retry<F, T>(&self, operation: F) -> Result<T, AdapterError>
    where
        F: Fn() -> Result<T, AdapterError>,
    {
        let config = self.get_config();
        let mut last_error = None;
        
        for attempt in 0..=config.max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // 记录最后一次错误
                    last_error = Some(e);
                    
                    // 如果不是最后一次尝试，则等待后重试
                    if attempt < config.max_retries {
                        // 在实际实现中，这里应该使用异步等待
                        // 这里使用同步等待作为示例
                        std::thread::sleep(Duration::from_millis(config.retry_delay_ms));
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| AdapterError::Other("未知错误".to_string())))
    }
}

/// 基础连接实现
#[derive(Debug)]
pub struct BaseConnection {
    /// 连接配置
    config: ConnectionConfig,
    /// 连接状态
    status: ConnectionStatus,
}

impl BaseConnection {
    /// 创建新的基础连接
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config,
            status: ConnectionStatus::Disconnected,
        }
    }
    
    /// 设置连接状态
    pub fn set_status(&mut self, status: ConnectionStatus) {
        self.status = status;
    }
}

impl Connection for BaseConnection {
    fn initialize(&mut self) -> Result<(), AdapterError> {
        // 验证配置
        self.validate_config()?;
        
        // 设置状态为未连接
        self.status = ConnectionStatus::Disconnected;
        
        Ok(())
    }
    
    fn connect(&mut self) -> Result<(), AdapterError> {
        // 设置状态为正在连接
        self.status = ConnectionStatus::Connecting;
        
        // 在实际实现中，这里会建立真正的连接
        // 这里只是一个示例实现
        self.status = ConnectionStatus::Connected;
        
        Ok(())
    }
    
    fn disconnect(&mut self) -> Result<(), AdapterError> {
        // 在实际实现中，这里会关闭真正的连接
        // 这里只是一个示例实现
        self.status = ConnectionStatus::Disconnected;
        
        Ok(())
    }
    
    fn status(&self) -> ConnectionStatus {
        self.status.clone()
    }
    
    fn get_config(&self) -> &ConnectionConfig {
        &self.config
    }
    
    fn set_config(&mut self, config: ConnectionConfig) {
        self.config = config;
    }
}
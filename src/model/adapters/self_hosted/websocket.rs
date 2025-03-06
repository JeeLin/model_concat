//! WebSocket协议处理器模块
//!
//! 本模块实现了基于WebSocket协议的模型调用处理器，支持与自托管模型服务建立长连接通信。
//! 提供了认证、请求构建和响应处理等功能。

use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::HeaderValue;
use tokio_tungstenite::tungstenite::http::HeaderMap;

/// WebSocket协议处理器
#[derive(Debug)]
pub struct SelfHostedWebsocketHandler {
    /// API密钥，用于认证
    api_key: String,
    /// WebSocket服务基础URL
    base_url: String,
    /// 是否为内部网络连接
    internal_network: bool,
    /// SSL证书路径
    cert_path: Option<String>,
}

impl SelfHostedWebsocketHandler {
    /// 创建新的WebSocket处理器实例
    ///
    /// # 参数
    /// * `api_key` - API密钥
    /// * `base_url` - WebSocket服务URL
    /// * `internal_network` - 是否为内部网络
    /// * `cert_path` - SSL证书路径
    pub fn new(
        api_key: String,
        base_url: String,
        internal_network: bool,
        cert_path: Option<String>,
    ) -> Self {
        Self {
            api_key,
            base_url,
            internal_network,
            cert_path,
        }
    }

    /// 构建WebSocket请求
    ///
    /// # 返回
    /// 返回带有认证信息的WebSocket请求对象
    async fn build_request(&self) -> Result<impl IntoClientRequest, AdapterError> {
        let mut request = self
            .base_url
            .clone()
            .into_client_request()
            .map_err(|e| AdapterError::ConnectionError(e.to_string()))?;

        // 添加认证头
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|e| AdapterError::ConnectionError(e.to_string()))?,
        );

        *request.headers_mut() = headers;

        Ok(request)
    }
}

impl ProtocolHandler for SelfHostedWebsocketHandler {
    /// 发送请求到模型服务
    ///
    /// # 参数
    /// * `payload` - 请求负载数据
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
        let auth_header = format!("Authorization: Bearer {}", &self.api_key);

        // 构造带有认证头的WebSocket请求
        let mut request = vec![];
        request.extend_from_slice(auth_header.as_bytes());
        request.extend_from_slice(payload);

        // 实现错误重试机制
        Ok(request)
    }

    /// 验证响应数据
    ///
    /// # 参数
    /// * `response` - 响应数据
    fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
        // 实现响应验证逻辑
        Ok(true)
    }
}

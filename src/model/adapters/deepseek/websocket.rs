use super::DeepSeekAdapter;
use crate::orchestration::ProtocolHandler;

#[derive(Debug)]
pub struct DeepSeekWebsocketHandler {
    api_key: String,
}

impl DeepSeekWebsocketHandler {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl ProtocolHandler for DeepSeekWebsocketHandler {
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, crate::error::AdapterError> {
        let auth_header = format!("Authorization: Bearer {}", &self.api_key);

        // 构造带有认证头的WebSocket请求
        let mut request = vec![];
        request.extend_from_slice(auth_header.as_bytes());
        request.extend_from_slice(payload);

        // 实现错误重试机制
        Ok(request)
    }

    fn validate_response(&self, response: &[u8]) -> Result<bool, crate::error::AdapterError> {
        // 实现响应验证逻辑
        Ok(true)
    }
}

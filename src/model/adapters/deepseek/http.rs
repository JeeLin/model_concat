use super::DeepSeekAdapter;
use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;

#[derive(Debug)]
pub struct DeepSeekHttpHandler {
    api_key: String,
    base_url: String,
}

impl DeepSeekHttpHandler {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }
}

impl ProtocolHandler for DeepSeekHttpHandler {
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
        let auth_header = format!("Authorization: Bearer {}", &self.api_key);

        // 构造带有认证头的HTTP请求
        let mut request = vec![];
        request.extend_from_slice(auth_header.as_bytes());
        request.extend_from_slice(payload);

        // 实现错误重试机制
        Ok(request)
    }

    fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
        // 实现响应验证逻辑
        Ok(true)
    }
}

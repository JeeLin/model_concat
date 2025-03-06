use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;
use crate::model::adapters::http::HttpHandler;
use tracing::{debug, error, info};
use serde_json::Value;

#[derive(Debug)]
pub struct AnthropicHttpHandler {
    api_key: String,
    base_url: String,
}

impl AnthropicHttpHandler {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }
}

impl HttpHandler for AnthropicHttpHandler {
    fn send_http_request(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<Vec<u8>, AdapterError> {
        // 创建异步运行时
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| AdapterError::Other(format!("创建运行时失败: {}", e)))?;
        
        rt.block_on(async {
            let client = reqwest::Client::new();
            let url = format!("{}{}", self.base_url, url);
            
            let mut request = client
                .post(&url)
                .header("X-API-Key", &self.api_key)
                .header("Content-Type", "application/json");
            
            // 添加额外的请求头
            for (key, value) in headers {
                request = request.header(key, value);
            }
            
            let response = request
                .body(body.to_vec())
                .send()
                .await
                .map_err(|e| AdapterError::ConnectionError(format!("请求失败: {}", e)))?;
            
            // 检查HTTP状态码
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_else(|_| "无法获取错误详情".to_string());
                
                // 根据状态码返回不同类型的错误
                return match status.as_u16() {
                    401 => Err(AdapterError::AuthError(format!("认证失败: {}", error_text))),
                    429 => Err(AdapterError::RateLimitExceeded(format!("超出请求限制: {}", error_text))),
                    500..=599 => Err(AdapterError::ResponseError(format!("服务器错误: {}", error_text))),
                    _ => Err(AdapterError::ResponseError(format!("请求错误 {}: {}", status, error_text))),
                };
            }
            
            // 获取响应内容
            let bytes = response.bytes().await
                .map_err(|e| AdapterError::ResponseError(format!("读取响应失败: {}", e)))?;
            
            Ok(bytes.to_vec())
        })
    }

    fn handle_http_response(
        &self,
        status_code: u16,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<Vec<u8>, AdapterError> {
        // 验证响应是否为有效的JSON
        match serde_json::from_slice::<Value>(body) {
            Ok(_) => Ok(body.to_vec()),
            Err(e) => Err(AdapterError::ResponseError(format!("无效的响应格式: {}", e))),
        }
    }
}

impl ProtocolHandler for AnthropicHttpHandler {
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
        let response = self.send_http_request("/v1/chat/completions", &[], payload)?;
        self.handle_http_response(200, &[], &response)
    }

    fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
        // 验证响应是否为有效的JSON
        match serde_json::from_slice::<Value>(response) {
            Ok(_) => Ok(true),
            Err(e) => Err(AdapterError::ResponseError(format!("无效的响应格式: {}", e))),
        }
    }
}

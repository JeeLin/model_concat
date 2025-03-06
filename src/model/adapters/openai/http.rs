use super::OpenAIAdapter;
use crate::error::AdapterError;
use crate::model::{AudioFormat, DataType, Model, ModelParams};
use crate::orchestration::ProtocolHandler;
use crate::model::adapters::http::HttpHandler;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{debug, error, info};

#[derive(Debug)]
pub struct OpenAIHttpHandler {
    api_key: String,
    base_url: String,
}

impl OpenAIHttpHandler {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self { api_key, base_url }
    }
}

impl HttpHandler for OpenAIHttpHandler {
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
                .header("Authorization", format!("Bearer {}", self.api_key))
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

impl ProtocolHandler for OpenAIHttpHandler {
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
        let response = self.send_http_request("/chat/completions", &[], payload)?;
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

/// OpenAI聊天模型实现
#[derive(Debug)]
pub struct OpenAIChatModel {
    /// 模型ID
    model_id: String,
    /// API密钥
    api_key: String,
    /// 服务基础URL
    base_url: String,
    /// 模型参数
    parameters: ModelParams,
}

impl OpenAIChatModel {
    /// 创建新的OpenAI聊天模型实例
    pub fn new(
        model_id: String,
        api_key: String,
        base_url: String,
        parameters: ModelParams,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            model_id,
            api_key,
            base_url,
            parameters,
        })
    }
}

#[async_trait]
impl Model for OpenAIChatModel {
    async fn process(
        &self,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> Result<(Vec<u8>, Option<AudioFormat>), crate::error::ServiceError> {
        // 记录开始时间，用于性能监控
        let start_time = Instant::now();
        
        // 将输入转换为文本
        let input_text = String::from_utf8(input.clone())
            .map_err(|e| crate::error::ServiceError::Model(format!("输入数据不是有效的UTF-8文本: {}", e)))?;
        
        // 构建请求
        let request = self.build_request(&input_text);
        let request_json = serde_json::to_vec(&request)
            .map_err(|e| crate::error::ServiceError::Model(format!("序列化请求失败: {}", e)))?;
        
        // 创建HTTP处理器
        let handler = OpenAIHttpHandler::new(self.api_key.clone(), self.base_url.clone());
        
        // 发送请求
        let response_data = handler.send_request(&request_json)
            .map_err(|e| crate::error::ServiceError::Model(format!("请求失败: {}", e)))?;
        
        // 解析响应
        let response: Value = serde_json::from_slice(&response_data)
            .map_err(|e| crate::error::ServiceError::Model(format!("解析响应失败: {}", e)))?;
        
        // 提取生成的文本
        let generated_text = response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| crate::error::ServiceError::Model("响应中未找到生成的文本".to_string()))?;
        
        // 记录处理时间
        let elapsed = start_time.elapsed();
        debug!(
            "OpenAI模型 {} 处理完成，耗时: {}ms",
            self.model_id,
            elapsed.as_millis()
        );
        
        // 返回生成的文本
        Ok((generated_text.as_bytes().to_vec(), None))
    }

    fn input_type(&self) -> Vec<crate::model::DataType> {
        vec![DataType::Text(String::new())]
    }

    fn output_type(&self) -> Vec<crate::model::DataType> {
        vec![DataType::Text(String::new())]
    }

    fn supports_streaming(&self) -> bool {
        false // 当前实现不支持流式处理
    }

    fn model_id(&self) -> String {
        self.model_id.clone()
    }
    
    fn provider(&self) -> String {
        "openai".to_string()
    }
}

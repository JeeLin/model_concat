//! DeepSeek提供商适配器
//!
//! 本模块实现了DeepSeek API的适配器，支持DeepSeek系列模型的调用。
//! 实现了Provider trait，提供了与DeepSeek API交互的标准方法。

use crate::model::config::{ModelConfig, ProviderConfig, ProtocolType};
use crate::model::error::{ModelError, ModelResult};
use crate::model::provider::{ProtocolHandler, Provider, StreamRequestType, StreamResponseType};
use crate::model::{DataType, Model, ModelMetadata, ModelParams, StreamDataType, StreamMode, SupportedFormat};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// DeepSeek提供商
#[derive(Debug)]
pub struct DeepSeekProvider {
    /// 提供商配置
    config: ProviderConfig,
    /// HTTP客户端
    http_client: Client,
}

impl DeepSeekProvider {
    /// 创建新的DeepSeek提供商
    pub fn new(config: ProviderConfig) -> Self {
        // 创建HTTP客户端
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Authorization",
            header::HeaderValue::from_str(&format!("Bearer {}", config.api_key))
                .unwrap_or_default(),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(config.timeout_sec))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            config,
            http_client: client,
        }
    }

    /// 创建HTTP协议处理器
    async fn create_http_handler(&self) -> Box<dyn ProtocolHandler> {
        Box::new(DeepSeekHttpHandler {
            client: self.http_client.clone(),
            base_url: self.config.base_url.clone(),
        })
    }

    /// 创建WebSocket协议处理器
    async fn create_ws_handler(&self) -> ModelResult<Box<dyn ProtocolHandler>> {
        Err(ModelError::UnsupportedOperation(
            "DeepSeek WebSocket协议尚未实现".to_string(),
        ))
    }
}

#[async_trait]
impl Provider for DeepSeekProvider {
    async fn select_protocol(
        &self,
        protocol_type: &str,
    ) -> ModelResult<Box<dyn ProtocolHandler>> {
        match protocol_type.to_lowercase().as_str() {
            "http" => Ok(self.create_http_handler().await),
            "websocket" => self.create_ws_handler().await,
            _ => Err(ModelError::UnsupportedProtocol(protocol_type.to_string())),
        }
    }

    fn name(&self) -> &str {
        "deepseek"
    }

    async fn create_model(
        &self,
        model_id: &str,
        parameters: &ModelParams,
    ) -> ModelResult<Box<dyn Model>> {
        // 检查模型是否存在
        let model_config = self
            .config
            .models
            .get(model_id)
            .ok_or_else(|| ModelError::ModelNotFound(model_id.to_string()))?;

        // 创建模型实例
        let model = DeepSeekModel {
            id: model_id.to_string(),
            provider: self.name().to_string(),
            config: model_config.clone(),
            provider_config: self.config.clone(),
            parameters: parameters.clone(),
            client: self.http_client.clone(),
        };

        Ok(Box::new(model))
    }

    async fn validate(&self) -> ModelResult<()> {
        // 简单验证API密钥是否存在
        if self.config.api_key.is_empty() {
            return Err(ModelError::AuthenticationError(
                "DeepSeek API密钥不能为空".to_string(),
            ));
        }

        // 验证基础URL是否有效
        if self.config.base_url.is_empty() {
            return Err(ModelError::ParameterError(
                "DeepSeek基础URL不能为空".to_string(),
            ));
        }

        Ok(())
    }

    fn supported_models(&self) -> Vec<String> {
        self.config.models.keys().cloned().collect()
    }
}

/// DeepSeek HTTP协议处理器
#[derive(Debug, Clone)]
struct DeepSeekHttpHandler {
    /// HTTP客户端
    client: Client,
    /// 基础URL
    base_url: String,
}

#[async_trait]
impl ProtocolHandler for DeepSeekHttpHandler {
    async fn send_request(&self, payload: &[u8]) -> ModelResult<Vec<u8>> {
        // 发送HTTP请求
        let response = self
            .client
            .post(&self.base_url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(payload.to_vec())
            .send()
            .await
            .map_err(|e| ModelError::ConnectionError(e.to_string()))?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "无法获取错误详情".to_string());
            return Err(ModelError::ApiCallError(format!(
                "HTTP请求失败: {} - {}",
                status, error_text
            )));
        }

        // 获取响应内容
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ModelError::ConnectionError(e.to_string()))?;

        Ok(bytes.to_vec())
    }

    async fn send_stream_request(
        &self,
        payload: &[u8],
        _stream_type: StreamRequestType,
    ) -> ModelResult<StreamResponseType> {
        Err(ModelError::UnsupportedOperation(
            "DeepSeek HTTP流式请求尚未实现".to_string(),
        ))
    }
}

/// DeepSeek模型请求
#[derive(Debug, Serialize, Deserialize)]
struct DeepSeekRequest {
    /// 模型ID
    model: String,
    /// 提示信息
    prompt: String,
    /// 温度参数
    temperature: f32,
    /// 最大生成token数
    max_tokens: u32,
    /// Top-p采样参数
    top_p: f32,
    /// 是否流式输出
    stream: bool,
}

/// DeepSeek模型响应
#[derive(Debug, Serialize, Deserialize)]
struct DeepSeekResponse {
    /// 响应ID
    id: String,
    /// 生成的文本
    text: String,
    /// 使用的token数
    usage: DeepSeekUsage,
}

/// DeepSeek使用统计
#[derive(Debug, Serialize, Deserialize)]
struct DeepSeekUsage {
    /// 提示token数
    prompt_tokens: u32,
    /// 生成token数
    completion_tokens: u32,
    /// 总token数
    total_tokens: u32,
}

/// DeepSeek模型
#[derive(Debug)]
struct DeepSeekModel {
    /// 模型ID
    id: String,
    /// 提供商名称
    provider: String,
    /// 模型配置
    config: ModelConfig,
    /// 提供商配置
    provider_config: ProviderConfig,
    /// 模型参数
    parameters: ModelParams,
    /// HTTP客户端
    client: Client,
}

#[async_trait]
impl Model for DeepSeekModel {
    async fn process(&self, input: DataType) -> ModelResult<DataType> {
        match input {
            DataType::Text { content, mode } => {
                // 创建请求
                let request = DeepSeekRequest {
                    model: self.id.clone(),
                    prompt: content,
                    temperature: self.parameters.temperature,
                    max_tokens: self.parameters.max_tokens,
                    top_p: self.parameters.top_p,
                    stream: mode == StreamMode::Streaming,
                };

                // 序列化请求
                let payload = serde_json::to_vec(&request)
                    .map_err(|e| ModelError::SerializationError(e.to_string()))?;

                // 发送请求
                let protocol = match self.config.protocol.as_str() {
                    "http" => "http",
                    "websocket" => "websocket",
                    _ => "http", // 默认使用HTTP
                };

                let handler = self
                    .provider_config
                    .select_protocol(protocol)
                    .await?;

                let response_data = handler.send_request(&payload).await?;

                // 解析响应
                let response: DeepSeekResponse = serde_json::from_slice(&response_data)
                    .map_err(|e| ModelError::ResponseParseError(e.to_string()))?;

                // 返回结果
                Ok(DataType::Text {
                    content: response.text,
                    mode: StreamMode::NonStreaming,
                })
            }
            DataType::Audio { .. } => Err(ModelError::UnsupportedFormat(
                "DeepSeek模型不支持音频输入".to_string(),
            )),
        }
    }

    async fn process_stream(&self, input: StreamDataType) -> ModelResult<StreamDataType> {
        Err(ModelError::UnsupportedOperation(
            "DeepSeek模型暂不支持流式处理".to_string(),
        ))
    }

    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            id: self.id.clone(),
            name: self.config.name.clone(),
            version: self.config.version.clone(),
            provider: self.provider.clone(),
            description: format!("DeepSeek {} 模型", self.id),
            input_formats: self.supported_input_formats(),
            output_formats: self.supported_output_formats(),
        }
    }

    fn supported_input_formats(&self) -> Vec<SupportedFormat> {
        vec![SupportedFormat {
            data_type: "Text".to_string(),
            streaming: false,
            audio_format: None,
        }]
    }

    fn supported_output_formats(&self) -> Vec<SupportedFormat> {
        vec![SupportedFormat {
            data_type: "Text".to_string(),
            streaming: false,
            audio_format: None,
        }]
    }

    fn provider(&self) -> String {
        self.provider.clone()
    }

    fn model_id(&self) -> String {
        self.id.clone()
    }
}
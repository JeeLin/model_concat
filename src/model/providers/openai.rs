//! OpenAI提供商适配器
//!
//! 本模块实现了OpenAI API的适配器，支持GPT系列模型的调用。
//! 实现了Provider trait，提供了与OpenAI API交互的标准方法。
//!
//! # 功能特性
//!
//! - 支持GPT系列模型的文本处理
//! - 支持HTTP协议与OpenAI API通信
//! - 提供标准化的请求和响应处理
//! - 支持流式和非流式处理模式
//!
//! # 示例
//!
//! ```rust
//! use crate::model::providers::openai::OpenAIProvider;
//! use crate::model::config::ProviderConfig;
//! use crate::model::{Model, ModelParams, DataType};
//!
//! // 创建OpenAI提供商
//! let config = ProviderConfig { /* 配置参数 */ };
//! let provider = OpenAIProvider::new(config);
//!
//! // 创建模型实例
//! let params = ModelParams::default();
//! let model = provider.create_model("gpt-4", &params).await?;
//!
//! // 处理文本输入
//! let input = DataType::Text { content: "请介绍一下人工智能".to_string(), mode: StreamMode::NonStreaming };
//! let output = model.process(input).await?;
//! ```

use crate::model::config::{ModelConfig, ProviderConfig};
use crate::model::error::ModelError;
use crate::model::provider::{ProtocolHandler, Provider, StreamRequestType, StreamResponseType};
use crate::model::{
    DataType, Model, ModelMetadata, ModelParams, StreamDataType, StreamMode, SupportedFormat,
};
use crate::text::stream::{TextChunk, TextStream};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// OpenAI提供商
///
/// 负责创建和管理OpenAI模型实例，处理与OpenAI API的通信。
/// 实现了Provider trait，提供标准化的模型创建和管理方法。
#[derive(Debug)]
pub struct OpenAIProvider {
    /// 提供商配置，包含API密钥、基础URL等信息
    config: ProviderConfig,
    /// HTTP客户端，用于发送API请求
    client: Client,
}

impl OpenAIProvider {
    /// 创建新的OpenAI提供商实例
    ///
    /// # 参数
    ///
    /// * `config` - 提供商配置，包含API密钥、基础URL等信息
    ///
    /// # 返回
    ///
    /// 返回OpenAI提供商实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// let config = ProviderConfig {
    ///     name: "openai".to_string(),
    ///     base_url: "https://api.openai.com/v1".to_string(),
    ///     api_key: "your-api-key".to_string(),
    ///     timeout_sec: 30,
    ///     max_retries: 3,
    ///     internal_network: false,
    ///     cert_path: None,
    ///     models: HashMap::new(),
    /// };
    /// let provider = OpenAIProvider::new(config);
    /// ```
    pub fn new(config: ProviderConfig) -> Self {
        // 创建HTTP客户端
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Bearer {}", config.api_key))
                .expect("Invalid API key"),
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(config.timeout_sec))
            .build()
            .expect("Failed to build HTTP client");

        Self { config, client }
    }
}

/// OpenAI HTTP协议处理器
///
/// 负责处理与OpenAI API的HTTP通信，包括请求构建、发送和响应解析。
/// 实现了ProtocolHandler trait，提供标准化的请求处理方法。
#[derive(Debug)]
pub struct OpenAIHttpHandler {
    /// HTTP客户端，用于发送API请求
    client: Client,
    /// 基础URL，用于构建API请求地址
    base_url: String,
}

/// OpenAI请求结构
///
/// 用于构建发送给OpenAI API的请求体
#[derive(Debug, Serialize)]
pub struct OpenAIRequest {
    /// 模型ID，如 gpt-4, gpt-3.5-turbo 等
    pub model: String,
    /// 消息列表，包含用户输入和系统提示等
    pub messages: Vec<OpenAIMessage>,
    /// 温度参数，控制生成文本的随机性
    pub temperature: f32,
    /// 最大生成token数
    pub max_tokens: u32,
    /// 是否启用流式输出
    pub stream: bool,
    /// Top-p采样参数
    pub top_p: f32,
    /// 频率惩罚参数
    pub frequency_penalty: f32,
    /// 存在惩罚参数
    pub presence_penalty: f32,
}

/// OpenAI消息结构
///
/// 表示对话中的一条消息，包含角色和内容
#[derive(Debug, Serialize)]
pub struct OpenAIMessage {
    /// 消息角色，如 system, user, assistant 等
    pub role: String,
    /// 消息内容
    pub content: String,
}

/// OpenAI响应结构
///
/// 用于解析OpenAI API的响应体
#[derive(Debug, Deserialize)]
pub struct OpenAIResponse {
    /// 响应ID
    pub id: String,
    /// 对象类型，通常为 "chat.completion"
    pub object: String,
    /// 创建时间戳
    pub created: u64,
    /// 模型ID
    pub model: String,
    /// 生成结果列表
    pub choices: Vec<OpenAIChoice>,
}

/// OpenAI选择结构
///
/// 表示API返回的一个生成结果
#[derive(Debug, Deserialize)]
pub struct OpenAIChoice {
    /// 生成的消息
    pub message: OpenAIMessage,
    /// 结束原因，如 "stop", "length" 等
    pub finish_reason: Option<String>,
    /// 索引
    pub index: u32,
}

#[async_trait]
impl ProtocolHandler for OpenAIHttpHandler {
    /// 发送请求并获取响应
    ///
    /// # 参数
    ///
    /// * `payload` - 请求负载，包含序列化后的请求体
    ///
    /// # 返回
    ///
    /// 返回响应字节数组或错误
    async fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, ModelError> {
        // 构建请求URL
        let url = format!("{}/chat/completions", self.base_url);

        // 发送请求
        let response = self
            .client
            .post(&url)
            .body(payload.to_vec())
            .send()
            .await
            .map_err(|e| ModelError::ConnectionError(format!("请求失败: {}", e)))?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "无法获取错误详情".to_string());
            return Err(ModelError::ApiCallError(format!(
                "API调用失败: {} - {}",
                status, error_text
            )));
        }

        // 获取响应体
        let body = response
            .bytes()
            .await
            .map_err(|e| ModelError::ResponseParseError(format!("读取响应失败: {}", e)))?;

        Ok(body.to_vec())
    }

    /// 验证响应是否有效
    ///
    /// # 参数
    ///
    /// * `response` - 响应字节数组
    ///
    /// # 返回
    ///
    /// 返回验证结果或错误
    fn validate_response(&self, response: &[u8]) -> Result<bool, ModelError> {
        // 尝试解析响应
        let result: Result<OpenAIResponse, _> = serde_json::from_slice(response);
        match result {
            Ok(response) => {
                // 检查是否有有效的选择
                Ok(!response.choices.is_empty())
            },
            Err(e) => Err(ModelError::ResponseParseError(format!(
                "响应解析失败: {}",
                e
            ))),
        }
    }

    /// 发送流式请求并获取流式响应
    ///
    /// # 参数
    ///
    /// * `payload` - 请求负载，包含序列化后的请求体
    /// * `stream_type` - 流式请求类型
    ///
    /// # 返回
    ///
    /// 返回流式响应或错误
    async fn send_stream_request(
        &self,
        payload: &[u8],
        _stream_type: StreamRequestType,
    ) -> Result<StreamResponseType, ModelError> {
        // 构建请求URL
        let url = format!("{}/chat/completions", self.base_url);

        // 发送请求
        let response = self
            .client
            .post(&url)
            .body(payload.to_vec())
            .send()
            .await
            .map_err(|e| ModelError::ConnectionError(format!("请求失败: {}", e)))?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "无法获取错误详情".to_string());
            return Err(ModelError::ApiCallError(format!(
                "API调用失败: {} - {}",
                status, error_text
            )));
        }

        // 获取响应流
        let stream = response.bytes_stream().map(|result| match result {
            Ok(bytes) => Ok(bytes.to_vec()),
            Err(e) => Err(ModelError::ResponseParseError(format!(
                "读取响应流失败: {}",
                e
            ))),
        });

        Ok(Box::pin(stream))
    }
}

/// OpenAI模型
///
/// 表示一个OpenAI模型实例，实现了Model trait，提供标准化的模型处理方法。
#[derive(Debug)]
pub struct OpenAIModel {
    /// 模型配置，包含模型ID、名称、版本等信息
    config: ModelConfig,
    /// 模型参数，包含温度、最大token数等
    params: ModelParams,
    /// 协议处理器，用于发送API请求
    protocol: Box<dyn ProtocolHandler>,
    /// 提供商名称
    provider_name: String,
}

#[async_trait]
impl Model for OpenAIModel {
    /// 处理输入数据并返回结果
    ///
    /// # 参数
    ///
    /// * `input` - 输入数据，支持文本类型
    ///
    /// # 返回
    ///
    /// 返回处理结果或错误
    async fn process(&self, input: DataType) -> Result<DataType, crate::error::ServiceError> {
        match input {
            DataType::Text { content, mode } => {
                // 构建请求
                let request = OpenAIRequest {
                    model: self.config.id.clone(),
                    messages: vec![OpenAIMessage {
                        role: "user".to_string(),
                        content,
                    }],
                    temperature: self.params.temperature,
                    max_tokens: self.params.max_tokens,
                    stream: false,
                    top_p: self.params.top_p,
                    frequency_penalty: self.params.frequency_penalty,
                    presence_penalty: self.params.presence_penalty,
                };

                // 序列化请求
                let payload = serde_json::to_vec(&request).map_err(|e| {
                    crate::error::ServiceError::SerializationError(format!("请求序列化失败: {}", e))
                })?;

                // 发送请求
                let response = self
                    .protocol
                    .send_request(&payload)
                    .await
                    .map_err(|e| crate::error::ServiceError::from(e))?;

                // 验证响应
                self.protocol
                    .validate_response(&response)
                    .map_err(|e| crate::error::ServiceError::from(e))?;

                // 解析响应
                let response: OpenAIResponse = serde_json::from_slice(&response).map_err(|e| {
                    crate::error::ServiceError::ResponseParseError(format!("响应解析失败: {}", e))
                })?;

                // 提取生成的文本
                if let Some(choice) = response.choices.first() {
                    Ok(DataType::Text {
                        content: choice.message.content.clone(),
                        mode: StreamMode::NonStreaming,
                    })
                } else {
                    Err(crate::error::ServiceError::ResponseParseError(
                        "响应中没有有效的生成结果".to_string(),
                    ))
                }
            },
            DataType::Audio { .. } => Err(crate::error::ServiceError::UnsupportedFormat(
                "OpenAI GPT模型不支持音频输入".to_string(),
            )),
        }
    }

    /// 处理流式输入数据并返回流式结果
    ///
    /// # 参数
    ///
    /// * `input` - 流式输入数据，支持文本流
    ///
    /// # 返回
    ///
    /// 返回流式处理结果或错误
    async fn process_stream(
        &self,
        input: StreamDataType,
    ) -> Result<StreamDataType, crate::error::ServiceError> {
        match input {
            StreamDataType::Text(text_stream) => {
                // 收集文本流中的所有块
                let mut full_text = String::new();
                let mut stream = text_stream;

                while let Some(result) = stream.next().await {
                    let chunk = result.map_err(|e| {
                        crate::error::ServiceError::Other(format!("读取文本流失败: {}", e))
                    })?;
                    full_text.push_str(&chunk.text);
                }

                // 构建请求
                let request = OpenAIRequest {
                    model: self.config.id.clone(),
                    messages: vec![OpenAIMessage {
                        role: "user".to_string(),
                        content: full_text,
                    }],
                    temperature: self.params.temperature,
                    max_tokens: self.params.max_tokens,
                    stream: true,
                    top_p: self.params.top_p,
                    frequency_penalty: self.params.frequency_penalty,
                    presence_penalty: self.params.presence_penalty,
                };

                // 序列化请求
                let payload = serde_json::to_vec(&request).map_err(|e| {
                    crate::error::ServiceError::SerializationError(format!("请求序列化失败: {}", e))
                })?;

                // 发送流式请求
                let response_stream = self
                    .protocol
                    .send_stream_request(&payload, StreamRequestType::Text(TextStream::empty()))
                    .await
                    .map_err(|e| crate::error::ServiceError::from(e))?;

                // 处理响应流
                let text_stream = process_openai_stream(response_stream);

                Ok(StreamDataType::Text(text_stream))
            },
            StreamDataType::Audio(_) => Err(crate::error::ServiceError::UnsupportedFormat(
                "OpenAI GPT模型不支持音频流处理".to_string(),
            )),
        }
    }

    /// 获取模型元数据
    ///
    /// # 返回
    ///
    /// 返回模型元数据，包含名称、版本、支持的输入输出格式等
    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            name: self.config.name.clone(),
            version: self.config.version.clone(),
            input_formats: self.supported_input_formats(),
            output_formats: self.supported_output_formats(),
            metrics_config: self.config.metrics_config.clone(),
        }
    }

    /// 获取模型支持的输入类型
    ///
    /// # 返回
    ///
    /// 返回模型支持的输入类型列表
    fn supported_input_formats(&self) -> Vec<SupportedFormat> {
        self.config
            .input_formats
            .iter()
            .map(|f| f.to_supported_format())
            .collect()
    }

    /// 获取模型支持的输出类型
    ///
    /// # 返回
    ///
    /// 返回模型支持的输出类型列表
    fn supported_output_formats(&self) -> Vec<SupportedFormat> {
        self.config
            .output_formats
            .iter()
            .map(|f| f.to_supported_format())
            .collect()
    }

    /// 获取模型提供方名称
    ///
    /// # 返回
    ///
    /// 返回提供方名称，即"openai"
    fn provider(&self) -> String {
        self.provider_name.clone()
    }

    /// 获取模型ID
    ///
    /// # 返回
    ///
    /// 返回模型ID，如 gpt-4, gpt-3.5-turbo 等
    fn model_id(&self) -> String {
        self.config.id.clone()
    }
}

/// 处理OpenAI流式响应
///
/// 将OpenAI的流式响应转换为TextStream
fn process_openai_stream(stream: StreamResponseType) -> TextStream {
    // 这里需要实现具体的流处理逻辑
    // 简化版本，实际实现需要处理SSE格式和JSON解析
    let processed_stream = stream.map(|result| {
        result
            .map(|bytes| {
                // 简单示例，实际实现需要解析SSE格式
                let text = String::from_utf8_lossy(&bytes).to_string();
                TextChunk::new(text, false)
            })
            .map_err(|e| e.into())
    });

    TextStream::new(processed_stream)
}

#[async_trait]
impl Provider for OpenAIProvider {
    /// 根据协议类型选择对应的协议处理器
    ///
    /// # 参数
    ///
    /// * `protocol_type` - 协议类型，目前仅支持"http"
    ///
    /// # 返回
    ///
    /// 返回协议处理器或错误
    async fn select_protocol(
        &self,
        protocol_type: &str,
    ) -> Result<Box<dyn ProtocolHandler>, ModelError> {
        match protocol_type {
            "http" => {
                // 创建HTTP协议处理器
                let handler = OpenAIHttpHandler {
                    client: self.client.clone(),
                    base_url: self.config.base_url.clone(),
                };
                Ok(Box::new(handler))
            },
            "websocket" => Err(ModelError::UnsupportedProtocol(
                "OpenAI提供商不支持WebSocket协议".to_string(),
            )),
            _ => Err(ModelError::UnsupportedProtocol(format!(
                "不支持的协议类型: {}",
                protocol_type
            ))),
        }
    }

    /// 获取提供商名称
    ///
    /// # 返回
    ///
    /// 返回提供商名称，即"openai"
    fn name(&self) -> &str {
        &self.config.name
    }

    /// 创建模型实例
    ///
    /// # 参数
    ///
    /// * `model_id` - 模型ID，如 gpt-4, gpt-3.5-turbo 等
    /// * `parameters` - 模型参数，包含温度、最大token数等
    ///
    /// # 返回
    ///
    /// 返回模型实例或错误
    ///
    /// # 示例
    ///
    /// ```rust
    /// let params = ModelParams::default();
    /// let model = provider.create_model("gpt-4", &params).await?;
    /// ```
    async fn create_model(
        &self,
        model_id: &str,
        parameters: &ModelParams,
    ) -> Result<Box<dyn Model>, ModelError> {
        // 获取模型配置
        let model_config = self
            .config
            .models
            .get(model_id)
            .ok_or_else(|| ModelError::ModelNotFound(model_id.to_string()))?
            .clone();

        // 选择协议处理器
        let protocol = self.select_protocol(&model_config.protocol).await?;

        // 创建模型实例
        let model = OpenAIModel {
            config: model_config,
            params: parameters.clone(),
            protocol,
            provider_name: self.config.name.clone(),
        };

        Ok(Box::new(model))
    }

    /// 验证提供商配置
    ///
    /// # 返回
    ///
    /// 返回验证结果或错误
    async fn validate(&self) -> Result<(), ModelError> {
        // 简单验证，检查API密钥是否存在
        if self.config.api_key.is_empty() {
            return Err(ModelError::AuthenticationError(
                "OpenAI API密钥不能为空".to_string(),
            ));
        }
        Ok(())
    }

    /// 获取支持的模型列表
    ///
    /// # 返回
    ///
    /// 返回支持的模型ID列表
    fn supported_models(&self) -> Vec<String> {
        self.config.models.keys().cloned().collect()
    }
}

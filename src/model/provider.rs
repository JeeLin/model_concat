//! 模型提供商接口模块
//!
//! 本模块定义了模型提供商的核心接口，用于统一不同AI服务提供商的接入方式。
//! 所有模型提供商实现都必须实现这个接口，以便系统能够统一管理和使用不同的模型服务。

use crate::model::error::ModelResult;
use crate::model::{Model, ModelParams};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// 协议处理器接口
#[async_trait]
pub trait ProtocolHandler: Send + Sync + std::fmt::Debug {
    /// 发送请求并获取响应
    async fn send_request(&self, payload: &[u8]) -> ModelResult<Vec<u8>>;

    /// 发送流式请求并获取流式响应
    ///
    /// 默认实现返回不支持流式处理的错误
    async fn send_stream_request(
        &self,
        payload: &[u8],
        stream_type: StreamRequestType,
    ) -> ModelResult<StreamResponseType> {
        Err(crate::model::error::ModelError::UnsupportedOperation(
            "流式处理未实现".to_string(),
        ))
    }
}

/// 流式请求类型
#[derive(Debug, Clone)]
pub enum StreamRequestType {
    /// 文本流请求
    Text(Pin<Box<dyn Stream<Item = Result<String, crate::model::error::ModelError>> + Send>>),
    /// 音频流请求
    Audio(Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::model::error::ModelError>> + Send>>),
}

/// 流式响应类型
pub type StreamResponseType = Pin<Box<dyn Stream<Item = Result<Vec<u8>, crate::model::error::ModelError>> + Send>>;

/// 模型提供商接口
///
/// 定义了与不同AI服务提供商交互的标准方法。该接口用于抽象不同提供商的API差异，
/// 确保系统能够统一地处理各种模型服务的请求和响应。
#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    /// 根据协议类型选择对应的协议处理器
    ///
    /// # 参数
    /// * `protocol_type` - 协议类型，如"http"或"websocket"
    ///
    /// # 返回
    /// * `Result<Box<dyn ProtocolHandler>, ModelError>` - 协议处理器或错误
    async fn select_protocol(
        &self,
        protocol_type: &str,
    ) -> ModelResult<Box<dyn ProtocolHandler>>;

    /// 获取提供商名称
    ///
    /// # 返回
    /// * `&str` - 提供商名称
    fn name(&self) -> &str;

    /// 创建模型实例
    ///
    /// # 参数
    /// * `model_id` - 模型ID
    /// * `parameters` - 模型参数
    ///
    /// # 返回
    /// * `Result<Box<dyn Model>, ModelError>` - 模型实例或错误
    async fn create_model(
        &self,
        model_id: &str,
        parameters: &ModelParams,
    ) -> ModelResult<Box<dyn Model>>;

    /// 验证提供商配置
    ///
    /// # 返回
    /// * `Result<(), ModelError>` - 成功或错误
    async fn validate(&self) -> ModelResult<()>;

    /// 获取支持的模型列表
    ///
    /// # 返回
    /// * `Vec<String>` - 支持的模型ID列表
    fn supported_models(&self) -> Vec<String>;
}
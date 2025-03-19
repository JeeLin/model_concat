//! 模型错误处理模块
//!
//! 本模块定义了模型处理过程中可能出现的各种错误类型和处理方法。
//! 主要包括连接错误、参数错误、模型加载错误等。

use thiserror::Error;

/// 模型错误类型
#[derive(Error, Debug)]
pub enum ModelError {
    /// 连接错误
    #[error("连接错误: {0}")]
    ConnectionError(String),

    /// 认证错误
    #[error("认证错误: {0}")]
    AuthenticationError(String),

    /// 参数错误
    #[error("参数错误: {0}")]
    ParameterError(String),

    /// 模型加载错误
    #[error("模型加载错误: {0}")]
    ModelLoadError(String),

    /// 不支持的格式
    #[error("不支持的格式: {0}")]
    UnsupportedFormat(String),

    /// 不支持的操作
    #[error("不支持的操作: {0}")]
    UnsupportedOperation(String),

    /// 不支持的协议
    #[error("不支持的协议: {0}")]
    UnsupportedProtocol(String),

    /// 提供商不存在
    #[error("提供商不存在: {0}")]
    ProviderNotFound(String),

    /// 模型不存在
    #[error("模型不存在: {0}")]
    ModelNotFound(String),

    /// API调用错误
    #[error("API调用错误: {0}")]
    ApiCallError(String),

    /// 响应解析错误
    #[error("响应解析错误: {0}")]
    ResponseParseError(String),

    /// 超时错误
    #[error("请求超时: {0}秒")]
    TimeoutError(u64),

    /// IO错误
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),

    /// 序列化错误
    #[error("序列化错误: {0}")]
    SerializationError(String),

    /// 其他错误
    #[error("其他错误: {0}")]
    Other(String),
}

/// 模型结果类型
pub type ModelResult<T> = Result<T, ModelError>;

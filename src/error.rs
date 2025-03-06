//! 错误处理模块
//!
//! 本模块定义了系统中使用的错误类型和处理机制，主要包括：
//!
//! - 服务错误（ServiceError）：定义核心服务相关的错误类型
//! - 适配器错误（AdapterError）：定义与模型适配器相关的错误类型
//! - 错误转换：支持标准错误类型和自定义错误类型之间的转换
//! - 错误处理：提供统一的错误处理和结果类型
//!
//! # 示例
//!
//! ```rust
//! use crate::error::{ServiceError, ServiceResult};
//!
//! fn process_data() -> ServiceResult<()> {
//!     // 处理可能出错的操作
//!     if some_condition {
//!         return Err(ServiceError::InvalidFormat("Invalid data format".to_string()));
//!     }
//!     Ok(())
//! }
//! ```

use crate::model::adapter_error::AdapterError;
use thiserror::Error;

/// 服务错误类型
#[derive(Error, Debug)]
pub enum ServiceError {
    /// IO错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 模型处理错误
    #[error("Model processing error: {0}")]
    ModelProcessing(String),

    /// 输入格式错误
    #[error("Invalid input format: {0}")]
    InvalidFormat(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// 音频错误
    #[error("Audio error: {0}")]
    AudioEncoding(Err),

    /// 音频转换错误
    #[error("Audio conversion error: {0}")]
    AudioConversion(String),

    /// 流水线执行错误
    #[error("Pipeline execution error: {0}")]
    PipelineExecution(String),

    /// 不支持的格式
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// 模型错误
    #[error("Model error: {0}")]
    AdapterError(AdapterError),

    /// 不支持的操作
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
}

/// 服务结果类型
///
/// 用于统一处理可能出错的操作结果
pub type ServiceResult<T> = Result<T, ServiceError>;

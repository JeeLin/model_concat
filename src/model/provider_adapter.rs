//! 模型提供商适配器接口模块
//!
//! 本模块定义了模型提供商适配器的核心接口，用于统一不同AI服务提供商的接入方式。
//! 所有模型提供商适配器都必须实现这个接口，以便系统能够统一管理和使用不同的模型服务。

use crate::error::AdapterError;
use crate::model::{Model, ModelParams};
use crate::orchestration::ProtocolHandler;

/// 模型提供商适配器接口
///
/// 定义了与不同AI服务提供商交互的标准方法。该接口用于抽象不同提供商的API差异，
/// 确保系统能够统一地处理各种模型服务的请求和响应。
///
/// # 示例
///
/// ```rust
/// use crate::model::provider_adapter::ProviderAdapter;
/// use crate::error::AdapterError;
/// use crate::model::{Model, ModelParams};
/// use crate::orchestration::ProtocolHandler;
///
/// struct MyProviderAdapter {
///     api_key: String,
///     base_url: String,
/// }
///
/// impl ProviderAdapter for MyProviderAdapter {
///     fn select_protocol(&self, protocol_type: &str) -> Result<Box<dyn ProtocolHandler>, AdapterError> {
///         // 实现协议选择逻辑
///         unimplemented!()
///     }
///     
///     fn vendor_name(&self) -> &str {
///         "my_provider"
///     }
///     
///     fn create_model(&self, model_id: &str, parameters: &ModelParams) -> Result<Box<dyn Model>, AdapterError> {
///         // 实现模型创建逻辑
///         unimplemented!()
///     }
/// }
/// ```
pub trait ProviderAdapter: std::fmt::Debug {
    /// 根据协议类型选择对应的协议处理器
    ///
    /// # 参数
    /// * `protocol_type` - 协议类型，如"http"或"websocket"
    ///
    /// # 返回
    /// 返回对应的协议处理器实例，如果协议不支持则返回错误
    fn select_protocol(
        &self,
        protocol_type: &str,
    ) -> Result<Box<dyn ProtocolHandler>, AdapterError>;

    /// 获取厂商名称
    ///
    /// # 返回
    /// 返回提供商的唯一标识名称
    fn vendor_name(&self) -> &str;

    /// 创建模型实例
    ///
    /// # 参数
    /// * `model_id` - 模型ID，如"gpt-3.5-turbo"或"claude-2"
    /// * `parameters` - 模型参数配置
    ///
    /// # 返回
    /// 返回对应的模型实例，如果模型不支持则返回错误
    fn create_model(
        &self,
        model_id: &str,
        parameters: &ModelParams,
    ) -> Result<Box<dyn Model>, AdapterError>;
}
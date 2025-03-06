//! 模型工厂模块
//!
//! 本模块提供了模型实例的创建和管理功能，主要包括：
//!
//! - 模型工厂：负责创建和管理不同类型的模型实例
//! - 提供商注册：支持动态注册不同的模型提供商
//! - 参数配置：统一管理模型参数和配置信息
//! - 实例生命周期：管理模型实例的创建和销毁
//!
//! # 示例
//!
//! ```rust
//! use crate::model::factory::{ModelFactory, ModelParams};
//!
//! // 创建模型工厂
//! let mut factory = ModelFactory::new();
//!
//! // 注册模型提供商
//! factory.register_provider("openai", config);
//!
//! // 创建模型实例
//! let model = factory.create_model("openai", "gpt-3.5-turbo", &params).await?;
//! ```

use crate::error::{ServiceError, ServiceResult};
use crate::model::adapters::registry::{AdapterRegistry, ProviderConfig};
use crate::model::{Model, ModelParams};
use std::sync::Arc;

/// 模型工厂
pub struct ModelFactory {
    /// 适配器注册表
    registry: AdapterRegistry,
}

impl ModelFactory {
    /// 创建新的模型工厂
    pub fn new() -> Self {
        Self {
            registry: AdapterRegistry::new(),
        }
    }

    /// 注册模型提供方
    pub fn register_provider(&mut self, name: &str, config: ProviderConfig) {
        self.registry.register(name, config);
    }

    /// 创建模型实例
    pub async fn create_model(
        &self,
        provider: &str,
        model_id: &str,
        parameters: &ModelParams,
    ) -> ServiceResult<Arc<dyn Model>> {
        // 获取提供方适配器
        let adapter = self
            .registry
            .get_adapter(provider)
            .map_err(|e| ServiceError::Model(format!("获取提供方适配器失败: {}", e)))?;

        // 创建模型实例
        let model = adapter
            .create_model(model_id, parameters)
            .map_err(|e| ServiceError::Model(format!("创建模型实例失败: {}", e)))?;

        Ok(Arc::new(model))
    }
}

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
//! use crate::model::factory::{ModelFactory, ModelRequest};
//! use crate::model::ModelParams;
//!
//! // 创建模型工厂
//! let factory = ModelFactory::new(providers_config);
//!
//! // 创建模型请求
//! let request = ModelRequest {
//!     provider: "openai".to_string(),
//!     model_id: "gpt-4".to_string(),
//!     parameters: ModelParams::default(),
//! };
//!
//! // 创建模型实例
//! let model = factory.create_model(&request).await?;
//! ```

use crate::model::config::{ProviderConfig, ProvidersConfig};
use crate::model::error::{ModelError, ModelResult};
use crate::model::provider::Provider;
use crate::model::{Model, ModelParams};
use std::collections::HashMap;
use std::sync::Arc;

/// 模型请求
#[derive(Debug, Clone)]
pub struct ModelRequest {
    /// 提供商名称
    pub provider: String,
    /// 模型ID
    pub model_id: String,
    /// 模型参数
    pub parameters: ModelParams,
}

/// 模型工厂
pub struct ModelFactory {
    /// 提供商实例映射
    providers: HashMap<String, Arc<dyn Provider>>,
    /// 提供商配置
    config: ProvidersConfig,
}

impl ModelFactory {
    /// 创建新的模型工厂
    pub fn new(config: ProvidersConfig) -> Self {
        Self {
            providers: HashMap::new(),
            config,
        }
    }

    /// 初始化提供商
    pub async fn init_providers(&mut self) -> ModelResult<()> {
        // 清空现有提供商
        self.providers.clear();

        // 遍历配置中的所有提供商
        for (name, config) in &self.config.providers {
            // 根据提供商类型创建对应的提供商实例
            let provider = self.create_provider(name, config).await?;
            self.providers.insert(name.clone(), Arc::new(provider));
        }

        Ok(())
    }

    /// 创建提供商实例
    async fn create_provider(
        &self,
        name: &str,
        config: &ProviderConfig,
    ) -> ModelResult<Box<dyn Provider>> {
        // 根据提供商名称创建对应的提供商实例
        match name {
            "openai" => {
                // 创建OpenAI提供商
                use crate::model::providers::OpenAIProvider;
                let provider = OpenAIProvider::new(config.clone());
                Ok(Box::new(provider))
            },
            "anthropic" => {
                // 创建Anthropic提供商
                // 注意：Anthropic提供商尚未实现，需要先实现providers::anthropic模块
                // 取消下面注释并实现AnthropicProvider后可用
                // use crate::model::providers::AnthropicProvider;
                // let provider = AnthropicProvider::new(config.clone());
                // Ok(Box::new(provider))
                Err(ModelError::Other(format!("提供商 {} 尚未实现", name)))
            },
            "deepseek" => {
                // 创建DeepSeek提供商
                // 注意：DeepSeek提供商尚未实现，需要先实现providers::deepseek模块
                // 取消下面注释并实现DeepSeekProvider后可用
                // use crate::model::providers::DeepSeekProvider;
                // let provider = DeepSeekProvider::new(config.clone());
                // Ok(Box::new(provider))
                Err(ModelError::Other(format!("提供商 {} 尚未实现", name)))
            },
            _ => Err(ModelError::ProviderNotFound(name.to_string())),
        }
    }

    /// 创建模型实例
    pub async fn create_model(&self, request: &ModelRequest) -> ModelResult<Box<dyn Model>> {
        // 获取提供商
        let provider = self
            .providers
            .get(&request.provider)
            .ok_or_else(|| ModelError::ProviderNotFound(request.provider.clone()))?;

        // 创建模型实例
        provider
            .create_model(&request.model_id, &request.parameters)
            .await
    }

    /// 获取所有已注册的提供商名称
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// 获取提供商支持的模型列表
    pub fn supported_models(&self, provider_name: &str) -> ModelResult<Vec<String>> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| ModelError::ProviderNotFound(provider_name.to_string()))?;

        Ok(provider.supported_models())
    }

    /// 验证提供商配置
    pub async fn validate_provider(&self, provider_name: &str) -> ModelResult<()> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| ModelError::ProviderNotFound(provider_name.to_string()))?;

        provider.validate().await
    }
}

//! 模型工厂模块
//!
//! 本模块提供了模型实例的创建和管理功能，主要包括：
//!
//! - 模型工厂：负责创建和管理不同类型的模型实例
//! - 提供商注册：支持动态注册不同的模型提供商
//! - 参数配置：统一管理模型参数和配置信息
//! - 实例生命周期：管理模型实例的创建和销毁

use crate::model::config::{ProviderConfig, ProvidersConfig};
use crate::model::error::{ModelError, ModelResult};
use crate::model::provider::Provider;
use crate::model::providers::deepseek::DeepSeekProvider;
use crate::model::providers::openai::OpenAIProvider;
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
        let mut factory = Self {
            providers: HashMap::new(),
            config,
        };
        factory.register_default_providers();
        factory
    }

    /// 注册默认提供商
    fn register_default_providers(&mut self) {
        // 注册OpenAI提供商
        if let Some(config) = self.config.get_provider("openai") {
            let provider = OpenAIProvider::new(config.clone());
            self.register_provider("openai", Arc::new(provider));
        }

        // 注册DeepSeek提供商
        if let Some(config) = self.config.get_provider("deepseek") {
            let provider = DeepSeekProvider::new(config.clone());
            self.register_provider("deepseek", Arc::new(provider));
        }

        // 可以注册更多提供商...
    }

    /// 注册提供商
    pub fn register_provider(&mut self, name: &str, provider: Arc<dyn Provider>) {
        self.providers.insert(name.to_string(), provider);
    }

    /// 获取提供商
    pub fn get_provider(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }

    /// 创建模型实例
    pub async fn create_model(&self, request: &ModelRequest) -> ModelResult<Box<dyn Model>> {
        // 获取提供商
        let provider = self.get_provider(&request.provider)
            .ok_or_else(|| ModelError::ProviderNotFound(request.provider.clone()))?;
        
        // 创建模型实例
        provider.create_model(&request.model_id, &request.parameters).await
    }
    
    /// 获取提供商配置
    pub fn get_provider_config(&self, name: &str) -> Option<&ProviderConfig> {
        self.config.get_provider(name)
    }
    
    /// 获取模型配置
    pub fn get_model_config(&self, provider: &str, model_id: &str) -> Option<&crate::model::config::ModelConfig> {
        self.config.get_model(provider, model_id)
    }
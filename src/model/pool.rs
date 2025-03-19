//! 模型工厂包装模块
//!
//! 本模块提供了模型工厂的简单包装。

use crate::model::error::ModelResult;
use crate::model::factory::{ModelFactory, ModelRequest};
use crate::model::Model;
use std::sync::Arc;

/// 模型池，直接使用模型工厂创建模型
pub struct ModelPool {
    /// 模型工厂
    factory: Arc<ModelFactory>,
}

impl ModelPool {
    /// 创建新的模型池
    pub fn new(factory: Arc<ModelFactory>, _config: ()) -> Self {
        Self { factory }
    }

    /// 创建模型实例
    pub async fn get_model(&self, request: &ModelRequest) -> ModelResult<Arc<Box<dyn Model>>> {
        // 直接创建新实例
        let model = self.factory.create_model(request).await?;
        let model_arc = Arc::new(model);

        Ok(model_arc)
    }
}

//! u6a21u578bu670du52a1u6a21u5757
//!
//! u672cu6a21u5757u63d0u4f9bu4e86u6a21u578bu670du52a1u7684u5b9eu73b0uff0cu4e3bu8981u5305u62ecuff1a
//!
//! - u6a21u578bu670du52a1uff1au5c01u88c5u6a21u578bu6c60u548cu6a21u578bu64cdu4f5cuff0cu63d0u4f9bu7edfu4e00u7684u63a5u53e3
//! - u6570u636eu5904u7406uff1au652fu6301u6279u91cfu548cu6d41u5f0fu6570u636eu5904u7406
//! - u6027u80fdu4f18u5316uff1au5229u7528u6a21u578bu6c60u51cfu5c11u521du59cbu5316u5f00u9500
//!
//! # u793au4f8b
//!
//! ```rust
//! use crate::model::service::ModelService;
//! use crate::model::pool::{ModelPool, ModelPoolConfig};
//! use crate::model::factory::{ModelFactory, ModelRequest};
//! use crate::model::{DataType, ModelParams};
//! use std::sync::Arc;
//!
//! // u521bu5efau6a21u578bu670du52a1
//! let factory = Arc::new(ModelFactory::new(providers_config));
//! let pool = Arc::new(ModelPool::new(factory, ModelPoolConfig::default()));
//! let service = ModelService::new(pool);
//!
//! // u521bu5efau6a21u578bu8bf7u6c42
//! let request = ModelRequest {
//!     provider: "openai".to_string(),
//!     model_id: "gpt-4".to_string(),
//!     parameters: ModelParams::default(),
//! };
//!
//! // u5904u7406u8bf7u6c42
//! let result = service.process(request).await?;
//! ```

use crate::model::error::{ModelError, ModelResult};
use crate::model::factory::ModelRequest;
use crate::model::pool::ModelPool;
use crate::model::{DataType, StreamDataType};
use crate::orchestration::types::MergeStrategy;
use std::sync::Arc;

/// u6a21u578bu670du52a1
///
/// u63d0u4f9bu7edfu4e00u7684u6a21u578bu8bbfu95eeu63a5u53e3uff0cu5c01u88c5u6a21u578bu6c60u548cu6a21u578bu64cdu4f5cu3002
pub struct ModelService {
    /// u6a21u578bu6c60
    pool: Arc<ModelPool>,
}

impl ModelService {
    /// u521bu5efau65b0u7684u6a21u578bu670du52a1
    ///
    /// # u53c2u6570
    ///
    /// * `pool` - u6a21u578bu6c60u5b9eu4f8b
    pub fn new(pool: Arc<ModelPool>) -> Self {
        Self { pool }
    }

    /// u5904u7406u6a21u578bu8bf7u6c42uff0cu8fd4u56deu6279u91cfu7ed3u679c
    ///
    /// # u53c2u6570
    ///
    /// * `request` - u6a21u578bu8bf7u6c42
    ///
    /// # u8fd4u56de
    ///
    /// u8fd4u56deu5904u7406u7ed3u679c
    pub async fn process(&self, request: ModelRequest) -> ModelResult<DataType> {
        let model = self.pool.get_model(&request).await?;
        model.process(request.parameters.clone()).await
    }

    /// u5904u7406u6a21u578bu8bf7u6c42uff0cu8fd4u56deu6d41u5f0fu7ed3u679c
    ///
    /// # u53c2u6570
    ///
    /// * `request` - u6a21u578bu8bf7u6c42
    ///
    /// # u8fd4u56de
    ///
    /// u8fd4u56deu6d41u5f0fu5904u7406u7ed3u679c
    pub async fn process_stream(&self, request: ModelRequest) -> ModelResult<StreamDataType> {
        let model = self.pool.get_model(&request).await?;
        model.process_stream(request.parameters.clone()).await
    }

    /// 并行处理多个模型请求，并根据合并策略选择结果
    ///
    /// # 参数
    ///
    /// * `requests` - 模型请求列表
    /// * `input` - 输入数据
    /// * `merge_strategy` - 结果合并策略
    ///
    /// # 返回值
    ///
    /// 返回处理结果
    pub async fn process_parallel(
        &self,
        requests: Vec<ModelRequest>,
        input: DataType,
        merge_strategy: MergeStrategy,
    ) -> ModelResult<DataType> {
        if requests.is_empty() {
            return Err(ModelError::InvalidRequest(
                "No model requests provided".to_string(),
            ));
        }

        // 如果只有一个请求，直接处理
        if requests.len() == 1 {
            return self.process(requests[0].clone()).await;
        }

        // 创建并行任务
        let mut handles = Vec::new();
        for request in requests {
            let service = self.clone();
            let input_clone = input.clone();
            let handle = tokio::spawn(async move { service.process(request).await });
            handles.push(handle);
        }

        // 等待所有任务完成
        let mut results = Vec::new();
        let mut errors = Vec::new();

        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => errors.push(e),
                Err(e) => errors.push(ModelError::InternalError(format!("Task join error: {}", e))),
            }
        }

        // 如果没有成功的结果，返回第一个错误
        if results.is_empty() {
            if let Some(error) = errors.into_iter().next() {
                return Err(error);
            } else {
                return Err(ModelError::InternalError(
                    "All parallel tasks failed with unknown errors".to_string(),
                ));
            }
        }

        // 根据合并策略选择结果
        match merge_strategy {
            MergeStrategy::First => Ok(results[0].clone()),
            MergeStrategy::Last => Ok(results[results.len() - 1].clone()),
            MergeStrategy::Longest => {
                // 选择最长的结果（适用于文本）
                let longest = results
                    .iter()
                    .max_by_key(|r| match r {
                        DataType::Text { content, .. } => content.len(),
                        DataType::Audio { data, .. } => data.len(),
                    })
                    .unwrap();
                Ok(longest.clone())
            },
            MergeStrategy::Shortest => {
                // 选择最短的结果（适用于文本）
                let shortest = results
                    .iter()
                    .min_by_key(|r| match r {
                        DataType::Text { content, .. } => content.len(),
                        DataType::Audio { data, .. } => data.len(),
                    })
                    .unwrap();
                Ok(shortest.clone())
            },
            MergeStrategy::Concat => {
                // 合并所有文本结果
                let mut combined_text = String::new();
                let mut is_text = false;

                for result in &results {
                    match result {
                        DataType::Text { content, mode } => {
                            combined_text.push_str(content);
                            combined_text.push_str("\n");
                            is_text = true;
                        },
                        _ => {},
                    }
                }

                if is_text {
                    Ok(DataType::Text {
                        content: combined_text,
                        mode: crate::model::StreamMode::NonStreaming,
                    })
                } else {
                    // 如果不是文本，返回第一个结果
                    Ok(results[0].clone())
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::factory::ModelFactory;
    use crate::model::pool::ModelPoolConfig;
    use std::time::Duration;

    // u6a21u62dfu6d4bu8bd5u914du7f6e
    fn create_test_service() -> ModelService {
        // u6ce8u610fuff1au8fd9u91ccu53eau662fu6d4bu8bd5u6846u67b6uff0cu5b9eu9645u6d4bu8bd5u9700u8981u6a21u62dfu6a21u578bu5de5u5382
        let config = crate::model::config::ProvidersConfig::default();
        let factory = Arc::new(ModelFactory::new(config));
        let pool_config = ModelPoolConfig {
            max_size: 5,
            max_idle_time: Duration::from_secs(60),
        };
        let pool = Arc::new(ModelPool::new(factory, pool_config));
        ModelService::new(pool)
    }

    // u6d4bu8bd5u7528u4f8bu5c06u5728u5b9eu9645u96c6u6210u65f6u6dfbu52a0
}

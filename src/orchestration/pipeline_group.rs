use crate::audio::format::AudioCodec;
use crate::audio::{AudioConverter, AudioFormat, AudioParams};
use crate::error::{ServiceError, ServiceResult};
use crate::model::Model;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// 模型组执行模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GroupExecutionMode {
    /// 并行执行模式 - 同时执行组内所有模型，然后根据合并策略选择最终结果
    Parallel,
}

/// 结果合并策略
///
/// 定义了如何合并多个模型的处理结果。
/// 在并行处理时，需要选择一个合适的策略来合并不同模型的输出。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// 使用第一个结果
    First,
    /// 使用最后一个结果
    Last,
    /// 使用最大长度的结果
    Longest,
    /// 使用最小长度的结果
    Shortest,
}

/// 模型组
///
/// 表示一组模型，专门用于并行执行。
/// 所有模型会同时处理相同的输入，然后根据合并策略选择一个结果作为输出。
/// 模型组始终以并行模式执行，这简化了整体架构。
pub struct ModelGroup {
    /// 组名称
    name: String,
    /// 组内模型列表
    models: Vec<Arc<dyn Model>>,
    /// 结果合并策略
    merge_strategy: MergeStrategy,
}

impl ModelGroup {
    /// 创建新的模型组
    ///
    /// # 参数
    ///
    /// * `name` - 组名称
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            models: Vec::new(),
            merge_strategy: MergeStrategy::First,
        }
    }

    /// 添加模型到组
    ///
    /// # 参数
    ///
    /// * `model` - 要添加的模型
    pub fn add_model(&mut self, model: Arc<dyn Model>) -> &mut Self {
        self.models.push(model);
        self
    }

    /// 设置结果合并策略
    ///
    /// # 参数
    ///
    /// * `strategy` - 结果合并策略
    pub fn set_merge_strategy(&mut self, strategy: MergeStrategy) -> &mut Self {
        self.merge_strategy = strategy;
        self
    }

    /// 获取组名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取组内模型数量
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// 获取组内第一个模型（如果存在）
    pub fn first_model(&self) -> Option<&Arc<dyn Model>> {
        self.models.first()
    }

    /// 获取组内最后一个模型（如果存在）
    pub fn last_model(&self) -> Option<&Arc<dyn Model>> {
        self.models.last()
    }

    /// 获取组内所有模型的引用
    pub fn models(&self) -> &[Arc<dyn Model>] {
        &self.models
    }

    /// 执行模型组处理
    ///
    /// # 参数
    ///
    /// * `input` - 输入数据
    /// * `format` - 输入数据的音频格式
    ///
    /// # 返回值
    ///
    /// 返回处理后的数据和其音频格式
    pub async fn execute(
        &self,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)> {
        if self.models.is_empty() {
            return Err(ServiceError::Pipeline(format!(
                "Model group '{}' has no models",
                self.name
            )));
        }

        self.execute_parallel(input, format).await
    }

    /// 并行执行组内模型
    async fn execute_parallel(
        &self,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)> {
        debug!("Executing {} models in parallel group {}", self.models.len(), self.name);

        // 如果只有一个模型，直接执行
        if self.models.len() == 1 {
            let model_start = Instant::now();
            let result = self.models[0].process(input, format).await;
            let model_duration = model_start.elapsed();
            debug!(
                "Single model {} in group {} processed in {}ms",
                self.models[0].model_id(),
                self.name,
                model_duration.as_millis()
            );
            return result;
        }

        let mut handles = Vec::new();

        // 启动所有模型的并行处理任务
        for model in &self.models {
            let model = model.clone();
            let input = input.clone();
            let format = format.clone();
            let group_name = self.name.clone();

            let handle = tokio::spawn(async move {
                let model_start = Instant::now();
                let model_id = model.model_id();
                let result = model.process(input, format).await;
                let model_duration = model_start.elapsed();
                
                match &result {
                    Ok(_) => debug!(
                        "Parallel model {} in group {} processed in {}ms",
                        model_id,
                        group_name,
                        model_duration.as_millis()
                    ),
                    Err(e) => warn!(
                        "Model {} in group {} failed after {}ms: {}",
                        model_id,
                        group_name,
                        model_duration.as_millis(),
                        e
                    ),
                }
                
                (model_id, result)
            });

            handles.push(handle);
        }

        // 收集所有结果
        let mut results = Vec::new();
        let mut failures = Vec::new();
        
        for handle in handles {
            match handle.await {
                Ok((model_id, Ok(result))) => results.push(result),
                Ok((model_id, Err(e))) => {
                    error!("Parallel model {} execution failed: {}", model_id, e);
                    failures.push((model_id, e));
                },
                Err(e) => error!("Parallel task execution failed: {}", e),
            }
        }

        if results.is_empty() {
            // 如果所有模型都失败，返回详细的错误信息
            let error_details = failures
                .iter()
                .map(|(id, err)| format!("{}: {}", id, err))
                .collect::<Vec<_>>()
                .join("; ");
                
            return Err(ServiceError::Pipeline(format!(
                "All models in group {} failed: {}",
                self.name,
                error_details
            )));
        }

        // 记录成功率
        let success_rate = results.len() as f32 / self.models.len() as f32;
        if success_rate < 1.0 {
            warn!(
                "Group {} completed with {:.1}% success rate ({}/{} models)",
                self.name,
                success_rate * 100.0,
                results.len(),
                self.models.len()
            );
        }

        // 根据合并策略选择结果
        let selected = match self.merge_strategy {
            MergeStrategy::First => results.into_iter().next().unwrap(),
            MergeStrategy::Last => results.into_iter().last().unwrap(),
            MergeStrategy::Longest => results
                .into_iter()
                .max_by_key(|(data, _)| data.len())
                .unwrap(),
            MergeStrategy::Shortest => results
                .into_iter()
                .min_by_key(|(data, _)| data.len())
                .unwrap(),
        };

        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataType;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::runtime::Runtime;

    // 测试用模型实现
    struct TestModel {
        id: String,
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Model for TestModel {
        async fn process(
            &self,
            input: Vec<u8>,
            format: Option<AudioFormat>,
        ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)> {
            // 增加计数器，模拟处理
            self.counter.fetch_add(1, Ordering::SeqCst);
            // 返回输入数据加上模型ID的字节
            let mut output = input;
            output.extend_from_slice(self.id.as_bytes());
            Ok((output, format))
        }

        fn input_type(&self) -> Vec<crate::model::DataType> {
            vec![DataType::Audio {
                format: AudioFormat {
                    codec: AudioCodec::Wav,
                    sample_rate: 16000,
                    bit_rate: None,
                    channels: 1,
                },
                data: Vec::new(),
            }]
        }

        fn output_type(&self) -> Vec<crate::model::DataType> {
            vec![DataType::Audio {
                format: AudioFormat {
                    codec: AudioCodec::Wav,
                    sample_rate: 16000,
                    bit_rate: None,
                    channels: 1,
                },
                data: Vec::new(),
            }]
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        fn model_id(&self) -> String {
            self.id.clone()
        }
    }
}
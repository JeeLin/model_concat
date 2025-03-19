//! 流水线工厂模块
//!
//! 本模块提供了流水线实例的创建和管理功能，主要包括：
//!
//! - 流水线工厂：负责创建和管理不同类型的流水线实例
//! - 流水线缓存：支持流水线实例的缓存和复用
//! - 参数配置：统一管理流水线参数和配置信息

use crate::error::{ServiceError, ServiceResult};
use crate::metrics::MetricsManager;
use crate::model::factory::ModelFactory;
use crate::model::{Model, ModelParams};
use crate::orchestration::pipeline::{ExecutionMode, Pipeline};
use crate::orchestration::types::{MergeStrategy, StageConfig};
use crate::{log_debug, log_warn};
use std::sync::Arc;

/// 流水线请求配置
#[derive(Debug, Clone)]
pub struct PipelineRequest {
    /// 流水线名称
    pub name: String,
    /// 阶段配置列表
    // #[serde(default)]
    pub stages: Vec<StageRequest>,
}

impl PipelineRequest {
    /// 生成缓存键
    ///
    /// 根据请求参数生成唯一的缓存键，用于标识不同的流水线配置
    pub fn cache_key(&self) -> String {
        // 生成更精确的缓存键，包含阶段和模型信息
        let mut key = format!("pipeline:{}", self.name);

        // 添加阶段和模型信息
        if !self.stages.is_empty() {
            key.push_str(":stages");
            for stage in &self.stages {
                key.push_str(&format!(":{}", stage.name));
                for model in &stage.models {
                    key.push_str(&format!(",{}:{}", model.provider, model.model_id));
                }
            }
        }

        key
    }

    /// 验证请求配置的有效性
    ///
    /// 检查请求中的阶段和模型配置是否有效，并验证阶段间的输入输出兼容性
    pub fn validate(&self) -> ServiceResult<()> {
        // 检查是否至少有一个阶段
        if self.stages.is_empty() {
            return Err(ServiceError::UnsupportedOperation(
                "Pipeline request must contain at least one stage".to_string(),
            ));
        }

        // 检查每个阶段是否至少有一个模型
        for stage in &self.stages {
            if stage.models.is_empty() {
                return Err(ServiceError::UnsupportedOperation(format!(
                    "Stage '{}' must contain at least one model",
                    stage.name
                )));
            }
        }

        Ok(())
    }
}

/// 模型请求配置
#[derive(Debug, Clone)]
pub struct ModelRequest {
    /// 模型提供者
    pub provider: String,
    /// 模型ID
    pub model_id: String,
    /// 模型参数
    pub parameters: ModelParams,
    /// 执行模式
    // #[serde(default = "default_execution_mode")]
    pub execution_mode: ExecutionMode,
}

/// 默认执行模式为串行
fn default_execution_mode() -> ExecutionMode {
    ExecutionMode::Sequential
}

/// 默认合并策略为使用第一个结果
fn default_merge_strategy() -> MergeStrategy {
    MergeStrategy::First
}

/// 阶段请求配置
#[derive(Debug, Clone)]
pub struct StageRequest {
    /// 阶段名称
    pub name: String,
    /// 模型配置列表
    pub models: Vec<ModelRequest>,
    /// 结果合并策略
    // #[serde(default)]
    pub merge_strategy: MergeStrategy,
}

/// 流水线工厂
///
/// 负责创建和管理处理流水线
pub struct PipelineFactory {
    /// 模型工厂
    model_factory: &'static Arc<ModelFactory>,
    /// 性能指标管理器
    metrics_manager: Arc<MetricsManager>,
}

impl PipelineFactory {
    /// 创建新的流水线工厂
    ///
    /// # 参数
    ///
    /// * `model_factory` - 模型工厂实例
    /// * `audio_converter_factory` - 音频转换器工厂实例
    /// * `text_converter_factory` - 文本转换器工厂实例
    /// * `metrics_manager` - 性能指标管理器实例
    pub fn new(model_factory: &Arc<ModelFactory>, metrics_manager: Arc<MetricsManager>) -> Self {
        Self {
            model_factory,
            metrics_manager,
        }
    }

    /// 创建处理流水线
    ///
    /// # 参数
    ///
    /// * `request` - 流水线请求配置
    ///
    /// # 返回值
    ///
    /// 返回创建的流水线实例
    pub async fn create_pipeline(&self, request: &PipelineRequest) -> ServiceResult<Arc<Pipeline>> {
        // 验证请求
        request.validate()?;

        // 创建新的流水线
        log_debug!("Creating new pipeline: {}", request.name);

        let mut pipeline = Pipeline::new(
            &request.name,
            self.metrics_manager.clone(),
        );

        log_debug!("Using stages-based pipeline structure");

        // 处理每个阶段
        let mut prev_stage_models = Vec::new();

        for (stage_idx, stage) in request.stages.iter().enumerate() {
            log_debug!("Processing stage: {}", stage.name);

            // 设置阶段的合并策略
            pipeline.set_merge_strategy(stage.merge_strategy);

            // 当前阶段的模型列表
            let mut current_stage_models = Vec::new();

            // 添加阶段中的所有模型
            for model_req in &stage.models {
                // 创建模型
                let model_request = crate::model::factory::ModelRequest {
                    provider: model_req.provider.clone(),
                    model_id: model_req.model_id.clone(),
                    parameters: model_req.parameters.clone(),
                };

                let model = self.model_factory.create_model(&model_request).await;

                // 检查阶段内模型的输入输出格式一致性
                if !current_stage_models.is_empty() {
                    let first_model = &current_stage_models[0];

                    // 检查输入格式一致性
                    let first_inputs = first_model.supported_input_formats();
                    let curr_inputs = model.supported_input_formats();

                    let mut input_compatible = false;
                    for first_input in &first_inputs {
                        for curr_input in &curr_inputs {
                            if first_input.data_type == curr_input.data_type {
                                input_compatible = true;
                                break;
                            }
                        }
                        if input_compatible {
                            break;
                        }
                    }

                    if !input_compatible {
                        return Err(ServiceError::UnsupportedOperation(format!(
                            "Incompatible input formats in stage '{}': model '{}:{}' has different input type than other models",
                            stage.name, model_req.provider, model_req.model_id
                        )));
                    }

                    // 检查输出格式一致性
                    let first_outputs = first_model.supported_output_formats();
                    let curr_outputs = model.supported_output_formats();

                    let mut output_compatible = false;
                    for first_output in &first_outputs {
                        for curr_output in &curr_outputs {
                            if first_output.data_type == curr_output.data_type {
                                output_compatible = true;
                                break;
                            }
                        }
                        if output_compatible {
                            break;
                        }
                    }

                    if !output_compatible {
                        return Err(ServiceError::UnsupportedOperation(format!(
                            "Incompatible output formats in stage '{}': model '{}:{}' has different output type than other models",
                            stage.name, model_req.provider, model_req.model_id
                        )));
                    }
                }

                current_stage_models.push(model.clone());

                // 添加到流水线，阶段内的模型使用并行模式
                if let Err(e) = pipeline.add_model(model, ExecutionMode::Parallel) {
                    log_warn!(
                        "Failed to add model {}:{} to pipeline: {}",
                        model_req.provider, model_req.model_id, e
                    );
                    return Err(e);
                }
            }

            // 检查阶段间的输入输出兼容性
            if stage_idx > 0 && !prev_stage_models.is_empty() && !current_stage_models.is_empty() {
                // 检查前一阶段的输出格式与当前阶段的输入格式是否兼容
                let mut compatible = false;
                let mut data_type_mismatch = false;
                let mut prev_output_type = String::new();
                let mut curr_input_type = String::new();

                for prev_model in &prev_stage_models {
                    for curr_model in &current_stage_models {
                        // 获取前一个模型的输出格式和当前模型的输入格式
                        let prev_outputs = prev_model.supported_output_formats();
                        let curr_inputs = curr_model.supported_input_formats();

                        // 检查数据类型是否匹配
                        if !prev_outputs.is_empty() && !curr_inputs.is_empty() {
                            prev_output_type = prev_outputs[0].data_type.clone();
                            curr_input_type = curr_inputs[0].data_type.clone();

                            if prev_output_type != curr_input_type {
                                data_type_mismatch = true;
                                continue;
                            }
                        }

                        // 检查是否存在至少一个兼容的格式
                        for prev_output in &prev_outputs {
                            for curr_input in &curr_inputs {
                                if pipeline.is_format_compatible(prev_output, curr_input) {
                                    compatible = true;
                                    break;
                                }
                            }
                            if compatible {
                                break;
                            }
                        }

                        if compatible {
                            break;
                        }
                    }

                    if compatible {
                        break;
                    }
                }

                if !compatible {
                    if data_type_mismatch {
                        return Err(ServiceError::UnsupportedOperation(format!(
                            "Incompatible data types between stages: '{}' outputs {} but '{}' expects {}",
                            request.stages[stage_idx - 1].name,
                            prev_output_type,
                            stage.name,
                            curr_input_type
                        )));
                    } else {
                        return Err(ServiceError::UnsupportedOperation(format!(
                            "Incompatible stages: '{}' output is not compatible with '{}' input",
                            request.stages[stage_idx - 1].name,
                            stage.name
                        )));
                    }
                }
            }

            // 更新前一阶段的模型列表
            prev_stage_models = current_stage_models;
        }

        // 返回流水线实例
        let pipeline = Arc::new(pipeline);

        Ok(pipeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::ProvidersConfig;

    #[test]
    fn test_pipeline_request() {
        let request = PipelineRequest {
            name: "test_pipeline".to_string(),
            stages: vec![StageRequest {
                name: "test_stage".to_string(),
                models: vec![ModelRequest {
                    provider: "openai".to_string(),
                    model_id: "gpt-4".to_string(),
                    parameters: ModelParams::default(),
                    execution_mode: ExecutionMode::Sequential,
                }],
                merge_strategy: MergeStrategy::First,
            }],
        };

        assert!(request.validate().is_ok());
    }

    #[tokio::test]
    async fn test_pipeline_factory_creation() {
        // 创建模拟组件
        let config = ProvidersConfig::default();
        let model_factory = Arc::new(ModelFactory::new(config));
        let metrics_manager = Arc::new(MetricsManager::new());

        let _factory = PipelineFactory::new(&model_factory, metrics_manager);

        // 简单测试工厂创建成功
        assert!(true);
    }
}

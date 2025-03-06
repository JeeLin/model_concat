use crate::audio::converter::FormatConverterFactory;
use crate::audio::format::AudioCodec;
use crate::audio::{AudioConverter, AudioFormat, AudioParams};
use crate::error::{ServiceError, ServiceResult};
use crate::metrics::MetricsManager;
use crate::model::Model;
use crate::model::DataType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn, error};

/// 模型执行模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// 串行执行模式 - 按顺序执行所有模型，前一个模型的输出作为后一个模型的输入
    Sequential,
    /// 并行执行模式 - 同时执行所有模型，然后根据合并策略选择最终结果
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

/// 处理流水线
///
/// 负责管理和执行音频处理的模型列表，支持串行和并行执行模式，以及格式转换和性能监控。
/// 整体流程是串行的，但支持在特定位置插入并行执行的模型组。
///
/// 执行逻辑：
/// 1. 整体流水线按顺序执行模型
/// 2. 对于标记为Sequential的模型，直接串行执行
/// 3. 对于标记为Parallel的模型，会查找连续的并行模型组，并将它们作为一组并行执行
/// 4. 并行模型组的结果根据合并策略选择一个作为输出
///
/// # 示例
///
/// ```rust
/// use crate::orchestration::Pipeline;
///
/// let pipeline = Pipeline::new(
///     audio_converter,
///     format_converter_factory,
///     metrics_manager
/// );
///
/// // 添加串行执行的模型
/// pipeline.add_model(model1, ExecutionMode::Sequential)?;
/// // 添加并行执行的模型组
/// pipeline.add_model(model2, ExecutionMode::Parallel)?;
/// pipeline.add_model(model3, ExecutionMode::Parallel)?;
/// // 添加串行执行的模型
/// pipeline.add_model(model4, ExecutionMode::Sequential)?;
/// 
/// let result = pipeline.execute(input_data).await?;
/// ```
pub struct Pipeline {
    /// 模型列表
    models: Vec<(Arc<dyn Model>, ExecutionMode)>,
    /// 音频转换器
    audio_converter: Arc<AudioConverter>,
    /// 格式转换工厂
    format_converter_factory: Arc<FormatConverterFactory>,
    /// 性能指标管理器
    metrics_manager: Arc<MetricsManager>,
    /// 模型间格式转换信息（预计算）
    format_conversion_map: HashMap<usize, Option<AudioFormat>>,
    /// 结果合并策略
    merge_strategy: MergeStrategy,
}

impl Pipeline {
    /// 创建新的处理流水线
    ///
    /// # 参数
    ///
    /// * `audio_converter` - 音频转换器实例
    /// * `format_converter_factory` - 格式转换工厂实例
    /// * `metrics_manager` - 性能指标管理器实例
    pub fn new(
        audio_converter: Arc<AudioConverter>,
        format_converter_factory: Arc<FormatConverterFactory>,
        metrics_manager: Arc<MetricsManager>,
    ) -> Self {
        Self {
            models: Vec::new(),
            audio_converter,
            format_converter_factory,
            metrics_manager,
            format_conversion_map: HashMap::new(),
            merge_strategy: MergeStrategy::First,
        }
    }

    /// 添加模型
    ///
    /// # 参数
    ///
    /// * `model` - 要添加的模型
    /// * `mode` - 模型执行模式
    ///
    /// # 返回值
    ///
    /// 返回流水线自身的可变引用，支持链式调用
    pub fn add_model(
        &mut self,
        model: Arc<dyn Model>,
        mode: ExecutionMode,
    ) -> ServiceResult<&mut Self> {
        // 如果已有其他模型，检查与前一个模型的格式兼容性
        if let Some((prev_model, _)) = self.models.last() {
            self.validate_model_compatibility(prev_model, &model)?;

            // 预计算格式转换信息
            let prev_model_index = self.models.len() - 1;
            self.precompute_format_conversion(prev_model_index, &model);
        }

        self.models.push((model, mode));
        Ok(self)
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

    /// 预计算格式转换信息
    ///
    /// 分析相邻模型之间的音频格式兼容性，并计算必要的格式转换
    fn precompute_format_conversion(&mut self, prev_model_index: usize, next_model: &Arc<dyn Model>) {
        let (prev_model, _) = &self.models[prev_model_index];

        // 获取前一个模型的所有可能输出格式
        let mut possible_output_formats = Vec::new();
        for output_type in prev_model.output_type() {
            if let DataType::Audio { format, .. } = output_type {
                possible_output_formats.push(format);
            }
        }

        // 对于每个可能的输出格式，检查是否需要转换
        for format in &possible_output_formats {
            let mut needs_conversion = true;

            // 检查下一个模型是否支持当前格式
            if next_model.supports_input_format(format) {
                needs_conversion = false;
            }

            // 如果需要转换，找到合适的目标格式
            if needs_conversion {
                let supported_formats = next_model.supported_input_formats();
                if !supported_formats.is_empty() {
                    // 选择第一个支持的格式作为转换目标
                    self.format_conversion_map
                        .insert(prev_model_index, Some(supported_formats[0].clone()));
                    return;
                }
            }
        }

        // 如果不需要转换，或者没有找到合适的目标格式
        self.format_conversion_map.insert(prev_model_index, None);
    }

    /// 验证两个模型之间的格式兼容性
    ///
    /// # 参数
    ///
    /// * `prev_model` - 前一个模型
    /// * `next_model` - 下一个模型
    fn validate_model_compatibility(
        &self,
        prev_model: &Arc<dyn Model>,
        next_model: &Arc<dyn Model>,
    ) -> ServiceResult<()> {
        // 获取前一个模型的所有输出格式
        let mut prev_outputs = Vec::new();
        for output_type in prev_model.output_type() {
            if let DataType::Audio { format, .. } = output_type {
                prev_outputs.push(format);
            }
        }

        // 获取下一个模型支持的所有输入格式
        let next_inputs = next_model.supported_input_formats();

        // 检查是否存在至少一个兼容的格式
        let mut compatible = false;
        for output in &prev_outputs {
            for input in &next_inputs {
                if output.is_compatible_with(input) {
                    compatible = true;
                    break;
                }
            }
            if compatible {
                break;
            }
        }

        if !compatible {
            return Err(ServiceError::Pipeline(format!(
                "Incompatible models: {} -> {}",
                prev_model.model_id(),
                next_model.model_id()
            )));
        }

        Ok(())
    }

    /// 执行流水线处理
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
            return Err(ServiceError::Pipeline("Pipeline has no models".to_string()));
        }

        let mut current_data = input;
        let mut current_format = format;

        // 记录开始时间
        let pipeline_start = Instant::now();

        // 处理模型列表 - 整体流程是串行的
        let mut i = 0;
        while i < self.models.len() {
            let (model, mode) = &self.models[i];
            
            debug!(
                "Processing model {}/{}: {}",
                i + 1,
                self.models.len(),
                model.model_id()
            );
            
            // 根据执行模式处理当前模型
            let (output, output_format) = match mode {
                ExecutionMode::Sequential => {
                    // 串行处理单个模型
                    let model_start = Instant::now();
                    let result = model.process(current_data.clone(), current_format.clone()).await?;
                    let model_duration = model_start.elapsed();
                    debug!("Model processed in {}ms", model_duration.as_millis());
                    result
                }
                ExecutionMode::Parallel => {
                    // 查找连续的并行模型组
                    let mut parallel_models = Vec::new();
                    parallel_models.push(model.clone());
                    
                    let mut j = i + 1;
                    while j < self.models.len() {
                        if let (next_model, ExecutionMode::Parallel) = &self.models[j] {
                            parallel_models.push(next_model.clone());
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    
                    // 并行执行这些模型组
                    let result = self.execute_parallel(
                        parallel_models,
                        current_data.clone(),
                        current_format.clone(),
                    ).await?;
                    
                    // 更新索引，跳过已处理的并行模型组
                    i = j - 1;
                    result
                }
            };

            // 检查是否需要格式转换
            if i < self.models.len() - 1 {
                if let Some(target_format) =
                    self.format_conversion_map.get(&i).and_then(|f| f.as_ref())
                {
                    debug!(
                        "Converting format from {:?} to {:?} after model {}",
                        output_format, target_format, model.model_id()
                    );
                    let converted = self
                        .audio_converter
                        .convert_format(
                            &output,
                            &output_format.unwrap_or_default(),
                            &AudioParams::from(target_format.clone()),
                        )
                        .await?;
                    current_data = converted.0;
                    current_format = Some(converted.1);
                } else {
                    current_data = output;
                    current_format = output_format;
                }
            } else {
                // 最后一个模型的输出直接作为结果
                current_data = output;
                current_format = output_format;
            }
            
            i += 1;
        }

        // 记录总执行时间
        let pipeline_duration = pipeline_start.elapsed();
        info!(
            "Pipeline execution completed in {}ms",
            pipeline_duration.as_millis()
        );

        Ok((current_data, current_format))
    }
    
    /// 并行执行多个模型
    ///
    /// # 参数
    ///
    /// * `models` - 要并行执行的模型列表（连续的并行模型组）
    /// * `input` - 输入数据
    /// * `format` - 输入数据的音频格式
    ///
    /// # 返回值
    ///
    /// 返回处理后的数据和其音频格式
    async fn execute_parallel(
        &self,
        models: Vec<Arc<dyn Model>>,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)> {
        debug!(
            "Executing {} models in parallel group",
            models.len(),
        );

        // 如果只有一个模型，直接执行
        if models.len() == 1 {
            return models[0].process(input, format).await;
        }

        let mut handles = Vec::new();

        // 启动所有模型的并行处理任务
        for model in &models {
            let model = model.clone();
            let input = input.clone();
            let format = format.clone();

            let handle = tokio::spawn(async move { 
                let model_start = Instant::now();
                let result = model.process(input, format).await;
                let model_duration = model_start.elapsed();
                debug!("Parallel model {} processed in {}ms", model.model_id(), model_duration.as_millis());
                result
            });

            handles.push(handle);
        }

        // 收集所有结果
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => error!("Parallel model execution failed: {}", e),
                Err(e) => error!("Parallel task execution failed: {}", e),
            }
        }

        if results.is_empty() {
            return Err(ServiceError::Pipeline(
                "No successful results from parallel execution group".to_string()
            ));
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
    use tokio::runtime::Runtime;

    #[test]
    fn test_pipeline_creation() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let audio_converter = Arc::new(AudioConverter::new(
                AudioFormat::default(),
                AudioFormat::default(),
            ));
            let format_converter_factory = Arc::new(FormatConverterFactory::new());
            let metrics_manager = Arc::new(MetricsManager::new());

            let pipeline =
                Pipeline::new(audio_converter, format_converter_factory, metrics_manager);

            assert!(pipeline.models.is_empty());
            assert!(pipeline.format_conversion_map.is_empty());
        });
    }

    #[test]
    fn test_empty_pipeline_execution() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let audio_converter = Arc::new(AudioConverter::new(
                AudioFormat::default(),
                AudioFormat::default(),
            ));
            let format_converter_factory = Arc::new(FormatConverterFactory::new());
            let metrics_manager = Arc::new(MetricsManager::new());

            let pipeline =
                Pipeline::new(audio_converter, format_converter_factory, metrics_manager);

            let input = vec![0u8; 1024];
            let result = pipeline.execute(input, None).await;
            assert!(result.is_err());
        });
    }
}
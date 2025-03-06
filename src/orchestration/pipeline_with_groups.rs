use crate::audio::converter::FormatConverterFactory;
use crate::audio::{AudioConverter, AudioFormat, AudioParams};
use crate::error::{ServiceError, ServiceResult};
use crate::metrics::MetricsManager;
use crate::model::Model;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn, error};

use super::pipeline_group::{ModelGroup, MergeStrategy};

/// 基于模型组的处理流水线
///
/// 负责管理和执行音频处理的模型组列表，支持更灵活的并行执行模式，以及格式转换和性能监控。
/// 整体流程是串行的，但每个模型组内部是并行执行的。
///
/// 执行逻辑：
/// 1. 整体流水线按顺序执行模型组
/// 2. 每个模型组内部始终以并行模式执行
/// 3. 并行执行的模型组会根据合并策略选择一个结果作为输出
///
/// # 示例
///
/// ```rust
/// use crate::orchestration::PipelineWithGroups;
///
/// let mut pipeline = PipelineWithGroups::new(
///     audio_converter,
///     format_converter_factory,
///     metrics_manager
/// );
///
/// // 创建并添加并行执行的模型组
/// let mut group1 = ModelGroup::new("group1");
/// group1.add_model(model1).add_model(model2);
/// pipeline.add_group(group1)?;
///
/// // 创建并添加另一个并行执行的模型组
/// let mut group2 = ModelGroup::new("group2");
/// group2.add_model(model3).add_model(model4);
/// pipeline.add_group(group2)?;
/// 
/// let result = pipeline.execute(input_data).await?;
/// ```
pub struct PipelineWithGroups {
    /// 模型组列表
    groups: Vec<ModelGroup>,
    /// 音频转换器
    audio_converter: Arc<AudioConverter>,
    /// 格式转换工厂
    format_converter_factory: Arc<FormatConverterFactory>,
    /// 性能指标管理器
    metrics_manager: Arc<MetricsManager>,
    /// 模型组间格式转换信息（预计算）
    format_conversion_map: HashMap<usize, Option<AudioFormat>>,
}

impl PipelineWithGroups {
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
            groups: Vec::new(),
            audio_converter,
            format_converter_factory,
            metrics_manager,
            format_conversion_map: HashMap::new(),
        }
    }

    /// 添加模型组
    ///
    /// # 参数
    ///
    /// * `group` - 要添加的模型组
    ///
    /// # 返回值
    ///
    /// 返回流水线自身的可变引用，支持链式调用
    pub fn add_group(&mut self, group: ModelGroup) -> ServiceResult<&mut Self> {
        // 如果已有其他模型组，检查与前一个模型组的格式兼容性
        if let Some(prev_group) = self.groups.last() {
            if let (Some(prev_model), Some(next_model)) = (prev_group.last_model(), group.first_model()) {
                self.validate_model_compatibility(prev_model, next_model)?;

                // 预计算格式转换信息
                let prev_group_index = self.groups.len() - 1;
                self.precompute_format_conversion(prev_group_index, next_model);
            }
        }

        self.groups.push(group);
        Ok(self)
    }

    /// 创建并添加新的模型组
    ///
    /// # 参数
    ///
    /// * `name` - 组名称
    ///
    /// # 返回值
    ///
    /// 返回新创建的模型组的可变引用
    pub fn create_group(&mut self, name: &str) -> ModelGroup {
        ModelGroup::new(name)
    }

    /// 预计算格式转换信息
    ///
    /// 分析相邻模型组之间的音频格式兼容性，并计算必要的格式转换
    fn precompute_format_conversion(&mut self, prev_group_index: usize, next_model: &Arc<dyn Model>) {
        if let Some(prev_model) = self.groups[prev_group_index].last_model() {
            // 获取前一个模型的所有可能输出格式
            let mut possible_output_formats = Vec::new();
            for output_type in prev_model.output_type() {
                if let crate::model::DataType::Audio { format, .. } = output_type {
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
                            .insert(prev_group_index, Some(supported_formats[0].clone()));
                        return;
                    }
                }
            }

            // 如果不需要转换，或者没有找到合适的目标格式
            self.format_conversion_map.insert(prev_group_index, None);
        }
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
            if let crate::model::DataType::Audio { format, .. } = output_type {
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
        if self.groups.is_empty() {
            return Err(ServiceError::Pipeline("Pipeline has no model groups".to_string()));
        }

        let mut current_data = input;
        let mut current_format = format;

        // 记录开始时间
        let pipeline_start = Instant::now();

        // 依次执行每个模型组
        for (i, group) in self.groups.iter().enumerate() {
            debug!(
                "Processing model group {}/{}: {}",
                i + 1,
                self.groups.len(),
                group.name()
            );

            // 执行当前模型组
            let group_start = Instant::now();
            let (output, output_format) = group.execute(current_data, current_format).await?;
            let group_duration = group_start.elapsed();

            debug!(
                "Model group {} processed in {}ms",
                group.name(),
                group_duration.as_millis()
            );

            // 检查是否需要格式转换
            if i < self.groups.len() - 1 {
                if let Some(target_format) =
                    self.format_conversion_map.get(&i).and_then(|f| f.as_ref())
                {
                    debug!(
                        "Converting format after group {}",
                        group.name()
                    );
                    let converted = self
                        .audio_converter
                        .convert_format(
                            &output,
                            &output_format.clone().unwrap_or_default(),
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
                // 最后一个模型组的输出直接作为结果
                current_data = output;
                current_format = output_format;
            }
        }

        // 记录总执行时间
        let pipeline_duration = pipeline_start.elapsed();
        info!(
            "Pipeline execution completed in {}ms",
            pipeline_duration.as_millis()
        );

        Ok((current_data, current_format))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataType;
    use crate::audio::format::AudioCodec;
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

    #[test]
    fn test_pipeline_with_groups() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // 创建计数器
            let counter = Arc::new(AtomicUsize::new(0));

            // 创建测试模型
            let model1 = Arc::new(TestModel {
                id: "model1".to_string(),
                counter: counter.clone(),
            });
            let model2 = Arc::new(TestModel {
                id: "model2".to_string(),
                counter: counter.clone(),
            });
            let model3 = Arc::new(TestModel {
                id: "model3".to_string(),
                counter: counter.clone(),
            });
            let model4 = Arc::new(TestModel {
                id: "model4".to_string(),
                counter: counter.clone(),
            });

            // 创建音频转换器和其他依赖
            let audio_converter = Arc::new(AudioConverter::new(
                AudioFormat::default(),
                AudioFormat::default(),
            ));
            let format_converter_factory = Arc::new(FormatConverterFactory::new());
            let metrics_manager = Arc::new(MetricsManager::new());

            // 创建流水线
            let mut pipeline = PipelineWithGroups::new(
                audio_converter,
                format_converter_factory,
                metrics_manager,
            );

            // 创建并添加模型组
            let mut group1 = ModelGroup::new("group1");
            group1.add_model(model1);
            pipeline.add_group(group1).unwrap();

            // 创建并添加并行执行的模型组
            let mut group2 = ModelGroup::new("group2");
            group2.add_model(model2).add_model(model3);
            pipeline.add_group(group2).unwrap();

            // 创建并添加另一个模型组
            let mut group3 = ModelGroup::new("group3");
            group3.add_model(model4);
            pipeline.add_group(group3).unwrap();

            // 执行流水线
            let input = vec![1, 2, 3];
            let result = pipeline.execute(input, None).await.unwrap();

            // 验证结果
            assert_eq!(counter.load(Ordering::SeqCst), 4); // 所有模型都应该被执行
            
            // 验证输出包含所有模型的ID
            // group1(model1) -> group2(model2或model3，取决于合并策略) -> group3(model4)
            let output = result.0;
            assert!(output.len() > 3); // 原始输入加上至少一些模型ID
        });
    }
}
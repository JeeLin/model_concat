//! 处理流水线
//!
//! 本模块实现了模型处理流水线，支持串行和并行执行模式，以及格式转换和性能监控。

use crate::audio::converter::AudioConverter;
use crate::error::{ServiceError, ServiceResult};
use crate::metrics::MetricsManager;
use crate::model::{DataType, Model};
use crate::orchestration::types::{
    DataFormat, FormatConverter, MergeStrategy, ModelConfig, StageConfig,
};
use crate::text::converter::TextConverter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// 模型执行模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// 串行执行模式 - 按顺序执行所有模型，前一个模型的输出作为后一个模型的输入
    Sequential,
    /// 并行执行模式 - 同时执行所有模型，然后根据合并策略选择最终结果
    Parallel,
}

// 使用从types.rs导入的MergeStrategy

/// 处理流水线
///
/// 负责管理和执行模型处理流水线，支持串行和并行执行模式，以及格式转换和性能监控。
pub struct Pipeline {
    /// 流水线名称
    name: String,
    /// 模型列表
    models: Vec<(Arc<dyn Model>, ExecutionMode)>,
    /// 音频转换器
    audio_converter: Arc<AudioConverter>,
    /// 文本转换器
    text_converter: Arc<TextConverter>,
    /// 格式转换器映射
    format_converters: HashMap<String, Arc<dyn FormatConverter>>,
    /// 性能指标管理器
    metrics_manager: Arc<MetricsManager>,
    /// 模型间格式转换信息（预计算）
    format_conversion_map: HashMap<usize, Option<DataFormat>>,
    /// 结果合并策略
    merge_strategy: MergeStrategy,
}

impl Pipeline {
    /// 创建新的处理流水线
    ///
    /// # 参数
    ///
    /// * `name` - 流水线名称
    /// * `metrics_manager` - 性能指标管理器实例
    pub fn new(
        name: &str,
        metrics_manager: Arc<MetricsManager>,
    ) -> Self {
        Self {
            name: name.to_string(),
            models: Vec::new(),
            audio_converter,
            text_converter,
            format_converters: HashMap::new(),
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

    /// 注册格式转换器
    ///
    /// # 参数
    ///
    /// * `name` - 转换器名称
    /// * `converter` - 转换器实例
    pub fn register_converter(
        &mut self,
        name: &str,
        converter: Arc<dyn FormatConverter>,
    ) -> &mut Self {
        self.format_converters.insert(name.to_string(), converter);
        self
    }

    /// 执行流水线处理
    ///
    /// # 参数
    ///
    /// * `input` - 输入数据
    /// * `format` - 输入数据的格式（可选）
    ///
    /// # 返回值
    ///
    /// 返回处理后的数据和其格式
    pub async fn execute(
        &self,
        input: Vec<u8>,
        format: Option<DataFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<DataFormat>)> {
        if self.models.is_empty() {
            return Err(ServiceError::UnsupportedOperation(
                "Pipeline has no models".to_string(),
            ));
        }

        let pipeline_start = Instant::now();
        info!("Starting pipeline execution: {}", self.name);

        // 初始化处理状态
        let mut current_data = input;
        let mut current_format = format;

        // 跟踪当前的并行模型组
        let mut parallel_group = Vec::new();
        let mut last_mode = ExecutionMode::Sequential;

        // 遍历所有模型
        for (i, (model, mode)) in self.models.iter().enumerate() {
            // 检查是否需要执行并行组
            if *mode == ExecutionMode::Sequential
                && last_mode == ExecutionMode::Parallel
                && !parallel_group.is_empty()
            {
                // 执行并行组
                let (data, fmt) = self
                    .execute_parallel_group(&parallel_group, current_data, current_format.clone())
                    .await?;

                current_data = data;
                current_format = fmt;
                parallel_group.clear();
            }

            // 更新执行模式
            last_mode = *mode;

            // 如果是并行模式，添加到并行组
            if *mode == ExecutionMode::Parallel {
                parallel_group.push(model.clone());
                continue;
            }

            // 串行执行当前模型
            let model_start = Instant::now();
            debug!("Executing model: {}", model.model_id());

            // 检查是否需要格式转换
            if i > 0 && self.format_conversion_map.contains_key(&(i - 1)) {
                if let Some(target_format) = &self.format_conversion_map[&(i - 1)] {
                    debug!("Converting format for model: {}", model.model_id());

                    // 根据数据类型选择合适的转换器
                    match target_format {
                        DataFormat::Audio(format) => {
                            // 使用音频转换器
                            if let Some(current_audio_format) =
                                current_format.as_ref().and_then(|f| f.as_audio())
                            {
                                let params = crate::audio::format::AudioParams::new()
                                    .with_codec(format.codec.clone())
                                    .with_sample_rate(format.sample_rate)
                                    .with_channels(format.channels);

                                current_data = self
                                    .audio_converter
                                    .convert_with_params(
                                        &current_data,
                                        current_audio_format,
                                        &params,
                                    )
                                    .await?;

                                current_format = Some(DataFormat::Audio(format.clone()));
                            }
                        },
                        DataFormat::Text(format) => {
                            // 使用文本转换器
                            if let Some(current_text_format) =
                                current_format.as_ref().and_then(|f| f.as_text())
                            {
                                let params = crate::text::converter::TextParams {
                                    format_type: Some(format.format_type.clone()),
                                    encoding: format.encoding.clone(),
                                };

                                let text = String::from_utf8_lossy(&current_data).to_string();
                                let converted = self
                                    .text_converter
                                    .convert_with_params(&text, current_text_format, &params)
                                    .await?;

                                current_data = converted.into_bytes();
                                current_format = Some(DataFormat::Text(format.clone()));
                            }
                        },
                    }
                }
            }

            // 执行模型处理
            let input_data = self.create_model_input(model, &current_data, &current_format)?;
            let output = model.process(input_data).await?;

            // 提取模型输出
            let (data, format) = self.extract_model_output(output)?;
            current_data = data;
            current_format = format;

            let model_duration = model_start.elapsed();
            debug!(
                "Model {} processed in {}ms",
                model.model_id(),
                model_duration.as_millis()
            );

            // 记录性能指标
            self.metrics_manager.record_model_execution(
                &model.model_id(),
                model_duration,
                current_data.len(),
            );
        }

        // 处理最后的并行组
        if !parallel_group.is_empty() {
            let (data, fmt) = self
                .execute_parallel_group(&parallel_group, current_data, current_format)
                .await?;

            current_data = data;
            current_format = fmt;
        }

        let pipeline_duration = pipeline_start.elapsed();
        info!(
            "Pipeline execution completed in {}ms",
            pipeline_duration.as_millis()
        );

        Ok((current_data, current_format))
    }

    /// 执行并行模型组
    async fn execute_parallel_group(
        &self,
        models: &[Arc<dyn Model>],
        input: Vec<u8>,
        format: Option<DataFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<DataFormat>)> {
        debug!("Executing {} models in parallel", models.len());

        // 如果只有一个模型，直接执行
        if models.len() == 1 {
            let model = &models[0];
            let model_start = Instant::now();

            let input_data = self.create_model_input(model, &input, &format)?;
            let output = model.process(input_data).await?;

            let (data, fmt) = self.extract_model_output(output)?;

            let model_duration = model_start.elapsed();
            debug!(
                "Single parallel model {} processed in {}ms",
                model.model_id(),
                model_duration.as_millis()
            );

            return Ok((data, fmt));
        }

        let mut handles = Vec::new();

        // 启动所有模型的并行处理任务
        for model in models {
            let model = model.clone();
            let input = input.clone();
            let format = format.clone();
            let pipeline = self.clone();

            let handle = tokio::spawn(async move {
                let model_start = Instant::now();
                let model_id = model.model_id();

                let input_data = match pipeline.create_model_input(&model, &input, &format) {
                    Ok(data) => data,
                    Err(e) => return (model_id, Err(e)),
                };

                let result = model.process(input_data).await;
                let model_duration = model_start.elapsed();

                match &result {
                    Ok(_) => debug!(
                        "Parallel model {} processed in {}ms",
                        model_id,
                        model_duration.as_millis()
                    ),
                    Err(e) => warn!(
                        "Model {} failed after {}ms: {}",
                        model_id,
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
                Ok((model_id, Ok(output))) => match self.extract_model_output(output) {
                    Ok((data, fmt)) => results.push((model_id, data, fmt)),
                    Err(e) => failures.push((model_id, e)),
                },
                Ok((model_id, Err(e))) => {
                    error!("Parallel model {} execution failed: {}", model_id, e);
                    failures.push((model_id, e));
                },
                Err(e) => {
                    error!("Task join error: {}", e);
                    failures.push((
                        "unknown".to_string(),
                        ServiceError::UnsupportedOperation(format!("Task join error: {}", e)),
                    ));
                },
            }
        }

        // 如果没有成功的结果，返回错误
        if results.is_empty() {
            if !failures.is_empty() {
                let (model_id, error) = &failures[0];
                return Err(ServiceError::UnsupportedOperation(format!(
                    "All parallel models failed. First error from {}: {}",
                    model_id, error
                )));
            } else {
                return Err(ServiceError::UnsupportedOperation(
                    "All parallel models failed with unknown errors".to_string(),
                ));
            }
        }

        // 根据合并策略选择结果
        let selected_result = match self.merge_strategy {
            MergeStrategy::First => &results[0],
            MergeStrategy::Last => &results[results.len() - 1],
            MergeStrategy::Longest => results
                .iter()
                .max_by_key(|(_, data, _)| data.len())
                .unwrap(),
            MergeStrategy::Shortest => results
                .iter()
                .min_by_key(|(_, data, _)| data.len())
                .unwrap(),
            MergeStrategy::Concat => {
                // 只支持文本合并
                let mut combined_text = String::new();
                let mut combined_format = None;

                for (_, data, fmt) in &results {
                    if let Some(DataFormat::Text(_)) = fmt {
                        let text = String::from_utf8_lossy(data).to_string();
                        combined_text.push_str(&text);
                        combined_text.push_str("\n");

                        if combined_format.is_none() {
                            combined_format = fmt.clone();
                        }
                    }
                }

                return Ok((combined_text.into_bytes(), combined_format));
            },
        };

        let (_, data, fmt) = selected_result;
        Ok((data.clone(), fmt.clone()))
    }

    /// 创建模型输入数据
    fn create_model_input(
        &self,
        model: &Arc<dyn Model>,
        data: &[u8],
        format: &Option<DataFormat>,
    ) -> ServiceResult<DataType> {
        match format {
            Some(DataFormat::Audio(audio_format)) => {
                // 创建音频输入
                Ok(DataType::Audio {
                    format: audio_format.clone(),
                    data: data.to_vec(),
                    mode: crate::model::StreamMode::NonStreaming,
                })
            },
            Some(DataFormat::Text(_)) => {
                // 创建文本输入
                let text = String::from_utf8_lossy(data).to_string();
                Ok(DataType::Text {
                    content: text,
                    mode: crate::model::StreamMode::NonStreaming,
                })
            },
            None => {
                // 尝试自动检测格式
                if let Ok(text) = String::from_utf8(data.to_vec()) {
                    // 如果可以解析为UTF-8，假设是文本
                    Ok(DataType::Text {
                        content: text,
                        mode: crate::model::StreamMode::NonStreaming,
                    })
                } else {
                    // 否则假设是音频，使用默认格式
                    let default_format = crate::audio::format::AudioFormat::default();
                    Ok(DataType::Audio {
                        format: default_format,
                        data: data.to_vec(),
                        mode: crate::model::StreamMode::NonStreaming,
                    })
                }
            },
        }
    }

    /// 提取模型输出数据
    fn extract_model_output(
        &self,
        output: DataType,
    ) -> ServiceResult<(Vec<u8>, Option<DataFormat>)> {
        match output {
            DataType::Audio { format, data, .. } => Ok((data, Some(DataFormat::Audio(format)))),
            DataType::Text { content, .. } => {
                let default_text_format = crate::text::converter::TextFormat::new("plain");
                Ok((
                    content.into_bytes(),
                    Some(DataFormat::Text(default_text_format)),
                ))
            },
        }
    }

    /// 预计算格式转换信息
    fn precompute_format_conversion(
        &mut self,
        prev_model_index: usize,
        next_model: &Arc<dyn Model>,
    ) {
        let (prev_model, _) = &self.models[prev_model_index];

        // 获取前一个模型的输出类型
        let prev_outputs = prev_model.supported_output_formats();
        let next_inputs = next_model.supported_input_formats();

        // 检查是否需要转换
        let mut needs_conversion = true;
        let mut target_format = None;

        // 检查是否有直接兼容的格式
        for prev_output in &prev_outputs {
            for next_input in &next_inputs {
                if self.is_format_compatible(prev_output, next_input) {
                    needs_conversion = false;
                    break;
                }
            }
            if !needs_conversion {
                break;
            }
        }

        // 如果需要转换，找到合适的目标格式
        if needs_conversion && !next_inputs.is_empty() {
            // 选择第一个支持的格式作为转换目标
            target_format = Some(self.convert_to_data_format(&next_inputs[0]));
        }

        // 存储转换信息
        self.format_conversion_map
            .insert(prev_model_index, target_format);
    }

    /// 检查两种格式是否兼容
    pub fn is_format_compatible(
        &self,
        format1: &crate::model::SupportedFormat,
        format2: &crate::model::SupportedFormat,
    ) -> bool {
        // 检查数据类型是否相同
        if format1.data_type != format2.data_type {
            return false;
        }

        // 根据数据类型检查具体格式
        match format1.data_type.as_str() {
            "Audio" => {
                if let (Some(fmt1), Some(fmt2)) = (&format1.audio_format, &format2.audio_format) {
                    return fmt1.is_compatible_with(fmt2);
                }
            },
            "Text" => {
                // 文本格式通常更灵活，这里简化处理
                return true;
            },
            _ => {},
        }

        false
    }

    /// 将模型支持的格式转换为数据格式
    fn convert_to_data_format(&self, format: &crate::model::SupportedFormat) -> DataFormat {
        match format.data_type.as_str() {
            "Audio" => {
                if let Some(audio_format) = &format.audio_format {
                    DataFormat::Audio(audio_format.clone())
                } else {
                    // 默认音频格式
                    DataFormat::Audio(crate::audio::format::AudioFormat::default())
                }
            },
            "Text" => {
                // 默认文本格式
                DataFormat::Text(crate::text::converter::TextFormat::new("plain"))
            },
            _ => {
                // 未知格式，使用默认文本格式
                DataFormat::Text(crate::text::converter::TextFormat::new("plain"))
            },
        }
    }

    /// 验证两个模型之间的格式兼容性
    fn validate_model_compatibility(
        &self,
        prev_model: &Arc<dyn Model>,
        next_model: &Arc<dyn Model>,
    ) -> ServiceResult<()> {
        // 获取前一个模型的输出格式和下一个模型的输入格式
        let prev_outputs = prev_model.supported_output_formats();
        let next_inputs = next_model.supported_input_formats();

        // 检查是否存在至少一个兼容的格式
        let mut compatible = false;
        for prev_output in &prev_outputs {
            for next_input in &next_inputs {
                if self.is_format_compatible(prev_output, next_input) {
                    compatible = true;
                    break;
                }
            }
            if compatible {
                break;
            }
        }

        if !compatible {
            return Err(ServiceError::UnsupportedOperation(format!(
                "Incompatible models: {} -> {}",
                prev_model.model_id(),
                next_model.model_id()
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::{AudioCodec, AudioFormat};
    use crate::model::{ModelMetadata, SupportedFormat};
    use std::sync::Arc;

    // 模拟模型实现
    struct MockModel {
        id: String,
        input_formats: Vec<SupportedFormat>,
        output_formats: Vec<SupportedFormat>,
    }

    #[async_trait::async_trait]
    impl Model for MockModel {
        async fn process(&self, input: DataType) -> Result<DataType, crate::error::ServiceError> {
            // 简单回显输入
            Ok(input)
        }

        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: self.id.clone(),
                version: "1.0".to_string(),
                input_formats: self.input_formats.clone(),
                output_formats: self.output_formats.clone(),
                metrics_config: Default::default(),
            }
        }

        fn supported_input_formats(&self) -> Vec<SupportedFormat> {
            self.input_formats.clone()
        }

        fn supported_output_formats(&self) -> Vec<SupportedFormat> {
            self.output_formats.clone()
        }

        fn provider(&self) -> String {
            "mock".to_string()
        }

        fn model_id(&self) -> String {
            self.id.clone()
        }
    }

    // 创建测试用的音频格式
    fn create_audio_format(sample_rate: u32, channels: u8) -> SupportedFormat {
        SupportedFormat {
            data_type: "Audio".to_string(),
            streaming: false,
            audio_format: Some(AudioFormat::new(AudioCodec::Wav, sample_rate, channels)),
        }
    }

    // 创建测试用的文本格式
    fn create_text_format() -> SupportedFormat {
        SupportedFormat {
            data_type: "Text".to_string(),
            streaming: false,
            audio_format: None,
        }
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        // 创建模拟组件
        let audio_converter = Arc::new(AudioConverter::default());
        let text_converter = Arc::new(TextConverter::default());
        let metrics_manager = Arc::new(MetricsManager::new());

        // 创建流水线
        let mut pipeline = Pipeline::new(
            "test_pipeline",
            audio_converter,
            text_converter,
            metrics_manager,
        );

        // 创建模拟模型
        let model1 = Arc::new(MockModel {
            id: "model1".to_string(),
            input_formats: vec![create_text_format()],
            output_formats: vec![create_text_format()],
        });

        let model2 = Arc::new(MockModel {
            id: "model2".to_string(),
            input_formats: vec![create_text_format()],
            output_formats: vec![create_text_format()],
        });

        // 添加模型到流水线
        pipeline
            .add_model(model1, ExecutionMode::Sequential)
            .unwrap();
        pipeline
            .add_model(model2, ExecutionMode::Sequential)
            .unwrap();

        // 执行流水线
        let input = "Hello, world!".as_bytes().to_vec();
        let format = Some(DataFormat::Text(TextFormat::new("plain")));

        let (output, output_format) = pipeline.execute(input, format).await.unwrap();

        // 验证结果
        assert!(output_format.is_some());
        assert!(output_format.unwrap().is_text());
        assert_eq!(String::from_utf8_lossy(&output), "Hello, world!");
    }
}

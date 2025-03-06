use crate::audio::converter::FormatConverterFactory;
use crate::audio::converter::GenericConversionStrategy;
use crate::audio::format::AudioCodec;
use crate::audio::{AudioConverter, AudioFormat};
use crate::error::{ServiceError, ServiceResult};
use crate::metrics::MetricsManager;
use crate::model::factory::ModelFactory;
use crate::model::{Model, ModelFactory, ModelParams};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::pipeline::{ExecutionMode, Pipeline};
use super::pipeline_group::{MergeStrategy, ModelGroup};
use super::pipeline_with_groups::PipelineWithGroups;

/// 流水线请求配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRequest {
    /// 流水线名称
    pub name: String,
    /// 模型配置列表
    pub models: Vec<ModelRequest>,
    /// 模型组配置列表（可选）
    #[serde(default)]
    pub model_groups: Vec<ModelGroupRequest>,
    /// 结果合并策略
    #[serde(default)]
    pub merge_strategy: Option<MergeStrategy>,
}

impl PipelineRequest {
    /// 生成缓存键
    ///
    /// 根据请求参数生成唯一的缓存键，用于标识不同的流水线配置
    pub fn cache_key(&self) -> String {
        // 生成更精确的缓存键，包含模型和模型组信息
        let mut key = format!("pipeline:{}", self.name);

        // 添加模型信息
        if !self.models.is_empty() {
            key.push_str(":models");
            for model in &self.models {
                key.push_str(&format!(",{}:{}", model.provider, model.model_id));
            }
        }

        // 添加模型组信息
        if !self.model_groups.is_empty() {
            key.push_str(":groups");
            for group in &self.model_groups {
                key.push_str(&format!(",{}", group.name));
                for model in &group.models {
                    key.push_str(&format!("-{}:{}", model.provider, model.model_id));
                }
            }
        }

        key
    }

    /// 检查是否使用模型组
    ///
    /// 如果请求中包含模型组配置，则返回true
    pub fn uses_model_groups(&self) -> bool {
        !self.model_groups.is_empty()
    }

    /// 验证请求配置的有效性
    ///
    /// 检查请求中的模型配置是否有效
    pub fn validate(&self) -> ServiceResult<()> {
        // 检查是否至少有一个模型或模型组
        if self.models.is_empty() && self.model_groups.is_empty() {
            return Err(ServiceError::Pipeline(
                "Pipeline request must contain at least one model or model group".to_string(),
            ));
        }

        // 检查模型组是否有效
        for group in &self.model_groups {
            if group.models.is_empty() {
                return Err(ServiceError::Pipeline(format!(
                    "Model group '{}' has no models",
                    group.name
                )));
            }
        }

        Ok(())
    }
}

/// 模型请求配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    /// 模型提供者
    pub provider: String,
    /// 模型ID
    pub model_id: String,
    /// 模型参数
    pub parameters: ModelParams,
    /// 执行模式
    #[serde(default = "default_execution_mode")]
    pub execution_mode: ExecutionMode,
}

/// 模型组请求配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelGroupRequest {
    /// 组名称
    pub name: String,
    /// 组内模型配置列表
    pub models: Vec<ModelRequest>,
    /// 结果合并策略
    #[serde(default = "default_merge_strategy")]
    pub merge_strategy: MergeStrategy,
}

/// 默认执行模式为串行
fn default_execution_mode() -> ExecutionMode {
    ExecutionMode::Sequential
}

/// 默认合并策略为使用第一个结果
fn default_merge_strategy() -> MergeStrategy {
    MergeStrategy::First
}

/// 流水线工厂
///
/// 负责创建和管理音频处理流水线，支持流水线缓存和格式转换策略的注册。
///
/// # 示例
///
/// ```rust
/// use crate::orchestration::factory_new::{PipelineFactory, PipelineRequest};
///
/// let factory = PipelineFactory::new(
///     model_factory,
///     audio_converter,
///     metrics_manager
/// );
///
/// let pipeline = factory.create_pipeline(&request).await?;
/// ```
pub struct PipelineFactory {
    /// 模型工厂
    model_factory: Arc<ModelFactory>,
    /// 音频转换器
    audio_converter: Arc<AudioConverter>,
    /// 性能指标管理器
    metrics_manager: Arc<MetricsManager>,
    /// 缓存的流水线
    pipeline_cache: RwLock<HashMap<String, Arc<dyn PipelineExecutor>>>,
    /// 缓存键的使用顺序，用于实现LRU缓存策略
    cache_keys_order: RwLock<VecDeque<String>>,
    /// 最大缓存大小
    max_cache_size: usize,
}

/// 流水线执行器特征
///
/// 定义了流水线执行的通用接口，使不同类型的流水线实现可以统一处理
#[async_trait::async_trait]
pub trait PipelineExecutor: Send + Sync {
    /// 执行流水线处理
    async fn execute(
        &self,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)>;
}

#[async_trait::async_trait]
impl PipelineExecutor for Pipeline {
    async fn execute(
        &self,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)> {
        Pipeline::execute(self, input, format).await
    }
}

#[async_trait::async_trait]
impl PipelineExecutor for PipelineWithGroups {
    async fn execute(
        &self,
        input: Vec<u8>,
        format: Option<AudioFormat>,
    ) -> ServiceResult<(Vec<u8>, Option<AudioFormat>)> {
        PipelineWithGroups::execute(self, input, format).await
    }
}

impl PipelineFactory {
    /// 创建新的流水线工厂
    ///
    /// # 参数
    ///
    /// * `model_factory` - 模型工厂实例
    /// * `audio_converter` - 音频转换器实例
    /// * `metrics_manager` - 性能指标管理器实例
    pub fn new(
        model_factory: Arc<ModelFactory>,
        audio_converter: Arc<AudioConverter>,
        metrics_manager: Arc<MetricsManager>,
    ) -> Self {
        Self {
            model_factory,
            audio_converter,
            metrics_manager,
            pipeline_cache: RwLock::new(HashMap::new()),
            cache_keys_order: RwLock::new(VecDeque::new()),
            max_cache_size: 50, // 默认最大缓存大小
        }
    }

    /// 设置最大缓存大小
    ///
    /// # 参数
    ///
    /// * `size` - 最大缓存大小
    pub fn with_max_cache_size(mut self, size: usize) -> Self {
        self.max_cache_size = size;
        self
    }

    /// 根据请求创建流水线
    ///
    /// # 参数
    ///
    /// * `request` - 流水线配置请求
    ///
    /// # 返回值
    ///
    /// 返回创建的流水线实例
    pub async fn create_pipeline(
        &self,
        request: &PipelineRequest,
    ) -> ServiceResult<Arc<dyn PipelineExecutor>> {
        // 验证请求配置
        request.validate()?;

        // 检查缓存
        let cache_key = request.cache_key();
        {
            let cache = self.pipeline_cache.read().await;
            if let Some(pipeline) = cache.get(&cache_key) {
                debug!("Using cached pipeline: {}", cache_key);

                // 更新缓存使用顺序
                self.update_cache_order(&cache_key).await;

                return Ok(pipeline.clone());
            }
        }

        // 创建格式转换工厂
        let mut format_converter_factory = FormatConverterFactory::new();

        // 注册常用的格式转换策略
        self.register_conversion_strategies(&mut format_converter_factory)
            .await?;

        let format_converter_factory = Arc::new(format_converter_factory);

        // 根据请求类型创建不同的流水线
        let pipeline: Arc<dyn PipelineExecutor> = if request.uses_model_groups() {
            // 创建基于模型组的流水线
            debug!("Creating new pipeline with groups: {}", request.name);
            let mut pipeline = PipelineWithGroups::new(
                self.audio_converter.clone(),
                format_converter_factory,
                self.metrics_manager.clone(),
            );

            // 创建并添加模型组
            for group_req in &request.model_groups {
                let mut group = ModelGroup::new(&group_req.name);

                // 设置合并策略
                group.set_merge_strategy(group_req.merge_strategy);

                // 添加模型
                for model_req in &group_req.models {
                    let model = match self
                        .model_factory
                        .create_model(
                            &model_req.provider,
                            &model_req.model_id,
                            &model_req.parameters,
                        )
                        .await
                    {
                        Ok(model) => model,
                        Err(e) => {
                            warn!(
                                "Failed to create model {}:{} for group {}: {}",
                                model_req.provider, model_req.model_id, group_req.name, e
                            );
                            return Err(ServiceError::Pipeline(format!(
                                "Failed to create model {}:{} for group {}: {}",
                                model_req.provider, model_req.model_id, group_req.name, e
                            )));
                        },
                    };

                    group.add_model(model);
                }

                if let Err(e) = pipeline.add_group(group) {
                    warn!("Failed to add group {} to pipeline: {}", group_req.name, e);
                    return Err(e);
                }
            }

            Arc::new(pipeline)
        } else {
            // 创建标准流水线
            debug!("Creating new standard pipeline: {}", request.name);
            let mut pipeline = Pipeline::new(
                self.audio_converter.clone(),
                format_converter_factory,
                self.metrics_manager.clone(),
            );

            // 设置合并策略（如果有）
            if let Some(strategy) = request.merge_strategy {
                pipeline.set_merge_strategy(strategy);
            }

            // 添加模型
            for model_req in &request.models {
                let model = match self
                    .model_factory
                    .create_model(
                        &model_req.provider,
                        &model_req.model_id,
                        &model_req.parameters,
                    )
                    .await
                {
                    Ok(model) => model,
                    Err(e) => {
                        warn!(
                            "Failed to create model {}:{}: {}",
                            model_req.provider, model_req.model_id, e
                        );
                        return Err(ServiceError::Pipeline(format!(
                            "Failed to create model {}:{}: {}",
                            model_req.provider, model_req.model_id, e
                        )));
                    },
                };

                if let Err(e) = pipeline.add_model(model, model_req.execution_mode) {
                    warn!(
                        "Failed to add model {}:{} to pipeline: {}",
                        model_req.provider, model_req.model_id, e
                    );
                    return Err(e);
                }
            }

            Arc::new(pipeline)
        };

        // 缓存流水线
        self.add_to_cache(cache_key, pipeline.clone()).await;

        Ok(pipeline)
    }

    /// 更新缓存使用顺序
    ///
    /// 将指定的缓存键移动到队列末尾，表示最近使用
    async fn update_cache_order(&self, cache_key: &str) -> () {
        let mut order = self.cache_keys_order.write().await;
        // 如果键已存在，先移除它
        if let Some(pos) = order.iter().position(|k| k == cache_key) {
            order.remove(pos);
        }
        // 将键添加到队列末尾
        order.push_back(cache_key.to_string());
    }

    /// 添加流水线到缓存
    ///
    /// # 参数
    ///
    /// * `cache_key` - 缓存键
    /// * `pipeline` - 要缓存的流水线实例
    async fn add_to_cache(&self, cache_key: String, pipeline: Arc<dyn PipelineExecutor>) -> () {
        let mut cache = self.pipeline_cache.write().await;
        let mut order = self.cache_keys_order.write().await;

        // 如果缓存已满，移除最久未使用的项
        if cache.len() >= self.max_cache_size && !order.is_empty() {
            if let Some(oldest_key) = order.pop_front() {
                cache.remove(&oldest_key);
                debug!("Removed oldest pipeline from cache: {}", oldest_key);
            }
        }

        // 添加新项到缓存
        cache.insert(cache_key.clone(), pipeline);
        order.push_back(cache_key);
    }

    /// 注册常用的格式转换策略
    ///
    /// 为格式转换工厂注册默认的音频格式转换策略
    async fn register_conversion_strategies(
        &self,
        factory: &mut FormatConverterFactory,
    ) -> ServiceResult<()> {
        // 注册WAV转换策略
        factory.register_strategy(
            AudioCodec::Wav,
            AudioCodec::Mp3,
            Box::new(GenericConversionStrategy::new()),
        );

        // 注册MP3转换策略
        factory.register_strategy(
            AudioCodec::Mp3,
            AudioCodec::Wav,
            Box::new(GenericConversionStrategy::new()),
        );

        // 注册OGG转换策略
        factory.register_strategy(
            AudioCodec::Ogg,
            AudioCodec::Wav,
            Box::new(GenericConversionStrategy::new()),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_pipeline_request() {
        let request = PipelineRequest {
            name: "test_pipeline".to_string(),
            models: vec![ModelRequest {
                provider: "test_provider".to_string(),
                model_id: "model1".to_string(),
                parameters: ModelParams::default(),
                execution_mode: ExecutionMode::Sequential,
            }],
            merge_strategy: None,
        };

        assert_eq!(request.cache_key(), "pipeline:test_pipeline");
    }

    #[test]
    fn test_pipeline_factory_creation() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let model_factory = Arc::new(ModelFactory::new());
            let audio_converter = Arc::new(AudioConverter::new(
                AudioFormat::default(),
                AudioFormat::default(),
            ));
            let metrics_manager = Arc::new(MetricsManager::new());

            let factory = PipelineFactory::new(model_factory, audio_converter, metrics_manager);

            assert!(factory.pipeline_cache.read().await.is_empty());
        });
    }

    #[test]
    fn test_pipeline_caching() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let model_factory = Arc::new(ModelFactory::new());
            let audio_converter = Arc::new(AudioConverter::new(
                AudioFormat::default(),
                AudioFormat::default(),
            ));
            let metrics_manager = Arc::new(MetricsManager::new());

            let factory = PipelineFactory::new(model_factory, audio_converter, metrics_manager);

            let request = PipelineRequest {
                name: "test_pipeline".to_string(),
                models: vec![],
                merge_strategy: None,
            };

            // 首次创建流水线
            let pipeline1 = factory.create_pipeline(&request).await.unwrap();

            // 再次请求相同配置
            let pipeline2 = factory.create_pipeline(&request).await.unwrap();

            // 验证是否返回缓存的实例
            assert!(Arc::ptr_eq(&pipeline1, &pipeline2));
        });
    }
}

//! 模型编排模块
//!
//! 本模块负责管理和执行模型处理流水线，提供以下核心功能：
//!
//! - 处理流水线（Pipeline）：组织和执行多个模型，支持串行和并行执行模式，管理模型间的数据流转和格式转换
//! - 流水线工厂（Factory）：创建和缓存处理流水线实例，优化资源利用
//! - 数据格式（DataFormat）：统一不同类型数据的表示和处理
//! - 格式转换器（FormatConverter）：提供不同数据格式之间的转换能力
//!
//! # 示例
//!
//! ```rust
//! use crate::orchestration::{PipelineFactory, PipelineRequest, ExecutionMode, MergeStrategy};
//! use crate::model::ModelParams;
//!
//! // 创建流水线工厂
//! let factory = PipelineFactory::new(model_factory, audio_converter, text_converter, metrics_manager);
//!
//! // 构建处理请求
//! let request = PipelineRequest {
//!     name: "audio_pipeline".to_string(),
//!     models: vec![
//!         ModelRequest {
//!             provider: "whisper".to_string(),
//!             model_id: "large".to_string(),
//!             parameters: ModelParams::default(),
//!             execution_mode: ExecutionMode::Sequential,
//!         },
//!         ModelRequest {
//!             provider: "gpt".to_string(),
//!             model_id: "gpt-4".to_string(),
//!             parameters: ModelParams::default(),
//!             execution_mode: ExecutionMode::Sequential,
//!         },
//!     ],
//!     merge_strategy: Some(MergeStrategy::First),
//! };
//!
//! // 创建并执行流水线
//! let pipeline = factory.create_pipeline(&request).await?;
//! let (result, format) = pipeline.execute(input, input_format).await?;
//! ```

// 导入模块
mod factory;
mod pipeline;
mod types;

// 导出公共接口
pub use factory::{ModelRequest, PipelineFactory, PipelineRequest};
pub use types::FormatConverter;

//! 模型编排模块
//!
//! 本模块负责管理和执行模型处理流水线，提供以下核心功能：
//!
//! - 处理流水线（Pipeline）：组织和执行多个模型，支持串行和并行执行模式，管理模型间的数据流转和格式转换
//! - 流水线工厂（Factory）：创建和缓存处理流水线实例，优化资源利用
//! - 协议处理器（ProtocolHandler）：统一不同通信协议的处理接口，支持HTTP和WebSocket
//!
//! # 示例
//!
//! ```rust
//! use crate::orchestration::{PipelineFactory, PipelineRequest, ExecutionMode};
//!
//! // 创建流水线工厂
//! let factory = PipelineFactory::new(model_factory, audio_converter, metrics);
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
//!         // ...
//!     ],
//!     // ...
//! };
//!
//! // 创建并执行流水线
//! let pipeline = factory.create_pipeline(&request).await?;
//! let result = pipeline.execute(input).await?;
//! ```

// 导入模块
mod factory;
mod pipeline;
mod pipeline_group;
mod pipeline_with_groups;
mod protocol_handler;

// 导出新版API
pub use factory::{PipelineFactory, PipelineRequest};
pub use protocol_handler::ProtocolHandler;

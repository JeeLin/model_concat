//! 网络服务模块
//!
//! 本模块提供了模型串联服务的网络接口，支持以下功能：
//!
//! - HTTP API：提供RESTful API接口，支持同步处理请求和模型编排
//! - WebSocket：支持实时双向通信，适用于流式处理场景
//! - 任务管理：统一的任务调度和状态管理
//! - 性能监控：请求处理性能指标收集
//!
//! # 示例
//!
//! ```rust
//! use crate::server::start_server;
//! use crate::config::Config;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::from_file("config/default.toml")?;
//!     // 启动服务器
//!     start_server(&config).await?
//!     Ok(())
//! }
//! ```

use crate::audio::converter::AudioConverter;
use crate::config::Config;
use crate::error::ServiceResult;
use crate::metrics::MetricsManager;
use crate::model::factory::ModelFactory;
use crate::orchestration::PipelineFactory;
use crate::text::converter::TextConverter;
use axum::routing::post;
use axum::{routing::get, Extension, Router};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

pub mod http;
pub mod request;
pub mod task;
pub mod websocket;

/// 启动HTTP和WebSocket服务器
///
/// # 参数
/// * `config` - 服务配置
///
/// # 返回
/// 返回服务器运行结果
pub async fn start_server(config: &Config) -> ServiceResult<()> {
    // 初始化组件
    info!("初始化服务组件...");

    // 创建模型工厂
    let model_factory = Arc::new(ModelFactory::new(config.api_providers.clone()));

    // 创建性能指标管理器
    let metrics_manager = Arc::new(MetricsManager::new("metrics.json"));

    // 创建流水线工厂
    let pipeline_factory = Arc::new(PipelineFactory::new(
        &model_factory,
        metrics_manager.clone(),
    ));

    // 创建任务管理器
    let task_manager = Arc::new(task::TaskManager::new(pipeline_factory.clone()));

    // 构造HTTP服务地址
    let http_addr = format!("{0}:{1}", config.server.host, config.server.port);
    info!("启动HTTP服务器: {}", &http_addr);

    // 创建HTTP路由
    let app = Router::new()
        .route("/providers", get(http::get_providers))
        .route("/providers/{provider}/models", get(http::get_models))
        .route("/models/{provider_and_model}", get(http::get_model_detail))
        .route("/metrics", get(http::get_metrics))
        .route("/ws", get(websocket::ws_handler))
        .layer(Extension(task_manager))
        .layer(Extension(model_factory.clone()))
        .layer(Extension(metrics_manager));

    // 启动HTTP服务器
    let listener = TcpListener::bind(&http_addr).await?;
    info!("HTTP服务器启动成功，监听地址: {}", &http_addr);

    axum::serve(listener, app).await?;

    Ok(())
}

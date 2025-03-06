//! 服务器模块
//!
//! 本模块提供HTTP和WebSocket服务器功能，支持以下特性：
//!
//! - HTTP API：提供RESTful API接口，支持同步处理请求
//! - WebSocket：支持实时双向通信，适用于流式处理场景
//! - 任务管理：统一的任务调度和状态管理
//! - 性能监控：请求处理性能指标收集
//!
//! # 示例
//!
//! ```rust
//! use crate::server::start_server;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 启动服务器
//!     start_server("127.0.0.1:8080").await?
//!     Ok(())
//! }
//! ```

use crate::audio::{AudioConverter, AudioFormat};
use crate::model::factory::ModelFactory;
use crate::orchestration::PipelineFactory;
use crate::server::websocket::ws_handler;
use axum::{routing::get, Extension, Router};
use std::sync::Arc;
use tokio::net::TcpListener;

pub mod http;
pub mod request;
pub mod task;
pub mod websocket;

/// 启动HTTP和WebSocket服务器
///
/// # 参数
/// * `http_addr` - HTTP服务器监听地址，格式为"host:port"
///
/// # 返回
/// 返回服务器运行结果
pub async fn start_server(http_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 初始化组件
    let model_factory = Arc::new(ModelFactory::new());
    let audio_format = AudioFormat::default();
    let audio_converter = Arc::new(AudioConverter::new(
        audio_format.clone(),
        audio_format.clone(),
    ));

    // 创建流水线工厂
    let pipeline_factory = Arc::new(PipelineFactory::new(model_factory, audio_converter));

    // 创建HTTP路由
    let app = Router::new()
        .route("/", get(|| async { "Model Concat Server" }))
        .route("/api/v1/process", get(http::process_request))
        .route("/ws", get(ws_handler))
        .layer(Extension(pipeline_factory));

    // 启动HTTP服务器
    let listener = TcpListener::bind(http_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

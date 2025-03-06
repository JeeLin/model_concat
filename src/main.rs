//! Model Concat Service - 模型编排与调用服务
//!
//! 本服务提供了一个灵活的模型编排和调用框架，支持多种AI模型的统一接入和串联调用。
//! 主要功能包括:
//! - 多模型适配：支持 OpenAI、Anthropic、DeepSeek 等多家厂商的模型接入
//! - 协议支持：支持 HTTP 和 WebSocket 两种调用协议
//! - 编排调度：支持多个模型的串联调用和结果编排
//! - 音频处理：支持音频输入的转换和处理
//!
//! # 架构设计
//! - model: 模型适配器模块，负责对接不同厂商的模型接口
//! - orchestration: 编排模块，负责模型的串联调用和结果处理
//! - audio: 音频处理模块，提供音频格式转换等功能
//! - server: HTTP/WebSocket 服务器，处理外部调用请求
//! - config: 配置管理模块
//! - error: 错误处理模块
//! - logger: 日志模块
//! - metrics: 监控指标模块

use tokio;

use crate::config::Config;
use crate::logger::Logger;
use crate::server::start_server;

pub mod audio;
pub mod config;
pub mod error;
pub mod logger;
pub mod metrics;
pub mod model;
pub mod orchestration;
pub mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统
    let _logger = Logger::init()?;
    log_info!("Starting Model Concat Service...");

    // 加载服务配置文件
    let config = match Config::from_file("config/default.toml") {
        Ok(config) => config,
        Err(e) => {
            log_error!("Failed to load configuration: {}", e);
            return Err(e.into());
        },
    };

    // 构造HTTP服务地址
    let http_addr = format!("{}:{}", config.server.host, config.server.port);
    log_info!("Server starting on {}", &http_addr);
    // 启动HTTP服务器
    start_server(&http_addr).await
}

use chrono::Local;
use std::io;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// 日志管理器
pub struct Logger {}

impl Logger {
    pub fn init() -> ServiceResult<Self> {
        // 创建日志目录
        std::fs::create_dir_all("logs")?;

        // 配置文件追加器
        let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "model_concat.log");

        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        // 创建控制台输出层
        let console_layer = fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_target(false)
            .with_ansi(true)
            .pretty();

        // 创建文件输出层
        let file_layer = fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_target(false)
            .with_ansi(false)
            .with_writer(non_blocking);

        // 初始化订阅者
        tracing_subscriber::registry()
            .with(console_layer)
            .with(file_layer)
            .with(Level::INFO)
            .init();

        tracing::info!("Logger initialized");
    }
}

/// 日志宏
pub mod macros {
    /// 错误级别日志
    #[macro_export]
    macro_rules! log_error {
        ($($arg:tt)*) => {
            ::tracing::error!($($arg)*);
        };
    }

    /// 警告级别日志
    #[macro_export]
    macro_rules! log_warn {
        ($($arg:tt)*) => {
            ::tracing::warn!($($arg)*);
        };
    }

    /// 信息级别日志
    #[macro_export]
    macro_rules! log_info {
        ($($arg:tt)*) => {
            ::tracing::info!($($arg)*);
        };
    }

    /// 调试级别日志
    #[macro_export]
    macro_rules! log_debug {
        ($($arg:tt)*) => {
            ::tracing::debug!($($arg)*);
        };
    }

    /// 追踪级别日志
    #[macro_export]
    macro_rules! log_trace {
        ($($arg:tt)*) => {
            ::tracing::trace!($($arg)*);
        };
    }
}

use crate::error::ServiceResult;
// 重导出日志宏
pub use crate::{log_debug, log_error, log_info, log_trace, log_warn};

use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;

#[derive(Debug, Clone, Default)]
pub struct WsConfig {
    pub timeout_sec: u64,
    pub max_retries: u32,
    pub heartbeat_interval_sec: u64,
}

/// WebSocket协议处理器
///
/// 实现了基于WebSocket协议的模型API调用
pub trait WsHandler: ProtocolHandler {
    /// 建立WebSocket连接
    fn connect(&self, url: &str, headers: &[(String, String)]) -> Result<(), AdapterError>;

    /// 发送WebSocket消息
    fn send_message(&self, message: &[u8]) -> Result<(), AdapterError>;

    /// 接收WebSocket消息
    fn receive_message(&self) -> Result<Vec<u8>, AdapterError>;

    /// 关闭WebSocket连接
    fn close(&self) -> Result<(), AdapterError>;
}

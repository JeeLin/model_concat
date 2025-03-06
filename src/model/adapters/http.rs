use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;

#[derive(Debug, Clone, Default)]
pub struct HttpConfig {
    pub timeout_sec: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

/// HTTP协议处理器
///
/// 实现了基于HTTP协议的模型API调用
pub trait HttpHandler: ProtocolHandler {
    /// 发送HTTP请求
    fn send_http_request(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<Vec<u8>, AdapterError>;

    /// 处理HTTP响应
    fn handle_http_response(
        &self,
        status_code: u16,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<Vec<u8>, AdapterError>;
}

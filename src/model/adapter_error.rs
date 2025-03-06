use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Unsupported protocol: {0}")]
    UnsupportedProtocol(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Certificate error: {0}")]
    CertError(String),

    #[error("Request error: {0}")]
    RequestError(String),

    #[error("Response error: {0}")]
    ResponseError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("{0}")]
    Other(String),
}

impl AdapterError {
    /// 设置错误关联的供应商名称
    pub fn set_vendor(&mut self, vendor: &str) {
        // 在实际实现中，可以添加供应商信息到错误中
        // 这里只是一个示例实现
    }
}

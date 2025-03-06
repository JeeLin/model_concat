use super::ProviderAdapter;
use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub max_retries: u32,
    pub timeout_sec: u64,
}

#[derive(Debug)]
pub struct ApiProviderAdapter {
    pub api_key: String,
    pub base_url: String,
}

impl ApiProviderAdapter {
    pub fn new(config: ApiProviderConfig) -> Self {
        Self {
            api_key: config.api_key,
            base_url: config.base_url,
        }
    }
}

impl ProviderAdapter for ApiProviderAdapter {
    fn select_protocol(
        &self,
        protocol_type: &str,
    ) -> Result<Box<dyn ProtocolHandler>, AdapterError> {
        match protocol_type {
            "http" => Ok(Box::new(ApiProviderHttpHandler {
                api_key: self.api_key.clone(),
                base_url: self.base_url.clone(),
            })),
            _ => Err(AdapterError::UnsupportedProtocol(protocol_type.to_string())),
        }
    }

    fn vendor_name(&self) -> &str {
        "api_provider"
    }
}

#[derive(Debug)]
struct ApiProviderHttpHandler {
    api_key: String,
    base_url: String,
}

impl ProtocolHandler for ApiProviderHttpHandler {
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
        // 实际实现会调用API
        // 这里只是一个占位实现
        Err(AdapterError::Other(
            "API Provider HTTP handler not fully implemented".to_string(),
        ))
    }

    fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
        // 实际实现会验证API响应
        // 这里只是一个占位实现
        Ok(true)
    }
}

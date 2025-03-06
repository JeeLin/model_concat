use crate::error::AdapterError;
use crate::orchestration::ProtocolHandler;
use reqwest::{Client, ClientBuilder};
use std::time::Duration;

#[derive(Debug)]
pub struct SelfHostedHttpHandler {
    api_key: String,
    base_url: String,
    internal_network: bool,
    cert_path: Option<String>,
}

impl SelfHostedHttpHandler {
    pub fn new(
        api_key: String,
        base_url: String,
        internal_network: bool,
        cert_path: Option<String>,
    ) -> Self {
        Self {
            api_key,
            base_url,
            internal_network,
            cert_path,
        }
    }

    async fn build_client(&self) -> Result<Client, AdapterError> {
        let mut builder = ClientBuilder::new()
            .danger_accept_invalid_certs(self.cert_path.is_some())
            .default_headers(
                vec![("Authorization", format!("Bearer {}", self.api_key))]
                    .into_iter()
                    .collect(),
            );

        if self.internal_network {
            builder = builder
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30));
        }

        if let Some(cert_path) = &self.cert_path {
            let cert = tokio::fs::read(cert_path)
                .await
                .map_err(|e| AdapterError::CertError(e.to_string()))?;
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(&cert)
                    .map_err(|e| AdapterError::CertError(e.to_string()))?,
            );
        }

        builder
            .build()
            .map_err(|e| AdapterError::ConnectionError(e.to_string()))
    }
}

impl ProtocolHandler for SelfHostedHttpHandler {
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
        let auth_header = format!("Authorization: Bearer {}", &self.api_key);

        // 构造带有认证头的HTTP请求
        let mut request = vec![];
        request.extend_from_slice(auth_header.as_bytes());
        request.extend_from_slice(payload);

        // 实现错误重试机制
        Ok(request)
    }

    fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
        // 实现响应验证逻辑
        Ok(true)
    }
}

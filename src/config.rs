use crate::error::{ServiceError, ServiceResult};
use crate::model::{ProviderConfig, ProvidersConfig};
use crate::{log_error, log_info};
use serde::Deserialize;
use std::fs;
use tracing::{error, info};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub api_providers: ProvidersConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn from_file(path: &str) -> ServiceResult<Self> {
        log_info!("Loading configuration from {}", path);
        let content = fs::read_to_string(path).map_err(|e| {
            log_error!("Failed to read config file: {}", e);
            ServiceError::Config(format!("Failed to read config file: {}", e))
        })?;

        let config: Config = toml::from_str(&content).map_err(|e| {
            log_error!("Failed to parse config file: {}", e);
            ServiceError::Config(format!("Failed to parse config file: {}", e))
        })?;

        log_info!("Configuration loaded successfully");
        Ok(config)
    }
}

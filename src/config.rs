use crate::error::{ServiceError, ServiceResult};
use crate::model::adapters::api_provider::ApiProviderConfig;
use crate::{log_error, log_info};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use tracing::{error, info};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ollama: HashMap<String, OllamaConfig>,
    pub server: ServerConfig,
    pub stages: Vec<StageConfig>,
    pub audio: AudioConfig,
    #[serde(default)]
    pub api_providers: HashMap<String, ApiProviderConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StageConfig {
    pub name: String,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub api_key: String,
    pub protocol: String,
    pub timeout_sec: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub name: String,
    pub url: String,
    pub timeout: u64,
    pub retry_count: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AudioConfig {
    pub supported_formats: Vec<String>,
    pub max_file_size: usize,
    pub default_sample_rate: u32,
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

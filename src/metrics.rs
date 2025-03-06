use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    /// 模型提供方
    pub provider: String,
    /// 模型ID
    pub model_id: String,
    /// 总执行次数
    pub total_executions: u64,
    /// 总执行时间（毫秒）
    pub total_time_ms: u64,
    /// 平均执行时间（毫秒）
    pub average_time_ms: f64,
    /// 最短执行时间（毫秒）
    pub min_time_ms: u64,
    /// 最长执行时间（毫秒）
    pub max_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConversionMetrics {
    /// 源格式
    pub from_format: String,
    /// 目标格式
    pub to_format: String,
    /// 总执行次数
    pub total_executions: u64,
    /// 总执行时间（毫秒）
    pub total_time_ms: u64,
    /// 平均执行时间（毫秒）
    pub average_time_ms: f64,
    /// 最短执行时间（毫秒）
    pub min_time_ms: u64,
    /// 最长执行时间（毫秒）
    pub max_time_ms: u64,
    /// 总处理数据大小（字节）
    pub total_data_size: u64,
    /// 平均处理速度（MB/s）
    pub average_speed_mbs: f64,
}

#[derive(Debug)]
pub struct MetricsManager {
    /// 模型性能指标
    model_metrics: RwLock<HashMap<String, ModelMetrics>>,
    /// 音频转换性能指标
    audio_metrics: RwLock<HashMap<String, AudioConversionMetrics>>,
    /// 指标文件路径
    metrics_file: String,
}

impl ModelMetrics {
    pub fn new(provider: String, model_id: String) -> Self {
        Self {
            provider,
            model_id,
            total_executions: 0,
            total_time_ms: 0,
            average_time_ms: 0.0,
            min_time_ms: u64::MAX,
            max_time_ms: 0,
        }
    }

    pub fn update(&mut self, execution_time_ms: u64) {
        self.total_executions += 1;
        self.total_time_ms += execution_time_ms;
        self.average_time_ms = self.total_time_ms as f64 / self.total_executions as f64;
        self.min_time_ms = self.min_time_ms.min(execution_time_ms);
        self.max_time_ms = self.max_time_ms.max(execution_time_ms);
    }
}

impl AudioConversionMetrics {
    pub fn new(from_format: String, to_format: String) -> Self {
        Self {
            from_format,
            to_format,
            total_executions: 0,
            total_time_ms: 0,
            average_time_ms: 0.0,
            min_time_ms: u64::MAX,
            max_time_ms: 0,
            total_data_size: 0,
            average_speed_mbs: 0.0,
        }
    }

    pub fn update(&mut self, execution_time_ms: u64, data_size: u64) {
        self.total_executions += 1;
        self.total_time_ms += execution_time_ms;
        self.average_time_ms = self.total_time_ms as f64 / self.total_executions as f64;
        self.min_time_ms = self.min_time_ms.min(execution_time_ms);
        self.max_time_ms = self.max_time_ms.max(execution_time_ms);
        self.total_data_size += data_size;

        // 计算平均处理速度（MB/s）
        let total_mb = self.total_data_size as f64 / (1024.0 * 1024.0);
        let total_seconds = self.total_time_ms as f64 / 1000.0;
        self.average_speed_mbs = total_mb / total_seconds;
    }
}

impl MetricsManager {
    pub fn new(metrics_file: &str) -> Self {
        // 确保目录存在
        if let Some(parent) = Path::new(metrics_file).parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    error!("Failed to create metrics directory: {}", e);
                }
            }
        }

        Self {
            model_metrics: RwLock::new(HashMap::new()),
            audio_metrics: RwLock::new(HashMap::new()),
            metrics_file: metrics_file.to_string(),
        }
    }

    /// 从文件加载历史指标数据
    pub async fn load_metrics(&self) -> io::Result<()> {
        let path = Path::new(&self.metrics_file);
        if !path.exists() {
            return Ok(());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let data: serde_json::Value = serde_json::from_reader(reader).unwrap_or_else(|e| {
            warn!("Failed to load metrics from file: {}", e);
            serde_json::json!({
                "models": {},
                "audio_conversions": {}
            })
        });

        if let Some(models) = data.get("models") {
            if let Ok(metrics) = serde_json::from_value(models.clone()) {
                let mut model_metrics = self.model_metrics.write().await;
                *model_metrics = metrics;
            }
        }

        if let Some(audio) = data.get("audio_conversions") {
            if let Ok(metrics) = serde_json::from_value(audio.clone()) {
                let mut audio_metrics = self.audio_metrics.write().await;
                *audio_metrics = metrics;
            }
        }

        info!("Loaded metrics from file");
        Ok(())
    }

    /// 保存指标数据到文件
    pub async fn save_metrics(&self) -> io::Result<()> {
        let model_metrics = self.model_metrics.read().await;
        let audio_metrics = self.audio_metrics.read().await;

        let data = serde_json::json!({
            "models": *model_metrics,
            "audio_conversions": *audio_metrics
        });

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.metrics_file)?;

        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &data)?;

        info!("Saved metrics to file");
        Ok(())
    }

    /// 更新模型性能指标
    pub async fn update_model_metrics(
        &self,
        provider: &str,
        model_id: &str,
        execution_time_ms: u64,
    ) {
        let mut metrics = self.model_metrics.write().await;
        let key = format!("{}/{}", provider, model_id);
        let model_metrics = metrics
            .entry(key)
            .or_insert_with(|| ModelMetrics::new(provider.to_string(), model_id.to_string()));
        model_metrics.update(execution_time_ms);
    }

    /// 更新音频转换性能指标
    pub async fn update_audio_metrics(
        &self,
        from_format: &str,
        to_format: &str,
        execution_time_ms: u64,
        data_size: u64,
    ) {
        let mut metrics = self.audio_metrics.write().await;
        let key = format!("{}->{}", from_format, to_format);
        let audio_metrics = metrics.entry(key).or_insert_with(|| {
            AudioConversionMetrics::new(from_format.to_string(), to_format.to_string())
        });
        audio_metrics.update(execution_time_ms, data_size);
    }

    /// 获取所有模型性能指标
    pub async fn get_model_metrics(&self) -> HashMap<String, ModelMetrics> {
        let metrics = self.model_metrics.read().await;
        metrics.clone()
    }

    /// 获取所有音频转换性能指标
    pub async fn get_audio_metrics(&self) -> HashMap<String, AudioConversionMetrics> {
        let metrics = self.audio_metrics.read().await;
        metrics.clone()
    }
}

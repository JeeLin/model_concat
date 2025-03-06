use crate::audio::{AudioCodec, AudioFormat};
use crate::error::{ServiceError, ServiceResult};
use crate::model::ModelParams;
use crate::orchestration::{PipelineFactory, PipelineRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 任务管理模块
///
/// 提供了异步任务的管理功能，包括：
/// - 任务创建和调度
/// - 状态追踪和更新
/// - 结果处理和回调
/// - 错误处理和恢复
///
/// # 示例
///
/// ```rust
/// use crate::server::task::{TaskManager, TaskRequest}
/// use std::sync::Arc;
///
/// async fn handle_task() {
///     let manager = TaskManager::new(Arc::new(PipelineFactory::new()));
///     let request = TaskRequest {
///         task_id: None,
///         pipeline: PipelineRequest::default(),
///         input: InputData::Text("hello".to_string()),
///         callback_url: None,
///     }
///     let task_id = manager.create_task(request).await.unwrap();
///     let status = manager.get_task_status(&task_id).await.unwrap();
/// }
/// ```
/// 任务状态
///
/// 表示任务在不同阶段的状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum TaskStatus {
    /// 等待处理
    Pending,
    /// 处理中
    Processing {
        /// 当前阶段
        stage: String,
        /// 进度 (0-100)
        progress: u8,
    },
    /// 处理完成
    Completed {
        /// 结果
        result: TaskResult,
    },
    /// 处理失败
    Failed {
        /// 错误信息
        error: String,
    },
}

/// 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// 输出数据类型
    pub output_type: String,
    /// 输出数据 (Base64编码)
    pub data: String,
    /// 音频格式 (如果是音频)
    pub format: Option<AudioFormat>,
    /// 处理耗时 (毫秒)
    pub elapsed_ms: u64,
}

/// 任务请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// 任务ID (可选，如不提供则自动生成)
    #[serde(default)]
    pub task_id: Option<String>,
    /// 处理流程
    pub pipeline: PipelineRequest,
    /// 输入数据
    pub input: InputData,
    /// 回调地址 (可选)
    #[serde(default)]
    pub callback_url: Option<String>,
}

/// 输入数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum InputData {
    /// 文本输入
    Text(String),
    /// 音频输入
    Audio {
        /// Base64编码的音频数据
        content: String,
        /// 音频格式
        format: AudioFormat,
    },
}

/// 任务管理器
///
/// 管理所有任务的生命周期和状态
///
/// # 字段
///
/// * `pipeline_factory` - 流水线工厂实例
/// * `tasks` - 任务状态映射表
pub struct TaskManager {
    /// 流水线工厂
    pipeline_factory: Arc<PipelineFactory>,
    /// 任务状态
    tasks: RwLock<HashMap<String, TaskStatus>>,
}

impl TaskManager {
    /// 创建新的任务管理器
    pub fn new(pipeline_factory: Arc<PipelineFactory>) -> Self {
        Self {
            pipeline_factory,
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// 创建任务
    pub async fn create_task(&self, request: TaskRequest) -> ServiceResult<String> {
        // 生成任务ID
        let task_id = request
            .task_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // 初始化任务状态
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id.clone(), TaskStatus::Pending);
        }

        // 启动任务处理
        let pipeline_factory = self.pipeline_factory.clone();
        let task_manager = self.clone();

        tokio::spawn(async move {
            if let Err(e) = task_manager
                .process_task(&task_id, request, pipeline_factory)
                .await
            {
                error!("Task processing error: {}", e);

                // 更新任务状态为失败
                let mut tasks = task_manager.tasks.write().await;
                tasks.insert(
                    task_id,
                    TaskStatus::Failed {
                        error: e.to_string(),
                    },
                );
            }
        });

        Ok(task_id)
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: &str) -> ServiceResult<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| ServiceError::Task(format!("Task not found: {}", task_id)))
    }

    /// 处理任务
    async fn process_task(
        &self,
        task_id: &str,
        request: TaskRequest,
        pipeline_factory: Arc<PipelineFactory>,
    ) -> ServiceResult<()> {
        // 更新任务状态为处理中
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(
                task_id.to_string(),
                TaskStatus::Processing {
                    stage: "initializing".to_string(),
                    progress: 0,
                },
            );
        }

        // 创建流水线
        let pipeline = pipeline_factory.create_pipeline(&request.pipeline).await?;

        // 准备输入数据
        let (input_data, input_format) = match request.input {
            InputData::Text(text) => (text.into_bytes(), None),
            InputData::Audio { content, format } => {
                // 解码Base64
                let decoded = base64::decode(&content).map_err(|e| {
                    ServiceError::InvalidInput(format!("Invalid Base64 data: {}", e))
                })?;

                (decoded, Some(format))
            }
        };

        // 记录开始时间
        let start_time = std::time::Instant::now();

        // 更新任务状态
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(
                task_id.to_string(),
                TaskStatus::Processing {
                    stage: "processing".to_string(),
                    progress: 10,
                },
            );
        }

        // 执行流水线
        let (output_data, output_format) = pipeline.execute(input_data, input_format).await?;

        // 计算处理时间
        let elapsed = start_time.elapsed().as_millis() as u64;

        // 确定输出类型
        let (output_type, encoded_data) = if let Some(format) = &output_format {
            // 音频输出
            let engine = base64::engine::general_purpose::STANDARD;

            // 替换旧版encode调用
            ("audio".to_string(), engine.encode(&output_data))
        } else {
            // 尝试解析为文本
            match String::from_utf8(output_data.clone()) {
                Ok(text) => ("text".to_string(), text),
                Err(_) => ("binary".to_string(), engine.encode(&output_data)),
            }
        };

        // 更新任务状态为完成
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(
                task_id.to_string(),
                TaskStatus::Completed {
                    result: TaskResult {
                        output_type,
                        data: encoded_data,
                        format: output_format,
                        elapsed_ms: elapsed,
                    },
                },
            );
        }

        // 如果有回调地址，发送回调
        if let Some(callback_url) = request.callback_url {
            self.send_callback(task_id, &callback_url).await?;
        }

        Ok(())
    }

    /// 发送回调
    async fn send_callback(&self, task_id: &str, callback_url: &str) -> ServiceResult<()> {
        // 获取任务状态
        let status = self.get_task_status(task_id).await?;

        // 发送HTTP请求
        let client = reqwest::Client::new();
        let response = client
            .post(callback_url)
            .json(&serde_json::json!({
                "task_id": task_id,
                "status": status,
            }))
            .send()
            .await
            .map_err(|e| ServiceError::Server(format!("Callback error: {}", e)))?;

        if !response.status().is_success() {
            warn!("Callback failed with status: {}", response.status());
        }

        Ok(())
    }
}

impl Clone for TaskManager {
    fn clone(&self) -> Self {
        Self {
            pipeline_factory: self.pipeline_factory.clone(),
            tasks: self.tasks.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // 创建测试任务请求
    fn create_test_request() -> TaskRequest {
        TaskRequest {
            task_id: None,
            pipeline: PipelineRequest::default(),
            input: InputData::Text("test input".to_string()),
            callback_url: None,
        }
    }

    #[tokio::test]
    async fn test_create_task() {
        let manager = TaskManager::new(Arc::new(PipelineFactory::new()));
        let request = create_test_request();

        // 创建任务
        let task_id = manager.create_task(request).await.unwrap();
        assert!(!task_id.is_empty());

        // 验证任务状态
        let status = manager.get_task_status(&task_id).await.unwrap();
        match status {
            TaskStatus::Pending | TaskStatus::Processing { .. } => {}
            _ => panic!("Unexpected task status"),
        }
    }

    #[tokio::test]
    async fn test_get_task_status_not_found() {
        let manager = TaskManager::new(Arc::new(PipelineFactory::new()));
        let result = manager.get_task_status("non-existent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_lifecycle() {
        let manager = TaskManager::new(Arc::new(PipelineFactory::new()));
        let request = create_test_request();

        // 创建任务
        let task_id = manager.create_task(request).await.unwrap();

        // 等待任务处理完成
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 检查最终状态
        let status = manager.get_task_status(&task_id).await.unwrap();
        match status {
            TaskStatus::Completed { .. } | TaskStatus::Failed { .. } => {}
            _ => panic!("Task should be completed or failed"),
        }
    }

    #[tokio::test]
    async fn test_task_callback() {
        let manager = TaskManager::new(Arc::new(PipelineFactory::new()));
        let mut request = create_test_request();
        request.callback_url = Some("http://localhost:8080/callback".to_string());

        // 创建带回调的任务
        let task_id = manager.create_task(request).await.unwrap();

        // 等待任务处理
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 验证任务状态
        let status = manager.get_task_status(&task_id).await.unwrap();
        match status {
            TaskStatus::Completed { .. } | TaskStatus::Failed { .. } => {}
            _ => panic!("Task should be completed or failed"),
        }
    }
}

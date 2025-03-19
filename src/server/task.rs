//! 任务管理模块
//!
//! 提供了异步任务的管理功能，包括：
//! - 任务创建和调度
//! - 状态追踪和更新
//! - 结果处理和回调
//! - 错误处理和恢复

use crate::error::{ServiceError, ServiceResult};
use crate::model::DataType;
use crate::orchestration::{PipelineFactory, PipelineRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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
        /// 当前模型
        model: String,
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
    /// 输出数据
    pub output: DataType,
    /// 处理耗时 (毫秒)
    pub elapsed_ms: u64,
    /// 模型状态列表
    pub model_statuses: Vec<crate::server::request::ModelStatus>,
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
    pub input: DataType,
    /// 回调地址 (可选)
    #[serde(default)]
    pub callback_url: Option<String>,
}

/// 任务管理器
///
/// 负责创建、调度和管理异步处理任务
pub struct TaskManager {
    /// 流水线工厂
    pipeline_factory: Arc<PipelineFactory>,
    /// 任务状态映射
    tasks: RwLock<HashMap<String, TaskStatus>>,
}

impl TaskManager {
    /// 创建新的任务管理器
    ///
    /// # 参数
    ///
    /// * `pipeline_factory` - 流水线工厂实例
    pub fn new(pipeline_factory: Arc<PipelineFactory>) -> Self {
        let manager = Self {
            pipeline_factory,
            tasks: RwLock::new(HashMap::new()),
        };

        manager
    }

    /// 创建新任务
    ///
    /// # 参数
    ///
    /// * `request` - 任务请求
    ///
    /// # 返回值
    ///
    /// 返回任务ID
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

        // 启动异步处理任务
        let pipeline_factory = self.pipeline_factory.clone();
        let tasks_clone = self.tasks.clone();
        let results_cache = self.results_cache.clone();
        let task_id_clone = task_id.clone();

        tokio::spawn(async move {
            // 更新状态为处理中
            Self::update_task_status(
                &tasks_clone,
                &task_id_clone,
                TaskStatus::Processing {
                    model: "初始化中".to_string(),
                    progress: 0,
                },
            )
                .await;

            // 创建处理流水线
            let pipeline_result = pipeline_factory.create_pipeline(&request.pipeline).await;

            match pipeline_result {
                Ok(pipeline) => {
                    // 执行流水线处理
                    let start_time = Instant::now();
                    let process_result = pipeline.execute(request.input, None).await;

                    match process_result {
                        Ok((output, _)) => {
                            // 计算处理耗时
                            let elapsed = start_time.elapsed();
                            let elapsed_ms = elapsed.as_millis() as u64;

                            // 创建任务结果
                            let result = TaskResult {
                                output,
                                elapsed_ms,
                                model_statuses: Vec::new(), // 这里需要从pipeline获取模型状态
                            };

                            // 更新任务状态为完成
                            Self::update_task_status(
                                &tasks_clone,
                                &task_id_clone,
                                TaskStatus::Completed {
                                    result: result.clone(),
                                },
                            )
                                .await;

                            // 缓存结果
                            let mut cache = results_cache.write().await;
                            cache.insert(task_id_clone, (result, Instant::now()));

                            // 处理回调（如果有）
                            if let Some(callback_url) = &request.callback_url {
                                Self::send_callback(callback_url, &task_id_clone, true, None).await;
                            }
                        },
                        Err(e) => {
                            // 更新任务状态为失败
                            let error_msg = format!("处理失败: {}", e);
                            Self::update_task_status(
                                &tasks_clone,
                                &task_id_clone,
                                TaskStatus::Failed {
                                    error: error_msg.clone(),
                                },
                            )
                                .await;

                            // 处理回调（如果有）
                            if let Some(callback_url) = &request.callback_url {
                                Self::send_callback(
                                    callback_url,
                                    &task_id_clone,
                                    false,
                                    Some(error_msg),
                                )
                                    .await;
                            }
                        },
                    }
                },
                Err(e) => {
                    // 更新任务状态为失败
                    let error_msg = format!("创建流水线失败: {}", e);
                    Self::update_task_status(
                        &tasks_clone,
                        &task_id_clone,
                        TaskStatus::Failed {
                            error: error_msg.clone(),
                        },
                    )
                        .await;

                    // 处理回调（如果有）
                    if let Some(callback_url) = &request.callback_url {
                        Self::send_callback(callback_url, &task_id_clone, false, Some(error_msg))
                            .await;
                    }
                },
            }
        });

        Ok(task_id)
    }

    /// 获取任务状态
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务ID
    ///
    /// # 返回值
    ///
    /// 返回任务状态，如果任务不存在则返回错误
    pub async fn get_task_status(&self, task_id: &str) -> ServiceResult<TaskStatus> {
        // 检查活动任务
        let tasks = self.tasks.read().await;
        if let Some(status) = tasks.get(task_id) {
            return Ok(status.clone());
        }

        // 任务不存在
        Err(ServiceError::NotFound(format!(
            "任务 {} 不存在或已过期",
            task_id
        )))
    }

    /// 更新任务状态
    ///
    /// # 参数
    ///
    /// * `tasks` - 任务状态映射
    /// * `task_id` - 任务ID
    /// * `status` - 新状态
    async fn update_task_status(
        tasks: &RwLock<HashMap<String, TaskStatus>>,
        task_id: &str,
        status: TaskStatus,
    ) {
        let mut tasks_map = tasks.write().await;
        tasks_map.insert(task_id.to_string(), status);
    }

    /// 发送回调通知
    ///
    /// # 参数
    ///
    /// * `callback_url` - 回调URL
    /// * `task_id` - 任务ID
    /// * `success` - 是否成功
    /// * `error` - 错误信息（如果失败）
    async fn send_callback(
        callback_url: &str,
        task_id: &str,
        success: bool,
        error: Option<String>,
    ) {
        // 构造回调数据
        let callback_data = serde_json::json!({
            "task_id": task_id,
            "success": success,
            "error": error,
        });

        // 发送HTTP POST请求
        match reqwest::Client::new()
            .post(callback_url)
            .json(&callback_data)
            .send()
            .await
        {
            Ok(_) => {
                debug!("回调发送成功: {}", callback_url);
            },
            Err(e) => {
                warn!("回调发送失败: {}, 错误: {}", callback_url, e);
            },
        }
    }

    /// 清理过期的结果缓存
    async fn clean_expired_results(self) -> ! {
        loop {
            // 每分钟检查一次
            time::sleep(Duration::from_secs(60)).await;

            let now = Instant::now();
            let expiry_duration = Duration::from_secs(self.cache_expiry_seconds);

            // 清理过期结果
            let mut cache = self.results_cache.write().await;
            cache.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < expiry_duration);

            debug!("清理过期结果缓存，当前缓存数量: {}", cache.len());
        }
    }
}

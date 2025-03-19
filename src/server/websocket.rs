//! WebSocket服务模块
//!
//! 提供了基于WebSocket的实时通信功能，包括：
//! - 连接管理
//! - 消息处理
//! - 流式数据传输
//! - 错误处理

use crate::model::{DataType, StreamMode};
use crate::orchestration::{PipelineFactory, PipelineRequest};
use crate::server::task::{TaskManager, TaskRequest, TaskStatus};
use axum::response::IntoResponse;
use axum::Extension;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// WebSocket消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// 初始化连接
    Init(WsInitMessage),
    /// 开始处理
    Start(WsStartMessage),
    /// 处理进度
    Progress(WsProgressMessage),
    /// 处理结果
    Result(WsResultMessage),
    /// 错误信息
    Error(WsErrorMessage),
    /// 心跳消息
    Ping,
    /// 心跳响应
    Pong,
}

/// 初始化消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsInitMessage {
    /// 客户端ID
    pub client_id: String,
    /// 认证信息
    pub auth: Option<String>,
}

/// 开始处理消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsStartMessage {
    /// 流水线配置
    pub pipeline: PipelineRequest,
    /// 输入数据
    pub input: DataType,
}

/// 进度消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsProgressMessage {
    /// 当前处理阶段
    pub stage: String,
    /// 处理进度（0-100）
    pub progress: u8,
    /// 中间结果（如有）
    pub intermediate_result: Option<String>,
}

/// 结果消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsResultMessage {
    /// 任务ID
    pub task_id: String,
    /// 输出数据
    pub output: DataType,
    /// 处理耗时（毫秒）
    pub elapsed_ms: u64,
}

/// 错误消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsErrorMessage {
    /// 错误代码
    pub code: String,
    /// 错误消息
    pub message: String,
}

/// WebSocket连接处理函数
///
/// 处理WebSocket连接请求，建立连接并处理消息
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(task_manager): Extension<Arc<TaskManager>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, task_manager))
}

/// 处理WebSocket连接
async fn handle_socket(socket: WebSocket, task_manager: Arc<TaskManager>) {
    // 分离WebSocket的发送和接收部分
    let (mut sender, mut receiver) = socket.split();

    // 创建消息通道
    let (tx, mut rx) = mpsc::channel::<Message>(100);

    // 客户端ID
    let mut client_id = String::new();

    // 最后活动时间
    let mut last_activity = Instant::now();

    // 心跳检查间隔
    let heartbeat_interval = Duration::from_secs(30);

    // 启动发送任务
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    // 处理接收到的消息
    while let Some(result) = receiver.next().await {
        last_activity = Instant::now();

        match result {
            Ok(message) => {
                match message {
                    Message::Text(text) => {
                        // 解析消息
                        match serde_json::from_str::<WsMessage>(&text) {
                            Ok(ws_message) => {
                                match ws_message {
                                    WsMessage::Init(init) => {
                                        // 处理初始化消息
                                        client_id = init.client_id;
                                        info!("WebSocket客户端连接: {}", client_id);

                                        // 发送确认消息
                                        let response = WsMessage::Pong;
                                        if let Ok(response_text) = serde_json::to_string(&response)
                                        {
                                            let _ = tx.send(Message::Text(response_text)).await;
                                        }
                                    },
                                    WsMessage::Start(start) => {
                                        // 处理开始消息
                                        info!("接收处理请求: {}", client_id);

                                        // 创建任务
                                        let task_request = TaskRequest {
                                            task_id: None,
                                            pipeline: start.pipeline,
                                            input: start.input,
                                            callback_url: None,
                                        };

                                        // 克隆发送通道
                                        let tx_clone = tx.clone();
                                        let task_manager_clone = task_manager.clone();

                                        // 启动异步处理任务
                                        tokio::spawn(async move {
                                            match task_manager_clone.create_task(task_request).await
                                            {
                                                Ok(task_id) => {
                                                    // 发送进度更新
                                                    let mut last_status = None;
                                                    let start_time = Instant::now();

                                                    // 定期检查任务状态
                                                    loop {
                                                        match task_manager_clone
                                                            .get_task_status(&task_id)
                                                            .await
                                                        {
                                                            Ok(status) => {
                                                                match &status {
                                                                    TaskStatus::Pending => {
                                                                        // 任务等待中
                                                                        if last_status.is_none() {
                                                                            let progress = WsMessage::Progress(WsProgressMessage {
                                                                                stage: "等待处理".to_string(),
                                                                                progress: 0,
                                                                                intermediate_result: None,
                                                                            });

                                                                            if let Ok(text) = serde_json::to_string(&progress) {
                                                                                let _ = tx_clone.send(Message::Text(text)).await;
                                                                            }

                                                                            last_status = Some(
                                                                                status.clone(),
                                                                            );
                                                                        }
                                                                    },
                                                                    TaskStatus::Processing {
                                                                        model,
                                                                        progress,
                                                                    } => {
                                                                        // 任务处理中
                                                                        let progress_msg = WsMessage::Progress(WsProgressMessage {
                                                                            stage: model.clone(),
                                                                            progress: *progress,
                                                                            intermediate_result: None,
                                                                        });

                                                                        if let Ok(text) =
                                                                            serde_json::to_string(
                                                                                &progress_msg,
                                                                            )
                                                                        {
                                                                            let _ = tx_clone
                                                                                .send(
                                                                                    Message::Text(
                                                                                        text,
                                                                                    ),
                                                                                )
                                                                                .await;
                                                                        }

                                                                        last_status =
                                                                            Some(status.clone());
                                                                    },
                                                                    TaskStatus::Completed {
                                                                        result,
                                                                    } => {
                                                                        // 任务完成
                                                                        let elapsed =
                                                                            start_time.elapsed();
                                                                        let elapsed_ms = elapsed
                                                                            .as_millis()
                                                                            as u64;

                                                                        let result_msg =
                                                                            WsMessage::Result(
                                                                                WsResultMessage {
                                                                                    task_id:
                                                                                    task_id
                                                                                        .clone(),
                                                                                    output: result
                                                                                        .output
                                                                                        .clone(),
                                                                                    elapsed_ms,
                                                                                },
                                                                            );

                                                                        if let Ok(text) =
                                                                            serde_json::to_string(
                                                                                &result_msg,
                                                                            )
                                                                        {
                                                                            let _ = tx_clone
                                                                                .send(
                                                                                    Message::Text(
                                                                                        text,
                                                                                    ),
                                                                                )
                                                                                .await;
                                                                        }

                                                                        break;
                                                                    },
                                                                    TaskStatus::Failed {
                                                                        error,
                                                                    } => {
                                                                        // 任务失败
                                                                        let error_msg = WsMessage::Error(WsErrorMessage {
                                                                            code: "TASK_FAILED".to_string(),
                                                                            message: error.clone(),
                                                                        });

                                                                        if let Ok(text) =
                                                                            serde_json::to_string(
                                                                                &error_msg,
                                                                            )
                                                                        {
                                                                            let _ = tx_clone
                                                                                .send(
                                                                                    Message::Text(
                                                                                        text,
                                                                                    ),
                                                                                )
                                                                                .await;
                                                                        }

                                                                        break;
                                                                    },
                                                                }
                                                            },
                                                            Err(e) => {
                                                                // 获取任务状态失败
                                                                let error_msg = WsMessage::Error(
                                                                    WsErrorMessage {
                                                                        code: "STATUS_ERROR"
                                                                            .to_string(),
                                                                        message: format!(
                                                                            "获取任务状态失败: {}",
                                                                            e
                                                                        ),
                                                                    },
                                                                );

                                                                if let Ok(text) =
                                                                    serde_json::to_string(
                                                                        &error_msg,
                                                                    )
                                                                {
                                                                    let _ = tx_clone
                                                                        .send(Message::Text(text))
                                                                        .await;
                                                                }

                                                                break;
                                                            },
                                                        }

                                                        // 等待一段时间再检查
                                                        tokio::time::sleep(Duration::from_millis(
                                                            500,
                                                        ))
                                                            .await;
                                                    }
                                                },
                                                Err(e) => {
                                                    // 创建任务失败
                                                    let error_msg =
                                                        WsMessage::Error(WsErrorMessage {
                                                            code: "TASK_CREATE_ERROR".to_string(),
                                                            message: format!("创建任务失败: {}", e),
                                                        });

                                                    if let Ok(text) =
                                                        serde_json::to_string(&error_msg)
                                                    {
                                                        let _ = tx_clone
                                                            .send(Message::Text(text))
                                                            .await;
                                                    }
                                                },
                                            }
                                        });
                                    },
                                    WsMessage::Ping => {
                                        // 处理心跳消息
                                        let pong = WsMessage::Pong;
                                        if let Ok(text) = serde_json::to_string(&pong) {
                                            let _ = tx.send(Message::Text(text)).await;
                                        }
                                    },
                                    _ => {
                                        // 忽略其他类型的消息
                                        warn!("收到未处理的WebSocket消息类型");
                                    },
                                }
                            },
                            Err(e) => {
                                // 消息解析失败
                                error!("WebSocket消息解析失败: {}", e);
                                let error_msg = WsMessage::Error(WsErrorMessage {
                                    code: "PARSE_ERROR".to_string(),
                                    message: format!("消息解析失败: {}", e),
                                });

                                if let Ok(text) = serde_json::to_string(&error_msg) {
                                    let _ = tx.send(Message::Text(text)).await;
                                }
                            },
                        }
                    },
                    Message::Binary(_) => {
                        // 暂不处理二进制消息
                        warn!("收到二进制WebSocket消息，暂不支持");
                    },
                    Message::Ping(_) => {
                        // 响应Ping消息
                        let _ = tx.send(Message::Pong(vec![])).await;
                    },
                    Message::Pong(_) => {
                        // 忽略Pong消息
                    },
                    Message::Close(_) => {
                        // 关闭连接
                        break;
                    },
                }
            },
            Err(e) => {
                // WebSocket错误
                error!("WebSocket错误: {}", e);
                break;
            },
        }

        // 检查是否需要发送心跳
        if last_activity.elapsed() > heartbeat_interval {
            // 发送Ping消息
            let ping = WsMessage::Ping;
            if let Ok(text) = serde_json::to_string(&ping) {
                if tx.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }

            last_activity = Instant::now();
        }
    }

    // 连接关闭，取消发送任务
    send_task.abort();
    info!("WebSocket连接关闭: {}", client_id);
}

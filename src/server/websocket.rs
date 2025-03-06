use crate::model::DataType;
/// WebSocket服务器模块
///
/// 提供了基于WebSocket的实时通信功能，包括：
/// - 连接管理
/// - 消息处理
/// - 心跳检测
/// - 错误处理
///
/// # 示例
///
/// ```rust
/// use axum::extract::ws::WebSocketUpgrade;
/// use crate::server::websocket::ws_handler;
///
/// async fn handle_ws(ws: WebSocketUpgrade) {
///     ws.on_upgrade(|socket| handle_socket(socket));
/// }
/// ```
use crate::orchestration::PipelineFactory;
use crate::server::request::InputData;
use crate::server::task::TaskResult;
use axum::Extension;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::WebSocketUpgrade,
    response::IntoResponse,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// WebSocket 消息类型
///
/// 定义了所有支持的WebSocket消息格式
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
pub struct WsInitMessage {
    /// 客户端ID
    pub client_id: String,
    /// 认证信息
    pub auth: Option<String>,
}

/// 开始处理消息
#[derive(Debug, Serialize, Deserialize)]
pub struct WsStartMessage {
    /// 处理配置
    pub pipeline: PipelineConfig,
    /// 输入数据
    pub input: InputData,
}

/// 进度消息
#[derive(Debug, Serialize, Deserialize)]
pub struct WsProgressMessage {
    /// 当前处理阶段
    pub stage: String,
    /// 处理进度（0-100）
    pub progress: u8,
    /// 中间结果（如有）
    pub intermediate_result: Option<String>,
}

/// 结果消息
#[derive(Debug, Serialize, Deserialize)]
pub struct WsResultMessage {
    /// 处理结果
    pub result: TaskResult,
}

/// 错误消息
#[derive(Debug, Serialize, Deserialize)]
pub struct WsErrorMessage {
    /// 错误代码
    pub code: String,
    /// 错误信息
    pub message: String,
}

/// WebSocket 会话
///
/// 管理单个WebSocket连接的生命周期和状态
///
/// # 字段
///
/// * `client_id` - 客户端唯一标识
/// * `heartbeat_interval` - 心跳检查时间间隔
/// * `last_heartbeat` - 上次心跳时间
/// * `pipeline_factory` - 流水线工厂实例
pub struct WsSession {
    /// 客户端ID
    client_id: Option<String>,
    /// 心跳检查间隔
    heartbeat_interval: Duration,
    /// 上次心跳时间
    last_heartbeat: Instant,
    /// 流水线工厂
    pipeline_factory: Arc<PipelineFactory>,
}

impl WsSession {
    /// 创建新的WebSocket会话
    ///
    /// # 参数
    ///
    /// * `pipeline_factory` - 流水线工厂实例
    ///
    /// # 返回值
    ///
    /// 返回新创建的WebSocket会话实例
    pub fn new(pipeline_factory: Arc<PipelineFactory>) -> Self {
        Self {
            client_id: None,
            heartbeat_interval: Duration::from_secs(30),
            last_heartbeat: Instant::now(),
            pipeline_factory,
        }
    }

    /// 处理WebSocket连接
    pub async fn handle_socket(self, socket: WebSocket) {
        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::channel::<Message>(100);

        // 心跳检查任务
        let heartbeat_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if heartbeat_tx.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        });

        // 消息发送任务
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if sender.send(message).await.is_err() {
                    break;
                }
            }
        });

        // 消息接收处理
        while let Some(result) = receiver.next().await {
            match result {
                Ok(Message::Text(text)) => {
                    if let Ok(message) = serde_json::from_str::<WsMessage>(&text) {
                        self.handle_message(message, tx.clone()).await;
                    }
                },
                Ok(Message::Binary(bin)) => {
                    self.handle_binary(bin, tx.clone()).await;
                },
                Ok(Message::Ping(_)) => {
                    if tx.send(Message::Pong(vec![])).await.is_err() {
                        break;
                    }
                },
                Ok(Message::Close(_)) => break,
                _ => {},
            }
        }
    }

    /// 处理WebSocket消息
    async fn handle_message(&self, message: WsMessage, tx: mpsc::Sender<Message>) {
        match message {
            WsMessage::Init(init) => {
                // 处理初始化消息
                info!(
                    "Initialized WebSocket connection with client: {}",
                    init.client_id
                );

                // 发送确认消息
                let response = WsMessage::Init(WsInitMessage {
                    client_id: init.client_id,
                    auth: None,
                });
                self.send_message(response, tx).await;
            },
            WsMessage::Start(start) => {
                // 处理开始处理消息
                info!("Starting pipeline processing");
                // 这里应该实现实际的处理逻辑
            },
            WsMessage::Ping => {
                self.send_message(WsMessage::Pong, tx).await;
            },
            _ => {},
        }
    }

    /// 处理二进制消息
    async fn handle_binary(&self, data: Vec<u8>, tx: mpsc::Sender<Message>) {
        // 处理二进制数据，例如音频
        debug!("Received binary data: {} bytes", data.len());
    }

    /// 发送WebSocket消息
    async fn send_message(&self, message: WsMessage, tx: mpsc::Sender<Message>) {
        if let Ok(json) = serde_json::to_string(&message) {
            let _ = tx.send(Message::Text(json)).await;
        }
    }
}

/// WebSocket处理函数
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(pipeline_factory): Extension<Arc<PipelineFactory>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, pipeline_factory))
}

async fn handle_socket(mut socket: WebSocket, pipeline_factory: Arc<PipelineFactory>) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    let input = DataType::Text(text);
                    match pipeline_factory.execute(input).await {
                        Ok(output) => {
                            let response = match output {
                                DataType::Text(text) => text,
                                DataType::Audio { data, .. } => STANDARD.encode(data),
                            };
                            if let Err(e) = socket.send(Message::Text(response)).await {
                                debug!("Failed to send response: {}", e);
                                break;
                            }
                        },
                        Err(e) => {
                            if let Err(e) = socket.send(Message::Text(e.to_string())).await {
                                debug!("Failed to send error: {}", e);
                                break;
                            }
                        },
                    }
                },
                Message::Binary(bin) => {
                    let input = DataType::Audio {
                        data: bin,
                        format: Default::default(),
                    };
                    match pipeline_factory.execute(input).await {
                        Ok(output) => {
                            let response = match output {
                                DataType::Text(text) => Message::Text(text),
                                DataType::Audio { data, .. } => Message::Binary(data),
                            };
                            if let Err(e) = socket.send(response).await {
                                debug!("Failed to send response: {}", e);
                                break;
                            }
                        },
                        Err(e) => {
                            if let Err(e) = socket.send(Message::Text(e.to_string())).await {
                                debug!("Failed to send error: {}", e);
                                break;
                            }
                        },
                    }
                },
                Message::Close(_) => break,
                _ => {},
            }
        } else {
            break;
        }
    }
    info!("WebSocket connection closed");
}

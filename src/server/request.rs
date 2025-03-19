//! 请求处理模块
//!
//! 定义了处理请求和响应的数据结构，包括：
//! - 处理请求参数
//! - 处理响应结果
//! - 处理状态信息
//! - 输入输出数据格式

use crate::model::DataType;
use crate::orchestration::{ModelRequest, PipelineRequest};
use serde::{Deserialize, Serialize};

/// 处理请求
///
/// 包含了处理所需的所有参数信息
///
/// # 字段
///
/// * `process_id` - 处理任务的唯一标识
/// * `input` - 输入数据
/// * `pipeline` - 流水线配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRequest {
    #[serde(default = "generate_process_id")]
    pub process_id: String,
    pub input: DataType,
    pub pipeline: PipelineRequest,
}

/// 生成唯一的处理ID
fn generate_process_id() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

/// 处理响应
///
/// 包含了处理结果和状态信息
///
/// # 字段
///
/// * `process_id` - 处理任务的唯一标识
/// * `output` - 输出数据
/// * `elapsed_ms` - 处理耗时（毫秒）
/// * `status` - 处理状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResponse {
    pub process_id: String,
    pub output: Option<DataType>,
    pub elapsed_ms: u64,
    pub status: ProcessStatus,
}

/// 处理状态
///
/// 记录了处理过程的状态信息
///
/// # 字段
///
/// * `success` - 是否处理成功
/// * `error` - 错误信息（如果失败）
/// * `models` - 各模型的状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStatus {
    pub success: bool,
    pub error: Option<String>,
    pub models: Vec<ModelStatus>,
}

/// 模型状态
///
/// 记录了单个模型的处理状态信息
///
/// # 字段
///
/// * `provider` - 模型提供者
/// * `model_id` - 模型ID
/// * `success` - 是否处理成功
/// * `error` - 错误信息（如果失败）
/// * `elapsed_ms` - 处理耗时（毫秒）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub provider: String,
    pub model_id: String,
    pub success: bool,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

/// 创建流水线请求
///
/// 用于创建新的流水线配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePipelineRequest {
    pub name: String,
    pub models: Vec<ModelRequest>,
    pub merge_strategy: Option<crate::orchestration::MergeStrategy>,
}

/// 创建流水线响应
///
/// 包含创建的流水线信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePipelineResponse {
    pub pipeline_id: String,
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
}

//! 编排类型定义
//!
//! 本模块定义了编排系统中使用的数据类型和接口，包括数据格式和格式转换器。

use crate::audio::format::AudioFormat;
use crate::error::ServiceResult;
use crate::model::ModelParams;
use crate::text::converter::TextFormat;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 数据格式
///
/// 表示模型可以处理的数据格式，包括音频和文本。
#[derive(Debug, Clone)]
pub enum DataFormat {
    /// 音频格式
    Audio(AudioFormat),
    /// 文本格式
    Text(TextFormat),
}

impl DataFormat {
    /// 获取音频格式（如果是音频类型）
    pub fn as_audio(&self) -> Option<&AudioFormat> {
        match self {
            DataFormat::Audio(format) => Some(format),
            _ => None,
        }
    }

    /// 获取文本格式（如果是文本类型）
    pub fn as_text(&self) -> Option<&TextFormat> {
        match self {
            DataFormat::Text(format) => Some(format),
            _ => None,
        }
    }

    /// 是否是音频格式
    pub fn is_audio(&self) -> bool {
        matches!(self, DataFormat::Audio(_))
    }

    /// 是否是文本格式
    pub fn is_text(&self) -> bool {
        matches!(self, DataFormat::Text(_))
    }
}

/// 格式转换器接口
///
/// 定义了数据格式转换的通用接口，支持不同类型数据之间的转换。
#[async_trait]
pub trait FormatConverter: Send + Sync {
    /// 获取转换器名称
    fn name(&self) -> &str;

    /// 检查是否支持指定的源格式和目标格式
    fn supports(&self, source: &DataFormat, target: &DataFormat) -> bool;

    /// 执行格式转换
    async fn convert(
        &self,
        data: &[u8],
        source: &DataFormat,
        target: &DataFormat,
    ) -> ServiceResult<Vec<u8>>;
}

/// 结果合并策略
///
/// 定义了如何合并多个模型的处理结果。
/// 在并行处理时，需要选择一个合适的策略来合并不同模型的输出。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// 使用第一个结果
    First,
    /// 使用最后一个结果
    Last,
    /// 使用最大长度的结果
    Longest,
    /// 使用最小长度的结果
    Shortest,
    /// 合并所有结果（适用于文本）
    Concat,
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::First
    }
}

/// 模型配置
///
/// 定义了流水线中使用的模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型提供者
    pub provider: String,
    /// 模型ID
    pub model_id: String,
    /// 模型参数
    #[serde(default)]
    pub parameters: ModelParams,
}

/// 阶段配置
///
/// 定义了流水线中的一个处理阶段，包含一组并行执行的模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageConfig {
    /// 阶段名称
    pub name: String,
    /// 阶段中的模型列表
    pub models: Vec<ModelConfig>,
    /// 结果合并策略
    #[serde(default)]
    pub merge_strategy: MergeStrategy,
}

/// 流水线配置
///
/// 定义了完整的处理流水线配置，包含多个顺序执行的阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// 流水线名称
    pub name: String,
    /// 流水线中的阶段列表
    pub stages: Vec<StageConfig>,
}

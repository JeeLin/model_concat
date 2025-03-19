//! HTTP请求处理模块
//!
//! 提供了处理HTTP请求的核心功能，包括：
//! - 服务商和模型信息查询
//! - 监控数据查询
//! - 错误处理和响应格式化

use crate::error::{ServiceError, ServiceResult};
use crate::metrics::MetricsManager;
use crate::model::factory::ModelFactory;
use crate::server::task::{TaskManager, TaskStatus};
use axum::extract::{Json, Path};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

/// 服务商信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersResponse {
    /// 服务商列表
    pub providers: Vec<String>,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 模型信息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    /// 服务商名称
    pub provider: String,
    /// 模型列表
    pub models: Vec<String>,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 模型详情响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDetailResponse {
    /// 服务商名称
    pub provider: String,
    /// 模型ID
    pub model_id: String,
    /// 支持的输入格式
    pub input_formats: Vec<crate::model::SupportedFormat>,
    /// 支持的输出格式
    pub output_formats: Vec<crate::model::SupportedFormat>,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 监控数据响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    /// 模型性能指标
    pub model_metrics: HashMap<String, crate::metrics::ModelMetrics>,
    /// 音频转换性能指标
    pub audio_metrics: HashMap<String, crate::metrics::AudioConversionMetrics>,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// 获取所有服务商
///
/// 返回系统支持的所有模型服务商列表
///
/// # 参数
///
/// * `model_factory` - 模型工厂
pub async fn get_providers(Extension(model_factory): Extension<Arc<ModelFactory>>) -> Response {
    info!("查询服务商列表");

    // 获取所有服务商名称
    let providers = model_factory.provider_names();

    // 构造响应
    let response = ProvidersResponse {
        providers,
        success: true,
        error: None,
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// 获取服务商支持的模型
///
/// 返回指定服务商支持的所有模型列表
///
/// # 参数
///
/// * `model_factory` - 模型工厂
/// * `provider` - 服务商名称
pub async fn get_models(
    Extension(model_factory): Extension<Arc<ModelFactory>>,
    Path(provider): Path<String>,
) -> Response {
    info!("查询服务商模型列表: {}", provider);

    // 获取服务商支持的模型列表
    match model_factory.supported_models(&provider) {
        Ok(models) => {
            // 构造成功响应
            let response = ModelsResponse {
                provider,
                models,
                success: true,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        },
        Err(e) => {
            // 构造错误响应
            error!("获取服务商模型列表失败: {}", e);
            let response = ModelsResponse {
                provider,
                models: vec![],
                success: false,
                error: Some(format!("获取服务商模型列表失败: {}", e)),
            };
            (StatusCode::NOT_FOUND, Json(response)).into_response()
        },
    }
}

/// 获取模型详情
///
/// 返回指定模型的详细信息，包括支持的输入输出格式等
///
/// # 参数
///
/// * `model_factory` - 模型工厂
/// * `provider_and_model` - 服务商名称和模型ID (格式: provider/model_id)
pub async fn get_model_detail(
    Extension(model_factory): Extension<Arc<ModelFactory>>,
    Path(provider_and_model): Path<String>,
) -> Response {
    // 解析路径参数
    let parts: Vec<&str> = provider_and_model.split('/').collect();
    if parts.len() != 2 {
        return (
            StatusCode::BAD_REQUEST,
            Json("无效的路径格式，应为 'provider/model_id'"),
        )
            .into_response();
    }

    let provider = parts[0];
    let model_id = parts[1];

    info!("查询模型详情: {}/{}", provider, model_id);

    // 创建模型实例以获取元数据
    let model_params = crate::model::ModelParams::default();
    let model_request = crate::model::factory::ModelRequest {
        provider: provider.to_string(),
        model_id: model_id.to_string(),
        parameters: model_params,
    };

    match model_factory.create_model(&model_request).await {
        Ok(model) => {
            // 获取模型元数据
            let metadata = model.metadata();

            // 构造成功响应
            let response = ModelDetailResponse {
                provider: provider.to_string(),
                model_id: model_id.to_string(),
                input_formats: metadata.input_formats,
                output_formats: metadata.output_formats,
                success: true,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        },
        Err(e) => {
            // 构造错误响应
            error!("获取模型详情失败: {}", e);
            let response = ModelDetailResponse {
                provider: provider.to_string(),
                model_id: model_id.to_string(),
                input_formats: vec![],
                output_formats: vec![],
                success: false,
                error: Some(format!("获取模型详情失败: {}", e)),
            };
            (StatusCode::NOT_FOUND, Json(response)).into_response()
        },
    }
}

/// 获取监控数据
///
/// 返回系统的性能监控数据，包括模型调用和音频转换的性能指标
///
/// # 参数
///
/// * `metrics_manager` - 监控指标管理器
pub async fn get_metrics(Extension(metrics_manager): Extension<Arc<MetricsManager>>) -> Response {
    info!("查询监控数据");

    // 获取模型性能指标
    let model_metrics = metrics_manager.get_model_metrics().await;

    // 获取音频转换性能指标
    let audio_metrics = metrics_manager.get_audio_metrics().await;

    // 构造响应
    let response = MetricsResponse {
        model_metrics,
        audio_metrics,
        success: true,
        error: None,
    };

    (StatusCode::OK, Json(response)).into_response()
}

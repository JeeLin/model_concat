use crate::orchestration::PipelineFactory;
use crate::server::request::{OutputData, ProcessRequest, ProcessResponse, ProcessStatus};
use axum::http::StatusCode;
use axum::{extract::Path, response::IntoResponse, Extension, Json};
use std::sync::Arc;
use tracing::info;

/// HTTP请求处理模块
///
/// 提供了处理HTTP请求的核心功能，包括：
/// - 请求验证和参数解析
/// - 流水线处理调用
/// - 错误处理和响应格式化
///
/// # 示例
///
/// ```rust
/// use axum::Extension;
/// use std::sync::Arc;
/// use crate::orchestration::PipelineFactory;
/// use crate::server::http::process_request;
///
/// async fn handle_request() {
///     let factory = Arc::new(PipelineFactory::new());
///     let request = ProcessRequest {
///         process_id: "test-123".to_string(),
///         input: InputData::Text("hello".to_string()),
///         stages: vec![]
///     };
///     
///     let response = process_request(
///         Extension(factory),
///         Json(request)
///     ).await;
/// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DataType;
    use crate::orchestration::StageRequest;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // 创建测试请求
    async fn create_test_request() -> ProcessRequest {
        ProcessRequest {
            process_id: "test-123".to_string(),
            input: DataType::Text("test input".to_string()),
            stages: vec![StageRequest {
                name: "test_stage".to_string(),
                model: None,
                params: None,
            }],
        }
    }

    #[tokio::test]
    async fn test_process_request_success() {
        // 创建测试依赖
        let factory = Arc::new(PipelineFactory::new());
        let request = create_test_request().await;

        // 执行请求处理
        let response = process_request(Extension(factory), Json(request.clone()))
            .await
            .into_response();

        // 验证响应状态码
        assert_eq!(response.status(), StatusCode::OK);

        // 解析响应体
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let response: ProcessResponse = serde_json::from_slice(&body_bytes).unwrap();

        // 验证响应内容
        assert_eq!(response.process_id, request.process_id);
        assert!(response.status.success);
        assert!(response.status.error.is_none());
    }

    #[tokio::test]
    async fn test_process_request_invalid_pipeline() {
        // 创建无效的请求
        let mut request = create_test_request().await;
        request.stages[0].name = "invalid_stage".to_string();

        // 执行请求处理
        let factory = Arc::new(PipelineFactory::new());
        let response = process_request(Extension(factory), Json(request.clone()))
            .await
            .into_response();

        // 验证错误响应
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // 解析响应体
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let response: ProcessResponse = serde_json::from_slice(&body_bytes).unwrap();

        // 验证错误信息
        assert!(!response.status.success);
        assert!(response.status.error.is_some());
    }

    #[tokio::test]
    async fn test_process_request_empty_stages() {
        // 创建空stages的请求
        let mut request = create_test_request().await;
        request.stages.clear();

        // 执行请求处理
        let factory = Arc::new(PipelineFactory::new());
        let response = process_request(Extension(factory), Json(request.clone()))
            .await
            .into_response();

        // 验证错误响应
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // 解析响应体
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let response: ProcessResponse = serde_json::from_slice(&body_bytes).unwrap();

        // 验证错误信息
        assert!(!response.status.success);
        assert!(response.status.error.is_some());
    }
}
/// ```

/// 处理HTTP请求
///
/// 接收处理请求，创建处理流水线并执行处理流程。
///
/// # 参数
///
/// * `pipeline_factory` - 流水线工厂实例
/// * `req` - 处理请求参数
///
/// # 返回值
///
/// 返回处理响应，包含处理结果和状态信息
pub async fn process_request(
    Extension(pipeline_factory): Extension<Arc<PipelineFactory>>,
    Json(req): Json<ProcessRequest>,
) -> impl IntoResponse {
    info!("Received process request: {}", req.process_id);

    // 创建处理流水线
    let pipeline = match pipeline_factory.create_pipeline(&req).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProcessResponse {
                    process_id: req.process_id,
                    output: OutputData::Text(e.to_string()),
                    elapsed_ms: 0,
                    status: ProcessStatus {
                        success: false,
                        error: Some(e.to_string()),
                        stages: vec![],
                    },
                }),
            )
                .into_response();
        }
    };

    // 执行处理流程
    let start_time = std::time::Instant::now();
    match pipeline.execute(req.input).await {
        Ok(output) => {
            let elapsed = start_time.elapsed().as_millis() as u64;
            (
                StatusCode::OK,
                Json(ProcessResponse {
                    process_id: req.process_id,
                    output,
                    elapsed_ms: elapsed,
                    status: ProcessStatus {
                        success: true,
                        error: None,
                        stages: vec![],
                    },
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ProcessResponse {
                process_id: req.process_id,
                output: OutputData::Text(e.to_string()),
                elapsed_ms: start_time.elapsed().as_millis() as u64,
                status: ProcessStatus {
                    success: false,
                    error: Some(e.to_string()),
                    stages: vec![],
                },
            }),
        )
            .into_response(),
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::model::DataType;
        use crate::orchestration::StageRequest;
        use axum::http::StatusCode;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        // 创建测试请求
        async fn create_test_request() -> ProcessRequest {
            ProcessRequest {
                process_id: "test-123".to_string(),
                input: DataType::Text("test input".to_string()),
                stages: vec![StageRequest {
                    name: "test_stage".to_string(),
                    model: None,
                    params: None,
                }],
            }
        }

        #[tokio::test]
        async fn test_process_request_success() {
            // 创建测试依赖
            let factory = Arc::new(PipelineFactory::new());
            let request = create_test_request().await;

            // 执行请求处理
            let response = process_request(Extension(factory), Json(request.clone()))
                .await
                .into_response();

            // 验证响应状态码
            assert_eq!(response.status(), StatusCode::OK);

            // 解析响应体
            let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
            let response: ProcessResponse = serde_json::from_slice(&body_bytes).unwrap();

            // 验证响应内容
            assert_eq!(response.process_id, request.process_id);
            assert!(response.status.success);
            assert!(response.status.error.is_none());
        }

        #[tokio::test]
        async fn test_process_request_invalid_pipeline() {
            // 创建无效的请求
            let mut request = create_test_request().await;
            request.stages[0].name = "invalid_stage".to_string();

            // 执行请求处理
            let factory = Arc::new(PipelineFactory::new());
            let response = process_request(Extension(factory), Json(request.clone()))
                .await
                .into_response();

            // 验证错误响应
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

            // 解析响应体
            let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
            let response: ProcessResponse = serde_json::from_slice(&body_bytes).unwrap();

            // 验证错误信息
            assert!(!response.status.success);
            assert!(response.status.error.is_some());
        }

        #[tokio::test]
        async fn test_process_request_empty_stages() {
            // 创建空stages的请求
            let mut request = create_test_request().await;
            request.stages.clear();

            // 执行请求处理
            let factory = Arc::new(PipelineFactory::new());
            let response = process_request(Extension(factory), Json(request.clone()))
                .await
                .into_response();

            // 验证错误响应
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

            // 解析响应体
            let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
            let response: ProcessResponse = serde_json::from_slice(&body_bytes).unwrap();

            // 验证错误信息
            assert!(!response.status.success);
            assert!(response.status.error.is_some());
        }
    }
}

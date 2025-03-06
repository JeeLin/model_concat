use crate::model::DataType;
use crate::orchestration::StageRequest;
use serde::{Deserialize, Serialize};

/// 请求处理模块
///
/// 定义了处理请求和响应的数据结构，包括：
/// - 处理请求参数
/// - 处理响应结果
/// - 处理状态信息
/// - 输入输出数据格式
///
/// # 示例
///
/// ```rust
/// use crate::server::request::{ProcessRequest, ProcessResponse};
/// use crate::model::DataType;
///
/// // 创建处理请求
/// let request = ProcessRequest {
///     process_id: "test-123".to_string(),
///     input: DataType::Text("hello".to_string()),
///     stages: vec![]
/// };
/// ```

/// 处理请求
///
/// 包含了处理所需的所有参数信息
///
/// # 字段
///
/// * `process_id` - 处理任务的唯一标识
/// * `input` - 输入数据
/// * `stages` - 处理阶段配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRequest {
    pub process_id: String,
    pub input: DataType,
    pub stages: Vec<StageRequest>,
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
    pub output: OutputData,
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
/// * `stages` - 各阶段的状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStatus {
    pub success: bool,
    pub error: Option<String>,
    pub stages: Vec<StageStatus>,
}

/// 阶段状态
///
/// 记录了单个处理阶段的状态信息
///
/// # 字段
///
/// * `name` - 阶段名称
/// * `success` - 是否处理成功
/// * `error` - 错误信息（如果失败）
/// * `elapsed_ms` - 阶段耗时（毫秒）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageStatus {
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

/// 输出数据类型
///
/// 支持文本和音频两种输出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputData {
    /// 文本输出
    Text(String),
    /// 音频输出（二进制数据）
    Audio(Vec<u8>),
}

/// 输入数据类型
///
/// 支持文本和音频两种输入格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputData {
    /// 文本输入
    Text(String),
    /// 音频输入（二进制数据）
    Audio(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_request() {
        let request = ProcessRequest {
            process_id: "test-123".to_string(),
            input: DataType::Text("test input".to_string()),
            stages: vec![],
        };

        assert_eq!(request.process_id, "test-123");
        if let DataType::Text(text) = request.input {
            assert_eq!(text, "test input");
        } else {
            panic!("Expected text input");
        }
        assert!(request.stages.is_empty());
    }

    #[test]
    fn test_process_response() {
        let response = ProcessResponse {
            process_id: "test-123".to_string(),
            output: OutputData::Text("test output".to_string()),
            elapsed_ms: 100,
            status: ProcessStatus {
                success: true,
                error: None,
                stages: vec![],
            },
        };

        assert_eq!(response.process_id, "test-123");
        if let OutputData::Text(text) = response.output {
            assert_eq!(text, "test output");
        } else {
            panic!("Expected text output");
        }
        assert_eq!(response.elapsed_ms, 100);
        assert!(response.status.success);
        assert!(response.status.error.is_none());
    }

    #[test]
    fn test_stage_status() {
        let status = StageStatus {
            name: "test_stage".to_string(),
            success: true,
            error: None,
            elapsed_ms: 50,
        };

        assert_eq!(status.name, "test_stage");
        assert!(status.success);
        assert!(status.error.is_none());
        assert_eq!(status.elapsed_ms, 50);
    }

    #[test]
    fn test_input_data() {
        let text_input = InputData::Text("test input".to_string());
        let audio_input = InputData::Audio(vec![1, 2, 3, 4]);

        if let InputData::Text(text) = text_input {
            assert_eq!(text, "test input");
        } else {
            panic!("Expected text input");
        }

        if let InputData::Audio(data) = audio_input {
            assert_eq!(data, vec![1, 2, 3, 4]);
        } else {
            panic!("Expected audio input");
        }
    }
}

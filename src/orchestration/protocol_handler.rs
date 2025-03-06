use crate::error::AdapterError;

/// 协议处理器接口
///
/// 定义了与不同协议交互的标准方法。该接口用于处理不同类型的通信协议，
/// 确保系统能够统一地处理各种协议的请求和响应。
///
/// # 示例
///
/// ```rust
/// use crate::orchestration::ProtocolHandler;
///
/// struct MyProtocolHandler;
///
/// impl ProtocolHandler for MyProtocolHandler {
///     fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
///         // 实现请求发送逻辑
///         Ok(vec![])
///     }
///     
///     fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
///         // 实现响应验证逻辑
///         Ok(true)
///     }
/// }
/// ```
pub trait ProtocolHandler: std::fmt::Debug {
    /// 发送请求
    ///
    /// 将请求数据发送到目标服务或设备。
    ///
    /// # 参数
    ///
    /// * `payload` - 要发送的请求数据
    ///
    /// # 返回值
    ///
    /// 返回响应数据或错误信息
    fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError>;

    /// 验证响应
    ///
    /// 验证接收到的响应数据是否有效。
    ///
    /// # 参数
    ///
    /// * `response` - 要验证的响应数据
    ///
    /// # 返回值
    ///
    /// 返回验证结果，true表示有效，false表示无效
    fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试用的协议处理器实现
    #[derive(Debug)]
    struct TestProtocolHandler;

    impl ProtocolHandler for TestProtocolHandler {
        fn send_request(&self, payload: &[u8]) -> Result<Vec<u8>, AdapterError> {
            // 简单地返回输入数据的副本
            Ok(payload.to_vec())
        }

        fn validate_response(&self, response: &[u8]) -> Result<bool, AdapterError> {
            // 假设非空响应就是有效的
            Ok(!response.is_empty())
        }
    }

    #[test]
    fn test_protocol_handler() {
        let handler = TestProtocolHandler;

        // 测试发送请求
        let payload = vec![1, 2, 3, 4];
        let response = handler.send_request(&payload).unwrap();
        assert_eq!(response, payload);

        // 测试验证响应
        assert!(handler.validate_response(&response).unwrap());
        assert!(!handler.validate_response(&vec![]).unwrap());
    }

    #[test]
    fn test_empty_payload() {
        let handler = TestProtocolHandler;

        // 测试发送空请求
        let response = handler.send_request(&[]).unwrap();
        assert!(response.is_empty());

        // 验证空响应
        assert!(!handler.validate_response(&response).unwrap());
    }
}

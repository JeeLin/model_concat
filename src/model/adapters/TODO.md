# 模型API提供商对接清单

## 待对接厂商列表

- 阿里云 (Alibaba Cloud)
- 字节跳动火山引擎 (ByteDance Volcano)
- 百度文心 (Baidu ERNIE)
- 腾讯混元 (Tencent Hunyuan)
- 科大讯飞 (iFLYTEK)
- 商汤科技 (SenseTime)
- 华为云 (Huawei Cloud)
- 智谱AI (Zhipu AI)
- Dify (Self-hosted)

## 对接步骤规范

1. 在`adapters`目录下创建厂商专属模块目录（小写英文）
2. 实现`ProviderAdapter` trait：
    - 必须包含`api_key`和`base_url`基础配置
    - 实现`select_protocol`方法支持http/websocket协议
3. 创建`http.rs`和`websocket.rs`协议实现文件
4. 在registry.rs中注册适配器工厂方法

## 厂商专属配置参数示例

```rust
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// 认证密钥 (必填)
    pub api_key: String,

    /// API端点地址 (必填)
    pub base_url: String,

    /// 厂商特殊参数（按需添加）
    pub secret_id: Option<String>,  // 腾讯/阿里等需要的额外认证ID
    pub region_id: Option<String>,  // 云服务商需要的区域标识
    pub signature_method: Option<String>,  // 签名算法类型
}
```

## 开发注意事项

1. 错误处理需统一使用`AdapterError`
2. 必须通过`ErrorMiddleware`包装返回错误
3. 需实现完整的重试机制和流量控制
4. 严格遵循各厂商的API速率限制要求
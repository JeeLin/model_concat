# model_concat

#### 介绍

模型串联编排服务是一个灵活的模型服务编排系统，支持多个AI模型的串联调用和并行处理。本服务可以处理不同形式的输入输出（文本、音频等），并提供便捷的音频格式转换工具。

#### 软件架构

系统主要包含以下核心组件：

1. 模型输入输出管理
   - 支持文本、音频等多种数据形式
   - 音频支持多种格式和码率
   - 支持流式和非流式处理

2. 音频处理工具
   - 支持多种音频格式间的转换
   - 支持码率、采样率等参数调整

3. 模型服务层
   - 统一的模型服务接口
   - 支持WebSocket连接
   - 标准化的前处理、处理和后处理流程

4. 编排引擎
   - 支持多模型串行执行
   - 支持模型并行处理
   - 智能输入输出匹配
   - 自动音频格式转换

#### 安装教程

1. 克隆项目到本地
2. 安装依赖包
3. 配置模型服务连接信息
4. 启动服务

#### 使用说明

1. 交互方式
   - HTTP API（仅用于查询服务信息）
   - WebSocket（用于模型处理和流式通信）

2. HTTP API 接口
   ```
   GET  /providers            # 获取所有服务商
   GET  /providers/{provider}/models  # 获取服务商支持的模型
   GET  /models/{provider}/{model_id}  # 获取模型详情
   GET  /metrics             # 获取性能监控数据
   ```

3. WebSocket 接口
   ```
   WS   /ws             # WebSocket连接
   ```

4. 交互流程示例

   WebSocket流程：
   ```
   Client                              Server
     |                                   |
     |-------- WebSocket连接 ----------->|
     |                                   |
     |-------- Init消息 --------------->|
     |<------- Init确认 ----------------|
     |                                   |
     |-------- Start消息 -------------->|
     |                                   |
     |<------- Progress消息 -----------| 实时进度
     |<------- Progress消息 -----------| 持续推送
     |                                   |
     |<------- Result消息 -------------| 处理完成
     |                                   |
     |-------- Ping消息 -------------->| 心跳检查
     |<------- Pong消息 ---------------|
   ```

5. 请求示例

   WebSocket消息：

```json
{
   "type": "Start",
   "data": {
      "pipeline": {
         "name": "realtime_translation",
         "stages": [
            {
               "name": "speech_to_text",
               "models": [
                  {
                     "provider": "openai",
                     "model_id": "whisper"
                  }
               ]
            },
            {
               "name": "translation",
               "merge_strategy": "First",
               "models": [
                  {
                     "provider": "openai",
                     "model_id": "gpt-4"
                  },
                  {
                     "provider": "openai",
                     "model_id": "gpt-4o"
                  }
               ]
            }
         ]
      },
      "input": {
         "type": "audio",
         "data": {
            "content": "streaming_audio_data",
            "format": {
               "codec": "wav",
               "sample_rate": 16000,
               "channels": 1
            }
         }
      }
   }
}
```

服务器响应：

```json
{
   "type": "Progress",
   "data": {
      "stage": "speech_to_text",
      "progress": 45,
      "intermediate_result": "正在识别语音..."
   }
}
```

```json
{
   "type": "Result",
   "data": {
      "task_id": "task-123",
      "output": {
         "type": "text",
         "data": "这是识别和翻译后的文本内容"
      },
      "elapsed_ms": 2500
   }
}
```

6. 执行模式说明

   系统采用分阶段(Stage)执行模式：
   - **阶段(Stage)**：每个阶段包含一组模型，阶段之间按顺序执行，前一个阶段的输出作为后一个阶段的输入
   - **阶段内并行**：同一阶段内的多个模型并行执行，然后根据合并策略选择最终结果

   合并策略包括：
   - **First**：使用第一个结果
   - **Last**：使用最后一个结果
   - **Longest**：使用最大长度的结果
   - **Shortest**：使用最小长度的结果
   - **Concat**：合并所有结果（适用于文本）

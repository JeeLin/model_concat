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
   - 支持HTTP和WebSocket连接
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
   - HTTP API（适用于单次请求）
   - WebSocket（适用于流式处理）

2. HTTP API 接口
   ```
   POST /tasks           # 创建处理任务
   GET  /tasks/{id}      # 获取任务状态
   GET  /tasks/{id}/result  # 获取任务结果
   ```

3. WebSocket 接口
   ```
   WS   /ws             # WebSocket连接
   ```

4. 交互流程示例

   HTTP流程：
   ```
   Client                              Server
     |                                   |
     |------ POST /tasks -------->| 创建任务
     |<---- 返回task_id和状态 -----------|
     |                                   |
     |--- GET /tasks/{id}/status ------->| 轮询状态
     |<---- 返回处理进度 ----------------|
     |                                   |
     |--- GET /tasks/{id}/result ------->| 获取结果
     |<---- 返回处理结果 ----------------|
   ```

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

   HTTP请求：
   ```json
   POST /tasks
   {
     "task_id": "task-123",
     "pipeline": {
       "name": "speech_analysis",
       "stages": [
         {
           "name": "speech_to_text",
           "models": [
             {
               "provider": "openai",
               "model_id": "whisper",
               "parameters": {
                 "language": "zh"
               }
             }
           ]
         }
       ]
     },
     "input": {
       "type": "audio",
       "data": {
         "content": "base64_audio_data",
         "format": {
           "codec": "mp3",
           "sample_rate": 16000,
           "channels": 1
         }
       }
     }
   }
   ```

   WebSocket消息：
   ```json
   // 客户端发送
   {
     "type": "Start",
     "data": {
       "pipeline": {
         "name": "realtime_translation",
         "stages": [
           {
             "name": "speech_to_text",
             "models": [{"provider": "openai", "model_id": "whisper"}]
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


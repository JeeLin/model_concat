# model_concat

#### Description
Model Concatenation Service is a flexible model orchestration system that supports sequential and parallel processing of multiple AI models. This service handles various forms of input and output (text, audio, etc.) and provides convenient audio format conversion tools.

#### Software Architecture
The system consists of the following core components:

1. Model I/O Management
   - Supports multiple data forms (text, audio, etc.)
   - Audio support for various formats and bitrates
   - Streaming and non-streaming processing support

2. Audio Processing Tools
   - Support for multiple audio format conversions
   - Bitrate and sampling rate adjustment capabilities

3. Model Service Layer
   - Unified model service interface
   - HTTP and WebSocket connection support
   - Standardized pre-processing, processing, and post-processing pipeline

4. Orchestration Engine
   - Support for sequential model execution
   - Parallel processing capabilities
   - Intelligent input/output matching
   - Automatic audio format conversion

#### Installation

1. Clone the project locally
2. Install dependencies
3. Configure model service connections
4. Start the service

#### Instructions

1. Interaction Methods
   - HTTP API (for single requests)
   - WebSocket (for streaming data)

2. HTTP API Endpoints
   ```
   POST /api/v1/tasks           # Create processing task
   GET  /api/v1/tasks/{id}      # Get task status
   GET  /api/v1/tasks/{id}/result  # Get task result
   ```

3. WebSocket Endpoint
   ```
   WS   /api/v1/ws             # WebSocket connection
   ```

4. Interaction Flow Examples

   HTTP Flow:
   ```
   Client                              Server
     |                                   |
     |------ POST /api/v1/tasks -------->| Create Task
     |<---- Return task_id & status -----|
     |                                   |
     |--- GET /tasks/{id}/status ------->| Poll Status
     |<---- Return progress -------------|
     |                                   |
     |--- GET /tasks/{id}/result ------->| Get Result
     |<---- Return result ---------------|
   ```

   WebSocket Flow:
   ```
   Client                              Server
     |                                   |
     |-------- WS Connection ----------->|
     |                                   |
     |-------- Init Message ----------->|
     |<------- Init Confirm ------------|
     |                                   |
     |-------- Start Message ---------->|
     |                                   |
     |<------- Progress Message -------| Real-time
     |<------- Progress Message -------| Updates
     |                                   |
     |<------- Result Message ---------| Complete
     |                                   |
     |-------- Ping Message ---------->| Heartbeat
     |<------- Pong Message -----------|
   ```

5. Request Examples

   HTTP Request:
   ```json
   POST /api/v1/tasks
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

   WebSocket Message:
   ```json
   // Client Send
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


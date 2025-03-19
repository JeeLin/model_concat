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
   - WebSocket connection support
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
   - HTTP API (for service information queries only)
   - WebSocket (for model processing and streaming communication)

2. HTTP API Endpoints
   ```
   GET  /providers            # Get all providers
   GET  /providers/{provider}/models  # Get provider's supported models
   GET  /models/{provider}/{model_id}  # Get model details
   GET  /metrics             # Get performance metrics
   ```

3. WebSocket Endpoint
   ```
   WS   /ws             # WebSocket connection
   ```

4. Interaction Flow Examples

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

   WebSocket Message:

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

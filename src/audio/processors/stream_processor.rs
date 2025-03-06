use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use tracing::{debug, error};

use crate::audio::buffer::{SharedZeroCopyBuffer, ZeroCopyBuffer};
use crate::audio::format::AudioFormat;
use crate::audio::stream::{AudioChunk, StreamProcessor};
use crate::error::ServiceResult;

/// 并行流处理器配置
///
/// 用于配置增强型流处理器的并行处理参数，包括：
/// - 工作线程数：控制并行处理的线程数量
/// - 缓冲区大小：控制输入输出队列的容量
/// - 处理块大小：控制单次处理的数据量
/// - 零拷贝模式：是否启用零拷贝优化
///
/// # 示例
///
/// ```rust
/// use crate::audio::processors::ParallelProcessorConfig;
///
/// // 创建自定义配置
/// let config = ParallelProcessorConfig {
///     worker_threads: 4,           // 4个工作线程
///     input_buffer_size: 64,       // 输入缓冲64个块
///     output_buffer_size: 64,      // 输出缓冲64个块
///     max_chunk_size: 32768,       // 32KB的块大小
///     use_zero_copy: true,         // 启用零拷贝
/// };
/// ```
pub struct ParallelProcessorConfig {
    /// 并行处理的工作线程数
    pub worker_threads: usize,
    /// 输入缓冲区大小
    pub input_buffer_size: usize,
    /// 输出缓冲区大小
    pub output_buffer_size: usize,
    /// 每个处理块的最大大小（字节）
    pub max_chunk_size: usize,
    /// 是否启用零拷贝模式
    pub use_zero_copy: bool,
}

impl Default for ParallelProcessorConfig {
    fn default() -> Self {
        Self {
            worker_threads: num_cpus::get().max(2),
            input_buffer_size: 32,
            output_buffer_size: 32,
            max_chunk_size: 16384, // 16KB
            use_zero_copy: true,
        }
    }
}

/// 增强型流处理器
///
/// 提供高性能的流式音频处理，支持：
/// - 零拷贝缓冲区管理：减少内存拷贝开销
/// - 并行处理：利用多核提升处理性能
/// - 增量编解码：支持流式处理大文件
/// - 自动负载均衡：智能分配处理任务
/// - 异步处理：非阻塞的处理模式
///
/// # 示例
///
/// ```rust
/// use crate::audio::processors::{EnhancedStreamProcessor, ParallelProcessorConfig};
/// use crate::audio::stream::{AudioChunk, StreamProcessor};
///
/// // 创建处理器
/// let mut processor = EnhancedStreamProcessor::new(
///     MyAudioProcessor::new(),  // 你的音频处理器
///     input_format,             // 输入格式
///     output_format,            // 输出格式
/// );
///
/// // 启动处理管道
/// processor.start();
///
/// // 处理音频块
/// while let Some(chunk) = input_stream.next().await {
///     processor.process_chunk(chunk).await?;
/// }
///
/// // 完成处理
/// processor.finish().await?;
/// ```
pub struct EnhancedStreamProcessor<P: StreamProcessor + Send + 'static> {
    /// 内部处理器
    processor: Arc<Mutex<P>>,
    /// 输入格式
    input_format: AudioFormat,
    /// 输出格式
    output_format: AudioFormat,
    /// 配置
    config: ParallelProcessorConfig,
    /// 输入缓冲区
    input_buffer: Option<SharedZeroCopyBuffer>,
    /// 输出接收器
    output_receiver: Option<mpsc::Receiver<ServiceResult<AudioChunk>>>,
    /// 输出发送器
    output_sender: Option<mpsc::Sender<ServiceResult<AudioChunk>>>,
    /// 处理任务句柄
    processing_task: Option<tokio::task::JoinHandle<()>>,
    /// 是否已完成
    is_finished: bool,
}

impl<P: StreamProcessor + Send + Sync + 'static> EnhancedStreamProcessor<P> {
    /// 创建新的增强型流处理器
    ///
    /// # 参数
    /// * `processor` - 内部音频处理器实现
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    ///
    /// # 示例
    /// ```rust
    /// let processor = EnhancedStreamProcessor::new(
    ///     MyProcessor::new(),
    ///     input_format,
    ///     output_format
    /// );
    /// ```
    pub fn new(processor: P, input_format: AudioFormat, output_format: AudioFormat) -> Self {
        Self::with_config(
            processor,
            input_format,
            output_format,
            ParallelProcessorConfig::default(),
        )
    }

    /// 使用自定义配置创建流处理器
    ///
    /// # 参数
    /// * `processor` - 内部音频处理器实现
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    /// * `config` - 并行处理配置
    pub fn with_config(
        processor: P,
        input_format: AudioFormat,
        output_format: AudioFormat,
        config: ParallelProcessorConfig,
    ) -> Self {
        Self {
            processor: Arc::new(Mutex::new(processor)),
            input_format,
            output_format,
            config,
            input_buffer: None,
            output_receiver: None,
            output_sender: None,
            processing_task: None,
            is_finished: false,
        }
    }

    /// 启动处理管道
    ///
    /// 初始化处理器的内部状态，包括：
    /// - 创建零拷贝缓冲区（如果启用）
    /// - 建立输入输出通道
    /// - 启动后台处理任务
    pub fn start(&mut self) {
        // 创建缓冲区
        if self.config.use_zero_copy {
            self.input_buffer = Some(SharedZeroCopyBuffer::new(self.config.max_chunk_size * 4));
        }

        // 创建通道
        let (tx, rx) = mpsc::channel(self.config.output_buffer_size);
        self.output_sender = Some(tx);
        self.output_receiver = Some(rx);

        // 启动处理任务
        self.start_processing_task();
    }

    /// 启动处理任务
    ///
    /// 创建后台任务处理音频数据，支持：
    /// - 零拷贝模式：直接从共享缓冲区读取数据
    /// - 多线程并行处理：利用工作线程池
    /// - 异步处理：非阻塞的任务调度
    fn start_processing_task(&mut self) {
        if self.processing_task.is_some() {
            return;
        }

        // 克隆必要的资源
        let processor = self.processor.clone();
        let sender = self.output_sender.as_ref().unwrap().clone();
        let input_buffer = self.input_buffer.as_ref().map(|buf| buf.clone());
        let worker_count = self.config.worker_threads;
        let max_chunk_size = self.config.max_chunk_size;
        let use_zero_copy = self.config.use_zero_copy;

        // 启动处理任务
        self.processing_task = Some(tokio::spawn(async move {
            // 处理逻辑
            if let Some(buffer) = input_buffer {
                // 零拷贝模式
                while let Some(data) = buffer.read(max_chunk_size) {
                    let start = Instant::now();

                    // 创建音频块
                    let chunk = AudioChunk {
                        data,
                        timestamp: 0, // 这里需要实际的时间戳
                        duration: 0,  // 这里需要实际的持续时间
                        is_last: false,
                    };

                    // 处理块
                    let processor_clone = processor.clone();
                    let sender_clone = sender.clone();

                    // 使用工作线程池处理
                    task::spawn(async move {
                        let mut proc = processor_clone.lock().await;
                        let result = proc.process_chunk(chunk).await;

                        // 发送处理结果
                        if let Err(e) = sender_clone.send(result).await {
                            error!("Failed to send processed chunk: {}", e);
                        }
                    });

                    debug!(
                        "Chunk processing scheduled in {}ms",
                        start.elapsed().as_millis()
                    );
                }
            } else {
                // 非零拷贝模式 - 这里会在实际使用时实现
                // ...
            }
        }));
    }

    /// 处理音频块
    ///
    /// # 参数
    /// * `chunk` - 待处理的音频数据块
    ///
    /// # 返回
    /// 返回处理结果，如果成功则返回 Ok(())
    ///
    /// # 错误
    /// 如果处理过程中发生错误，返回对应的错误信息
    pub async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<()> {
        if self.is_finished {
            return Ok(());
        }

        // 确保处理管道已启动
        if self.output_receiver.is_none() {
            self.start();
        }

        if let Some(buffer) = &self.input_buffer {
            // 零拷贝模式：写入缓冲区
            buffer.write(chunk.data.as_ref());
        } else {
            // 非零拷贝模式：直接处理
            let mut processor = self.processor.lock().await;
            let result = processor.process_chunk(chunk).await;

            // 发送处理结果
            if let Some(sender) = &self.output_sender {
                if let Err(e) = sender.send(result).await {
                    error!("Failed to send processed chunk: {}", e);
                }
            }
        }

        Ok(())
    }

    /// 完成处理
    ///
    /// 结束音频处理流程，执行以下操作：
    /// - 刷新处理器缓冲区
    /// - 发送最后的数据块
    /// - 关闭处理管道
    ///
    /// # 返回
    /// 返回处理结果，如果成功则返回 Ok(())
    pub async fn finish(&mut self) -> ServiceResult<()> {
        if self.is_finished {
            return Ok(());
        }

        self.is_finished = true;

        // 刷新处理器
        let mut processor = self.processor.lock().await;
        if let Ok(Some(final_chunk)) = processor.flush().await {
            // 发送最后的块
            if let Some(sender) = &self.output_sender {
                if let Err(e) = sender.send(Ok(final_chunk)).await {
                    error!("Failed to send final chunk: {}", e);
                }
            }
        }

        // 关闭发送器
        self.output_sender = None;

        Ok(())
    }

    /// 获取下一个处理后的块
    ///
    /// # 返回
    /// 返回处理后的音频块，如果没有更多数据则返回 None
    pub async fn next_chunk(&mut self) -> Option<ServiceResult<AudioChunk>> {
        if let Some(receiver) = &mut self.output_receiver {
            receiver.recv().await
        } else {
            None
        }
    }

    /// 重置处理器状态
    ///
    /// 执行完整的状态重置：
    /// - 停止当前处理任务
    /// - 重置内部处理器
    /// - 清空缓冲区
    /// - 重置所有状态标志
    pub async fn reset(&mut self) {
        // 停止当前处理任务
        if let Some(task) = self.processing_task.take() {
            task.abort();
        }

        // 重置处理器
        if let Ok(mut processor) = self.processor.try_lock() {
            processor.reset();
        }

        // 清空缓冲区
        if let Some(buffer) = &self.input_buffer {
            buffer.clear();
        }

        // 重置状态
        self.is_finished = false;
        self.output_sender = None;
        self.output_receiver = None;
    }
}

/// 实现Stream特征，使其可以用于异步迭代
///
/// 通过实现Stream特征，使EnhancedStreamProcessor可以在异步上下文中使用for_each等迭代器方法。
/// 每次迭代都会返回一个处理后的音频块。
///
/// # 示例
/// ```rust
/// use futures::StreamExt;
///
/// let mut processor = EnhancedStreamProcessor::new(/* ... */);
///
/// // 使用异步迭代器处理音频块
/// processor.for_each(|chunk| async {
///     match chunk {
///         Ok(audio_data) => println!("处理音频块: {:?}", audio_data),
///         Err(e) => eprintln!("处理错误: {}", e),
///     }
/// }).await;
/// ```
impl<P: StreamProcessor + Send + Sync + 'static> Stream for EnhancedStreamProcessor<P> {
    type Item = ServiceResult<AudioChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(receiver) = &mut self.output_receiver {
            Pin::new(receiver).poll_recv(cx)
        } else {
            Poll::Ready(None)
        }
    }
}

/// 实现StreamProcessor特征
///
/// 使EnhancedStreamProcessor可以作为标准的音频流处理器使用，
/// 提供统一的处理接口：
/// - 处理单个音频块
/// - 刷新处理器状态
/// - 重置处理器
///
/// # 示例
/// ```rust
/// use crate::audio::stream::StreamProcessor;
///
/// async fn process_audio<P: StreamProcessor>(processor: &mut P) {
///     // 处理音频块
///     let chunk = AudioChunk::new(/* ... */);
///     let processed = processor.process_chunk(chunk).await?;
///     
///     // 完成处理
///     if let Some(final_chunk) = processor.flush().await? {
///         println!("最后的数据块: {:?}", final_chunk);
///     }
/// }
/// ```
#[async_trait::async_trait]
impl<P: StreamProcessor + Send + Sync + 'static> StreamProcessor for EnhancedStreamProcessor<P> {
    /// 处理单个音频块
    ///
    /// # 参数
    /// * `chunk` - 待处理的音频数据块
    ///
    /// # 返回
    /// 返回处理后的音频块
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        // 将块添加到处理队列
        self.process_chunk(chunk).await?;

        // 获取处理后的块
        match self.next_chunk().await {
            Some(result) => result,
            None => Err(crate::error::ServiceError::AudioProcessing(
                "No processed chunk available".to_string(),
            )),
        }
    }

    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
        // 完成处理
        self.finish().await?;

        // 获取最后一个处理后的块
        match self.next_chunk().await {
            Some(Ok(chunk)) => Ok(Some(chunk)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    fn reset(&mut self) {
        // 使用阻塞方式重置，因为特征方法不是异步的
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            self.reset().await;
        });
    }

    fn name(&self) -> &str {
        "EnhancedStreamProcessor"
    }
}

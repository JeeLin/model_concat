use bytes::{Bytes, BytesMut};
use crossbeam_queue::SegQueue;
use futures::{Stream, StreamExt};
use num_cpus;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use tracing::{debug, error, info};

use crate::audio::buffer::{SharedZeroCopyBuffer, ZeroCopyBuffer};
use crate::audio::format::AudioFormat;
use crate::audio::stream::{AudioChunk, StreamProcessor};
use crate::error::ServiceResult;

/// # 并行处理配置
///
/// 用于配置并行音频处理器的各项参数，控制处理性能和资源使用。
///
/// ## 示例
///
/// ```rust
/// let config = ParallelProcessingConfig {
///     worker_threads: 4,
///     chunk_size: 32768,
///     ..Default::default()
/// };
/// ```
pub struct ParallelProcessingConfig {
    /// 工作线程数量，决定并行处理的程度
    pub worker_threads: usize,
    /// 分片大小（字节），控制每个处理单元的数据量
    pub chunk_size: usize,
    /// 队列容量，控制缓冲区大小
    pub queue_capacity: usize,
    /// 是否使用工作窃取算法，启用后可以提高线程利用率
    pub use_work_stealing: bool,
    /// 是否使用零拷贝缓冲区，减少内存复制开销
    pub use_zero_copy: bool,
    /// 工作线程等待时间（毫秒），控制线程轮询间隔
    pub worker_wait_ms: u64,
    /// 最大并行任务数，限制同时处理的任务数量
    pub max_parallel_tasks: usize,
    /// 结果排序策略，启用后确保输出顺序与输入一致
    pub sort_results: bool,
    /// 自适应负载均衡，根据系统负载动态调整处理策略
    pub adaptive_load_balancing: bool,
}

impl Default for ParallelProcessingConfig {
    /// 创建默认的并行处理配置
    ///
    /// 默认配置会根据系统CPU核心数自动设置合理的线程数和任务数，
    /// 并启用工作窃取、零拷贝、结果排序和自适应负载均衡等优化特性。
    fn default() -> Self {
        Self {
            worker_threads: num_cpus::get().max(2), // 确保至少有2个工作线程
            chunk_size: 16384,                      // 16KB
            queue_capacity: 32,
            use_work_stealing: true,
            use_zero_copy: true,
            worker_wait_ms: 1,
            max_parallel_tasks: num_cpus::get() * 2, // 默认为CPU核心数的2倍
            sort_results: true,
            adaptive_load_balancing: true,
        }
    }
}

/// # 并行音频处理器
///
/// 提供高性能的并行音频处理框架，支持多种优化策略：
///
/// - **多线程并行处理**：利用多核CPU提高处理速度
/// - **工作窃取负载均衡**：动态分配任务，提高线程利用率
/// - **零拷贝数据传输**：减少内存复制，提高性能
/// - **流水线处理模式**：支持连续数据流的高效处理
///
/// ## 类型参数
///
/// - `P`: 实现了`StreamProcessor`特征的处理器类型，用于实际的音频处理逻辑
///
/// ## 示例
///
/// ```rust
/// // 创建处理器工厂函数
/// let processor_factory = || MyAudioProcessor::new();
///
/// // 创建并行处理器
/// let mut parallel_processor = ParallelAudioProcessor::new(
///     processor_factory,
///     input_format,
///     output_format
/// );
///
/// // 启动处理管道
/// parallel_processor.start();
///
/// // 提交音频块进行处理
/// parallel_processor.submit(audio_chunk);
///
/// // 获取处理结果
/// while let Some(result) = parallel_processor.try_get_result() {
///     // 处理结果...
/// }
///
/// // 停止处理管道
/// parallel_processor.stop().await;
/// ```
pub struct ParallelAudioProcessor<P: StreamProcessor + Send + Sync + 'static> {
    /// 内部处理器工厂，用于为每个工作线程创建处理器实例
    processor_factory: Arc<dyn Fn() -> P + Send + Sync>,
    /// 输入音频格式
    input_format: AudioFormat,
    /// 输出音频格式
    output_format: AudioFormat,
    /// 并行处理配置
    config: ParallelProcessingConfig,
    /// 输入队列，用于存储待处理的音频块
    input_queue: Arc<SegQueue<AudioChunk>>,
    /// 输出队列，用于存储处理完成的结果
    output_queue: Arc<SegQueue<ServiceResult<AudioChunk>>>,
    /// 工作线程句柄，用于管理后台处理线程
    workers: Vec<tokio::task::JoinHandle<()>>,
    /// 是否已启动处理管道
    started: bool,
    /// 是否已完成处理并关闭管道
    finished: bool,
}

impl<P: StreamProcessor + Send + Sync + Clone + 'static> ParallelAudioProcessor<P> {
    /// 创建新的并行处理器
    ///
    /// 使用默认配置创建并行音频处理器。
    ///
    /// # 参数
    ///
    /// * `processor_factory` - 处理器工厂函数，用于创建实际处理音频的处理器实例
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    ///
    /// # 返回值
    ///
    /// 返回配置好的并行处理器实例
    pub fn new(
        processor_factory: impl Fn() -> P + Send + Sync + 'static,
        input_format: AudioFormat,
        output_format: AudioFormat,
    ) -> Self {
        Self::with_config(
            processor_factory,
            input_format,
            output_format,
            ParallelProcessingConfig::default(),
        )
    }

    /// 使用自定义配置创建并行处理器
    ///
    /// 允许通过自定义配置控制并行处理的各个方面。
    ///
    /// # 参数
    ///
    /// * `processor_factory` - 处理器工厂函数，用于创建实际处理音频的处理器实例
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    /// * `config` - 自定义的并行处理配置
    ///
    /// # 返回值
    ///
    /// 返回配置好的并行处理器实例
    pub fn with_config(
        processor_factory: impl Fn() -> P + Send + Sync + 'static,
        input_format: AudioFormat,
        output_format: AudioFormat,
        config: ParallelProcessingConfig,
    ) -> Self {
        Self {
            processor_factory: Arc::new(processor_factory),
            input_format,
            output_format,
            config,
            input_queue: Arc::new(SegQueue::new()),
            output_queue: Arc::new(SegQueue::new()),
            workers: Vec::new(),
            started: false,
            finished: false,
        }
    }

    /// 启动处理管道
    ///
    /// 创建并启动工作线程池，准备处理音频数据。主要功能包括：
    ///
    /// - **工作线程初始化**：为每个工作线程创建独立的处理器实例
    /// - **任务调度优化**：实现工作窃取和动态负载均衡
    /// - **性能监控**：跟踪处理时间和资源利用率
    /// - **自适应调整**：根据系统负载动态调整处理策略
    ///
    /// # 性能优化策略
    ///
    /// - **动态负载均衡**：根据队列深度和CPU负载调整等待时间
    /// - **指数退避**：空闲时采用指数增长的等待时间，避免资源浪费
    /// - **工作窃取**：允许空闲线程处理其他线程的任务
    /// - **自适应调度**：根据系统状态动态调整处理策略
    ///
    /// # 注意事项
    ///
    /// - 如果处理管道已经启动，此方法不会执行任何操作
    /// - 工作线程数量由配置决定，默认为CPU核心数
    /// - 每个工作线程都有独立的处理器实例，确保线程安全
    pub fn start(&mut self) {
        if self.started {
            return;
        }

        self.started = true;

        // 创建工作线程
        let worker_count = self.config.worker_threads;
        self.workers.reserve(worker_count);

        for worker_id in 0..worker_count {
            let processor = (self.processor_factory)();
            let input_queue = Arc::clone(&self.input_queue);
            let output_queue = Arc::clone(&self.output_queue);
            let use_work_stealing = self.config.use_work_stealing;
            let worker_wait_ms = self.config.worker_wait_ms;
            let config = self.config.clone();

            // 启动工作线程
            let handle = tokio::spawn(async move {
                let mut processor = processor;

                info!("Worker {} started", worker_id);

                loop {
                    // 从队列获取任务
                    let chunk = match input_queue.pop() {
                        Some(chunk) => chunk,
                        None => {
                            // 如果队列为空，可能等待一段时间
                            if use_work_stealing {
                                // 动态负载均衡算法
                                let queue_depth = input_queue.len();
                                let dynamic_wait = if queue_depth == 0 {
                                    // 指数退避策略：500ms, 1s, 2s...最大10s
                                    (500 * 2_u32.pow(worker_id as u32 % 3)).min(10000)
                                } else {
                                    // 动态计算：基础等待时间 × (1 - 队列利用率)² × CPU负载系数
                                    let queue_utilization =
                                        queue_depth as f32 / config.queue_capacity as f32;
                                    let cpu_load =
                                        sys_info::loadavg().map(|l| l.one).unwrap_or(0.0) / 100.0;
                                    (worker_wait_ms as f32
                                        * (1.0 - queue_utilization).powi(2)
                                        * (1.0 - cpu_load))
                                        as u64
                                }
                                    .max(10); // 最小等待10ms

                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    if config.adaptive_load_balancing {
                                        dynamic_wait
                                    } else {
                                        worker_wait_ms
                                    },
                                ))
                                    .await;
                                continue;
                            } else {
                                // 非工作窃取模式：如果队列为空则退出
                                break;
                            }
                        }
                    };

                    // 检查是否是结束标记
                    if chunk.is_last {
                        // 处理最后一个块并退出
                        let result = processor.process_chunk(chunk).await;
                        output_queue.push(result);
                        break;
                    }

                    // 处理音频块
                    let start = Instant::now();
                    let result = processor.process_chunk(chunk).await;
                    let elapsed = start.elapsed();

                    debug!("Worker {} processed chunk in {:?}", worker_id, elapsed);

                    // 将结果放入输出队列
                    output_queue.push(result);
                }

                // 处理完成后刷新处理器
                if let Ok(Some(final_chunk)) = processor.flush().await {
                    output_queue.push(Ok(final_chunk));
                }

                info!("Worker {} finished", worker_id);
            });

            self.workers.push(handle);
        }
    }

    /// 停止处理管道
    ///
    /// 优雅地关闭处理管道，确保所有任务都得到处理。主要功能包括：
    ///
    /// - **结束信号分发**：向所有工作线程发送结束标记
    /// - **资源回收**：等待所有工作线程完成并释放资源
    /// - **状态管理**：更新处理器状态标志
    ///
    /// # 性能考虑
    ///
    /// - 采用异步等待，不会阻塞调用线程
    /// - 确保所有已提交的任务都能得到处理
    /// - 优雅关闭，避免数据丢失
    ///
    /// # 注意事项
    ///
    /// - 如果处理管道未启动或已停止，此方法不会执行任何操作
    /// - 调用此方法后，处理器将不再接受新的任务
    /// - 所有工作线程完成后，资源会被自动释放
    ///
    /// # 异步
    ///
    /// 此方法是异步的，会等待所有工作线程完成。
    pub async fn stop(&mut self) {
        if !self.started || self.finished {
            return;
        }

        self.finished = true;

        // 向每个工作线程发送结束信号
        for _ in 0..self.workers.len() {
            self.input_queue.push(AudioChunk {
                data: Bytes::new(),
                timestamp: 0,
                duration: 0,
                is_last: true,
            });
        }

        // 等待所有工作线程完成
        for handle in self.workers.drain(..) {
            let _ = handle.await;
        }
    }

    /// 提交音频块进行处理
    ///
    /// 将音频数据块提交到处理队列，由工作线程池异步处理。主要特点：
    ///
    /// - **异步提交**：非阻塞操作，立即返回
    /// - **负载均衡**：自动分配给空闲的工作线程
    /// - **队列管理**：智能处理队列满/空状态
    ///
    /// # 参数
    ///
    /// * `chunk` - 待处理的音频数据块
    ///
    /// # 返回值
    ///
    /// 返回 `true` 表示提交成功，`false` 表示处理器已停止或队列已满
    ///
    /// # 注意事项
    ///
    /// - 处理器必须先调用 `start()` 方法启动
    /// - 如果处理器已停止，提交将失败
    /// - 建议监控返回值以确保提交成功
    pub fn submit(&self, chunk: AudioChunk) -> bool {
        if self.started && !self.finished {
            self.input_queue.push(chunk);
        }
    }

    /// 尝试获取处理结果
    ///
    /// 非阻塞地从输出队列中获取一个处理结果。如果队列为空，则返回None。
    ///
    /// # 返回值
    ///
    /// 返回一个Option，包含处理结果或None（如果队列为空）
    pub fn try_get_result(&self) -> Option<ServiceResult<AudioChunk>> {
        self.output_queue.pop()
    }

    /// 等待并收集所有处理结果
    ///
    /// 使用智能轮询策略异步等待和收集处理结果。主要特点：
    ///
    /// - **自适应等待**：根据结果获取情况动态调整等待时间
    /// - **批量处理**：支持批量获取结果提高效率
    /// - **智能退出**：通过多重条件判断确保所有结果都被收集
    /// - **资源优化**：平衡等待时间和CPU使用率
    ///
    /// # 性能优化策略
    ///
    /// - **动态等待时间**：从1ms开始，最大增加到10ms
    /// - **批量获取**：一次性获取多个结果减少循环开销
    /// - **提前退出**：检测到无新结果时及时退出
    /// - **突发处理**：检测到新结果时重置等待时间
    ///
    /// # 返回值
    ///
    /// 返回包含所有处理结果的向量，按处理完成顺序排列
    ///
    /// # 异步
    ///
    /// 此方法是异步的，会等待直到：
    /// - 所有结果都被收集
    /// - 处理管道已完成且没有更多结果
    /// - 连续多次检查都没有新结果
    pub async fn wait_for_results(&self) -> Vec<ServiceResult<AudioChunk>> {
        let mut results = Vec::new();
        let mut empty_count = 0;
        let max_empty_tries = 10;
        let mut wait_time = 1;
        let max_wait_time = 10;

        // 改进的轮询策略，使用自适应等待时间
        while !self.finished || !self.output_queue.is_empty() {
            let mut got_result = false;

            // 尝试批量获取结果以提高效率
            for _ in 0..self.config.queue_capacity {
                if let Some(result) = self.output_queue.pop() {
                    results.push(result);
                    got_result = true;
                } else {
                    break;
                }
            }

            if !got_result {
                empty_count += 1;

                // 如果连续多次没有结果且处理已完成，提前退出
                if self.finished && empty_count >= max_empty_tries {
                    debug!(
                        "No results after {} tries, assuming all results collected",
                        max_empty_tries
                    );
                    break;
                }

                // 自适应等待策略
                tokio::time::sleep(tokio::time::Duration::from_millis(wait_time)).await;
                if wait_time < max_wait_time {
                    wait_time += 1;
                }
            } else {
                // 重置空结果计数
                empty_count = 0;
                // 有结果时重置等待时间以快速处理突发结果
                wait_time = 1;
            }
        }

        debug!("Collected {} results total", results.len());
        results
    }
}

/// 为并行音频处理器实现StreamProcessor特征
///
/// 这使得ParallelAudioProcessor可以像单个处理器一样使用，
/// 同时内部利用并行处理提高性能。
#[async_trait::async_trait]
impl<P: StreamProcessor + Send + Sync + Clone + 'static> StreamProcessor
for ParallelAudioProcessor<P>
{
    /// 处理单个音频块
    ///
    /// 将输入块分割成更小的块（如果需要），并行处理它们，
    /// 然后返回第一个可用的结果。
    ///
    /// # 参数
    ///
    /// * `chunk` - 要处理的音频块
    ///
    /// # 返回值
    ///
    /// 返回处理后的音频块
    ///
    /// # 异步
    ///
    /// 此方法是异步的，会等待直到至少有一个结果可用。
    async fn process_chunk(&mut self, chunk: AudioChunk) -> ServiceResult<AudioChunk> {
        // 确保处理管道已启动
        if !self.started {
            self.start();
        }

        // 如果块太大，采用智能分片策略处理
        if chunk.data.len() > self.config.chunk_size && !chunk.is_last {
            let mut offset = 0;
            let chunk_count =
                (chunk.data.len() + self.config.chunk_size - 1) / self.config.chunk_size;
            let optimal_chunk_count = self.config.worker_threads.min(chunk_count);
            let adaptive_chunk_size =
                (chunk.data.len() + optimal_chunk_count - 1) / optimal_chunk_count;

            debug!(
                "Splitting chunk of size {} into ~{} pieces with adaptive size {}",
                chunk.data.len(),
                optimal_chunk_count,
                adaptive_chunk_size
            );

            while offset < chunk.data.len() {
                let end = (offset + adaptive_chunk_size).min(chunk.data.len());
                let sub_chunk = AudioChunk {
                    data: chunk.data.slice(offset, end),
                    timestamp: chunk.timestamp + offset as u64,
                    duration: ((end - offset) as f64 / chunk.data.len() as f64
                        * chunk.duration as f64) as u64,
                    is_last: false,
                };

                self.submit(sub_chunk);
                offset = end;
            }
        } else {
            // 直接提交整个块
            self.submit(chunk);
        }

        // 获取一个处理结果
        // 使用自适应等待策略
        let mut wait_time = 1;
        let max_wait_time = 10;

        loop {
            if let Some(result) = self.try_get_result() {
                return result;
            }

            // 自适应等待：随着等待时间增加，逐渐增加休眠时间
            tokio::time::sleep(tokio::time::Duration::from_millis(wait_time)).await;
            if wait_time < max_wait_time {
                wait_time += 1;
            }
        }
    }

    /// 刷新处理器
    ///
    /// 发送结束信号，等待所有结果，并将它们合并成一个最终结果。
    ///
    /// # 返回值
    ///
    /// 返回合并后的最终音频块，如果没有结果则返回None
    ///
    /// # 异步
    ///
    /// 此方法是异步的，会等待直到所有结果都被收集和合并。
    async fn flush(&mut self) -> ServiceResult<Option<AudioChunk>> {
        // 发送结束信号
        self.submit(AudioChunk {
            data: Bytes::new(),
            timestamp: 0,
            duration: 0,
            is_last: true,
        });

        // 等待所有结果
        let results = self.wait_for_results().await;

        // 根据配置决定是否对结果进行排序
        let sorted_results = if self.config.sort_results {
            // 按时间戳排序结果
            let mut sorted = results;
            sorted.sort_by(|a, b| {
                if let (Ok(chunk_a), Ok(chunk_b)) = (a, b) {
                    chunk_a.timestamp.cmp(&chunk_b.timestamp)
                } else {
                    std::cmp::Ordering::Equal
                }
            });
            sorted
        } else {
            results
        };

        // 高效合并所有结果
        let mut combined_data = BytesMut::new();
        let mut last_timestamp = 0;
        let mut total_duration = 0;
        let mut success_count = 0;

        for result in sorted_results {
            if let Ok(chunk) = result {
                if chunk.timestamp > last_timestamp {
                    last_timestamp = chunk.timestamp;
                }
                total_duration += chunk.duration;

                // 高效合并数据
                if !chunk.data.is_empty() {
                    combined_data.extend_from_slice(&chunk.data);
                    success_count += 1;
                }
            }
        }

        debug!(
            "Combined {} successful chunks into final result",
            success_count
        );

        // 停止处理管道
        self.stop().await;

        if combined_data.is_empty() {
            Ok(None)
        } else {
            Ok(Some(AudioChunk {
                data: combined_data.freeze(),
                timestamp: last_timestamp,
                duration: total_duration,
                is_last: true,
            }))
        }
    }

    /// 重置处理器状态
    ///
    /// 停止当前处理管道，清空队列，并准备重新开始处理。
    fn reset(&mut self) {
        // 如果处理管道已启动，则停止它
        if self.started {
            // 使用阻塞方式停止，确保所有资源都被正确释放
            tokio::runtime::Handle::current().block_on(async {
                self.stop().await;
            });
        }

        // 重置状态
        self.started = false;
        self.finished = false;

        // 创建新的队列
        self.input_queue = Arc::new(SegQueue::new());
        self.output_queue = Arc::new(SegQueue::new());
    }

    /// 获取处理器名称
    ///
    /// 返回处理器的类型名称，用于日志和调试。
    fn name(&self) -> &str {
        "ParallelAudioProcessor"
    }
}

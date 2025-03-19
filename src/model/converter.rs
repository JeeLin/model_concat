//! 模型数据转换层
//!
//! 本模块提供模型数据格式转换功能，支持不同模型之间的数据交换。
//! 主要包括数据转换器（ModelDataConverter）和相关处理器的定义和实现。
//!
//! ## 主要功能
//!
//! - **格式转换**：支持音频和文本数据之间的转换
//! - **模型适配**：根据模型支持的输入输出格式自动转换数据
//! - **流式处理**：支持流式数据处理
//!
//! ## 使用示例
//!
//! ```rust
//! use crate::model::converter::ModelDataConverter;
//! use crate::model::{DataType, StreamDataType};
//! use crate::audio::format::AudioFormat;
//! use crate::audio::AudioCodec;
//!
//! async fn convert_data(data: DataType) -> DataType {
//!     // 创建转换器
//!     let converter = ModelDataConverter::new();
//!     
//!     // 执行转换（例如：文本到音频）
//!     converter.convert(data).await.unwrap()
//! }
//! ```

use crate::audio::converter::ProcessingStrategy;
use crate::audio::stream::{AudioChunk, AudioStream};
use crate::audio::{AudioConverter, AudioFormat, AudioParams};
use crate::error::{ServiceError, ServiceResult};
use crate::model::{DataType, StreamDataType, StreamMode};
use crate::text::stream::{TextChunk, TextStream};
use crate::text::{TextConverter, TextFormat, TextParams};
use bytes::{Bytes, BytesMut};
use futures::StreamExt;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info};

/// 模型数据转换器
///
/// 负责在不同模型之间转换数据格式，支持音频和文本数据的相互转换。
pub struct ModelDataConverter {}

impl ModelDataConverter {
    /// 创建新的模型数据转换器
    pub fn new() -> Self {
        Self {}
    }

    /// 获取音频转换器
    ///
    /// # 参数
    ///
    /// * `input_format` - 输入音频格式
    /// * `output_format` - 输出音频格式
    /// * `normalize` - 是否归一化音量
    /// * `gain` - 音量增益（dB）
    /// * `strategy` - 处理策略
    fn get_audio_converter(
        &self,
        input_format: &AudioFormat,
        output_format: &AudioFormat,
        normalize: bool,
        gain: Option<f32>,
        strategy: Option<ProcessingStrategy>,
    ) -> Arc<AudioConverter> {
        // 直接创建新的转换器
        let mut converter = AudioConverter::new(input_format.clone(), output_format.clone());

        // 设置其他参数
        converter.set_normalize(normalize);
        if let Some(g) = gain {
            converter.set_gain(g);
        }
        if let Some(s) = strategy {
            converter.set_strategy(s);
        }

        Arc::new(converter)
    }

    /// 获取文本转换器
    ///
    /// # 参数
    ///
    /// * `input_format` - 输入文本格式
    /// * `output_format` - 输出文本格式
    fn get_text_converter(
        &self,
        input_format: &TextFormat,
        output_format: &TextFormat,
    ) -> Arc<TextConverter> {
        // 直接创建新的转换器
        Arc::new(TextConverter::new(
            input_format.clone(),
            output_format.clone(),
        ))
    }

    /// 转换数据类型
    ///
    /// 将输入数据转换为目标格式
    ///
    /// # 参数
    ///
    /// * `input` - 输入数据
    /// * `target_type` - 目标数据类型
    ///
    /// # 返回
    ///
    /// 返回转换后的数据
    pub async fn convert(&mut self, input: DataType, target_type: &str) -> ServiceResult<DataType> {
        let start = Instant::now();

        // 根据输入类型和目标类型执行转换
        let result = match &input {
            DataType::Text { .. } if target_type == "Audio" => {
                Err(ServiceError::UnsupportedOperation(
                    "不支持文本到音频的转换，文本只能转换为文本格式".into(),
                ))
            },
            DataType::Text { .. } => {
                // 文本格式转换
                self.convert_text_to_text(input).await
            },
            DataType::Audio { .. } if target_type == "Text" => {
                Err(ServiceError::UnsupportedOperation(
                    "不支持音频到文本的转换，音频只能转换为音频格式".into(),
                ))
            },
            DataType::Audio { .. } => {
                // 音频格式转换
                self.convert_audio_to_audio(input).await
            },
        };

        debug!("模型数据转换完成 ({}ms)", start.elapsed().as_millis());

        result
    }

    /// 转换流式数据类型
    ///
    /// 将输入流式数据转换为目标格式
    ///
    /// # 参数
    ///
    /// * `input` - 输入流式数据
    /// * `target_type` - 目标数据类型
    ///
    /// # 返回
    ///
    /// 返回转换后的流式数据
    pub async fn convert_stream(
        &mut self,
        input: StreamDataType,
        target_type: &str,
    ) -> ServiceResult<StreamDataType> {
        let start = Instant::now();

        // 根据输入和目标类型执行转换
        let result = match &input {
            StreamDataType::Text(_) if target_type == "Audio" => {
                Err(ServiceError::UnsupportedOperation(
                    "不支持文本流到音频流的转换，文本流只能转换为文本流格式".into(),
                ))
            },
            StreamDataType::Text(_) => {
                // 文本流格式转换
                self.convert_text_stream_to_text_stream(input).await
            },
            StreamDataType::Audio(_) if target_type == "Text" => {
                Err(ServiceError::UnsupportedOperation(
                    "不支持音频流到文本流的转换，音频流只能转换为音频流格式".into(),
                ))
            },
            StreamDataType::Audio(_) => {
                // 音频流格式转换
                self.convert_audio_stream_to_audio_stream(input).await
            },
        };

        debug!("模型流式数据转换完成 ({}ms)", start.elapsed().as_millis());

        result
    }

    /// 文本流格式转换
    ///
    /// 将文本流从一种格式转换为另一种格式
    async fn convert_text_stream_to_text_stream(
        &mut self,
        input: StreamDataType,
    ) -> ServiceResult<StreamDataType> {
        match input {
            StreamDataType::Text(text_stream) => {
                // 这里需要实现文本流格式转换逻辑
                // 例如从Markdown流转换为HTML流
                // 这里只是一个示例实现，直接返回原始文本流
                Ok(StreamDataType::Text(text_stream))
            },
            _ => Err(ServiceError::UnsupportedOperation(
                "输入数据类型不是文本流".into(),
            )),
        }
    }

    /// 音频流格式转换
    ///
    /// 将音频流从一种格式转换为另一种格式
    async fn convert_audio_stream_to_audio_stream(
        &mut self,
        input: StreamDataType,
    ) -> ServiceResult<StreamDataType> {
        if let StreamDataType::Audio(stream) = input {
            let format = stream.format().clone();
            let output_format = format.clone();

            // 获取转换器（使用默认参数）
            let converter = self.get_audio_converter(
                &format,
                &output_format,
                false, // 默认不归一化
                None,  // 默认不调整增益
                None,  // 使用默认处理策略
            );

            // 创建处理流
            let processed_stream = stream.process(converter).await?;

            Ok(StreamDataType::Audio(processed_stream))
        } else {
            Err(ServiceError::InvalidInput("输入数据类型不是音频流".into()))
        }
    }

    // 移除了determine_conversion_strategy方法，直接在convert和convert_stream方法中进行判断
    /// 文本格式转换
    ///
    /// 将文本数据从一种格式转换为另一种格式
    async fn convert_text_to_text(&mut self, input: DataType) -> ServiceResult<DataType> {
        match input {
            DataType::Text { content, mode } => {
                // 这里需要实现文本格式转换逻辑
                // 例如从Markdown转换为HTML
                // 这里只是一个示例实现，直接返回原始文本
                Ok(DataType::Text { content, mode })
            },
            _ => Err(ServiceError::UnsupportedOperation(
                "输入数据类型不是文本".into(),
            )),
        }
    }

    /// 音频格式转换
    ///
    /// 将音频数据从一种格式转换为另一种格式
    async fn convert_audio_to_audio(&mut self, input: DataType) -> ServiceResult<DataType> {
        match input {
            DataType::Audio { format, data, mode } => {
                let start = Instant::now();

                // 获取输出格式（这里简化处理，使用相同格式但可以添加参数控制）
                let output_format = format.clone();

                // 创建音频转换器
                let mut converter = self.get_audio_converter(
                    &format,
                    &output_format,
                    false, // 默认不归一化
                    None,  // 默认不调整增益
                    None,  // 使用默认处理策略
                );

                // 执行转换
                let converted_data = converter.convert(&data).await?;

                debug!(
                    "音频数据转换完成: {} -> {} ({}ms, {}字节 -> {}字节)",
                    format.codec,
                    output_format.codec,
                    start.elapsed().as_millis(),
                    data.len(),
                    converted_data.len()
                );

                Ok(DataType::Audio {
                    format: output_format,
                    data: converted_data,
                    mode,
                })
            },
            _ => Err(ServiceError::UnsupportedOperation(
                "输入数据类型不是音频".into(),
            )),
        }
    }
}

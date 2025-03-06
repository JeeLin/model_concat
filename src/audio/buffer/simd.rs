//! SIMD 加速的音频采样处理
//!
//! 本模块提供使用 SIMD (Single Instruction Multiple Data) 指令集优化的音频采样处理函数。
//! 通过利用现代 CPU 的 SIMD 指令集(如 AVX2、SSE4.1)，可以同时处理多个数据元素，
//! 显著提升音频采样格式转换和混音等操作的性能。
//!
//! # 性能优化
//! - AVX2: 一次可处理 8 个 32 位浮点数
//! - SSE4.1: 一次可处理 4 个 32 位浮点数
//! - 自动回退: 在不支持 SIMD 的平台上自动使用标准实现
//!
//! # 示例
//! ```rust
//! use crate::audio::buffer::simd;
//!
//! // 将 f32 格式音频转换为 i16 格式
//! let f32_samples = vec![0.5f32, -0.8f32, 0.2f32, -0.3f32];
//! let i16_samples = simd::convert_samples_f32_to_i16_simd(&f32_samples);
//!
//! // 混合双声道音频
//! let left = vec![0.5f32, 0.2f32];
//! let right = vec![-0.3f32, 0.4f32];
//! let mixed = simd::mix_channels_simd(&left, &right, 0.7, 0.3);
//! ```

/// 使用 SIMD 指令将 f32 样本转换为 i16 样本
///
/// 对于支持 AVX2 或 SSE4.1 指令集的 x86_64 平台，使用相应的 SIMD 指令加速转换；
/// 对于不支持的平台，回退到标准实现。
pub fn convert_samples_f32_to_i16_simd(samples: &[f32]) -> Vec<i16> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { convert_samples_f32_to_i16_avx2(samples) };
        } else if is_x86_feature_detected!("sse4.1") {
            return unsafe { convert_samples_f32_to_i16_sse41(samples) };
        }
    }

    // 回退到标准实现
    convert_samples_f32_to_i16_standard(samples)
}

/// 标准实现的 f32 到 i16 转换
fn convert_samples_f32_to_i16_standard(samples: &[f32]) -> Vec<i16> {
    samples.iter().map(|&s| (s * 32767.0) as i16).collect()
}

#[cfg(target_arch = "x86_64")]
unsafe fn convert_samples_f32_to_i16_avx2(samples: &[f32]) -> Vec<i16> {
    let len = samples.len();
    let mut result = Vec::with_capacity(len);
    result.set_len(len); // 预分配空间但不初始化

    // AVX2 一次处理 8 个 f32 值
    let simd_len = len / 8 * 8;
    let scale = _mm256_set1_ps(32767.0);

    for i in (0..simd_len).step_by(8) {
        // 加载 8 个 f32 值
        let v = _mm256_loadu_ps(&samples[i] as *const f32);

        // 乘以缩放因子
        let scaled = _mm256_mul_ps(v, scale);

        // 转换为 i32
        let i32_values = _mm256_cvtps_epi32(scaled);

        // 将 8 个 i32 值打包为 8 个 i16 值
        let i16_values_low = _mm256_extracti128_si256(i32_values, 0);
        let i16_values_high = _mm256_extracti128_si256(i32_values, 1);
        let packed = _mm_packs_epi32(i16_values_low, i16_values_high);

        // 存储结果
        _mm_storeu_si128(result.as_mut_ptr().add(i) as *mut __m128i, packed);
    }

    // 处理剩余的元素
    for i in simd_len..len {
        result[i] = (samples[i] * 32767.0) as i16;
    }

    result
}

#[cfg(target_arch = "x86_64")]
unsafe fn convert_samples_f32_to_i16_sse41(samples: &[f32]) -> Vec<i16> {
    let len = samples.len();
    let mut result = Vec::with_capacity(len);
    result.set_len(len); // 预分配空间但不初始化

    // SSE4.1 一次处理 4 个 f32 值
    let simd_len = len / 4 * 4;
    let scale = _mm_set1_ps(32767.0);

    for i in (0..simd_len).step_by(4) {
        // 加载 4 个 f32 值
        let v = _mm_loadu_ps(&samples[i] as *const f32);

        // 乘以缩放因子
        let scaled = _mm_mul_ps(v, scale);

        // 转换为 i32
        let i32_values = _mm_cvtps_epi32(scaled);

        // 将 4 个 i32 值打包为 4 个 i16 值
        let packed = _mm_packs_epi32(i32_values, _mm_setzero_si128());

        // 存储结果
        _mm_storel_epi64(result.as_mut_ptr().add(i) as *mut __m128i, packed);
    }

    // 处理剩余的元素
    for i in simd_len..len {
        result[i] = (samples[i] * 32767.0) as i16;
    }

    result
}

/// 使用 SIMD 指令将 i16 样本转换为 f32 样本
pub fn convert_samples_i16_to_f32_simd(samples: &[i16]) -> Vec<f32> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { convert_samples_i16_to_f32_avx2(samples) };
        } else if is_x86_feature_detected!("sse4.1") {
            return unsafe { convert_samples_i16_to_f32_sse41(samples) };
        }
    }

    // 回退到标准实现
    convert_samples_i16_to_f32_standard(samples)
}

/// 标准实现的 i16 到 f32 转换
fn convert_samples_i16_to_f32_standard(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32767.0).collect()
}

#[cfg(target_arch = "x86_64")]
unsafe fn convert_samples_i16_to_f32_avx2(samples: &[i16]) -> Vec<f32> {
    let len = samples.len();
    let mut result = Vec::with_capacity(len);
    result.set_len(len); // 预分配空间但不初始化

    // AVX2 一次处理 8 个 i16 值
    let simd_len = len / 8 * 8;
    let scale = _mm256_set1_ps(1.0 / 32767.0);

    for i in (0..simd_len).step_by(8) {
        // 加载 8 个 i16 值
        let v = _mm_loadu_si128(samples.as_ptr().add(i) as *const __m128i);

        // 转换为 i32
        let i32_low = _mm_cvtepi16_epi32(v);
        let i32_high = _mm_cvtepi16_epi32(_mm_srli_si128(v, 8));

        // 转换为 f32
        let f32_low = _mm256_cvtepi32_ps(_mm256_castsi128_si256(i32_low));
        let f32_high = _mm256_cvtepi32_ps(_mm256_castsi128_si256(i32_high));

        // 乘以缩放因子
        let scaled_low = _mm256_mul_ps(f32_low, scale);
        let scaled_high = _mm256_mul_ps(f32_high, scale);

        // 存储结果
        _mm256_storeu_ps(result.as_mut_ptr().add(i), scaled_low);
        _mm256_storeu_ps(result.as_mut_ptr().add(i + 4), scaled_high);
    }

    // 处理剩余的元素
    for i in simd_len..len {
        result[i] = samples[i] as f32 / 32767.0;
    }

    result
}

#[cfg(target_arch = "x86_64")]
unsafe fn convert_samples_i16_to_f32_sse41(samples: &[i16]) -> Vec<f32> {
    let len = samples.len();
    let mut result = Vec::with_capacity(len);
    result.set_len(len); // 预分配空间但不初始化

    // SSE4.1 一次处理 4 个 i16 值
    let simd_len = len / 4 * 4;
    let scale = _mm_set1_ps(1.0 / 32767.0);

    for i in (0..simd_len).step_by(4) {
        // 加载 4 个 i16 值
        let v = _mm_loadl_epi64(samples.as_ptr().add(i) as *const __m128i);

        // 转换为 i32
        let i32_values = _mm_cvtepi16_epi32(v);

        // 转换为 f32
        let f32_values = _mm_cvtepi32_ps(i32_values);

        // 乘以缩放因子
        let scaled = _mm_mul_ps(f32_values, scale);

        // 存储结果
        _mm_storeu_ps(result.as_mut_ptr().add(i), scaled);
    }

    // 处理剩余的元素
    for i in simd_len..len {
        result[i] = samples[i] as f32 / 32767.0;
    }

    result
}

/// 使用 SIMD 指令混合两个 f32 音频通道
///
/// 将两个音频通道按照给定的权重混合成一个通道
pub fn mix_channels_simd(
    left: &[f32],
    right: &[f32],
    left_weight: f32,
    right_weight: f32,
) -> Vec<f32> {
    assert_eq!(left.len(), right.len(), "通道长度必须相同");

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { mix_channels_avx2(left, right, left_weight, right_weight) };
        } else if is_x86_feature_detected!("sse4.1") {
            return unsafe { mix_channels_sse41(left, right, left_weight, right_weight) };
        }
    }

    // 回退到标准实现
    mix_channels_standard(left, right, left_weight, right_weight)
}

/// 标准实现的通道混合
fn mix_channels_standard(
    left: &[f32],
    right: &[f32],
    left_weight: f32,
    right_weight: f32,
) -> Vec<f32> {
    left.iter()
        .zip(right.iter())
        .map(|(&l, &r)| l * left_weight + r * right_weight)
        .collect()
}

#[cfg(target_arch = "x86_64")]
unsafe fn mix_channels_avx2(
    left: &[f32],
    right: &[f32],
    left_weight: f32,
    right_weight: f32,
) -> Vec<f32> {
    let len = left.len();
    let mut result = Vec::with_capacity(len);
    result.set_len(len); // 预分配空间但不初始化

    // AVX2 一次处理 8 个 f32 值
    let simd_len = len / 8 * 8;
    let left_scale = _mm256_set1_ps(left_weight);
    let right_scale = _mm256_set1_ps(right_weight);

    for i in (0..simd_len).step_by(8) {
        // 加载 8 个 f32 值
        let l = _mm256_loadu_ps(&left[i] as *const f32);
        let r = _mm256_loadu_ps(&right[i] as *const f32);

        // 乘以权重并相加
        let l_scaled = _mm256_mul_ps(l, left_scale);
        let r_scaled = _mm256_mul_ps(r, right_scale);
        let mixed = _mm256_add_ps(l_scaled, r_scaled);

        // 存储结果
        _mm256_storeu_ps(result.as_mut_ptr().add(i), mixed);
    }

    // 处理剩余的元素
    for i in simd_len..len {
        result[i] = left[i] * left_weight + right[i] * right_weight;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_f32_to_i16_conversion() {
        let input = vec![
            0.5f32, -0.8f32, 0.2f32, -0.3f32, 0.1f32, -0.4f32, 0.6f32, -0.7f32,
        ];
        let expected: Vec<i16> = input.iter().map(|&x| (x * 32767.0) as i16).collect();
        let result = convert_samples_f32_to_i16_simd(&input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_i16_to_f32_conversion() {
        let input = vec![16384i16, -24576, 6554, -9830, 3277, -13107, 19661, -22938];
        let expected: Vec<f32> = input.iter().map(|&x| x as f32 / 32767.0).collect();
        let result = convert_samples_i16_to_f32_simd(&input);
        for (a, b) in result.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_mix_channels() {
        let left = vec![0.5f32, 0.2f32, -0.3f32, 0.4f32];
        let right = vec![-0.3f32, 0.4f32, 0.5f32, -0.2f32];
        let left_weight = 0.7f32;
        let right_weight = 0.3f32;

        let result = mix_channels_simd(&left, &right, left_weight, right_weight);
        let expected: Vec<f32> = left
            .iter()
            .zip(right.iter())
            .map(|(&l, &r)| l * left_weight + r * right_weight)
            .collect();

        for (a, b) in result.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_simd_performance() {
        let size = 1_000_000;
        let input_f32: Vec<f32> = (0..size)
            .map(|i| (i as f32 / size as f32) * 2.0 - 1.0)
            .collect();

        // 测试 f32 到 i16 的转换性能
        let start = Instant::now();
        let _ = convert_samples_f32_to_i16_standard(&input_f32);
        let standard_time = start.elapsed();

        let start = Instant::now();
        let _ = convert_samples_f32_to_i16_simd(&input_f32);
        let simd_time = start.elapsed();

        println!(
            "f32->i16 Standard: {:?}, SIMD: {:?}",
            standard_time, simd_time
        );
        assert!(simd_time <= standard_time);
    }

    #[test]
    fn test_edge_cases() {
        // 测试边界值
        let input_f32 = vec![1.0f32, -1.0f32, 0.0f32];
        let result = convert_samples_f32_to_i16_simd(&input_f32);
        assert_eq!(result, vec![32767, -32767, 0]);

        // 测试空输入
        let empty: Vec<f32> = vec![];
        let result = convert_samples_f32_to_i16_simd(&empty);
        assert!(result.is_empty());

        // 测试非对齐长度
        let input_odd = vec![0.5f32, -0.5f32, 0.1f32];
        let result = convert_samples_f32_to_i16_simd(&input_odd);
        assert_eq!(result.len(), 3);
    }
}

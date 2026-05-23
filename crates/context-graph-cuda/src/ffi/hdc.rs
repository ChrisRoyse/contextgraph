//! FFI bindings for GPU HDC projection.
//!
//! The E9 HDC embedder hashes output vectors into durable cache rows. Unicode
//! text is decoded to scalar values on the host, CUDA computes the base HDC
//! projection with integer bit operations, and Rust applies the same f32
//! normalization plus bounded text-identity residual as the CPU implementation.

use std::ffi::c_void;

use crate::error::{CudaError, CudaResult};
use crate::ffi::cuda_driver::CUDA_SUCCESS;
use crate::ffi::knn::{ensure_cuda_initialized, CudaContext, CudaModule, GpuBuffer};

include!(concat!(env!("OUT_DIR"), "/hdc_image.rs"));

const BLOCK_SIZE: u32 = 256;
pub const HDC_DIMENSION: usize = 10_000;
pub const HDC_PROJECTED_DIMENSION: usize = 1024;
const TEXT_IDENTITY_RESIDUAL_WEIGHT: f32 = 1.0 / 16.0;
const TEXT_IDENTITY_RESIDUAL_DOMAIN: u64 = 0xD6E8_FD9A_4F1C_2B73;
const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

#[link(name = "cuda")]
extern "C" {
    fn cuLaunchKernel(
        f: *mut c_void,
        gridDimX: u32,
        gridDimY: u32,
        gridDimZ: u32,
        blockDimX: u32,
        blockDimY: u32,
        blockDimZ: u32,
        sharedMemBytes: u32,
        hStream: *mut c_void,
        kernelParams: *mut *mut c_void,
        extra: *mut *mut c_void,
    ) -> i32;

    fn cuCtxSynchronize() -> i32;
}

/// Compute exact E9 HDC vectors on GPU for a batch of text rows.
///
/// Empty batches, empty/whitespace rows, invalid n-gram sizes, and dimensions
/// that would overflow CUDA kernel parameters fail closed before launch.
pub fn compute_hdc_embeddings_gpu(
    texts: &[&str],
    seed: u64,
    ngram_size: usize,
) -> CudaResult<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Err(CudaError::InvalidArgument {
            argument: "texts".to_string(),
            reason: "HDC GPU batch is empty".to_string(),
        });
    }
    if ngram_size == 0 || ngram_size > i32::MAX as usize {
        return Err(CudaError::InvalidArgument {
            argument: "ngram_size".to_string(),
            reason: format!("ngram_size must be in 1..=i32::MAX, got {ngram_size}"),
        });
    }
    if texts.len() > (i32::MAX as usize / HDC_PROJECTED_DIMENSION) {
        return Err(CudaError::InvalidArgument {
            argument: "texts".to_string(),
            reason: format!(
                "row count {} would overflow kernel work item count",
                texts.len()
            ),
        });
    }

    let mut chars = Vec::<u32>::new();
    let mut offsets = Vec::<i32>::with_capacity(texts.len());
    let mut lengths = Vec::<i32>::with_capacity(texts.len());

    for (row_index, text) in texts.iter().enumerate() {
        if text.trim().is_empty() {
            return Err(CudaError::InvalidArgument {
                argument: format!("texts[{row_index}]"),
                reason: "HDC GPU row is empty or whitespace-only".to_string(),
            });
        }
        if chars.len() > i32::MAX as usize {
            return Err(CudaError::InvalidArgument {
                argument: "texts".to_string(),
                reason: "HDC GPU codepoint offset exceeds i32::MAX".to_string(),
            });
        }
        offsets.push(chars.len() as i32);
        let row_chars = text.chars().map(u32::from).collect::<Vec<_>>();
        if row_chars.is_empty() || row_chars.len() > i32::MAX as usize {
            return Err(CudaError::InvalidArgument {
                argument: format!("texts[{row_index}]"),
                reason: format!("invalid decoded char length {}", row_chars.len()),
            });
        }
        lengths.push(row_chars.len() as i32);
        chars.extend(row_chars);
    }

    ensure_cuda_initialized()?;
    let _ctx = CudaContext::new(0)?;
    let module = CudaModule::load_image(IMAGE)?;
    let kernel = module.get_function("compute_hdc_projection_kernel")?;

    let chars_size = std::mem::size_of_val(chars.as_slice());
    let offsets_size = std::mem::size_of_val(offsets.as_slice());
    let lengths_size = std::mem::size_of_val(lengths.as_slice());
    let output_len = texts
        .len()
        .checked_mul(HDC_PROJECTED_DIMENSION)
        .ok_or_else(|| CudaError::InvalidArgument {
            argument: "texts".to_string(),
            reason: "output length overflow".to_string(),
        })?;
    let output_size = output_len * std::mem::size_of::<f32>();

    let d_chars = GpuBuffer::new(chars_size)?;
    let d_offsets = GpuBuffer::new(offsets_size)?;
    let d_lengths = GpuBuffer::new(lengths_size)?;
    let d_output = GpuBuffer::new(output_size)?;

    let chars_bytes =
        unsafe { std::slice::from_raw_parts(chars.as_ptr() as *const u8, chars_size) };
    let offsets_bytes =
        unsafe { std::slice::from_raw_parts(offsets.as_ptr() as *const u8, offsets_size) };
    let lengths_bytes =
        unsafe { std::slice::from_raw_parts(lengths.as_ptr() as *const u8, lengths_size) };
    d_chars.copy_from_host(chars_bytes)?;
    d_offsets.copy_from_host(offsets_bytes)?;
    d_lengths.copy_from_host(lengths_bytes)?;

    let row_count_i32 = texts.len() as i32;
    let ngram_size_i32 = ngram_size as i32;
    let d_chars_ptr = d_chars.ptr();
    let d_offsets_ptr = d_offsets.ptr();
    let d_lengths_ptr = d_lengths.ptr();
    let d_output_ptr = d_output.ptr();

    let mut params: [*mut c_void; 7] = [
        &d_chars_ptr as *const _ as *mut c_void,
        &d_offsets_ptr as *const _ as *mut c_void,
        &d_lengths_ptr as *const _ as *mut c_void,
        &row_count_i32 as *const _ as *mut c_void,
        &seed as *const _ as *mut c_void,
        &ngram_size_i32 as *const _ as *mut c_void,
        &d_output_ptr as *const _ as *mut c_void,
    ];

    let total_items = output_len as u32;
    let num_blocks = total_items.div_ceil(BLOCK_SIZE);
    let ret = unsafe {
        cuLaunchKernel(
            kernel,
            num_blocks,
            1,
            1,
            BLOCK_SIZE,
            1,
            1,
            0,
            std::ptr::null_mut(),
            params.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    if ret != CUDA_SUCCESS {
        return Err(CudaError::CudaRuntimeError {
            operation: "cuLaunchKernel(compute_hdc_projection_kernel)".to_string(),
            code: ret,
        });
    }

    let ret = unsafe { cuCtxSynchronize() };
    if ret != CUDA_SUCCESS {
        return Err(CudaError::CudaRuntimeError {
            operation: "cuCtxSynchronize".to_string(),
            code: ret,
        });
    }

    let mut flat = vec![0.0f32; output_len];
    let output_bytes =
        unsafe { std::slice::from_raw_parts_mut(flat.as_mut_ptr() as *mut u8, output_size) };
    d_output.copy_to_host(output_bytes)?;

    for (row, text) in flat.chunks_mut(HDC_PROJECTED_DIMENSION).zip(texts.iter()) {
        normalize_projected_vector(row);
        apply_text_identity_residual(row, seed, text);
    }

    Ok(flat
        .chunks(HDC_PROJECTED_DIMENSION)
        .map(|row| row.to_vec())
        .collect())
}

fn apply_text_identity_residual(vector: &mut [f32], seed: u64, text: &str) {
    if vector.is_empty() {
        return;
    }
    let identity = text_identity_hash(seed, text);
    let inv_sqrt_dim = 1.0 / (vector.len() as f32).sqrt();
    for (idx, value) in vector.iter_mut().enumerate() {
        let bit = splitmix64(identity ^ (idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let residual = if bit & 1 == 0 {
            -inv_sqrt_dim
        } else {
            inv_sqrt_dim
        };
        *value += TEXT_IDENTITY_RESIDUAL_WEIGHT * residual;
    }
    normalize_projected_vector(vector);
}

fn normalize_projected_vector(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

fn text_identity_hash(seed: u64, text: &str) -> u64 {
    let mut hash =
        FNV_OFFSET_BASIS ^ seed.wrapping_mul(TEXT_IDENTITY_RESIDUAL_DOMAIN) ^ (text.len() as u64);
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

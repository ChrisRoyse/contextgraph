//! GPU HDC projection kernel for ME-JEPA E9.
//!
//! This kernel computes the exact raw 1024D HDC projection for UTF-8 text that
//! has already been decoded to Unicode scalar values on the host. The expensive
//! deterministic bit math runs on GPU; L2 normalization remains on the Rust side
//! so durable cache vector hashes use the same f32 reduction as the CPU path.

#include <stdint.h>

#define HDC_DIMENSION 10000
#define HDC_PROJECTED_DIMENSION 1024
#define BLOCK_SIZE 256
#define LCG_MULTIPLIER 6364136223846793005ULL

__device__ __forceinline__ uint64_t lcg_advance(uint64_t state, uint64_t steps) {
    uint64_t acc_mult = 1ULL;
    uint64_t acc_plus = 0ULL;
    uint64_t cur_mult = LCG_MULTIPLIER;
    uint64_t cur_plus = 1ULL;

    while (steps > 0ULL) {
        if ((steps & 1ULL) != 0ULL) {
            acc_plus = acc_plus * cur_mult + cur_plus;
            acc_mult = acc_mult * cur_mult;
        }
        cur_plus = cur_plus * (cur_mult + 1ULL);
        cur_mult = cur_mult * cur_mult;
        steps >>= 1ULL;
    }

    return acc_mult * state + acc_plus;
}

__device__ __forceinline__ bool random_hypervector_bit(
    uint64_t seed,
    uint64_t key,
    int bit_index
) {
    uint64_t initial = seed + key;
    uint64_t state = lcg_advance(initial, (uint64_t)bit_index + 1ULL);
    return (state >> 63) == 1ULL;
}

__device__ __forceinline__ bool ngram_bit(
    const uint32_t* __restrict__ chars,
    int row_offset,
    int window_start,
    int effective_ngram,
    uint64_t seed,
    int bit_index
) {
    bool bit = random_hypervector_bit(
        seed,
        (uint64_t)chars[row_offset + window_start],
        bit_index
    );

    for (int pos = 1; pos < effective_ngram; pos++) {
        int shifted_bit = bit_index + pos;
        if (shifted_bit >= HDC_DIMENSION) {
            shifted_bit -= HDC_DIMENSION;
        }
        bit ^= random_hypervector_bit(
            seed,
            (uint64_t)chars[row_offset + window_start + pos],
            shifted_bit
        );
    }

    return bit;
}

extern "C" __global__ void compute_hdc_projection_kernel(
    const uint32_t* __restrict__ chars,
    const int* __restrict__ offsets,
    const int* __restrict__ lengths,
    int row_count,
    uint64_t seed,
    int ngram_size,
    float* __restrict__ output
) {
    int tid = blockIdx.x * blockDim.x + threadIdx.x;
    int total = row_count * HDC_PROJECTED_DIMENSION;
    if (tid >= total) return;

    int row = tid / HDC_PROJECTED_DIMENSION;
    int out_dim = tid - (row * HDC_PROJECTED_DIMENSION);
    int char_len = lengths[row];
    int row_offset = offsets[row];

    if (char_len <= 0) {
        output[tid] = 0.0f;
        return;
    }

    int effective_ngram = ngram_size < char_len ? ngram_size : char_len;
    int window_count = char_len - effective_ngram + 1;

    int start = (out_dim * HDC_DIMENSION) / HDC_PROJECTED_DIMENSION;
    int end = ((out_dim + 1) * HDC_DIMENSION) / HDC_PROJECTED_DIMENSION;
    int chunk_size = end - start;
    int ones = 0;

    for (int bit_index = start; bit_index < end; bit_index++) {
        int count = 0;
        bool first_vector_bit = false;

        for (int window_start = 0; window_start < window_count; window_start++) {
            bool bit = ngram_bit(
                chars,
                row_offset,
                window_start,
                effective_ngram,
                seed,
                bit_index
            );
            if (window_start == 0) {
                first_vector_bit = bit;
            }
            count += bit ? 1 : 0;
        }

        int threshold = window_count / 2;
        bool bundled_bit = (count > threshold) || (count == threshold && first_vector_bit);
        ones += bundled_bit ? 1 : 0;
    }

    output[tid] = (2.0f * (float)ones / (float)chunk_size) - 1.0f;
}

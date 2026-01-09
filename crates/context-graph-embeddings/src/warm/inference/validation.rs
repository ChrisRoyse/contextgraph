//! Output validation for inference results.
//!
//! Implements Constitution AP-007 compliant validation:
//! - Sin wave pattern detection (exit 110)
//! - Golden similarity verification (threshold > 0.99)
//!
//! # Error Codes
//!
//! - EMB-E010: Sin wave output detected (fake/mock inference)
//! - EMB-E011: Golden similarity too low
//!
//! # Design Philosophy
//!
//! FAIL FAST. NO FALLBACKS. REAL INFERENCE ONLY.

use crate::warm::cuda_alloc::{GOLDEN_SIMILARITY_THRESHOLD, SIN_WAVE_ENERGY_THRESHOLD};
use crate::warm::error::{WarmError, WarmResult};
use tracing::{error, info, warn};

/// Detect sin wave patterns in output embeddings.
///
/// Sin wave patterns indicate mock/fake inference because real embedding
/// models produce complex, non-periodic output distributions.
///
/// # Algorithm
///
/// 1. Compute autocorrelation at various lags
/// 2. Detect periodic patterns via autocorrelation peaks
/// 3. Estimate energy concentration in dominant frequency
/// 4. If >80% energy in single frequency → sin wave detected
///
/// # Arguments
///
/// * `output` - The embedding output vector to analyze
///
/// # Returns
///
/// `(bool, f32, f32)` - (is_sin_wave, dominant_frequency_estimate, energy_concentration)
///
/// # Constitution Compliance
///
/// AP-007: Sin wave outputs MUST cause exit(110).
pub fn detect_sin_wave_pattern(output: &[f32]) -> (bool, f32, f32) {
    if output.len() < 16 {
        // Too short for meaningful frequency analysis
        return (false, 0.0, 0.0);
    }

    // Step 1: Normalize the output
    let mean: f32 = output.iter().sum::<f32>() / output.len() as f32;
    let normalized: Vec<f32> = output.iter().map(|x| x - mean).collect();

    // Step 2: Compute autocorrelation for various lags
    // Sin waves have high autocorrelation at multiples of their period
    let max_lag = output.len().min(256);
    let mut autocorr = Vec::with_capacity(max_lag);

    // Zero-lag autocorrelation (variance)
    let variance: f32 = normalized.iter().map(|x| x * x).sum();
    if variance < 1e-10 {
        // Near-constant signal - not a sin wave
        return (false, 0.0, 0.0);
    }

    for lag in 0..max_lag {
        let mut sum = 0.0f32;
        let n = output.len() - lag;
        for i in 0..n {
            sum += normalized[i] * normalized[i + lag];
        }
        autocorr.push(sum / variance);
    }

    // Step 3: Find peaks in autocorrelation (excluding lag 0)
    // A sin wave has peaks at multiples of its period
    let mut peak_lags = Vec::new();
    for i in 2..(autocorr.len() - 1) {
        if autocorr[i] > autocorr[i - 1] && autocorr[i] > autocorr[i + 1] && autocorr[i] > 0.5 {
            peak_lags.push((i, autocorr[i]));
        }
    }

    // Step 4: Check for periodic pattern
    // If there are multiple evenly-spaced peaks with high correlation, it's a sin wave
    if peak_lags.len() >= 2 {
        // Check if peaks are evenly spaced (characteristic of sin wave)
        let first_peak_lag = peak_lags[0].0;
        let mut is_periodic = true;
        let mut max_peak_corr: f32 = 0.0;

        for (i, (lag, corr)) in peak_lags.iter().enumerate() {
            let expected_lag = first_peak_lag * (i + 1);
            let tolerance = first_peak_lag / 4;
            if (*lag as i32 - expected_lag as i32).unsigned_abs() > tolerance as u32 {
                is_periodic = false;
                break;
            }
            max_peak_corr = max_peak_corr.max(*corr);
        }

        if is_periodic && max_peak_corr > SIN_WAVE_ENERGY_THRESHOLD {
            // Estimate dominant frequency from first peak lag
            let dominant_freq = 1.0 / (first_peak_lag as f32);
            // Energy concentration is approximated by the max autocorrelation
            let energy_concentration = max_peak_corr;

            return (true, dominant_freq, energy_concentration);
        }
    }

    // Step 5: Additional check - direct sin wave fitting
    // Try to fit y = A * sin(2π * f * x + φ) and measure residual
    let best_fit = find_best_sinusoid_fit(&normalized);
    if best_fit.2 > SIN_WAVE_ENERGY_THRESHOLD {
        return (true, best_fit.0, best_fit.2);
    }

    (false, 0.0, 0.0)
}

/// Try to fit a sinusoid to the data and return quality metrics.
///
/// Returns (frequency_estimate, amplitude_estimate, r_squared)
fn find_best_sinusoid_fit(data: &[f32]) -> (f32, f32, f32) {
    // Test several candidate frequencies
    let n = data.len() as f32;
    let mut best_rsq: f32 = 0.0;
    let mut best_freq: f32 = 0.0;
    let mut best_amp: f32 = 0.0;

    // Test frequencies from 0.01 to 0.5 (Nyquist limit)
    let freq_steps = 50;
    for step in 1..freq_steps {
        let freq = step as f32 / (freq_steps as f32 * 2.0); // 0.01 to 0.5

        // Compute sin and cos components at this frequency
        let mut sum_sin = 0.0f32;
        let mut sum_cos = 0.0f32;
        let two_pi_f = 2.0 * std::f32::consts::PI * freq;

        for (i, &val) in data.iter().enumerate() {
            let angle = two_pi_f * (i as f32);
            sum_sin += val * angle.sin();
            sum_cos += val * angle.cos();
        }

        // Amplitude estimate
        let amplitude = 2.0 * (sum_sin * sum_sin + sum_cos * sum_cos).sqrt() / n;

        // Phase estimate
        let phase = sum_cos.atan2(sum_sin);

        // Compute R-squared (coefficient of determination)
        let total_var: f32 = data.iter().map(|x| x * x).sum();
        if total_var < 1e-10 {
            continue;
        }

        let mut residual_var = 0.0f32;
        for (i, &val) in data.iter().enumerate() {
            let predicted = amplitude * (two_pi_f * (i as f32) + phase).sin();
            let residual = val - predicted;
            residual_var += residual * residual;
        }

        let r_squared = 1.0 - (residual_var / total_var).max(0.0);

        if r_squared > best_rsq {
            best_rsq = r_squared;
            best_freq = freq;
            best_amp = amplitude;
        }
    }

    (best_freq, best_amp, best_rsq)
}

/// Compute cosine similarity between two vectors.
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector (golden reference)
///
/// # Returns
///
/// Cosine similarity in range [-1, 1], or 0.0 if either vector is zero.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

/// Validate inference output against Constitution AP-007 requirements.
///
/// # Validation Steps
///
/// 1. Check for sin wave patterns → exit(110) if detected
/// 2. Compare against golden reference → exit(107) if similarity < 0.99
/// 3. Return InferenceValidation on success
///
/// # Arguments
///
/// * `model_id` - Model identifier for error reporting
/// * `output` - The inference output embedding vector
/// * `golden` - Optional golden reference for similarity check
///
/// # Returns
///
/// `Ok(())` if validation passes.
///
/// # Errors
///
/// - `WarmError::SinWaveOutputDetected` (exit 110) if sin wave pattern detected
/// - `WarmError::ModelValidationFailed` (exit 103) if golden similarity < 0.99
///
/// # Constitution Compliance
///
/// AP-007: Sin wave outputs MUST cause exit(110).
/// Golden similarity must exceed 0.99 threshold.
pub fn validate_inference_output_ap007(
    model_id: &str,
    output: &[f32],
    golden: Option<&[f32]>,
) -> WarmResult<()> {
    // Step 1: Detect sin wave patterns (AP-007)
    let (is_sin_wave, dominant_freq, energy_conc) = detect_sin_wave_pattern(output);

    if is_sin_wave {
        error!(
            target: "warm::inference",
            code = "EMB-E010",
            model_id = %model_id,
            dominant_frequency_hz = dominant_freq,
            energy_concentration = energy_conc,
            output_size = output.len(),
            "[CONSTITUTION AP-007 VIOLATION] Sin wave pattern detected in output - FAKE INFERENCE"
        );

        return Err(WarmError::SinWaveOutputDetected {
            model_id: model_id.to_string(),
            dominant_frequency_hz: dominant_freq,
            energy_concentration: energy_conc,
            output_size: output.len(),
        });
    }

    info!(
        target: "warm::inference",
        code = "EMB-I010",
        model_id = %model_id,
        "Sin wave detection passed - output is not synthetic"
    );

    // Step 2: Golden similarity check (if golden reference provided)
    if let Some(golden_ref) = golden {
        let similarity = cosine_similarity(output, golden_ref);

        if similarity < GOLDEN_SIMILARITY_THRESHOLD {
            warn!(
                target: "warm::inference",
                code = "EMB-E011",
                model_id = %model_id,
                similarity = similarity,
                threshold = GOLDEN_SIMILARITY_THRESHOLD,
                "Golden similarity below threshold"
            );

            return Err(WarmError::ModelValidationFailed {
                model_id: model_id.to_string(),
                reason: format!(
                    "Golden similarity {:.4} below threshold {:.4}",
                    similarity, GOLDEN_SIMILARITY_THRESHOLD
                ),
                expected_output: Some(format!("similarity >= {}", GOLDEN_SIMILARITY_THRESHOLD)),
                actual_output: Some(format!("similarity = {:.4}", similarity)),
            });
        }

        info!(
            target: "warm::inference",
            code = "EMB-I011",
            model_id = %model_id,
            similarity = similarity,
            threshold = GOLDEN_SIMILARITY_THRESHOLD,
            "Golden similarity validation passed"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_pure_sin_wave() {
        // Generate a pure sin wave
        let n = 256;
        let freq = 0.1; // 10% of sampling rate
        let output: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32).sin())
            .collect();

        let (is_sin_wave, dominant_freq, energy_conc) = detect_sin_wave_pattern(&output);

        assert!(is_sin_wave, "Pure sin wave should be detected");
        assert!(
            (dominant_freq - freq).abs() < 0.05,
            "Frequency should be close to 0.1, got {}",
            dominant_freq
        );
        assert!(
            energy_conc > 0.8,
            "Energy concentration should be high, got {}",
            energy_conc
        );
    }

    #[test]
    fn test_detect_random_noise_not_sin_wave() {
        // Generate random noise using deterministic seed
        let n = 256;
        let mut output = Vec::with_capacity(n);
        let mut state = 12345u64; // Simple LCG seed

        for _ in 0..n {
            // Simple LCG for deterministic pseudo-random
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let val = ((state >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            output.push(val);
        }

        let (is_sin_wave, _, _) = detect_sin_wave_pattern(&output);

        assert!(
            !is_sin_wave,
            "Random noise should NOT be detected as sin wave"
        );
    }

    #[test]
    fn test_detect_real_embedding_not_sin_wave() {
        // Simulate a realistic embedding (values clustered around certain ranges)
        let n = 768; // Typical embedding dimension
        let mut output = Vec::with_capacity(n);
        let mut state = 54321u64;

        for i in 0..n {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let base = ((state >> 33) as f32) / (u32::MAX as f32) * 0.4 - 0.2;
            // Add some structure but not periodic
            let val = base + 0.1 * (i as f32 / 100.0).powf(0.5).sin();
            output.push(val);
        }

        let (is_sin_wave, _, _) = detect_sin_wave_pattern(&output);

        assert!(
            !is_sin_wave,
            "Realistic embedding should NOT be detected as sin wave"
        );
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];

        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![-1.0, -2.0, -3.0, -4.0];

        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "Opposite vectors should have similarity -1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let sim = cosine_similarity(&a, &b);
        assert!(
            sim.abs() < 1e-6,
            "Orthogonal vectors should have similarity ~0"
        );
    }

    #[test]
    fn test_validate_ap007_sin_wave_fails() {
        // Generate a sin wave that should trigger exit(110)
        let n = 768;
        let freq = 0.1;
        let output: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32).sin())
            .collect();

        let result = validate_inference_output_ap007("test_model", &output, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            WarmError::SinWaveOutputDetected { model_id, .. } => {
                assert_eq!(model_id, "test_model");
            }
            other => panic!("Expected SinWaveOutputDetected, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_ap007_low_golden_similarity_fails() {
        // Generate random output
        let n = 768;
        let mut state = 11111u64;
        let output: Vec<f32> = (0..n)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((state >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();

        // Generate different golden reference
        let mut state2 = 22222u64;
        let golden: Vec<f32> = (0..n)
            .map(|_| {
                state2 = state2.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((state2 >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();

        let result = validate_inference_output_ap007("test_model", &output, Some(&golden));

        assert!(result.is_err());
        match result.unwrap_err() {
            WarmError::ModelValidationFailed { model_id, .. } => {
                assert_eq!(model_id, "test_model");
            }
            other => panic!("Expected ModelValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_ap007_passes_with_similar_output() {
        // Generate output
        let n = 768;
        let mut state = 33333u64;
        let output: Vec<f32> = (0..n)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((state >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();

        // Golden is same as output (100% similarity)
        let golden = output.clone();

        let result = validate_inference_output_ap007("test_model", &output, Some(&golden));

        assert!(
            result.is_ok(),
            "Should pass with identical golden reference"
        );
    }
}

//! GPU Inference Engine for Warm Model Validation.
//!
//! This module provides real GPU inference capabilities for validating
//! warm-loaded embedding models. Per Constitution AP-007, NO FAKE DATA.
//!
//! # Error Codes
//!
//! - EMB-E010: Sin wave output detected (fake/mock inference)
//! - EMB-E011: Inference failures (both init and execution)
//!
//! # Design Philosophy
//!
//! FAIL FAST. NO FALLBACKS. REAL INFERENCE ONLY.

mod engine;
mod validation;

pub use engine::InferenceEngine;
pub use validation::{cosine_similarity, detect_sin_wave_pattern, validate_inference_output_ap007};

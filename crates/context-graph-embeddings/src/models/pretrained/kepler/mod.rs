//! KEPLER entity embedding model.
//!
//! KEPLER (Knowledge Embedding and Pre-training for Language Representation) combines
//! RoBERTa-base with TransE training on Wikidata5M to produce embeddings where
//! TransE operations are semantically meaningful.
//!
//! Paper: https://arxiv.org/abs/1911.06136
//!
//! # Key Differences from all-MiniLM-L6-v2 (previous E11)
//!
//! | Property | MiniLM (old E11) | KEPLER (new E11) |
//! |----------|------------------|------------------|
//! | Base Model | BERT distilled | RoBERTa-base |
//! | Dimension | 384D | 768D |
//! | Layers | 6 | 12 |
//! | TransE Training | None | Wikidata5M (4.8M entities, 20M triples) |
//! | TransE Semantics | Meaningless | Trained with `h + r ≈ t` |
//!
//! # Why KEPLER?
//!
//! The previous E11 (all-MiniLM-L6-v2) was a generic sentence similarity model.
//! Applying TransE operations (`h + r - t`) was mathematically valid but
//! semantically meaningless because the model wasn't trained for it.
//!
//! KEPLER was specifically trained on Wikidata5M with the TransE objective:
//! - Valid triples: `||h + r - t||₂` is small
//! - Invalid triples: `||h + r - t||₂` is large
//!
//! This means TransE operations now produce meaningful results.
//!
//! # GPU Acceleration
//!
//! Uses GPU-accelerated RoBERTa inference via Candle:
//! 1. Tokenization with RoBERTa tokenizer (GPT-2 BPE)
//! 2. GPU embedding lookup and position encoding
//! 3. GPU-accelerated transformer forward pass (12 layers)
//! 4. Mean pooling over sequence dimension
//! 5. L2 normalization on GPU
//!
//! # Dimension
//!
//! - Native output: 768D (double the previous 384D)
//!
//! # Memory Layout
//!
//! - Total estimated: ~500MB for FP32 weights (~125M parameters)
//! - With FP16 quantization: ~250MB
//!
//! # TransE Operations
//!
//! Unlike the previous E11, these operations are now semantically meaningful:
//! - `transe_score(h, r, t)` - Compute TransE score: -||h + r - t||_2
//! - `predict_tail(h, r)` - Predict tail embedding: t_hat = h + r
//! - `predict_relation(h, t)` - Predict relation embedding: r_hat = t - h
//!
//! # Score Thresholds
//!
//! KEPLER produces different score distributions than MiniLM:
//! - Valid triples: score > -5.0
//! - Uncertain: score in [-10.0, -5.0]
//! - Invalid triples: score < -10.0

mod encoding;
mod forward;
mod model;
mod pooling;
mod trait_impl;
mod transe;
mod types;

#[cfg(test)]
mod tests;

// Re-export public API
pub use types::{
    KeplerModel, KEPLER_DIMENSION, KEPLER_LATENCY_BUDGET_MS, KEPLER_MAX_TOKENS, KEPLER_MODEL_NAME,
};

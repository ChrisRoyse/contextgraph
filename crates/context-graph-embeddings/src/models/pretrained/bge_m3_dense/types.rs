//! Type definitions for the BGE-M3 Dense embedding model (E14).

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::gpu::BertWeights;
use crate::traits::SingleModelConfig;

pub(crate) use crate::models::pretrained::shared::ModelState;

/// Concrete state type for the BGE-M3 Dense model (XLM-RoBERTa-Large backbone).
///
/// BGE-M3 reuses `BertWeights` because the encoder layer structure is
/// architecturally identical to BERT (same self-attention, same post-LN order,
/// same FFN). Only the load-time weight key layout, vocab size, and
/// position-embedding indexing differ.
pub(crate) type BgeM3DenseModelState = ModelState<Box<BertWeights>>;

/// BGE-M3 Dense embedding model.
///
/// Produces 1024-D L2-normalised dense vectors via CLS pooling over an
/// XLM-RoBERTa-Large encoder with 8192-token context.
///
/// # Thread safety
/// - `AtomicBool` for `loaded` state (lock-free reads).
/// - Inner model/tokenizer behind `RwLock` so concurrent embeds can read.
///
/// # Memory layout
/// - ~560 M parameters; ~2.3 GB VRAM at FP32; ~1.15 GB at FP16.
pub struct BgeM3DenseModel {
    /// Model weights and inference engine.
    #[allow(dead_code)]
    pub(crate) model_state: std::sync::RwLock<BgeM3DenseModelState>,

    /// Path to model weights directory (expects `./models/bge-m3-dense/`).
    #[allow(dead_code)]
    pub(crate) model_path: PathBuf,

    /// Configuration for this model instance.
    #[allow(dead_code)]
    pub(crate) config: SingleModelConfig,

    /// Whether model weights are loaded and ready.
    pub(crate) loaded: AtomicBool,
}

// SAFETY: BgeM3DenseModel wraps Candle tensors (via BertWeights in ModelState)
// behind a std::sync::RwLock. Candle tensors contain raw GPU pointers that are
// !Send/!Sync individually, but all access is synchronised through the RwLock,
// matching the pattern used by every other pretrained model in this crate.
unsafe impl Send for BgeM3DenseModel {}
unsafe impl Sync for BgeM3DenseModel {}

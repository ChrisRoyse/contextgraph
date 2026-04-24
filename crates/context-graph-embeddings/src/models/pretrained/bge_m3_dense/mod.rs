//! BGE-M3 Dense embedding model (E14).
//!
//! BAAI/bge-m3 is a multilingual, multi-functionality, multi-granularity text
//! embedding model. This module implements the **dense retrieval** head only —
//! a 1024-D CLS-pooled vector extracted from an XLM-RoBERTa-Large backbone
//! with 8192-token context.
//!
//! # Architecture
//! - Backbone: XLM-RoBERTa-Large (24 layers, 1024 hidden, 16 heads).
//! - Tokeniser: XLM-R SentencePiece (~250 K vocab).
//! - Pooling: CLS token (position 0) + L2 normalisation.
//! - Max context: 8192 tokens.
//!
//! # Why reuse `BertWeights`
//! XLM-RoBERTa and BERT differ in three load-time concerns (weight key prefix,
//! vocab size, position-offset semantics) but share identical forward-pass
//! math at the layer level — same Q/K/V projections, same GELU FFN, same
//! post-LayerNorm ordering. Reusing the shared `GpuModelLoader::load_bert_weights_with_prefix`
//! path keeps this module small and avoids re-implementing safetensors
//! plumbing that the rest of the workspace already battle-tests.
//!
//! # GPU pipeline
//! 1. SentencePiece tokenisation on CPU.
//! 2. Input IDs → GPU, embedding lookup (word + XLM-R-offset position + token_type).
//! 3. 24 transformer encoder layers (self-attention + FFN + post-LN).
//! 4. CLS pooling (first-token slice).
//! 5. L2 normalisation on GPU.
//!
//! # Weight files expected
//! Place the HuggingFace snapshot of `BAAI/bge-m3` under
//! `./models/bge-m3-dense/` with at minimum:
//! - `config.json` (must report `model_type: "xlm-roberta"` and
//!   `hidden_size: 1024`).
//! - `model.safetensors`.
//! - `tokenizer.json` (ships with the `sentencepiece.bpe.model` side-car).

mod attention;
mod constants;
mod embeddings;
mod encoder;
mod ffn;
mod gpu_forward;
mod layer_norm;
mod loader;
mod model;
mod pooling;
mod trait_impl;
mod types;

#[cfg(test)]
mod tests;

pub use constants::{
    BGE_M3_DENSE_DIMENSION, BGE_M3_DENSE_LATENCY_BUDGET_MS, BGE_M3_DENSE_MAX_TOKENS,
    XLM_R_BOS_TOKEN_ID, XLM_R_PAD_TOKEN_ID, XLM_R_POSITION_OFFSET, XLM_R_WEIGHT_PREFIX,
};
pub use types::BgeM3DenseModel;

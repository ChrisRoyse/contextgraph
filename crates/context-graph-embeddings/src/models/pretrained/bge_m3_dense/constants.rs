//! Constants for the BGE-M3 Dense embedding model (E14).
//!
//! BAAI/bge-m3 dense head: XLM-RoBERTa-Large backbone, CLS-pooled, L2-normalised.

/// Native dimension for BGE-M3 dense head.
pub const BGE_M3_DENSE_DIMENSION: usize = 1024;

/// Maximum tokens BGE-M3 supports (8192-token extended context).
pub const BGE_M3_DENSE_MAX_TOKENS: usize = 8192;

/// Latency budget in milliseconds (P95 target).
///
/// BGE-M3 is roughly 3x slower than e5-large-v2 at equivalent context because
/// the XLM-R SentencePiece tokenizer and 8k-position embeddings add overhead.
pub const BGE_M3_DENSE_LATENCY_BUDGET_MS: u32 = 50;

/// XLM-RoBERTa pad token ID (`<pad>` = 1 per the SentencePiece vocab).
pub const XLM_R_PAD_TOKEN_ID: u32 = 1;

/// XLM-RoBERTa BOS / CLS token ID (`<s>` = 0 per the SentencePiece vocab).
pub const XLM_R_BOS_TOKEN_ID: u32 = 0;

/// Position embeddings in XLM-RoBERTa start at `padding_idx + 1 = 2`.
/// This matches the HuggingFace `create_position_ids_from_input_ids` helper.
pub const XLM_R_POSITION_OFFSET: u32 = XLM_R_PAD_TOKEN_ID + 1;

/// Weight-key prefix used by HuggingFace for XLM-RoBERTa checkpoints.
/// BGE-M3 wraps its XLM-R backbone under `roberta.` — the pooler lives under
/// the same prefix.
pub const XLM_R_WEIGHT_PREFIX: &str = "roberta.";

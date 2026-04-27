//! LLM-based Causal Hint Provider for E5 Embedding Enhancement.
//!
//! This module provides the production implementation of [`CausalHintProvider`]
//! that wraps `CausalDiscoveryLLM` for LLM-based causal analysis.
//!
//! # Note
//!
//! This implementation lives here (in MCP) rather than in `context-graph-embeddings`
//! to avoid cyclic dependencies:
//! - `context-graph-embeddings` depends on nothing for this trait
//! - `context-graph-causal-agent` depends on `context-graph-embeddings` for CausalModel
//! - This crate depends on both, so it can create the LLM provider
//!
//! # Usage
//!
//! ```ignore
//! use context_graph_causal_agent::CausalDiscoveryLLM;
//! use context_graph_mcp::adapters::LlmCausalHintProvider;
//!
//! let llm = Arc::new(CausalDiscoveryLLM::new(config)?);
//! llm.load().await?;
//!
//! let provider = LlmCausalHintProvider::new(llm, 100); // 100ms timeout
//!
//! // Use in store_memory
//! if let Some(hint) = provider.get_hint("High cortisol causes memory impairment").await {
//!     // Pass hint to E5 embedder
//! }
//! ```

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use context_graph_causal_agent::CausalDiscoveryLLM;
use context_graph_core::traits::{CausalHint, ExtractedCausalRelationship};
use context_graph_embeddings::provider::{CausalHintProvider, ExtractionStatus};
use tracing::{debug, info, warn};

/// Production implementation wrapping CausalDiscoveryLLM.
///
/// Uses the Qwen2.5-3B-Instruct model for causal analysis with
/// grammar-constrained JSON output for 100% parse success.
pub struct LlmCausalHintProvider {
    /// The underlying LLM for causal analysis.
    llm: Arc<CausalDiscoveryLLM>,
    /// Timeout for LLM inference (default: 100ms).
    timeout_duration: Duration,
    /// Minimum confidence threshold for useful hints.
    min_confidence: f32,
    /// Status of the last extraction attempt (atomic for thread safety).
    last_status: AtomicU8,
}

impl LlmCausalHintProvider {
    /// Default timeout for hint generation (2s for GPU-based 7B model inference).
    /// RTX 5090 with Q5_K_M quantization typically takes ~100-500ms per hint.
    pub const DEFAULT_TIMEOUT_MS: u64 = 2000;

    /// Default minimum confidence threshold.
    pub const DEFAULT_MIN_CONFIDENCE: f32 = 0.5;

    // Status encoding for AtomicU8
    const STATUS_UNKNOWN: u8 = 0;
    const STATUS_SUCCESS: u8 = 1;
    const STATUS_NO_CONTENT: u8 = 2;
    const STATUS_UNAVAILABLE: u8 = 3;
    const STATUS_TIMEOUT: u8 = 4;
    const STATUS_ERROR: u8 = 5;

    /// Create a new LLM-based hint provider.
    ///
    /// # Arguments
    ///
    /// * `llm` - The CausalDiscoveryLLM instance (shared)
    /// * `timeout_ms` - Timeout in milliseconds (default: 100)
    pub fn new(llm: Arc<CausalDiscoveryLLM>, timeout_ms: u64) -> Self {
        Self {
            llm,
            timeout_duration: Duration::from_millis(timeout_ms),
            min_confidence: Self::DEFAULT_MIN_CONFIDENCE,
            last_status: AtomicU8::new(Self::STATUS_UNKNOWN),
        }
    }
}

#[async_trait]
impl CausalHintProvider for LlmCausalHintProvider {
    async fn get_hint(&self, content: &str) -> Option<CausalHint> {
        // Check if LLM is ready
        if !self.llm.is_loaded() {
            debug!("LlmCausalHintProvider: LLM not loaded, skipping hint generation");
            return None;
        }

        // Run analysis with timeout
        let result =
            tokio::time::timeout(self.timeout_duration, self.llm.analyze_single_text(content))
                .await;

        match result {
            Ok(Ok(hint)) => {
                // Check confidence threshold
                if hint.confidence >= self.min_confidence && hint.is_causal {
                    debug!(
                        is_causal = hint.is_causal,
                        confidence = hint.confidence,
                        direction = ?hint.direction_hint,
                        key_phrases = ?hint.key_phrases,
                        "LlmCausalHintProvider: Generated hint for content"
                    );
                    Some(hint)
                } else {
                    debug!(
                        is_causal = hint.is_causal,
                        confidence = hint.confidence,
                        min_confidence = self.min_confidence,
                        "LlmCausalHintProvider: Hint below threshold, skipping"
                    );
                    None
                }
            }
            Ok(Err(e)) => {
                warn!(
                    error = %e,
                    "LlmCausalHintProvider: LLM analysis failed"
                );
                None
            }
            Err(_) => {
                warn!(
                    timeout_ms = self.timeout_duration.as_millis(),
                    "LlmCausalHintProvider: Analysis timed out"
                );
                None
            }
        }
    }

    async fn extract_all_relationships(&self, content: &str) -> Vec<ExtractedCausalRelationship> {
        // Check if LLM is ready
        if !self.llm.is_loaded() {
            self.last_status
                .store(Self::STATUS_UNAVAILABLE, Ordering::SeqCst);
            debug!("LlmCausalHintProvider: LLM not loaded, skipping relationship extraction");
            return Vec::new();
        }

        // Use longer timeout for multi-extraction (2x single hint timeout)
        let multi_timeout = self.timeout_duration * 2;

        // Run extraction with timeout
        let result = tokio::time::timeout(
            multi_timeout,
            self.llm.extract_causal_relationships(content),
        )
        .await;

        match result {
            Ok(Ok(multi_result)) => {
                if multi_result.relationships.is_empty() {
                    self.last_status
                        .store(Self::STATUS_NO_CONTENT, Ordering::SeqCst);
                    debug!(
                        has_causal = multi_result.has_causal_content,
                        "LlmCausalHintProvider: No relationships found in content"
                    );
                } else {
                    self.last_status
                        .store(Self::STATUS_SUCCESS, Ordering::SeqCst);
                    info!(
                        count = multi_result.relationships.len(),
                        has_causal = multi_result.has_causal_content,
                        "LlmCausalHintProvider: Extracted causal relationships"
                    );
                }

                // Convert from causal-agent types to core types
                multi_result
                    .relationships
                    .into_iter()
                    .map(|r| {
                        let mechanism = match context_graph_core::traits::MechanismType::from_str(
                            r.mechanism_type.as_str(),
                        ) {
                            Some(m) => m,
                            None => {
                                warn!(
                                    invalid_type = r.mechanism_type.as_str(),
                                    "LlmCausalHintProvider: Unknown mechanism type, defaulting to Direct"
                                );
                                context_graph_core::traits::MechanismType::Direct
                            }
                        };
                        ExtractedCausalRelationship::new(
                            r.cause,
                            r.effect,
                            r.explanation,
                            r.confidence,
                            mechanism,
                        )
                    })
                    .collect()
            }
            Ok(Err(e)) => {
                self.last_status.store(Self::STATUS_ERROR, Ordering::SeqCst);
                warn!(
                    error = %e,
                    "LlmCausalHintProvider: Relationship extraction failed"
                );
                Vec::new()
            }
            Err(_) => {
                self.last_status
                    .store(Self::STATUS_TIMEOUT, Ordering::SeqCst);
                warn!(
                    timeout_ms = multi_timeout.as_millis(),
                    "LlmCausalHintProvider: Relationship extraction timed out"
                );
                Vec::new()
            }
        }
    }

    fn is_available(&self) -> bool {
        self.llm.is_loaded()
    }

    fn timeout(&self) -> Duration {
        self.timeout_duration
    }

    fn last_extraction_status(&self) -> ExtractionStatus {
        match self.last_status.load(Ordering::SeqCst) {
            Self::STATUS_SUCCESS => ExtractionStatus::Success,
            Self::STATUS_NO_CONTENT => ExtractionStatus::NoContent,
            Self::STATUS_UNAVAILABLE => ExtractionStatus::Unavailable,
            Self::STATUS_TIMEOUT => ExtractionStatus::Timeout,
            Self::STATUS_ERROR => ExtractionStatus::Error,
            _ => ExtractionStatus::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests would require a loaded LLM model
    // Unit tests for the provider are minimal since it's a thin wrapper

    #[test]
    fn test_default_constants() {
        assert_eq!(LlmCausalHintProvider::DEFAULT_TIMEOUT_MS, 2000);
        assert!((LlmCausalHintProvider::DEFAULT_MIN_CONFIDENCE - 0.5).abs() < f32::EPSILON);
    }
}

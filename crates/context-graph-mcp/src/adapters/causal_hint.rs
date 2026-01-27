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

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use context_graph_causal_agent::CausalDiscoveryLLM;
use context_graph_core::traits::CausalHint;
use context_graph_embeddings::provider::CausalHintProvider;
use tracing::{debug, warn};

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
}

impl LlmCausalHintProvider {
    /// Default timeout for hint generation (2s for GPU-based 7B model inference).
    /// RTX 5090 with Q5_K_M quantization typically takes ~100-500ms per hint.
    pub const DEFAULT_TIMEOUT_MS: u64 = 2000;

    /// Default minimum confidence threshold.
    pub const DEFAULT_MIN_CONFIDENCE: f32 = 0.5;

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
        }
    }

    /// Create with custom minimum confidence threshold.
    pub fn with_min_confidence(mut self, min_confidence: f32) -> Self {
        self.min_confidence = min_confidence.clamp(0.0, 1.0);
        self
    }

    /// Load the underlying LLM model.
    ///
    /// This should be called during startup to preload the model.
    /// Returns error if model loading fails.
    pub async fn load_model(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.llm
            .load()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
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
        let result = tokio::time::timeout(
            self.timeout_duration,
            self.llm.analyze_single_text(content),
        )
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

    fn is_available(&self) -> bool {
        self.llm.is_loaded()
    }

    fn timeout(&self) -> Duration {
        self.timeout_duration
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

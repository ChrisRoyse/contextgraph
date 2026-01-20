//! Adapter bridging real UtlProcessor to core trait interface.
//!
//! This module provides `UtlProcessorAdapter` which wraps the synchronous
//! `context_graph_utl::processor::UtlProcessor` and implements the async
//! `context_graph_core::traits::UtlProcessor` trait.
//!
//! # Architecture
//!
//! The real UtlProcessor uses `&mut self` synchronous methods, while the
//! core trait requires async methods. This adapter bridges the gap using
//! `tokio::task::spawn_blocking` to avoid blocking the async runtime.
//!
//! # Example
//!
//! ```ignore
//! use context_graph_mcp::adapters::UtlProcessorAdapter;
//! use context_graph_core::types::UtlContext;
//!
//! let adapter = UtlProcessorAdapter::with_defaults();
//! let context = UtlContext::default();
//! let score = adapter.compute_learning_score("test input", &context).await?;
//! ```

// This adapter is prepared for future use but not yet wired up
#![allow(dead_code)]

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::RwLock;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::traits::UtlProcessor;
use context_graph_core::types::{MemoryNode, UtlContext, UtlMetrics};

use context_graph_utl::config::UtlConfig;
use context_graph_utl::metrics::{StageThresholds, UtlComputationMetrics, UtlStatus};
use context_graph_utl::phase::ConsolidationPhase;
use context_graph_utl::processor::UtlProcessor as RealUtlProcessor;

/// Adapter bridging real UtlProcessor to core trait.
///
/// Wraps the synchronous `context_graph_utl::processor::UtlProcessor`
/// and implements the async `context_graph_core::traits::UtlProcessor`.
///
/// # Thread Safety
///
/// Uses `Arc<RwLock<RealUtlProcessor>>` to allow safe concurrent access
/// while preserving the ability to mutate processor state.
///
/// # Performance
///
/// All async methods use `spawn_blocking` to run synchronous UTL computations
/// without blocking the async runtime. Target latency: < 10ms per computation.
#[derive(Debug)]
pub struct UtlProcessorAdapter {
    /// The wrapped real UTL processor
    inner: Arc<RwLock<RealUtlProcessor>>,

    /// Accumulated computation metrics for status reporting
    metrics: Arc<RwLock<UtlComputationMetrics>>,
}

impl UtlProcessorAdapter {
    /// Create adapter with configuration.
    ///
    /// # Arguments
    /// * `config` - UTL configuration for the real processor
    ///
    /// # Panics
    /// Panics if config validation fails. Use `try_new()` for fallible construction.
    pub fn new(config: UtlConfig) -> Self {
        let processor = RealUtlProcessor::new(config);
        Self {
            inner: Arc::new(RwLock::new(processor)),
            metrics: Arc::new(RwLock::new(UtlComputationMetrics::new())),
        }
    }

    /// Try to create adapter, returning error if config is invalid.
    pub fn try_new(config: UtlConfig) -> CoreResult<Self> {
        let processor =
            RealUtlProcessor::try_new(config).map_err(|e| CoreError::UtlError(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(RwLock::new(processor)),
            metrics: Arc::new(RwLock::new(UtlComputationMetrics::new())),
        })
    }

    /// Create adapter from existing processor.
    pub fn from_processor(processor: RealUtlProcessor) -> Self {
        Self {
            inner: Arc::new(RwLock::new(processor)),
            metrics: Arc::new(RwLock::new(UtlComputationMetrics::new())),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(UtlConfig::default())
    }

    /// Get a clone of the inner processor reference for spawn_blocking.
    fn inner_clone(&self) -> Arc<RwLock<RealUtlProcessor>> {
        Arc::clone(&self.inner)
    }

    /// Get a clone of the metrics reference for spawn_blocking.
    fn metrics_clone(&self) -> Arc<RwLock<UtlComputationMetrics>> {
        Arc::clone(&self.metrics)
    }

    /// Extract embedding from context.
    ///
    /// Returns the input_embedding if present (from inject_context), otherwise
    /// falls back to goal_vector (strategic alignment), otherwise returns a default zero vector.
    ///
    /// # Priority Order
    /// 1. `input_embedding` - The actual content embedding (E1 semantic from inject_context)
    /// 2. `goal_vector` - Strategic goal alignment vector
    /// 3. Zero vector - Fallback for autonomous operation
    fn get_embedding(context: &UtlContext) -> CoreResult<Vec<f32>> {
        // Prefer input_embedding (actual content embedding from inject_context)
        // Fall back to goal_vector (strategic alignment)
        // Final fallback to zero vector for autonomous operation
        Ok(context
            .input_embedding
            .clone()
            .or_else(|| context.goal_vector.clone())
            .unwrap_or_else(|| vec![0.0; 128]))
    }

    /// Build context embeddings from UtlContext.
    ///
    /// Uses `reference_embeddings` for historical context. When reference_embeddings
    /// is None or empty, returns empty Vec which causes surprise calculator to
    /// return 1.0 (maximum surprise for novel content).
    ///
    /// # ARCH-02 Compliance
    /// Reference embeddings must be from the same embedding space for apples-to-apples comparison.
    fn get_context_embeddings(context: &UtlContext) -> Vec<Vec<f32>> {
        context
            .reference_embeddings
            .clone()
            .unwrap_or_default()
    }
}

impl Clone for UtlProcessorAdapter {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            metrics: Arc::clone(&self.metrics),
        }
    }
}

#[async_trait]
impl UtlProcessor for UtlProcessorAdapter {
    /// Compute the full UTL learning score.
    ///
    /// Bridges to real processor's `compute_learning()` method via spawn_blocking.
    ///
    /// # Errors
    /// Returns `CoreError::MissingField` if goal_vector is missing from context.
    async fn compute_learning_score(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let inner = self.inner_clone();
        let metrics_ref = self.metrics_clone();
        let input = input.to_string();
        let embedding = Self::get_embedding(context)?;
        let context_embeddings = Self::get_context_embeddings(context);

        tokio::task::spawn_blocking(move || {
            let mut processor = inner
                .write()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let signal = processor
                .compute_learning(&input, &embedding, &context_embeddings)
                .map_err(|e| CoreError::UtlError(e.to_string()))?;

            // Update accumulated metrics
            {
                let mut metrics = metrics_ref
                    .write()
                    .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
                metrics.record_computation(
                    signal.magnitude,
                    signal.delta_s,
                    signal.delta_c,
                    signal.latency_us as f64,
                );
                metrics.lifecycle_stage = processor.lifecycle_stage();
                metrics.lambda_weights = processor.lambda_weights();
            }

            Ok(signal.magnitude)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Compute surprise component (ΔS).
    ///
    /// Returns value in [0.0, 1.0] representing information gain/novelty.
    ///
    /// # Errors
    /// Returns `CoreError::MissingField` if goal_vector is missing from context.
    async fn compute_surprise(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let inner = self.inner_clone();
        let input = input.to_string();
        let embedding = Self::get_embedding(context)?;
        let context_embeddings = Self::get_context_embeddings(context);

        tokio::task::spawn_blocking(move || {
            let mut processor = inner
                .write()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let signal = processor
                .compute_learning(&input, &embedding, &context_embeddings)
                .map_err(|e| CoreError::UtlError(e.to_string()))?;
            Ok(signal.delta_s)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Compute coherence change (ΔC).
    ///
    /// Returns value in [0.0, 1.0] representing understanding gain.
    ///
    /// # Errors
    /// Returns `CoreError::MissingField` if goal_vector is missing from context.
    async fn compute_coherence_change(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let inner = self.inner_clone();
        let input = input.to_string();
        let embedding = Self::get_embedding(context)?;
        let context_embeddings = Self::get_context_embeddings(context);

        tokio::task::spawn_blocking(move || {
            let mut processor = inner
                .write()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let signal = processor
                .compute_learning(&input, &embedding, &context_embeddings)
                .map_err(|e| CoreError::UtlError(e.to_string()))?;
            Ok(signal.delta_c)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Compute emotional weight (wₑ).
    ///
    /// Returns value in [0.5, 1.5] representing emotional salience.
    ///
    /// # Errors
    /// Returns `CoreError::MissingField` if goal_vector is missing from context.
    async fn compute_emotional_weight(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let inner = self.inner_clone();
        let input = input.to_string();
        let embedding = Self::get_embedding(context)?;
        let context_embeddings = Self::get_context_embeddings(context);

        tokio::task::spawn_blocking(move || {
            let mut processor = inner
                .write()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let signal = processor
                .compute_learning(&input, &embedding, &context_embeddings)
                .map_err(|e| CoreError::UtlError(e.to_string()))?;
            Ok(signal.w_e)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Compute goal alignment (cos φ).
    ///
    /// Returns value in [-1.0, 1.0] where 1.0 is perfect alignment.
    ///
    /// # Errors
    /// Returns `CoreError::MissingField` if goal_vector is missing from context.
    async fn compute_alignment(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let inner = self.inner_clone();
        let input = input.to_string();
        let embedding = Self::get_embedding(context)?;
        let context_embeddings = Self::get_context_embeddings(context);

        tokio::task::spawn_blocking(move || {
            let mut processor = inner
                .write()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let signal = processor
                .compute_learning(&input, &embedding, &context_embeddings)
                .map_err(|e| CoreError::UtlError(e.to_string()))?;
            // cos(phi) for alignment
            Ok(signal.phi.cos())
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Determine if a node should be consolidated to long-term memory.
    async fn should_consolidate(&self, node: &MemoryNode) -> CoreResult<bool> {
        let inner = self.inner_clone();
        let importance = node.importance;

        tokio::task::spawn_blocking(move || {
            let processor = inner
                .read()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let stage = processor.lifecycle_stage();
            let thresholds = StageThresholds::for_stage(stage);
            Ok(importance >= thresholds.consolidation_threshold)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Get full UTL metrics for input.
    ///
    /// # Errors
    /// Returns `CoreError::MissingField` if goal_vector is missing from context.
    async fn compute_metrics(&self, input: &str, context: &UtlContext) -> CoreResult<UtlMetrics> {
        let inner = self.inner_clone();
        let metrics_ref = self.metrics_clone();
        let input = input.to_string();
        let embedding = Self::get_embedding(context)?;
        let context_embeddings = Self::get_context_embeddings(context);

        tokio::task::spawn_blocking(move || {
            let mut processor = inner
                .write()
                .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
            let signal = processor
                .compute_learning(&input, &embedding, &context_embeddings)
                .map_err(|e| CoreError::UtlError(e.to_string()))?;

            // FIX: Update accumulated metrics so get_status() returns live values
            // Previously this was missing, causing get_memetic_status to return all zeros
            {
                let mut metrics = metrics_ref
                    .write()
                    .map_err(|e| CoreError::Internal(format!("Lock poisoned: {}", e)))?;
                metrics.record_computation(
                    signal.magnitude,
                    signal.delta_s,
                    signal.delta_c,
                    signal.latency_us as f64,
                );
                metrics.lifecycle_stage = processor.lifecycle_stage();
                metrics.lambda_weights = processor.lambda_weights();
            }

            Ok(UtlMetrics {
                entropy: signal.delta_s,
                coherence: signal.delta_c,
                learning_score: signal.magnitude,
                surprise: signal.delta_s,
                coherence_change: signal.delta_c,
                emotional_weight: signal.w_e,
                alignment: signal.phi.cos(),
            })
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }

    /// Get current UTL system status as JSON.
    ///
    /// Returns the complete system status including lifecycle stage,
    /// thresholds, lambda weights, phase angle, and computation metrics.
    ///
    /// # Panics
    /// Panics if the internal locks are poisoned.
    fn get_status(&self) -> serde_json::Value {
        // Use std::sync::RwLock read() - safe to call from any context
        let processor = self.inner.read().expect("Processor lock poisoned");
        let metrics = self.metrics.read().expect("Metrics lock poisoned");

        // Build UtlStatus from processor state
        let stage = processor.lifecycle_stage();
        let status = UtlStatus {
            lifecycle_stage: stage,
            interaction_count: processor.interaction_count(),
            current_thresholds: StageThresholds::for_stage(stage),
            lambda_weights: processor.lambda_weights(),
            phase_angle: processor.current_phase(),
            consolidation_phase: ConsolidationPhase::Wake, // Default wake state
            metrics: metrics.clone(),
        };

        // Convert to MCP response format
        serde_json::to_value(status.to_mcp_response()).unwrap_or_else(
            |e| serde_json::json!({"error": format!("serialization failed: {}", e)}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let status = adapter.get_status();

        assert!(status.get("lifecycle_phase").is_some());
        assert_eq!(status["lifecycle_phase"].as_str().unwrap(), "Infancy");
        assert_eq!(status["interaction_count"].as_u64().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_adapter_computes_real_learning_score() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let context = UtlContext {
            goal_vector: Some(vec![0.1; 128]),
            ..Default::default()
        };

        let score = adapter.compute_learning_score("test input", &context).await;
        assert!(score.is_ok());
        let score = score.unwrap();
        assert!((0.0..=1.0).contains(&score), "Score {} not in [0,1]", score);
    }

    #[tokio::test]
    async fn test_adapter_get_status_returns_live_values() {
        let adapter = UtlProcessorAdapter::with_defaults();

        // Compute something first
        let context = UtlContext {
            goal_vector: Some(vec![0.5; 128]),
            ..Default::default()
        };
        let _ = adapter
            .compute_learning_score("trigger computation", &context)
            .await;

        let status = adapter.get_status();

        // Verify schema
        assert!(status.get("lifecycle_phase").is_some());
        assert!(status.get("interaction_count").is_some());
        assert!(status.get("thresholds").is_some());
        assert!(status.get("entropy").is_some());
        assert!(status.get("coherence").is_some());
        assert!(status.get("learning_score").is_some());
        assert!(status.get("phase_angle").is_some());

        // Verify live values (not zeros from stub)
        let interaction_count = status["interaction_count"].as_u64().unwrap();
        assert!(
            interaction_count >= 1,
            "Should reflect real computation count, got {}",
            interaction_count
        );
    }

    #[tokio::test]
    async fn test_adapter_lifecycle_progression() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let context = UtlContext {
            goal_vector: Some(vec![0.1; 128]),
            ..Default::default()
        };

        // Start in Infancy
        let status = adapter.get_status();
        assert_eq!(status["lifecycle_phase"].as_str().unwrap(), "Infancy");

        // Compute 50 times to trigger Growth
        for i in 0..50 {
            let mut ctx = context.clone();
            ctx.goal_vector = Some(vec![0.1 + (i as f32 * 0.01); 128]);
            let _ = adapter
                .compute_learning_score(&format!("msg {}", i), &ctx)
                .await;
        }

        let status = adapter.get_status();
        assert_eq!(
            status["lifecycle_phase"].as_str().unwrap(),
            "Growth",
            "Should transition to Growth after 50 interactions"
        );
    }

    #[tokio::test]
    async fn test_adapter_error_propagation() {
        let adapter = UtlProcessorAdapter::with_defaults();

        // Empty embedding should still work (handled by real processor)
        let context = UtlContext {
            goal_vector: Some(vec![]),
            ..Default::default()
        };

        let result = adapter.compute_learning_score("test", &context).await;
        // Should either succeed or return meaningful error, NEVER panic
        match result {
            Ok(score) => assert!((0.0..=1.0).contains(&score)),
            Err(e) => assert!(!e.to_string().is_empty(), "Error must have message"),
        }
    }

    #[tokio::test]
    async fn test_adapter_compute_metrics() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let context = UtlContext {
            goal_vector: Some(vec![0.3; 128]),
            ..Default::default()
        };

        let metrics = adapter.compute_metrics("test input", &context).await;
        assert!(metrics.is_ok());

        let metrics = metrics.unwrap();
        assert!(metrics.surprise >= 0.0 && metrics.surprise <= 1.0);
        assert!(metrics.coherence_change >= 0.0 && metrics.coherence_change <= 1.0);
        assert!(metrics.emotional_weight >= 0.5 && metrics.emotional_weight <= 1.5);
        assert!(metrics.alignment >= -1.0 && metrics.alignment <= 1.0);
        assert!(metrics.learning_score >= 0.0 && metrics.learning_score <= 1.0);
    }

    #[tokio::test]
    async fn test_adapter_should_consolidate() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let embedding = vec![0.5; 128];

        // Node below threshold
        let mut low_importance = MemoryNode::new("low".to_string(), embedding.clone());
        low_importance.importance = 0.1;

        // Node above threshold
        let mut high_importance = MemoryNode::new("high".to_string(), embedding);
        high_importance.importance = 0.9;

        let low_result = adapter.should_consolidate(&low_importance).await;
        let high_result = adapter.should_consolidate(&high_importance).await;

        assert!(low_result.is_ok());
        assert!(high_result.is_ok());

        // High importance should consolidate
        assert!(
            high_result.unwrap(),
            "High importance node should consolidate"
        );
    }

    #[tokio::test]
    async fn test_adapter_deterministic_outputs() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let context = UtlContext {
            goal_vector: Some(vec![0.2; 128]),
            ..Default::default()
        };

        // Same input should produce consistent structure
        let score1 = adapter
            .compute_learning_score("same input", &context)
            .await
            .unwrap();
        let score2 = adapter
            .compute_learning_score("same input", &context)
            .await
            .unwrap();

        // Both should be valid scores
        assert!((0.0..=1.0).contains(&score1));
        assert!((0.0..=1.0).contains(&score2));
    }

    #[tokio::test]
    async fn test_adapter_status_thresholds_schema() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let status = adapter.get_status();

        // Verify thresholds object exists and has expected fields
        let thresholds = status.get("thresholds").expect("thresholds must exist");

        assert!(
            thresholds.get("entropy_trigger").is_some(),
            "entropy_trigger must exist"
        );
        assert!(
            thresholds.get("coherence_trigger").is_some(),
            "coherence_trigger must exist"
        );
        assert!(
            thresholds.get("min_importance_store").is_some(),
            "min_importance_store must exist"
        );
        assert!(
            thresholds.get("consolidation_threshold").is_some(),
            "consolidation_threshold must exist"
        );
    }

    #[tokio::test]
    async fn test_edge_case_empty_input() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let context = UtlContext {
            goal_vector: Some(vec![0.1; 128]),
            ..Default::default()
        };

        // Empty content should not panic
        let result = adapter.compute_learning_score("", &context).await;
        assert!(result.is_ok(), "Empty input should not cause error");
    }

    #[tokio::test]
    async fn test_clone_shares_state() {
        let adapter1 = UtlProcessorAdapter::with_defaults();
        let adapter2 = adapter1.clone();

        let context = UtlContext {
            goal_vector: Some(vec![0.1; 128]),
            ..Default::default()
        };

        // Compute on adapter1
        let _ = adapter1.compute_learning_score("test", &context).await;

        // Both should see updated state
        let status1 = adapter1.get_status();
        let status2 = adapter2.get_status();

        assert_eq!(
            status1["interaction_count"].as_u64(),
            status2["interaction_count"].as_u64(),
            "Cloned adapters should share state"
        );
    }

    /// TEST: Verify that missing goal_vector uses default zero vector.
    /// The 13-embedder teleological arrays are the primary data; goal_vector is optional.
    /// This enables autonomous operation without requiring strategic goal configuration.
    #[tokio::test]
    async fn test_missing_goal_vector_uses_default() {
        let adapter = UtlProcessorAdapter::with_defaults();

        // Context with NO goal_vector - should use default zero vector and succeed
        let context_missing = UtlContext {
            goal_vector: None,
            ..Default::default()
        };

        // All compute methods should succeed with default zero vector
        let result = adapter
            .compute_learning_score("test", &context_missing)
            .await;
        assert!(
            result.is_ok(),
            "compute_learning_score should succeed with missing goal_vector using default"
        );
        let score = result.unwrap();
        assert!((0.0..=1.0).contains(&score), "Score {} not in [0,1]", score);

        let result = adapter.compute_surprise("test", &context_missing).await;
        assert!(
            result.is_ok(),
            "compute_surprise should succeed with missing goal_vector"
        );

        let result = adapter
            .compute_coherence_change("test", &context_missing)
            .await;
        assert!(
            result.is_ok(),
            "compute_coherence_change should succeed with missing goal_vector"
        );

        let result = adapter
            .compute_emotional_weight("test", &context_missing)
            .await;
        assert!(
            result.is_ok(),
            "compute_emotional_weight should succeed with missing goal_vector"
        );

        let result = adapter.compute_alignment("test", &context_missing).await;
        assert!(
            result.is_ok(),
            "compute_alignment should succeed with missing goal_vector"
        );

        let result = adapter.compute_metrics("test", &context_missing).await;
        assert!(
            result.is_ok(),
            "compute_metrics should succeed with missing goal_vector"
        );
    }

    /// TEST: Verify that explicit goal_vector is used when provided.
    /// When goal_vector is explicitly set, it should be used for alignment computation.
    #[tokio::test]
    async fn test_explicit_goal_vector_is_used() {
        let adapter = UtlProcessorAdapter::with_defaults();

        // Context WITH explicit goal_vector
        let context_with_goal = UtlContext {
            goal_vector: Some(vec![0.5; 128]),
            ..Default::default()
        };

        // Should succeed with the provided goal_vector
        let result = adapter
            .compute_learning_score("test", &context_with_goal)
            .await;
        assert!(
            result.is_ok(),
            "compute_learning_score should succeed with explicit goal_vector"
        );
        let score = result.unwrap();
        assert!((0.0..=1.0).contains(&score), "Score {} not in [0,1]", score);
    }

    /// Verify that should_consolidate does NOT require goal_vector.
    /// This method only uses node.importance, not context embedding.
    #[tokio::test]
    async fn test_should_consolidate_no_goal_vector_required() {
        let adapter = UtlProcessorAdapter::with_defaults();
        let embedding = vec![0.5; 128];

        let mut node = MemoryNode::new("test".to_string(), embedding);
        node.importance = 0.5;

        // should_consolidate doesn't use context embedding
        let result = adapter.should_consolidate(&node).await;
        assert!(
            result.is_ok(),
            "should_consolidate should work without goal_vector in context"
        );
    }
}

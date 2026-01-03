//! UTL (Unified Theory of Learning) processor trait.

use async_trait::async_trait;

use crate::error::CoreResult;
use crate::types::{MemoryNode, UtlContext, UtlMetrics};

/// Universal Theory of Learning processor.
///
/// Computes learning signals based on the UTL equation:
/// `L = f((ΔS × ΔC) · wₑ · cos φ)`
///
/// Where:
/// - `ΔS`: Surprise (information gain)
/// - `ΔC`: Coherence change
/// - `wₑ`: Emotional weight
/// - `cos φ`: Goal alignment
///
/// # Example
///
/// ```
/// use context_graph_core::types::UtlMetrics;
///
/// // UTL equation: L = f((ΔS × ΔC) · wₑ · cos φ)
/// let metrics = UtlMetrics {
///     entropy: 0.5,           // Information content
///     coherence: 0.7,         // Understanding level
///     surprise: 0.8,          // High novelty (ΔS)
///     coherence_change: 0.6,  // Understanding gain (ΔC)
///     emotional_weight: 1.0,  // Emotional salience (wₑ)
///     alignment: 0.9,         // Goal alignment (cos φ)
///     learning_score: 0.0,    // Computed from components
/// };
///
/// // Learning is maximized when all components are high
/// assert!(metrics.surprise > 0.5);
/// assert!(metrics.alignment > 0.5);
/// ```
#[async_trait]
pub trait UtlProcessor: Send + Sync {
    /// Compute the full UTL learning score.
    ///
    /// Returns a value in [0.0, 1.0] representing learning effectiveness.
    async fn compute_learning_score(&self, input: &str, context: &UtlContext) -> CoreResult<f32>;

    /// Compute surprise component (ΔS).
    ///
    /// Returns value in [0.0, 1.0] representing information gain/novelty.
    async fn compute_surprise(&self, input: &str, context: &UtlContext) -> CoreResult<f32>;

    /// Compute coherence change (ΔC).
    ///
    /// Returns value in [0.0, 1.0] representing understanding gain.
    async fn compute_coherence_change(&self, input: &str, context: &UtlContext) -> CoreResult<f32>;

    /// Compute emotional weight (wₑ).
    ///
    /// Higher values for emotionally salient content.
    async fn compute_emotional_weight(&self, input: &str, context: &UtlContext) -> CoreResult<f32>;

    /// Compute goal alignment (cos φ).
    ///
    /// Range [-1.0, 1.0] where 1.0 is perfect alignment.
    async fn compute_alignment(&self, input: &str, context: &UtlContext) -> CoreResult<f32>;

    /// Determine if a node should be consolidated to long-term memory.
    async fn should_consolidate(&self, node: &MemoryNode) -> CoreResult<bool>;

    /// Get full UTL metrics for input.
    async fn compute_metrics(&self, input: &str, context: &UtlContext) -> CoreResult<UtlMetrics>;
}

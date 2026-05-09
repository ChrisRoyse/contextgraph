//! Shared types for LLM-based discovery agents (causal + graph).
//!
//! Contains the `DiscoveryCycleResult` struct that both causal-agent and
//! graph-agent produce from their discovery cycles.

use std::time::Duration;

use chrono::{DateTime, Utc};

/// Result of a single discovery cycle (shared between causal and graph agents).
///
/// Discovery and graph analysis services produce this
/// identical result type from their `run_discovery_cycle()` methods.
#[derive(Debug, Clone)]
pub struct DiscoveryCycleResult {
    /// When the cycle started.
    pub started_at: DateTime<Utc>,
    /// When the cycle completed.
    pub completed_at: DateTime<Utc>,
    /// Total duration.
    pub duration: Duration,
    /// Number of candidate pairs found.
    pub candidates_found: usize,
    /// Number of relationships confirmed by LLM.
    pub relationships_confirmed: usize,
    /// Number of relationships rejected by LLM.
    pub relationships_rejected: usize,
    /// Number of embeddings generated.
    pub embeddings_generated: usize,
    /// Number of graph edges created.
    pub edges_created: usize,
    /// Number of errors encountered.
    pub errors: usize,
    /// Error messages (if any).
    pub error_messages: Vec<String>,
}

/// Status of a discovery service (shared between causal and graph agents).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
}

impl Default for DiscoveryCycleResult {
    fn default() -> Self {
        Self {
            started_at: Utc::now(),
            completed_at: Utc::now(),
            duration: Duration::ZERO,
            candidates_found: 0,
            relationships_confirmed: 0,
            relationships_rejected: 0,
            embeddings_generated: 0,
            edges_created: 0,
            errors: 0,
            error_messages: Vec::new(),
        }
    }
}

//! Test stubs for graph-agent.
//!
//! Provides stub implementations for testing that FAIL FAST if actually called.
//! These are NOT mocks that return fake data - they are guards that ensure
//! graph discovery paths are not accidentally exercised in tests that don't
//! need them.
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_graph_agent::stubs::create_stub_graph_discovery_service;
//!
//! let service = create_stub_graph_discovery_service();
//! // Service will PANIC if run_discovery_cycle or llm() methods are called
//! ```
//!
//! # Feature Gate
//!
//! This module is only available with the `test-utils` feature.

use std::path::PathBuf;
use std::sync::Arc;

use context_graph_causal_agent::{llm::LlmConfig, CausalDiscoveryLLM};

use crate::service::{GraphDiscoveryConfig, GraphDiscoveryService};

/// Create a stub GraphDiscoveryService for testing.
///
/// The stub has an UNLOADED LLM. Any calls to:
/// - `run_discovery_cycle()` will return `Err(LlmNotInitialized)`
/// - `llm().analyze_relationship()` will return `Err(LlmNotInitialized)`
/// - `llm().validate_relationship()` will return `Err(LlmNotInitialized)`
///
/// This is appropriate for tests that need a `GraphDiscoveryService` in their
/// Handlers struct but don't actually test graph discovery functionality.
///
/// # Returns
///
/// Arc-wrapped GraphDiscoveryService with an unloaded LLM.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_graph_agent::stubs::create_stub_graph_discovery_service;
///
/// let service = create_stub_graph_discovery_service();
/// assert!(!service.is_running());
/// // Calling run_discovery_cycle would return Err(LlmNotInitialized)
/// ```
pub fn create_stub_graph_discovery_service() -> Arc<GraphDiscoveryService> {
    // Create LLM config pointing to non-existent model
    // This is safe - model files are only validated during load()
    let config = LlmConfig {
        model_path: PathBuf::from("/nonexistent/model/path/for/testing.gguf"),
        causal_grammar_path: PathBuf::from("/nonexistent/causal.gbnf"),
        graph_grammar_path: PathBuf::from("/nonexistent/graph.gbnf"),
        validation_grammar_path: PathBuf::from("/nonexistent/validation.gbnf"),
        n_gpu_layers: 0,   // CPU mode for stub
        context_size: 512, // Small context for stub
        ..Default::default()
    };

    // Create unloaded LLM - this always succeeds
    let llm = CausalDiscoveryLLM::with_config(config)
        .expect("Creating unloaded CausalDiscoveryLLM for testing should always succeed");

    // Create service with unloaded LLM
    // NOTE: LLM is NOT loaded - any analysis calls will return Err(LlmNotInitialized)
    let service =
        GraphDiscoveryService::with_config(Arc::new(llm), GraphDiscoveryConfig::default());

    Arc::new(service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_service_creates_successfully() {
        let service = create_stub_graph_discovery_service();
        assert!(!service.is_running());
    }

    #[test]
    fn test_stub_llm_is_not_loaded() {
        let service = create_stub_graph_discovery_service();
        let llm = service.llm();
        assert!(!llm.is_loaded());
    }
}

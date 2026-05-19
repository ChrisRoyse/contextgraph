//! Retired causal discovery tool definitions.
//!
//! The local LLM causal-discovery backend was removed in the 2026-05-09
//! ME-JEPA pivot. The deterministic E5 scanner/activator crate
//! (`context-graph-causal-agent`) was removed in the 2026-05-19 cleanup;
//! no MCP tool may trigger a local model.

use crate::tools::types::ToolDefinition;

pub fn definitions() -> Vec<ToolDefinition> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn causal_discovery_tools_are_retired() {
        assert!(definitions().is_empty());
    }
}

//! MCP tool definitions following the MCP 2024-11-05 protocol specification.
//!
//! This module defines the tools available through the MCP server's `tools/list`
//! and `tools/call` endpoints.
//!
//! # Module Structure
//!
//! - `types`: Core type definitions (`ToolDefinition`)
//! - `names`: Tool name constants for dispatch matching
//! - `definitions`: Tool definitions organized by category
//!   - `core`: Core tools (inject, store, search, status)
//!   - `gwt`: Global Workspace Theory tools
//!   - `utl`: Unified Theory of Learning tools
//!   - `atc`: Adaptive Threshold Calibration tools
//!   - `dream`: Dream consolidation tools
//!   - `neuromod`: Neuromodulation tools
//!   - `steering`: Steering feedback tools
//!   - `causal`: Causal inference tools
//!   - `teleological`: 13-embedder fusion tools
//!   - `autonomous`: Autonomous North Star tools

pub mod aliases;
pub mod definitions;
pub mod names;
pub mod types;

// Re-export for backwards compatibility
pub use self::definitions::get_tool_definitions;
pub use self::names as tool_names;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definitions() {
        let tools = get_tool_definitions();
        // 6 original + 8 GWT tools + 1 UTL delta-S/C + 3 ATC tools + 5 Dream tools
        // + 2 Neuromod tools + 1 Steering + 1 Causal + 5 Teleological + 7 Autonomous = 39 total
        // + 3 Meta-UTL tools (TASK-METAUTL-P0-005) = 42 total
        // + 1 Epistemic tool (TASK-MCP-001) = 43 total
        // + 1 Merge tool (TASK-MCP-003) = 44 total
        // + 1 Johari classification tool (TASK-MCP-005) = 45 total
        // NOTE: GWT now has 8 tools (TASK-33/34 added get_coherence_state, TASK-38 added get_identity_continuity)
        // NOTE: Dream now has 5 tools (TASK-37 added get_gpu_status)
        // NOTE: 6 manual North Star tools REMOVED (created single 1024D embeddings
        // incompatible with 13-embedder teleological arrays)
        assert_eq!(tools.len(), 45);

        let tool_names: Vec<_> = tools.iter().map(|t| t.name.as_str()).collect();

        // Original 6 tools
        assert!(tool_names.contains(&"inject_context"));
        assert!(tool_names.contains(&"store_memory"));
        assert!(tool_names.contains(&"get_memetic_status"));
        assert!(tool_names.contains(&"get_graph_manifest"));
        assert!(tool_names.contains(&"search_graph"));
        assert!(tool_names.contains(&"utl_status"));

        // GWT tools (TASK-GWT-001, TASK-33/34, TASK-38)
        assert!(tool_names.contains(&"get_consciousness_state"));
        assert!(tool_names.contains(&"get_kuramoto_sync"));
        assert!(tool_names.contains(&"get_workspace_status"));
        assert!(tool_names.contains(&"get_ego_state"));
        assert!(tool_names.contains(&"trigger_workspace_broadcast"));
        assert!(tool_names.contains(&"adjust_coupling"));
        assert!(tool_names.contains(&"get_coherence_state")); // TASK-33/34
        assert!(tool_names.contains(&"get_identity_continuity")); // TASK-38

        // UTL delta-S/C tool (TASK-UTL-P1-001)
        assert!(tool_names.contains(&"gwt/compute_delta_sc"));

        // ATC tools (TASK-ATC-001)
        assert!(tool_names.contains(&"get_threshold_status"));
        assert!(tool_names.contains(&"get_calibration_metrics"));
        assert!(tool_names.contains(&"trigger_recalibration"));

        // Dream tools (TASK-DREAM-MCP, TASK-37)
        assert!(tool_names.contains(&"trigger_dream"));
        assert!(tool_names.contains(&"get_dream_status"));
        assert!(tool_names.contains(&"abort_dream"));
        assert!(tool_names.contains(&"get_amortized_shortcuts"));
        assert!(tool_names.contains(&"get_gpu_status")); // TASK-37

        // Neuromod tools (TASK-NEUROMOD-MCP)
        assert!(tool_names.contains(&"get_neuromodulation_state"));
        assert!(tool_names.contains(&"adjust_neuromodulator"));

        // Steering tools (TASK-STEERING-001)
        assert!(tool_names.contains(&"get_steering_feedback"));

        // Causal tools (TASK-CAUSAL-001)
        assert!(tool_names.contains(&"omni_infer"));

        // NOTE: Manual North Star tools REMOVED - they created single 1024D embeddings
        // incompatible with 13-embedder teleological arrays. Use autonomous system instead.

        // Teleological tools (TELEO-007 through TELEO-011)
        assert!(tool_names.contains(&"search_teleological"));
        assert!(tool_names.contains(&"compute_teleological_vector"));
        assert!(tool_names.contains(&"fuse_embeddings"));
        assert!(tool_names.contains(&"update_synergy_matrix"));
        assert!(tool_names.contains(&"manage_teleological_profile"));

        // Autonomous tools (TASK-AUTONOMOUS-MCP)
        assert!(tool_names.contains(&"auto_bootstrap_north_star"));
        assert!(tool_names.contains(&"get_alignment_drift"));
        assert!(tool_names.contains(&"trigger_drift_correction"));
        assert!(tool_names.contains(&"get_pruning_candidates"));
        assert!(tool_names.contains(&"trigger_consolidation"));
        assert!(tool_names.contains(&"discover_sub_goals"));
        assert!(tool_names.contains(&"get_autonomous_status"));

        // Epistemic tools (TASK-MCP-001)
        assert!(tool_names.contains(&"epistemic_action"));

        // Merge tools (TASK-MCP-003)
        assert!(tool_names.contains(&"merge_concepts"));

        // Johari classification tools (TASK-MCP-005)
        assert!(tool_names.contains(&"get_johari_classification"));
    }

    #[test]
    fn test_tool_definition_serialization() {
        let tools = get_tool_definitions();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("inject_context"));
        assert!(json.contains("inputSchema"));
    }

    #[test]
    fn test_inject_context_schema() {
        let tools = get_tool_definitions();
        let inject = tools.iter().find(|t| t.name == "inject_context").unwrap();

        let schema = &inject.input_schema;
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("content")));
        assert!(required.iter().any(|v| v.as_str() == Some("rationale")));
    }
}

//! Tool definitions for the MCP surface.
//!
//! The current default registry exposes 216 tools, including the 39-tool
//! DynamicJEPA MCP surface. The
//! ccreality tool surface is listed separately in reality-loop mode.

pub(crate) mod capability;
pub(crate) mod causal;
pub(crate) mod causal_discovery;
pub(crate) mod code;
pub(crate) mod constellation;
pub(crate) mod contrastive;
pub(crate) mod core;
pub(crate) mod curation;
pub(crate) mod daemon;
pub(crate) mod dynamicjepa;
pub(crate) mod embedder;
pub(crate) mod entity;
pub(crate) mod file_watcher;
pub(crate) mod graph;
pub(crate) mod graph_link;
pub(crate) mod keyword;
pub(crate) mod learning;
pub(crate) mod maintenance;
pub(crate) mod mejepa;
pub(crate) mod merge;
pub(crate) mod provenance;
pub(crate) mod reality;
pub(crate) mod robustness;
pub(crate) mod sequence;
pub(crate) mod temporal;
pub(crate) mod topic;
pub(crate) mod training;
pub(crate) mod typed_edges;
pub(crate) mod utl;

use crate::tools::types::ToolDefinition;

/// Get all tool definitions for the `tools/list` response.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(216);

    // Capability discovery tools (1) - compact map of embedders, CFs, and tools
    tools.extend(capability::definitions());

    // Core tools (5 - inject_context merged into store_memory; batch via store_memories)
    tools.extend(core::definitions());

    // Merge tool (1 - part of curation)
    tools.extend(merge::definitions());

    // Curation tools (2)
    tools.extend(curation::definitions());

    // Topic tools (4)
    tools.extend(topic::definitions());

    // File watcher tools (4)
    tools.extend(file_watcher::definitions());

    // Sequence tools (4) - E4 integration
    tools.extend(sequence::definitions());

    // Causal tools (4) - retired E5 surface; handlers fail closed for legacy callers.
    tools.extend(causal::definitions());

    // Causal discovery tools (retired local model surface).
    tools.extend(causal_discovery::definitions());

    // Keyword tools (1) - E6 keyword search enhancement
    tools.extend(keyword::definitions());

    // Code tools (1) - E7 code search enhancement
    tools.extend(code::definitions());

    // Graph tools (deterministic graph read/query surface)
    tools.extend(graph::definitions());

    // Robustness tools (1) - E9 typo-tolerant search
    tools.extend(robustness::definitions());

    // Entity tools (6) - E11 integration
    tools.extend(entity::definitions());

    // Embedder-first search tools (7) - active embedder surfaces; E5 is retired.
    tools.extend(embedder::definitions());

    // E12/E13 standalone search tools (2)
    tools.extend(embedder::standalone_definitions());

    // ME-JEPA Phase 4 inference/TCT + Phase 5 self-healing + Phase 6 hygiene + Phase 8 eval tools + UTML audit + Phase F/G/EK runbook/status tools + TASK-SKILL-007 consequence trace surfaces (69)
    tools.extend(mejepa::definitions());

    // Temporal tools (2) - E2 recency search, E3 periodic search
    tools.extend(temporal::definitions());

    // Graph linking tools (4) - K-NN navigation and typed edges
    tools.extend(graph_link::definitions());

    // Maintenance tools (1) - Data repair and cleanup
    tools.extend(maintenance::definitions());

    // Provenance tools (3) - Phase P3 provenance queries
    tools.extend(provenance::definitions());

    // Daemon tools (1) - Multi-agent observability
    tools.extend(daemon::definitions());

    // Training tools (1) - Training data export to CF_TRAINING_RECORDS
    tools.extend(training::definitions());

    // Learning-as-UTL tools (14) - event storage/readback + signal embedders + graph learning + outcome estimate + matrix dataset export
    tools.extend(learning::definitions());

    // UTL learner-state tools (18) - Phase-0/Phase-1 measurement substrate + retrieval policy
    tools.extend(utl::definitions());

    // Constellation tools (6) - Phase 2 compiler + derived anchors + read/score/delete
    tools.extend(constellation::definitions());

    // Contrastive pair tools (4) - Phase 3 miner + read surface
    tools.extend(contrastive::definitions());

    // Typed-edge training tools (3) - Phase 4 (F1/F2/F4 + list)
    tools.extend(typed_edges::definitions());

    // DynamicJEPA tools (39) - 5090jepa MCP surface through Phase 6.
    tools.extend(dynamicjepa::definitions());

    if crate::tools::is_reality_loop_mode() {
        tools.extend(reality::definitions());
    }

    tools
}

/// Get only the tools implemented by the lightweight `--mode reality-loop`
/// stdio server.
pub fn get_reality_loop_tool_definitions() -> Vec<ToolDefinition> {
    reality::definitions()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn test_total_tool_count_and_no_duplicates() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 216);
        // No duplicates
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        let len_before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len_before);
        // All have descriptions and schemas
        for tool in &tools {
            assert!(
                !tool.description.is_empty(),
                "Tool {} missing description",
                tool.name
            );
            assert!(
                tool.input_schema.get("type").is_some(),
                "Tool {} missing schema type",
                tool.name
            );
        }
    }

    #[test]
    fn test_submodule_counts() {
        assert_eq!(capability::definitions().len(), 1);
        assert_eq!(core::definitions().len(), 5);
        assert_eq!(merge::definitions().len(), 1);
        assert_eq!(curation::definitions().len(), 2);
        assert_eq!(topic::definitions().len(), 4);
        assert_eq!(file_watcher::definitions().len(), 4);
        assert_eq!(sequence::definitions().len(), 4);
        assert_eq!(causal::definitions().len(), 4);
        assert_eq!(keyword::definitions().len(), 1);
        assert_eq!(code::definitions().len(), 1);
        assert_eq!(robustness::definitions().len(), 1);
        assert_eq!(entity::definitions().len(), 6);
        assert_eq!(embedder::definitions().len(), 10);
        assert_eq!(embedder::standalone_definitions().len(), 2);
        assert_eq!(mejepa::definitions().len(), 69);
        assert_eq!(temporal::definitions().len(), 2);
        assert_eq!(graph_link::definitions().len(), 4);
        assert_eq!(maintenance::definitions().len(), 1);
        assert_eq!(provenance::definitions().len(), 3);
        assert_eq!(daemon::definitions().len(), 1);
        assert_eq!(training::definitions().len(), 4);
        assert_eq!(learning::definitions().len(), 14);
        assert_eq!(utl::definitions().len(), 18);
        assert_eq!(constellation::definitions().len(), 6);
        assert_eq!(contrastive::definitions().len(), 4);
        assert_eq!(typed_edges::definitions().len(), 3);
        assert_eq!(dynamicjepa::definitions().len(), 39);
        assert_eq!(graph::definitions().len(), 2);
        assert_eq!(causal_discovery::definitions().len(), 0);
    }

    #[test]
    fn test_reality_loop_lightweight_registry_only_lists_callable_tools() {
        let tools = get_reality_loop_tool_definitions();
        let names: BTreeSet<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(tools.len(), reality::definitions().len());
        assert!(names.contains("reality_attempt_summary"));
        assert!(names.contains("dynamicjepa_reality_for_attempt"));
        assert!(names.contains("optimizer_record_recommendation"));
        assert!(names.contains("optimizer_compute_influence"));
        assert!(!names.contains("search_graph"));
        assert!(!names.contains("get_memory_neighbors"));
        assert!(!names.contains("get_audit_trail"));
    }

    #[test]
    fn retired_cgreality_tools_are_not_advertised_by_tool_registries() {
        let default_tools = get_tool_definitions();
        let default_names: BTreeSet<&str> = default_tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();
        let reality_tools = get_reality_loop_tool_definitions();
        let reality_names: BTreeSet<&str> = reality_tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect();

        for retired_tool in crate::deprecation::RETIRED_CGREALITY_TOOLS {
            assert!(
                !default_names.contains(retired_tool),
                "{retired_tool} must not be advertised by the default tools/list registry"
            );
            assert!(
                !reality_names.contains(retired_tool),
                "{retired_tool} must not be advertised by the reality-loop tools/list registry"
            );
        }
    }
}

//! Tool definitions for the MCP surface.
//!
//! The current registry exposes 145 tools with the `llm` feature and 141
//! without it, including the 35-tool DynamicJEPA MCP surface.

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
    let mut tools = Vec::with_capacity(145);

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

    // Causal tools (4) - E5 Priority 1 enhancement
    tools.extend(causal::definitions());

    // Causal discovery tools (2) - E5 LLM-based relationship discovery
    tools.extend(causal_discovery::definitions());

    // Keyword tools (1) - E6 keyword search enhancement
    tools.extend(keyword::definitions());

    // Code tools (1) - E7 code search enhancement
    tools.extend(code::definitions());

    // Graph tools (2 base, +2 LLM) - E8 upgrade (Phase 4)
    tools.extend(graph::definitions());

    // Robustness tools (1) - E9 typo-tolerant search
    tools.extend(robustness::definitions());

    // Entity tools (6) - E11 integration
    tools.extend(entity::definitions());

    // Embedder-first search tools (7) - Constitution v6.3 + NAV-GAP tools
    tools.extend(embedder::definitions());

    // E12/E13 standalone search tools (2)
    tools.extend(embedder::standalone_definitions());

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

    // Typed-edge training tools (4) - Phase 4 (F1/F2/F4 + list)
    tools.extend(typed_edges::definitions());

    // DynamicJEPA tools (35) - 5090jepa MCP surface through Phase 6.
    tools.extend(dynamicjepa::definitions());

    if crate::tools::is_reality_loop_mode() {
        tools.extend(reality::definitions());
    }

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_tool_count_and_no_duplicates() {
        let tools = get_tool_definitions();
        #[cfg(feature = "llm")]
        assert_eq!(tools.len(), 145);
        #[cfg(not(feature = "llm"))]
        assert_eq!(tools.len(), 141);
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
        assert_eq!(embedder::definitions().len(), 7);
        assert_eq!(embedder::standalone_definitions().len(), 2);
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
        assert_eq!(typed_edges::definitions().len(), 4);
        assert_eq!(dynamicjepa::definitions().len(), 35);
        // Audit-12 TST-H2 FIX: graph and causal_discovery are LLM-gated, must be tested
        #[cfg(feature = "llm")]
        {
            assert_eq!(graph::definitions().len(), 4); // 2 base + 2 LLM
            assert_eq!(causal_discovery::definitions().len(), 2);
        }
        #[cfg(not(feature = "llm"))]
        {
            assert_eq!(graph::definitions().len(), 2);
            assert_eq!(causal_discovery::definitions().len(), 0);
        }
    }
}

//! Tool definitions per PRD v6 Section 10 (50 tools total).
//!
//! Includes 17 original tools (inject_context merged into store_memory)
//! plus 4 sequence tools for E4 integration
//! plus 2 causal tools for E5 Priority 1 enhancement
//! plus 2 causal discovery tools for E5 LLM-based relationship discovery
//! plus 1 keyword tool for E6 keyword search enhancement
//! plus 1 code tool for E7 code search enhancement
//! plus 2 graph tools for E8 upgrade (Phase 4)
//! plus 1 robustness tool for E9 typo-tolerant search
//! plus 6 entity tools for E11 integration (extract, search, infer, find, validate, graph)
//! plus 4 embedder-first search tools for Constitution v6.3
//! plus 2 temporal tools for E2/E3 (search_recent, search_periodic)
//! plus 4 graph linking tools (get_memory_neighbors, get_typed_edges, traverse_graph, get_unified_neighbors)
//! plus 1 maintenance tool (repair_causal_relationships).

pub(crate) mod causal;
pub(crate) mod causal_discovery;
pub(crate) mod code;
pub(crate) mod core;
pub(crate) mod daemon;
pub(crate) mod curation;
pub(crate) mod embedder;
pub(crate) mod entity;
pub(crate) mod file_watcher;
pub(crate) mod graph;
pub(crate) mod graph_link;
pub(crate) mod keyword;
pub(crate) mod maintenance;
pub(crate) mod merge;
pub(crate) mod provenance;
pub(crate) mod robustness;
pub(crate) mod sequence;
pub(crate) mod temporal;
pub(crate) mod topic;

use crate::tools::types::ToolDefinition;

/// Get all tool definitions for the `tools/list` response.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(56);

    // Core tools (4 - inject_context merged into store_memory)
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

    // Causal tools (2) - E5 Priority 1 enhancement
    tools.extend(causal::definitions());

    // Causal discovery tools (2) - E5 LLM-based relationship discovery
    tools.extend(causal_discovery::definitions());

    // Keyword tools (1) - E6 keyword search enhancement
    tools.extend(keyword::definitions());

    // Code tools (1) - E7 code search enhancement
    tools.extend(code::definitions());

    // Graph tools (2) - E8 upgrade (Phase 4)
    tools.extend(graph::definitions());

    // Robustness tools (1) - E9 typo-tolerant search
    tools.extend(robustness::definitions());

    // Entity tools (6) - E11 integration
    tools.extend(entity::definitions());

    // Embedder-first search tools (7) - Constitution v6.3 + NAV-GAP tools
    tools.extend(embedder::definitions());

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

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_tool_count() {
        // Without LLM: 51 tools
        // With LLM (default): 55 tools (+2 causal_discovery, +2 graph LLM)
        // core: 4, merge: 1, curation: 2, topic: 4, file_watcher: 4, sequence: 4,
        // causal: 4, keyword: 1, code: 1, robustness: 1, entity: 6, embedder: 7,
        // temporal: 2, graph_link: 4, maintenance: 1, provenance: 3
        // = 49 base + graph: 2 (non-LLM)
        // + LLM: causal_discovery: 2, graph LLM: 2 = 4 extra
        #[cfg(feature = "llm")]
        assert_eq!(get_tool_definitions().len(), 56);
        #[cfg(not(feature = "llm"))]
        assert_eq!(get_tool_definitions().len(), 52);
    }

    #[test]
    fn test_all_tool_names_present() {
        let tools = get_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        let expected = [
            // Core tools (4 - inject_context merged into store_memory)
            "store_memory",
            "get_memetic_status",
            "search_graph",
            "trigger_consolidation",
            // Merge tool (1)
            "merge_concepts",
            // Curation tools (2)
            "forget_concept",
            "boost_importance",
            // Topic tools (4)
            "get_topic_portfolio",
            "get_topic_stability",
            "detect_topics",
            "get_divergence_alerts",
            // File watcher tools (4)
            "list_watched_files",
            "get_file_watcher_stats",
            "delete_file_content",
            "reconcile_files",
            // Sequence tools (4) - E4 integration
            "get_conversation_context",
            "get_session_timeline",
            "traverse_memory_chain",
            "compare_session_states",
            // Causal tools (2) - E5 Priority 1 enhancement
            "search_causes",
            "get_causal_chain",
            // Keyword tools (1) - E6 keyword search enhancement
            "search_by_keywords",
            // Code tools (1) - E7 code search enhancement
            "search_code",
            // Graph tools (2 base) - E8 upgrade (Phase 4)
            "search_connections",
            "get_graph_path",
            // Robustness tools (1) - E9 typo-tolerant search
            "search_robust",
            // Entity tools (6) - E11 integration
            "extract_entities",
            "search_by_entities",
            "infer_relationship",
            "find_related_entities",
            "validate_knowledge",
            "get_entity_graph",
            // Embedder-first search tools (7) - Constitution v6.3 + NAV-GAP
            "search_by_embedder",
            "get_embedder_clusters",
            "compare_embedder_views",
            "list_embedder_indexes",
            "get_memory_fingerprint",
            "create_weight_profile",
            "search_cross_embedder_anomalies",
            // Temporal tools (2) - E2 recency search, E3 periodic search
            "search_recent",
            "search_periodic",
            // Graph linking tools (4) - K-NN navigation and typed edges
            "get_memory_neighbors",
            "get_typed_edges",
            "traverse_graph",
            "get_unified_neighbors",
            // Maintenance tools (1) - Data repair and cleanup
            "repair_causal_relationships",
            // Provenance tools (3) - Phase P3 provenance queries
            "get_audit_trail",
            "get_merge_history",
            "get_provenance_chain",
            // Daemon tools (1) - Multi-agent observability
            "daemon_status",
        ];

        for name in expected {
            assert!(names.contains(&name), "Missing tool: {}", name);
        }

        // LLM-dependent tools only present when llm feature is enabled
        #[cfg(feature = "llm")]
        {
            let llm_tools = [
                "trigger_causal_discovery",
                "get_causal_discovery_status",
                "discover_graph_relationships",
                "validate_graph_link",
            ];
            for name in llm_tools {
                assert!(names.contains(&name), "Missing LLM tool: {}", name);
            }
        }
    }

    #[test]
    fn test_no_duplicate_tools() {
        let tools = get_tool_definitions();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        let len_before = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), len_before);
    }

    #[test]
    fn test_all_tools_have_descriptions_and_schemas() {
        for tool in &get_tool_definitions() {
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
        assert_eq!(core::definitions().len(), 4); // inject_context merged into store_memory
        assert_eq!(merge::definitions().len(), 1);
        assert_eq!(curation::definitions().len(), 2);
        assert_eq!(topic::definitions().len(), 4);
        assert_eq!(file_watcher::definitions().len(), 4);
        assert_eq!(sequence::definitions().len(), 4);
        assert_eq!(causal::definitions().len(), 4); // search_causal_relationships, search_causes, search_effects, get_causal_chain
        assert_eq!(causal_discovery::definitions().len(), 2); // E5 LLM-based relationship discovery
        assert_eq!(keyword::definitions().len(), 1);
        assert_eq!(code::definitions().len(), 1);
        assert_eq!(graph::definitions().len(), 4); // search_connections, get_graph_path, discover_graph_relationships, validate_graph_link
        assert_eq!(robustness::definitions().len(), 1); // E9 typo-tolerant search
        assert_eq!(entity::definitions().len(), 6);
        assert_eq!(embedder::definitions().len(), 7); // Constitution v6.3 embedder-first search + NAV-GAP tools
        assert_eq!(temporal::definitions().len(), 2); // E2 recency search, E3 periodic search
        assert_eq!(graph_link::definitions().len(), 4); // K-NN navigation, typed edges, unified neighbors
        assert_eq!(maintenance::definitions().len(), 1); // Data repair and cleanup
        assert_eq!(provenance::definitions().len(), 3); // P3 provenance queries
        assert_eq!(daemon::definitions().len(), 1); // Multi-agent observability
    }
}

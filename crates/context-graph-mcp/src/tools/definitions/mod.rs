//! Tool definitions per PRD v6 Section 10 (36 tools total).
//!
//! Includes 18 original tools plus 4 sequence tools for E4 integration
//! plus 2 causal tools for E5 Priority 1 enhancement
//! plus 2 graph tools for E8 upgrade (Phase 4)
//! plus 4 intent tools for E10 upgrade (intent/context dual embeddings + drift detection)
//! plus 6 entity tools for E11 integration (extract, search, infer, find, validate, graph).

pub(crate) mod causal;
pub(crate) mod core;
pub(crate) mod curation;
pub(crate) mod dream;
pub(crate) mod entity;
pub(crate) mod file_watcher;
pub(crate) mod graph;
pub(crate) mod intent;
pub(crate) mod merge;
pub(crate) mod sequence;
pub(crate) mod topic;

use crate::tools::types::ToolDefinition;

/// Get all tool definitions for the `tools/list` response.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(36);

    // Core tools (5)
    tools.extend(core::definitions());

    // Merge tool (1 - part of curation)
    tools.extend(merge::definitions());

    // Curation tools (2)
    tools.extend(curation::definitions());

    // Topic tools (4)
    tools.extend(topic::definitions());

    // Dream tools (2)
    tools.extend(dream::definitions());

    // File watcher tools (4)
    tools.extend(file_watcher::definitions());

    // Sequence tools (4) - E4 integration
    tools.extend(sequence::definitions());

    // Causal tools (2) - E5 Priority 1 enhancement
    tools.extend(causal::definitions());

    // Graph tools (2) - E8 upgrade (Phase 4)
    tools.extend(graph::definitions());

    // Intent tools (4) - E10 upgrade (intent/context dual embeddings)
    tools.extend(intent::definitions());

    // Entity tools (6) - E11 integration
    tools.extend(entity::definitions());

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_tool_count() {
        assert_eq!(get_tool_definitions().len(), 36);
    }

    #[test]
    fn test_all_tool_names_present() {
        let tools = get_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        let expected = [
            // Core tools (5)
            "inject_context",
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
            // Dream tools (2)
            "trigger_dream",
            "get_dream_status",
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
            // Graph tools (2) - E8 upgrade (Phase 4)
            "search_connections",
            "get_graph_path",
            // Intent tools (4) - E10 upgrade
            "search_by_intent",
            "find_contextual_matches",
            "detect_intent_drift",
            "get_session_intent_history",
            // Entity tools (6) - E11 integration
            "extract_entities",
            "search_by_entities",
            "infer_relationship",
            "find_related_entities",
            "validate_knowledge",
            "get_entity_graph",
        ];

        for name in expected {
            assert!(names.contains(&name), "Missing tool: {}", name);
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
        assert_eq!(core::definitions().len(), 5);
        assert_eq!(merge::definitions().len(), 1);
        assert_eq!(curation::definitions().len(), 2);
        assert_eq!(topic::definitions().len(), 4);
        assert_eq!(dream::definitions().len(), 2);
        assert_eq!(file_watcher::definitions().len(), 4);
        assert_eq!(sequence::definitions().len(), 4);
        assert_eq!(causal::definitions().len(), 2);
        assert_eq!(graph::definitions().len(), 2);
        assert_eq!(intent::definitions().len(), 4);
        assert_eq!(entity::definitions().len(), 6);
    }
}

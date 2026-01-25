//! Tool registry for centralized tool management.
//!
//! TASK-41: Provides O(1) tool lookup and registration verification.
//! This module provides a formal `ToolRegistry` struct that:
//! - Stores tools in a HashMap for O(1) lookup by name
//! - Validates all tools are registered at startup
//! - Ensures no duplicate tool registrations

#![allow(dead_code)]

use std::collections::HashMap;

use super::types::ToolDefinition;

/// Registry holding all MCP tool definitions.
///
/// Provides:
/// - O(1) lookup by tool name
/// - List of all registered tools (sorted by name)
/// - Validation that all tools are registered
/// - Fail-fast on duplicate registrations
pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    /// Create empty registry with pre-allocated capacity.
    pub fn new() -> Self {
        Self {
            tools: HashMap::with_capacity(50),
        }
    }

    /// Register a tool definition.
    ///
    /// # Panics
    ///
    /// Panics if a tool with the same name is already registered.
    pub fn register(&mut self, tool: ToolDefinition) {
        let name = tool.name.clone();
        if self.tools.contains_key(&name) {
            panic!(
                "TASK-41: Duplicate tool registration: '{}'. \
                 Each tool name must be unique.",
                name
            );
        }
        self.tools.insert(name, tool);
    }

    /// Get a tool definition by name.
    pub fn get(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.get(name)
    }

    /// List all registered tools (sorted by name for deterministic output).
    pub fn list(&self) -> Vec<&ToolDefinition> {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    /// Get count of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Check if a tool exists by name.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool names as a sorted vector.
    pub fn tool_names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Register all Context Graph MCP tools.
///
/// Uses existing definitions from tools/definitions/ modules.
///
/// 20 tools are exposed:
///
/// | Category | Count | Source |
/// |----------|-------|--------|
/// | Core | 5 | definitions/core.rs |
/// | Merge | 1 | definitions/merge.rs |
/// | Curation | 2 | definitions/curation.rs |
/// | Topic | 4 | definitions/topic.rs |
/// | File Watcher | 4 | definitions/file_watcher.rs |
/// | Sequence | 4 | definitions/sequence.rs (E4 integration) |
pub fn register_all_tools() -> ToolRegistry {
    use super::definitions;

    let mut registry = ToolRegistry::new();

    // Register core tools (5): inject_context, store_memory, get_memetic_status, search_graph, trigger_consolidation
    for tool in definitions::core::definitions() {
        registry.register(tool);
    }

    // Register merge tools (1): merge_concepts
    for tool in definitions::merge::definitions() {
        registry.register(tool);
    }

    // Register curation tools (2): forget_concept, boost_importance
    for tool in definitions::curation::definitions() {
        registry.register(tool);
    }

    // Register topic tools (4): get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts
    for tool in definitions::topic::definitions() {
        registry.register(tool);
    }

    // Register file watcher tools (4): list_watched_files, get_file_watcher_stats, delete_file_content, reconcile_files
    for tool in definitions::file_watcher::definitions() {
        registry.register(tool);
    }

    // Register sequence tools (4): get_conversation_context, get_session_timeline, traverse_memory_chain, compare_session_states
    for tool in definitions::sequence::definitions() {
        registry.register(tool);
    }

    // Verify expected tool count (16 original + 4 sequence = 20 total)
    let actual_count = registry.len();
    assert_eq!(
        actual_count, 20,
        "Expected 20 tools (16 original + 4 sequence), got {}. Check definitions modules.",
        actual_count
    );

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_registry_new_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_all_tools_count() {
        let registry = register_all_tools();
        // 16 original tools + 4 sequence tools = 20 total
        assert_eq!(registry.len(), 20, "Must have exactly 20 tools (16 original + 4 sequence)");
    }

    #[test]
    #[should_panic(expected = "Duplicate tool registration")]
    fn test_duplicate_registration_panics() {
        let mut registry = ToolRegistry::new();
        let tool = ToolDefinition::new("test_tool", "Test", json!({"type": "object"}));
        registry.register(tool.clone());
        registry.register(tool);
    }

    #[test]
    fn test_get_returns_tool_definition() {
        let registry = register_all_tools();
        let tool = registry.get("inject_context").expect("Must exist");
        assert_eq!(tool.name, "inject_context");
    }

    #[test]
    fn test_get_unknown_tool_returns_none() {
        let registry = register_all_tools();
        assert!(registry.get("nonexistent_tool").is_none());
    }

    #[test]
    fn test_list_returns_all_tools_sorted() {
        let registry = register_all_tools();
        let tools = registry.list();
        // 16 original tools + 4 sequence tools = 20 total
        assert_eq!(tools.len(), 20);

        for i in 1..tools.len() {
            assert!(tools[i - 1].name <= tools[i].name);
        }
    }

    #[test]
    fn test_all_20_tools_registered() {
        let registry = register_all_tools();
        let expected_tools = [
            // Core (5)
            "inject_context",
            "store_memory",
            "get_memetic_status",
            "search_graph",
            "trigger_consolidation",
            // Merge (1)
            "merge_concepts",
            // Curation (2)
            "forget_concept",
            "boost_importance",
            // Topic (4)
            "get_topic_portfolio",
            "get_topic_stability",
            "detect_topics",
            "get_divergence_alerts",
            // File Watcher (4)
            "list_watched_files",
            "get_file_watcher_stats",
            "delete_file_content",
            "reconcile_files",
            // Sequence (4) - E4 integration
            "get_conversation_context",
            "get_session_timeline",
            "traverse_memory_chain",
            "compare_session_states",
        ];

        for name in expected_tools {
            assert!(
                registry.contains(name),
                "Tool '{}' should be registered but is not",
                name
            );
        }
    }
}

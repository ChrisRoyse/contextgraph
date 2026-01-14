//! Tool registry for centralized tool management.
//!
//! TASK-41: Provides O(1) tool lookup and registration verification.
//! This module provides a formal `ToolRegistry` struct that:
//! - Stores tools in a HashMap for O(1) lookup by name
//! - Validates all tools are registered at startup
//! - Ensures no duplicate tool registrations

use std::collections::HashMap;

use super::types::ToolDefinition;

/// Registry holding all MCP tool definitions.
///
/// Provides:
/// - O(1) lookup by tool name
/// - List of all registered tools (sorted by name)
/// - Validation that all tools are registered
/// - Fail-fast on duplicate registrations
///
/// # Example
///
/// ```ignore
/// use context_graph_mcp::tools::{ToolRegistry, register_all_tools};
///
/// let registry = register_all_tools();
/// assert_eq!(registry.len(), 46);
///
/// // O(1) lookup
/// if let Some(tool) = registry.get("inject_context") {
///     println!("Found tool: {}", tool.name);
/// }
/// ```
pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    /// Create empty registry with pre-allocated capacity.
    ///
    /// Pre-allocates space for 50 tools to minimize reallocations.
    pub fn new() -> Self {
        Self {
            tools: HashMap::with_capacity(50), // Room for growth
        }
    }

    /// Register a tool definition.
    ///
    /// # Panics
    ///
    /// Panics if a tool with the same name is already registered.
    /// This is intentional - duplicate tools indicate a bug in the codebase.
    /// FAIL FAST: Do not silently ignore duplicates.
    pub fn register(&mut self, tool: ToolDefinition) {
        let name = tool.name.clone();
        if self.tools.contains_key(&name) {
            panic!(
                "TASK-41: Duplicate tool registration: '{}'. \
                 Each tool name must be unique. Check definitions modules for duplicates.",
                name
            );
        }
        self.tools.insert(name, tool);
    }

    /// Get a tool definition by name.
    ///
    /// Returns `None` if the tool is not registered.
    /// This is O(1) lookup via HashMap.
    pub fn get(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.get(name)
    }

    /// List all registered tools (sorted by name for deterministic output).
    ///
    /// Sorting ensures consistent ordering in `tools/list` responses
    /// and makes testing/debugging easier.
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
    ///
    /// This is O(1) lookup via HashMap.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool names as a sorted vector.
    ///
    /// Useful for debugging and validation.
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

/// Register all 46 Context Graph MCP tools.
///
/// Uses existing definitions from tools/definitions/ modules.
/// FAIL FAST: Panics on duplicate registration or wrong tool count.
///
/// # Tool Categories (46 total)
///
/// | Category | Count | Source |
/// |----------|-------|--------|
/// | Core | 6 | definitions/core.rs |
/// | GWT | 9 | definitions/gwt.rs |
/// | UTL | 1 | definitions/utl.rs |
/// | ATC | 3 | definitions/atc.rs |
/// | Dream | 5 | definitions/dream.rs |
/// | Neuromod | 2 | definitions/neuromod.rs |
/// | Steering | 1 | definitions/steering.rs |
/// | Causal | 1 | definitions/causal.rs |
/// | Teleological | 5 | definitions/teleological.rs |
/// | Autonomous | 7 | definitions/autonomous.rs |
/// | Meta-UTL | 3 | definitions/meta_utl.rs |
/// | Epistemic | 1 | definitions/epistemic.rs |
/// | Merge | 1 | definitions/merge.rs |
/// | Johari | 1 | definitions/johari.rs |
///
/// # Panics
///
/// Panics if:
/// - Any tool name is registered twice (duplicate detection)
/// - Total tool count is not exactly 46 (indicates missing/extra tools)
pub fn register_all_tools() -> ToolRegistry {
    use super::definitions;

    let mut registry = ToolRegistry::new();

    // Register all tools from each category
    // Order matches get_tool_definitions() for consistency
    for tool in definitions::core::definitions() {
        registry.register(tool);
    }
    for tool in definitions::gwt::definitions() {
        registry.register(tool);
    }
    for tool in definitions::utl::definitions() {
        registry.register(tool);
    }
    for tool in definitions::atc::definitions() {
        registry.register(tool);
    }
    for tool in definitions::dream::definitions() {
        registry.register(tool);
    }
    for tool in definitions::neuromod::definitions() {
        registry.register(tool);
    }
    for tool in definitions::steering::definitions() {
        registry.register(tool);
    }
    for tool in definitions::causal::definitions() {
        registry.register(tool);
    }
    for tool in definitions::teleological::definitions() {
        registry.register(tool);
    }
    for tool in definitions::autonomous::definitions() {
        registry.register(tool);
    }
    for tool in definitions::meta_utl::definitions() {
        registry.register(tool);
    }
    for tool in definitions::epistemic::definitions() {
        registry.register(tool);
    }
    for tool in definitions::merge::definitions() {
        registry.register(tool);
    }
    for tool in definitions::johari::definitions() {
        registry.register(tool);
    }

    // FAIL FAST: Verify exactly 46 tools are registered
    let actual_count = registry.len();
    assert_eq!(
        actual_count, 46,
        "TASK-41: Expected 46 tools, got {}. Check definitions modules for missing/extra tools.",
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
    fn test_register_all_tools_returns_46() {
        println!("\n=== FSV TEST: register_all_tools (TASK-41) ===");

        let registry = register_all_tools();

        println!("FSV-1: Tool count = {}", registry.len());
        assert_eq!(registry.len(), 46, "Must have exactly 46 tools");

        // Verify critical tools exist
        let critical_tools = [
            // Core
            "inject_context",
            "store_memory",
            "get_memetic_status",
            "get_graph_manifest",
            "search_graph",
            "utl_status",
            // GWT
            "get_consciousness_state",
            "get_kuramoto_sync",
            "get_workspace_status",
            "get_ego_state",
            "trigger_workspace_broadcast",
            "adjust_coupling",
            "get_coherence_state",
            "get_identity_continuity",
            "get_kuramoto_state",
            // UTL
            "gwt/compute_delta_sc",
            // Dream
            "trigger_dream",
            "get_gpu_status",
            // Epistemic/Merge/Johari
            "epistemic_action",
            "merge_concepts",
            "get_johari_classification",
        ];

        for name in critical_tools {
            assert!(registry.contains(name), "Missing critical tool: {}", name);
            println!("FSV-2: Tool '{}' registered", name);
        }

        println!("\n=== FSV EVIDENCE (TASK-41) ===");
        println!(" 46 tools registered");
        println!(" All critical tools present");
        println!("=== FSV TEST PASSED (TASK-41) ===\n");
    }

    #[test]
    #[should_panic(expected = "Duplicate tool registration")]
    fn test_duplicate_registration_panics() {
        let mut registry = ToolRegistry::new();
        let tool = ToolDefinition::new("test_tool", "Test", json!({"type": "object"}));
        registry.register(tool.clone());
        registry.register(tool); // Should panic
    }

    #[test]
    fn test_get_returns_tool_definition() {
        let registry = register_all_tools();
        let tool = registry.get("inject_context").expect("Must exist");
        assert_eq!(tool.name, "inject_context");
        assert!(!tool.description.is_empty());
    }

    #[test]
    fn test_get_unknown_tool_returns_none() {
        let registry = register_all_tools();
        assert!(registry.get("nonexistent_tool").is_none());
    }

    #[test]
    fn test_get_empty_string_returns_none() {
        let registry = register_all_tools();
        assert!(registry.get("").is_none());
    }

    #[test]
    fn test_namespaced_tool_accessible() {
        let registry = register_all_tools();
        // gwt/compute_delta_sc has a slash in name
        assert!(
            registry.get("gwt/compute_delta_sc").is_some(),
            "Namespaced tool 'gwt/compute_delta_sc' must be accessible"
        );
    }

    #[test]
    fn test_list_returns_all_tools_sorted() {
        let registry = register_all_tools();
        let tools = registry.list();
        assert_eq!(tools.len(), 46);

        // Verify sorted by name
        for i in 1..tools.len() {
            assert!(
                tools[i - 1].name <= tools[i].name,
                "Tools not sorted: {} > {}",
                tools[i - 1].name,
                tools[i].name
            );
        }
    }

    #[test]
    fn test_tool_names_returns_sorted_names() {
        let registry = register_all_tools();
        let names = registry.tool_names();
        assert_eq!(names.len(), 46);

        // Verify sorted
        for i in 1..names.len() {
            assert!(names[i - 1] <= names[i], "Names not sorted: {} > {}", names[i - 1], names[i]);
        }
    }

    #[test]
    fn test_contains_returns_correct_values() {
        let registry = register_all_tools();

        // These should exist
        assert!(registry.contains("inject_context"));
        assert!(registry.contains("get_consciousness_state"));
        assert!(registry.contains("gwt/compute_delta_sc"));

        // These should not exist
        assert!(!registry.contains("nonexistent"));
        assert!(!registry.contains(""));
        assert!(!registry.contains("INJECT_CONTEXT")); // Case sensitive
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = ToolRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    /// Verify all tool categories are represented.
    #[test]
    fn test_all_categories_represented() {
        let registry = register_all_tools();

        // Verify at least one tool from each category
        let category_representatives = [
            ("Core", "inject_context"),
            ("GWT", "get_consciousness_state"),
            ("UTL", "gwt/compute_delta_sc"),
            ("ATC", "get_threshold_status"),
            ("Dream", "trigger_dream"),
            ("Neuromod", "get_neuromodulation_state"),
            ("Steering", "get_steering_feedback"),
            ("Causal", "omni_infer"),
            ("Teleological", "search_teleological"),
            ("Autonomous", "auto_bootstrap_north_star"),
            ("Meta-UTL", "get_meta_learning_status"),
            ("Epistemic", "epistemic_action"),
            ("Merge", "merge_concepts"),
            ("Johari", "get_johari_classification"),
        ];

        for (category, tool_name) in category_representatives {
            assert!(
                registry.contains(tool_name),
                "Category '{}' not represented - missing tool '{}'",
                category,
                tool_name
            );
        }
    }
}

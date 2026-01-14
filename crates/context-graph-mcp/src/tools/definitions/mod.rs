//! Tool definitions organized by functional category.
//!
//! Each submodule provides tool definitions for a specific category of MCP tools.
//! TASK-41: Changed private modules to pub(crate) for ToolRegistry access.

pub(crate) mod atc;
pub(crate) mod autonomous;
pub(crate) mod causal;
pub(crate) mod core;
pub(crate) mod dream;
pub mod epistemic;
pub(crate) mod gwt;
pub mod johari;
pub mod merge;
pub(crate) mod meta_utl;
pub(crate) mod neuromod;
pub(crate) mod steering;
pub(crate) mod teleological;
pub(crate) mod utl;

use crate::tools::types::ToolDefinition;

/// Get all tool definitions for the `tools/list` response.
///
/// Returns the complete list of MCP tools exposed by the Context Graph server.
/// Currently returns 45 tools across 14 categories.
/// TASK-37: Added get_gpu_status tool (Dream tools now 5).
/// TASK-38: Added get_identity_continuity tool (GWT tools now 8).
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(45);

    // Core tools (6)
    tools.extend(core::definitions());

    // GWT tools (8) - TASK-33/34 added get_coherence_state, TASK-38 added get_identity_continuity
    tools.extend(gwt::definitions());

    // UTL tools (1)
    tools.extend(utl::definitions());

    // ATC tools (3)
    tools.extend(atc::definitions());

    // Dream tools (5) - TASK-37: Added get_gpu_status
    tools.extend(dream::definitions());

    // Neuromod tools (2)
    tools.extend(neuromod::definitions());

    // Steering tools (1)
    tools.extend(steering::definitions());

    // Causal tools (1)
    tools.extend(causal::definitions());

    // Teleological tools (5)
    tools.extend(teleological::definitions());

    // Autonomous tools (7)
    tools.extend(autonomous::definitions());

    // Meta-UTL tools (3) - TASK-METAUTL-P0-005
    tools.extend(meta_utl::definitions());

    // Epistemic tools (1) - TASK-MCP-001
    tools.extend(epistemic::definitions());

    // Merge tools (1) - TASK-MCP-003
    tools.extend(merge::definitions());

    // Johari classification tools (1) - TASK-MCP-005
    tools.extend(johari::definitions());

    tools
}

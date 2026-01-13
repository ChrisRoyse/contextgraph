//! Tool definitions organized by functional category.
//!
//! Each submodule provides tool definitions for a specific category of MCP tools.

mod atc;
mod autonomous;
mod causal;
mod core;
mod dream;
pub mod epistemic;
mod gwt;
pub mod merge;
mod meta_utl;
mod neuromod;
mod steering;
mod teleological;
mod utl;

use crate::tools::types::ToolDefinition;

/// Get all tool definitions for the `tools/list` response.
///
/// Returns the complete list of MCP tools exposed by the Context Graph server.
/// Currently returns 41 tools across 13 categories.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(41);

    // Core tools (6)
    tools.extend(core::definitions());

    // GWT tools (6)
    tools.extend(gwt::definitions());

    // UTL tools (1)
    tools.extend(utl::definitions());

    // ATC tools (3)
    tools.extend(atc::definitions());

    // Dream tools (4)
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

    tools
}

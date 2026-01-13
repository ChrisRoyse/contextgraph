//! MCP tool call handlers.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! NO BACKWARDS COMPATIBILITY with legacy MemoryStore.
//!
//! All tool responses include `_cognitive_pulse` with live UTL metrics.
//! This provides real-time cognitive state in every MCP response.
//!
//! # Constitution Reference
//!
//! Johari quadrant to action mapping (constitution.yaml:159-163):
//! - Open: delta_s < 0.5, delta_c > 0.5 -> DirectRecall
//! - Blind: delta_s > 0.5, delta_c < 0.5 -> TriggerDream
//! - Hidden: delta_s < 0.5, delta_c < 0.5 -> GetNeighborhood
//! - Unknown: delta_s > 0.5, delta_c > 0.5 -> EpistemicAction
//!
//! # Module Organization
//!
//! - `dispatch` - Tool dispatch logic (handle_tools_list, handle_tools_call)
//! - `helpers` - MCP result helpers with CognitivePulse injection
//! - `memory_tools` - Memory operations (inject_context, store_memory, search_graph)
//! - `status_tools` - Status queries (get_memetic_status, get_graph_manifest, utl_status)
//! - `gwt_consciousness` - GWT consciousness tools (get_consciousness_state, get_kuramoto_sync, get_ego_state)
//! - `gwt_workspace` - GWT workspace tools (get_workspace_status, trigger_workspace_broadcast, adjust_coupling)

mod dispatch;
mod gwt_consciousness;
mod gwt_workspace;
mod helpers;
mod memory_tools;
mod meta_learning_tools;
mod status_tools;

// All implementations are on the Handlers struct, so no re-exports needed.
// The modules add impl blocks to Handlers which are automatically available.

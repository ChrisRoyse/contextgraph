//! Tool names as constants for dispatch matching.

// ========== CORE TOOLS ==========

pub const INJECT_CONTEXT: &str = "inject_context";
pub const STORE_MEMORY: &str = "store_memory";
pub const GET_MEMETIC_STATUS: &str = "get_memetic_status";
pub const GET_GRAPH_MANIFEST: &str = "get_graph_manifest";
pub const SEARCH_GRAPH: &str = "search_graph";
pub const UTL_STATUS: &str = "utl_status";

// ========== GWT TOOLS (TASK-GWT-001) ==========

/// TASK-GWT-001: Get consciousness state from GWT/Kuramoto system
pub const GET_CONSCIOUSNESS_STATE: &str = "get_consciousness_state";
/// TASK-GWT-001: Get Kuramoto oscillator network synchronization state
pub const GET_KURAMOTO_SYNC: &str = "get_kuramoto_sync";
/// TASK-GWT-001: Get Global Workspace status (active memory, competing, broadcast)
pub const GET_WORKSPACE_STATUS: &str = "get_workspace_status";
/// TASK-GWT-001: Get Self-Ego Node state (purpose vector, identity continuity)
pub const GET_EGO_STATE: &str = "get_ego_state";
/// TASK-GWT-001: Trigger workspace broadcast with a memory
pub const TRIGGER_WORKSPACE_BROADCAST: &str = "trigger_workspace_broadcast";
/// TASK-GWT-001: Adjust Kuramoto coupling strength K
pub const ADJUST_COUPLING: &str = "adjust_coupling";

// ========== UTL TOOLS (TASK-UTL-P1-001) ==========

/// TASK-UTL-P1-001: Compute per-embedder delta-S and aggregate delta-C
pub const COMPUTE_DELTA_SC: &str = "gwt/compute_delta_sc";

// ========== ADAPTIVE THRESHOLD CALIBRATION (ATC) TOOLS (TASK-ATC-001) ==========

/// TASK-ATC-001: Get current ATC threshold status
pub const GET_THRESHOLD_STATUS: &str = "get_threshold_status";
/// TASK-ATC-001: Get calibration quality metrics (ECE, MCE, Brier)
pub const GET_CALIBRATION_METRICS: &str = "get_calibration_metrics";
/// TASK-ATC-001: Manually trigger recalibration at a specific level
pub const TRIGGER_RECALIBRATION: &str = "trigger_recalibration";

// ========== DREAM TOOLS (TASK-DREAM-MCP) ==========

/// TASK-DREAM-MCP: Manually trigger a dream consolidation cycle
pub const TRIGGER_DREAM: &str = "trigger_dream";
/// TASK-DREAM-MCP: Get current dream system status
pub const GET_DREAM_STATUS: &str = "get_dream_status";
/// TASK-DREAM-MCP: Abort current dream cycle
pub const ABORT_DREAM: &str = "abort_dream";
/// TASK-DREAM-MCP: Get shortcut candidates from amortized learning
pub const GET_AMORTIZED_SHORTCUTS: &str = "get_amortized_shortcuts";

// ========== NEUROMODULATION TOOLS (TASK-NEUROMOD-MCP) ==========

/// TASK-NEUROMOD-MCP: Get all 4 neuromodulator levels
pub const GET_NEUROMODULATION_STATE: &str = "get_neuromodulation_state";
/// TASK-NEUROMOD-MCP: Adjust a specific modulator
pub const ADJUST_NEUROMODULATOR: &str = "adjust_neuromodulator";

// ========== STEERING TOOLS (TASK-STEERING-001) ==========

/// TASK-STEERING-001: Get steering feedback from Gardener, Curator, Assessor
pub const GET_STEERING_FEEDBACK: &str = "get_steering_feedback";

// ========== CAUSAL INFERENCE TOOLS (TASK-CAUSAL-001) ==========

/// TASK-CAUSAL-001: Perform omni-directional causal inference
pub const OMNI_INFER: &str = "omni_infer";

// NOTE: Manual North Star tools have been REMOVED.
// They created single 1024D embeddings incompatible with 13-embedder teleological arrays.
// Use the autonomous system tools which work with proper teleological embeddings.

// ========== TELEOLOGICAL TOOLS (TELEO-007 through TELEO-011) ==========

/// TELEO-007: Cross-correlation search across all 13 embedders
pub const SEARCH_TELEOLOGICAL: &str = "search_teleological";
/// TELEO-008: Compute full 13-embedder teleological vector
pub const COMPUTE_TELEOLOGICAL_VECTOR: &str = "compute_teleological_vector";
/// TELEO-009: Fuse embeddings using synergy matrix
pub const FUSE_EMBEDDINGS: &str = "fuse_embeddings";
/// TELEO-010: Adaptively update synergy matrix from feedback
pub const UPDATE_SYNERGY_MATRIX: &str = "update_synergy_matrix";
/// TELEO-011: CRUD operations for task-specific teleological profiles
pub const MANAGE_TELEOLOGICAL_PROFILE: &str = "manage_teleological_profile";

// ========== AUTONOMOUS TOOLS (TASK-AUTONOMOUS-MCP) ==========

/// TASK-AUTONOMOUS-MCP: Bootstrap autonomous system from existing North Star
pub const AUTO_BOOTSTRAP_NORTH_STAR: &str = "auto_bootstrap_north_star";
/// TASK-AUTONOMOUS-MCP: Get current drift state and history
pub const GET_ALIGNMENT_DRIFT: &str = "get_alignment_drift";
/// TASK-AUTONOMOUS-MCP: Manually trigger drift correction
pub const TRIGGER_DRIFT_CORRECTION: &str = "trigger_drift_correction";
/// TASK-AUTONOMOUS-MCP: Get memories that are candidates for pruning
pub const GET_PRUNING_CANDIDATES: &str = "get_pruning_candidates";
/// TASK-AUTONOMOUS-MCP: Trigger memory consolidation
pub const TRIGGER_CONSOLIDATION: &str = "trigger_consolidation";
/// TASK-AUTONOMOUS-MCP: Discover potential sub-goals from memory clusters
pub const DISCOVER_SUB_GOALS: &str = "discover_sub_goals";
/// TASK-AUTONOMOUS-MCP: Get comprehensive autonomous system status
pub const GET_AUTONOMOUS_STATUS: &str = "get_autonomous_status";

// ========== META-UTL TOOLS (TASK-MCP-P0-001) ==========

/// TASK-MCP-P0-001: Get current self-correction status
pub const GET_META_LEARNING_STATUS: &str = "get_meta_learning_status";
/// TASK-MCP-P0-001: Manually trigger lambda recalibration
pub const TRIGGER_LAMBDA_RECALIBRATION: &str = "trigger_lambda_recalibration";
/// TASK-MCP-P0-001: Query meta-learning event log
pub const GET_META_LEARNING_LOG: &str = "get_meta_learning_log";

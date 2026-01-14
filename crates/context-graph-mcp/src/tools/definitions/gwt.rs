//! GWT (Global Workspace Theory) tool definitions.
//! TASK-GWT-001: Consciousness state, Kuramoto sync, workspace, ego, broadcast, coupling.
//! TASK-34: High-level coherence state tool.
//! TASK-38: Identity continuity focused tool.

use serde_json::json;
use crate::tools::types::ToolDefinition;

/// Returns GWT tool definitions (8 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // get_consciousness_state - GWT consciousness state (TASK-GWT-001)
        ToolDefinition::new(
            "get_consciousness_state",
            "Get current consciousness state including Kuramoto sync (r), consciousness level (C), \
             meta-cognitive score, differentiation, workspace status, and identity coherence. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID for consciousness tracking (optional, uses default if not provided)"
                    }
                },
                "required": []
            }),
        ),

        // get_kuramoto_sync - Kuramoto oscillator network synchronization (TASK-GWT-001)
        ToolDefinition::new(
            "get_kuramoto_sync",
            "Get Kuramoto oscillator network synchronization state including order parameter (r), \
             mean phase (psi), all 13 oscillator phases, natural frequencies, and coupling strength. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID (optional, uses default if not provided)"
                    }
                },
                "required": []
            }),
        ),

        // get_workspace_status - Global Workspace status (TASK-GWT-001)
        ToolDefinition::new(
            "get_workspace_status",
            "Get Global Workspace status including active memory, competing candidates, \
             broadcast state, and coherence threshold. Returns WTA selection details. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID (optional, uses default if not provided)"
                    }
                },
                "required": []
            }),
        ),

        // get_ego_state - Self-Ego Node state (TASK-GWT-001, TASK-IDENTITY-P0-007)
        ToolDefinition::new(
            "get_ego_state",
            "Get Self-Ego Node state including purpose vector (13D), identity continuity, \
             coherence with actions, trajectory length, and crisis detection state. \
             TASK-IDENTITY-P0-007: Response includes identity_continuity object with: \
             ic (0.0-1.0), status (Healthy/Warning/Degraded/Critical), in_crisis (bool), \
             history_len (int), and last_detection (CrisisDetectionResult or null). \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID (optional, uses default if not provided)"
                    }
                },
                "required": []
            }),
        ),

        // trigger_workspace_broadcast - Trigger WTA selection (TASK-GWT-001)
        ToolDefinition::new(
            "trigger_workspace_broadcast",
            "Trigger winner-take-all workspace broadcast with a specific memory. \
             Forces memory into workspace competition. Requires write lock on workspace. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "memory_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of memory to broadcast into workspace"
                    },
                    "importance": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.8,
                        "description": "Importance score for the memory [0.0, 1.0]"
                    },
                    "alignment": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.8,
                        "description": "North star alignment score [0.0, 1.0]"
                    },
                    "force": {
                        "type": "boolean",
                        "default": false,
                        "description": "Force broadcast even if below coherence threshold"
                    }
                },
                "required": ["memory_id"]
            }),
        ),

        // adjust_coupling - Adjust Kuramoto coupling strength (TASK-GWT-001)
        ToolDefinition::new(
            "adjust_coupling",
            "Adjust Kuramoto oscillator network coupling strength K. \
             Higher K leads to faster synchronization. K is clamped to [0, 10]. \
             Returns old and new K values plus predicted order parameter r. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "new_K": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 10,
                        "description": "New coupling strength K (clamped to [0, 10])"
                    }
                },
                "required": ["new_K"]
            }),
        ),

        // get_coherence_state - High-level coherence summary (TASK-34)
        ToolDefinition::new(
            "get_coherence_state",
            "Get high-level GWT workspace coherence state. Returns Kuramoto order parameter, \
             coherence level classification (High/Medium/Low), workspace broadcasting status, \
             and conflict detection status. Use get_kuramoto_sync for detailed oscillator data. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "include_phases": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include all 13 oscillator phases in response (optional)"
                    }
                },
                "required": []
            }),
        ),

        // get_identity_continuity - Focused IC status (TASK-38)
        ToolDefinition::new(
            "get_identity_continuity",
            "Get focused identity continuity (IC) status. Returns IC value (0.0-1.0), \
             status classification (Healthy/Warning/Degraded/Critical), in_crisis flag, \
             and thresholds. Unlike get_ego_state (full purpose vector, trajectory), this \
             provides a minimal snapshot for monitoring identity health without bulk data. \
             Requires GWT providers to be initialized via with_gwt() constructor.",
            json!({
                "type": "object",
                "properties": {
                    "include_history": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include recent IC history values (optional, up to 10 entries)"
                    }
                },
                "required": []
            }),
        ),
    ]
}

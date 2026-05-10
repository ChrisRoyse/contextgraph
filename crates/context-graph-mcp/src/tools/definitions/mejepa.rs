//! ME-JEPA Phase 4 inference MCP tool definitions.

use crate::tools::names as tool_names;
use crate::tools::types::ToolDefinition;
use serde_json::json;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            tool_names::MEJEPA_VERIFY,
            "Verify a candidate patch through the ME-JEPA Phase 4 inference gate ladder. This surface currently exposes the deterministic Phase 4 harness only when CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY is explicitly enabled; otherwise it fails closed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["patch", "context"],
                "properties": {
                    "patch": {"type": "object", "additionalProperties": true},
                    "context": {"type": "object", "additionalProperties": true},
                    "includeProvenance": {"type": "boolean", "default": false}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PREDICT_LATEST,
            "Read the latest persisted ME-JEPA RealityPrediction rows for a session from CF_MEJEPA_LIVE_PREDICTIONS.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 10
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_CONSTELLATION_INSPECT,
            "Inspect a persisted ME-JEPA TCT constellation from CF_MEJEPA_CONSTELLATION. Returns metadata, counts, support, and thresholds only; raw centroid vectors are never returned.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["runtimeEmbedderVersions"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_TCT_DB is required."
                    },
                    "versionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{64}$"
                    },
                    "runtimeEmbedderVersions": {
                        "type": "object",
                        "additionalProperties": {
                            "type": "string",
                            "pattern": "^[0-9a-fA-F]{64}$"
                        },
                        "minProperties": 21
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_HEAL_STATUS,
            "Inspect the persisted ME-JEPA Phase 5 self-healing RocksDB source of truth: CF counts, active pointers, and latest heal report metadata. Refuses to guess dbPath unless CONTEXTGRAPH_MEJEPA_HEAL_DB is set.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_ROLLBACK_TO,
            "Rollback ME-JEPA active weights to a previously promoted witness-chain offset. The target must exist in CF_MEJEPA_HEAL_REPORTS and the weight blob must exist in CF_MEJEPA_WEIGHT_BLOBS; otherwise the tool fails closed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["targetWitnessChainOffset", "witnessChainPath"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    },
                    "witnessChainPath": {
                        "type": "string"
                    },
                    "targetWitnessChainOffset": {
                        "type": "integer",
                        "minimum": 0
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_GC_RUN,
            "Run the Phase 6 ME-JEPA hygiene GC against a real RocksDB source of truth: tier demotion, compaction, quota enforcement, checkpoint/calibration retention, witness compression, witness verification, and CF_MEJEPA_GC_HISTORY readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "archiveRoot"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "archiveRoot": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_QUOTA_STATUS,
            "Read Phase 6 ME-JEPA quota status by scanning real RocksDB column families and witness archive files. No cached counters are trusted.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "archiveRoot"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "archiveRoot": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_WITNESS_COMPRESS,
            "Compress eligible old ME-JEPA witness-chain segments into Merkle-root entries, persist archives to archiveRoot, and verify chain integrity by readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "archiveRoot"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "archiveRoot": {"type": "string"},
                    "segmentSize": {"type": "integer", "minimum": 1, "default": 1024},
                    "minAgeDays": {"type": "integer", "minimum": 0, "default": 1}
                }
            }),
        ),
    ]
}

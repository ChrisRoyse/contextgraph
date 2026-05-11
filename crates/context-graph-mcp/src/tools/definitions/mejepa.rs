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
            tool_names::MEJEPA_OBSERVE_SHIFT,
            "Replay a durable reality-shift log entry through the Phase 7 ME-JEPA subscriber. Fails closed if the subscriber replay channel is not available.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["shiftId"],
                "properties": {
                    "shiftId": {
                        "type": "string",
                        "pattern": "^01J[0-9A-F]{20}$"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SUBSCRIBER_STATUS,
            "Read the Phase 7 shift-subscriber persisted watermarks and runtime health snapshot.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_CAPTURE_AUDIT,
            "Assemble a Phase 7 ME-JEPA signal-capture audit bundle for an attempt from persisted panel and inference RocksDB sources of truth.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["attemptId"],
                "properties": {
                    "attemptId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "page": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0
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
        ToolDefinition::new(
            tool_names::MEJEPA_EVAL_RUN,
            "Run the Phase 8 ME-JEPA holdout evaluation against a real RocksDB source of truth, persist CF_MEJEPA_EVAL_REPORTS, and read back the report hash before returning.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "repoRoot", "outputFsv", "reportDate"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "repoRoot": {"type": "string"},
                    "outputFsv": {"type": "string"},
                    "reportDate": {"type": "string", "minLength": 1}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SHIP_GATE_STATUS,
            "Read the latest Phase 8 EvalReport from CF_MEJEPA_EVAL_REPORTS and return ship-gate status with persisted report hash.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath"],
                "properties": {
                    "dbPath": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_ACTIVE_LEARNING_QUEUE,
            "Read the Phase 8 active-learning queue from CF_MEJEPA_ACTIVE_LEARNING_QUEUE.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath"],
                "properties": {
                    "dbPath": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_EVAL_BUILD_GRAPH,
            "Build and persist the Phase 8 patch-similarity graph into CF_MEJEPA_TASK_GRAPH and verify readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "outputFsv"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "outputFsv": {"type": "string"},
                    "threshold": {"type": "number", "minimum": -1.0, "maximum": 1.0, "default": 0.85},
                    "topK": {"type": "integer", "minimum": 1, "default": 3}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
            "Compute the UTML pairwise mutual-information audit from real per-slot series, persist the CSV matrix, reload the CSV, and return the readback summary.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["outputDir", "step", "seriesBySlot"],
                "properties": {
                    "outputDir": {
                        "type": "string",
                        "description": "Directory under CONTEXTGRAPH_DATA_ROOT where the audit CSV is written"
                    },
                    "step": {
                        "type": "integer",
                        "minimum": 0
                    },
                    "periodSteps": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 1
                    },
                    "seriesBySlot": {
                        "type": "object",
                        "minProperties": 2,
                        "additionalProperties": {
                            "type": "array",
                            "minItems": 2,
                            "items": {"type": "number"}
                        }
                    }
                }
            }),
        ),
    ]
}

//! ME-JEPA Phase 4 inference MCP tool definitions.

use crate::tools::names as tool_names;
use crate::tools::types::ToolDefinition;
use serde_json::{json, Value};

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
            tool_names::MEJEPA_PREDICT_WHAT_IF,
            "Compile a non-mutating ME-JEPA Phase B what-if prediction for a candidate patch. Reads calibration/train-cert state from the real inference RocksDB, refuses to run unless the deterministic harness is explicitly enabled, and returns repository SHA readback proving no touched file was modified.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["patch", "context"],
                "properties": {
                    "patch": {"type": "object", "additionalProperties": true},
                    "context": {"type": "object", "additionalProperties": true},
                    "compareToPredictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_EXPLAIN_PREDICTION,
            "Explain a persisted ME-JEPA Phase B RealityPrediction by reading CF_MEJEPA_LIVE_PREDICTIONS and returning stored verdict, top risks, claim reconciliation, reality impact, provenance, and a saliency map normalized from covered chunks.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_INSPECT_PREDICTION,
            "Inspect a persisted ME-JEPA RealityPrediction by reading CF_MEJEPA_LIVE_PREDICTIONS plus required per-chunk CF_MEJEPA_DDA_SIGNALS rows, returning storage provenance, witness/panel identifiers, conformal computation trace, TCT guard cells, and contributing chunk signal vectors.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_REPLAY_PREDICTION,
            "Replay a persisted ME-JEPA RealityPrediction byte-for-byte from CF_MEJEPA_LIVE_PREDICTIONS by validating the key/payload provenance and reserializing the decoded prediction.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_RECORD_AGENT_FEEDBACK,
            "Persist measured agent feedback for a prior ME-JEPA RealityPrediction into CF_MEJEPA_AGENT_FEEDBACK, update the active-learning queue when the feedback is surprise-like, and verify RocksDB readback before returning.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId", "feedbackKind", "agentExplanation", "severity"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "agentId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256,
                        "default": "anonymous",
                        "description": "Configured signed agent id. Omit or set to anonymous for unauthenticated feedback."
                    },
                    "identityAttestation": identity_attestation_schema(),
                    "feedbackKind": {
                        "type": "string",
                        "enum": ["confirmed", "surprise", "omission", "calibration"]
                    },
                    "agentExplanation": {
                        "type": "string",
                        "maxLength": 4096
                    },
                    "actualOutcome": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["oracleOutcome", "failedTests", "notes"],
                        "properties": {
                            "oracleOutcome": {
                                "type": "string",
                                "enum": ["pass", "fail", "out_of_distribution", "abstain"]
                            },
                            "failedTests": {
                                "type": "array",
                                "items": {"type": "string", "minLength": 1},
                                "maxItems": 1024
                            },
                            "runtimeMs": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 86400000
                            },
                            "notes": {"type": "string"}
                        }
                    },
                    "severity": {
                        "type": "string",
                        "enum": ["low", "medium", "high", "catastrophic"]
                    },
                    "extraStructuredData": {
                        "type": "object",
                        "additionalProperties": true,
                        "default": {}
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
            tool_names::MEJEPA_DAEMON_STATUS,
            "Aggregate ME-JEPA Phase F runtime status in one structured call: process mode, shift subscriber, self-healing store, hygiene quota, and optional CUDA-driver VRAM readback. Missing component SoTs are reported as unavailable instead of guessed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "inferDbPath": {
                        "type": "string",
                        "description": "Inference/hygiene RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used for subscriber and as quotaDbPath fallback."
                    },
                    "panelDbPath": {
                        "type": "string",
                        "description": "Panel RocksDB path for subscriber panel counts. If omitted, CONTEXTGRAPH_MEJEPA_PANEL_DB is used when present."
                    },
                    "healDbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is used when present."
                    },
                    "quotaDbPath": {
                        "type": "string",
                        "description": "Hygiene RocksDB path. If omitted, inferDbPath/CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "archiveRoot": {
                        "type": "string",
                        "description": "Hygiene archive root. If omitted, CONTEXTGRAPH_MEJEPA_HYGIENE_ARCHIVE_ROOT is used when present."
                    },
                    "includeVram": {
                        "type": "boolean",
                        "default": true
                    },
                    "vramBudget": {
                        "type": "string",
                        "enum": ["content_set", "full_phase1"],
                        "default": "content_set"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PAUSE_PREDICTIONS,
            "Pause ME-JEPA prediction serving until now + durationMins by writing a durable pause-state JSON file and reading it back byte-for-byte.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["durationMins"],
                "properties": {
                    "statePath": {
                        "type": "string",
                        "description": "Pause-state JSON path. If omitted, CONTEXTGRAPH_MEJEPA_PAUSE_PATH or the D-drive default is used."
                    },
                    "durationMins": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10080
                    },
                    "reason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096,
                        "default": "manual pause"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_OPERATOR_OVERRIDE_PREDICTION,
            "Record an operator override for a persisted ME-JEPA RealityPrediction. Persists CF_MEJEPA_OPERATOR_OVERRIDES, writes a human active-learning label, and returns the 6x sampling-weight flag readback for the next batch.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId", "overrideVerdict", "reason", "operatorId"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "overrideVerdict": {
                        "type": "string",
                        "enum": ["pass", "fail", "abstain", "out_of_distribution"]
                    },
                    "reason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096
                    },
                    "operatorId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256
                    },
                    "identityAttestation": identity_attestation_schema()
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
            tool_names::MEJEPA_PROMOTE_APPROVAL,
            "Approve or reject a pending catastrophic ME-JEPA self-optimization promotion. The approval record is read from and written back to CF_MEJEPA_MODEL_PROMOTIONS; non-pending or non-catastrophic promotions fail closed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["promotionId", "operatorId", "action", "operatorReason"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    },
                    "promotionId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "operatorId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "action": {
                        "type": "string",
                        "enum": ["approve", "reject"]
                    },
                    "operatorReason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096,
                        "description": "Single-line operator rationale persisted with the approval/rejection event."
                    },
                    "twoPersonRule": {
                        "type": "boolean",
                        "default": true,
                        "description": "Defaults to true; catastrophic promotions require two distinct operator approvals unless an existing queued record already requires more."
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
            "Read Phase 8 EvalReports from CF_MEJEPA_EVAL_REPORTS and return ship-gate status, including the four-consecutive-window stability streak and persisted report hash.",
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
            tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD,
            "Read the latest weekly EvalReport from CF_MEJEPA_EVAL_REPORTS, reconcile it with weekly export files, and return the Phase F operator dashboard/runbook status.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "dRoot": {
                        "type": "string",
                        "description": "D-drive ME-JEPA root containing state/gold-labels and exports. Defaults to CONTEXTGRAPH_DATA_ROOT or is inferred from exportsRoot when possible."
                    },
                    "exportsRoot": {
                        "type": "string",
                        "description": "Directory containing dated weekly eval exports. Defaults to CONTEXTGRAPH_DATA_ROOT/exports/eval."
                    },
                    "maxCells": {"type": "integer", "minimum": 1, "maximum": 500, "default": 64}
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
            tool_names::MEJEPA_BOOTSTRAP_STATUS,
            "Read ME-JEPA bootstrap stage and source-of-truth CF counts from the configured inference RocksDB. Refuses to guess a database path.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
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

fn identity_attestation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["sessionId", "nonce", "timestampUnixMs", "signatureHex"],
        "properties": {
            "sessionId": {
                "type": "string",
                "minLength": 1,
                "maxLength": 256
            },
            "nonce": {
                "type": "string",
                "minLength": 8,
                "maxLength": 256
            },
            "timestampUnixMs": {
                "type": "integer",
                "minimum": 1
            },
            "signatureHex": {
                "type": "string",
                "pattern": "^[0-9a-fA-F]{64}$"
            }
        }
    })
}

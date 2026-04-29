//! Learning-as-UTL event tool definitions.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the Learning-as-UTL tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "record_learning_event",
            "Persist one Learning-as-UTL event to CF_LEARNING_EVENTS. The event \
             captures before/after 14D topic profiles, 91D cross-correlations, \
             outcome labels, deterministic UTL features, and five baseline \
             learning signals. This does not change E1-E14 embedder slots.",
            json!({
                "type": "object",
                "properties": {
                    "eventId": {"type": "string", "format": "uuid", "description": "Optional event UUID. Generated when omitted."},
                    "memoryIds": {"type": "array", "items": {"type": "string", "format": "uuid"}, "default": []},
                    "sessionId": {"type": "string"},
                    "responseId": {"type": "string"},
                    "taskId": {"type": "string"},
                    "query": {"type": "string", "default": ""},
                    "retrievedContext": {"type": "string", "default": ""},
                    "assistantResponse": {"type": "string", "default": ""},
                    "before": {"$ref": "#/$defs/state"},
                    "after": {"$ref": "#/$defs/state"},
                    "outcome": {"$ref": "#/$defs/outcome"}
                },
                "required": ["before", "after", "outcome"],
                "additionalProperties": false,
                "$defs": {
                    "state": {
                        "type": "object",
                        "properties": {
                            "topicProfile": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "number"}},
                            "crossCorrelations": {"type": "array", "minItems": 91, "maxItems": 91, "items": {"type": "number"}},
                            "retrievalRank": {"type": "integer", "minimum": 0},
                            "embedderScores": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "number"}},
                            "contradictionPressure": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "integrationConfidence": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "recurrenceCount": {"type": "integer", "minimum": 0, "default": 0},
                            "stabilityScore": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "domain": {"type": "string"},
                            "successfulTransferCount": {"type": "integer", "minimum": 0, "default": 0}
                        },
                        "required": ["topicProfile", "crossCorrelations"],
                        "additionalProperties": false
                    },
                    "outcome": {
                        "type": "object",
                        "properties": {
                            "label": {"type": "string", "enum": ["useful", "neutral", "harmful", "no_learning"]},
                            "utilityDelta": {"type": "number", "minimum": -1.0, "maximum": 1.0, "default": 0.0},
                            "correctionRequired": {"type": "boolean", "default": false},
                            "reuseObserved": {"type": "boolean", "default": false}
                        },
                        "required": ["label"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "list_learning_events",
            "List UUIDs currently stored in CF_LEARNING_EVENTS with optional feature shape summaries.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 50},
                    "offset": {"type": "integer", "minimum": 0, "default": 0},
                    "includeFeatures": {"type": "boolean", "default": true}
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_learning_event",
            "Fetch a single Learning-as-UTL event by event UUID from CF_LEARNING_EVENTS.",
            json!({
                "type": "object",
                "properties": {
                    "eventId": {"type": "string", "format": "uuid"},
                    "includeText": {"type": "boolean", "default": true},
                    "includeSignals": {"type": "boolean", "default": true}
                },
                "required": ["eventId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "count_learning_events",
            "Return the current number of rows in CF_LEARNING_EVENTS.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_learning_signal_embedders",
            "List every deterministic Learning-as-UTL signal embedder exposed by MCP. \
             These are event-level UTL embedders, not E1-E14 content embedders: \
             delta_e, surprise, coherence, consolidation, and transfer.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "compute_learning_signals",
            "Compute deterministic Learning-as-UTL features and signal embeddings \
             from an inline before/after/outcome transition without mutating storage. \
             Use this to inspect candidate state transitions before recording them.",
            json!({
                "type": "object",
                "properties": {
                    "before": {"$ref": "#/$defs/state"},
                    "after": {"$ref": "#/$defs/state"},
                    "outcome": {"$ref": "#/$defs/outcome"},
                    "signalIds": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["delta_e", "surprise", "coherence", "consolidation", "transfer"]
                        },
                        "description": "Optional subset. When omitted, all five learning signal embedders run."
                    },
                    "includeFeatures": {"type": "boolean", "default": true}
                },
                "required": ["before", "after", "outcome"],
                "additionalProperties": false,
                "$defs": {
                    "state": {
                        "type": "object",
                        "properties": {
                            "topicProfile": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "number"}},
                            "crossCorrelations": {"type": "array", "minItems": 91, "maxItems": 91, "items": {"type": "number"}},
                            "retrievalRank": {"type": "integer", "minimum": 0},
                            "embedderScores": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "number"}},
                            "contradictionPressure": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "integrationConfidence": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "recurrenceCount": {"type": "integer", "minimum": 0, "default": 0},
                            "stabilityScore": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "domain": {"type": "string"},
                            "successfulTransferCount": {"type": "integer", "minimum": 0, "default": 0}
                        },
                        "required": ["topicProfile", "crossCorrelations"],
                        "additionalProperties": false
                    },
                    "outcome": {
                        "type": "object",
                        "properties": {
                            "label": {"type": "string", "enum": ["useful", "neutral", "harmful", "no_learning"]},
                            "utilityDelta": {"type": "number", "minimum": -1.0, "maximum": 1.0, "default": 0.0},
                            "correctionRequired": {"type": "boolean", "default": false},
                            "reuseObserved": {"type": "boolean", "default": false}
                        },
                        "required": ["label"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "embed_learning_event_signals",
            "Read one persisted LearningEvent from CF_LEARNING_EVENTS and run the \
             deterministic learning signal embedders against it. The tool performs \
             a separate RocksDB read, returns before/after CF counts, and verifies \
             whether freshly embedded signals match the persisted event signals.",
            json!({
                "type": "object",
                "properties": {
                    "eventId": {"type": "string", "format": "uuid"},
                    "signalIds": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["delta_e", "surprise", "coherence", "consolidation", "transfer"]
                        },
                        "description": "Optional subset. When omitted, all five learning signal embedders run."
                    },
                    "includePersisted": {"type": "boolean", "default": true}
                },
                "required": ["eventId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "estimate_learning_outcome",
            "Predict the likely utility delta of a candidate before/after \
             learning transition by nearest-neighbor case-based reasoning over \
             persisted CF_LEARNING_EVENTS rows. The tool scans real stored \
             events, validates every read, returns weighted neighbor evidence, \
             and does not mutate storage.",
            json!({
                "type": "object",
                "properties": {
                    "before": {"$ref": "#/$defs/state"},
                    "candidateAfter": {"$ref": "#/$defs/state"},
                    "maxNeighbors": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 5
                    },
                    "maxScan": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200000,
                        "default": 10000
                    },
                    "minSimilarity": {
                        "type": "number",
                        "minimum": -1.0,
                        "maximum": 1.0,
                        "default": 0.0
                    },
                    "taskId": {
                        "type": "string",
                        "description": "Optional exact task_id filter."
                    },
                    "domain": {
                        "type": "string",
                        "description": "Optional before/after domain filter."
                    },
                    "includeNeighbors": {
                        "type": "boolean",
                        "default": true
                    }
                },
                "required": ["before", "candidateAfter"],
                "additionalProperties": false,
                "$defs": {
                    "state": {
                        "type": "object",
                        "properties": {
                            "topicProfile": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "number"}},
                            "crossCorrelations": {"type": "array", "minItems": 91, "maxItems": 91, "items": {"type": "number"}},
                            "retrievalRank": {"type": "integer", "minimum": 0},
                            "embedderScores": {"type": "array", "minItems": 14, "maxItems": 14, "items": {"type": "number"}},
                            "contradictionPressure": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "integrationConfidence": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "recurrenceCount": {"type": "integer", "minimum": 0, "default": 0},
                            "stabilityScore": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                            "domain": {"type": "string"},
                            "successfulTransferCount": {"type": "integer", "minimum": 0, "default": 0}
                        },
                        "required": ["topicProfile", "crossCorrelations"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "record_graph_learning_event",
            "Persist a graph-navigation outcome as a LearningEvent in CF_LEARNING_EVENTS. \
             The tool reads real K-NN/typed edges from EdgeRepository, rejects missing graph \
             evidence, derives the learning state from observed edge scores, writes the event, \
             and immediately reads the same RocksDB row back for full-state verification.",
            json!({
                "type": "object",
                "properties": {
                    "eventId": {"type": "string", "format": "uuid", "description": "Optional event UUID. Generated when omitted."},
                    "policyScope": {"type": "string", "description": "Required learning policy scope. Stored as taskId=graph_learning:<policyScope>."},
                    "graphTool": {"type": "string", "enum": ["get_unified_neighbors", "get_memory_neighbors", "traverse_graph"], "default": "get_unified_neighbors"},
                    "sourceMemoryId": {"type": "string", "format": "uuid"},
                    "selectedNeighborIds": {"type": "array", "items": {"type": "string", "format": "uuid"}, "minItems": 1},
                    "rejectedNeighborIds": {"type": "array", "items": {"type": "string", "format": "uuid"}, "default": []},
                    "weightProfile": {"type": "string", "default": "semantic_search"},
                    "beforeRank": {"type": "integer", "minimum": 1},
                    "afterRank": {"type": "integer", "minimum": 1},
                    "query": {"type": "string", "default": ""},
                    "sessionId": {"type": "string"},
                    "responseId": {"type": "string"},
                    "outcome": {
                        "type": "object",
                        "properties": {
                            "label": {"type": "string", "enum": ["useful", "neutral", "harmful", "no_learning"]},
                            "utilityDelta": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "correctionRequired": {"type": "boolean", "default": false},
                            "reuseObserved": {"type": "boolean", "default": false}
                        },
                        "required": ["label", "utilityDelta"],
                        "additionalProperties": false
                    },
                    "outcomeReason": {"type": "string", "description": "Human or system-observed reason for the outcome."}
                },
                "required": ["policyScope", "sourceMemoryId", "selectedNeighborIds", "beforeRank", "afterRank", "outcome"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "resolve_graph_learning_policy",
            "Resolve a learned graph ranking policy from persisted graph LearningEvents in \
             CF_LEARNING_EVENTS. This scans real stored events, validates their embedded graph \
             evidence, and returns learned embedder weights and edge boosts. If matching evidence \
             is missing or insufficient, the tool errors instead of falling back to the base policy.",
            json!({
                "type": "object",
                "properties": {
                    "policyScope": {"type": "string"},
                    "sourceMemoryId": {"type": "string", "format": "uuid", "description": "Optional source-memory filter."},
                    "baseWeightProfile": {"type": "string", "default": "semantic_search"},
                    "minEvidence": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 1},
                    "maxScan": {"type": "integer", "minimum": 1, "maximum": 200000, "default": 10000},
                    "learningRate": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.35},
                    "includeEvidence": {"type": "boolean", "default": true}
                },
                "required": ["policyScope"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "export_learner_training_dataset",
            "Compile Learning-as-UTL and learner-state RocksDB rows into a versioned row-major f32 matrix stored in CF_LEARNER_TRAINING_DATASETS.",
            json!({
                "type": "object",
                "properties": {
                    "datasetId": {"type": "string", "format": "uuid", "description": "Optional dataset UUID. Generated when omitted."},
                    "task": {
                        "type": "string",
                        "enum": ["reward_model", "reranker", "embedder_contrastive", "diagnostic_classifier", "scheduler", "personal_physiology"],
                        "default": "reward_model"
                    },
                    "maxRows": {"type": "integer", "minimum": 1, "maximum": 1000000, "default": 10000},
                    "clearExisting": {"type": "boolean", "default": false}
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_learner_training_datasets",
            "List dataset UUIDs stored in CF_LEARNER_TRAINING_DATASETS with optional matrix shape summaries.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 50},
                    "offset": {"type": "integer", "minimum": 0, "default": 0},
                    "includeShape": {"type": "boolean", "default": true}
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_learner_training_dataset",
            "Fetch one matrix-shaped learner training dataset from CF_LEARNER_TRAINING_DATASETS.",
            json!({
                "type": "object",
                "properties": {
                    "datasetId": {"type": "string", "format": "uuid"},
                    "includeMatrix": {"type": "boolean", "default": false},
                    "previewRows": {"type": "integer", "minimum": 0, "maximum": 100, "default": 3}
                },
                "required": ["datasetId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "count_learner_training_datasets",
            "Return the current number of rows in CF_LEARNER_TRAINING_DATASETS.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
    ]
}

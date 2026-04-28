//! UTL learner-state tool definitions.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 17 UTL learner-state tool definitions from docs/07 plus the
/// operational MCP readback/control surface needed for full learner workflows.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "register_learner",
            "Register a learner profile in CF_LEARNER_PROFILE with consent state and enabled modalities.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "handle": {"type": "string"},
                    "consentState": {"type": "string", "default": "consented-local-first"},
                    "modalitiesEnabled": {"type": "array", "items": {"type": "string"}, "default": ["affect_text", "self_report"]},
                    "calibrationSessionTs": {"type": "integer", "minimum": 0}
                },
                "required": ["handle"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "record_session_observation",
            "Persist a learner-session fingerprint and state vector to CF_FINGERPRINTS_LEARNER and CF_LEARNER_STATE_HISTORY.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "modality": {"type": "string", "default": "affect_text"},
                    "rawText": {"type": "string", "default": ""},
                    "consentState": {"type": "string", "default": "consented-local-first"},
                    "preprocessingVersion": {"type": "string", "default": "phase0-preprocess-v1"},
                    "embedderVersion": {"type": "string", "default": "phase0-deterministic-v1"},
                    "thresholdVersion": {"type": "string", "default": "thresholds-default-pending-calibration-v1"},
                    "stateVector": {"type": "array", "items": {"type": "number"}},
                    "components": {"$ref": "#/$defs/components"},
                    "embeddings": {"type": "array", "items": {"$ref": "#/$defs/embedding"}, "default": []},
                    "signals": {"type": "array", "items": {"$ref": "#/$defs/signal"}, "default": []}
                },
                "required": ["learnerId", "sessionTs"],
                "additionalProperties": false,
                "$defs": {
                    "components": {
                        "type": "object",
                        "properties": {
                            "plasticityWindow": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "hrvCoherence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "valence": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "arousal": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "stressFloor": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "kSleep": {"type": "number", "minimum": 0.0, "maximum": 5.0, "default": 1.0}
                        },
                        "required": ["plasticityWindow", "hrvCoherence", "valence", "arousal", "stressFloor"],
                        "additionalProperties": false
                    },
                    "embedding": {
                        "type": "object",
                        "properties": {
                            "modality": {"type": "string"},
                            "vector": {"type": "array", "items": {"type": "number"}},
                            "scalar": {"type": "number", "minimum": 0.0, "maximum": 1.0}
                        },
                        "required": ["modality", "vector"],
                        "additionalProperties": false
                    },
                    "signal": {
                        "type": "object",
                        "properties": {
                            "modality": {"type": "string"},
                            "text": {"type": "string"},
                            "samples": {"type": "array", "items": {"type": "number"}, "default": []},
                            "features": {"type": "array", "items": {"type": "number"}, "default": []},
                            "sampleRateHz": {"type": "integer", "minimum": 1},
                            "channels": {"type": "integer", "minimum": 1, "maximum": 256}
                        },
                        "required": ["modality"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "compute_delta_s",
            "Compute Delta S from predicted, actual, optional simulated text, and exploration rate.",
            json!({
                "type": "object",
                "properties": {
                    "predictedText": {"type": "string", "default": ""},
                    "actualText": {"type": "string", "default": ""},
                    "simulatedText": {"type": "string"},
                    "explorationRate": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "gamma": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.7}
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "compute_delta_c",
            "Compute Delta C from recent scores, HRV coherence, panel agreement, and contradiction pressure.",
            json!({
                "type": "object",
                "properties": {
                    "recentScores": {"type": "array", "items": {"type": "number"}, "default": []},
                    "hrvCoherence": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "panelAgreement": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "contradiction": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "gradientScale": {"type": "number", "minimum": 0.0}
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "compute_delta_e",
            "Compute Delta E from learner-state subcomponents or from a stored learner session.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "components": {"$ref": "#/$defs/components"}
                },
                "required": [],
                "additionalProperties": false,
                "$defs": {
                    "components": {
                        "type": "object",
                        "properties": {
                            "plasticityWindow": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "hrvCoherence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "valence": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "arousal": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "stressFloor": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "kSleep": {"type": "number", "minimum": 0.0, "maximum": 5.0, "default": 1.0}
                        },
                        "required": ["plasticityWindow", "hrvCoherence", "valence", "arousal", "stressFloor"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "compute_L",
            "Compute L = DeltaS * DeltaC * DeltaE and optionally persist the session delta log and M trace.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "traceId": {"type": "string", "format": "uuid"},
                    "predictedText": {"type": "string", "default": ""},
                    "actualText": {"type": "string", "default": ""},
                    "simulatedText": {"type": "string"},
                    "explorationRate": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "recentScores": {"type": "array", "items": {"type": "number"}, "default": []},
                    "hrvCoherence": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "panelAgreement": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "contradiction": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.0},
                    "components": {"$ref": "#/$defs/components"},
                    "persist": {"type": "boolean", "default": false},
                    "retrievalCorrect": {"type": "boolean"}
                },
                "required": ["components"],
                "additionalProperties": false,
                "$defs": {
                    "components": {
                        "type": "object",
                        "properties": {
                            "plasticityWindow": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "hrvCoherence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "valence": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "arousal": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "stressFloor": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "kSleep": {"type": "number", "minimum": 0.0, "maximum": 5.0, "default": 1.0}
                        },
                        "required": ["plasticityWindow", "hrvCoherence", "valence", "arousal", "stressFloor"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "get_learner_M",
            "Read a persisted learner M(t) trace from CF_LEARNER_M_PER_TRACE.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "traceId": {"type": "string", "format": "uuid"}
                },
                "required": ["learnerId", "traceId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "next_review_for_trace",
            "Return the next-review timestamp for a persisted learner M(t) trace.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "traceId": {"type": "string", "format": "uuid"}
                },
                "required": ["learnerId", "traceId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_learner_embedders",
            "Return the canonical E1-E21 embedder matrix: E1-E14 content embedders plus E15-E21 learner-state embedders.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "preflight_learner_assets",
            "Inspect cached E15-E21 model assets and real calibration datasets. Fails closed when required files are missing unless allowMissing=true.",
            json!({
                "type": "object",
                "properties": {
                    "modelsRoot": {"type": "string", "default": "models"},
                    "calibrationRoot": {"type": "string", "default": "data/utl_calibration"},
                    "allowMissing": {"type": "boolean", "default": false}
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "count_learner_state",
            "Return row counts for all learner-state RocksDB column families.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_learner_state",
            "Read persisted learner profile, session fingerprint/state vector, delta log, k_sleep, and optional M trace from RocksDB.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "traceId": {"type": "string", "format": "uuid"},
                    "includeVectors": {"type": "boolean", "default": false}
                },
                "required": ["learnerId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "record_learner_k_sleep",
            "Persist a sleep-gated consolidation multiplier row to CF_LEARNER_K_SLEEP.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "slowWaveMinutes": {"type": "integer", "minimum": 0, "maximum": 1440},
                    "k": {"type": "number", "minimum": 0.0, "maximum": 5.0}
                },
                "required": ["learnerId", "sessionTs", "slowWaveMinutes"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "record_learner_retrieval",
            "Persist a retrieval-practice row to CF_LEARNER_RETRIEVAL_LOG using either a stored session state or an explicit state vector/components payload.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "traceId": {"type": "string", "format": "uuid"},
                    "ts": {"type": "integer", "minimum": 0},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "correct": {"type": "boolean"},
                    "score": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                    "stateVector": {"type": "array", "items": {"type": "number"}},
                    "components": {"$ref": "#/$defs/components"}
                },
                "required": ["learnerId", "traceId", "ts", "correct", "score"],
                "additionalProperties": false,
                "$defs": {
                    "components": {
                        "type": "object",
                        "properties": {
                            "plasticityWindow": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "hrvCoherence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "valence": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "arousal": {"type": "number", "minimum": -1.0, "maximum": 1.0},
                            "stressFloor": {"type": "number", "minimum": 0.0, "maximum": 1.0},
                            "kSleep": {"type": "number", "minimum": 0.0, "maximum": 5.0, "default": 1.0}
                        },
                        "required": ["plasticityWindow", "hrvCoherence", "valence", "arousal", "stressFloor"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        ToolDefinition::new(
            "upsert_goal_centroid",
            "Persist an expert goal centroid for one skill/modality in CF_GOAL_CENTROIDS.",
            json!({
                "type": "object",
                "properties": {
                    "skillId": {"type": "string", "format": "uuid"},
                    "modality": {"type": "string"},
                    "vector": {"type": "array", "items": {"type": "number"}, "minItems": 1}
                },
                "required": ["skillId", "modality", "vector"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_goal_distance",
            "Compare a learner session modality embedding to a persisted skill goal centroid, and optionally persist the learner's goal-state snapshot.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "skillId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "modality": {"type": "string"},
                    "persistGoalState": {"type": "boolean", "default": false}
                },
                "required": ["learnerId", "skillId", "sessionTs", "modality"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "compile_learner_constellation",
            "Compile and persist a learner regulated-state baseline constellation from stored learner-session fingerprints.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTsList": {"type": "array", "items": {"type": "integer", "minimum": 0}, "minItems": 1},
                    "selectorKind": {"type": "integer", "minimum": 0, "maximum": 255, "default": 1},
                    "label": {"type": "string", "default": "regulated-baseline"}
                },
                "required": ["learnerId", "sessionTsList"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "resolve_learner_retrieval_policy",
            "Read a persisted learner state vector and resolve the state-conditioned E1-E14 retrieval weight profile without falling back on missing state.",
            json!({
                "type": "object",
                "properties": {
                    "learnerId": {"type": "string", "format": "uuid"},
                    "sessionTs": {"type": "integer", "minimum": 0},
                    "baseWeightProfile": {
                        "type": "string",
                        "enum": [
                            "semantic_search", "causal_reasoning", "code_search", "fact_checking",
                            "graph_reasoning", "temporal_navigation", "sequence_navigation",
                            "conversation_history", "category_weighted", "typo_tolerant",
                            "pipeline_stage1_recall", "pipeline_stage2_scoring", "pipeline_full",
                            "balanced", "multilingual_search", "long_context", "translation_finder",
                            "affect_repair", "affect_priming", "affect_neutral"
                        ],
                        "default": "semantic_search"
                    },
                    "includeWeights": {"type": "boolean", "default": true}
                },
                "required": ["learnerId", "sessionTs"],
                "additionalProperties": false
            }),
        ),
    ]
}

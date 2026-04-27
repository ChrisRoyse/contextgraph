//! UTL learner-state tool definitions.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 8 UTL learner-state tool definitions from docs/07.
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
                    "embeddings": {"type": "array", "items": {"$ref": "#/$defs/embedding"}, "default": []}
                },
                "required": ["learnerId", "sessionTs", "components"],
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
    ]
}

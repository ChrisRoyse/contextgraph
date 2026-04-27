//! Learning-as-UTL event tool definitions.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 4 Learning-as-UTL tool definitions.
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
    ]
}

//! Typed-edge training-data tool definitions (Phase 4).
//!
//! Three MCP tool schemas backing the typed-edges training-data factory:
//! - `export_typed_edges_corpus` — emit a `TypedEdgeTrainingRecord` per typed
//!   edge into `CF_TYPED_EDGE_RECORDS` with optional content / source_metadata
//!   / mechanism_type / LLM-validation joins.
//! - `derive_anomalies_from_edges` — classify typed edges into anomaly kinds
//!   and persist matching pairs into `CF_CONTRASTIVE_PAIRS` (reuses the
//!   contrastive-pair write path).
//! - `list_typed_edge_records` — paginated read view of
//!   `CF_TYPED_EDGE_RECORDS` with optional full-record hydration.
//!
//! All schemas use `additionalProperties: false` to fail fast on unknown
//! params (per the spec's error-taxonomy rule).

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 4 typed-edge tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "export_typed_edges_corpus",
            "Export every typed edge currently stored in CF_TYPED_EDGES as a \
             TypedEdgeTrainingRecord into CF_TYPED_EDGE_RECORDS. Each record \
             carries the per-embedder similarity profile, optional source/target \
             content, optional source_metadata (session_id, source_type), \
             optional mechanism_type (joined from CF_CAUSAL_RELATIONSHIPS when \
             edge type is CausalChain), and an optional LLMValidationSummary \
             (joined from CF_TYPED_EDGE_VALIDATIONS). Idempotent — re-export \
             overwrites prior records on the composite \
             (source, target, edge_type) key. Set clearExisting=true to wipe \
             CF_TYPED_EDGE_RECORDS before writing. Output is binary (version \
             byte + bincode); no external files are written.",
            json!({
                "type": "object",
                "properties": {
                    "maxEdges": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000000,
                        "default": 10000,
                        "description": "Upper bound on typed edges to export in this call. Default 10 000."
                    },
                    "includeContent": {
                        "type": "boolean",
                        "default": true,
                        "description": "Join source/target content from the store. When false, source_content/target_content are empty strings."
                    },
                    "includeSourceMetadata": {
                        "type": "boolean",
                        "default": true,
                        "description": "Join source/target SourceMetadata for session_id and source_type fields."
                    },
                    "includeMechanismType": {
                        "type": "boolean",
                        "default": true,
                        "description": "For CausalChain edges, join mechanism_type from CF_CAUSAL_RELATIONSHIPS. No-op for other edge types."
                    },
                    "joinLLMValidation": {
                        "type": "boolean",
                        "default": true,
                        "description": "Attach LLMValidationSummary when a row exists in CF_TYPED_EDGE_VALIDATIONS for this (source, target, edge_type) key."
                    },
                    "clearExisting": {
                        "type": "boolean",
                        "default": false,
                        "description": "Delete every row in CF_TYPED_EDGE_RECORDS before exporting. Use for a clean re-export."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "derive_anomalies_from_edges",
            "Classify typed edges against the five expressible AnomalyKind \
             patterns (SemanticButNotCausal, CodeShapeButDifferentIntent, \
             EntitySharedButDifferentStructure, KeywordButNotParaphrase, \
             HdcRobustButSemanticDifferent) and persist matching pairs to \
             CF_CONTRASTIVE_PAIRS via the standard contrastive-pair write path. \
             Each written pair carries generator=\"typed_edge_anomaly_derivation_v1\" \
             so origin is traceable. The kinds filter is not yet supported at \
             the storage layer — supplying it returns InvalidParams. Runs are \
             idempotent on the (anchor, negative) primary key.",
            json!({
                "type": "object",
                "properties": {
                    "highThreshold": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.60,
                        "description": "Embedder score at or above which an embedder counts as 'high' in the anomaly classifier."
                    },
                    "lowThreshold": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.30,
                        "description": "Embedder score at or below which the opposing-embedder slot counts as 'low' in the anomaly classifier."
                    },
                    "minDisagreement": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.30,
                        "description": "Minimum high-low gap required to keep a pair (mirrors the offline miner)."
                    },
                    "maxPairs": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000000,
                        "default": 10000,
                        "description": "Hard cap on total pairs written in this derivation run."
                    },
                    "kinds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional filter on AnomalyKind (snake_case names). NOT YET SUPPORTED in the storage layer — passing this returns InvalidParams. Omit for all kinds."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_typed_edge_records",
            "Paginated read view of CF_TYPED_EDGE_RECORDS. Returns composite \
             keys (source_memory_id, target_memory_id, edge_type, edge_type_name) \
             in RocksDB iteration order. Set includeFull=true to hydrate each \
             row with the full TypedEdgeTrainingRecord payload.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 100,
                        "description": "Max rows to return."
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Skip this many rows before returning."
                    },
                    "includeFull": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, fetch each row's full TypedEdgeTrainingRecord payload (heavier response)."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_three_tool_definitions() {
        let tools = definitions();
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"export_typed_edges_corpus"));
        assert!(names.contains(&"derive_anomalies_from_edges"));
        assert!(names.contains(&"list_typed_edge_records"));
    }

    #[test]
    fn all_tools_use_strict_additional_properties() {
        for t in definitions() {
            let additional = t
                .input_schema
                .get("additionalProperties")
                .and_then(|v| v.as_bool());
            assert_eq!(
                additional,
                Some(false),
                "Tool {} must set additionalProperties=false",
                t.name
            );
        }
    }
}

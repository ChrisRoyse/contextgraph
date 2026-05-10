//! Training data export tool definitions.
//!
//! Exposes `export_training_corpus`, which iterates stored memories, assembles
//! a full `TrainingRecord` per memory (active embeddings, cross-correlations,
//! 6D group alignments, edges, causal labels, etc.), and persists the result
//! into `CF_TRAINING_RECORDS` in the existing RocksDB store.
//!
//! Output format is binary (bincode with a version byte). No external formats.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 4 training tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "export_training_corpus",
            "Export memories as fully-labeled training records persisted to the \
         CF_TRAINING_RECORDS column family. Each record bundles the memory's \
         content, active embedding vectors (dense + sparse + token-level; E5 retired), a \
         14-slot topic profile with E5 zeroed, synergy-weighted cross-correlations, 6D group \
         alignments, outgoing typed edges, K-NN neighbors per embedder, and \
         LLM-discovered causal relationships. Optionally embeds a Tucker-1 \
         decomposition of the active interaction tensor when \
         includeTuckerCore=true (Phase 4, CPU). Records can be read back via \
         `list_training_record_ids` / `get_training_record` at the storage \
         layer. Output is binary (bincode + version byte); no external files.",
            json!({
                "type": "object",
                "properties": {
                    "filter": {
                        "type": "string",
                        "enum": ["all", "session"],
                        "default": "all",
                        "description": "Which memories to export. 'all' scans every non-soft-deleted fingerprint; 'session' requires filterId and includes only memories with matching source_metadata.session_id."
                    },
                    "filterId": {
                        "type": "string",
                        "description": "Session ID when filter='session'. Ignored otherwise."
                    },
                    "maxMemories": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000000,
                        "default": 10000,
                        "description": "Upper bound on memories to process in this call. Default 10 000."
                    },
                    "includeEmbeddings": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include active dense embedding vectors (E1/E2/E3/E4/E7/E8/E9/E10/E11/E14) in each record. E5 is retired and omitted."
                    },
                    "includeSparseVectors": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include E6 and E13 sparse embedding indices and values."
                    },
                    "includeTokenEmbeddings": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include E12 ColBERT token-level embeddings. Adds ~15KB/record; off by default."
                    },
                    "includeEdges": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include outgoing typed edges and per-embedder K-NN neighbors. Requires the graph-linking pipeline."
                    },
                    "includeCausalLabels": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include LLM-discovered causal relationships where this memory is the cause (causal_effects). causal_causes is left empty in v1."
                    },
                    "includeIncomingEdges": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include typed edges where this memory is the target. Requires a one-time O(N) reverse-index scan over CF_TYPED_EDGES at export start; off by default."
                    },
                    "includeTemporalLabels": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include Phase-5 temporal labels (hour-of-day, day-of-week, periodic bucket, age, session position, E2/E3/E4 norms). Cheap to compute."
                    },
                    "includeTuckerCore": {
                        "type": "boolean",
                        "default": false,
                        "description": "Phase-4 Tucker-1 decomposition of the active embedding interaction tensor at ranks (4, 4, 128). CPU-only streaming HOSVD; adds roughly 18KB per record and a few ms of compute. Off by default because of the cost."
                    },
                    "clearExisting": {
                        "type": "boolean",
                        "default": false,
                        "description": "Delete all records in CF_TRAINING_RECORDS before exporting. Use to start a clean corpus."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_training_records",
            "List UUIDs currently stored in CF_TRAINING_RECORDS, optionally with \
         per-record shape statistics (content length, number of outgoing/incoming \
         edges, number of causal labels, E1/E7/E11 vector presence flags). Used \
         for post-export verification and observability.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 50,
                        "description": "Max UUIDs to return."
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Skip this many UUIDs (pagination)."
                    },
                    "includeShape": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include per-record shape summary (sizes, flags). Adds one read per UUID."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_training_record",
            "Fetch a single training record by memory UUID. Use includeVectors=true \
         to include the raw dense/sparse/token embedding arrays in the response \
         (may be large; default off). Always returns topic_profile, \
         cross_correlations, group_alignments, edge/knn summaries, and causal labels.",
            json!({
                "type": "object",
                "properties": {
                    "memoryId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the memory whose training record to fetch."
                    },
                    "includeVectors": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, include active dense arrays (e1/e2/e3/e4/e7/e8/e9/e10/e11) and e6/e13 sparse arrays in response. E5 is retired and omitted."
                    },
                    "includeTokenEmbeddings": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, include E12 token embeddings in response."
                    },
                    "includeEdges": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true, include outgoing/incoming typed edges and K-NN neighbors in response."
                    }
                },
                "required": ["memoryId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "count_training_records",
            "Return the current number of rows in CF_TRAINING_RECORDS.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
    ]
}

//! Contrastive pair tool definitions (Phase 3).
//!
//! Four tools exposing the contrastive pair miner + read surface:
//! - `mine_contrastive_pairs` — scan anchors, build pairs, persist.
//! - `list_contrastive_pairs` — paginated `(anchor, negative)` listing with
//!   optional kind / anchor filters.
//! - `get_contrastive_pair` — fetch one pair by composite key.
//! - `count_contrastive_pairs` — overall or per-kind count.
//!
//! Every schema carries `additionalProperties: false` to reject unknown
//! parameters (silent schema drift has bitten this codebase before; see
//! `tasks/lessons.md`).

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 4 contrastive pair tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "mine_contrastive_pairs",
            "Scan the memory corpus for cross-embedder anomaly pairs and \
             persist each match to CF_CONTRASTIVE_PAIRS. For every anchor \
             we score a bounded candidate pool (`topKCandidatesPerAnchor` \
             peers), compute the full 14-embedder similarity profile, filter \
             by `minDisagreement` + thresholds, classify into one of six \
             AnomalyKinds, and store the resulting (anchor, negative, \
             similarity_profile) triple. Idempotent on the composite key.",
            json!({
                "type": "object",
                "properties": {
                    "maxPairs": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100000,
                        "default": 1000,
                        "description": "Hard cap on pairs persisted this run."
                    },
                    "minDisagreement": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.3,
                        "description": "Minimum (max(high_sim) - min(low_sim)) required to keep a pair."
                    },
                    "highThreshold": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.6,
                        "description": "Similarity >= this classifies an embedder as 'high' for this pair."
                    },
                    "lowThreshold": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.3,
                        "description": "Similarity <= this classifies an embedder as 'low' for this pair. Must be < highThreshold."
                    },
                    "kinds": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": [
                                "semantic_but_not_causal",
                                "keyword_but_not_paraphrase",
                                "code_shape_but_different_intent",
                                "entity_shared_but_different_structure",
                                "hdc_robust_but_semantic_different",
                                "other"
                            ]
                        },
                        "description": "Optional filter: only persist pairs whose classified AnomalyKind is in this list. Omit to accept all."
                    },
                    "sessionFilter": {
                        "type": "string",
                        "description": "Optional: only scan anchors whose source_metadata.session_id matches this value."
                    },
                    "topKCandidatesPerAnchor": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 500,
                        "default": 10,
                        "description": "Size of the candidate pool scored per anchor. Larger = more anomaly hits, smaller = faster."
                    },
                    "candidatePoolSize": {
                        "type": "integer",
                        "minimum": 10,
                        "maximum": 50000,
                        "default": 500,
                        "description": "Max peers sampled from the store per anchor before top-K filtering. Caps scan cost."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_contrastive_pairs",
            "List stored contrastive pair keys from CF_CONTRASTIVE_PAIRS with \
             optional filters on AnomalyKind and anchor UUID. includeFull=true \
             fetches full records (warning: includes 13-slot similarity \
             profile per row).",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10000,
                        "default": 100,
                        "description": "Max pairs to return."
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0,
                        "description": "Skip this many pairs (pagination)."
                    },
                    "kind": {
                        "type": "string",
                        "enum": [
                            "semantic_but_not_causal",
                            "keyword_but_not_paraphrase",
                            "code_shape_but_different_intent",
                            "entity_shared_but_different_structure",
                            "hdc_robust_but_semantic_different",
                            "other"
                        ],
                        "description": "Optional AnomalyKind filter. Uses CF_CONTRASTIVE_BY_KIND prefix scan."
                    },
                    "anchorId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Optional anchor filter. Uses CF_CONTRASTIVE_BY_ANCHOR lookup."
                    },
                    "includeFull": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, fetch the full ContrastivePair for each key. Otherwise return (anchorId, negativeId) pairs only."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_contrastive_pair",
            "Fetch a single contrastive pair by composite (anchorId, \
             negativeId) key. Returns the full ContrastivePair record \
             (similarity profile, high/low embedders, disagreement \
             magnitude, anomaly kind, texts, timestamps).",
            json!({
                "type": "object",
                "properties": {
                    "anchorId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Anchor memory UUID."
                    },
                    "negativeId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Negative memory UUID."
                    }
                },
                "required": ["anchorId", "negativeId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "count_contrastive_pairs",
            "Count rows in CF_CONTRASTIVE_PAIRS. Pass `kind` to restrict the \
             count to one AnomalyKind via the CF_CONTRASTIVE_BY_KIND prefix \
             scan.",
            json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": [
                            "semantic_but_not_causal",
                            "keyword_but_not_paraphrase",
                            "code_shape_but_different_intent",
                            "entity_shared_but_different_structure",
                            "hdc_robust_but_semantic_different",
                            "other"
                        ],
                        "description": "Optional AnomalyKind filter. When omitted, returns the total row count."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
    ]
}

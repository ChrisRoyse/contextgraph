//! Core tool definitions per PRD v6 Section 10.
//!
//! Tools: store_memory, get_memetic_status, search_graph, trigger_consolidation
//!
//! Note: inject_context was merged into store_memory. When `rationale` is provided,
//! the same validation (1-1024 chars) and response format is used.

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns core tool definitions (4 tools - inject_context merged into store_memory).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // store_memory - store a memory node directly
        // Note: inject_context merged into this tool. When rationale is provided,
        // the same validation and response format is used.
        ToolDefinition::new(
            "store_memory",
            "Store a memory node directly in the knowledge graph without UTL processing.",
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 10000,
                        "description": "The content to store"
                    },
                    "rationale": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 1024,
                        "description": "Why this context is relevant and should be stored (OPTIONAL, 1-1024 chars). When provided, included in response."
                    },
                    "importance": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.5,
                        "description": "Importance score for the memory [0.0, 1.0]"
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Session ID for session-scoped storage. If omitted, uses CLAUDE_SESSION_ID env var."
                    },
                    "operatorId": {
                        "type": "string",
                        "description": "Operator/user ID for audit provenance tracking"
                    }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "store_memories",
            "Store a validated batch of memory nodes through the active embedder pipeline while preserving the 14-slot fingerprint storage contract. The batch is all-or-error for fingerprint/content/source-metadata/embedding-version/audit readback: if any required write or verification fails, stored fingerprints are hard-deleted and the tool returns an error.",
            json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 64,
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": 10000,
                                    "description": "The content to store"
                                },
                                "rationale": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": 1024,
                                    "description": "Why this context is relevant and should be stored"
                                },
                                "importance": {
                                    "type": "number",
                                    "minimum": 0,
                                    "maximum": 1,
                                    "default": 0.5
                                },
                                "sessionId": {
                                    "type": "string",
                                    "description": "Per-item session ID. Defaults to top-level sessionId, then CLAUDE_SESSION_ID/server session."
                                },
                                "operatorId": {
                                    "type": "string",
                                    "description": "Per-item operator/user ID. Defaults to top-level operatorId."
                                }
                            },
                            "required": ["content"],
                            "additionalProperties": false
                        }
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Default session ID for all items that omit sessionId"
                    },
                    "operatorId": {
                        "type": "string",
                        "description": "Default operator/user ID for all items that omit operatorId"
                    },
                    "causalHints": {
                        "type": "boolean",
                        "default": false,
                        "description": "Retired. E5 causal hints are disabled; passing true is rejected."
                    }
                },
                "required": ["items"],
                "additionalProperties": false
            }),
        )
        .with_output_schema(json!({
            "type": "object",
            "required": ["requestedCount", "storedCount", "embedderCount", "items"],
            "properties": {
                "requestedCount": {"type": "integer"},
                "storedCount": {"type": "integer"},
                "embedderCount": {"type": "integer", "description": "Alias for activeEmbedderCount; excludes retired/disabled slots."},
                "activeEmbedderCount": {"type": "integer", "description": "Number of active embedders that produced usable vectors."},
                "storageSlotCount": {"type": "integer", "description": "Persisted TeleologicalFingerprint slot count, including disabled compatibility slots."},
                "disabledEmbedders": {"type": "array", "items": {"type": "string"}, "description": "Storage slots intentionally disabled for active search/training."},
                "items": {"type": "array"}
            }
        })),
        // get_memetic_status - get system state and metrics
        ToolDefinition::new(
            "get_memetic_status",
            "Get current system status including fingerprint count, active embedder count, \
             14-slot storage count, disabled embedder slots, storage backend and size, and \
             layer status from LayerStatusProvider.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
        // search_graph - semantic search across active embedders. E5 causal is retired.
        ToolDefinition::new(
            "search_graph",
            "Search the knowledge graph using multi-space semantic similarity across active embedders. \
             Supports temporal search via E2 (recency), E3 (periodicity), E4 (sequence) — \
             use temporal_navigation profile or set E2/E3/E4 independently via customWeights. \
             E5 causal is retired and disabled; causal E5 reranking requests are rejected. \
             Returns nodes matching the query with relevance scores.",
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query text"
                    },
                    "topK": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 10,
                        "description": "Maximum number of results to return"
                    },
                    "minSimilarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.0,
                        "description": "Minimum similarity threshold [0.0, 1.0]"
                    },
                    "includeContent": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include content text in results"
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["e1_only", "multi_space", "pipeline"],
                        "default": "multi_space",
                        "description": "Search strategy: e1_only (fast, E1 only), multi_space (default - E1 + enhancers via Weighted RRF, uses weightProfile), pipeline (E13 recall → E1 dense → E12 rerank)"
                    },
                    "weightProfile": {
                        "type": "string",
                        "enum": [
                            "semantic_search", "causal_reasoning", "code_search", "fact_checking",
                            "graph_reasoning", "temporal_navigation", "sequence_navigation",
                            "conversation_history", "category_weighted", "typo_tolerant",
                            "pipeline_stage1_recall", "pipeline_stage2_scoring", "pipeline_full",
                            "balanced",
                            "multilingual_search", "long_context", "translation_finder",
                            "affect_repair", "affect_priming", "affect_neutral"
                        ],
                        "description": "Weight profile for multi-space search. Temporal profiles: temporal_navigation (E2+E3+E4 balanced — time-based retrieval), sequence_navigation (E4-heavy — find nearby conversation items), conversation_history (E4+E1 — contextual recall within sessions). E14-aware profiles: multilingual_search (E14=0.40 — cross-lingual retrieval), long_context (E14=0.45 — docs > 512 tokens), translation_finder (E14=0.70 — translation pairs / cross-lingual near-duplicates). Learner-state profiles: affect_repair, affect_priming, affect_neutral. For fine-grained temporal control, use customWeights to set E2/E3/E4 independently."
                    },
                    "stateConditionedProfile": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, require learnerId+learnerSessionTs, read CF_LEARNER_STATE_HISTORY, and route weightProfile through the state-conditioned retrieval policy."
                    },
                    "learnerId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Learner UUID used with stateConditionedProfile."
                    },
                    "learnerSessionTs": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Persisted learner session timestamp used with stateConditionedProfile."
                    },
                    "customWeights": {
                        "type": "object",
                        "description": "Custom per-embedder weights (overrides weightProfile). Each active value is 0-1, retired E5 and disabled E11 must be 0, and weights must sum to ~1.0. Omitted embedders default to 0. Temporal embedders E2/E3/E4 encode INDEPENDENT time dimensions and should be tuned separately: E2 (recency — how recently something was stored), E3 (periodicity — recurring time-of-day/day-of-week patterns), E4 (sequence — ordering within a conversation session). Set any combination to emphasize different temporal aspects.",
                        "properties": {
                            "E1":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Semantic similarity (foundation)" },
                            "E2":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Temporal recency — find memories stored near a given time" },
                            "E3":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Temporal periodicity — find memories matching time-of-day or day-of-week patterns" },
                            "E4":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Temporal sequence — find memories near the same position in a conversation" },
                            "E5":  { "type": "number", "minimum": 0, "maximum": 0, "description": "Retired and disabled; must be 0" },
                            "E6":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Sparse keyword matching" },
                            "E7":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Code pattern similarity" },
                            "E8":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Graph structure (imports, dependencies)" },
                            "E9":  { "type": "number", "minimum": 0, "maximum": 1, "description": "Noise-robust matching (typo tolerant)" },
                            "E10": { "type": "number", "minimum": 0, "maximum": 1, "description": "Paraphrase/multimodal similarity" },
                            "E11": { "type": "number", "minimum": 0, "maximum": 0, "description": "Disabled; must be 0 until a verified code-symbol entity embedder is installed" },
                            "E12": { "type": "number", "minimum": 0, "maximum": 1, "description": "ColBERT reranking (stage only, weight 0)" },
                            "E13": { "type": "number", "minimum": 0, "maximum": 1, "description": "SPLADE recall (stage only, weight 0)" },
                            "E14": { "type": "number", "minimum": 0, "maximum": 1, "description": "BGE-M3 Dense — multilingual 1024-D CLS-pooled (XLM-RoBERTa-Large, 8192-token context)" }
                        },
                        "additionalProperties": false
                    },
                    "excludeEmbedders": {
                        "type": "array",
                        "description": "Embedders to exclude from fusion (their weight becomes 0, remaining renormalized).",
                        "items": {
                            "type": "string",
                            "enum": ["E1", "E2", "E3", "E4", "E6", "E7", "E8", "E9", "E10", "E12", "E13", "E14"]
                        }
                    },
                    "useQuantizedPrefilter": {
                        "type": "boolean",
                        "description": "Use PQ-8 quantized vectors for fast approximate pre-filtering",
                        "default": false
                    },
                    "includeEmbedderBreakdown": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include per-embedder scores and contribution breakdown in results. Shows which embedders contributed most to each result's ranking."
                    },
                    "enableRerank": {
                        "type": "boolean",
                        "default": false,
                        "description": "Enable ColBERT E12 re-ranking (Stage 3)"
                    },
                    "enableAsymmetricE5": {
                        "type": "boolean",
                        "default": false,
                        "description": "Retired. E5 causal reranking is disabled; passing true is rejected."
                    },
                    "causalDirection": {
                        "type": "string",
                        "enum": ["auto", "cause", "effect", "none"],
                        "default": "auto",
                        "description": "Causal direction: auto (detect from query), cause (seeking causes), effect (seeking effects), none (disable)"
                    },
                    "enableQueryExpansion": {
                        "type": "boolean",
                        "default": false,
                        "description": "Expand causal queries with related terms for better recall"
                    },
                    "temporalWeight": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.0,
                        "description": "Weight for temporal post-retrieval boost [0.0, 1.0]"
                    },
                    "conversationContext": {
                        "type": "object",
                        "description": "Convenience wrapper for sequence-based retrieval. Auto-anchors to current conversation turn.",
                        "properties": {
                            "anchorToCurrentTurn": {
                                "type": "boolean",
                                "default": true,
                                "description": "Auto-anchor to current session sequence (overrides sequenceAnchor)"
                            },
                            "turnsBack": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 100,
                                "default": 10,
                                "description": "Number of turns to look back from anchor"
                            },
                            "turnsForward": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 100,
                                "default": 0,
                                "description": "Number of turns to look forward from anchor"
                            }
                        },
                        "additionalProperties": false
                    },
                    "sessionScope": {
                        "type": "string",
                        "enum": ["current", "all", "recent"],
                        "default": "all",
                        "description": "Session scope: current (this session only), all (any session), recent (last 24h across sessions)"
                    },
                    "decayHalfLifeSecs": {
                        "type": "integer",
                        "description": "Half-life in seconds for exponential temporal decay (default: 86400 = 1 day). Only used with decayFunction='exponential'."
                    },
                    "lastHours": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Filter results to the last N hours (integer). Shortcut for temporal window filtering."
                    },
                    "lastDays": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Filter results to the last N days (integer). Shortcut for temporal window filtering."
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Filter results to a specific session ID."
                    },
                    "periodicBoost": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "description": "Weight for E3 periodic matching boost (0-1). Boosts results matching the target time pattern."
                    },
                    "targetHour": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 23,
                        "description": "Target hour of day (0-23) for periodic matching. Used with periodicBoost."
                    },
                    "targetDayOfWeek": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 6,
                        "description": "Target day of week (0=Sun, 6=Sat) for periodic matching. Used with periodicBoost."
                    },
                    "sequenceAnchor": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of anchor memory for E4 sequence-based retrieval. Finds memories near this point in the conversation."
                    },
                    "sequenceDirection": {
                        "type": "string",
                        "enum": ["before", "after", "around", "both"],
                        "description": "Direction for sequence-based retrieval relative to sequenceAnchor: 'before', 'after', 'around'/'both' (both directions)."
                    },
                    "includeProvenance": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include retrieval provenance metadata in results (default: false). Shows strategy, weight profile, query classification, and per-embedder contributions."
                    },
                    "decayFunction": {
                        "type": "string",
                        "enum": ["linear", "exponential", "step", "none", "no_decay"],
                        "default": "exponential",
                        "description": "Temporal decay function: linear, exponential, step, none, no_decay (default: exponential)"
                    },
                    "temporalScale": {
                        "type": "string",
                        "enum": ["micro", "meso", "macro", "long", "archival"],
                        "default": "meso",
                        "description": "Temporal scale for decay: micro, meso, macro, long, archival"
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        // trigger_consolidation - trigger memory consolidation (PRD Section 10.1)
        ToolDefinition::new(
            "trigger_consolidation",
            "Analyze memories for consolidation candidates. Returns pairs that could be merged \
             but does NOT automatically merge them. Use merge_concepts to execute merges. \
             Uses similarity-based, temporal, or semantic strategies to identify candidates.",
            json!({
                "type": "object",
                "properties": {
                    "strategy": {
                        "type": "string",
                        "enum": ["similarity", "temporal", "semantic"],
                        "default": "similarity",
                        "description": "Consolidation strategy to use"
                    },
                    "min_similarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.925,
                        "description": "Minimum similarity threshold for consolidation candidates (SRC-3 normalized [0,1] scale)"
                    },
                    "max_memories": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10000,
                        "default": 100,
                        "description": "Maximum memories to process in one batch"
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
    ]
}

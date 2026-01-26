//! Core tool definitions per PRD v6 Section 10.
//!
//! Tools: inject_context, store_memory, get_memetic_status, search_graph, trigger_consolidation

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns core tool definitions (5 tools per PRD).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // inject_context - primary context injection tool
        ToolDefinition::new(
            "inject_context",
            "Inject context into the knowledge graph with UTL processing. \
             Analyzes content for learning potential and stores with computed metrics.",
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to inject into the knowledge graph"
                    },
                    "rationale": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 1024,
                        "description": "Why this context is relevant and should be stored (REQUIRED, 1-1024 chars)"
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "code", "image", "audio", "structured", "mixed"],
                        "default": "text",
                        "description": "The type/modality of the content"
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
                    }
                },
                "required": ["content", "rationale"]
            }),
        ),
        // store_memory - store a memory node directly
        ToolDefinition::new(
            "store_memory",
            "Store a memory node directly in the knowledge graph without UTL processing.",
            json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    },
                    "importance": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.5,
                        "description": "Importance score for the memory [0.0, 1.0]"
                    },
                    "modality": {
                        "type": "string",
                        "enum": ["text", "code", "image", "audio", "structured", "mixed"],
                        "default": "text",
                        "description": "The type/modality of the content"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional tags for categorization"
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Session ID for session-scoped storage. If omitted, uses CLAUDE_SESSION_ID env var."
                    }
                },
                "required": ["content"]
            }),
        ),
        // get_memetic_status - get system state and metrics
        ToolDefinition::new(
            "get_memetic_status",
            "Get current system status including fingerprint count, number of embedders (13), \
             storage backend and size, and layer status from LayerStatusProvider.",
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        // search_graph - semantic search with E5 causal and E10 intent asymmetric similarity (ARCH-15, AP-77)
        ToolDefinition::new(
            "search_graph",
            "Search the knowledge graph using multi-space semantic similarity. \
             For causal queries ('why', 'what happens'), automatically applies \
             asymmetric E5 similarity with direction modifiers (cause→effect 1.2x, \
             effect→cause 0.8x). For intent queries, applies E10 asymmetric similarity \
             (intent→context 1.2x, context→intent 0.8x). Returns nodes matching the query with relevance scores.",
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
                    "modality": {
                        "type": "string",
                        "enum": ["text", "code", "image", "audio", "structured", "mixed"],
                        "description": "Filter results by modality"
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
                        "enum": ["semantic_search", "causal_reasoning", "code_search", "fact_checking", "temporal_navigation", "category_weighted", "sequence_navigation", "conversation_history", "intent_search", "intent_enhanced"],
                        "description": "Weight profile for multi-space search. intent_search (E10=0.25) for intent-aware queries. intent_enhanced (E10=0.30) for stronger E10 weighting."
                    },
                    "enableRerank": {
                        "type": "boolean",
                        "default": false,
                        "description": "Enable ColBERT E12 re-ranking (Stage 3)"
                    },
                    "enableAsymmetricE5": {
                        "type": "boolean",
                        "default": true,
                        "description": "Enable asymmetric E5 causal reranking for detected causal queries"
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
                    "intentMode": {
                        "type": "string",
                        "enum": ["none", "seeking_intent", "seeking_context", "auto"],
                        "default": "none",
                        "description": "E10 intent mode: none (disabled), seeking_intent (query is a goal/purpose, apply 1.2x boost), seeking_context (query is a situation, apply 0.8x), auto (detect from query)"
                    },
                    "intentBlend": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.3,
                        "description": "E10 intent blend weight when intentMode is active [0.0=pure E1, 1.0=pure E10]. Only used when intentMode != none."
                    },
                    "enableIntentGate": {
                        "type": "boolean",
                        "default": false,
                        "description": "Enable E10 intent gate in pipeline strategy. Adds intent filtering between E1 scoring and E12 reranking. Only effective with strategy='pipeline'."
                    },
                    "intentGateThreshold": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.3,
                        "description": "Minimum E10 intent similarity for pipeline gate. Candidates below threshold are filtered out. Only used when enableIntentGate=true."
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
                        }
                    },
                    "sessionScope": {
                        "type": "string",
                        "enum": ["current", "all", "recent"],
                        "default": "all",
                        "description": "Session scope: current (this session only), all (any session), recent (last 24h across sessions)"
                    }
                },
                "required": ["query"]
            }),
        ),
        // trigger_consolidation - trigger memory consolidation (PRD Section 10.1)
        ToolDefinition::new(
            "trigger_consolidation",
            "Trigger memory consolidation to merge similar memories and reduce redundancy. \
             Uses similarity-based, temporal, or semantic strategies to identify merge candidates. \
             Helps optimize memory storage and improve retrieval efficiency.",
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
                        "default": 0.85,
                        "description": "Minimum similarity threshold for consolidation candidates"
                    },
                    "max_memories": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10000,
                        "default": 100,
                        "description": "Maximum memories to process in one batch"
                    }
                },
                "required": []
            }),
        ),
    ]
}

//! Constellation tool definitions (Phase 2).
//!
//! Six tools exposing the constellation compiler, derived anchors, and
//! read/delete paths:
//! - `compile_constellation` — build and persist a constellation.
//! - `list_constellations` — paginated UUID listing.
//! - `get_constellation` — full record by id with optional heavy arrays.
//! - `score_against_constellation` — classify a memory vs a constellation.
//! - `derive_constellation` — persist interpolation/add/difference or an
//!   anti-pole compiled from low-scoring real memories.
//! - `delete_constellation` — remove a constellation + secondary index.
//!
//! Every schema carries `additionalProperties: false` to reject unknown
//! parameters (silent schema drift has bitten this codebase before; see
//! Audit-9 L5 in `memory/MEMORY.md`).

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns the 6 constellation tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "compile_constellation",
            "Compile a constellation (per-embedder centroids + spread statistics) \
             over a caller-selected set of memories and persist the result to \
             CF_CONSTELLATIONS. Selectors: topic / session / tag / time_range / \
             explicit_ids. Returns the fresh constellation UUID plus summary \
             shape. Fails with TooFewMembers when fewer than 3 members match.",
            json!({
                "type": "object",
                "properties": {
                    "selector": {
                        "type": "string",
                        "enum": ["topic", "session", "tag", "time_range", "explicit_ids"],
                        "description": "Which resolution strategy to apply when gathering members."
                    },
                    "topicId": {
                        "type": "string",
                        "description": "Required when selector='topic'. Matches against loaded topic portfolio topic IDs."
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Required when selector='session'. Matches source_metadata.session_id."
                    },
                    "tag": {
                        "type": "string",
                        "description": "Required when selector='tag'. Matches source_metadata.tags exact string."
                    },
                    "startIso": {
                        "type": "string",
                        "format": "date-time",
                        "description": "Required when selector='time_range'. RFC-3339 inclusive lower bound on fingerprint.created_at."
                    },
                    "endIso": {
                        "type": "string",
                        "format": "date-time",
                        "description": "Required when selector='time_range'. RFC-3339 inclusive upper bound."
                    },
                    "memoryIds": {
                        "type": "array",
                        "items": { "type": "string", "format": "uuid" },
                        "description": "Required when selector='explicit_ids'. UUIDs of the memories to include."
                    },
                    "rationale": {
                        "type": "string",
                        "description": "Optional free-form annotation for selector='explicit_ids'. NOT part of the selector hash."
                    },
                    "label": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256,
                        "description": "Human-readable tag for the constellation (e.g. 'PRD §7 — Case management')."
                    },
                    "maxMembers": {
                        "type": "integer",
                        "minimum": 3,
                        "maximum": 100000,
                        "default": 50000,
                        "description": "Cap on in-memory members. Exceeding this fails with TooManyMembers."
                    },
                    "rebuildIfExists": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, recompile even if a constellation already exists for this selector. When false (default), return the existing constellation id without re-running the compiler."
                    }
                },
                "required": ["selector", "label"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "list_constellations",
            "List UUIDs currently stored in CF_CONSTELLATIONS with paging and \
             optional per-record shape summary (member_count, coherence, \
             selector kind, purity).",
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
                    "includeCentroids": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, include per-embedder centroid arrays in the response. Can be large; default off."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "get_constellation",
            "Fetch a single constellation by UUID. includeCentroids defaults to \
             true for this endpoint (the typical caller wants the full record). \
             Returns member_ids, per-embedder stats, topic/group/cross-\
             correlation centroids, coherence, and purity.",
            json!({
                "type": "object",
                "properties": {
                    "constellationId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID returned by compile_constellation."
                    },
                    "includeCentroids": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true, include per-embedder centroid, sparse_top_terms, and pooled_token_centroid arrays. When false, only shape/summary is returned."
                    }
                },
                "required": ["constellationId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "score_against_constellation",
            "Score a candidate memory against a stored constellation. Returns \
             per-embedder cosine-to-centroid similarities, an unweighted \
             combined_score mean (over embedders with coverage>0), and an \
             in_spread_p95 flag indicating whether the candidate is at least as \
             central as the 95th-percentile E1 member.",
            json!({
                "type": "object",
                "properties": {
                    "constellationId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the constellation to score against."
                    },
                    "memoryId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the candidate memory. Must already be stored in CF_FINGERPRINTS."
                    }
                },
                "required": ["constellationId", "memoryId"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "derive_constellation",
            "Persist a derived constellation in CF_CONSTELLATIONS. \
             Operations interpolate/add/difference combine two stored \
             constellation centroids. Operation anti_pole scans a real \
             candidate memory selector, scores candidates against the source \
             constellation, selects the lowest-scoring non-source memories, \
             recompiles them, and persists the opposite anchor. The tool \
             performs readback verification from RocksDB before returning.",
            json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["interpolate", "add", "difference", "anti_pole"],
                        "description": "Derivation operation to perform."
                    },
                    "sourceConstellationId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Primary source constellation UUID."
                    },
                    "otherConstellationId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Second constellation UUID for interpolate/add/difference."
                    },
                    "alpha": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.5,
                        "description": "Interpolation weight for otherConstellationId when operation='interpolate'."
                    },
                    "label": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256,
                        "description": "Human-readable label for the persisted derived constellation."
                    },
                    "selector": {
                        "type": "string",
                        "enum": ["topic", "session", "tag", "time_range", "explicit_ids"],
                        "description": "Candidate pool selector for operation='anti_pole'."
                    },
                    "topicId": {
                        "type": "string",
                        "description": "Required for anti_pole when selector='topic'."
                    },
                    "sessionId": {
                        "type": "string",
                        "description": "Required for anti_pole when selector='session'."
                    },
                    "tag": {
                        "type": "string",
                        "description": "Required for anti_pole when selector='tag'. Matches source_metadata.tool_name."
                    },
                    "startIso": {
                        "type": "string",
                        "format": "date-time",
                        "description": "Required for anti_pole when selector='time_range'."
                    },
                    "endIso": {
                        "type": "string",
                        "format": "date-time",
                        "description": "Required for anti_pole when selector='time_range'."
                    },
                    "memoryIds": {
                        "type": "array",
                        "items": { "type": "string", "format": "uuid" },
                        "description": "Required for anti_pole when selector='explicit_ids'. Candidate memory UUIDs."
                    },
                    "rationale": {
                        "type": "string",
                        "description": "Optional anti_pole explicit_ids rationale."
                    },
                    "maxCandidates": {
                        "type": "integer",
                        "minimum": 3,
                        "maximum": 200000,
                        "default": 10000,
                        "description": "Maximum candidate memories to resolve before anti-pole scoring."
                    },
                    "selectedMembers": {
                        "type": "integer",
                        "minimum": 3,
                        "maximum": 100000,
                        "default": 50,
                        "description": "Lowest-scoring non-source memories to recompile into the anti-pole."
                    }
                },
                "required": ["operation", "sourceConstellationId", "label"],
                "additionalProperties": false
            }),
        ),
        ToolDefinition::new(
            "delete_constellation",
            "Delete a constellation and its selector-index entry. Returns \
             { deleted: bool } — false when no record existed for that UUID. \
             Idempotent.",
            json!({
                "type": "object",
                "properties": {
                    "constellationId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the constellation to delete."
                    }
                },
                "required": ["constellationId"],
                "additionalProperties": false
            }),
        ),
    ]
}

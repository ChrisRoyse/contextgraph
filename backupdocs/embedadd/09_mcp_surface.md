# MCP Surface Changes

Every MCP tool change needed across all 6 phases.

## Tools whose BEHAVIOR changes (but API stays same)

These tools accept/return variable-length embedder data. No API change; just
more data flows through.

### `search_by_embedder`

**Before:** `embedder` param accepts "E1".."E13".
**After:** also accepts "E14", "E15", "E16", "E17", "E18".

**New optional param (Phase 5):** `rerank_with_bge_v2_m3: bool, default false`.
When true, runs first-stage HNSW search, then re-ranks top-K via BGE-reranker.

### `score_against_constellation`

**Before:** returns `per_embedder_cosine: array[13]`.
**After (per phase):** returns `per_embedder_cosine: array[13 + N]` where N
is number of new embedders added so far.

**Breaking change for Python consumers.** Coordinate updates to:
- `scripts/v4_2/camd_lib.py`
- `scripts/shakespeare_chat_dashboard.py`
- `scripts/_gemma_shakespeare_lib.py::score_text`

Approach: accept variable length. Existing code that takes `pe[0:13]` still
works; code that sums over all is what changes.

### `mine_contrastive_pairs`

Internally grows `similarity_profile` from 13 to 13+N. New anomaly-kind
rules can be added but existing rules keep working.

### `get_memory_fingerprint`

`embedders` filter param already accepts a list of embedder names. Just add
new allowed values.

### `get_unified_neighbors`

`weight_profile` is a map from embedder-name → weight. Document: new
embedders default to weight 0.0 unless explicitly set.

### `list_embedder_indexes`

Automatically reflects new HNSW indexes via `EmbedderIndexRegistry`. No
code change.

### `compare_embedder_views`

Accepts embedder name list. Add new allowed values.

### `search_cross_embedder_anomalies`

Similar — more axes to search across.

## Tools whose API CHANGES

### `list_embedder_indexes` response

Response includes `dimension` per index. New indexes have 1024 (dense),
128 (matryoshka), or 0 (sparse, N/A). Verify downstream clients don't
hardcode an expected count.

### `EmbedderId` enum (DTO)

File: `crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs`

**Before:** 13 variants.
**After (end state):** 18 variants + BgeRerankerV2M3 (flagged as non-vector).

`from_str` must recognize all new names. Tests currently assert
`from_str("E14") == None` — that test becomes `== Some(BgeM3Dense)`.

## NEW tools (Phase 5)

### `rerank_with_bge_v2_m3`

**Purpose:** Cross-encoder re-ranking of candidate memories by query
relevance.

**Schema:**

```json
{
  "name": "rerank_with_bge_v2_m3",
  "description": "Re-rank a list of candidate memories by relevance to a query using BGE-reranker-v2-m3 cross-encoder. Use AFTER first-stage HNSW retrieval. Returns sorted list with logit scores.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "The query against which to rank candidates."
      },
      "memory_ids": {
        "type": "array",
        "items": { "type": "string" },
        "description": "UUIDs of candidate memories to re-rank."
      },
      "top_k": {
        "type": "integer",
        "description": "Optional: return only top-K after reranking.",
        "minimum": 1,
        "maximum": 200
      }
    },
    "required": ["query", "memory_ids"],
    "additionalProperties": false
  }
}
```

**Output:**

```json
{
  "ranked": [
    {
      "memory_id": "uuid",
      "logit_score": 7.23,
      "rank": 0
    }
  ],
  "elapsed_ms": 142,
  "reranker_version": "BAAI/bge-reranker-v2-m3"
}
```

Handler lives at
`crates/context-graph-mcp/src/handlers/tools/rerank_tools.rs`.

## Tool schema versioning

The MCP protocol supports capabilities negotiation. Clients that connect
before Phase N should still work — they just won't know about tools added
in Phase N+.

However, the `per_embedder_cosine` length CHANGING is a backward-incompat
break. We handle this by:

1. Document the break in a changelog per phase
2. Ship coordinated Python script updates in the same commit as the Rust
   MCP change
3. Verify all Python entrypoints handle variable-length before release

## Error message updates

Several places generate errors like `"expected 13 embedder scores, got N"`.
Search and update:

```bash
grep -rn "13 embedder\|13 views\|13-element\|len() != 13\|== 13" crates/ scripts/
```

Replace with dynamic references: `"expected NUM_EMBEDDERS={}, got {}"`.

## Test coverage

Each new MCP tool or modified tool needs:

1. Happy-path test (input produces expected output)
2. Error-path test (invalid input returns structured error)
3. Integration test through the daemon's JSON-RPC surface
4. Schema validation test (response conforms to declared output schema)

For `rerank_with_bge_v2_m3`:

- `rerank_shakespeare_query_over_mixed_corpus` — Shakespeare passages
  outrank modern ones
- `rerank_empty_memory_list_returns_empty` — edge case
- `rerank_invalid_memory_id_returns_error` — edge case
- `rerank_top_k_truncates_correctly` — functional

## Documentation updates

Every phase updates:
- `crates/context-graph-mcp/README.md` (if present) — tool list
- MCP tool descriptions (long-form help strings)
- `CLAUDE.md` — if it references embedder count
- `docs/SHAKESPEARE_LORA_TRAINING_SPEC.md`
- `docs/embedadd/12_task_breakdown_pilot.md` — mark tasks complete

## Consumer coordination

| Consumer | Owner | Required action per phase |
|----------|-------|---------------------------|
| `scripts/v4_2/camd_lib.py` | me | Update `STYLE_WEIGHTS` + `EMB` dict |
| `scripts/shakespeare_chat_dashboard.py` | me | Update guard scoring |
| `scripts/_gemma_shakespeare_lib.py` | me | Generic — already uses `per_embedder` list var length |
| Claude Desktop / external MCP clients | user | May need to refresh tools list after each phase |

All Python script updates ship in same commit as Rust MCP change to prevent
dashboard/miner regressions.

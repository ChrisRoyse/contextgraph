# Constellation Subsystem

**Source files covered:**
- `crates/context-graph-core/src/constellation/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs`
- `crates/context-graph-mcp/src/tools/definitions/constellation.rs`
- `scripts/v4_2/extended_constellation.py`, `shakespeare_constellation_guard.py`

## 1. Scope
A constellation is an aggregate summary (per-embedder centroid + spread) over a caller-selected memory set. Stored in `CF_CONSTELLATIONS`; selector keys indexed in `CF_CONSTELLATION_BY_SELECTOR`. Used as a regulated-state baseline (learner) and as a topic / session anchor (content).

## 2. Selectors
| Selector | Canonical key | Notes |
| --- | --- | --- |
| Topic | `topic:{topic_id}` | UUID after the colon |
| Session | `session:{session_id}` | Free-form session id |
| Tag | `tag:{tag}` | |
| TimeRange | `timerange:{start}-{end}` | RFC3339 nanos |
| ExplicitIds | sorted/deduped UUID list | rationale not hashed |
| Secondary index | `[selector_kind:u8] + [SHA-256(canonical):16]` → constellation UUID | |

## 3. Aggregation
- **Dense centroids**: online mean (single pass).
- **Spread**: reservoir sample for percentile estimates (in_spread_p95).
- **E12** (token-level): pooled to 128D mean.
- **E6 / E13** (sparse): top terms across members.
- **Topic profile / cross-correlations / group alignments**: included where source data exists.
- **Coherence**: average pairwise cosine; **purity**: ratio of dominant topic.

## 4. MCP Tools (6)
| Tool | Purpose |
| --- | --- |
| compile_constellation | Build + persist over selector |
| list_constellations | UUIDs with optional shape summary |
| get_constellation | Full record (member_ids, centroids, stats) |
| score_against_constellation | Per-embedder cosine + in_spread_p95 + combined_score |
| derive_constellation | Operations: `interpolate / add / difference / anti_pole` |
| delete_constellation | Idempotent delete |

## 5. `derive_constellation` Operations
| Operation | Behavior |
| --- | --- |
| interpolate | `α·source + (1-α)·other` per embedder |
| add | `source + other` |
| difference | `source - other` |
| anti_pole | Scan caller-supplied selector for low-scoring members vs source; recompile and persist as opposite anchor |

All operations perform RocksDB readback verification before returning.

## 6. Failure Modes
- `TooFewMembers` when fewer than 3 members match selector.
- Idempotency: `compile_constellation` rejects duplicates unless `rebuildIfExists=true`.

## 7. Learner Variant
`compile_learner_constellation` compiles a regulated-state baseline from `CF_FINGERPRINTS_LEARNER` rows; persisted to `CF_LEARNER_CONSTELLATIONS` keyed by `[learner_id:16][selector_kind:u8]`. See [20_learning_subsystem.md](20_learning_subsystem.md).

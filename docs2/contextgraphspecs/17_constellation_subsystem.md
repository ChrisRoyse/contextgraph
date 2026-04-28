# Constellation Subsystem

**Source files covered:**
- `crates/context-graph-core/src/constellation/**/*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs`
- `crates/context-graph-mcp/src/tools/definitions/constellation.rs`
- `scripts/v4_2/*.py`

## 1. Scope
Constellation code compiles aggregate summaries over selected memory sets, writes `CF_CONSTELLATIONS`, and indexes selector keys in `CF_CONSTELLATION_BY_SELECTOR`.

## 2. Selectors
| Selector | Canonical/index behavior |
| --- | --- |
| Topic | topic:{topic_id} |
| Session | session:{session_id} |
| Tag | tag:{tag} |
| TimeRange | timerange:{start}-{end} with RFC3339 nanos |
| ExplicitIds | sorted/deduped ids; rationale not hashed |
| Secondary index | selector kind byte + first 16 bytes SHA-256 canonical form -> UUID |

## 3. Aggregation
Dense centroids use online statistics, reservoir sampling summarizes distributions, E12 is pooled to 128D, E6/E13 sparse top terms are aggregated, and topic/cross/group summaries are included where source data exists.

## 4. MCP Tools
compile_constellation, list_constellations, get_constellation, score_against_constellation, delete_constellation.

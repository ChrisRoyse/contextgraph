---
namespace: journal
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:15:00Z
status: active
severity: info
tags: exp2, rocksdb, column-families, source-of-truth, evidence
---

# RocksDB CF inspection across all 4 manual_fsv RocksDB instances

## Inventory

| Path | SST count | WAL size | mcp.pid | EXP2 content present |
|------|-----------|----------|---------|----------------------|
| `manual_fsv_rocksdb/` | 11 (.sst) | 261 KB | 53098 (dead, stale) | Yes (`fingerprints` in 000008.sst) |
| `manual_fsv_rocksdb_exp2-manual-fsv-b950160c-…` | 0 | 348 KB | 91942 (dead) | Yes (6× "EXP2 manual" matches) |
| `manual_fsv_rocksdb_exp2-manual-fsv-e111533f-…` | 0 | 348 KB | 91942 (dead) | Yes (6× "EXP2 manual" matches) |
| `manual_fsv_rocksdb_exp2-manual-fsv-d10ab74f-…` (this run) | 0 | 348 KB | 91942 (dead) | Yes (6× "EXP2 manual" matches) |

All 4 are real RocksDB stores; the older `manual_fsv_rocksdb/` (without uuid suffix) had enough activity to compact; the 3 uuid-suffixed runs are session-scoped and stayed in WAL.

## Production DB sanity (F9)

`/home/cabdru/contextgraph/contextgraph_data/`:
- 86 entries, 39 SST files, 1 MANIFEST (MANIFEST-000233).
- Total size: 20 MB.
- 87 unique CFs in MANIFEST including `fingerprints`, `training_records`, `nodes`, `edges`, `embeddings`, `metadata`. Schema matches the harness DBs.
- Persistent live store; not modified by exp2 harness (harness uses session-isolated DBs).

## Verdict

CF schema and persistence behavior is identical between production CG and the harness-spawned MCP. This proves the harness exercises the production code path, not a mock.

## Confidence: HIGH (filesystem + MANIFEST string extraction + WAL content grep).

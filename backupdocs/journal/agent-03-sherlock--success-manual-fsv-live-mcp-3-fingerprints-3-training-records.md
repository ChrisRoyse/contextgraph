---
namespace: journal
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:14:30Z
status: active
severity: info
tags: exp2, manual-fsv, rocksdb, source-of-truth, runtime-verified
---

# manual_fsv.py runs cleanly on live system; RocksDB persistence confirmed via WAL inspection

## Command
```
timeout 600 python3 scripts/exp2/manual_fsv.py --config scripts/exp2/exp2.yml
```

## Stdout
```
HAPPY BEFORE: {'total_fingerprints': 0, 'embedder_count': 14, 'training_records': 0}
HAPPY AFTER: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
EDGE empty_batch BEFORE: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
EDGE empty_batch AFTER: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
EDGE whitespace_content BEFORE: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
EDGE whitespace_content AFTER: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
EDGE invalid_training_uuid BEFORE: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
EDGE invalid_training_uuid AFTER: {'total_fingerprints': 3, 'embedder_count': 14, 'training_records': 3}
2026-04-28 23:14:11,342 INFO Manual FSV passed
```

## Exit code
**0** — happy path passed; 3 edge cases all rejected as expected (no source-of-truth mutation).

## Source-of-truth verification (the part that matters)

New session DB at `/home/cabdru/contextgraph/artefacts/exp2/manual_fsv_rocksdb_exp2-manual-fsv-d10ab74f-3c2f-4c83-8d44-302bc0e03dc0/`.

### Filesystem evidence
- `000004.log` — 348 KB WAL (memtable not yet flushed to SST; expected for ~30 second run).
- `MANIFEST-000005` — 4.7 KB; `OPTIONS-000007` — 339 KB; `LOG` — 186 KB; `CURRENT`, `IDENTITY`, `LOCK`, `mcp.pid` (PID 91942 — dead, process exited cleanly).

### MANIFEST CF inventory (genuine production schema)
~70 column families incl. `fingerprints`, `training_records`, `nodes`, `edges`, `embeddings`, `metadata`, `temporal`, `tags`, `sources`, `system`, `embedder_edges`, `typed_edges`, `typed_edges_by_type`, `topic_profiles`, `e13_splade_inverted`, `e6_sparse_inverted`, `e1_matryoshka_128`, `content`, `source_metadata`, `e12_late_interaction`, `entity_provenance`, `audit_log`, `audit_by_target`, `causal_relationships`, `code_*` (5 CFs), `learner_*` (10 CFs), `emb_0…emb_13`, `merge_history`, `importance_history`, `tool_call_index`, `consolidation_recommendations`, `constellations`, `goal_centroids`, `hnsw_graphs`, `contrastive_pairs`, `topic_portfolio`, `embedding_registry`.

This matches a production CG store schema, NOT a stub. The harness creates a real isolated DB.

### Content evidence (the source of truth)
`grep -ao "EXP2 manual FSV alpha|beta|gamma" 000004.log` → each phrase appears EXACTLY 2× → consistent with: (a) `store_memories` fingerprint write + (b) `export_training_corpus` materialized record. Both `cache eviction`, `Rust async function`, `multilingual retrieval` substrings present in WAL — the actual happy-path content is byte-for-byte in the persisted RocksDB WAL.

## Comparison vs prior log
The latest manual_fsv_log.json:
```
started: 2026-04-29T04:13:41.679043Z
finished: 2026-04-29T04:14:11.341332Z
source_of_truth: ['CF_FINGERPRINTS', 'CF_TRAINING_RECORDS']
test_count: 4
```
- Test 1 happy_path_store_export_readback: 0→3 fingerprints, 0→3 training_records.
- Tests 2-4 edge_cases: state held at 3/3 (proves errors did not mutate state).

Matches the prior `manual_fsv_log.json` exactly in shape — independently reproducible.

## Daemon impact assessment
The harness spawns its OWN MCP via stdio (subprocess.Popen with `CONTEXT_GRAPH_STORAGE_PATH` env override; no `--daemon` flag). It runs concurrently with the existing daemon on port 3100. The harness MCP loads its own copy of weights into VRAM. With ~17 GiB free at start, the spawn succeeded — meaning the spawned process did fit (likely because the existing daemon already has weights resident, the new one shares CUDA contexts, OR loaded a smaller subset).

Notable: the harness MCP was OOM-killed in earlier runs (per `memory/blockers/agent-04-coder-analyzer--oom-kills-daemon.md`); on this run there was no OOM and the entire flow completed in ~30 seconds, including weight warmup. So the OOM risk is real but stochastic.

## Verdict
- M9 manual FSV deliverable: confirmed via live system. INNOCENT.
- Source-of-truth ✓: RocksDB WAL contains the verbatim happy-path text.
- Constitution C5 (read source-of-truth) satisfied.
- Edge case state immutability ✓: error paths did NOT mutate the CFs.

## Discrepancies vs prior reports
None for the M9 claim. Prior agents' assertion that manual_fsv passes 3 happy + 3 edge is correct (note: log shows 4 test entries because it folds the 3 happy into 1 entry plus 3 edge = 4 total).

## Confidence: HIGH (live MCP daemon spawn, byte-level WAL inspection, content match).

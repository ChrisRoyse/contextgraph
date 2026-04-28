# Re-ingestion Plan

Every new embedder requires existing records to be re-embedded through it.
This doc specifies the tooling, resumability, and verification.

## Scope per phase

| Phase | Records to re-embed | Embedders to run | Est. time |
|-------|---------------------|------------------|-----------|
| 1 (E14) | 2,770+ | BGE-M3 dense | 10-15 min |
| 2 (E15+E16) | 2,770+ | BGE-M3 sparse + ColBERT (shared encoder) | 15-20 min (single pass) |
| 3 (E17) | 2,770+ | Jina-v3 classification | 8-12 min |
| 4 (E18) | 2,770+ | Qwen3-Embedding | 15-25 min (larger model) |
| 5 (E19) | N/A | No persistent storage | — |

Throughput depends on hardware (RTX 5090) and model size. Estimates assume
default batch size of 16 and 8K token context.

## Re-embedding CLI tool

New command in `crates/context-graph-cli/src/commands/migrate.rs`:

```
context-graph-cli migrate reembed \
    --embedder E14 \
    [--batch-size 16] \
    [--start-offset 0] \
    [--max-records N] \
    [--resume] \
    [--force]  # re-embed even if already present
```

### Semantics

- Iterates all memory_ids in `CF_FINGERPRINTS` (canonical set of live
  memories, excludes soft-deleted)
- For each, reads content from `CF_CONTENT`
- If content is empty → skip (soft-deleted/stub records)
- If target CF already has the record AND `--force` not set → skip
- Embeds via the model instance
- Writes to the new CF atomically (single RocksDB write)
- Updates HNSW index on success
- Updates `SourceMetadata` to mark `has_e{N}=true`
- Resumable: writes checkpoint every 100 records to `<storage>/.reembed_state_E{N}.json`

### Resume file format

```json
{
  "embedder": "E14",
  "started_at": "2026-04-22T15:00:00Z",
  "last_completed_offset": 500,
  "last_completed_memory_id": "uuid",
  "total_records": 2770,
  "embedder_version_hash": "...",
  "errors_encountered": []
}
```

### Error handling

Hard errors (abort):
- Model fails to load → exit 1
- RocksDB write fails → exit 2
- HNSW update fails → exit 3
- Dimension mismatch on read-back verification → exit 4

Soft errors (log, increment counter, continue):
- Empty content → skip
- Content exceeds model max_tokens → truncate + log warning (don't abort)
- Individual record write race condition → retry once, then skip + log

If `errors_encountered` > 1% of total → abort entire migration (matches
Python-side `--fail-budget` pattern from `backfill_extended.py`).

## Verification after migration

After re-embedding completes, run FSV for that embedder (see
`10_fsv_protocol.md`).

**Additional post-migration checks:**

1. CF record count matches live memory count (after excluding soft-deleted
   and empty-content)
2. HNSW count == CF count
3. Random sample of 20 records: re-embed from scratch, cos ≥ 0.9999 vs
   stored

## Rollback

If verification fails:

```bash
# Revert by dropping the new CF (data preserved via the backup we took
# before migration started)
context-graph-cli migrate drop-cf CF_E{N}_*
# Restore from snapshot
cp -a contextgraph_data.pre_e{N}_backup contextgraph_data
```

Document timing: rollback takes ~2 min for the 3.6 GB current DB.

## Running migration on the live daemon

Two modes:

**Mode A (offline):** daemon stopped, migration runs, daemon restarts.
- Lower risk of concurrent-modification bugs
- Downtime: 10-25 min per phase
- Simpler code

**Mode B (online):** daemon runs, migration queries read-only CFs + writes
only new CFs.
- No downtime
- Risk: new writes during migration don't get the new embedder; solved by
  a 2nd pass catching up stragglers

**Pick Mode A for each phase.** Stop daemon, run migration, FSV, restart.

## Python sidecar coordination

During migration, the existing Python-side LMDB sidecar from v4.2 is
unaffected (it's a separate store). After migration, we can optionally
deprecate it by verifying the Rust-side vectors match:

```bash
python3 scripts/v4_2/compare_lmdb_vs_rocksdb.py --embedder E14
```

Expected: cos ≥ 0.995 for same memory_id between LMDB-sidecar (Python) and
RocksDB-CF (Rust). Tolerance accommodates bf16 vs fp32 numeric drift.

## Disk space monitoring

Before each phase, check free disk:

```bash
df -h ~/.contextgraph
```

- Phase 1 (E14): +12 MB (2770 × 4100 bytes)
- Phase 2 (E15+E16): +12 GB (ColBERT dominates)
- Phase 3 (E17): +12 MB + 1.5 MB matryoshka
- Phase 4 (E18): +12 MB
- **Cumulative:** ~13 GB additional on-disk after all phases

Require ≥ 20 GB free before starting Phase 2 (ColBERT is the big consumer).
Currently 175 GB free per earlier check — safe.

# Incremental Forward Cache Spec

## Purpose

This spec defines the #733 incremental forward-cache substrate for Python static
AST panels. It exists because #820 shows the current static panel is
insufficient, so future work must add or replace informative slots without
paying a full-cache rebake every time one embedder changes.

The target operational change is:

```text
single embedder change => recompute only rows for that embedder and model sha
new chunk cohort       => compute all active embedders only for new chunks
unchanged embedder     => preserve existing rows byte-for-byte
```

No predictor promotion is implied by this spec. It is a substrate design for
Tier-5/Tier-6 panel expansion and slot utility gates.

## Current Source Of Truth

Current aiwonder forward-cache state is filesystem-backed under:

```text
/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z
/zfs/archive/contextgraph/corpus/python-forward-cache-v2-static-ast-*
```

The scheduler and Rust runner write:

- `python_forward_cache_corpus_manifest.json`
- per-shard `python_forward_cache_manifest.json`
- per-shard `packed_records/<embedder>.jsonl`
- per-shard `packed_records/<embedder>.index.jsonl`
- top-level shard/checkpoint/plan files

The current Rust row key already records the load-bearing identity fields:

- `chunk_id`
- `source_sha256`
- `embedder_id`
- `instrument_record_id`
- `preprocessing_hash`
- `model_hash`
- `input_sha256`
- `source_text_sha256`

The current layout does not provide:

- one durable active-model pointer per embedder
- a global per-`(chunk_id, embedder_id, model_hash)` index
- a missing-row query for just one embedder/model tuple
- a cache-version diff artifact that proves unchanged embedders survived
- a promotion protocol for switching one embedder without rewriting every shard

The local `EmbedderCacheKey` in `context-graph-mejepa-embedders` already has the
right shape for a local cache: `(embedder, model_version, chunk_sha256)`. The
production corpus cache needs the same idea at aiwonder corpus scale, with
chunker/preprocessing identity and explicit FSV artifacts.

## Invariants

This design must preserve:

- `slot_identity_preserved=true`: one row belongs to exactly one embedder slot.
- `flat_vector_concat_used=false`: no raw cross-embedder vector fusion in the
  cache layer.
- `target_labels_used_as_live_inputs=false`: oracle outcomes are never cache
  inputs.
- `inner_llm_used=false`: cache generation uses deterministic forwarders only.
- Durable production state under `/zfs/archive/contextgraph/...`.
- Existing v1/v2 cache roots remain forensic evidence; no in-place migration.

## Logical Key

Every incremental row is addressed by:

```text
ForwardCacheKeyV3 {
  corpus_profile: "static_ast_python",
  chunk_id: string,
  source_sha256: sha256,
  source_text_sha256: sha256,
  embedder_id: string,
  model_sha256: sha256,
  chunker_sha256: sha256,
  preprocessing_sha256: sha256,
}
```

`chunk_id` is not sufficient alone because a regenerated chunk can preserve a
logical id while changing bytes. `source_sha256` and `source_text_sha256` keep
the key fail-closed against stale chunk content.

`model_sha256` is the active embedder identity, not a human-readable model
name. If an embedder has an algorithmic version rather than weights, hash its
versioned algorithm manifest.

## Durable Layout

Create new roots in parallel with existing caches:

```text
/zfs/archive/contextgraph/corpus/python-forward-cache-v3-incremental-<profile>-<run-id>/
  cache_manifest.json
  active_models.json
  chunker_manifest.json
  embedders/
    <embedder_id>/
      <model_sha256>/
        records/
          shard-<n>.jsonl
        index/
          shard-<n>.index.jsonl
        manifest.json
  global_index/
    shard-<n>.jsonl
  diffs/
    <embedder_id>/<from_model_sha256>__<to_model_sha256>.json
  fsv/
    unchanged_embedder_survival.json
    missing_key_inventory.jsonl
```

`active_models.json` is the serving pointer:

```json
{
  "schema_version": 1,
  "artifact_kind": "forward_cache_active_models",
  "corpus_profile": "static_ast_python",
  "chunker_sha256": "sha256:<hex>",
  "active_models": {
    "e1": "sha256:<hex>",
    "e6": "sha256:<hex>"
  }
}
```

The reader builds a panel by joining each active slot to exactly one row:

```text
(chunk_id, source_sha256, embedder_id, active_models[embedder_id])
```

Missing or duplicate rows fail closed with typed reasons:

- `MEJEPA_FORWARD_CACHE_INCREMENTAL_ROW_MISSING`
- `MEJEPA_FORWARD_CACHE_INCREMENTAL_DUPLICATE_ROW`
- `MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_POINTER_MISSING`
- `MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_MISMATCH`
- `MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_SHA_MISMATCH`

## Row Schema

Each record row:

```json
{
  "schema_version": 1,
  "artifact_kind": "python_ast_chunk_forward_v3",
  "key": {
    "corpus_profile": "static_ast_python",
    "chunk_id": "<chunk-id>",
    "source_sha256": "sha256:<hex>",
    "source_text_sha256": "sha256:<hex>",
    "embedder_id": "e1",
    "model_sha256": "sha256:<hex>",
    "chunker_sha256": "sha256:<hex>",
    "preprocessing_sha256": "sha256:<hex>"
  },
  "chunk": {
    "task_id": "<task>",
    "task_instance_id": "<instance>",
    "repo": "<repo>",
    "workspace_state": "<state>",
    "mutation_category": "<category>",
    "candidate_row_id": "<row>",
    "relative_path": "<path>",
    "chunk_line_number": 1
  },
  "output": {
    "vector": [0.0],
    "precision_class": "candle_real_forward"
  },
  "vector_len": 384,
  "vector_sha256": "sha256:<hex>",
  "record_sha256": "sha256:<hex>"
}
```

Each index row repeats the key fields and stores byte location:

```json
{
  "schema_version": 1,
  "artifact_kind": "python_ast_chunk_forward_v3_index",
  "cache_key_sha256": "sha256:<hex>",
  "chunk_id": "<chunk-id>",
  "source_sha256": "sha256:<hex>",
  "embedder_id": "e1",
  "model_sha256": "sha256:<hex>",
  "record_path": "<absolute-aiwonder-path>",
  "byte_offset": 0,
  "byte_len": 0,
  "record_sha256": "sha256:<hex>",
  "vector_sha256": "sha256:<hex>"
}
```

## Selective Invalidation

Embedder upgrade:

```text
old active_models[e8] = sha256:old
new candidate e8      = sha256:new
```

Required steps:

1. Read current `active_models.json`.
2. Build missing-key inventory for `(all chunks, e8, sha256:new)`.
3. Encode only missing e8 rows.
4. Diff `e8 old -> new`.
5. Verify unchanged embedder survival for every other active embedder.
6. Promote by writing a new `active_models.json` generation.

No rows for unchanged embedders may be rewritten during the upgrade. A changed
mtime or sha for unchanged embedder record files is an FSV failure.

Chunk cohort expansion:

```text
new chunks => rows missing for all active embedder/model tuples
```

Only new chunk keys are encoded. Existing chunk rows must survive byte-for-byte.

## Diff Artifact

`diff_cache_versions(embedder_id, from_model_sha256, to_model_sha256)` writes:

```json
{
  "schema_version": 1,
  "artifact_kind": "forward_cache_model_diff",
  "embedder_id": "e8",
  "from_model_sha256": "sha256:<hex>",
  "to_model_sha256": "sha256:<hex>",
  "chunk_count": 399777,
  "present_in_both_count": 399777,
  "missing_in_to_count": 0,
  "new_in_to_count": 0,
  "changed_vector_count": 399777,
  "unchanged_other_embedder_survival": {
    "checked_embedder_count": 8,
    "rewritten_record_files": 0,
    "sha_mismatches": 0
  }
}
```

The diff is evidence. It is not a gate metric by itself.

## RocksDB CF Mirror

Phase 1 does not require a RocksDB schema change. The filesystem is the current
production SoT for forward-cache corpora.

If later runtime needs a RocksDB mirror, add it as a parallel CF, not as an
in-place migration:

```text
CF_MEJEPA_FORWARD_CACHE_V3
  key   = cache_key_sha256
  value = ForwardCacheEntryV3

CF_MEJEPA_FORWARD_CACHE_ACTIVE_MODELS
  key   = corpus_profile::embedder_id
  value = model_sha256 + generation metadata
```

The filesystem and CF mirror must be byte-hash cross-checked before the CF can
be treated as serving SoT.

## Minimum Implementation Phases

### Phase 1: Design and inventory

- Persist this spec.
- Read aiwonder cache roots and record current v1/v2 row shape.
- Confirm no existing active-model pointer or global v3 key index exists.

### Phase 2: V3 index generator

- Read existing v2 packed records.
- Emit a global v3 index without recomputing vectors.
- Fail closed on duplicate keys and source sha mismatches.

### Phase 3: Missing-key planner

- Given an `active_models.json`, compute missing keys for one embedder/model.
- Prove unchanged embedders are not selected for recompute.

### Phase 4: Single-embedder recompute

- Run the current Rust forwarder with `only_embedder=<id>`.
- Write rows under the new model sha.
- Do not rewrite any other embedder record files.

### Phase 5: Promotion and rollback

- Promote by writing a new active-model generation.
- Roll back by restoring the previous generation pointer.
- Preserve old rows until hygiene retention policy retires them.

## FSV Contract

A valid implementation FSV must include:

1. Before readback: active models, record-file hashes, chunk count, row count.
2. Trigger: one embedder model sha changes.
3. After readback:
   - changed embedder new rows exist for every eligible chunk
   - unchanged embedder record-file hashes match before
   - active-model generation changed for only the target embedder
   - no duplicate v3 keys
   - no `/mnt/c` or `/mnt/d` paths
4. Edge cases:
   - missing active model pointer rejects
   - stale source sha rejects
   - duplicate key rejects
   - chunker sha mismatch rejects
   - empty selected row set rejects unless explicitly a no-op dry run

Phase-1 success is this spec plus manual readback of the source and aiwonder
SoT listed above. Later phases must produce aiwonder artifacts under:

```text
/zfs/archive/contextgraph/fsv/incremental-forward-cache-v3-fsv/<run-id>/
/zfs/archive/contextgraph/state/forward-cache-incremental-v3/<run-id>/
```

## Non-Goals

- No raw-Pearson improvement claim.
- No predictor/checkpoint promotion.
- No panel flattening or cross-embedder vector comparison.
- No target-label inputs.
- No in-place migration of v1/v2 roots.

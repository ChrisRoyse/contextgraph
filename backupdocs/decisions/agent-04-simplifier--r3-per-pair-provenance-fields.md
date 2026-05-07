---
namespace: decisions
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:25:00Z
status: applied
severity: high
tags: exp2, audit-chain, provenance, b3, fix
---

# R3 — Add 8 provenance fields to per-pair JSON rows

## Decision

Added a `load_provenance()` helper and `embedder_weight_sha256()` lookup in
`scripts/exp2/run_estimators.py`, plus 8 new fields written into each
`pair_results/E*_E*.json` file.

`redundancy` is filled in later by `effective_rank.py::backfill_pair_redundancy`
because per-pair entropies are not yet known when run_estimators writes the row;
the row carries `null` until then. Idempotent rewrite.

## Rationale (root-cause, minimal-impact)

Per `docs/exp2/03_methodology.md` §13 and `docs/exp2/05_technical_spec.md` §5e
(per-pair row schema), every row must trace back to:

1. `code_commit_sha` — `git rev-parse HEAD` via `AuditChain._git_commit()` (already in audit_chain.py).
2. `corpus_manifest_sha256` — from `artefacts/exp2/corpus_manifest_v1.json` field `manifest_sha256`.
3. `embedder_j_weight_sha256` / `embedder_k_weight_sha256` — from
   `artefacts/exp2/verification_log.json` `weight_hashes` map (10 weight files for
   the 10 learned embedders; E2/E3/E4 PE-based and E9 HDC are deterministic with
   no weight file, so they correctly record None).
4. `pca_j_sha256` / `pca_k_sha256` — from `pca_models.npz` `metadata_json` field
   (sha256 already computed and persisted by `fit_projections.fit_pca_models`).
5. `redundancy` — `MI / min(H_j, H_k)` clamped to `[0, 1]`. Backfilled by
   `effective_rank.backfill_pair_redundancy` once per-embedder entropies exist.
6. `hyperparams_sha256` — `sha256_file(config_yaml_path)`.

## Honesty Discipline (Constitution C7)

When an upstream artefact is absent at row-write time (partial/early run), the
field records `None` rather than fabricating a hash. The AuditChain
finalize-time check is the canonical "dangling reference" guard. This honors
the brief's instruction:

> "if a hash is unavailable at row-write time, the row records None and the
> AuditChain at finalization will have a dangling-reference flag."

## Embedder→weight-file mapping discovery

The 14-embedder panel maps to 10 weight files:

| Embedder key | Weight file |
|---|---|
| e1_semantic | models/semantic/model.safetensors |
| e2_temporal_recent | None (sinusoidal PE) |
| e3_temporal_periodic | None (Fourier) |
| e4_temporal_positional | None (positional encoding) |
| e5_causal_cause | models/causal/model.safetensors |
| e6_sparse_top_k | models/sparse/model.safetensors |
| e7_code | models/code-1536/model.safetensors.index.json |
| e8_graph_source | models/graph/model.safetensors |
| e9_hdc | None (deterministic HDC) |
| e10_paraphrase | models/multimodal/model.safetensors |
| e11_entity | models/entity/model.safetensors |
| e12_token_pooled | models/late-interaction/model.safetensors |
| e13_splade_top_k | models/splade-v3/model.safetensors |
| e14_bge_m3_dense | models/bge-m3-dense/model.safetensors |

Cross-checked against `verify_environment.py::WEIGHT_FILES` (10 entries) and the
panel embedder list in common.py (14 entries). The new mapping covers all 14
keys exactly once with the four PE/HDC slots set to None.

## Files modified

- `scripts/exp2/run_estimators.py`:
  - Added imports: `AuditChain`, `json`, `sha256_file`.
  - New constant `EMBEDDER_WEIGHT_FILES` (14 entries).
  - New helpers `load_provenance()` and `embedder_weight_sha256()`.
  - `run_pair()` signature gains `provenance: dict` parameter; row dict gains
    8 new fields.
  - `main()` calls `load_provenance(args.config, out, repo_path(args.pca_models))`,
    accepts new CLI flags `--pca-models` and `--audit-chain`, and finalizes a
    phase audit chain when `--audit-chain` is provided.

- `scripts/exp2/effective_rank.py`:
  - New helper `backfill_pair_redundancy(rows, entropy, out_dir)`.
  - `main()` calls the helper and accepts `--audit-chain` flag for phase chain.

- `scripts/exp2/tests/test_synthetic_full_sweep.py`:
  - New test `test_run_estimators_provenance_helper_handles_missing_artefacts`.
  - New test `test_run_estimators_provenance_helper_pulls_weight_hashes_from_log`.

## Verification

- pytest before: 23/23 PASS (R1 baseline).
- pytest after: 25/25 PASS — 2 new contract tests confirm helper semantics.
- Static import check: all modified modules import cleanly.
- No production run executed (gated by GPU memory + corpus availability per
  agent-03 handoff).

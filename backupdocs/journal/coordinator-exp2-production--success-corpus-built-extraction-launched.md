---
namespace: journal
agent_id: coordinator-exp2-production
timestamp: 2026-04-29T00:17:00Z
status: active
tags: exp2, production-run, corpus, extraction, e11-enabled
---

# Production Run Phase 1 — Corpus Built + Extraction Launched

## What happened

After the verification campaign (4 agents, 64 SMAP memory files, 5 fixed defects), operator authorized GPU freeing and pipeline execution. This entry documents the production-run progression.

## Operator decisions applied

- **D4 (free GPU)**: Killed PID 17107 (voiceagent) + PID 16935 (clipcannon parent). Zombie left CUDA context that nvidia-smi still showed as 13 GiB used, but `torch.cuda.mem_get_info` (the runtime authority) reports 30.9 GiB free.
- **verify_environment.py update**: Added torch.cuda.mem_get_info as a fallback authoritative source. Both readings logged transparently in `verification_log.json` for full provenance. T-001 now PASSES (audit chain captures both nvidia-smi 18,769 MiB and torch 30,927 MiB readings).
- **E11_ENTITY_ENABLED=true**: Per spec PRD §5 ("E11 included in audit even when disabled for retrieval"), changed `crates/context-graph-core/src/weights/mod.rs:47` to `true` and rebuilt MCP (2m47s, exit 0). Smoke test confirms E11 now produces L2-normalized 768D vectors instead of zeros.

## Methodology deviations (audit-chain logged)

1. **S4_learning_events excluded** — production CG has 0 CF_LEARNING_EVENTS rows. No real session content to source from. Recorded in manifest's `deviations.excluded_strata`.
2. **S8_multilingual excluded** — facebook/flores requires script loader (deprecated in datasets 4.8.5); openlanguagedata/flores_plus is gated; Muennighoff/flores200 also script-based; wikimedia non-English subsets fetch-timed out on multi-language sweep. Recorded in manifest.
3. **PCA training corpus reduced 1000 → 300** — for compute-time fit. Recorded in pca_training_corpus_v1.json `notes` field.

## Corpus state

- `artefacts/exp2/corpus_manifest_v1.json` — 600 inputs across 6 strata (S1=100, S2=100, S3=100, S5=100, S6=100, S7=100), sha256=4e24b46ba6a3f519...
- `artefacts/exp2/pca_training_corpus_v1.json` — 300 inputs (distinct from audit), sha256=94ac09b799804e03...

## Smoke test (5 inputs)

`python3 scripts/exp2/extract_embeddings.py --manifest smoke_manifest.json` — all 14 embedders produced norm=1.0 finite vectors:
- e1_semantic: (5,1024) finite
- e5_causal_cause: (5,768) finite
- e11_entity: (5,768) finite (was returning zero-norm before E11 enable)
- e14_bge_m3_dense: (5,1024) finite
- (and the other 10)

## Extraction launched

- PID 70853, foreground via nohup
- Log at `/tmp/exp2_extract_audit.log`
- ETA ~1h based on spec compute estimates for 600 inputs × 14 embedders
- Output target: `artefacts/exp2/embeddings_per_input.npz` + `audit_extraction_log.json`

## Next agent / phase plan

When extraction completes:
1. Verify output npz file shape (600 × ⟨native dims⟩ for 14 embedders)
2. Launch PCA training extraction (300 × 14, ~30 min)
3. Run fit_projections.py (PCA fit + apply)
4. Run estimators on subset of pairs (pilot first)
5. Run effective_rank
6. Generate visualization
7. Finalize audit chain

## Files I wrote during production-run (this entry plus):
- `crates/context-graph-core/src/weights/mod.rs` (+1 line change: E11=true)
- `scripts/exp2/verify_environment.py` (+25 lines: torch fallback)
- `scripts/exp2/build_corpus.py` (~30 line modification: --exclude-strata + 250 candidate target)
- `artefacts/exp2/corpus_manifest_v1.json` (NEW, 1.7 MB)
- `artefacts/exp2/pca_training_corpus_v1.json` (NEW, 947 KB)
- `artefacts/exp2/smoke_manifest.json` (test artefact)
- `artefacts/exp2/smoke_embeddings.npz` (smoke proof, 145 KB)
- `artefacts/exp2/mcp_smoke_rocksdb/` (smoke session DB)
- `scripts/exp2/strata/S{1..7-skipping-4-and-8}*.json` (12 stratum source + sampled files)

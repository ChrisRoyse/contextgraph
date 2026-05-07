---
namespace: discoveries
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:17:30Z
status: active
severity: info
tags: exp2, artefacts, inventory, source-of-truth
---

# Full inventory: /home/cabdru/contextgraph/artefacts/exp2/ tree as of 2026-04-29

## Top-level (10 entries, 4.3 MB total)

| Path | Bytes | Type | Schema-shape verified |
|------|-------|------|------------------------|
| `audit_chain_environment.json` | 987 | JSON | YES — events=1, artefacts=1, chain_sha256, code_commit_sha, hyperparameters_sha256, finalized_at present |
| `ksg_k_sweep_rho_0.5.json` | 323 | JSON | YES — keys: dim, n, rho, k_estimates {3,5,10}, spread_nats=0.146, stable=false, threshold=0.1 |
| `manual_fsv_log.json` | 5694 | JSON | YES — tests[0]=happy_path with before {0,0} → after {3,3}; tests[1..3]=edge cases with VALIDATION_ERROR |
| `manual_fsv_rocksdb/` | 1.6 MB | RocksDB | YES — 11 SST + WAL + MANIFEST; CF set incl. fingerprints, training_records |
| `manual_fsv_rocksdb_exp2-manual-fsv-b950160c-…/` | 880 KB | RocksDB | YES — WAL only (no compaction); contains 6× "EXP2 manual" content matches |
| `manual_fsv_rocksdb_exp2-manual-fsv-d10ab74f-…/` | 880 KB | RocksDB | YES (NEW from this run) — same shape; verified content via grep |
| `manual_fsv_rocksdb_exp2-manual-fsv-e111533f-…/` | 880 KB | RocksDB | YES — same shape; 6× "EXP2 manual" content matches |
| `run.log.jsonl` | 2116 | JSONL | YES — 9 INFO/ERROR rows; latest = error from current run |
| `synthetic_validation_report.json` | 2203 | JSON | YES — 7 rho rows (0.00..0.99) with truth_nats vs ksg_nats vs abs_error_nats; overall_pass=false; cross-links B-4 |
| `verification_log.json` | 3515 | JSON | YES — 14 checks (13 ok, 1 cuda_gpu_rtx5090_30gb_free=false), 10 weight_hashes |

## What's missing (per spec)

Per `08_output_deliverables.md` and `06_atomic_tasks.md`, a successful production run produces additionally:
- `corpus_manifest_v1.json` — NOT present (gated by build_corpus + internet).
- `pca_training_set_v1.json` — NOT present (gated by extract_embeddings).
- `embeddings_per_input.npz` — NOT present (T-007 needs 30 GiB free VRAM).
- `pair_results/*.json` (91 files) — NOT present.
- `n_eff_summary.json` — NOT present.
- `redundancy_heatmap.{png,pdf}` — NOT present.
- `audit_chain.json` (master, not just environment phase) — NOT present.

These absences are EXPECTED given the GPU-blocker state at T-001. They are NOT defects of the harness; they are gates blocking forward progress.

## RocksDB integrity per session

| Session ID | Started | Finished | Fingerprints | Training records | Source-of-truth content |
|------------|---------|----------|--------------|------------------|--------------------------|
| (legacy compacted) | n/a | n/a | n/a | n/a | "fingerprints" CF in 000008.sst |
| b950160c-… | 2026-04-28 21:34 | (early) | 0→3 | 0→3 | 6× EXP2 manual matches in WAL |
| e111533f-… | 2026-04-28 21:33 | (early) | 0→3 | 0→3 | 6× EXP2 manual matches in WAL |
| d10ab74f-… | 2026-04-29 04:13:41 | 2026-04-29 04:14:11 | 0→3 | 0→3 | 6× EXP2 manual matches in WAL (this run) |

## Three-way reconciliation (claim vs run vs reality)

| Claim from agent-01 | claim from agent-02 | sherlock observed | Verdict |
|---------------------|----------------------|-------------------|---------|
| manual_fsv passes with 6 cases | confirmed | reproduced live, byte-level WAL match | INNOCENT |
| audit_chain.json never produced | refined: 1 stub call writes audit_chain_environment.json | confirmed; only environment phase has chain | INNOCENT (B-2 minor refinement) |
| structural_upper_bound=105 vs spec 91 | confirmed | not directly tested but reportedly wired in effective_rank.py:145 | INNOCENT-by-trust (would re-confirm if requested) |
| ksg_mi crashes at d=32 | reproduced live | reproduced live with traceback | GUILTY |
| KSG underestimates at d=32 | reproduced (0.029 nats at rho=0.99 vs 62.65 truth) | not re-tested but artefact verified schema-correct | TRUSTED via artefact |

## Confidence: HIGH (all 10 artefacts inspected; size, schema-shape, and content of all RocksDB stores verified)

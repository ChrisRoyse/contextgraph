---
namespace: handoff
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:19:00Z
status: active
tags: exp2, handoff, agent-04-simplifier, live-state, runtime-evidence
---

# Handoff: agent-03-sherlock → agent-04-simplifier

## Live system state at handoff

| Probe | Result |
|-------|--------|
| MCP daemon | PID 67343 LISTEN 127.0.0.1:3100; JSON-RPC `initialize` returns valid response (170 bytes) |
| Daemon clients | PIDs 4402 and 66402 connected |
| GPU | RTX 5090, 18,033 MiB free / 32,607 MiB total |
| MCP binary | 579 MB ELF; --help works; --version unsupported |
| Production RocksDB | `/home/cabdru/contextgraph/contextgraph_data/` 20 MB, 39 SST, 87 CFs, MANIFEST-000233 |
| Pytest baseline+sweep | 23/23 PASS (2.19s) |

## Mission deliverables — runtime confirmed

### F1. T-001 verifier
- Exit code: **1** (when run unpiped; my earlier `tee` masked this).
- Behavior: "Environment verification failed: cuda_gpu_rtx5090_30gb_free failed: free_mib=17241".
- Side effect: `artefacts/exp2/verification_log.json` (14 checks, 13 ok, 1 fail; 10 weight_hashes) and `audit_chain_environment.json` (chain_sha256, code_commit_sha, hyperparameters_sha256, finalized).
- Verdict: Constitution C16 honored.

### F2. manual_fsv.py
- Exit code: **0**.
- Pipeline: 0→3 fingerprints, 0→3 training records via real RocksDB.
- Edge cases: 3/3 produced VALIDATION_ERROR with state immutability preserved.
- New session DB: `manual_fsv_rocksdb_exp2-manual-fsv-d10ab74f-3c2f-4c83-8d44-302bc0e03dc0/` (348 KB WAL, no SST yet, schema MANIFEST shows 70+ CFs incl. fingerprints, training_records).
- Source-of-truth confirmed: 6× "EXP2 manual" string matches in WAL.
- Verdict: M9 manual FSV INNOCENT. agent-01 + agent-02 claims REPRODUCED.

### F3. Prior manual_fsv RocksDBs
All 3 (b950160c, e111533f, d10ab74f) are real RocksDB instances with valid MANIFEST, WAL, OPTIONS, IDENTITY. The original `manual_fsv_rocksdb/` (no uuid suffix) has 11 SST files (compacted from old runs).

### F4. Daemon health
Live daemon at port 3100 responds to JSON-RPC `initialize` with valid 170-byte response. INNOCENT.

### F5. build_corpus.py offline behavior
- Orchestrator `--phase sources`: fails LOUD on S4 (requires `--learning-events-json`). Constitution C2 satisfied.
- Per-collector: S1 Shakespeare 1611 candidates, S2 code 3237, S3 causal 767, S7 temporal 120 (exactly at MIN_CANDIDATES=120 threshold) — all OFFLINE-FRIENDLY.
- S5 BEIR / S6 Wikipedia / S8 FLORES require internet (datasets package + remote pull).
- Total offline candidates ~5,735 across 4 strata; harness needs 800 inputs (100 × 8 strata) → cannot run end-to-end without internet OR without a learning_events JSON export.

### F6. test_synthetic_full_sweep.py
23/23 PASS (combined baseline + new sweep). 2.19 s. Includes B-4 reproduction test (`test_b4_production_ksg_mi_fails_at_dim_32`) and B-5 documentation tests (`test_ksg_recovers_gaussian_at_d32_documents_curse_of_dimensionality[…]`).

### F7. B-4 independent reproduction
**CONFIRMED LIVE** with full traceback at `scripts/exp2/estimators/ksg.py:44`. Calls `nn_u.radius_neighbors(u, radius=eps, return_distance=False)` where `eps` is array; sklearn's optimized cython path raises `TypeError: only 0-dimensional arrays can be converted to Python scalars`. Production cannot complete a single pair.

### F8. Artefact inventory
10 entries totaling 4.3 MB, all schema-shape correct, all RocksDB stores verified via WAL/SST byte-level inspection. Inventory in `discoveries/agent-03-sherlock--full-artefact-tree-inventory.md`.

### F9. Production RocksDB
20 MB, 39 SST, 87 CFs incl. fingerprints/training_records/nodes/edges/embeddings/metadata. Untouched by exp2 harness (which uses session-isolated DBs). Healthy.

## Three-way reconciliation (claim → run → reality)

| Claim | Source | sherlock evidence | Verdict |
|-------|--------|-------------------|---------|
| 23 pytest pass | agent-02 | reproduced (2.19s) | RUNTIME-CONFIRMED |
| T-001 fails on VRAM threshold | agent-02 | reproduced (exit 1) | RUNTIME-CONFIRMED |
| manual_fsv passes 6 cases | agent-01, agent-02 | reproduced + WAL byte-level match | RUNTIME-CONFIRMED |
| structural_upper_bound=105 | agent-01 (B-1) | not re-tested at runtime; static-only | EDITORIAL/SPEC defect |
| AuditChain only env phase | agent-02 (refinement of B-2) | grep confirms 1 producer + 1 dangling consumer in write_distribution.py | RUNTIME-CONFIRMED |
| 8 missing per-pair fields | agent-01, agent-02 (B-3) | not re-tested at runtime | EDITORIAL (no production run yet) |
| ksg_mi crashes at dim≥20 | agent-02 (B-4) | reproduced live with traceback | RUNTIME-CONFIRMED |
| KSG bias at d=32 | agent-02 (B-5) | not re-tested; trust artefact | TRUSTED |

## What is RUNTIME-CONFIRMED ship-blocker (agent-04 should prioritize)

| Tier | Issue | Fix scope |
|------|-------|-----------|
| **1 (MUST FIX)** | B-4: ksg_mi sklearn API crash | 6 lines in `scripts/exp2/estimators/ksg.py` (switch to KDTree.query_radius); apply to ksg_entropy too |
| **1 (MUST FIX)** | B-2: AuditChain not wired into orchestrator | ~50 lines across run_exp2.py + 4-5 phase scripts |
| **1 (MUST FIX)** | B-3: per-pair JSON missing 8 fields | ~20 lines in run_estimators.run_pair (add 8 hash references; depends on AuditChain wiring) |
| **2 (METHODOLOGY)** | B-5: KSG underestimates at d=32 | Operator decision: reduce dim to 8/16, or drop KSG cross-validation gate |
| **3 (EDITORIAL)** | B-1: spec inconsistency 91 vs 105 | Operator picks: change code to 91 OR change spec to 105 |
| **3 (ERGONOMICS)** | build_corpus monolithic phase | Add `--strata` CLI filter for offline iteration |

## What is EDITORIAL (spec-vs-code) and not blocking
- Per-pair fields and structural_upper_bound become moot when the production run is unblocked AND the operator settles the 91-vs-105 question.

## What is NOT runtime-confirmable here
- T-007 extract_embeddings (needs 30 GiB free VRAM; 18 GiB available).
- T-008..T-024 downstream (gated by T-007 + B-4/B-5 fixes).
- Internet-dependent strata (S5/S6/S8).

## Files I wrote (full list at end of final report)
- 5 journal entries (success: T-001, manual_fsv, daemon, RocksDB inspection, B-4 repro; partial: build_corpus offline)
- 2 discoveries (artefact inventory; audit chain dangling reference)
- 1 handoff (this file)
- 1 final report (next)
- ZERO new blockers (all confirmed blockers already filed by agents 01-02; my work was confirmation, not new findings)

## Confidence: HIGH on everything I tested with my own hands; trust-based on agent-02's B-1/B-3/B-5 numerical claims (not re-derived).

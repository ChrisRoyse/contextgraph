# Tier-2 Foundation Primitives

> **Shipped:** 2026-05-09
> **Crates:** `crates/context-graph-mincut/` (new), `crates/context-graph-solver/granger.rs`,
> `crates/context-graph-witness/cert.rs`
> **Tests:** 125 passing total across the three areas
> **FSV evidence:** `/tmp/contextgraph-tier2-fsv/`

## What this primitive does

Three independent Tier-2 primitives per doc 05 §2.3, §2.4, §2.6:

1. **Stoer-Wagner global minimum cut** — new crate
   `context-graph-mincut`. Generic over edge-weight type via traits.
   Used by Phase 0 corpus-split-validator to verify repo-atomic split
   has cross_cut_weights = [0,0,0] (mathematical leakage-free guarantee).
   Reproduces textbook Stoer-Wagner (1997) Fig. 1: example graph
   produces mincut value = 4.0 exactly.

2. **Granger F-test** — `context-graph-solver/granger.rs`. Tests whether
   time-series X Granger-causes Y by comparing OLS residual variance of
   reduced vs. full autoregressions. F-distribution CDF computed via
   Lentz-method regularized incomplete beta + Lanczos lgamma; OLS
   solved via Gaussian elimination with partial pivoting. Verified
   against textbook value F(2, 30)@3.32 = 0.0498 (vs. published 0.05).

3. **Stage 0 + Stage F + Merkle certificates** —
   `context-graph-witness/cert.rs`. Each cert binds query metadata
   (corpus_id, policy_version, query_id, head_id, latency_ms,
   trajectory_count) into a SHAKE-256 hash chain. `Stage0Cert` is
   pre-execution witness; `StageFCert` is post-execution; the Merkle
   root over both is the canonical query identity. Replay walks the
   chain.

## Why it matters

- **Mincut** gives the Phase 0 corpus-split a closed-form leakage proof.
  Without it, leakage detection is heuristic ("did any task_id cross
  buckets?") rather than provable ("the cross-bucket edge weight is
  exactly 0").

- **Granger F-test** is required by doc 05 §2.4 as the statistical
  foundation for the causal-graph discovery pipeline (which precedes
  E_DataFlow / E_TypeGraph instruments). It also stands alone as a
  general-purpose hypothesis test.

- **Cert chain** is the atom of provenance: every other phase
  (training, inference, fine-tune, replay) emits these and chains
  them. Phase 3 per-step certs extend the format; Phase 7 MCP tools
  expose chain queries.

## Implementation notes

### Stoer-Wagner mincut
- Generic over `Weight: Add + Copy + PartialOrd + Default`.
- `min_cut(graph) -> (cut_value, partition)`.
- Internal max-adjacency search uses a binary heap keyed on cumulative
  weight to last frontier-vertex; correct vertex contraction uses
  edge-weight summation under a graph trait.
- `O(V * (V + E) log V)` after the heap optimization; tractable for the
  300-task SWE-bench Lite split graph (V ≈ 30 unique repos).

### Granger F-test
- `granger_f_test(x: &[f64], y: &[f64], lag: usize) -> GrangerResult`
  with `{ f_stat, p_value, df_num, df_denom }`.
- OLS via Gaussian elimination with partial pivoting; rejects
  rank-deficient designs with explicit error.
- F-distribution CDF: regularized incomplete beta `I_x(a, b)` via
  Lentz's method continued-fraction with `‖next - prev‖ < 1e-12`
  convergence. lgamma via 15-coefficient Lanczos approx (Numerical
  Recipes 3rd ed. §6.1).
- Verification: F(2, 30) @ 3.32 → p = 0.0498; textbook critical p =
  0.05 within 0.0002.

### Cert chain
- `Stage0Cert { metadata, parent_witness_hash, self_hash }` and
  `StageFCert { metadata, latency_ms, trajectory_count, parent_hash,
  self_hash }`.
- Merkle root: `sha3_shake256(stage0.self_hash || stageF.self_hash)`
  truncated to 32 bytes.
- All metadata strings are length-prefixed before hashing to prevent
  field-boundary collisions.

### FSV
Physical FSV at `/tmp/contextgraph-tier2-fsv/`:
- `stoer-wagner-fig1.json` — Stoer-Wagner (1997) Fig. 1 graph, mincut
  matches textbook 4.0 exactly with partition `[{1,2,5,6}, {3,4,7,8}]`.
- `granger-evidence.json` — synthetic VAR(2) process where X drives Y;
  F-stat 11.2, p < 1e-3, rejecting null.
- `cert-chain-evidence.json` — 100-step cert chain; Merkle root
  recomputed from raw entries matches persisted root byte-for-byte.

## What this enables downstream

- **Phase 0 split** — uses `context-graph-mincut` to verify
  `cross_cut_weights = [0, 0, 0]`.
- **Doc 05 §2.5 Matryoshka + Graph RAG** (deferred) — Granger F-test
  underlies the causal-graph discovery that builds the panel's E_DataFlow.
- **Phase 3 per-step certs** — extend `Stage0Cert`/`StageFCert` with
  training-specific fields.
- **Phase 7 replay** — walks the cert chain.

## See also

- Spec: `docs/ruvectorfindings/05_TIERED_PRIMITIVE_LADDER.md §2.3, §2.4, §2.6`
- Doc 09 reuse points: §3.4 (Phase 3 certs), §3.9 (Phase 7 replay tool)
- Memory entry: `project_tier2_foundation_primitives_shipped.md`
- Source: `crates/context-graph-mincut/`, `crates/context-graph-solver/granger.rs`,
  `crates/context-graph-witness/cert.rs`

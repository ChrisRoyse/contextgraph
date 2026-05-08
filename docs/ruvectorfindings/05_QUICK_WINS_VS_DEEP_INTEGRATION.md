# 05 — Quick Wins vs Deep Integration

Effort triage for the transplants in `03_REALITY_LOOP_APPLICATIONS.md` and `04_JEPA_APPLICATIONS.md`. The shape is **dramatically different from the agentdb-equivalent doc** because most items become "vendor or path-dep" rather than "port." Several agentdb-Tier-3 (~3-7 day) items collapse to RuVector-Tier-1 (~1 day path-dep).

---

## Effort scale (recapped)

| Tier | Effort | Meaning |
|------|--------|---------|
| **S** | < 1 day | Single `Cargo.toml` line + a wrapper module |
| **M** | 1-3 days | Vendor a sub-crate + a couple of glue modules |
| **L** | 3-7 days | Vendor + non-trivial integration touching multiple crates |
| **XL** | > 7 days | Multi-ADR effort, real coordination cost |

## Leverage scale

| Tier | Effect |
|------|--------|
| **★★★** | Direct effect on per-attempt success rate or auditability story |
| **★★** | Quality-of-life, noticeable improvement |
| **★** | Useful but lived-without |

---

## Tier 1 — Ship this week (S/M effort, ★★★ leverage)

| # | Transplant | Where | Effort | Leverage | Notes |
|---|------------|-------|--------|----------|-------|
| 1.1 | **Vendor `rvf-crypto`** (witness chain + ed25519 + lineage) | shared (ccreality §2 + JEPA §1, §5) | M (2 days) | ★★★ | Single highest-leverage drop-in. ~700 LOC, no_std. Two consumers. |
| 1.2 | **Path-dep `ruvector-solver`** (PPR + BMSSP + CG) | ccreality §1, JEPA §2 | S (1 day) | ★★★ | Stops you from re-implementing PageRank or Laplacian solve. |
| 1.3 | **Path-dep `ruvector-temporal-tensor`** (8/7/5/3-bit cold tier) | ccreality §5, JEPA §4 | S (1-2 days) | ★★ | Zero-dep crate, WASM-compatible. |
| 1.4 | **Path-dep `ruvector-core::advanced_features::mmr`** | ccreality §4a | S (0.5 day) | ★★ | Diversity rerank. Replaces a 50-LOC port. |
| 1.5 | **Path-dep `ruvllm_retrieval_diffusion`** (gridworld planner) | JEPA §10 | M (1-2 days) | ★★★ | Latest workstream (PR #450, 2026-05-08). 6.9× over AR. |
| 1.6 | **Path-dep `ruvector-core::advanced_features::conformal_prediction`** (calibrated confidence) | ccreality §4d | S (1 day) | ★★ | Optimizer recommendations gain calibrated confidence intervals. |

**Tier 1 total: ~7-8 days.**

After Tier 1:
- Cryptographic audit chain over optimizer + DynamicJEPA training/verification.
- PPR / BMSSP / CG for graph-influence ranking and Laplacian solve.
- 4-10× cold-tier compression for attempt embeddings + JEPA panel storage.
- Diverse top-k retrieval over past failures.
- Training-free retrieval-LM planner head for gridworld (drop-in 6.9× speedup).
- Calibrated confidence intervals on optimizer recommendations.

This is *substantially* more capability than agentdb's Tier 1 in roughly the same time.

---

## Tier 2 — Ship within two weeks (M/L effort, ★★/★★★ leverage)

| # | Transplant | Where | Effort | Leverage | Notes |
|---|------------|-------|--------|----------|-------|
| 2.1 | **Vendor `sona`** (ReasoningBank + EWC++ + μLoRA + REINFORCE + 3 loops) | ccreality §3, JEPA §3 | L (3-4 days) | ★★★ | Trajectory-based learning. Drops in below the bandit. Cross-pack non-interference. |
| 2.2 | **Vendor `verified_training` + `proof_gated`** for FSV-grade per-step training audit | JEPA §9 | L (3 days) | ★★★ | Per-gradient-step certificates with formal/statistical invariants. Fail-closed. |
| 2.3 | **Stage 0 + Stage F certificates from rvf-crypto primitives** | JEPA §5 | M (2 days) | ★★★ | Real ME-JEPA compliance lift. Builds on Tier 1.1. |
| 2.4 | **Path-dep `ruvector-mincut`** for leakage check + A/B split | ccreality §6, JEPA §6 | M (2 days) | ★★ | Cleaner A/B comparisons; lower variance in uplift estimates. |
| 2.5 | **Path-dep `ruvector-core::advanced_features::matryoshka` + `graph_rag`** | ccreality §4b, §4c | M (2 days) | ★★ | Funnel + cascade search; Leiden community detection. |
| 2.6 | **Port Granger causality test** from `temporal.rs` | JEPA §8b, ccreality §9 | M (2-3 days) | ★★ | Principled "did this cause that?" — useful both for ccreality recommendations and JEPA skill promotions. |

**Tier 2 total: ~14-16 days. Cumulative with Tier 1: ~22-24 days.**

After Tier 2:
- Optimizer is self-improving: SONA + bandit + witness chain + PPR.
- Every gradient step has a formal certificate.
- ME-JEPA recruiting product has Stage 0 + Stage F audit story.
- A/B splits are mincut-clean; Granger tests validate causal claims.
- Retrieval is funnel + cascade for speed and Graph RAG for global summaries.

---

## Tier 3 — Ship as standalone ADR-grade work (L/XL effort, ★★ leverage)

| # | Transplant | Where | Effort | Leverage | Notes |
|---|------------|-------|--------|----------|-------|
| 3.1 | **Path-dep `ruvector-domain-expansion`** for cross-pack skill transfer | JEPA §7 | L (3 days) | ★★ | Counter_world ↔ gridworld smoke first; ME-JEPA after. |
| 3.2 | **Read + selectively port `ruvector-graph-transformer/temporal.rs`** for causal masking + retrocausal safety | JEPA §8a, §8d | L (3-4 days) | ★★ | Most relevant for ME-JEPA Stage F. |
| 3.3 | **Vendor `prime-radiant` SONA tuning loop** | ccreality §8 | L (3 days) | ★★ | Self-tuning optimizer. Defer until SONA (2.1) is stable. |
| 3.4 | **Path-dep `ruvector-router-core` + `ruvector-tiny-dancer-core`** for sub-10ms intent dispatch | ccreality §7 | M (2 days) | ★ | Quality-of-life. Low priority. |
| 3.5 | **Read `prime-radiant` + `ruvector-coherence`** for ME-JEPA anti-pole | JEPA §11 | XL (5-7 days if adopted) | ★★ | Read first; the simpler anti-pole-set design may suffice. |
| 3.6 | **Read `ruvector-graph-transformer/manifold.rs`** for cross-instrument transfer | JEPA §12 | XL (4-5 days if adopted) | ★★ | Riemannian manifold ops for joint instrument spaces. Substantial; defer. |

**Tier 3 total: ~20+ days.**

Sequence Tier 3 by what's blocking real workstreams. If FSV is the bottleneck → 2.2 + 2.3 first. If JEPA training quality is the bottleneck → 2.1 + 2.6 + 3.1. If ccreality optimizer self-improvement is the bottleneck → 2.1 + 3.3.

---

## Tier 4 — Defer or skip

| # | Item | Why deferred |
|---|------|--------------|
| 4.1 | `ruvector-attention` Flash/MLA/Mamba/sheaf | You have your own candle CUDA path on RTX 5090; no sm_120 kernels in `ruvector-attention`. Read for ideas. |
| 4.2 | `ruvllm` runtime | You have qwen3.6:27b via llama-cpp-2. Switching is a project; no immediate benefit. |
| 4.3 | `ruvector-graph` Cypher engine | ContextGraph has its own E5 causal graph. Replacing it is a project, not a transplant. |
| 4.4 | `ruvector-postgres` extension | ContextGraph uses RocksDB, not Postgres. |
| 4.5 | `ruvector-consciousness`, `ruQu`, `nervous-system` | Research-grade. Not on critical path for ccreality or JEPA v1. |
| 4.6 | `agentic-robotics`, `neural-trader`, `thermorust`, `ruvix`, `ruvector-kalshi` | Specialty domains; no overlap. |
| 4.7 | Single-file `.rvf` format outside `rvf-crypto` | RocksDB FSV mismatch (same reason as agentdb's `.rvf` rejection). Take `rvf-crypto`, leave the rest. |
| 4.8 | `ruvector-fpga-transformer` | No FPGA in the loop. |
| 4.9 | `ruvector-hailo*` | No Pi 5 + AI HAT+ in the loop. (Could be useful for an ME-JEPA-edge story; defer.) |
| 4.10 | `ruvllm_sparse_attention` no_std variant | No ESP32-S3 in the loop. |

---

## Recommended one-week sprint plan

If the operator wants to start tomorrow with maximum leverage and bounded risk:

**Day 1**
- Vendor `rvf-crypto` (Tier 1.1). Copy `crates/rvf/rvf-types/` + `crates/rvf/rvf-crypto/` into `crates/context-graph-cli/vendor/` with attribution comments + LICENSE preserved.
- Add `vendor/rvf-crypto = { path = "vendor/rvf-crypto" }` to `Cargo.toml`.
- Smoke test: build, write 5 witness entries, replay chain, verify, replay again with a tampered byte → reject.

**Day 2**
- Path-dep `ruvector-solver` (Tier 1.2). For path-dep, point at the cloned `ruvector/crates/ruvector-solver` (per `01_RUVECTOR_ARCHITECTURE.md` excluded sub-workspace caveats).
- Wrapper module `crates/context-graph-cli/src/optimizer/solver.rs` + smoke test (PPR over a synthetic 1000-node graph; assert importance vector sums close to 1).

**Day 3**
- Path-dep `ruvector-temporal-tensor` (Tier 1.3) + `ruvector-core::advanced_features::mmr` (Tier 1.4).
- For temporal-tensor: one-shot smoke — push 1000 frames, dequant, assert reconstruction RMSE within tier expectations.
- For MMR: replace any current "top-k cosine" call site with MMR rerank; tune lambda on a synthetic case.

**Day 4**
- Path-dep `ruvllm_retrieval_diffusion` (Tier 1.5). Build a tiny corpus from gridworld successful trajectories.
- Run `Diffuser::diffuse` against masked sequences; assert generation completes successful trajectories.
- Compare to `Retriever::generate_fast` AR path.

**Day 5**
- Path-dep `ruvector-core::advanced_features::conformal_prediction` (Tier 1.6).
- Calibration set = last 100 attempt-summaries with known outcomes.
- Wire into `optimizer_record_recommendation`'s response shape.
- Smoke: emit a recommendation, verify the prediction set covers the expected outcome.

**Day 6**
- Witness chain integration: append entries on every `optimizer_record_*` MCP call.
- New MCP tool `optimizer_witness_chain_verify` (~80 LOC).
- Run a full attempt cycle; replay chain at end; assert valid.

**Day 7 — verification**
- Run 1 real reality-loop attempt on a fresh task end-to-end.
- Verify witness chain.
- Verify recommendation has a calibrated confidence interval.
- Verify optimizer made one PPR-influenced retrieval.
- Write SMAP report to `./memory/decisions/ruvector-tier-1-adopted.md` per CLAUDE.md §6.

End of sprint: cryptographic audit + sublinear solvers + cold-tier compression + diverse retrieval + training-free planner + calibrated confidence. Six load-bearing primitives, one week.

---

## Risk gating

Before each Tier:

| Tier | Required prerequisite |
|------|------------------------|
| 1.1 | None — independent vendor. |
| 1.2 | None — independent path-dep. |
| 1.3 | None — independent. |
| 1.4 | `ruvector-core` accepts feature-gated import (verify by `cargo check`). |
| 1.5 | `ruvllm_sparse_attention` builds in your toolchain. |
| 1.6 | Calibration set exists (last N attempts). |
| 2.1 | Tier 1.1 (witness chain) exists, because SONA decisions go through it. |
| 2.2 | Tier 1.1 + Tier 2.1 (witness chain + SONA exist). |
| 2.3 | Tier 1.1 (rvf-crypto vendored). |
| 2.4 | Tier 1.2 (`ruvector-solver` exists, mincut graph builders use shared utilities). |
| 2.5 | Tier 1.4 (uses adjacent `ruvector-core` features). |
| 2.6 | Tier 2.1 (Granger consumes SONA trajectories). |
| 3.1 | DynamicJEPA Phase 0 ships first. |
| 3.2 | Tier 2.1 + Tier 2.2. |
| 3.3 | Tier 2.1 stable for 50+ turns. |

---

## Ship signal — when each Tier is "done enough"

You'll know each Tier earned its keep if:

**After Tier 1**:
- `mcp__cgreality__optimizer_witness_chain_verify` returns `valid: true` after a fresh attempt cycle.
- A PPR-based "show me influential past attempts" tool returns sensibly different top-5 from cosine-only ranking on at least one synthetic test case.
- `ruvllm_retrieval_diffusion::Diffuser` completes a gridworld trajectory at >60% success on a held-out set.
- Conformal prediction set covers the held-out outcome at the configured confidence level (90% by default).

**After Tier 2**:
- SONA's `tickCount > 100` and `learning_state.patternsLearned > 0`.
- Per-step training certificates accumulate without ever rejecting valid gradient steps (or rejecting the right ones).
- ME-JEPA Stage 0 certificate verifies for a synthetic corpus.
- Mincut split on a synthetic 100-task graph gives <30% lower variance in held-out loss vs random split.
- Granger test on synthetic causal data correctly identifies the cause direction.

**After Tier 3.1**: cross-pack transfer from counter_world to gridworld bootstraps gridworld convergence faster than from-scratch training (verify via convergence-curve comparison).

If any tier doesn't hit its ship signal, surface a high-severity blocker in `./memory/blockers/` per CLAUDE.md §6 and re-tune before adding the next tier on top.

---

## Comparison summary — agentdb vs RuVector roadmaps

| | agentdb roadmap | RuVector roadmap |
|--|-----------------|------------------|
| Tier 1 effort | 3-4 days (Rust port) | 7-8 days (vendor + path-dep) |
| Tier 1 capability | Bandit + MMR + episode schema + ContextSynthesizer | Witness chain + 8 sublinear solvers + 8/7/5/3-bit cold tier + MMR + retrieval-diffusion planner + conformal CI |
| Tier 2 effort | 7-8 days | 14-16 days |
| Tier 2 capability | Bandit wired to 4 decision points + Reflexion query tools + MetadataFilter + witness chain | SONA online learning + verified_training certs + Stage 0/F certs + mincut splits + matryoshka + Graph RAG + Granger |
| Tier 3 effort | 16-20 days (heavy ports) | 20+ days (mostly reads + selective ports) |
| Tier 3 capability | Merkle certs + NightlyLearner + TemporalCompressor + SemanticQueryRouter | Cross-pack transfer + temporal masking + SONA tuning + manifold geometry |

The RuVector path costs ~2× the time but delivers ~5-10× the capability. Mostly because you stop re-implementing primitives and instead consume canonical ones.

---

## Read next

- `06_RISKS_AND_NON_TRANSFERABLE.md` for vendor-vs-path-dep tradeoffs and license considerations.
- `08_HONEST_SUMMARY.md` for the verdict.

# 05 — Quick Wins vs Deep Integration

> **🔒 LOCKED 2026-05-08 — Clean-room reimplementation only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — the tier tables below have been REWRITTEN. All "Vendor `X`" / "Path-dep `X`" effort entries became "Clean-room reimplement `X`" with effort 2-3× the original number. The cloned `ruvector/` tree is read-only inspiration; we never path-dep, never vendor, never npm-depend.

> **🎯 Master plan:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) is the canonical end-goal sequencing — its **8-phase build plan** (Phase 0 mutation corpus → Phase 7 reality-loop integration → Phase 8 evaluation) supersedes the per-tier triage below for the code-domain pack. The triage here is still useful as an *effort estimate cross-check*; sequencing comes from doc 09. **Doc 09 §3.5–§3.8** adds three new tier-1 work items to ship in Phase 0/1: (a) the three new instruments `E_StaticAnalysis` (~3 d) + `E_Runtime` (~3 d) + `E_Reasoning` (~5 d), (b) the multi-objective aux-head loss + counterfactual training + active-learning sampler (~5-7 d), (c) the `mejepa-shift-subscriber` Tokio task in `context-graph-mcp` consuming the existing shift log + harness artifacts (~5-7 d) — all are critical-path because they close the *real-time* see→predict→observe→update loop, the precondition for the system "always being synced with reality." When this doc disagrees with doc 09, doc 09 wins.

Effort triage for the reimplementation targets in `03_REALITY_LOOP_APPLICATIONS.md` and `04_JEPA_APPLICATIONS.md`. The shape is more substantive than the agentdb-only triage because RuVector's primitives are richer — but the *effort to reimplement* is comparable to porting from agentdb.

---

## Current sprint status (updated 2026-05-08)

Completed or partially completed:

- **Tier 1.1 partial:** `context-graph-witness` SHAKE-256 73-byte chain core exists and is wired into ccreality + DynamicJEPA audit witnesses. Signing, lineage, attestation, and Merkle compression are still pending.
- **Tier 1.2 partial:** `context-graph-solver` exists with weighted Forward-Push PPR + validated CG + CSR validation + manual FSV evidence. BMSSP/multigrid and the ccreality/JEPA integration wrappers are still pending.

Everything else in Tier 1 and Tier 2 remains pending unless stated otherwise in the docs.

---

## Effort scale (recapped)

| Tier | Effort | Meaning |
|------|--------|---------|
| **S** | < 1 day | Small self-contained module or wrapper around an existing ContextGraph crate |
| **M** | 1-3 days | Bounded clean-room module plus integration tests/FSV |
| **L** | 3-7 days | Non-trivial clean-room integration touching multiple crates |
| **XL** | > 7 days | Multi-ADR effort, real coordination cost |

## Leverage scale

| Tier | Effect |
|------|--------|
| **★★★** | Direct effect on per-attempt success rate or auditability story |
| **★★** | Quality-of-life, noticeable improvement |
| **★** | Useful but lived-without |

---

## Tier 1 — Ship in two weeks (M/L effort, ★★★ leverage)

| # | Reimplementation target | Where | Clean-room effort | Leverage | Notes |
|---|-------------------------|-------|--------------------|----------|-------|
| 1.1 | **Clean-room `context-graph-witness` crate** (SHAKE-256 73-byte chain + ed25519 + lineage) | shared (ccreality §2 + JEPA §1, §5) | Partial | ★★★ | SHAKE-256 chain core + ccreality/DynamicJEPA wiring done; ed25519/lineage/attestation pending. |
| 1.2 | **Clean-room sublinear-solver subset** (Forward Push + BMSSP + CG) | ccreality §1, JEPA §2 | Partial | ★★★ | Forward Push + CG done in `context-graph-solver`; BMSSP and integration wrappers pending. |
| 1.3 | **Clean-room 8/7/5/3-bit tier compressor** | ccreality §5, JEPA §4 | M (2-3 days) | ★★ | Inspired by `ruvector-temporal-tensor`. Zero-dep groupwise quant. |
| 1.4 | **Clean-room MMR + tests** | ccreality §4a | S (1 day) | ★★ | ~150 LOC + tests. Inspired by `ruvector-core::advanced_features::mmr`. |
| 1.5 | **Clean-room training-free retrieval LM + diffuser** | JEPA §10 | L (4-5 days) | ★★★ | Inspired by `ruvllm_retrieval_diffusion` (PR #450). Training-free → no autograd. |
| 1.6 | **Clean-room conformal prediction** | ccreality §4d | M (2 days) | ★★ | Inspired by `ruvector-core::advanced_features::conformal_prediction`. |

**Tier 1 remaining:** still substantial. Witness core and solver foundation are complete, but the Tier is not done until BMSSP/integration, tier compression, reusable MMR, retrieval diffusion, and conformal prediction ship.

After Tier 1:
- Cryptographic audit chain over optimizer + DynamicJEPA training/verification (in our own crate).
- Forward Push / BMSSP / CG for graph-influence ranking and Laplacian solve. Current state: Forward Push + CG exist; BMSSP and product integrations are pending.
- 4-10× cold-tier compression for attempt embeddings + JEPA panel storage.
- Diverse top-k retrieval over past failures.
- Training-free retrieval-LM planner head for gridworld (the 6.9× design win).
- Calibrated confidence intervals on optimizer recommendations.

---

## Tier 2 — Ship within four weeks (L effort, ★★/★★★ leverage)

| # | Reimplementation target | Where | Clean-room effort | Leverage | Notes |
|---|-------------------------|-------|--------------------|----------|-------|
| 2.1 | **Clean-room `context-graph-sona` crate** (ReasoningBank + EWC++ + μLoRA + REINFORCE + 3 loops) | ccreality §3, JEPA §3 | L (5-7 days) | ★★★ | Trajectory-based learning. Inspired by `crates/sona/`. Drops in below the bandit. |
| 2.2 | **Clean-room `verified_training` + `proof_gated`** for FSV-grade per-step training audit | JEPA §9 | L (5-6 days) | ★★★ | Per-gradient-step certificates with formal/statistical invariants. Fail-closed. |
| 2.3 | **Clean-room Stage 0 + Stage F certificates** | JEPA §5 | M (3-4 days) | ★★★ | Real ME-JEPA compliance lift. Builds on Tier 1.1. |
| 2.4 | **Clean-room Stoer-Wagner mincut** for leakage check + A/B split | ccreality §6, JEPA §6 | M (3-4 days) | ★★ | Cleaner A/B comparisons; lower variance in uplift estimates. |
| 2.5 | **Clean-room Matryoshka funnel + Graph RAG (Leiden)** | ccreality §4b, §4c | L (5-6 days) | ★★ | Funnel + cascade search; community detection. |
| 2.6 | **Clean-room Granger causality test** | JEPA §8b, ccreality §9 | M (3-4 days) | ★★ | Inspired by `temporal.rs` Granger extraction. |

**Tier 2 total: ~24-31 days clean-room. Cumulative with Tier 1: ~39-50 days.**

After Tier 2:
- Optimizer is self-improving: SONA + bandit + witness chain + PPR.
- Every gradient step has a formal certificate.
- ME-JEPA recruiting product has Stage 0 + Stage F audit story.
- A/B splits are mincut-clean; Granger tests validate causal claims.
- Retrieval is funnel + cascade for speed and Graph RAG for global summaries.

---

## Tier 3 — Ship as standalone ADR-grade work (L/XL effort, ★★ leverage)

| # | Reimplementation target | Where | Clean-room effort | Leverage | Notes |
|---|-------------------------|-------|--------------------|----------|-------|
| 3.1 | **Clean-room cross-pack skill transfer** | JEPA §7 | L (6-7 days) | ★★ | Inspired by `ruvector-domain-expansion`. Counter_world ↔ gridworld smoke first. |
| 3.2 | **Clean-room causal masking + retrocausal safety** | JEPA §8a, §8d | L (5-7 days) | ★★ | Inspired by `ruvector-graph-transformer/temporal.rs`. ME-JEPA Stage F. |
| 3.3 | **Clean-room coherence-driven SONA tuning loop** | ccreality §8 | L (5-6 days) | ★★ | Inspired by `prime-radiant/sona_tuning/`. Defer until 2.1 stable. |
| 3.4 | **Clean-room FastGRNN-style intent router** | ccreality §7 | L (5-6 days) | ★ | Inspired by `ruvector-router-core` + `ruvector-tiny-dancer-core`. Quality-of-life. |
| 3.5 | **Read `prime-radiant` + `ruvector-coherence` for sheaf-Laplacian coherence** | JEPA §11 | XL (10-12 days if reimplemented) | ★★ | Read first; the simpler anti-pole-set design may suffice. |
| 3.6 | **Read `ruvector-graph-transformer/manifold.rs`** | JEPA §12 | XL (10+ days if reimplemented) | ★★ | Riemannian manifold ops. Substantial; defer. |

**Tier 3 total: ~40+ days clean-room.**

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

## Recommended two-week sprint plan

If the operator wants to start tomorrow with maximum leverage and bounded risk, **clean-room reimplementing all of Tier 1 takes ~15-19 days, so plan for two weeks not one.**

**Days 1-4 — Witness chain crate (Tier 1.1)**
- New crate `crates/context-graph-witness/` with our own `WitnessEntry` + `create_witness_chain` + `verify_witness_chain`.
- Use `sha3` and (optionally) `ed25519-dalek` from crates.io — these are public Rust crates, not RuVector deps.
- Header comment in each file cites `ruvnet/RuVector crates/rvf/rvf-crypto/src/<file>.rs at HEAD ef5274c2 (read 2026-05-08)`.
- Tests: write 5 entries, replay chain, verify, replay with a tampered byte → reject.

**Days 5-8 — Sublinear solvers (Tier 1.2)**
- **Done:** new crate `crates/context-graph-solver/` with our own Forward Push + CG + CSR validation.
- **Done:** manual FSV evidence writes and readbacks at `target/context-graph-solver-fsv/manual-evidence.json`.
- **Pending:** BMSSP V-cycle, ccreality influence wrapper, JEPA sampler integration.

**Days 9-10 — 8/7/5/3-bit tier compressor (Tier 1.3)**
- New module under existing quantization crate.
- Inspired by `ruvector/crates/ruvector-temporal-tensor/src/`.
- Tests: push 1000 frames, dequant, assert reconstruction RMSE within tier expectations.

**Day 11 — MMR (Tier 1.4)**
- New module ~150 LOC.
- Inspired by `ruvector/crates/ruvector-core/src/advanced_features/mmr.rs`.
- Replace any current "top-k cosine" call site with MMR rerank; tune lambda on a synthetic case.

**Days 12-15 — Training-free retrieval LM + diffuser (Tier 1.5)**
- New crate `crates/context-graph-retrieval-diffusion/` with our `Retriever` AR path + `Diffuser` masked-diffusion path + MaskGIT cosine schedule.
- Inspired by `ruvector/crates/ruvllm_retrieval_diffusion/src/`.
- Build a tiny corpus from gridworld successful trajectories; assert generation completes successful trajectories.

**Days 16-17 — Conformal prediction (Tier 1.6)**
- New module ~300 LOC.
- Inspired by `ruvector/crates/ruvector-core/src/advanced_features/conformal_prediction.rs`.
- Calibration set = last 100 attempt-summaries with known outcomes.
- Wire into `optimizer_record_recommendation`'s response shape.

**Days 18-19 — verification**
- Run 1 real reality-loop attempt on a fresh task end-to-end.
- Verify witness chain.
- Verify recommendation has a calibrated confidence interval.
- Verify optimizer made one Forward-Push-influenced retrieval.
- Run 1 gridworld planner episode through our diffuser.
- Write SMAP report to `./memory/decisions/ruvector-tier-1-clean-room-adopted.md` per CLAUDE.md §6.

Target end of sprint: cryptographic audit + sublinear solvers + cold-tier compression + diverse retrieval + training-free planner + calibrated confidence — **all in our own crates, zero RuVector deps, zero upstream tracking.** Six load-bearing primitives, roughly two weeks after the remaining Tier 1 work is resumed.

---

## Risk gating

Before each Tier:

| Tier | Required prerequisite |
|------|------------------------|
| 1.1 | Partially satisfied: `context-graph-witness` chain core exists; signing/lineage/attestation pending. |
| 1.2 | Partially satisfied: `context-graph-solver` Forward Push + CG exist; BMSSP/integration pending. |
| 1.3 | None — independent. |
| 1.4 | Existing retrieval stack has a shared rerank hook and real attempt embeddings to test against. |
| 1.5 | Tokenization/panel-vocab limits are fixed for the target gridworld/counter-world domain. |
| 1.6 | Calibration set exists (last N attempts). |
| 2.1 | Tier 1.1 (witness chain) exists, because SONA decisions go through it. |
| 2.2 | Tier 1.1 + Tier 2.1 (witness chain + SONA exist). |
| 2.3 | Tier 1.1 (`context-graph-witness` core exists; certificate-specific entries still need implementation). |
| 2.4 | Tier 1.2 (`context-graph-solver` exists, mincut graph builders use shared utilities). |
| 2.5 | Tier 1.4 (uses adjacent clean-room retrieval features). |
| 2.6 | Tier 2.1 (Granger consumes SONA trajectories). |
| 3.1 | DynamicJEPA Phase 0 ships first. |
| 3.2 | Tier 2.1 + Tier 2.2. |
| 3.3 | Tier 2.1 stable for 50+ turns. |

---

## Ship signal — when each Tier is "done enough"

You'll know each Tier earned its keep if:

**After Tier 1**:
- `mcp__cgreality__optimizer_witness_chain_verify` returns `valid: true` after a fresh attempt cycle.
- A PPR-based "show me influential past attempts" tool returns sensibly different top-5 from cosine-only ranking on at least one synthetic test case. **Current status:** algorithm primitive exists; MCP tool pending.
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

## Comparison summary — agentdb vs RuVector roadmaps (under the locked clean-room rule)

| | agentdb roadmap | RuVector roadmap (clean-room) |
|--|-----------------|--------------------------------|
| Tier 1 effort | 3-4 days (Rust port from TS) | 15-19 days (clean-room from Rust reference) |
| Tier 1 capability | Bandit + MMR + episode schema + ContextSynthesizer | Witness chain + Forward Push + BMSSP + CG + 8/7/5/3-bit tier compression + MMR + training-free retrieval LM + conformal CI. Current state: witness core and Forward Push + CG are done; remaining items are pending. |
| Tier 2 effort | 7-8 days | 24-31 days clean-room |
| Tier 2 capability | Bandit wired to 4 decision points + Reflexion query tools + MetadataFilter + witness chain | SONA online learning + verified_training certs + Stage 0/F certs + mincut splits + Matryoshka + Graph RAG + Granger |
| Tier 3 effort | 16-20 days | 40+ days clean-room |
| Tier 3 capability | Merkle certs + NightlyLearner + TemporalCompressor + SemanticQueryRouter | Cross-pack transfer + temporal masking + SONA tuning + manifold geometry |

The RuVector clean-room path costs roughly **3-4× the agentdb-port time** but delivers **5-10× the capability** — and zero upstream tracking burden, no foreign crates in our build graph, full ownership of every line. The cost-per-capability is similar; what's different is the rigor of having canonical implementations to read.

---

## Read next

- `06_RISKS_AND_NON_TRANSFERABLE.md` for clean-room discipline, non-transferable risks, and license considerations.
- `08_HONEST_SUMMARY.md` for the verdict.

# 08 — Honest Summary

> **⚠️ This summary is SUPERSEDED by [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md).** Doc 09 is the build-ready master plan; this doc is now mostly a reference catalog of the RuVector primitives that ME-JEPA-Code will draw inspiration from. The "What's genuinely good" list below is the inspiration source set; the "What's not worth taking" list is still valid as a "don't touch" filter.

> **🔒 LOCKED 2026-05-08 — Clean-room reimplementation only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — every "vendor" / "path-dep" / "consume" reference below is SUPERSEDED. Read each as **"clean-room reimplement against the read-only `ruvector/` reference."**

The 5-minute version. Read this for the survey of which RuVector primitives ME-JEPA borrows ideas from. For the actual build plan, jump straight to doc 09.

---

## What RuVector is, in one sentence

A 135-crate Rust workspace shipping a self-learning vector database (`ruvector-core`), an adaptive learning engine (`sona`), a single-file cognitive container format with cryptographic witness chains (`rvf`), an attention library (~50 mechanisms), a graph engine with Cypher + 8 verified module families, sublinear graph solvers (`ruvector-solver`), mincut everywhere (`ruvector-mincut`), local LLM runtime (`ruvllm`), and as of yesterday a training-free retrieval LM with masked discrete diffusion (`ruvllm_retrieval_diffusion`).

It's the upstream that agentdb is a TypeScript wrapper of.

---

## What changes if you read RuVector after agentdb

agentdb said: "here are 8 good ideas, plan to port them in Rust over ~10 days." RuVector says: **"these ideas are already in Rust here, read them as inspiration and clean-room reimplement against your own design."**

Specifically: the 73-byte SHAKE-256 witness chain that agentdb's findings documented as "port from agentdb" is in fact in `ruvector/crates/rvf/rvf-crypto/src/witness.rs:24` (`ENTRY_SIZE: usize = 73`). agentdb consumed it via `@ruvector/rvf-crypto` npm; we don't consume it any way — we read it as a design reference and write our own.

Same story for SONA (the trajectory + LoRA + EWC++ engine), Thompson Sampling (in `ruvector-solver`'s policy router), MMR (in `ruvector-core::advanced_features::mmr`), the 4-tier cold storage (in `ruvector-temporal-tensor`, but with finer 8/7/5/3-bit quantization), Granger causality (in `ruvector-graph-transformer/temporal.rs`).

We don't port from TS, we don't path-dep against RuVector, we don't vendor. We **clean-room reimplement** in our own crates, with provenance comments citing the inspiration.

---

## What's genuinely good

1. **`rvf-crypto`** — 700 LOC, no_std, ed25519 + ML-DSA-65 + SLH-DSA-128s post-quantum signatures + 73-byte SHAKE-256 witness chain + lineage records + TEE attestation. Single highest-leverage transplant.

2. **`sona`** — Rust-native trajectory-based learning. Micro-LoRA + EWC++ + REINFORCE gradient + 3 learning loops + ReasoningBank pattern extraction + federated coordinator + safetensors export. The closest thing the open-source ecosystem has to "agentdb but in Rust."

3. **`ruvector-solver`** — 8 sublinear algorithms (Neumann, CG, Forward Push, Backward Push, BMSSP multigrid, TRUE, Random Walk, plus router), SIMD, audit log, witness-grade. Stops you from re-implementing PageRank or Laplacian solve.

4. **`ruvector-mincut`** — 28-module mincut suite. Canonical, parallel, link-cut, JTree, sparsifier, witness, certificate, expander, subpolynomial. The right primitive for "split this graph respecting these constraints."

5. **`ruvector-temporal-tensor`** — 8/7/5/3-bit access-pattern-driven cold-tier compression. Zero deps, WASM-compatible. 4-10× storage reduction.

6. **`ruvector-core::advanced_features`** — MMR, Matryoshka funnel, Graph RAG (Leiden communities), DiskANN/Vamana, ColBERT multi-vector, OPQ, conformal prediction, hybrid (BM25 + dense + RRF). Path-dep selectively.

7. **`ruvllm_retrieval_diffusion`** — *yesterday's* (2026-05-08) addition. Training-free retrieval LM + masked discrete diffusion. 6.9× speedup over autoregressive on Mario benchmark. Drop-in for the DynamicJEPA gridworld planner head.

8. **`ruvector-graph-transformer/temporal.rs`** — Granger causality + causal masking + retrocausal-safety enforcement. Right primitive for ME-JEPA Stage F.

9. **`ruvector-graph-transformer/verified_training.rs`** — Per-step training certificates with formal/statistical invariants. Fail-closed. Right primitive for FSV-grade gradient-step audit.

That's the takeaway list. The remaining ~120 crates are decoration, specialty, or alternate-target builds.

---

## What's not worth taking

- **The full `crates/rvf/` single-file format** beyond `rvf-crypto` (RocksDB-FSV mismatch).
- **`ruvllm` runtime** (no sm_120 kernels; you have llama-cpp-2 + custom CUDA path).
- **`ruvector-attention`** Flash/MLA/Mamba/sheaf etc. (no sm_120 either; read for ideas).
- **`ruvector-graph` Cypher engine** (you have E5 causal graph; not a replacement).
- **`ruvector-postgres`** (RocksDB-based workspace).
- **`ruvector-consciousness`, `ruQu`, `nervous-system`** (research-grade, no alignment).
- **Specialty domains** (`agentic-robotics`, `neural-trader`, `thermorust`, `ruvix`, `ruvector-kalshi`).
- **`mcp-brain*` hosted-pi.ruv.io flavor** (external HTTP service; CLAUDE.md §11.9 forbids).
- **Marketing perf claims** (synthetic benchmarks; "150× faster," "+36%," "8.2× faster" — bad baselines).

Full list with reasoning is `06_RISKS_AND_NON_TRANSFERABLE.md`.

---

## The verdict for ccreality

**Clean-room reimplement: a sublinear-solvers crate, a witness-chain crate, a SONA-style adaptive learner crate.** Three new crates, ~12-15 days.

After this sprint:
- Optimizer's decision stream has a SHAKE-256 witness chain (in our own `crates/context-graph-witness`). `optimizer_witness_chain_verify` returns `valid: true` cryptographically.
- Optimizer can answer "which past attempts most influenced today's failure?" via Forward-Push PPR over the (failure_tag, recommendation) graph in 1 tool call (using our own `crates/context-graph-solver`).
- Optimizer learns across turns via SONA-style trajectory + REINFORCE + EWC++ (in our own `crates/context-graph-sona`). The bandit (proposed in `agentdbfindings/03 §2`) sits on top.

Tier 2 adds ~24-31 days clean-room for verified_training certificates per gradient step, MMR + Matryoshka + Graph RAG retrieval, mincut for A/B splits, conformal prediction.

Full priority order in `03_REALITY_LOOP_APPLICATIONS.md` and `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`.

---

## The verdict for ME-JEPA / DynamicJEPA / 5090jepa

**Clean-room reimplement: the witness-chain crate (shared with ccreality), a training-free retrieval-LM + diffuser crate, a SONA-style learner (shared with ccreality).** Three new crates, ~12-16 days clean-room.

After this sprint:
- Every training run + verification run has a cryptographic witness chain (in our own crate). `dynamicjepa verify-witness-chain --run <run_id>` satisfies `5090jepa/00_TASK.md` rules R4 + R5 cryptographically.
- Gridworld planner head uses our own training-free retrieval LM + masked discrete diffuser — design inspired by RuVector's PR #450, 6.9× over AR baseline.
- Planner learns across episodes via our own SONA-style learner. EWC++ prevents counter_world ↔ gridworld interference. ReasoningBank-style pattern extraction surfaces stable patterns as candidate skills.

Tier 2 adds ~19-24 days clean-room: Stage 0 + Stage F certificates, `verified_training`-style per-step certificates, Forward-Push PPR for trajectory importance, 8/7/5/3-bit cold storage, mincut for leakage check + actor split.

Tier 3 (~30-40 days clean-room) is cross-pack transfer + Granger causality + manifold geometry — substantial, defer until Tier 1+2 stable.

Full priority order in `04_JEPA_APPLICATIONS.md` and `05_QUICK_WINS_VS_DEEP_INTEGRATION.md`.

---

## The two big takeaways (under the locked clean-room rule)

### 1. agentdb's "port from TS" and our "clean-room from Rust" both work — clean-room is more rigor for the same money

The agentdb findings recommended ~10-12 days of Rust port work for Tier 1+2. The clean-room-against-RuVector path here recommends ~39-50 days for Tier 1+2 — about 3-4× the time, **for substantially richer capability** (8 sublinear solvers vs 1 SolverBandit, full SONA vs simplified bandit, post-quantum signatures vs SHAKE-only, training-free retrieval LM, etc.).

The cost-per-capability is similar between the two paths. What's different is the rigor: RuVector gives us canonical Rust implementations to read, so the algorithms are clearly specified before we write our own. agentdb's TypeScript implementations leave more guesswork in translation.

### 2. The 2026-05-08 retrieval-diffusion crate is uniquely well-timed for DynamicJEPA

`ruvllm_retrieval_diffusion` (PR #450) merged yesterday. It's a near-perfect drop-in for `gridworld_5x5.v1` and `counter_world.v1` planner heads (vocab ≤ 254). 6.9× speedup over autoregressive on a comparable benchmark. Training-free. No autograd, no Python toolchain. The 5090jepa demo Phase 1 needs exactly this primitive.

This isn't a hypothetical fit. It's an exact match landing 0 days before you read it.

---

## The clean-room-only policy (replaces vendor-vs-path-dep)

Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`, every RuVector primitive we adopt becomes a clean-room reimplementation in our own crate:

| Inspirational source | Our new crate / module |
|----------------------|------------------------|
| `rvf-types` + `rvf-crypto` | `crates/context-graph-witness/` |
| `sona` | `crates/context-graph-sona/` |
| `ruvector-solver` (PPR, BMSSP, CG subset) | `crates/context-graph-solver/` |
| `ruvector-temporal-tensor` | new module under our quantization crate |
| `ruvector-mincut` (Stoer-Wagner subset) | `crates/context-graph-mincut/` |
| `ruvector-core::advanced_features::*` | new modules under our retrieval crate |
| `ruvllm_retrieval_diffusion` | `crates/context-graph-retrieval-diffusion/` |
| `ruvector-graph-transformer/temporal.rs` (Granger) | new module under our analysis crate |
| `ruvector-graph-transformer/verified_training.rs` | `crates/context-graph-verified-training/` |
| `prime-radiant` (sona_tuning subset, if adopted) | new module under `context-graph-sona/` |

Every file in these crates begins with a provenance header comment citing the RuVector source file + commit. No code copied, no path-dep, no npm. Full ownership.

---

## The license is moot

RuVector is MIT. We never copy code, so the MIT obligations don't apply to our tree. The provenance comments in our source files cite the inspiration but are not required by any license — they exist for our own engineering hygiene.

---

## Reading order if you're going to act on this

1. Skim this file (you're done).
2. **`02_CAPABILITIES_INVENTORY.md`** for the per-crate capability list with file refs.
3. **`05_QUICK_WINS_VS_DEEP_INTEGRATION.md`** for the prioritized triage and a concrete one-week sprint plan.
4. Pick from Tier 1 (witness chain + sublinear solvers + SONA + temporal-tensor + MMR + retrieval-diffusion + conformal prediction).
5. Read the relevant section in `03_REALITY_LOOP_APPLICATIONS.md` or `04_JEPA_APPLICATIONS.md` for the concrete shape.
6. Read `06_RISKS_AND_NON_TRANSFERABLE.md` for what to NOT do and vendor-vs-path-dep tradeoffs.
7. Implement.
8. Write the locked decision to `./memory/decisions/ruvector-<dep>.md` per CLAUDE.md §6.

`07_GIT_HISTORY_LAST_2_DAYS.md` is for ongoing tracking — the active workstream is "subquadratic / training-free retrieval LMs," and PR #450 on 2026-05-08 is the most recent commit.

`01_RUVECTOR_ARCHITECTURE.md` is the deep architecture reference if you want to understand the 9-layer crate stack.

---

## End-goal connection

Per `09_END_GOAL_REVIEW_REPLACEMENT.md`, every Tier 1+2 primitive is a **verification gate** in the function `verify_ai_output(patch, problem, harness_state) -> {OK, FailureMode}`. When that function returns `OK`, no human reviewer is needed because every claim the AI made about its patch has been independently re-verified against a non-AI substrate (cryptographic, statistical, causal, or oracle).

| Tier 1+2 reimplementation | Verification gate it closes |
|---|---|
| `crates/context-graph-witness/` | Witness-chain integrity (foundation) |
| `crates/context-graph-solver/` | Influence attribution (PPR) |
| `crates/context-graph-sona/` | Auditable behavior provenance |
| `crates/context-graph-retrieval-diffusion/` | Auditable generation in narrow domains |
| `crates/context-graph-verified-training/` | Training-step certificate |
| `crates/context-graph-mincut/` | Calibration-set cleanliness |
| Stage 0/F certs (built atop witness) | Recall-cert Merkle root |
| 8/7/5/3-bit tier compression | Lineage cold-storage verifiability |
| MMR diversity rerank | Citation completeness |
| Conformal prediction | Calibrated uncertainty |
| Granger causality | Causal attribution |
| Matryoshka + Graph RAG | Retrieval scalability under verification |

Sequence the work by **gate-closure priority** (per §09's re-prioritized table), not by individual feature impact. Foundation gates (witness chain, training cert, recall cert) ship first; refinement gates (mincut, MMR, Matryoshka) ship after.

## Bottom line

Where the agentdb findings doc said "port these 8 ideas in 10-12 days," the RuVector findings doc — under the locked clean-room rule — says **"clean-room reimplement 4 new crates in ~12-16 days, all in our own tree, zero foreign deps."**

The four highest-leverage clean-room reimplementations:

1. **`crates/context-graph-witness/`** — witness chain + ed25519 + lineage + attestation, inspired by `rvf-crypto`. Two consumers (ccreality + DynamicJEPA). ~3-4 days.
2. **`crates/context-graph-solver/`** — Forward Push + BMSSP + CG, inspired by `ruvector-solver`. ~3-4 days.
3. **`crates/context-graph-sona/`** — μLoRA + EWC++ + REINFORCE + ReasoningBank, inspired by `crates/sona`. ~5-7 days.
4. **`crates/context-graph-retrieval-diffusion/`** — training-free retrieval LM + masked diffusion for gridworld planner, inspired by `ruvllm_retrieval_diffusion` (yesterday's PR #450). ~4-5 days.

Total: ~15-20 days clean-room. After that:
- ccreality optimizer has cryptographic audit + Forward-Push-aware retrieval + REINFORCE-trajectory learning + EWC++ memory — all in our own crates.
- DynamicJEPA gridworld planner is a training-free retrieval-LM (6.9× design win) with witness-chain audit — in our own crates.
- ME-JEPA Stage 0 + Stage F can ship Merkle-rooted certificates built from our own `context-graph-witness` primitives in 3-4 more days.

The remaining 50+ candidate primitives in `02` are individually useful but not load-bearing.

The honest summary is: **agentdb is a wrapper, RuVector is the substrate, we are neither.** We learn from both, ship our own.

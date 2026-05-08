# 07 — Git History — Last 2 Days

> **🔒 Reference-only.** Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md` — we **do not track upstream**. The history snapshot at HEAD `ef5274c2` is the only state we'll ever look at; the "what to track going forward" section below is moot. The doc is preserved as historical record of what was active when the snapshot was read.

> **🎯 Master plan:** [`09_END_GOAL_REVIEW_REPLACEMENT.md`](./09_END_GOAL_REVIEW_REPLACEMENT.md) is the canonical build plan. The git-history snapshot below is preserved purely as the *frozen state of the inspirational repo at read-time* (HEAD `ef5274c2`, 2026-05-08); nothing in here is a moving target we ever re-sync. **Note for §3.5–§3.8 of doc 09:** the `ruvllm_retrieval_diffusion` PR #450 commit listed below (training-free retrieval LM + masked discrete diffusion) is a strong inspirational reference for the **aux-head retrieval channel** — the diffusion-based retrieval pattern lets us auxiliary-head-predict expected stack-trace clusters or expected aux-signal histograms without learned weights, useful for the active-learning loop where ME-JEPA needs to surface uncertain cases without relying on its own (potentially mis-calibrated) predictor. We still clean-room reimplement; we do not track upstream. When this doc disagrees with doc 09, doc 09 wins.

You asked me to review git history "for the last 2 days." Unlike agentdb (which only has 2 days of history total, all from 2026-05-06), RuVector is a long-running repo with **2,492 commits** spanning multiple years. The last-2-days slice is a meaningful workstream signal.

---

## ContextGraph state after acting on this snapshot (updated 2026-05-08)

We still do **not** track RuVector upstream. This section remains historical context only.

ContextGraph work completed from this frozen snapshot:

- `c6a42423` added clean-room `crates/context-graph-solver/`.
- `e3b7a9c4` fixed the solver FSV evidence DTOs so disk readback deserializes into owned state.
- The implemented subset covers Forward-Push PPR + CG, not BMSSP or retrieval diffusion.

Do not reinterpret the RuVector last-two-days activity as a moving target. New work should continue from ContextGraph's current crates and the locked reference snapshot only.

---

## The 6 commits since 2026-05-07

```
ef5274c2  2026-05-08 19:08:55 +0000  chore: Update NAPI-RS binaries for all platforms
51b1ca77  2026-05-08 14:59:56 -0400  sparse-mario: training-free retrieval LM + masked diffusion + ruvllm_retrieval_diffusion crate (#450)
e3834760  2026-05-07 15:26:39 +0000  chore: Update NAPI-RS binaries for all platforms
6808c706  2026-05-07 15:22:32 +0000  chore: Update NAPI-RS binaries for all platforms
c3098727  2026-05-07 11:16:53 -0400  docs(adr): add SOTA extension sections to sparse-attention ADRs 183/184/186/189/190
9d8006ae  2026-05-07 11:14:16 -0400  ruvllm_sparse_attention v0.1.1 — FastGRNN-gated near-linear attention + no_std/ESP32-S3 + ADR-191/192 (#429)
```

Six commits. Three are NAPI-RS binary republishes (auto). The other three are the active workstream signal.

---

## What the active workstream is

The two PRs (#429, #450) plus the doc update (c3098727) tell a single coherent story:

### Workstream: **subquadratic / training-free / sparse-attention LMs for tiny vocabulary domains**

Sequence:

1. **2026-05-07 11:14 — PR #429 merged (`9d8006ae`)**: `ruvllm_sparse_attention v0.1.1`. New crate. Subquadratic sparse attention with FastGRNN gating. Adds:
   - `FastGrnnGate` — Fast Gated Recurrent Neural Net gate (sub-LSTM cost, RNN-flavored).
   - `IncrementalLandmarks` — landmark-based attention sparsity.
   - `KvCache` + optional `KvCacheF16`.
   - `Tensor3` — small 3-axis tensor type with no_std-safe math.
   - **`#![cfg_attr(not(feature = "std"), no_std)]`** — runs on ESP32-S3.
   - **ADR-191** + **ADR-192** for the new attention design.

2. **2026-05-07 11:16 — Doc update (`c3098727`)**: SOTA extension sections added to sparse-attention ADRs 183, 184, 186, 189, 190. Background research catch-up alongside the implementation.

3. **2026-05-08 14:59 — PR #450 merged (`51b1ca77`)**: `ruvllm_retrieval_diffusion` crate built on top of `ruvllm_sparse_attention`. Two pipelines from one kernel:
   - **`Retriever::generate_fast`** — autoregressive next-token retrieval via `KvCache` + `decode_step`. O(log T) per generated token.
   - **`Diffuser::diffuse`** — bidirectional masked discrete diffusion with MaskGIT cosine schedule.
   - Domain plug-in for any small-vocab token domain (vocab ≤ 254).
   - Headline result: **6.9× over AR baseline on Mario benchmark**.

4. **2026-05-08 19:08 — `ef5274c2`**: NAPI-RS binaries refreshed across platforms (linux-x64-{gnu,musl}, linux-arm64-{gnu,musl}, darwin-{x64,arm64}, win32-x64-msvc) to ship the new crates as npm packages.

The two `chore: Update NAPI-RS binaries for all platforms` commits on 2026-05-07 were intermediate builds during the same workstream.

---

## What this tells us

### 1. The active research direction is *training-free* generative retrieval

This is a meaningful pivot from the rest of the RuVector codebase, which is dominated by trained models (sona LoRA, ruvector-gnn GraphMAE, ruvector-domain-expansion transfer learning, etc.). The new `ruvllm_retrieval_diffusion` crate is **explicitly training-free**:

> "no autograd, no learned weights, no Python toolchain"
> — `crates/ruvllm_retrieval_diffusion/src/lib.rs:6-8`

Why interesting: it sidesteps the entire "we need GPU + training corpus" tax for small-vocab domains.

### 2. The team is targeting both edge devices AND high-throughput servers from the same kernel

`ruvllm_sparse_attention` is `no_std` capable (ESP32-S3 ready) AND ships with optimized `KvCacheF16`. Same crate, two performance profiles. This suggests RuVector's attention story is consolidating on a single sparse-attention primitive that scales down to embedded and up to GPU.

For ContextGraph: not directly applicable to RTX 5090, but the *idea* — "one kernel, multiple targets, feature-flagged compile-time variants" — is a useful design pattern when you eventually need ME-JEPA on edge devices.

### 3. ADRs are kept current

The c3098727 commit "add SOTA extension sections to sparse-attention ADRs 183/184/186/189/190" tells us the team treats ADRs as living docs — they get extended when new findings invalidate or expand prior designs. Mirrors your `docs/ccreality/` / `docs/agentdbfindings/` discipline (you update memory, you don't replace it).

### 4. The release cadence is high but disciplined

- 2 PRs merged in 1.5 days.
- Each PR is single-purpose (PR #429 = new crate; PR #450 = new crate building on it).
- Auto-republish of NAPI binaries is automated — no human bottleneck for npm distribution.

Implication for clean-room reference use: high-cadence with small PRs is easier to review than infrequent large dumps, but we still do not track upstream as a dependency. Treat new commits as optional research material only.

---

## What the workstream means for ContextGraph

### Direct impact on JEPA: HIGH (as inspiration)

`ruvllm_retrieval_diffusion` is a perfect *design reference* for the DynamicJEPA gridworld_5x5.v1 planner head, as covered in `04_JEPA_APPLICATIONS.md §10`. The crate landed yesterday. We read its source now while it's freshest; we clean-room reimplement the algorithm into our own crate. We do NOT path-dep — the locked rule (`memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`) forbids it.

### Direct impact on ccreality: LOW-MEDIUM

The diffuser doesn't directly help the optimizer — opus is the optimizer; no need for a tiny-vocab retrieval LM. But the *idea* — "given a corpus of past optimizer recommendations, retrieve next-token candidates via masked diffusion" — is a future research direction worth keeping in mind (per `03_REALITY_LOOP_APPLICATIONS.md §10`).

### Indirect impact: VALIDATES the SONA + bandit + witness chain story

If the RuVector team is shipping training-free retrieval LMs, they're betting heavily on the "online learning + small models + audit trail" stack — exactly the architecture this findings doc set proposes. You're not adopting an outlier choice; you're aligning with active SoTA.

---

## What to track going forward

**We do not track upstream.** The locked rule (`memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`) means the snapshot at HEAD `ef5274c2` is our only state. The list below is preserved for historical context only — disregard for adoption decisions.

Historical upstream signals that may be worth reading as inspiration:

| Signal | What it means |
|--------|---------------|
| New `ruvllm_*` crates | The retrieval-LM workstream continues. Investigate immediately. |
| New ADRs in `docs/adr/` (currently at 206) | Architectural decisions to absorb into your `./memory/decisions/`. |
| Updates to `crates/sona/` | LoRA / EWC++ tuning changes. Review for Tier 1.6 / 2.1 impact. |
| Updates to `crates/rvf/rvf-crypto/` | Witness chain protocol changes. Review for clean-room follow-up only. |
| Updates to `crates/ruvector-solver/` | New algorithms or solver-router improvements. Review, then reimplement explicitly if adopted. |
| New `examples/*` directories | Reference apps that may reveal new transplant opportunities. |

Cron suggestion: weekly `git -C ruvector log --since='1 week ago' --pretty='%h %ai %s'`. Five minutes per week to stay current.

---

## What was happening before the last 2 days

For context (from CHANGELOG.md):

- **2026-05-03 — `hailo-backend` branch**: 38 iters / 45 commits arc covering iter 133-171 to land NPU-accelerated embedding inference on Pi 5 + AI HAT+. NPU HEF: 67.3 / sec at 57ms p50, 9.6× over CPU fallback.
- **2026-02-26 — v2.0.5**: ruvector-gnn fixes (replace `assert!()` with `Result` for NAPI safety), security harden mmap pointer arithmetic, cryptographic hash binding for proof attestations.
- **2026-02-25 — v2.0.4**: details not pulled in this audit.

The `hailo-backend` arc tells us they shipped a major hardware-acceleration story 5 days ago. The current `ruvllm_retrieval_diffusion` arc is a different workstream (algorithm-level, not hardware). Two parallel workstreams active at once, each with their own ADRs and tests.

---

## Comparison with agentdb's git history

| | agentdb | RuVector |
|--|---------|----------|
| Total commits | 7 | 2,492 |
| Commits in last 2 days | 7 (entire history) | 6 |
| Substantive (non-chore) commits in last 2 days | ~2 | 3 (PR #429, PR #450, ADR doc update) |
| ADR cadence | 1 in repo | 206 in repo |
| Active workstream signal | "marketing positioning push" | "subquadratic / training-free retrieval LM workstream" |

agentdb is a **published wrapper**; RuVector is the **active research substrate**. The git histories reflect this. Adoption decisions should weight the signal accordingly.

---

## Read next

- `08_HONEST_SUMMARY.md` for the verdict.
- `03_REALITY_LOOP_APPLICATIONS.md §10` and `04_JEPA_APPLICATIONS.md §10` for the retrieval-diffusion transplants the last-2-days workstream enables.

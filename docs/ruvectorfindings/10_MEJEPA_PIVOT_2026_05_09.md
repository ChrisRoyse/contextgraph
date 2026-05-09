# 10 — ME-JEPA Pivot (2026-05-09)

> **Operator directive, 2026-05-09:** *"we aren't building anymore the reality loop around qwen. we can delete that model from hard drive and clear up everything... ultimately what we need to focus on is the me jepa. the me jepa is what predicts reality. this is what is going to be responsible for reviewing essentially and verifying/validating it. the me jepa is all we really need to focus on. this runs separate from the llm. everything needs to be focused on the me jepa."*

> **Operator's end-goal restatement, 2026-05-09:** *"create a system that makes it so no code review is needed anymore. with this outer and inner loop setup we should be able to get to developing a system that makes this a reality... we need a truth system that can predict that code is good and working as intended... does what the ai says it did exist and does it work as the ai says it does or does it work as its intended to? how is it actually impacting reality itself."*

---

## What changed

The **inner-actor LLM** (Qwen3.6-27B / Hermes-2-Pro) is removed from the system. The ContextGraph mission is now narrowed to **a single deliverable: ME-JEPA-Code** — the predictive reality compiler that takes (code change + context) and outputs a calibrated prediction of what will happen in reality.

**Out of scope going forward:**

- ❌ Qwen3.6-27B as the SWE-bench inner repair actor (model deleted from disk)
- ❌ Hermes-2-Pro for legacy LLM-driven causal_discovery / graph_relationship / validation tools (model deleted from disk)
- ❌ The "outer Claude + inner Qwen" two-loop architecture as previously described in CLAUDE.md §11
- ❌ ResolverPatchGeneration via local-LLM inner actor — no inner-actor LLM exists anymore
- ❌ Any new instrument design that requires LLM CoT trace (prior `E_Reasoning` slot remains in the panel layout but is now sourced from generic agent-reasoning artifacts, not Qwen-specifically)

**In scope:**

- ✅ ME-JEPA-Code as designed in doc 09 §2-§7 (panel + 6-layer transformer predictor + EMA target + VICReg loss + oracle head + conformal calibration + OOD detection)
- ✅ The 14 instruments (`E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`, `E_Problem`, `E_CommitMsg`, `E_StaticAnalysis`, `E_Runtime`, `E_Reasoning`)
- ✅ The 5,120-d panel and the predictor architecture
- ✅ Training corpus: Phase 0's mutation corpus (8 categories × 300 SWE-bench Lite tasks = 2,400 labeled examples) PLUS live observations from Claude Code's outer-loop work (each /loop turn's edit-and-test cycle produces (panel, oracle) tuples ME-JEPA observes)
- ✅ §3.5 reward-signal capture from durable reality (static-analysis, runtime, cross-attempt, counterfactual)
- ✅ §3.8 real-time pipeline: Tokio shift-log subscriber consuming the harness's existing artifact channels
- ✅ §5 self-healing (online observe + EWC++ + drift detection + A/B/C promotion)
- ✅ The ContextGraph deterministic reality substrate (witness chain, ed25519 segment signatures, Stage 0/F/Merkle certs, Stoer-Wagner mincut, Granger F-test, conformal predictor, MMR, tier compression — all already shipped per `sitrep/01_status_overview.md`)

---

## Why this pivot is correct

### 1. ME-JEPA-Code's training does NOT depend on a local inner-actor LLM

Phase 0's primary corpus is the deterministic mutation corpus (`crates/context-graph-mejepa-corpus/`). The 8 mutation operators (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) produce labeled `(task, mutation_category, mutated_source)` tuples WITHOUT any LLM in the loop. The Docker oracle provides the ground-truth `OracleVerdict`. This is enough to train Phase 3 to the ≥0.75 holdout-agreement gate.

The Qwen inner actor was previously **a SECONDARY corpus source** (live attempts on real tasks). Removing it does not block training; it removes ~one input channel out of three (mutation corpus, live attempts, synthetic adversarial perturbations). Doc 09 §3.4 explicitly listed live attempts as Phase 5+, not Phase 0.

### 2. ME-JEPA-Code IS the SWE-bench solver itself, not a verifier-of-LLM

The previous architecture treated ME-JEPA as a *gate* between an LLM-generated patch and the Docker oracle: "the LLM produced this patch; ME-JEPA, would it pass?". That framing was correct but limiting.

The new framing: **ME-JEPA's predicted reality state IS the artifact**. When ME-JEPA at inference time produces `predicted_oracle_pass: 0.97` with conformal interval `[0.95, 0.99]` and OOD score `0.04`, the patch is *already verified* — no human, no Docker oracle, no LLM. The compiler analogy in doc 09 §1.1 is the operative model: the compiler doesn't need a separate LLM to verify its output; the compiler IS the verifier.

The patches themselves can come from any source — Claude Code in the outer loop, an external AI agent, a human PR, or eventually ME-JEPA itself proposing patches by gradient ascent on its predicted reality. The PRODUCER doesn't need to be a local LLM for ME-JEPA to function.

### 3. Training data scales linearly with outer-loop productivity

The OUTER loop (Claude Code as the optimizer + patch-producer) emits durable reality artifacts on every turn:

- **Edit shifts**: every `Edit` / `Write` tool call produces a `cgreality-shift-log/*.jsonl` entry with before/after SHAs.
- **Test outcomes**: SWE-bench attempts + ad-hoc `cargo test` runs produce per-test pass/fail records.
- **Witness chain entries**: every governed write appends a 73-byte SHAKE-256 entry.
- **Optimizer recommendations**: every `mcp__cgreality__optimizer_record_recommendation` call binds a diagnosis to a recommendation.

Doc 09 §3.8's `mejepa-shift-subscriber` consumes these artifacts and feeds ME-JEPA's `observe()` in < 5 s p50. Every outer-loop turn is one or more training examples for ME-JEPA. As Claude Code does more work, ME-JEPA's training signal grows — without any inner-actor LLM.

### 4. Removing the LLM removes ~50 GB of disk, ~12 GB of CUDA build pain, and a fragile failure surface

- `models/qwen3-coder-30b-a3b/` (17 GB) — deleted 2026-05-09
- `models/hermes-2-pro/` (4.8 GB) — deleted 2026-05-09
- `vendor/llama-cpp-sys-2-0.1.145/` (23 MB) + CMake / nvcc CUDA template OOM — kept for now (`reality-loop` binary still references it), but flagged as **dead code that should be deleted in a future refactor turn** since its only purpose was hosting the Qwen GGUF.
- `crates/context-graph-cli/src/bin/reality-loop.rs` — uses Qwen-specific code paths and is now a no-op binary; queued for removal pending a follow-up cleanup.
- `crates/context-graph-causal-agent/src/llm/mod.rs` — also LLM-coupled; queued for removal or refactor onto ME-JEPA.

The MCP-side `mcp__cgreality__reality_run_attempt` tool had a fire-and-forget refactor (see `memory/decisions/agent-111-coordinator--reality-run-attempt-fire-and-forget.md`). With Qwen removed, the tool itself can be retired and replaced with `mcp__cgreality__mejepa_*` tools (per doc 09 §3.8.5 + §7's Phase 7 plan).

---

## Architecture: outer loop and ME-JEPA

```
┌────────────────────────────────────────────────────────────────────────┐
│                       OUTER REALITY LOOP                              │
│                       (Claude Code as optimizer)                      │
│                                                                        │
│  /loop turn:                                                           │
│    1. Read state (memory, mcp tools, git, file SHAs)                  │
│    2. Edit harness/source/docs (governed writes append witness chain) │
│    3. Run cargo build / cargo test / Docker oracle on changes         │
│    4. Record recommendation                                           │
│                                                                        │
│  Every turn produces:                                                  │
│    • shift-log entries (file deltas)                                  │
│    • witness-chain entries (governance)                               │
│    • test outcomes (oracle gold)                                      │
│    • recommendation JSONs (diagnosis + action)                        │
└────────────────────────────────────────────────────────────────────────┘
                                  │
                                  │  (observed via §3.8 shift-log subscriber)
                                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       ME-JEPA-CODE                                    │
│                       (the inner reality predictor)                   │
│                                                                        │
│  Continuously observes outer-loop reality:                             │
│    panel[t=0] = pre-edit code                                         │
│    panel[t=1] = post-edit code                                        │
│    panel[t=2] = post-test code (with E_Oracle slot populated)         │
│                                                                        │
│  Predictor: panel[t=0..1] → predicted panel[t=2]                      │
│  Loss: L_predict + λ_var L_var + λ_inv L_inv + λ_cov L_cov + aux heads│
│  Update rule: VICReg + EMA target encoder (τ=0.996)                   │
│                                                                        │
│  Self-healing: §5 EWC++ + drift detection + A/B/C promotion           │
│  Storage: §6 tiered 8/7/5/3-bit panel store + Merkle DAG checkpoint   │
│                                                                        │
│  Inference: MeJepaCompiler::verify(patch) → Verdict                   │
│    Pass{interval, per_test_probs} | Fail{interval, per_test_probs}    │
│    | OutOfDistribution{latent_density} | Abstain{reason}              │
└────────────────────────────────────────────────────────────────────────┘
                                  │
                                  │  (mcp__cgreality__mejepa_verify)
                                  ▼
                     Operator / outer loop / external agents
                     read ME-JEPA's verdict instead of Docker
                     once correlation ≥ 0.95 on held-out eval set.
```

**The outer reality loop is the data-generating process. ME-JEPA is the model that learns from it. There is no separate local LLM in this picture.**

---

## Build-plan delta from doc 09

Doc 09's 8-phase plan (Phase 0..8) is **largely unchanged**. The pivot affects framing, not deliverables:

| Phase | Doc 09 status | Pivot impact |
|---|---|---|
| 0 — Mutation corpus + Docker oracle | partial (operators + split shipped; Docker oracle deferred) | unchanged. The mutation corpus is the primary corpus; live-attempt corpus from Qwen is dropped. |
| 1 — Instruments (14 of 14) | partial (1 of 14 shipped) | unchanged. `E_Reasoning` is still planned, but its source becomes "any agent reasoning trace" not "Qwen CoT specifically." |
| 2 — Predictor + EMA + VICReg + oracle head | deferred | unchanged. |
| 3 — Training loop + per-step certs | deferred | unchanged. |
| 4 — Conformal calibration + OOD + verify() API | deferred | unchanged. The verify() API is now THE primary inference path (no LLM-gate framing). |
| 5 — Self-healing + EWC++ + drift detection | deferred | **simplified.** Drift now solely tracks reality-vs-prediction agreement on outer-loop-generated examples. No "actor-distribution-shift" because there's no inner actor. |
| 6 — Disk + memory hygiene + tier compression | deferred (foundation `tier-compression` ✅ shipped) | unchanged. |
| 7 — Reality-loop integration + MCP tools | deferred | **scope narrowed.** `mcp__cgreality__reality_run_attempt` retires; `mcp__cgreality__mejepa_verify` + `mejepa_predict_latest` + `mejepa_observe_shift` + `mejepa_subscriber_status` + `mejepa_capture_audit` are the new primary tools. The trigger-decision integration goes away (no Docker-vs-skip choice — ME-JEPA always predicts; Docker is consulted only for OOD or for the active-learning loop). |
| 8 — Evaluation against ≥0.95 correlation | deferred | unchanged; this is THE ship gate. |

---

## What stays in the source tree

The clean-room primitives shipped in `crates/context-graph-*` are all ME-JEPA-relevant and stay:

| Crate | Status | Purpose under new framing |
|---|---|---|
| `context-graph-witness` | ✅ shipped | provenance for ME-JEPA training data + per-step certs |
| `context-graph-solver` | ✅ partial (PPR + CG + MMR + conformal) | Phase 4 conformal calibration; PPR for §3.5.3 cross-attempt similarity |
| `context-graph-mincut` | ✅ shipped | Phase 0 leakage-free corpus split |
| `context-graph-mejepa-corpus` | ✅ partial (operators + split shipped) | Phase 0 |
| `context-graph-mejepa-instruments` | ✅ partial (1 of 14 shipped) | Phase 1 |
| `context-graph-tier-compression` | ✅ shipped | Phase 6 |
| `context-graph-storage` | ✅ in production | RocksDB CFs for panels + checkpoints + observe state |
| `context-graph-mcp` | ✅ in production | MCP entry point for ME-JEPA tools |
| `context-graph-embeddings` | ✅ in production | E1/E5/E7/etc. that some instruments wrap |

**Queued for removal** (LLM-coupled, no longer in scope):

- `crates/context-graph-cli/src/bin/reality-loop.rs` (Qwen GGUF host)
- `crates/context-graph-causal-agent/src/llm/mod.rs` (Hermes GGUF host)
- `vendor/llama-cpp-sys-2-0.1.145/` (only consumer is the above two)
- `[patch.crates-io] llama-cpp-sys-2` entry in workspace `Cargo.toml`
- All `qwen3-coder` / `hermes-2-pro` references in documentation, configs, model paths

These are not mass-deleted in this turn (operator authorized cleanup of *data*, not source-code refactor). A follow-up turn should retire them.

---

## What an outer-loop turn looks like, post-pivot

Per CLAUDE.md §11 (rewritten), every Claude Code session in this repo is an outer-loop turn doing one of:

- **Implementation work** on ME-JEPA — building instruments, the predictor, training loop, etc. Each turn is one or more shipped primitives with FSV evidence.
- **Maintenance work** — updating docs, fixing tests, migrating old artifact formats.
- **Self-validation** — running `cargo test` / `cargo build --release` / `cargo clippy` and recording the deltas.

**No more `mcp__cgreality__reality_run_attempt` drips against SWE-bench tasks.** The /loop cron's role shifts from "drip an inner-actor attempt and record the optimizer recommendation" to "perform one increment of ME-JEPA implementation work and record the recommendation." The recommendation schema stays compatible (doc 04, doc 19 still apply); the contents shift toward "shipped X primitive of ME-JEPA" rather than "tweaked Y harness for the inner actor."

The Stop hook contract — every turn must call `optimizer_record_recommendation` — stays. The contents of the recommendation now describe ME-JEPA implementation deltas, not harness tweaks for an inner LLM.

---

## Migration checklist for existing artifacts

| Artifact | What to do |
|---|---|
| `/tmp/contextgraph-rust-reality-loop/` | DELETED 2026-05-09 (no longer used) |
| `models/qwen3-coder-30b-a3b/` | DELETED 2026-05-09 |
| `models/hermes-2-pro/` | DELETED 2026-05-09 |
| `tasks/swebench-lite/*.json` | KEEP — these are still the corpus task list for Phase 0 mutation generation. The "champion attempt" framing is dropped; manifests now define training-corpus tasks. |
| `memory/decisions/agent-77-...inner-actor-model-swap...md` | KEEP, mark `status: superseded`. Don't delete (SMAP rule 8: "Resolved blockers stay; superseded decisions stay"). |
| `memory/blockers/agent-72-coordinator--docker-permission-denied-blocks-official-eval.md` | KEEP at `status: resolved`. Docker oracle is still needed for Phase 0 corpus generation, just not for inner-actor live attempts. |
| `crates/context-graph-cli/src/bin/reality-loop.rs` | KEEP for now; queued for retirement. |
| `vendor/llama-cpp-sys-2-0.1.145/` | KEEP for now; queued for retirement after `reality-loop.rs` retires. |
| Doc 09 §3.2 ("live reality-loop attempts" as secondary corpus) | KEEP, but re-frame: live attempts come from Claude Code's outer-loop work, not from a separate inner LLM. |
| Doc 09 §3.6 #5 ("Inner-actor reasoning trace") | KEEP, but rename to "AI agent reasoning trace" — Claude Code's CoT (or any agent's CoT) feeds `E_Reasoning`. |

---

## Operator's immediate priorities (per 2026-05-09 directive)

1. **Build the ME-JEPA predictor that takes code + AST + context and predicts whether the code will work in reality.** This is the system's reason for existing.
2. **Use the existing AST / instrument infrastructure** as the input pipeline. Doc 09's 14-instrument design is the architecture; Phase 1 is the implementation.
3. **Train against the Phase 0 mutation corpus** as the labeled-data backbone.
4. **Make `MeJepaCompiler::verify(patch)` callable from MCP** so any agent or human can ask "does this patch work in reality?" and get a calibrated answer.

Per doc 09 §1.3, the **end state** is when `predicted_oracle_pass > 0.95` with `confidence.outcome_set` size ≤ 2 and `out_of_distribution_score < 0.5` — at which point human code review of AI patches is statistically unjustified. That's the "no code review needed" North Star.

---

## See also

- Master plan: `09_END_GOAL_REVIEW_REPLACEMENT.md` (still authoritative; sections that mention Qwen/inner-actor are re-framed by this doc)
- Operator contract: `CLAUDE.md §11` (rewritten in the same commit as this doc)
- Sitrep: `sitrep/01_status_overview.md`, `sitrep/02_shipped/*`, `sitrep/03_deferred/*`, `sitrep/04_implementation_plans/*`
- Disk-cleanup record: this file's "Migration checklist" section is the authoritative log of what was deleted.

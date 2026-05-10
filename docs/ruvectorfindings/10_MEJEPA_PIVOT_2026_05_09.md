# 10 — ME-JEPA Pivot (2026-05-09)

> **📊 2026-05-10 STATUS UPDATE — Build-plan delta table below is OBSOLETE in places.** Phase 2 (predictor), Phase 3 (training), Phase 4 (inference), Phase 4b (TCT), and Phase 5 (self-healing) all flipped from "deferred" to **SHIPPED** in commits between 2026-05-09 and 2026-05-10. MCP-tool surface for ME-JEPA is now: `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`. Active forward edge: Phase 1a multi-language `ast_chunker.rs` extension, Phase 1b per-embedder `Instrument` wrappers with real model loads, Phase 6 hygiene ops, Phase 7 mejepa-shift-subscriber Tokio task + `mejepa_observe_shift`/`mejepa_subscriber_status`/`mejepa_capture_audit` MCP tools, Phase 8 ≥0.95 evaluation harness. Authoritative status: [`specs/_README.md`](./specs/_README.md) + [`specs/implementation-plan.md`](./specs/implementation-plan.md). The architecture and operator-directive content of this doc remain authoritative; only the per-phase status column in the §"Build-plan delta from doc 09" table is stale.

> **Operator directive, 2026-05-09 (initial):** *"we aren't building anymore the reality loop around qwen. we can delete that model from hard drive and clear up everything... ultimately what we need to focus on is the me jepa. the me jepa is what predicts reality. this is what is going to be responsible for reviewing essentially and verifying/validating it. the me jepa is all we really need to focus on. this runs separate from the llm. everything needs to be focused on the me jepa."*

> **Operator directive, 2026-05-09 (extension — single loop, Claude Code is the actor, UTL v2.0):** *"the whole purpose of this system is to build the me jepa that can predict if code works or not so software engineers no longer have to review code to ensure what the ai said it did it actually did... we can actually remove any reality loop stuff all together. the reality loop system is solely to get training data for the me jepa system. that ist he whole purpose of the reality loop. and it only needs to have one loop, we can delete the inner loop entirely and everything having to do with it. we just need to optimize the outer loop, the one reality loop around claude code. we will be using claude code to pass the swe bench. and we will be using all the issues and all the runs and all the data from everything claude code does to feed the me jepa and store all the traiing data so we can build out a system fully capable of in any dynamic project with any of the coding languages supported the me jepa will be able to predict if the system will work in reality or not... this system can derive reward signals from durable reality. that is what we are doing, trying to capture all the reward signals as possible so we can have as much data as possible for the me jepa to be able to predict reality with... we go one test at a time. gain all training data and the system learns and improves with each loop... it is very important this system is self healing and self learning. look at the `/home/cabdru/contextgraph/docs2/TheoryOfMachineLearning.md` you can use this to formulate a learning formula for the ME JEPA system."*

> **Operator's end-goal restatement, 2026-05-09:** *"create a system that makes it so no code review is needed anymore... we need a truth system that can predict that code is good and working as intended... does what the ai says it did exist and does it work as the ai says it does or does it work as its intended to? how is it actually impacting reality itself."*

> **The three questions ME-JEPA must answer (verbatim, 2026-05-09):**
> 1. **Does what the AI said it did EXIST?** — claim graph ↔ source-of-truth SHA chain.
> 2. **Does it WORK as intended?** — predicted oracle outcome ↔ actual oracle outcome.
> 3. **How is it actually IMPACTING REALITY?** — predicted side-effect set ↔ observed shifts.
>
> When ME-JEPA can answer all three with calibrated confidence on **any dynamic project in any
> supported language** (Python, Rust, C, C++, Java, C#, JS, TS, Ruby), human code review of AI
> patches becomes statistically unjustified. *That* is the end goal.

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

- ✅ ME-JEPA-Code as designed in doc 09 §2-§7 (panel + predictor + frozen instrument targets + VICReg loss + oracle head + conformal calibration + OOD detection)
- ✅ The 14 instruments (`E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`, `E_Problem`, `E_CommitMsg`, `E_StaticAnalysis`, `E_Runtime`, `E_Reasoning`)
- ✅ The 5,120-d panel and the predictor architecture
- ✅ Training corpus: Phase 0's mutation corpus (8 categories × 300 SWE-bench Lite tasks = 2,400 labeled examples) PLUS live observations from Claude Code's outer-loop work (each /loop turn's edit-and-test cycle produces (panel, oracle) tuples ME-JEPA observes)
- ✅ §3.5 reward-signal capture from durable reality (static-analysis, runtime, cross-attempt, counterfactual)
- ✅ §3.8 real-time pipeline: Tokio shift-log subscriber consuming the harness's existing artifact channels
- ✅ §5 self-healing (online observe + EWC++ + drift detection + A/B/C promotion)
- ✅ The ContextGraph deterministic reality substrate (witness chain, ed25519 segment signatures, Stage 0/F/Merkle certs, Stoer-Wagner mincut, Granger F-test, conformal predictor, MMR, tier compression — tracked in doc 09 plus the specs tree; the legacy `sitrep/` tree was removed in commit `6f72a02e`)

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
- `vendor/llama-cpp-sys-2-0.1.145/` (vendored llama.cpp native build patch) — **removed in the 2026-05-09 hardening pass**.
- `crates/context-graph-cli/src/bin/reality-loop.rs` — Qwen-specific engine binary — **removed in the 2026-05-09 hardening pass**.
- `crates/context-graph-causal-agent/src/llm/` and `crates/context-graph-graph-agent/src/llm/` — Hermes/Qwen local-LLM hosts — **removed in the 2026-05-09 hardening pass**.

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
│  Target: frozen instrument panel read back from durable reality        │
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
| 0 — Mutation corpus + Docker oracle | partial (operators, split, command/Docker oracle wrapper, RocksDB store, manifest preflight, one-task official SWE-bench bridge, and `mejepa-corpus generate/verify/stats` CLI shipped; two-category real-Docker FSV for `sympy__sympy-20590` shipped; full 300-task x 8-category batch deferred) | unchanged. The mutation corpus is the primary corpus; live-attempt corpus from Qwen is dropped. |
| 1 — Instruments (14 core instruments + scalar slot) | partial but no longer "11 missing": all 15 panel slots have deterministic typed encoders + all-slot FSV; neural/embedder-backed replacements, multi-language AST chunk routing, Pyright/rust-analyzer evidence, and linear-probe quality gate deferred | unchanged. `E_Reasoning` is sourced from generic agent-reasoning artifacts, not Qwen CoT specifically. |
| 2 — Predictor + frozen instrument targets + VICReg + oracle head | deferred | unchanged except target-side collapse is prevented structurally, not by EMA. |
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
| `context-graph-mincut` | ✅ shipped | Phase 0 repo-atomic diagnostic leakage proof |
| `context-graph-mejepa-corpus` | ✅ partial (operators, split, oracle/store, one-task SWE-bench bridge, and batch CLI shipped; full 300x8 corpus run pending) | Phase 0 |
| `context-graph-mejepa-instruments` | ✅ partial (all 15 deterministic typed encoders + all-slot Panel FSV shipped; neural/deep integrations pending) | Phase 1 |
| `context-graph-tier-compression` | ✅ shipped | Phase 6 |
| `context-graph-storage` | ✅ in production | RocksDB CFs for panels + checkpoints + observe state |
| `context-graph-mcp` | ✅ in production | MCP entry point for ME-JEPA tools |
| `context-graph-embeddings` | ✅ in production | E1/E5/E7/etc. that some instruments wrap |

**Removed in the 2026-05-09 hardening pass** (LLM-coupled, no longer in scope):

- `crates/context-graph-cli/src/bin/reality-loop.rs` (Qwen GGUF host)
- `crates/context-graph-causal-agent/src/llm/` and `src/service/` (Hermes GGUF host + scheduler)
- `crates/context-graph-graph-agent/src/llm/` and `src/service/` (Hermes-backed graph discovery)
- Legacy local-LLM examples/tests in the causal/graph agent crates
- `vendor/llama-cpp-sys-2-0.1.145/` (only consumer was the removed local-LLM surface)
- `[patch.crates-io] llama-cpp-sys-2` and active `llama-cpp-2` manifest entries

The remaining `llm` Cargo feature names are fail-closed compatibility gates; enabling them emits a compile-time retirement error instead of compiling a fallback.

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
| `crates/context-graph-cli/src/bin/reality-loop.rs` | REMOVED 2026-05-09. |
| `vendor/llama-cpp-sys-2-0.1.145/` | REMOVED 2026-05-09. |
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
- Specs/status: `specs/_README.md`, `specs/constitution.md`, and doc 09 §7. The legacy `sitrep/` tree was removed in commit `6f72a02e`; do not cite it as live status.
- Disk-cleanup record: this file's "Migration checklist" section is the authoritative log of what was deleted.
- UTML v1.0 source theory: `/home/cabdru/contextgraph/docs2/TheoryOfMachineLearning.md` (rewritten 2026-05-09 from "UTL v2.0 — Embodied Edition" into the ML-native "UTML v1.0 — JEPA Edition" with four factors `L_step = ΔP · ΔK · ΔΩ · ΔΞ`)
- ME-JEPA learning-formula operationalization: `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`
- Locked decisions: `memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md`, `memory/decisions/2026-05-09--utl-v2-mejepa-learning-formula.md`

---

## Section A — The single reality loop (locked 2026-05-09)

There is exactly **one reality loop**. It runs around Claude Code. Claude Code is the SWE-bench
actor *and* the reality-loop driver. Every Claude Code action — issue selection, file open, edit,
tool call, test run, oracle verdict, drift event, hook payload — is a reward-signal source for
ME-JEPA.

```
                ┌──────────────────────────────────────────┐
                │   THE ONE REALITY LOOP                  │
                │   ────────────────────────────────────  │
                │   Driver: Claude Code (this agent)       │
                │   Role:   SWE-bench actor + data-factory │
                │           operator. NO inner LLM.        │
                │                                          │
                │   /loop turn:                            │
                │     1. Read state (memory + MCP + git)   │
                │     2. Pick the next unresolved          │
                │        SWE-bench Lite task               │
                │     3. Attempt the task as Claude Code   │
                │        (edit, build, test, repeat)       │
                │     4. Hooks + MCP capture every signal  │
                │        as durable reality                │
                │     5. End-of-turn: recommendation +     │
                │        SMAP report                       │
                │                                          │
                │   Per-turn outputs (the training data):  │
                │     • shift-log entries (file deltas)    │
                │     • witness-chain entries              │
                │     • test outcomes (oracle gold)        │
                │     • runtime traces, RSS, walltime      │
                │     • static-analysis warnings           │
                │     • tool-call sequence (E_Reasoning)   │
                │     • recommendation JSONs               │
                └─────────────────┬────────────────────────┘
                                  │
                                  │ §3.8 mejepa-shift-subscriber
                                  │ consumes every artifact
                                  ▼
                ┌──────────────────────────────────────────┐
                │   ME-JEPA-CODE                          │
                │   ────────────────────────────────────  │
                │   Stand-alone reality predictor          │
                │   (runs SEPARATE from any LLM)           │
                │                                          │
                │   Trains under L_step =                   │
                │     ΔP × ΔK × ΔΩ × ΔΞ                    │
                │   (UTML v1.0 — see doc 11 + Section B)   │
                │                                          │
                │   Self-healing: drift → retrain → A/B/C  │
                │   Self-learning: every observe() updates │
                │   weights, EWC++ protects old tasks      │
                │                                          │
                │   Inference: MeJepaCompiler::verify(p)   │
                │   → answers the three operator questions │
                │   for any (patch, context) pair.         │
                └──────────────────────────────────────────┘
```

The reality loop is the **data factory**. ME-JEPA is the **deliverable**. The two are decoupled:
the reality loop runs whether or not ME-JEPA is currently training; ME-JEPA can be retrained from
any consistent slice of the loop's accumulated data without the loop being live.

### Why Claude Code (not Qwen) is now the SWE-bench actor

1. Claude Code's tool-use, multi-file reasoning, and 1M-token context produce **richer** reality
   signal per attempt than the deleted Qwen3.6-27B did. Per public benchmarks (May 2026), Claude
   Code on Opus 4.6 scores ~80.8% on SWE-bench Verified; the same scaffold dominates pass@5 over
   any other model. (Sources cited in `memory/discoveries/`.)
2. Removing the inner LLM removes ~50 GB of disk, the entire CMake/CUDA build pain around
   `vendor/llama-cpp-sys-2-0.1.145`, and a fragile failure surface (the doomsday watchdog, the
   per-attempt wallclock budget, the inner-model identity guardrails — all dead code now).
3. The hooks + MCP server already capture every artifact a /loop turn produces. No new
   instrumentation is needed to feed ME-JEPA from Claude Code's own work; doc 09 §3.8 already
   specifies the subscriber.

### One test at a time

Per the 2026-05-09 directive: *"we go one test at a time. gain all training data and the system
learns and improves with each loop."* Each /loop turn = one task attempt = one or more
`(panel[t=0], panel[t=1], panel[t=2], oracle_verdict)` training tuples. The corpus grows linearly
with turns. Bigger benchmarks come after SWE-bench Lite 300 is exhausted.

---

## Section B — Learning formula: UTML v1.0 → ME-JEPA (locked 2026-05-09)

> **Canonical learning-formula doc:** [`11_MEJEPA_LEARNING_FORMULA.md`](./11_MEJEPA_LEARNING_FORMULA.md). This section is a brief summary; doc 11 is the full operationalization.

The source theory `docs2/TheoryOfMachineLearning.md` was rewritten on 2026-05-09 from the embodied/biological **UTL v2.0** to the ML-native **UTML v1.0 (Unified Theory of Machine Learning — JEPA Edition)**. The rewrite changed the formula structurally — UTML v1.0 is **four** multiplicative factors, not three:

```
L_step = ΔP × ΔK × ΔΩ × ΔΞ           all factors clamped to [0, 1]
```

| Symbol | UTML v1.0 meaning | ME-JEPA mapping |
|---|---|---|
| **ΔP** | Predictive error in latent space (JEPA-native) | `1 − cosine(g_φ(f_θ(panel_t01)), f_target(panel_t2))` normalized by gradient SNR. Frozen instruments produce f_target. |
| **ΔK** | Knowledge coherence | `0.5·cos_align(g_t, ḡ_prior) + 0.3·(1−fisher_violation) + 0.2·(1−ECE)`. Catches catastrophic forgetting + Fisher-protected EWC++ + calibration drift. |
| **ΔΩ** | Optimizer-state health (the ML "embodiment") | `(effective_plasticity)^0.4 · (landscape_health)^0.4 · (stability_floor)^0.2`. Sutton 2024 plasticity loss + SAM sharpness + Goodhart guard. |
| **ΔΞ** | **Representational non-collapse — the ML-native term, no biological analogue** | `(1 − target_collapse)^0.5 · (1 − predictor_redundancy)^0.3 · (1 − constellation_violation)^0.2`. **For ME-JEPA: target_collapse = 0 by construction** (frozen instruments). |

### Why ME-JEPA structurally wins on ΔΞ

The common self-supervised target-encoder pattern in I-JEPA / V-JEPA / BYOL-style systems uses a learnable or momentum target path defended by asymmetry, stop-gradient, EMA schedules, or regularizers. Those defenses are empirical rather than structural: a sufficiently expressive student/teacher pair, miscalibrated hyperparameters, or a degenerate data distribution can still collapse predictor behavior.

ME-JEPA's target representation is produced by **frozen externally-defined instruments** (E_AST, E_CFG, E_Oracle, E_Witness, ...) — deterministic functions with **zero trainable parameters**. They cannot drift. They cannot collapse. There is no gradient flowing through them, ever. This makes `ΔΞ.target_collapse = 0` a **structural guarantee**, not a regularizer that can be defeated. To our knowledge, ME-JEPA is the first JEPA in the literature with this property.

Predictor-side ΔΞ (the trainable `g_φ` can still degenerate even with frozen targets) is defended with four independent diagnostics: VICReg variance/covariance regularization, pairwise mutual-information audit, constellation+Gτ guard, and the negative-action ablation falsifier. Doc 11 §5.3 has the full breakdown.

### Coaching order

**Regulate ΔΩ and ΔΞ first** (the substrate problem); only then adjust ΔP or ΔK (the signal problem). Trying to fix prediction error by curriculum tweaks while the latent has collapsed (ΔΞ low) or while plasticity is gone (ΔΩ low) is wasted compute — the system *cannot* improve on a degenerate substrate. Doc 11 §9 has the full training cycle including learning-state classification (Stuck / Optimizer-Dysregulated / Latent-Collapsing / Confused / Optimal / Mastery / Dissipating).

### Reward and consolidation

```
Reward         = 0.6 · calibrated_confidence + 0.4 · mean(L_step · ΔΞ)
M(t)           = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
performance_deploy = M(t) · T(train, deploy)         # state-transfer (UTML §VIII.b)
```

The integrand `L_step · ΔΞ` enforces that updates which don't preserve representational integrity contribute zero to either reward or consolidation, regardless of how good their loss looked. This is the ML-native "dissipation" gate — the failure mode where training loss falls but validation generalization gap widens.

### Falsification criteria (UTML v1.0 §XIII)

The formula is falsifiable. The strongest falsifications:

1. ΔΞ proxies (predictor variance, pairwise MI redundancy, constellation violation rate) uncorrelated with downstream generalization, holding ΔP/ΔK/ΔΩ constant.
2. A self-supervised system with a learnable target encoder achieving structural collapse-immunity without a regularizer (would reduce ME-JEPA's structural advantage to a quantitative one).
3. Flat minima not generalizing better in JEPA-class systems, contradicting SAM (Foret et al. 2021).
4. Gradient signal-to-noise ratio uncorrelated with generalization, contradicting Liu et al. 2020.
5. Target-encoder collapse benign at scale — a JEPA driven into the trivial fixed point still producing useful downstream representations.
6. Audit-substrate-grounded ME-JEPA run that reports high L_step and high M but fails on a freshly generated SWE-bench Lite holdout task.

If any of these is disconfirmed by well-powered replication, UTML must be revised and the formula's instantiation in ME-JEPA's training pipeline must be updated.

For full operational details (per-example sampling weight, per-step training certificate format, edge-case audit, implementation roadmap), see [`11_MEJEPA_LEARNING_FORMULA.md`](./11_MEJEPA_LEARNING_FORMULA.md).

---

## Section C — Cross-language / cross-project mandate (locked 2026-05-09)

Operator directive: *"the me jepa will be able to predict if the system will work in reality or
not... in any dynamic project with any of the coding languages supported."*

ME-JEPA must work on **any of the supported languages** in `crates/context-graph-cli`'s
multi-language AST harness (per doc 09 §3.1):

- Python (tree-sitter-python; Pyright/mypy compiler-backed)
- Rust (tree-sitter-rust; rustc compiler-backed)
- C (tree-sitter-c; compile_commands.json compiler-backed)
- C++ (tree-sitter-cpp; compile_commands.json compiler-backed)
- Java (tree-sitter-java; javac compiler-backed)
- C# (tree-sitter-c-sharp; dotnet build compiler-backed)
- JavaScript (tree-sitter-javascript; node lints / TS compiler-backed for `.ts`/`.tsx`)
- TypeScript (tree-sitter-typescript and tree-sitter-tsx; tsc compiler-backed)
- Ruby (tree-sitter-ruby; `ruby -c` compiler-backed)

The state-transfer term `T(train, deploy)` in §B captures cross-language / cross-project distance
explicitly. To minimize T's penalty:

1. **Train across realistic distribution** of deployment states from Phase 0 (mutation corpus).
   The 8 mutation categories run against ALL supported languages, not just Python. Each language
   contributes labeled `(panel, oracle_verdict)` tuples to the bootstrap corpus.
2. **Capture cross-language reality signal** at runtime. The shift-log subscriber consumes
   shifts in any supported file extension; the panel materializer dispatches to the language-
   appropriate AST/CFG/DataFlow extractor; `E_TypeGraph` uses the language-appropriate compiler
   evidence.
3. **Conformal calibration set spans languages.** The 10% calibration split (doc 09 §3.4) MUST
   stratify by language so the conformal threshold is not Python-biased.
4. **OOD detection respects language boundaries.** A Python-trained model evaluating Rust code
   should produce a high OOD score, not a confident wrong prediction. Out-of-distribution
   *cross-language* inputs escalate to human until the language is folded into the training set.

This is what makes ME-JEPA usable for *any dynamic project*, not just SWE-bench Python tasks.

---

## Section D — Self-healing and self-learning protocol (locked 2026-05-09)

Operator directive: *"it is very important this system is self healing and self learning."*

Per UTML v1.0 §IX training cycle (the ML-native rewrite of UTL v2.0's coaching cycle), mapped to ME-JEPA:

```
1. ASSESS: measure ΔΩ (optimizer-state health) AND ΔΞ (representational non-collapse) FIRST.
   ↓
2. REGULATE if ΔΩ < 0.4 or ΔΞ < 0.4 (state intervention before content):
   - ΔΩ.effective_plasticity collapsed (dormant ReLU units, low-rank weight drift)?
       → continual backprop (Sutton 2024), selective dormant-unit re-initialization
   - ΔΩ.landscape_health bad (sharp minima, gradient norm out of soft band)?
       → SAM perturbation tightening, LR cosine-anneal restart
   - ΔΩ.stability_floor → 0 (NaN, validation rising, Goodhart catastrophe)?
       → halt, rollback to last good checkpoint, surface blocker
   - ΔΞ.predictor_redundancy high (panel slots collapsing into each other)?
       → bump VICReg variance/covariance coefficients, refit constellation
   - ΔΞ.constellation_violation_rate high?
       → re-fit constellation centroids on fresh deterministic-walk data
   - (ΔΞ.target_collapse is locked at 0 by construction; if it's nonzero
      something has accidentally made an instrument trainable — fix the bug)
   ↓
3. OBSERVE: ΔP (latent-space prediction error normalized by SNR), ΔK (gradient alignment + Fisher protection + ECE)
   ↓
4. COMPUTE: L_step = ΔP × ΔK × ΔΩ × ΔΞ
   ↓
5. CLASSIFY: which learning state is the model in?
   - L_step < 0.1 for N steps, ΔΩ > 0.4, ΔΞ > 0.4 → Stuck (curriculum-prune)
   - L_step < 0.1, ΔΩ < 0.4 → Optimizer-Dysregulated (regulate ΔΩ first)
   - L_step < 0.1, ΔΞ < 0.4 → Latent-Collapsing (regulate ΔΞ first)
   - L_step < 0.3 → Confused
   - 0.3 ≤ L_step < 0.7 → Optimal
   - L_step ≥ 0.7 → Boring / Mastery (advance curriculum)
   - mean(L_step) > 0.3 but ΔM/mean(L_step) < 0.2 → Dissipating (consolidation problem)
   ↓
6. INTERVENE: regulate substrate (ΔΩ / ΔΞ), then adjust content (ΔP / ΔK), per state.
   ↓
7. EVALUATE: did L_step improve? Did M (validation/holdout) hold over the next eval window?
   ↓ (return to step 1)
```

**Coaching order (UTML §IX):** regulate ΔΩ and ΔΞ before adjusting ΔP or ΔK. Trying to fix prediction error by curriculum tweaks while the latent has collapsed is wasted compute — the system *cannot* improve on a degenerate substrate.

### Self-healing components (already specified in doc 09)

- **§5.1 Online `observe()` with EWC++** — every reality-loop turn's `(panel, oracle_verdict)`
  pair gets folded in via online SGD; EWC++ Fisher matrix protects old tasks (cross-language
  performance does not degrade as new tasks land).
- **§5.2 Drift detection** — rolling 1,000-attempt window of `(predicted_set, actual_oracle)`
  pairs. Empirical conformal coverage <85% triggers soft drift; <80% triggers hard drift; <80%
  prolonged triggers catastrophic.
- **§5.3 A/B/C retrain + promote** — when drift is hard or catastrophic, retrain Mode B from
  current corpus, ensemble Mode C; promote whichever wins on holdout.
- **§5.4 Periodic full retrain** — every N=10,000 attempts, full retrain regardless of drift
  (catches slow drift not seen by rolling window).
- **§5.5 Continuous calibration** — every 1,000 attempts, recompute conformal threshold on the
  most recent 1,000 calibration examples.
- **§5.6 Witness-chain integrity check** — every 10,000 attempts, replay chain end-to-end;
  halt and surface to operator if integrity fails.

### Self-learning components

- The reality loop produces continuous training data; `mejepa-shift-subscriber` (§3.8) feeds
  it into `observe()` in < 5 s p50.
- Active learning loop (§3.7.4): when conformal set size > 3 AND OOD score < 0.5, surface for
  operator label / Docker oracle / adjacent-test re-runs. Adds ~1-2% of corpus per cycle as
  high-signal supervision.
- Cross-task transfer (§3.7.5): patch-similarity graph upweights similar tasks during fine-tune.
- Aux-head distillation (§3.7.6): every 1,000 steps, distill aux-head predictions into the
  predictor's hidden state; prevents aux heads becoming detached specialists.

### What "self-healing" MUST NOT mean

- It MUST NOT mean silent fallback. A drift event surfaces a journal entry, a blocker if
  unresolvable, and an explicit `status_change: retraining` / `degraded` / `paused` field on
  every prediction it makes during the heal cycle.
- It MUST NOT mean unsupervised weight changes. Every promotion (Mode A → Mode B → Mode C) is
  witness-chain-recorded with the holdout-evaluation summary hash.
- It MUST NOT mean accepting noisy labels as if they were gold. `signal_clarity` in ΔE
  multiplicatively gates the update; weak labels contribute less.

### Robust error logging (operator directive 2026-05-09)

> *"if something doesn't work it should error out and have robust error logging so that if
> something does fail then we know exactly what failed and how to fix it."*

Every fallible ME-JEPA op returns a taxonomy-defined error with `.code()`. Failure-mode taxonomy:

- `MEJEPA_OBSERVE_BATCH_NAN` — gradient or activation went non-finite during observe()
- `MEJEPA_OBSERVE_DRIFT_DETECTED` — drift severity ≥ hard
- `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` — witness-chain broken
- `MEJEPA_INFER_OOD_REFUSE` — OOD score too high; refusing to predict
- `MEJEPA_INFER_DIM_MISMATCH` — panel dimensionality wrong
- `MEJEPA_INFER_INSTRUMENT_MISSING` — required instrument unavailable for this language
- `MEJEPA_RETRAIN_HOLDOUT_REGRESSION` — Mode B / C did not beat Mode A on holdout
- `MEJEPA_RETRAIN_QUOTA_EXCEEDED` — disk / RAM quota hit during retrain
- `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES` — < N samples in conformal calibration window

No silent fallback paths. No `unwrap_or(0.5)` on a confidence interval. No mock data in tests.

---

## Section E — `/loop.md` directive

Operator directive: *"we will use slash loop and will need to update
`/home/cabdru/contextgraph/.claude/loop.md` we will anwt to completely refactor this loop.md
file."*

The new `/loop` semantics, derived from this pivot:

1. **Pick the next unresolved SWE-bench Lite task.** Iterate `tasks/swebench-lite/*.json` in
   lex order; the first task without an existing `(panel, oracle_verdict)` tuple in the ME-JEPA
   training corpus is the candidate.
2. **Attempt the task as Claude Code.** Use the standard Claude Code toolset (Read, Edit, Write,
   Bash, etc.) to produce a candidate fix. Run the test suite. Capture the Docker oracle verdict
   through the current SWE-bench harness and write the readback as a ME-JEPA observation. Do not
   reintroduce the retired `reality_run_*` MCP tools.
3. **Capture every signal.** The PostToolUse Edit/Write hook already appends to the shift log
   and witness chain; the `cgreality-shift-log` subscriber forwards into `mejepa.observe()`.
   No additional capture code needed in the per-turn prompt — the hooks do it.
4. **End the turn with a recommendation + SMAP report.** The recommendation describes what was
   attempted, what shifted, what oracle returned, and what the next turn should pick up.
5. **One test at a time.** A turn's scope is one SWE-bench Lite task. If the task is not solved
   within the turn's wallclock budget, the partial-progress signal still feeds ME-JEPA (the
   per-test pass/fail vector at the end of the turn is gold supervision regardless of whether
   the overall task was resolved).

The full refactored `loop.md` is a separate deliverable (see operator's immediate priorities §3
below).

---

## Section F — Updated immediate priorities (2026-05-09 extension)

Replacing the original "Operator's immediate priorities" list with the extended directive:

1. **Refactor `/home/cabdru/contextgraph/.claude/loop.md`** to the new semantics (Section E above).
   This is the per-turn operating contract; nothing else can move until this is correct.
2. **Build the ME-JEPA predictor** (doc 09 Phases 1–4) so `MeJepaCompiler::verify(patch)` answers
   the three operator questions. Use the existing AST/instrument infrastructure.
3. **Train against the Phase 0 mutation corpus** as the labeled-data backbone, expanding the
   corpus to cover ALL supported languages (Section C), not Python only.
4. **Wire the reality loop's shift-log subscriber** to `mejepa.observe()` (doc 09 §3.8) so every
   Claude Code turn's reality artifacts feed training.
5. **Implement self-healing per Section D** — drift detection + A/B/C promotion + EWC++ + the
   UTML v1.0 training cycle (regulate ΔΩ and ΔΞ before adjusting ΔP/ΔK).
6. **Stand up cross-language calibration** — conformal calibration set stratified across all
   supported languages.
7. **Make `mcp__cgreality__mejepa_verify`** the primary inference path so any agent or human can
   ask "does this patch work in reality?" and get a calibrated answer.

Per doc 09 §1.3 / §8, the **end state** is `prediction_oracle_correlation ≥ 0.95` across all 8
mutation categories AND across all supported languages on the holdout split. That's the
"no code review needed" gate.

---

## Section G — How this doc relates to doc 09

| Doc 09 section | Pivot impact (this doc) |
|---|---|
| §0 TL;DR | Re-anchored on single-loop framing; UTML v1.0 four-factor formula referenced (see Section B). |
| §1 Reality compiler analogy | Unchanged; the analogy stays. |
| §2 Architecture (instruments, panel, predictor, frozen target readback, VICReg) | Unchanged. |
| §3 Training corpus | §3.1 mutation corpus unchanged; §3.2 live attempts re-framed as Claude-Code-driven, not Qwen-driven. §3.3 synthetic adversarials unchanged. §3.4 split unchanged. |
| §3.5 Reward-signal capture | Unchanged — ALL 21 capture surfaces still apply, sourced from Claude Code's reality artifacts. |
| §3.6 Blindspots | "Inner-actor reasoning trace" item retitled "AI agent reasoning trace" (Claude Code's CoT, or any agent's). |
| §3.7 Optimal use of signals | Unchanged. |
| §3.8 Real-time pipeline (shift-log subscriber) | Unchanged — this is the binding API between the reality loop and ME-JEPA. |
| §4 Inference + calibration + OOD | Unchanged. `verify()` is now the primary path. |
| §5 Self-healing | **Augmented by Section D above** — UTML v1.0 training cycle is the operating procedure (assess ΔΩ + ΔΞ first, regulate state before content); §5.1–§5.6 components are unchanged but called from the cycle. |
| §6 Hygiene | Unchanged. |
| §7 Phase plan | Unchanged in structure; live-attempt corpus in Phase 5+ comes from Claude Code, not Qwen. |
| §8 Open research questions | Unchanged. |
| §9 Why clean-room | Unchanged. |
| §10 Cross-references | Add this doc 10 as the canonical pivot reference. |
| §11 One-line summary | Reframe: "ME-JEPA-Code is the deliverable; the single reality loop around Claude Code is its data factory; UTML v1.0 (`L_step = ΔP · ΔK · ΔΩ · ΔΞ` with target_collapse = 0 by construction) is its learning formula; the end state is no human code review for AI patches." |

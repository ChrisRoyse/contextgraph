# 10 — ME-JEPA Pivot (2026-05-09)

> **Operator directive, 2026-05-09 (initial):** *"we aren't building anymore the reality loop around qwen. we can delete that model from hard drive and clear up everything... ultimately what we need to focus on is the me jepa. the me jepa is what predicts reality. this is what is going to be responsible for reviewing essentially and verifying/validating it. the me jepa is all we really need to focus on. this runs separate from the llm. everything needs to be focused on the me jepa."*

> **Operator directive, 2026-05-09 (extension — single loop, Claude Code is the actor, UTL v2.0):** *"the whole purpose of this system is to build the me jepa that can predict if code works or not so software engineers no longer have to review code to ensure what the ai said it did it actually did... we can actually remove any reality loop stuff all together. the reality loop system is solely to get training data for the me jepa system. that ist he whole purpose of the reality loop. and it only needs to have one loop, we can delete the inner loop entirely and everything having to do with it. we just need to optimize the outer loop, the one reality loop around claude code. we will be using claude code to pass the swe bench. and we will be using all the issues and all the runs and all the data from everything claude code does to feed the me jepa and store all the traiing data so we can build out a system fully capable of in any dynamic project with any of the coding languages supported the me jepa will be able to predict if the system will work in reality or not... this system can derive reward signals from durable reality. that is what we are doing, trying to capture all the reward signals as possible so we can have as much data as possible for the me jepa to be able to predict reality with... we go one test at a time. gain all training data and the system learns and improves with each loop... it is very important this system is self healing and self learning. look at the `/home/cabdru/contextgraph/docs2/UTL_v2_embodied.md` you can use this to formulate a learning formula for the ME JEPA system."*

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
- UTL v2.0 source: `/home/cabdru/contextgraph/docs2/UTL_v2_embodied.md`
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
                │   Trains under L = ΔS × ΔC × ΔE          │
                │   (UTL v2.0; see Section B below)        │
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

## Section B — Learning formula: UTL v2.0 → ME-JEPA (locked 2026-05-09)

The Unified Theory of Learning v2.0 (`docs2/UTL_v2_embodied.md`) is the **canonical learning
formula for ME-JEPA**. Each peer-reviewed-grounded human-physiology term maps to a measurable
AI/ML signal. The multiplicative gating preserves UTL's "any factor at zero zeroes the whole
product" property — critical for self-healing, because it forces the model to **not learn**
when its own internal state is corrupted.

### The mapped equation

```
L_ME-JEPA(t) = ΔS(t) × ΔC(t) × ΔE(t)         all factors clamped to [0, 1]
```

| Symbol | UTL v2.0 meaning | ME-JEPA mapping |
|---|---|---|
| **L** | Per-moment learning efficiency | Training-signal strength right now: how much this `(panel, oracle_verdict)` pair should move the model |
| **ΔS** | Surprise: prediction error | `\|oracle_predicted − oracle_actual\|` (gold) OR `γ · \|oracle_predicted − oracle_simulated\|` (counterfactual rollouts via panel perturbation; γ ≈ 0.7) |
| **ΔC** | Coherence: integrates with prior knowledge | Prediction stability + gradient direction agreement + **multi-embedder coherence** (E1–E13 cross-space neighbor agreement using the existing embeddings crate) |
| **ΔE** | Embodiment: state permits the update to stick | Plasticity-window × signal-clarity × drift-floor (geometric weighting, any subcomponent at 0 zeros ΔE) |

### Detailed sub-formulas

```
ΔS = min(1.0,
         max(|oracle_predicted − oracle_actual|,
             γ · |oracle_predicted − oracle_simulated|)
         + 0.5 · exploration_rate)
       γ ≈ 0.7 (UTL v2.0 mental-rehearsal coefficient — counterfactual
                  rollouts contribute ~70% as much as gold supervision)

ΔC = min(1.0,
         1/(1 + CV_recent_predictions)
         + 0.3 · gradient_effectiveness
         + 0.2 · embedder_coherence)
       embedder_coherence uses the existing 13-embedder system
       (E1–E13 in crates/context-graph-embeddings/)

ΔE = (plasticity_window)^0.4
     · (signal_clarity)^0.4
     · (drift_floor)^0.2

       plasticity_window:  0.0 frozen | 0.6 standard online | 0.85 cosine+EWC
                          | 1.0 scheduled offline retrain (analog of theta sleep)
       signal_clarity:     1.0 gold oracle | 0.6 operator override | 0.3 heuristic
                          | 0.1 weak/noisy
       drift_floor:        1.0 in-dist | 0.4 moderate drift (trigger A/B/C)
                          | 0.1 severe drift (refuse, full retrain)
```

### Consolidation integral M(t)

```
M_ME-JEPA(t) = ∫₀ᵗ L(τ) · ΔE(τ) · k(τ) · e^(−(t−τ)/τ_decay(τ)) dτ

       k(τ) = 1.0  during online observe() updates (the §3.8 subscriber)
       k(τ) = 3–5× during scheduled offline retrain windows (analog of slow-wave sleep)
```

### State-transfer T (cross-language / cross-project)

```
T(SWE-bench-Lite, target_project) = exp(−d(state_train, state_deploy) / λ)
performance_deploy = M_ME-JEPA(t) · T(SWE-bench-Lite, target_project)

       d = distance over (language, repo_size, test_density,
                          dependency_graph_depth, coverage_pattern)
       λ ≈ 1.5 default. Tighter for high-stakes (production code).
                Looser for prototypes / scratch repos.
```

State-transfer is what makes the cross-language / cross-project mandate (Section C) achievable.
SWE-bench Lite is mostly Python; deployment must work on Rust, C, C++, Java, C#, JS, TS, Ruby
too. Train across the realistic deployment-state distribution from day one — *stress-inoculation
training* in UTL parlance — not Python-only.

### Reward signal (for the data-factory optimizer)

```
Reward = 0.6 · prediction_oracle_correlation + 0.4 · mean(L · ΔE)
```

Claude Code (the data-factory operator) maximizes this reward by improving the capture machinery
(instruments, hooks, shift-log, witness chain) so ME-JEPA gets richer L per attempt at higher ΔE.

### Learning states (UTL v2.0 §XII, mapped)

| Pattern | Learning state | Action |
|---|---|---|
| L < 0.1 for 3+ steps, ΔE > 0.4 | **Stuck** (path problem) | Prune or change architecture |
| L < 0.1 for 3+ steps, ΔE < 0.4 | **Dysregulated** (state problem) | Regulate first: pause online updates, run drift detection, retrain if needed |
| L < 0.3 | **Confused** | Improve coverage; reduce challenge temporarily |
| 0.3 ≤ L < 0.7 | **Optimal** | Continue |
| L ≥ 0.7 | **Mastery / Boring** | Increase challenge (harder mutation categories, harder benchmarks, broader languages) |
| mean(L) > 0.3 but ΔM(24h)/mean(L) < 0.2 | **Dissipating** (consolidation problem) | Increase k(τ): trigger offline retrain window |

**Coaching order (UTL v2.0 §VIII):** regulate ΔE first, then adjust ΔS / ΔC. Increasing the
training challenge while ΔE is low wastes signal.

### Pruning rule

```
prune learning path if (L < 0.1 for 3+ steps) AND (ΔE > 0.4 throughout)
```

If ΔE was low, the diagnosis is **bad model state, not bad data path**. Pruning a path because
the model was dysregulated when it saw it teaches the wrong lesson.

### Confidence calibration (gated by ΔE_mean)

```
calibrated_confidence = raw_conf · prediction_oracle_correlation
                       · gate_agreement · evidence_factor · ΔE_mean
clamped to [0.10, 0.95]
```

Confidence formed in a dysregulated model state is not trusted to the same degree as confidence
formed when the model is healthy.

### Falsification criteria (UTL v2.0 §XI.6 discipline)

This formula is **falsifiable**. Any of these would invalidate it:

1. **Drift-floor proxies (KL divergence, embedding shift) uncorrelated with ME-JEPA prediction
   degradation**, holding ΔS and ΔC constant. → drop the drift_floor term.
2. **Counterfactual / latent-perturbation rollouts producing zero gradient benefit** vs. gold-only
   training in a controlled comparison. → the γ ≈ 0.7 mental-rehearsal coefficient is misapplied.
3. **Capability_target_vector having zero effect** on convergence rate vs. history-only
   prediction. → drop the future-self prior.

If any falsification holds, the corresponding sub-formula gets `status: superseded` in
`memory/decisions/` and a new file describes the corrected version.

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

Per UTL v2.0 §VIII coaching cycle, mapped to ME-JEPA:

```
1. ASSESS STATE: compute current ΔE_model.
   ↓
2. REGULATE if ΔE < 0.4 (state intervention before content):
   - Drift severe (drift_floor < 0.4)?
       → pause inference, trigger A/B/C retrain (doc 09 §5.3)
   - Plasticity collapsed (LR too low or grad explosion)?
       → reset LR schedule
   - Signal clarity too low (mostly weak labels)?
       → reweight batch toward gold-supervised examples
   ↓
3. OBSERVE: ΔS (oracle prediction error), ΔC (multi-embedder agreement)
   ↓
4. COMPUTE: L = ΔS × ΔC × ΔE
   ↓
5. CLASSIFY: which learning state is the model in (Section B table)?
   ↓
6. INTERVENE: adjust either CONTENT (more data, harder mutations) or STATE
   (drift correction, retrain) depending on which signal is low
   ↓
7. EVALUATE: did L improve? Did M (consolidation) hold over the 24h window?
   ↓ (return to step 1)
```

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
   Bash, etc.) to produce a candidate fix. Run the test suite. Capture the oracle verdict via
   `mcp__cgreality__reality_run_official_eval` (Docker oracle).
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
   UTL v2.0 coaching cycle.
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
| §0 TL;DR | Re-anchored on single-loop framing; UTL v2.0 formula referenced. |
| §1 Reality compiler analogy | Unchanged; the analogy stays. |
| §2 Architecture (instruments, panel, predictor, EMA, VICReg) | Unchanged. |
| §3 Training corpus | §3.1 mutation corpus unchanged; §3.2 live attempts re-framed as Claude-Code-driven, not Qwen-driven. §3.3 synthetic adversarials unchanged. §3.4 split unchanged. |
| §3.5 Reward-signal capture | Unchanged — ALL 21 capture surfaces still apply, sourced from Claude Code's reality artifacts. |
| §3.6 Blindspots | "Inner-actor reasoning trace" item retitled "AI agent reasoning trace" (Claude Code's CoT, or any agent's). |
| §3.7 Optimal use of signals | Unchanged. |
| §3.8 Real-time pipeline (shift-log subscriber) | Unchanged — this is the binding API between the reality loop and ME-JEPA. |
| §4 Inference + calibration + OOD | Unchanged. `verify()` is now the primary path. |
| §5 Self-healing | **Augmented by Section D above** — UTL v2.0 coaching cycle is the operating procedure; §5.1–§5.6 components are unchanged but called from the cycle. |
| §6 Hygiene | Unchanged. |
| §7 Phase plan | Unchanged in structure; live-attempt corpus in Phase 5+ comes from Claude Code, not Qwen. |
| §8 Open research questions | Unchanged. |
| §9 Why clean-room | Unchanged. |
| §10 Cross-references | Add this doc 10 as the canonical pivot reference. |
| §11 One-line summary | Reframe: "ME-JEPA-Code is the deliverable; the single reality loop around Claude Code is its data factory; UTL v2.0 (`L = ΔS × ΔC × ΔE`) is its learning formula; the end state is no human code review for AI patches." |

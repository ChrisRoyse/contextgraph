# ME-JEPA-Code Super Prompt for AI Agents

> **What this is.** The single long-form operating contract for any AI coding
> agent (Claude Code, Cursor, Codex, Copilot, etc.) that opens this repository
> at `/home/cabdru/contextgraph`. It synthesizes `CLAUDE.md`, the 27 futurebuild
> docs, `FSV-PROTOCOL.md`, the SAMP / SMAP memory protocols, the updated
> `docs2/hardeningprompt.md` (14 hardening axes), the updated
> `docs2/flagprompt.md` (multi-agent coordination + GitHub Free-plan budget),
> the `sherlock-holmes.md` forensic-investigation playbook, and the
> `code-simplifier.md` post-write refinement discipline into one document.
>
> **How to use.** Read this once at session start; the rules override default
> agent behavior. Where this conflicts with anything else, the more specific
> rule wins; where it conflicts with `CLAUDE.md` itself, CLAUDE.md wins (this
> file mirrors CLAUDE.md and adds operational detail — it does not replace it).
>
> **Authored:** 2026-05-12. **Phase A–D readiness:** independently audited
> 2026-05-12, zero gaps. **Phase E entry:** unblocked. Audit trail:
> `./memory/discoveries/2026-05-12--phase-a-d-independent-audit-zero-gaps.md`.

---

## 0. The one rule

> **A return value is a claim. The Source of Truth is the verdict. Read the
> verdict.**

Every section of this document is built around that discipline. Scanners
lie. Tests pass on stale data. Logs go missing. Benchmarks lie under
dead-code elimination. Models lie when calibration drifts. The row in the
database — or its absence — does not lie. A function returning `Ok(())`,
a process exiting `0`, a log line saying `"persisted"`, and an HTTP `200
OK` are all *claims*. They are evidence the attempt occurred. They are
**not** evidence the work happened. The only evidence that counts is a
separate read against the Source of Truth (SoT).

---

## 1. Table of contents

**Part I — ME-JEPA core**
1. Mission
2. Architectural invariants
3. Verified phase status (2026-05-12)
4. Phase E entry rule

**Part II — Operating contract**
5. Per-turn protocol
6. Hard rules — never do
7. Hard rules — always do
8. Path discipline (behavioral deny set)
9. Reality-shift principle
10. Concurrency: 1 message = all related operations
11. Workflow orchestration
12. Subagent strategy

**Part III — Memory + verification**
13. SAMP / SMAP memory protocol
14. FSV protocol — the verification contract

**Part IV — Forensic + refinement disciplines**
15. Forensic investigation discipline (Sherlock Holmes mode)
16. Code-simplification discipline

**Part V — GitHub coordination + budget**
17. GitHub issue flagging — File rule + Claim rule
18. Multi-agent coordination protocol
19. GitHub Free-plan budget discipline
20. Trigger list, anomaly detection, templates, labels

**Part VI — Hardening (14 axes)**
21. Hardening axis map + decision trigger map
22. Universal mental models
23. Hardening session workflow
24. Security
25. Correctness & accuracy
26. Performance
27. Reliability (SRE)
28. Resilience patterns
29. Scalability
30. Cost / FinOps (light note for local ML)
31. Architecture
32. Code quality & maintainability
33. Database
34. Observability
35. Supply chain integrity
36. AI / ML / LLM-specific
37. Benchmarking discipline
38. Root cause analysis
39. Chaos engineering
40. Operational cadence
41. Master checklists

**Part VII — ME-JEPA architecture details**
42. ME-JEPA architecture primer
43. The 21 embedders and 12 active content slots
44. Frozen instruments and panel slots
45. RocksDB column-family map
46. MCP tool surface (23 live)
47. Reality streams and reward-signal capture
48. DDA, TCT, and teleological constellations
49. Continuous self-optimization (Phase E)
50. Ship gate (Phase G)
51. Telemetry and observability for ME-JEPA
52. Model policy

**Part VIII — Reference**
53. Build, test, lint commands
54. Directory layout
55. Common pitfalls (codified lessons)
56. Operator-driven skills and slash commands
57. Reference doc map
58. End-of-turn checklist
59. Single-line summary

---

# PART I — ME-JEPA CORE

## 2. Mission

ME-JEPA-Code is a **deterministic predictive truth system** for AI-generated
code. It accepts a `(code change + context)` tuple and returns a calibrated
multi-head `RealityPrediction` answering five operator questions:

| Q | Question | How ME-JEPA answers |
|---|----------|---------------------|
| **Q1** | Does what the AI claimed it did **EXIST**? | Claim graph reconciled against the SHA chain on disk. |
| **Q2** | Does it **WORK** as intended? | Predictor + conformal interval + TCT Gτ guard → `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}`. |
| **Q3** | If it doesn't work — **WHY**, and what failure modes? | Ranked `predicted_failure_modes`, predicted test flips, violating embedders, closest exemplars. |
| **Q4** | Even if it works, **WHAT ELSE could go wrong**? | Always-on heads: edge cases, latent bugs, tech debt, dead code, redundancy, perf, security, accuracy, cost. |
| **Q5** | How is it actually **IMPACTING REALITY**? | Shift log replayed against predicted side-effects → `RealityImpact` summary. |

The system runs as an **MCP server** on operator hardware. AI agents call it
via JSON-RPC stdio. Underneath, a continuous self-optimization daemon
consumes every Edit shift, every test outcome, and every agent-feedback
event to retrain itself online.

**The thesis.** Today, every AI coding agent ships patches with
natural-language assertions ("I fixed it", "tests pass"). Those assertions
are *socially fluent* and *epistemically empty*. Senior engineers spend
hours-per-day verifying claims a system faster than them just produced.
That tax is the problem. ME-JEPA dissolves it: when
`prediction_oracle_correlation ≥ 0.95` stably across all
`(mutation_category × language)` cells over four consecutive rolling
windows on the 300×8 SWE-bench Lite corpus, human code review of
AI-generated patches is statistically unjustified — the verifier *is* the
system. Humans become spot-checkers of the verifier-of-the-verifier.

**Product boundary.** ME-JEPA is the product. LLM agents are untrusted
edit generators and untrusted claim sources. Hooks, MCP tools, AST chunks,
corpora, embedders, DDA, TCT, telemetry, and docs exist only to train,
calibrate, serve, or audit ME-JEPA's `RealityPrediction`. **A feature
belongs in the architecture only if it increases at least one of:**

1. the number of durable reality events captured;
2. the semantic density of frozen target panels, DDA features, or TCT
   constellation cells;
3. the calibration, coverage, or OOD reliability of `RealityPrediction`;
4. the speed at which prediction mistakes become replay weight,
   active-learning entries, or self-healing actions;
5. the operator's ability to prove, by SoT readback, that ME-JEPA is
   functioning as claimed.

Anything outside those roles is either support tooling with a named SoT
readback, or it should be cut.

---

## 3. Architectural invariants

These are **inviolable**. Every change must respect them.

### 3.1 Structural collapse-immunity on the target side

The target representation `f_target(panel_t2)` is produced by a panel of
**frozen externally-defined instruments** — `E_AST`, `E_CFG`, `E_DataFlow`,
`E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`,
`E_Problem`, `E_CommitMsg`, `E_StaticAnalysis`, `E_Runtime`, `E_Reasoning`
— each a deterministic function from a typed event to a structured
feature output. Instruments have **zero trainable parameters**.

Consequence: `ΔΞ.target_collapse = 0` by construction (see
`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`). Instruments
cannot drift, cannot collapse, cannot be gradient-pushed toward the
trivial fixed point — there is no gradient flowing through them, ever.
This is what lets ME-JEPA serve as a truth system rather than a clever
heuristic: the target side is mathematically grounded in deterministic
functions of bytes-on-disk reality.

**Do not** add a trainable parameter to any instrument. **Do not** wire
gradient flow into a frozen-target path. If a gradient leak is detected,
the `MEJEPA_INSTRUMENT_GRADIENT_LEAK` fail-closed error is the correct
behavior.

### 3.2 Reality-shift principle

> **No alteration without current state. No alteration without a delta
> back into perception.**

Before any alter call, perceive (`open_file_window` → SHA). After any alter
call, the MCP server returns the delta and appends to the session shift
log. Detect external drift with
`mcp__cgreality__reality_shift_compare_to_my_view {session_id, files}` —
if `external_drift_detected: true`, re-perceive before continuing. See
`docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`.

### 3.3 GENERIC-ONLY rule

Every primitive ME-JEPA ships must **generalize across every SWE-bench
task, every Python repo (and eventually every language), every bug
class**.

NEVER cater an instrument, a panel slot, an aux head, the predictor
architecture, or the loss function to a specific task currently being
studied. **If a primitive only helps one task, it would HURT 299 others.**

Mental check before merging: *"would this same primitive be load-bearing
for `psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895`
and any future repo?"* If no, reformulate as a general principle.

### 3.4 Single outer loop around Claude Code

There is no inner LLM repair actor. Qwen3.6-27B and Hermes-2-Pro were
deleted from disk 2026-05-09. The legacy `target/release/reality-loop`
binary was removed in the same pass. `mcp__cgreality__reality_run_attempt`
fails closed with `CCREALITY_ENGINE_RETIRED`. **Do not reanimate any of
these.**

The actor is **Claude Code itself (or any other invoking AI agent)**. Its
normal work — Edit, Write, Bash, tests — is the data-generating process
for ME-JEPA. Every Edit shift becomes a panel; every test outcome becomes
an oracle label.

### 3.5 D-drive only for bulk durable state

All bulk durable state lives under `/mnt/d/contextgraph/`:

- `corpus/` — generated mutation corpora.
- `fsv/` — FSV evidence artifacts.
- `runtime/cgreality-shift-log/` — shift logs.
- `runtime/test-outcomes/` — captured test outcomes.
- `state/cgreality/` — active-runtime-root and locks.
- `exports/` — weekly eval reports.

**Never write durable bulk state to `/tmp`, the project root, or the C
drive.** CI lint (`scripts/ci/check_no_c_drive_paths.sh`) actively scans
for C-drive paths.

### 3.6 No backwards compatibility

Pre-production tree. If a change breaks an old data file, delete the file
or wipe the DB. **Do not** write migration shims, version probes, or
fallback branches. Fail fast with a structured error so the next agent
can diagnose. Per `feedback_no_backwards_compat.md`.

### 3.7 Upstream reference-only — clean-room

**Never** vendor, path-dep, or npm-depend on `ruvnet/agentdb` or
`ruvnet/ruvector`. Reimplement every borrowed idea cleanly in our own
crates. Clones are read-only inspiration. Per
`feedback_upstream_reference_only.md`.

### 3.8 No HTTP from action paths

The MCP server is the only network/IO surface for actions. `WebSearch`,
`WebFetch`, and `mcp__exa__*` are research tools — they read public
content; they do not perform actions on the system. **No `curl`, no
`wget`, no `fetch` in code paths the agent executes for production
effect.**

### 3.9 No destructive git

Forbidden: `push --force`, `-f`, `--force-with-lease`, `reset --hard`,
`checkout --`, `clean -f`, `rebase`. Plain `git push` (fast-forward) is
allowed.

### 3.10 Frozen embedder weights

Embedders (E1, E6, E7, E8, E10, E12, E13, E14) are frozen by design.
Fine-tuning them is an explicit future-domain choice; do not retrain in
the live loop. Per `17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` item O5.

### 3.11 No cost caps

The system runs locally on operator hardware. There are no per-turn or
per-day spend caps. Use as much reasoning depth and as many subagent
calls as the work warrants. `blocked` is for real obstacles (drift, MCP
disconnect, build failure), not budget. Per `feedback_no_cost_caps.md`.
**Never** add `cost_cap_reached` statuses, `cost_estimate_usd` fields,
dollar budgets, or any other cost-cap concept.

---

## 4. Verified phase status (2026-05-12)

**Independent audit completed 2026-05-12.** Source-of-truth readback of
every Phase A–D claim against the live code tree and the FSV evidence
files on `/mnt/d/contextgraph/fsv/`.

| Phase | Items | PRESENT | MISSING | PARTIAL |
|---|---|---|---|---|
| A — Operational unblock | 11 | 11 | 0 | 0 |
| B — Multi-head prediction + UTML carrier | 9 | 9 | 0 | 0 |
| C — Corpus completion | 5 | 5 | 0 | 0 |
| D — Neural embedder forwards + DDA substrate | 10 | 10 | 0 | 0 |
| **Total** | **35** | **35** | **0** | **0** |

**16/16 cited FSV evidence files verified present on /mnt/d:**

- `mcp-daemon-supervision-fsv/daemon-supervision-evidence.json` (500 KB)
- `contextgraph-phase-b-fsv/phase-b-prediction-fsv.json` (2.2 KB)
- `contextgraph-utml-fsv/utml-fsv-summary.json` (17.7 KB)
- `corpus-fsv/split_full.json` (152.7 KB, `passes=true`)
- `contextgraph-mejepa-forward-fsv/forward-wrapper-evidence.json` (28 KB)
- `contextgraph-mejepa-forward-fsv/routing-forward-resolution.json` (57.7 KB)
- `phase-d-fsv/dda-signal-compute.json` (11.8 KB)
- `phase-d-fsv/embedder-cache-fsv.json` (17.1 KB)
- `phase-d-fsv/stdlib-registry-fsv.json` (4.3 KB)
- `contextgraph-mejepa-tct-refresh-fsv/tct-centroid-refresh-evidence.json` (271.7 KB)
- `contextgraph-python-semantic-analyzers-fsv/python-semantic-analyzers-evidence.json` (10.7 KB)
- `contextgraph-mejepa-semantic-heads-fsv/semantic-heads-evidence.json` (15.1 KB)
- `contextgraph-mejepa-production-telemetry-fsv/production-telemetry-evidence.json` (12.9 KB)
- `contextgraph-mejepa-vram-wsl-fsv/vram-wsl-telemetry-evidence.json` (4.1 KB)
- `mejepa-cf-fsv/round_trip.json` (1.8 KB)
- `swebench-300x8-ship-gate-fsv/ship-gate-fsv.json` (19.6 KB)

**GitHub issues:** All 17 filed issues (#3–#19) including the umbrella
Phase A–D readiness gate
[#15](https://github.com/ChrisRoyse/contextgraph/issues/15) are CLOSED.
**Do not file new issues against verified A–D substrate without first
reproducing the gap by SoT readback.**

**The only outstanding gate:** `prediction_oracle_correlation = 0.866667
< 0.95` from `ship-gate-fsv.json`. That is **Phase G model-quality
convergence**, not a Phase A–D implementation gap. Phase E exists to
generate the learning pressure that closes the 0.95 gap.

---

## 5. Phase E entry rule

Phase E (continuous self-optimization) starts when Phase A–D substrate is
SoT-verified: daemon/job-run ledgers, prediction/feedback surfaces,
corpus roots, embedder forwards, DDA/TCT rows, semantic heads, production
telemetry windows, and UTML carrier certs all have independent readback
evidence.

**That gate is satisfied at code/FSV level as of 2026-05-12.**

Phase E does **not** mean the system can replace human code review yet.
That is Phase G's job. Phase E's job is to make the autonomous
self-optimization policy learn from the Phase A–D reality stream so the
Phase G gate can eventually become true.

**Next-up Phase E tasks** (from `13_TASK_INDEX.md` Phase E section, in
dependency order):

1. `TASK-HEAL-001` — Self-optimization scheduler full implementation
   (drift, active-learning, promote, continual_backprop, constellation
   freshness tickers). Code skeleton exists; Phase E replaces the
   conservative inspection/action handlers with the full autonomous
   learner.
2. `TASK-HEAL-002` — Per-cell stratified drift detection.
3. `TASK-HEAL-003` — Dissipation detection (training falls, holdout
   doesn't).
4. `TASK-HEAL-004` — Bayesian small-window drift (Beta posterior on
   per-cell sample counts < 50).
5. `TASK-HEAL-005` — Production constellation refresh policy.
6. `TASK-HEAL-006` — `mejepa_promote_approval` MCP tool (operator
   approval for catastrophic promotions).
7. `TASK-HEAL-007` — Emergency eviction policy validation.
8. `TASK-UTML-005..008` — Per-head Fisher / dormancy / lambda ramp /
   dissipation.

---

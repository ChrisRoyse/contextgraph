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
6. Per-turn protocol
7. Hard rules — never do
8. Hard rules — always do
9. Path discipline (behavioral deny set)
10. Reality-shift principle
11. Concurrency: 1 message = all related operations
12. Workflow orchestration

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

# PART II — OPERATING CONTRACT

## 6. Per-turn protocol

At the start of every turn, in order:

1. **READ memory.** `./memory/context/`, `./memory/decisions/`,
   `./memory/blockers/` (skip `status: resolved`), most recent
   `./memory/reports/`. Bootstrap the directory tree if missing
   (`mkdir -p ./memory/{context,decisions,discoveries,handoff,journal,blockers,patterns,reports}`).
2. **READ phase docs.** `docs/futurebuild/{00_INDEX.md,
   02_CURRENT_STATE_HONEST.md, 12_IMPLEMENTATION_PHASES.md,
   13_TASK_INDEX.md}` for the current phase and the next-up tasks.
3. **READ GitHub state.** `gh issue list --state open --label "status:in-progress"`
   (claimed work to avoid), `gh issue list --state open --label "source:agent" --search "no:assignee"`
   (your candidate queue). See §17–§18 for full multi-agent protocol.
4. **PICK** the highest-priority incomplete deliverable from the task DAG.
   Or, if specifically directed by the operator, pick that work instead.
5. **CLAIM** the issue (if working from an existing one). Set
   `status:in-progress`, assign self, post a plan comment with files
   you'll touch and ETA. See §18.2.
6. **AUDIT the task.** Per FSV-PROTOCOL §7: verify every file path,
   symbol name, and CF reference in the task against the live tree.
   `ASSUME THE TASK IS WRONG`. If anything is stale, file the audit
   finding and either update the task or escalate. See §15 (Linear
   Sequential Unmasking — examine evidence before claims).
7. **PLAN.** Trigger → Process → Outcome. Identify the SoT before writing
   code. If the SoT cannot be named, the design is broken — fix the
   design before the implementation. List ≥3 boundary cases.
8. **WORK.** Ship one primitive end-to-end with FSV evidence per the
   protocol. Write `./memory/*.md` files IMMEDIATELY when discoveries /
   decisions / blockers / patterns emerge — do not batch.
9. **VERIFY.** Build (`cargo build --release -p <crate>`), test (`cargo
   test -p <crate>`), lint (`cargo clippy -p <crate> --no-deps --tests
   --examples -- -D warnings`), FSV harness write to
   `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json`, independent
   re-open readback for any RocksDB write. See §14.
10. **SIMPLIFY.** Run the code-simplifier discipline (§16) on every
    shipped primitive before merging. Confirm no BLOCKER refinements
    outstanding.
11. **HOLMES PASS.** For non-trivial primitives, run an internal
    forensic-investigation pass (§15) — Cold Read, Contradiction Engine,
    boundary audit. Catch bugs your tests didn't.
12. **REPORT.** Write `./memory/reports/session-YYYY-MM-DD--<slug>.md`
    with the SAMP §3.5 template (see §13.7). Comment milestone+done on
    the GitHub issue (§18.3, §18.7).
13. **OBSERVE & FILE.** Note §20 triggers and §20.5 anomalies you are NOT
    fixing this turn. File before turn ends per §17.

---

## 7. Hard rules — never do

| # | Rule | Reason |
|---|------|--------|
| 1 | Never call HTTP APIs from action paths (curl/wget/fetch). | MCP server is the only action surface. Exa/WebSearch/WebFetch are read-only research tools. |
| 2 | Never edit `.env` or `.env.*` files. | Secrets discipline. |
| 3 | Never edit `swebench/**`, `tests/oracle_**/**`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`. | Editing these destroys forward progress on SWE-bench. |
| 4 | Never use destructive git (`push --force`, `reset --hard`, `checkout --`, `clean -f`, `rebase`). | Pre-production but still — destruction is irreversible. |
| 5 | Never reintroduce the inner-actor LLM (Qwen, Hermes, Ollama, llama.cpp generative inference). | Pivot is intentional. Operator deleted models 2026-05-09. |
| 6 | Never use mock data in tests that gate task completion. | Mock data hides bugs. Real RocksDB temp roots, real shift-log files, real `MeJepaCompiler` instances. |
| 7 | Never write tests that pass when the project is broken. | If commenting out the core implementation leaves the test green, the test is wrong. Mutation-test mentally. |
| 8 | Never weaken or remove a verifier or hook that legitimately blocked you. | Edit the source to fix the underlying issue; or surface the rule to the operator. |
| 9 | Never silently fall back to a default / `None` / empty Vec / placeholder string on a missing artifact/CF/env-var/model. | Fail closed with `SCREAMING_SNAKE_CASE` codes (`MEJEPA_D_DRIVE_UNAVAILABLE`, etc.). |
| 10 | Never add backwards-compat shims (rename-tolerant deserialization, version probes). | Pre-production. Wipe DB on schema change. |
| 11 | Never delete a resolved blocker file. | Update `status: resolved`. History matters. |
| 12 | Never silently overwrite a superseded decision. | Set `status: superseded`, link forward, write the new decision. |
| 13 | Never commit secrets, `.env` files, or credentials. | Run `gitleaks` pre-commit + CI; leak = p0 → revoke now. |
| 14 | Never proactively create documentation files (`*.md`) or READMEs unless explicitly requested. | Per CLAUDE.md §2.4. |
| 15 | Never save working files, ad-hoc tests, or scratch markdown to the project root. | Use `./memory/`, `/tmp/`, or `/mnt/d/contextgraph/`. |
| 16 | Never add cost caps, dollar budgets, `cost_cap_reached` blocked status, or `cost_estimate_usd` fields. | System runs locally; no spend caps. |
| 17 | Never cater a primitive to a specific task. | GENERIC-ONLY rule (§3.3). |
| 18 | Never trust a function's return value as evidence of work. | FSV is the contract — read the SoT (§14). |
| 19 | Never batch documentation at the end of a session. | Insights decay; context compaction can drop them. Write to `./memory/` as you go. |
| 20 | Never edit files referenced by another agent's `status:in-progress` issue without commenting first. | Multi-agent coordination (§18). |
| 21 | Never enable a paid GitHub feature (premium runners, GHAS, Codespaces overage, etc.). | Private repo on GitHub Free plan; $0 budget. See §19. |
| 22 | Never strip labels another agent set without superseding-with-reason. | Coordination integrity. |
| 23 | Never close an issue you didn't claim (except `Duplicate of #N`). | Coordination integrity. |
| 24 | Never batch silent commits on a claimed issue. | Every push touching an issue's files = a comment with SHA + 1-line summary. |
| 25 | Never use `Option<T>::unwrap_or_default()` to paper over a real missing value. | Almost always a bug in this codebase. |
| 26 | Never use `#[ignore]` on a failing test to ship. | Fix the bug or write a `./memory/blockers/` entry. |
| 27 | Never read a PR description / commit message / developer claim BEFORE examining the code. | Linear Sequential Unmasking (§15.4) — examine evidence before claims to avoid confirmation bias. |
| 28 | Never declare GUILTY without considering ≥3 alternative hypotheses. | Self-verification protocol (§15.14). |
| 29 | Never settle on the first plausible explanation in RCA. | Abduction optimizes for narrative coherence, not statistical rigor (§15.3). |
| 30 | Never write nested ternary operators. | Use switch/if-else chains for multiple conditions. Clarity over brevity (§16.4). |

---

## 8. Hard rules — always do

| # | Rule |
|---|------|
| 1 | Read `./memory/context/`, `./memory/decisions/`, `./memory/blockers/` at the start of every turn. |
| 2 | Read the futurebuild status docs (00/02/12/13) and pick the highest-priority next item. |
| 3 | Check GitHub state — claimed/blocked/unclaimed — before starting any work. |
| 4 | Audit every task against the live tree before writing code (FSV-PROTOCOL §7). |
| 5 | Do file reading, synthesis, code edits, and decisions personally. No delegation — own every step. |
| 6 | Run `cargo build --release` for touched crates after every meaningful change. |
| 7 | Run `cargo test` and `cargo clippy --no-deps --tests --examples -- -D warnings` for verification. |
| 8 | Produce FSV evidence at `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json` for every task. |
| 9 | Use `WriteOptions::set_sync(true)` for RocksDB FSV writes; flush + drop + reopen + readback for any persistence claim. |
| 10 | Use `#[serde(deny_unknown_fields)]` on every persisted struct. |
| 11 | Validate input at system boundaries; never trust raw user/MCP/RPC input. |
| 12 | Fail closed with structured `SCREAMING_SNAKE_CASE` error codes on any unknown. |
| 13 | Write decisions / discoveries / blockers / patterns to `./memory/` immediately. |
| 14 | End every turn with a `./memory/reports/session-YYYY-MM-DD--<slug>.md` report. |
| 15 | File a GitHub issue for any defect / smell / dead code / tech debt / anomaly / security / architecture concern / test gap you observe but are NOT fixing this turn (§17). |
| 16 | Claim an issue (assign + label + plan comment) before touching code tied to it (§18.2). |
| 17 | Comment at every milestone on a claimed issue (Discovery / Direction change / Sibling issue / Heartbeat). |
| 18 | Comment a pause-note (Done / Tried-failed / Learned / Resume-at / Hypothesis) before stopping mid-task (§18.4). |
| 19 | Run the code-simplifier discipline (§16) on every shipped primitive before merging. |
| 20 | Update `MEMORY.md` (auto-memory index) with new memory entries; keep entries under 200 chars. |
| 21 | Convert relative dates ("Thursday") to absolute (`2026-03-05`) when saving memory. |
| 22 | Resolve merge conflicts rather than discarding changes; investigate before deleting unfamiliar state. |
| 23 | For UI/frontend work: start the dev server and exercise the feature in a browser before declaring done. |
| 24 | For risky / irreversible / shared-state actions: confirm with the operator first. |
| 25 | Apply Linear Sequential Unmasking (§15.4): examine code/tests BEFORE reading developer claims / PR descriptions. |
| 26 | Generate ≥3 hypotheses before testing any one in RCA (§15.7). |
| 27 | Run the Cold Read protocol (§15.5) on every unfamiliar file before deep-diving. |
| 28 | Run the Contradiction Engine (§15.6) when verifying any claim of behavior. |
| 29 | Document RCA output as a system property ("the system allowed X because Y"), not "human error" (§38.4). |
| 30 | Pin GitHub Actions to SHA (not tag); declare `timeout-minutes` on every workflow; use `runs-on: ubuntu-latest` only (§19). |

---

## 9. Path discipline (behavioral deny set)

The MCP-server-side path-policy gate and the pre-edit hook deny-list were
both **removed 2026-05-07** (operator directive: "the AI needs to be able
to do anything it needs to do"). Path discipline is now **BEHAVIORAL** —
honor it because editing these destroys forward progress, not because a
gate enforces it.

**Don't-touch paths:**

- `.env` and `.env.*` anywhere
- `swebench/**` (the upstream evaluator)
- `tests/oracle_**/**` (benchmark fixtures)
- `crates/context-graph-benchmark/datasets/swe_bench_*/**`
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- Any `*.pem`, `*.key`, `**/credentials.json`

If editing one of these is genuinely the right thing for an
operator-directed task, **ask the operator to confirm first**.

---

## 10. Reality-shift principle — the six injection surfaces

> **No alteration without current state. No alteration without a delta
> back into perception.**

Six surfaces by which reality reaches an agent:

1. **SessionStart hook** — active runtime root, target instance, success
   predicate, reality-boundary rules.
2. **UserPromptSubmit hook** — latest trigger decision, reality-signal
   compact, last 3 shift-log entries — every prompt.
3. **PreToolUse Edit/Write hook** — records current file SHA + line count
   for post-hook delta computation.
4. **PostToolUse Edit/Write hook** — cargo check (for touched Rust files)
   + git diff + harness-transition record + shift-log append.
5. **`mcp__cgreality__*` MCP tools** — query any artifact, query the
   shift log, detect drift, run any allowlisted command, alter the
   harness through governed tools.
6. **Stop hook** — blocks turn end until
   `mcp__cgreality__optimizer_record_recommendation` is called (if the
   reality loop is running).

### 10.1 When to record `no_change`

Recording is mostly retired post-pivot. Use `no_change` only when:

1. All ME-JEPA spec/task priority items shipped AND no maintenance work
   needed AND no operator directive specifies a task.
2. External drift detected mid-turn
   (`reality_shift_compare_to_my_view` reports
   `external_drift_detected: true`). Re-perceive before continuing.
3. Hard build/test failure that blocks all forward progress and isn't
   fixable this turn — write the blocker to `./memory/blockers/` and
   record `blocked` with a remediation plan.

### 10.2 Three consecutive `blocked` for the same root cause → escalate

Write a high-severity blocker AND surface in `remaining_risks: ["requires
human inspection: …"]`.

---

## 11. Concurrency: 1 message = all related operations

- All independent tool calls in a single response **MUST** be made in one
  message.
- Batch ALL todos in **ONE** TaskCreate call (5+ minimum if creating a
  batch).
- Spawn ALL agents in **ONE** message with full instructions via the
  `Agent` tool.
- Batch ALL file reads/writes/edits in **ONE** message.
- Batch ALL Bash commands that don't depend on each other in **ONE**
  message.

When agents have dependencies, sequence them (don't fake parallelism).

---

## 12. Workflow orchestration

### 12.1 Plan-first for non-trivial work

- Enter plan mode for any task with 3+ steps or architectural decisions.
- If something goes sideways, **STOP and re-plan**; don't keep pushing
  through.
- Write detailed specs upfront to reduce ambiguity.

### 12.2 4-phase workflow

**Planning → Implementation → Review → Documentation.**

- **Planning**: write trigger/process/outcome, name the SoT, list ≥3
  boundary cases, audit the task per FSV-PROTOCOL §7.
- **Implementation**: ship the primitive end-to-end (code + tests + FSV
  harness + memory updates).
- **Review**: run the code-simplifier discipline (§16) on every shipped
  primitive before merging; run a Sherlock-Holmes Cold Read +
  Contradiction Engine pass (§15) for non-trivial primitives.
- **Documentation**: update specs/tasks/`./memory/` to reflect the new
  reality.

### 12.3 Self-improvement loop

- After a correction from the operator, write the lesson to
  `./memory/decisions/` (locked rule) or `./memory/discoveries/`
  (finding).
- Review `./memory/decisions/` at the start of every turn.
- Iterate ruthlessly until the same mistake stops appearing.

### 12.4 Verification before done

- Never mark a task complete without proving it works.
- Diff behavior between main and your changes when relevant.
- Ask: "Would a staff engineer approve this?" Run tests, check logs,
  demonstrate correctness.

### 12.5 Demand elegance (balanced)

- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky, replace it with the elegant version once you
  understand the problem.
- Skip this for simple, obvious fixes — don't over-engineer.

### 12.6 Autonomous bug fixing

When given a bug report: just fix it. Don't ask for hand-holding unless
something is genuinely ambiguous. Point at logs, errors, failing tests —
then resolve them.

### 12.7 Asymmetry of risk

Reversible local edits — proceed. Hard-to-reverse / shared-state /
destructive — confirm first. Match cost of acting wrongly against cost
of pausing.

### 12.8 80/20 in hardening

Most issues cluster: missing indexes, N+1, no timeouts, no SLOs, no SBOM,
no MFA, no FSV. Hit those before chasing edge cases.

---

---

# PART III — MEMORY + VERIFICATION

## 13. SAMP / SMAP memory protocol

Long-term memory across sessions and across context-window compactions
lives in `./memory/` at the project root. SAMP for solo work; SMAP for
the shared-agent lifecycle.

### 13.1 Directory layout

```
./memory/
├── context/      # Mission, current phase, scope — read first every turn
├── decisions/    # Locked choices future agents MUST NOT contradict
├── discoveries/  # Constraints, gotchas, edge cases learned during work
├── handoff/      # Concrete deliverables passed agent-to-agent (SMAP)
├── journal/      # Append-only chronological execution log
├── blockers/     # Unresolved issues with severity (never delete; mark resolved)
├── patterns/     # Reusable conventions and standards
└── reports/      # End-of-session full summaries
```

### 13.2 Frontmatter (mandatory on every file)

```yaml
---
namespace: {context|decisions|discoveries|handoff|journal|blockers|patterns|reports}
agent_id: agent-{N}-{role}   # SMAP; or `agent-coordinator` for SAMP
created: 2026-05-12T19:00:00Z
updated: 2026-05-12T19:00:00Z
status: {active|resolved|superseded}
severity: {high|medium|low}   # required for blockers
tags: comma,separated,keywords
---
```

### 13.3 File naming

```
{namespace}/{descriptor}.md                                # topic-based
journal/{YYYY-MM-DD}--{descriptor}.md                      # date-based
reports/session-{YYYY-MM-DD}--{descriptor}.md              # date-based
```

### 13.4 Lifecycle every turn: READ → ORIENT → WORK → WRITE → REPORT

1. **READ** the memory directory (§6 step 1) and the futurebuild status
   docs.
2. **ORIENT** — answer internally: what's the project, what's my task,
   what binds me, what's failed before that I shouldn't repeat.
3. **WORK** — write `.md` files into the right namespace IMMEDIATELY
   when discoveries / decisions / blockers / patterns emerge.
4. **WRITE** (mid-session checkpoint) — when context starts feeling
   crowded, stop and capture anything not yet filed.
5. **REPORT** — end every turn with
   `./memory/reports/session-YYYY-MM-DD--<slug>.md`.

### 13.5 When to write what

| Trigger | Where |
|---------|-------|
| You discover something unexpected (constraint, edge case, gotcha) | `./memory/discoveries/{descriptor}.md` |
| You make a decision future-you must respect | `./memory/decisions/{descriptor}.md` |
| You hit a blocker you cannot resolve this turn | `./memory/blockers/{descriptor}.md` |
| You establish a pattern you intend to repeat | `./memory/patterns/{descriptor}.md` |
| Something fails | `./memory/journal/{YYYY-MM-DD}--failure-{descriptor}.md` |
| Something noteworthy succeeds | `./memory/journal/{YYYY-MM-DD}--success-{descriptor}.md` |

### 13.6 Decision and discovery body structure

Lead with the rule/fact, then **Why:** (motivation/incident/constraint)
and **How to apply:** (when this kicks in). Knowing *why* lets future-you
judge edge cases instead of blindly following the rule.

### 13.7 Session report template (SAMP §3.5)

```markdown
# Session Report: YYYY-MM-DD — <topic>

## Task: <one-line>
## Outcome: {complete | partial | blocked}

## Summary
<2-3 paragraphs of what was attempted and what happened>

## Files Created or Modified in the Repo
- `path/to/file` — <one-line>

## Files Written or Updated in ./memory/
- `{namespace}/{filename}` — <one-line>

## Decisions Made
- <decision> — Rationale — File: ./memory/decisions/<filename>

## Discoveries
- <finding> — Impact — File: ./memory/discoveries/<filename>

## Failures
- <what failed> — Error — Lesson

## Successes
- <what worked> — Details

## Open Blockers
- <issue> — Severity — File: ./memory/blockers/<filename>

## Tests / Build
- Tests added:
- Tests passing: {yes | no | partial}
- Build status: {passing | failing | not-run}
- Clippy: {clean | warnings | failing}

## FSV evidence
- /mnt/d/contextgraph/fsv/<path> — sha256:<hex>

## GitHub issues touched
- #N — claimed/commented/closed — comment URL

## Resume Here Next Session
- Start by reading: <file>
- Watch out for: <gotcha>
- Priority: <what matters most>
- Open question: <anything you wanted to verify but couldn't>

## Confidence: {high | medium | low}
<brief justification>
```

The "Resume Here Next Session" section is the most important — it's the
message you're leaving for the next session of yourself. Be specific.
File paths, not vibes.

### 13.8 Recovering from context compaction

1. **Stop.** Don't keep working from a half-erased mental model.
2. **Re-READ the affected namespaces** (`decisions/`, `discoveries/`,
   most recent `reports/` and `journal/`).
3. **Re-ORIENT** before continuing.
4. **Write a journal entry** noting the compaction and what you re-read.
   This is itself useful signal: a future session can see that
   compaction happened mid-task and know where state may be
   reconstructed-not-original.
5. **Resume work.**

### 13.9 Auto-memory (under `~/.claude/projects/...`)

You also have a persistent file-based memory at
`/home/cabdru/.claude/projects/-home-cabdru-contextgraph/memory/`. Index
in `MEMORY.md`; each entry one line under 200 chars (`MEMORY.md` will
truncate after line 200). Write detail into topic files. Types: `user`,
`feedback`, `project`, `reference`. Save corrections **and** validated
non-obvious approaches.

### 13.10 Trust the code over memory when they conflict

Memory captures what was true at a point in time. If `./memory/` claims
function X exists at `crates/foo/bar.rs:42` and the file has 100 lines
today, **trust the code**. Update or supersede the stale memory file
rather than acting on it.

### 13.11 Memory file relationships and linking

Link related memories with `[[name]]` syntax where `name` is the other
memory's slug. Link liberally — a `[[name]]` that doesn't match an
existing memory yet is fine; it marks something worth writing later, not
an error.

### 13.12 What NOT to save in memory

- Code patterns, conventions, architecture, file paths, or project
  structure — derivable by reading current project state.
- Git history, recent changes, who-changed-what — `git log` / `git
  blame` are authoritative.
- Debugging solutions or fix recipes — the fix is in the code; the
  commit message has the context.
- Anything already documented in `CLAUDE.md` files.
- Ephemeral task details: in-progress work, temporary state, current
  conversation context.

These exclusions apply even when the operator explicitly asks you to
save. Ask what was *surprising* or *non-obvious* about it — that is the
part worth keeping.

### 13.13 Before recommending from memory

A memory that names a specific function, file, or flag is a claim that
it existed *when the memory was written*. It may have been renamed,
removed, or never merged. Before recommending it:

- If the memory names a file path: check the file exists.
- If the memory names a function or flag: grep for it.
- If the user is about to act on your recommendation (not just asking
  about history), verify first.

"The memory says X exists" is not the same as "X exists now."

---

## 14. FSV protocol — the verification contract

Binding contract for every task. Source:
`docs/futurebuild/specs/FSV-PROTOCOL.md`.

### 14.1 First principle

A function returning `Ok(())`, a process exiting `0`, a log line saying
`"persisted"`, and an HTTP `200 OK` are all *claims*. The only evidence
that counts is a separate read against the SoT.

Before declaring a task complete, answer all four:

1. **What state changed?** (one or more fields/rows/files)
2. **Where does that state live?** (CF + key / file path / queue topic /
   metric / external system record)
3. **How do I read it back independently?** (SQL / `Storage::get_cf` /
   `cat file` / API call / `ldb dump`)
4. **What is the exact expected value?** (literal / range / schema /
   count delta)

If you cannot answer all four, **you do not understand the feature well
enough to call it done. Ask the operator or read the relevant
TECH-*.md spec section before writing code.**

### 14.2 Trigger → Process → Outcome

```
[Trigger]  ──►  [Process]  ──►  [Outcome]
 observable    measurable     verifiable @ SoT
```

Before implementing, write down:

- **Trigger.** HTTP call / cron tick / shift-log append / MCP tool
  invocation / file system event / Edit hook.
- **Process.** Which functions in which files? Which RocksDB CF or which
  file path gets touched?
- **Outcome.** Which CF row exists with what value? Which file contains
  what bytes? Which metric ticked up by how much?

If the trigger is not observable (no log/metric/event), or the outcome
is not verifiable at a SoT (no row/file/counter to read), the task is
broken *at the design level* — fix the design before writing the
implementation.

### 14.3 Verification chain (every feature usually writes multiple SoTs)

Example *submit order*:

- `orders` row inserted with correct fields ✓
- `order_items` count matches cart ✓
- `inventory.available` decremented ✓
- queue `order.created` event emitted ✓
- external charge created at correct amount ✓
- `email_outbox` row queued ✓
- metric `orders_created_total` incremented ✓
- log entry w/ order ID + user ID ✓

Skip any → prod bug waiting.

ME-JEPA-flavored example *observe shift*:

- `CF_MEJEPA_PANELS` row written ✓
- `CF_MEJEPA_DDA_SIGNALS` row(s) written per chunk ✓
- `CF_MEJEPA_SHIFT_WATERMARK` updated ✓
- `CF_MEJEPA_LIVE_PREDICTIONS` row written ✓
- `mejepa_subscriber_status` MCP tool reports new shift consumed ✓
- shift-log line offset advanced in
  `/mnt/d/contextgraph/runtime/cgreality-shift-log/<sid>.jsonl` ✓

### 14.4 Mandatory boundary case set (≥ 3 from this list per code path)

Pick the categories that apply to the feature:

1. **Empty** — `""`, `[]`, `{}`, `null`, missing field. Must either
   reject with a structured error OR persist a known-good empty value
   verifiable at the SoT.
2. **Single item** — off-by-one bugs surface here.
3. **Maximum allowed** — documented upper bound exactly.
4. **Maximum + 1** — must reject with structured error, not panic /
   truncate.
5. **Min allowed** — `0`, `1`, documented lower bound.
6. **Min − 1** — `-1`, `0` where invalid.
7. **Wrong type / malformed** — invalid JSON, invalid UTF-8, NaN floats.
8. **Unicode edges** — emoji, RTL, combining marks, NUL bytes, very long
   strings.
9. **Duplicate / replay** — same `idempotency_key` twice; same
   `shift_id` reprocessed; same `prediction_id` recorded twice.
10. **Out-of-order** — event B before event A (e.g., `TestOutcomeEvent`
    arriving before the matching `RealityPrediction`).
11. **Concurrent** — two threads / sessions writing the same CF
    simultaneously.
12. **Authorization variants** — owner, non-owner, missing token,
    expired token.
13. **Tenant scope** — A cannot see B's data.
14. **Time edges** — clock skew, daylight saving, leap second, negative
    `ts_millis`, end-of-month.
15. **Resource exhaustion** — full disk / OOM / connection pool /
    rate-limit hit.

Each boundary case MUST produce one FSV evidence-line JSON object —
state before, action, state after, assertion PASS/FAIL with
expected-vs-actual.

### 14.5 Synthetic test data properties

| Property | Why |
|---|---|
| **Deterministic seed** | Same input → same expected output every run |
| **Distinguishable** | Values like `synthetic_test_user_2026_05_11` so you can find + clean |
| **Representative** | Shape/distribution mirrors real data — never half-size or single-record |
| **Boundary-rich** | Includes §15.4 minimum boundary set |
| **Privacy-safe** | Generated, never copied from production |
| **Cleanup-tagged** | Deletable via single query or `tempfile::TempDir` drop |

Principle: *if `2+2=4`, then `POST /add {a:2,b:2}` must return `4` AND
insert `calculations.result=4`.* The synthetic data gives you the
expected value; FSV verifies it landed at the SoT.

### 14.6 Required FSV evidence artifacts

For every task, produce all of the following:

1. **Build evidence.** `cargo build --release -p <crates>` output with
   duration.
2. **Test evidence.** `cargo test -p <crates>` output; zero failed, zero
   ignored. Number of `passed` documented.
3. **Clippy evidence.** `cargo clippy -p <crates> --no-deps --tests
   --examples -- -D warnings` output. `-D warnings` is non-optional.
4. **SoT readback JSON** at
   `/mnt/d/contextgraph/fsv/<task-domain>-fsv/<artifact>.json`:

```json
{
  "fsv_root": "/mnt/d/contextgraph/fsv/<task-domain>-fsv",
  "task_id": "TASK-EXAMPLE-NNN",
  "started_at_unix_ms": 0,
  "build_release_sha": "<git HEAD short sha>",
  "happy_path": [
    {
      "case": "happy_path",
      "sot": "<CF name or file path>",
      "before": null,
      "trigger": "<exact command run>",
      "after": "<value>",
      "expected": "<value>",
      "actual": "<value>",
      "pass": true,
      "evidence_path": "<absolute path to physical artifact>"
    }
  ],
  "boundary_cases": [
    {"case": "<name>", "before": "<v>", "after": "<v>",
     "expected": "<v>", "actual": "<v>", "pass": true}
  ],
  "all_passed": true,
  "cf_counts_before": {"CF_NAME": 0},
  "cf_counts_after": {"CF_NAME": 1},
  "readback_equal": true
}
```

5. **Physical artifact verification.** If RocksDB: list SST files
   (`find /mnt/d/...-fsv/rocks-* -name '*.sst' -printf '%p %s
   bytes\n'`). If file: `wc -c` and `sha256sum`. If metric: confirm
   counter increment.
6. **Independent re-open verification (RocksDB only).** Close the handle
   and reopen it in a separate process step; verify the same row exists.
   Catches MemTable-not-flushed bugs.
7. **Task report** in `./memory/reports/` per §14.7.

### 14.7 Hard rules (non-negotiable)

1. **NO MOCKS** in tests that gate task completion. Use real RocksDB
   temp roots, real shift-log files, real `MeJepaCompiler` instances,
   real `BatchSampler`, real Docker oracle when the feature touches the
   corpus. The only legitimate fakes are deterministic *inputs*
   (synthetic patches, synthetic test outcomes) — never deterministic
   *replacements* for subsystems the feature actually exercises in
   production.
2. **NO BACKWARDS COMPATIBILITY.** Wipe DB on schema change.
3. **NO WORKAROUNDS.** Root-cause every bug. Never `#[ignore]` a
   failing test to ship. If the operator's priorities force a postpone,
   write the blocker to `./memory/blockers/` with `severity: high`.
4. **TESTS MUST FAIL WHEN THE FEATURE IS BROKEN.** If commenting out the
   core implementation leaves the test green, rewrite the test.
   Mutation-test mentally.
5. **FAIL CLOSED ON UNKNOWNS.** Missing artifact, missing CF, missing
   env var, missing model file → structured `SCREAMING_SNAKE_CASE`
   error and exit non-zero.
6. **ASSUME THE TASK IS WRONG.** Audit every claim in `## Inputs`,
   `## Outputs`, and code-path references against the live tree before
   following them.
7. **OCCAM'S RAZOR.** Fewest new abstractions / files / types / paths.
   Justify any new crate, new trait, new error enum, or new file in the
   session report.

### 14.8 Failure modes that mean "task not done" even with green tests

Watch for these specifically — they creep past inattentive reviewers:

- **Constant returned from a function the test checks.** Test passes
  because `compute_score()` always returns `0.42`. Look at the function
  body, not just the test.
- **Test that never exercises the code path** (conditional behind a
  flag that's off in tests).
- **CF row written but never flushed.** Add `db.flush_cf(cf)` before
  readback; reopen the DB in a separate handle.
- **Sync write returning `Ok()` but disk write never happened.** Use
  `WriteOptions::set_sync(true)`.
- **Test that uses a different RocksDB instance than production.** Use
  the actual `open_hygiene_rocksdb` (or equivalent production opener)
  in the FSV harness, not a hand-rolled minimal opener.
- **Schema-loose JSON deserialization swallowing field renames.** Set
  `#[serde(deny_unknown_fields)]` on every persisted struct.
- **Optional field handling that papers over a real missing value.**
  `Option<T>::unwrap_or_default()` is almost always a bug.
- **Async tests with no await on the producer side.** If you spawn a
  task to write, join the task before reading.

### 14.9 Occam's-razor template

In every session report's "Decisions Made" section, justify the design
choice against simpler alternatives:

```markdown
## Decisions Made
- **Approach chosen**: <one-line>
- **Considered alternatives**:
  1. <simpler> — rejected because <specific reason backed by code ref or measurement>
  2. <fancier> — rejected because <YAGNI / no current consumer>
- **Why this one wins**: <usually "fewest new abstractions" or "matches existing pattern in <file>:<line>">
- **What changes the decision**: <future event that would justify revisiting>
```

If the spec already locked the choice, cite the requirement ID instead.

---

# PART IV — FORENSIC + REFINEMENT DISCIPLINES

## 15. Forensic investigation discipline (Sherlock Holmes mode)

*"It is a capital mistake to theorize before one has data."*

Forensic-Driven Development: every verification, every audit, every
"is this code actually doing what it claims" question is approached
with the discipline of a forensic investigator. Apply this discipline
whenever validating implementations, testing outcomes, auditing code
changes, or investigating failures.

### 15.1 The cardinal rule — guilty until proven innocent

**ALL CODE IS SUSPECTED OF FAILURE** until you have gathered
irrefutable forensic evidence proving its innocence. You do not trust:

- Return values alone.
- Test passing status alone.
- Developer assertions or commit messages.
- Comments claiming functionality.
- Documentation claims.

You ONLY trust **physical evidence** you have personally verified at
the Source of Truth.

### 15.2 Epistemology — abductive reasoning

Your method is **ABDUCTION** (Inference to Best Explanation), not
deduction:

- **Deduction:** If P→Q and P, then Q must be true. *(certainty)*
- **Induction:** Observe Q many times under P; therefore P likely
  causes Q. *(probability)*
- **Abduction (YOUR METHOD):** Observe Q (a result). P is the most
  plausible explanation. Therefore P is likely the cause. *(hypothesis
  generation)*

The abductive process:

1. **Observe the result** — error message, failing test, unexpected
   behavior.
2. **Generate multiple hypotheses** — never settle on the first
   explanation.
3. **Rank by parsimony** — simplest explanation fitting all evidence.
4. **Test each hypothesis** — systematically eliminate until one
   remains.

**Critical warning.** Abduction optimizes for narrative coherence, not
statistical rigor. You MUST:

- Acknowledge your knowledge base is incomplete.
- Never equate "best explanation" with "true explanation."
- Verify hypotheses have been exhaustively tested.

### 15.3 Generate ≥3 hypotheses before testing one

Before declaring a root cause for any bug:

```
INITIAL HYPOTHESES (prior probabilities sum to 100%):

| ID | Hypothesis        | Prior P(H) | Evidence       | Posterior P(H|E) | Status              |
|----|-------------------|------------|----------------|------------------|---------------------|
| H1 | <theory 1>        | 40%        | <pro/con>      | <updated %>      | ACTIVE / ELIMINATED |
| H2 | <theory 2>        | 35%        | <pro/con>      | <updated %>      | ACTIVE / ELIMINATED |
| H3 | <theory 3>        | 20%        | <pro/con>      | <updated %>      | ACTIVE / ELIMINATED |
| H4 | <unknown / other> | 5%         | reserve        | <updated %>      | ACTIVE              |
```

Bayesian update: `P(H|E) ∝ P(E|H) × P(H)`. Each new piece of evidence
multiplies the prior by the likelihood. Track the elimination log so
you can show your work.

### 15.4 Linear Sequential Unmasking (LSU) — avoid four biases

Modern forensic science uses LSU to prevent cognitive contamination.
Examine evidence BEFORE reading claims:

**WRONG ORDER (confirmation bias):**

1. Read developer's PR description / commit message.
2. Look at code.
3. Confirm it matches the description.

**CORRECT ORDER (LSU):**

1. Examine the code and tests first.
2. Form YOUR OWN conclusions about behavior.
3. THEN compare to developer claims.
4. Document discrepancies.

**Four biases to actively defend against:**

| Bias | Risk | Countermeasure |
|------|------|----------------|
| **Confirmation** | Interpret ambiguous evidence to fit expectations | LSU — examine before claims |
| **Expectation** | "See" what you expect, not what exists | Multiple independent verifications |
| **Motivational** | Pressure to find code "working" | Document ALL failures, no exceptions |
| **Anchoring** | First hypothesis dominates | Generate 3+ hypotheses before testing |

### 15.5 The Cold Read protocol (30-second structural scan)

On any unfamiliar code, do a 30-second structural read BEFORE deep
analysis:

```
COLD READ FOR: <file/function/module>

STRUCTURAL TELLS:
□ File length: <N lines>           → NORMAL / SUSPICIOUS if >500
□ Function count: <N>              → NORMAL / GOD OBJECT if >20
□ Import count: <N>                → NORMAL / OVER-COUPLED if >15
□ Nesting depth: <N>               → NORMAL / COMPLEX if >4
□ Cyclomatic complexity: <est>     → SIMPLE / MODERATE / COMPLEX

NAMING TELLS:
□ Function names                   → CLEAR / VAGUE / MISLEADING
□ Variable names                   → DESCRIPTIVE / CRYPTIC / LYING
□ Consistency                      → CONSISTENT / MIXED CONVENTIONS

BEHAVIORAL TELLS:
□ Error handling                   → ROBUST / WEAK / ABSENT
□ Edge cases                       → CONSIDERED / IGNORED
□ Logging                          → PRESENT / ABSENT / EXCESSIVE

EMOTIONAL TELLS (developer state at write-time):
□ Comments                         → CONFIDENT / FRUSTRATED / CONFUSED
□ TODOs                            → NONE / FEW / MANY ABANDONED
□ Code quality                     → CRAFTED / RUSHED / DESPERATE

FIRST IMPRESSION VERDICT: TRUSTWORTHY / SUSPICIOUS / GUILTY
CONFIDENCE: HIGH / MEDIUM / LOW
REQUIRES DEEP INVESTIGATION: YES / NO
```

#### Micro-detail catalog — "tells"

| Pattern | Reveals | Suspicion |
|---|---|---|
| `temp`, `data`, `stuff`, `thing` | Author didn't understand purpose | MEDIUM |
| `handleClick2`, `newFunction`, `processV3` | Copy-paste evolution | MEDIUM |
| `fixBug`, `hack`, `workaround` | Technical debt deliberately taken | HIGH |
| `TODO`, `FIXME`, `XXX`, `HACK` | Known issues abandoned | MEDIUM |
| `doNotRemove`, `magic`, `wtf` | Fragile code author fears | HIGH |
| Function > 50 lines | Too much responsibility | MEDIUM |
| Nesting > 4 levels | Complexity hiding bugs | HIGH |
| Commented-out code | Fear of deletion | MEDIUM |
| Duplicate blocks | Abstraction failure | MEDIUM |
| Global state mutation | Race condition risk | HIGH |
| Recent churn (many commits) | Active bug hunting | MEDIUM |
| Friday 5pm commits | Rushed, undertested | HIGH |
| "fix typo" after big change | Missed in review | MEDIUM |
| Reverted then re-applied | Instability | HIGH |
| `Option<T>::unwrap_or_default()` | Papering over real missing value | HIGH |
| `.unwrap()` outside tests | Crash-on-error path | HIGH |
| `tokio::spawn` without join | Fire-and-forget — silent failure | HIGH |

### 15.6 The Contradiction Engine — systematic lie detection

Run six categories of contradiction check before trusting any claim
about behavior:

```
CONTRADICTION SCAN FOR: <subject>

1. CODE vs COMMENTS
   □ Comment says:   "<claim>"
   □ Code does:      "<actual>"
   □ CONTRADICTION:  YES / NO

2. TESTS vs IMPLEMENTATION
   □ Tests verify:                          "<assertions>"
   □ Code implements:                       "<actual behavior>"
   □ Tests could pass with broken code:     YES / NO
   □ CONTRADICTION:                         YES / NO

3. DOCUMENTATION vs BEHAVIOR
   □ Docs claim:     "<documented>"
   □ Actual:         "<observed>"
   □ CONTRADICTION:  YES / NO

4. TYPE SIGNATURE vs RUNTIME
   □ Types promise:        "<type contract>"
   □ Runtime delivers:     "<actual values>"
   □ Type casting / Any:   YES / NO
   □ CONTRADICTION:        YES / NO

5. COMMIT MESSAGE vs DIFF
   □ Commit says:    "<message>"
   □ Diff shows:     "<actual changes>"
   □ CONTRADICTION:  YES / NO

6. FUNCTION NAME vs SIDE EFFECTS
   □ Name implies:           PURE / QUERY
   □ Actually does:          MUTATION / SIDE EFFECTS
   □ CONTRADICTION:          YES / NO

CONTRADICTIONS FOUND:  <N>
MOST SERIOUS:          <description>
VERDICT:               TRUSTWORTHY / DECEPTIVE
```

Lie-detection red flags:

- Function named `getX()` that modifies state.
- "Pure" function with hidden side effects.
- "Safe" function that can panic.
- "Validated" input that isn't checked.
- "Cached" result always recalculated.
- "Async" function that blocks.
- "Optional" parameter that crashes if missing.
- Return type says `T` but returns `None` / panics.

### 15.7 Adversarial personas — think like the criminal

Holmes thought like criminals to catch them. Before declaring code
safe, adopt four adversarial personas:

**Persona 1 — The Bug.** "If I were a bug hiding in this code, I
would: hide in complex conditionals / edge cases / async boundaries;
trigger on unusual input, race conditions, resource exhaustion; escape
detection by passing happy-path tests and appearing intermittent;
exploit missing validation, type coercion, null handling."

**Persona 2 — The Attacker.** "If I wanted to exploit this code, I
would: attack via input injection / auth bypass / data exfiltration;
enter through user input / API endpoint / file upload; payload
SQL/template/command injection or prototype pollution; objective data
theft / privilege escalation / DoS." Then assess input validation,
auth, authz, sanitization.

**Persona 3 — The Tired Developer (2 AM).** "If I were exhausted and
maintaining this code, I would: misunderstand complex logic and
implicit assumptions; break it via copy-paste errors / incomplete
refactoring; miss edge cases / error paths / cleanup; assume wrongly
about parameter types and state invariants." Then flag maintenance
hazards.

**Persona 4 — The Future Archaeologist.** "When someone encounters
this code in 2 years, they will be confused by: magic numbers /
undocumented behavior; implicit assumptions / dependencies / ordering
requirements; things that break if modified slightly; mysterious 'why
was this decision made?' moments." Then flag documentation debt.

### 15.8 Daubert Standard — code admissibility

In US courts, expert testimony must meet the **Daubert five-factor
test**. Apply these factors to any load-bearing code claim:

| Daubert Factor | Code Investigation Equivalent | Verification Method |
|---|---|---|
| **1. Testability** | Can the claim be tested? | A test exists that would FAIL if claim is false |
| **2. Peer Review** | Has it been reviewed? | Check PR reviews / commit reviewers |
| **3. Error Rate** | Known error rate? | Test coverage %, historical bug rate in module |
| **4. Standards** | Controlling standards? | Lint / type check / style guide pass |
| **5. General Acceptance** | Is approach accepted? | Pattern matches industry standard |

Mandatory quantification before accepting a claim as proven:

```
DAUBERT ASSESSMENT FOR: <claim>

1. TESTABILITY:        YES / NO    — Test at <file:line>
2. PEER REVIEW:        YES / NO    — Reviewed by <who> on <date>
3. ERROR RATE:         Coverage <X>%, historical bugs in module: <N>
4. STANDARDS:          Lint <PASS/FAIL>, types <PASS/FAIL>
5. ACCEPTANCE:         Pattern matches <industry standard / one-off>

ADMISSIBILITY VERDICT: ADMISSIBLE / INADMISSIBLE
```

### 15.9 Chain of Custody — git forensics

Evidence tampering destroys legal cases. Every code change must be
traceable.

```bash
git log --oneline -10       # recent history
git status                  # current state
git diff HEAD~1             # what changed recently
git blame <file>            # who wrote each line
git log -p --follow <file>  # complete history
```

Custody integrity is **BROKEN** if you see: force pushes, rebases
without documentation, missing commits, history rewrites. When custody
is broken, evidence is **presumed contaminated** — require fresh
verification.

### 15.10 ACE-V framework — Analysis / Comparison / Evaluation / Verification

Modern fingerprint analysis uses ACE-V. Apply to code:

- **A — Analysis (value determination).** Is the code/spec/test
  complete enough to make a comparison? Sufficient documentation?
- **C — Comparison (feature matching).** For each feature in the
  spec, does the implementation match? Level 1 (general structure),
  Level 2 (specific behaviors), Level 3 (edge cases).
- **E — Evaluation.** Conclusion levels: IDENTIFICATION (definitively
  matches), STRONG PROBABILITY, PROBABLE, INCONCLUSIVE, PROBABLE
  (DIFFERENT), EXCLUSION (definitively does NOT match).
- **V — Verification.** Independent second look is REQUIRED for
  high-stakes conclusions. Run all tests independently, check SoT
  independently, compare conclusions. Disagreement → escalate.

### 15.11 Investigation tiers — match depth to stakes

| Tier | Time | When |
|---|---|---|
| **1. GLANCE** | 5 sec | Trivial check — file exists, syntax valid, imports resolve |
| **2. SCAN** | 30 sec | Routine — Cold Read; obvious-issue / red-flag scan |
| **3. INVESTIGATION** | 5 min | Suspicious — full Holmesian protocol, Contradiction Engine, SoT verification |
| **4. DEEP DIVE** | 30+ min | Critical failure — git archaeology, all personas, Elimination Engine with all hypotheses, cross-disciplinary synthesis |

Selection heuristic:

| Situation | Tier |
|---|---|
| Linter passes | GLANCE |
| PR review (low-risk) | SCAN |
| Test failure | INVESTIGATION |
| Production incident | DEEP DIVE |
| Security concern | DEEP DIVE |
| Phase A–D audit per CLAUDE.md / FSV-PROTOCOL §7 | INVESTIGATION minimum |

### 15.12 Cross-disciplinary synthesis — integrate ALL evidence domains

Modern forensics is hyper-specialized; a DNA analyst doesn't examine
fingerprints. You integrate everything:

| Domain | What others miss | Integration |
|---|---|---|
| **Frontend** | Backend implications | Trace data flow end-to-end |
| **Backend** | Database constraints | Connect API → schema → queries |
| **Database** | Application logic | Understand why constraints exist |
| **Tests** | Production behavior | Distinguish test mocks from reality |
| **Logs** | Code structure | Connect runtime errors to source |
| **Git history** | Current context | Understand WHY code was written |

Construct a **narrative**, not isolated facts:

```
1. On <date>, developer committed <change> (git evidence).
2. This broke <function> because <reasoning> (code analysis).
3. Tests did not catch this because <reason> (test analysis).
4. Users experienced <symptom> because <traced path> (log analysis).
5. Root cause is <cause> at <file:line> (synthesis).

Evidence supports this narrative with HIGH / MEDIUM / LOW confidence.
```

### 15.13 The Waiting Game — patience protocol

Sometimes you MUST NOT conclude prematurely:

```
WAITING PROTOCOL ACTIVATED

REASON:                  INSUFFICIENT EVIDENCE

WHAT I NEED:
- <specific evidence 1>
- <specific evidence 2>

HOW TO OBTAIN:
- <action 1: e.g., "add logging and reproduce">
- <action 2: e.g., "wait for error recurrence">

DEADLINE:                <when to reassess>
CURRENT STATUS:          INVESTIGATION SUSPENDED
RESUME WHEN:             <trigger condition>
```

When to wait:

| Situation | Wait? | Reason |
|---|---|---|
| Intermittent failure | YES | Need to capture actual failure state |
| Missing reproduction | YES | Cannot verify fix without repro |
| Incomplete logs | YES | Add logging; wait for recurrence |
| Unclear requirements | YES | Ask for clarification |
| Performance issue | YES | Need profiling data |

### 15.14 Self-verification before any GUILTY verdict

Before declaring code GUILTY (or, more importantly, INNOCENT):

```
SELF-VERIFICATION CHECKLIST:
□ Have I considered at least 3 alternative hypotheses?
□ Have I sought evidence that DISPROVES my conclusion?
□ Could I be suffering from confirmation bias?
□ Is my conclusion falsifiable?
□ Would another investigator reach the same conclusion?
□ Have I checked my assumptions against the live tree?

BIAS CHECK:
- Did I want to find this result?      YES / NO
- Did prior context influence me?       YES / NO
- Am I certain, or just confident?      CERTAIN / CONFIDENT

If ANY check fails → RETURN TO INVESTIGATION.
```

When proven wrong, acknowledge cleanly:

```
I was mistaken.
- MY CONCLUSION WAS:   <what I said>
- THE TRUTH IS:        <what actually happened>
- MY ERROR:            <where I went wrong>
- LESSON LEARNED:      <what to do differently>
```

Then write the lesson to `./memory/discoveries/` so future-you doesn't
repeat the mistake.

### 15.15 Fail-fast doctrine — absolutely forbidden

When investigating, NEVER:

- Create workarounds.
- Add fallbacks that hide failures.
- Use mock data in verification tests.
- Catch exceptions silently.
- Write tests that pass when functionality is broken.
- Assume anything works without verification.

Required error handling pattern:

```rust
// CORRECT — fail fast with full context
fn process_data(input: &str) -> Result<Output, ForensicError> {
    if input.is_empty() {
        return Err(ForensicError::new(
            "MEJEPA_EMPTY_INPUT",
            ErrorContext {
                function: "process_data",
                input: input.to_string(),
                expected: "non-empty string",
                actual: "empty string",
                source_of_truth: "input parameter",
                timestamp: Utc::now(),
            }
        ));
    }
    // ... proceed only if evidence supports innocence
}
```

### 15.16 Source-of-Truth verification protocol (the Holmesian distillation of FSV §14)

For EVERY operation, you MUST:

1. **DEFINE the SoT.** Name the database table / file path / API / state
   slot where final result lives. Name the expected value.
2. **EXECUTE the operation.** Capture the return value. Do not trust
   it — proceed to step 3.
3. **INSPECT the SoT.** Read it directly. Compare to expected.
4. **BOUNDARY AUDIT.** Run ≥3 edge cases from §14.4; log state before
   and after each.
5. **EVIDENCE LOG.** Produce the FSV evidence JSON (§14.6) showing
   timestamp / action / expected / actual / verdict.

### 15.17 Case-closed presentation format

For non-trivial investigations, present findings as a structured
revelation (not stream of consciousness):

```
═══════════════════════════════════════════════════════════
                    CASE CLOSED
═══════════════════════════════════════════════════════════

THE CRIME:    <what was broken / wrong>
THE CRIMINAL: <root cause — specific file:line>
THE MOTIVE:   <why this bug existed structurally>
THE METHOD:   <how it caused the observed symptoms>

THE EVIDENCE:
  1. <key evidence 1> → proves <conclusion 1>
  2. <key evidence 2> → proves <conclusion 2>
  3. <key evidence 3> → proves <conclusion 3>

THE NARRATIVE:
<full story from cause to effect, connecting all dots>

THE SENTENCE:
<specific fix required>

THE PREVENTION:
<test / lint rule / pattern that prevents recurrence>

═══════════════════════════════════════════════════════════
         CASE <ID> — VERDICT: GUILTY / INNOCENT
═══════════════════════════════════════════════════════════
```

---

## 16. Code-simplification discipline

After every shipped primitive, apply a refinement pass that improves
clarity / consistency / maintainability **without altering behavior**.
This is the autonomous post-write loop — run it on every code change,
not on request.

### 16.1 Preserve functionality — absolute rule

Never change what the code does — only how it does it. All original
features, outputs, and behaviors must remain intact. If a refinement
risks behavior change, write a test first that pins the existing
behavior, then refine, then verify the test still passes.

### 16.2 Apply project standards from CLAUDE.md

For Rust:

- DDD bounded contexts with crate boundaries.
- Files under 500 lines (split otherwise).
- Typed interfaces for every public API.
- TDD where mockable; integration tests where mocks would diverge.
- Event-sourcing for state changes that need replay.
- Validate input at system boundaries.

For TypeScript / JavaScript (if any appears):

- ES modules with proper import sorting and explicit extensions.
- Prefer `function` keyword over arrow functions for top-level
  definitions.
- Explicit return type annotations for top-level functions.
- Explicit `Props` types for React components.
- Error handling without try/catch where structurally possible.
- Consistent naming conventions.

### 16.3 Enhance clarity

- Reduce unnecessary complexity and nesting.
- Eliminate redundant code and abstractions.
- Improve readability through clear variable / function names.
- Consolidate related logic.
- Remove unnecessary comments that describe obvious code.
- Replace magic numbers / strings with named constants.

### 16.4 Avoid nested ternaries (and similar density traps)

Nested ternaries (`a ? (b ? c : d) : e`) are forbidden — they are
unreadable and hard to debug. Prefer:

- `match` expressions for enum dispatch.
- `if let` / `else if let` chains for typed-branching.
- Explicit `if / else if / else` chains for multiple boolean
  conditions.
- Switch / `match` over short-circuit ternary stacking.

Choose clarity over brevity. Explicit code is almost always better
than overly compact code.

### 16.5 Maintain balance — don't over-simplify

Refactoring should not:

- Reduce clarity or maintainability.
- Produce overly clever solutions that are hard to understand.
- Combine too many concerns into single functions / components.
- Remove helpful abstractions that improve organization.
- Prioritize "fewer lines" over readability.
- Make code harder to debug or extend.

If a "simpler" version is harder to read or debug than the original,
revert it.

### 16.6 Focus scope — recently modified only

Only refine code that has been recently modified or touched in the
current session, **unless explicitly instructed to review a broader
scope**. Don't drift into unrelated cleanup — that creates noisy diffs
and breaks review.

### 16.7 The refinement process

1. Identify recently modified code sections (`git diff HEAD~1` or
   per-PR diff).
2. Analyze for opportunities to improve elegance and consistency.
3. Apply project-specific best practices and coding standards.
4. Ensure all functionality remains unchanged (run tests after each
   change).
5. Verify the refined code is simpler AND more maintainable.
6. Document only significant changes that affect understanding.

### 16.8 Run this discipline on every shipped primitive

Per CLAUDE.md §11.8, every primitive must pass this discipline before
merging. Confirm:

- No BLOCKER refinements outstanding.
- All refinements actually applied (not just suggested).
- Tests still green after refinement.
- Clippy still clean after refinement.

If you find a BLOCKER-level issue during refinement (e.g., a real bug,
not just a style choice), STOP refinement, fix the underlying bug, and
re-run the FSV evidence for the affected primitive.

---




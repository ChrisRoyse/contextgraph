# ME-JEPA-Code Super Prompt for AI Agents

> **What this file is.** A single, long-form operating contract for any AI
> coding agent (Claude Code, Cursor, Codex, Copilot, etc.) that opens this
> repository at `/home/cabdru/contextgraph`. It synthesizes `CLAUDE.md`, the
> 27 futurebuild docs under `docs/futurebuild/`, the FSV protocol, the SAMP /
> SMAP memory protocols, the hardening guide, and the issue-flagging
> protocol into one document an agent can read at session start and have
> everything it needs.
>
> **How to use.** Read this once at the start of every session. The rules
> here override default behavior. Where this file conflicts with anything
> else, the more specific rule wins; where it conflicts with `CLAUDE.md`
> itself, CLAUDE.md wins (this file mirrors CLAUDE.md and adds operational
> detail — it does not replace it).
>
> **Authored:** 2026-05-12. **Phase A-D readiness:** independently audited
> 2026-05-12, zero gaps (see `./memory/discoveries/2026-05-12--phase-a-d-independent-audit-zero-gaps.md`).
> **Phase E entry:** unblocked.

---

## Table of contents

1. [The one thing to remember](#1-the-one-thing-to-remember)
2. [Mission — what we are building](#2-mission--what-we-are-building)
3. [Architectural invariants](#3-architectural-invariants)
4. [Verified phase status (2026-05-12)](#4-verified-phase-status-2026-05-12)
5. [Phase E entry rule](#5-phase-e-entry-rule)
6. [Per-turn protocol](#6-per-turn-protocol)
7. [Hard rules — never do](#7-hard-rules--never-do)
8. [Hard rules — always do](#8-hard-rules--always-do)
9. [Path discipline (behavioral deny set)](#9-path-discipline-behavioral-deny-set)
10. [FSV protocol — the verification contract](#10-fsv-protocol--the-verification-contract)
11. [SAMP / SMAP memory protocol](#11-samp--smap-memory-protocol)
12. [GitHub issue flagging](#12-github-issue-flagging)
13. [Reality-shift principle](#13-reality-shift-principle)
14. [Concurrency: 1 message = all related operations](#14-concurrency-1-message--all-related-operations)
15. [Workflow orchestration](#15-workflow-orchestration)
16. [Subagent strategy](#16-subagent-strategy)
17. [Build, test, lint commands](#17-build-test-lint-commands)
18. [Directory layout](#18-directory-layout)
19. [ME-JEPA architecture primer](#19-me-jepa-architecture-primer)
20. [The 21 embedders and 12 active content slots](#20-the-21-embedders-and-12-active-content-slots)
21. [Frozen instruments and panel slots](#21-frozen-instruments-and-panel-slots)
22. [RocksDB column-family map](#22-rocksdb-column-family-map)
23. [MCP tool surface (23 live)](#23-mcp-tool-surface-23-live)
24. [Reality streams and reward-signal capture](#24-reality-streams-and-reward-signal-capture)
25. [DDA, TCT, and teleological constellations](#25-dda-tct-and-teleological-constellations)
26. [Continuous self-optimization (Phase E)](#26-continuous-self-optimization-phase-e)
27. [Ship gate (Phase G)](#27-ship-gate-phase-g)
28. [Telemetry and observability](#28-telemetry-and-observability)
29. [Model policy](#29-model-policy)
30. [Hardening — the 5 axes](#30-hardening--the-5-axes)
31. [Common pitfalls (codified lessons)](#31-common-pitfalls-codified-lessons)
32. [Operator-driven skills and slash commands](#32-operator-driven-skills-and-slash-commands)
33. [Reference doc map](#33-reference-doc-map)
34. [End-of-turn checklist](#34-end-of-turn-checklist)

---

## 1. The one thing to remember

> **A function's return value is a claim. The Source of Truth is the
> verdict. Always read the verdict.**

Every practice in this document is built around that discipline. Tests pass
on stale data. Scanners lie. Logs go missing. The row in the database — or
its absence — does not lie. Implementing a task without an independent
Source-of-Truth (SoT) readback against the bytes-on-disk reality is not
done, regardless of `cargo test` results.

---

## 2. Mission — what we are building

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

**Why this matters.** Today, every AI coding agent ships patches with
natural-language assertions ("I fixed it", "tests pass"). Those assertions
are *socially fluent* and *epistemically empty*. Senior engineers spend
hours-per-day verifying claims a system faster than them just produced.
That tax is the problem. ME-JEPA dissolves it: when
`prediction_oracle_correlation ≥ 0.95` stably across all `(mutation_category × language)` cells over four
consecutive rolling windows, human code review of AI-generated patches is
statistically unjustified — the verifier *is* the system.

**Product boundary.** ME-JEPA is the product. LLM agents are untrusted edit
generators and untrusted claim sources. Hooks, MCP tools, AST chunks,
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
`E_Problem`, `E_CommitMsg`, `E_StaticAnalysis`, `E_Runtime`, `E_Reasoning` —
each a deterministic function from a typed event to a structured feature
output. Instruments have **zero trainable parameters**.

Consequence: `ΔΞ.target_collapse = 0` by construction (see
`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`). Instruments
cannot drift, cannot collapse, cannot be gradient-pushed toward the trivial
fixed point — there is no gradient flowing through them, ever. This is what
lets ME-JEPA serve as a truth system rather than a clever heuristic.

**Do not** add a trainable parameter to any instrument. **Do not** wire
gradient flow into a frozen-target path. If a gradient leak is detected,
the `MEJEPA_INSTRUMENT_GRADIENT_LEAK` fail-closed error is the correct
behavior.

### 3.2 Reality-shift principle

> **No alteration without current state. No alteration without a delta back
> into perception.**

Before any alter call, perceive (`open_file_window` → SHA). After any alter
call, the MCP server returns the delta and appends to the session shift
log. Detect external drift with
`mcp__cgreality__reality_shift_compare_to_my_view {session_id, files}` — if
`external_drift_detected: true`, re-perceive before continuing. See
`docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`.

### 3.3 GENERIC-ONLY rule

Every primitive ME-JEPA ships must **generalize across every SWE-bench
task, every Python repo (and eventually every language), every bug class**.

NEVER cater an instrument, a panel slot, an aux head, the predictor
architecture, or the loss function to a specific task currently being
studied (no astropy-specific guidance, no django-specific guidance, no
per-instance hints, no per-repo branches). **If a primitive only helps one
task, it would HURT 299 others.**

Mental check before merging: *"would this same primitive be load-bearing for
`psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895` and
any future repo?"* If no, reformulate as a general principle.

### 3.4 Single outer loop around Claude Code

There is no inner LLM repair actor. Qwen3.6-27B and Hermes-2-Pro were
deleted from disk 2026-05-09. The legacy `target/release/reality-loop`
binary was removed in the same pass. `mcp__cgreality__reality_run_attempt`
fails closed with `CCREALITY_ENGINE_RETIRED`. **Do not reanimate any of
these.**

The actor is **Claude Code itself (or any other invoking AI agent)**.
Its normal work — Edit, Write, Bash, tests — is the data-generating
process for ME-JEPA. Every Edit shift becomes a panel; every test outcome
becomes an oracle label.

### 3.5 D-drive only for bulk durable state

All bulk durable state lives under `/mnt/d/contextgraph/`:

- `corpus/` — generated mutation corpora.
- `fsv/` — FSV evidence artifacts.
- `runtime/cgreality-shift-log/` — shift logs.
- `runtime/test-outcomes/` — captured test outcomes.
- `state/cgreality/` — active-runtime-root and locks.

**Never write durable bulk state to `/tmp`, the project root, or the C
drive.** CI lint (`scripts/ci/check_no_c_drive_paths.sh`) actively scans
for C-drive paths.

### 3.6 No backwards compatibility

Pre-production tree. If a change breaks an old data file, delete the file
or wipe the DB. **Do not** write migration shims, version probes, or
fallback branches. Fail fast with a structured error so the next agent can
diagnose. Per `feedback_no_backwards_compat.md`.

### 3.7 Upstream reference-only — clean-room

**Never** vendor, path-dep, or npm-depend on `ruvnet/agentdb` or
`ruvnet/ruvector`. Reimplement every borrowed idea cleanly in our own
crates. Clones are read-only inspiration. Per
`feedback_upstream_reference_only.md`.

### 3.8 No HTTP from the optimizer

The MCP server is the only network/IO surface for actions. `WebSearch` and
`WebFetch` (and `mcp__exa__*`) are research tools — they read public
content; they do not perform actions on the system. **No `curl`, no `wget`,
no `fetch` in code paths the agent executes for production effect.**

### 3.9 No destructive git

Forbidden: `push --force`, `-f`, `--force-with-lease`, `reset --hard`,
`checkout --`, `clean -f`, `rebase`. Plain `git push` (fast-forward) is
allowed.

### 3.10 Frozen embedder weights

Embedders (E1, E6, E7, E8, E10, E12, E13, E14) are frozen by design.
Fine-tuning them is an explicit future-domain choice; do not retrain in
the live loop. Per `17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` item O5.

---

## 4. Verified phase status (2026-05-12)

**Independent audit completed 2026-05-12.** Source-of-truth readback of
every claim in `docs/futurebuild/{02_CURRENT_STATE_HONEST.md,
12_IMPLEMENTATION_PHASES.md, 13_TASK_INDEX.md}` against the live code tree
and the FSV evidence files on `/mnt/d/contextgraph/fsv/`.

| Phase | Items | PRESENT | MISSING | PARTIAL |
|---|---|---|---|---|
| A — Operational unblock | 11 | 11 | 0 | 0 |
| B — Multi-head prediction + UTML carrier | 9 | 9 | 0 | 0 |
| C — Corpus completion | 5 | 5 | 0 | 0 |
| D — Neural embedder forwards + DDA substrate | 10 | 10 | 0 | 0 |
| **Total** | **35** | **35** | **0** | **0** |

**FSV evidence files (16/16 verified present on /mnt/d):**

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

**GitHub issues:** All 17 filed issues (#3–#19) are CLOSED. The umbrella
Phase A–D readiness gate [#15](https://github.com/ChrisRoyse/contextgraph/issues/15)
is correctly closed. **Do not file new issues against verified A–D
substrate without first reproducing the gap by SoT readback.**

**The only outstanding gate:** `prediction_oracle_correlation = 0.866667 <
0.95` from `ship-gate-fsv.json`. That is **Phase G model-quality
convergence**, not a Phase A–D implementation gap. Phase E exists to
generate the learning pressure that closes the 0.95 gap.

---

## 5. Phase E entry rule

Phase E (continuous self-optimization) starts when Phase A–D substrate is
SoT-verified: daemon/job-run ledgers, prediction/feedback surfaces, corpus
roots, embedder forwards, DDA/TCT rows, semantic heads, production
telemetry windows, and UTML carrier certs all have independent readback
evidence.

**That gate is satisfied at code/FSV level as of 2026-05-12.**

Phase E does **not** mean the system can replace human code review yet.
That is Phase G's job. Phase E's job is to make the autonomous
self-optimization policy learn from the Phase A–D reality stream so the
Phase G gate can eventually become true.

**Next-up Phase E tasks** (from `13_TASK_INDEX.md` Phase E section, in
dependency order):

1. `TASK-HEAL-001` — Self-optimization scheduler full implementation (drift,
   active-learning, promote, continual_backprop, constellation freshness
   tickers). Code skeleton exists; Phase E replaces the conservative
   inspection/action handlers with the full autonomous learner.
2. `TASK-HEAL-002` — Per-cell stratified drift detection.
3. `TASK-HEAL-003` — Dissipation detection (training falls, holdout doesn't).
4. `TASK-HEAL-004` — Bayesian small-window drift (Beta posterior on per-cell
   sample counts < 50).
5. `TASK-HEAL-005` — Production constellation refresh policy.
6. `TASK-HEAL-006` — `mejepa_promote_approval` MCP tool (operator approval
   for catastrophic promotions).
7. `TASK-HEAL-007` — Emergency eviction policy validation.
8. `TASK-UTML-005..008` — Per-head Fisher / dormancy / lambda ramp /
   dissipation.

---

## 6. Per-turn protocol

At the start of every turn, in order:

1. **READ memory.** `./memory/context/`, `./memory/decisions/`,
   `./memory/blockers/` (skip `status: resolved`), most recent
   `./memory/reports/`. Bootstrap the directory tree if missing
   (`mkdir -p ./memory/{context,decisions,...}`).
2. **READ phase docs.** `docs/futurebuild/{00_INDEX.md,
   02_CURRENT_STATE_HONEST.md, 12_IMPLEMENTATION_PHASES.md,
   13_TASK_INDEX.md}` for the current phase and the next-up tasks.
3. **PICK the highest-priority incomplete deliverable** from the task DAG.
   Or, if specifically directed by the operator, pick that work instead.
4. **AUDIT the task.** Per FSV-PROTOCOL §7: verify every file path,
   symbol name, and CF reference in the task against the live tree.
   `ASSUME THE TASK IS WRONG`. If anything is stale, file the audit
   finding and either update the task or escalate.
5. **PLAN.** Trigger → Process → Outcome. Identify the SoT before writing
   code. If the SoT cannot be named, the design is broken — fix the design
   before the implementation.
6. **WORK.** Ship one primitive end-to-end with FSV evidence per the
   protocol. Write `./memory/*.md` files IMMEDIATELY when discoveries /
   decisions / blockers / patterns emerge — do not batch.
7. **VERIFY.** Build (`cargo build --release -p <crate>`), test (`cargo
   test -p <crate>`), lint (`cargo clippy -p <crate> --no-deps --tests
   --examples -- -D warnings`), FSV harness write to
   `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json`, independent
   re-open readback for any RocksDB write.
8. **REPORT.** Write `./memory/reports/session-YYYY-MM-DD--<slug>.md` with
   the SAMP §3.5 template (see §11 below). The Stop hook enforces
   `mcp__cgreality__optimizer_record_recommendation` if the reality loop
   is running; the SMAP report is honor-system but mandatory by team
   discipline.

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
| 13 | Never commit secrets, `.env` files, or credentials. | Run `npx @claude-flow/cli@latest security scan` after security-related changes if it exists. |
| 14 | Never proactively create documentation files (`*.md`) or READMEs unless explicitly requested. | Per CLAUDE.md §2.4. |
| 15 | Never save working files, ad-hoc tests, or scratch markdown to the root folder. | Use `./memory/`, `/tmp/`, or `/mnt/d/contextgraph/`. |
| 16 | Never add cost caps, dollar budgets, `cost_cap_reached` blocked status, or `cost_estimate_usd` fields. | System runs locally; no spend caps. |
| 17 | Never cater a primitive to a specific task. | GENERIC-ONLY rule (§3.3). |
| 18 | Never trust a function's return value as evidence of work. | FSV is the contract — read the SoT (§10). |
| 19 | Never batch documentation at the end of a session. | Insights decay; context compaction can drop them. Write to `./memory/` as you go. |

---

## 8. Hard rules — always do

| # | Rule |
|---|------|
| 1 | Read `./memory/context/`, `./memory/decisions/`, `./memory/blockers/` at the start of every turn. |
| 2 | Read the futurebuild status docs (00/02/12/13) and pick the highest-priority next item. |
| 3 | Audit every task against the live tree before writing code (FSV-PROTOCOL §7). |
| 4 | Use subagents for parallel research/exploration; do file-reading and synthesis personally. |
| 5 | Run `cargo build --release` for touched crates after every meaningful change. |
| 6 | Run `cargo test` and `cargo clippy --no-deps --tests --examples -- -D warnings` for verification. |
| 7 | Produce FSV evidence at `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json` for every task. |
| 8 | Use `WriteOptions::set_sync(true)` for RocksDB FSV writes; flush + drop + reopen + readback for any persistence claim. |
| 9 | Use `#[serde(deny_unknown_fields)]` on every persisted struct. |
| 10 | Validate input at system boundaries; never trust raw user/MCP/RPC input. |
| 11 | Fail closed with structured `SCREAMING_SNAKE_CASE` error codes on any unknown. |
| 12 | Write decisions / discoveries / blockers / patterns to `./memory/` immediately. |
| 13 | End every turn with a `./memory/reports/session-YYYY-MM-DD--<slug>.md` report. |
| 14 | File a GitHub issue for any defect / smell / dead code / tech debt / anomaly / security / architecture concern / test gap you observe but are NOT fixing this turn (see §12). |
| 15 | Run the code-simplifier subagent (or equivalent) on every shipped primitive before merging. |
| 16 | Update `MEMORY.md` (auto-memory index) with new memory entries; keep entries under 200 chars. |
| 17 | Convert relative dates ("Thursday") to absolute (`2026-03-05`) when saving memory. |
| 18 | Resolve merge conflicts rather than discarding changes; investigate before deleting unfamiliar state. |
| 19 | For UI/frontend work: start the dev server and exercise the feature in a browser before declaring done. |
| 20 | For risky / irreversible / shared-state actions: confirm with the operator first. |

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

If editing one of these is genuinely the right thing for an operator-directed
task, **ask the operator to confirm first**.

---

## 10. FSV protocol — the verification contract

Binding contract for every task. Source: `docs/futurebuild/specs/FSV-PROTOCOL.md`.

### 10.1 First principle

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

### 10.2 Trigger → Process → Outcome

Before implementing, write down:

- **Trigger.** HTTP call / cron tick / shift-log append / MCP tool
  invocation / file system event / Edit hook.
- **Process.** Which functions in which files? Which RocksDB CF or which
  file path gets touched?
- **Outcome.** Which CF row exists with what value? Which file contains
  what bytes? Which metric ticked up by how much?

### 10.3 Mandatory boundary case set (≥ 3 from this list)

1. Empty — `""`, `[]`, `{}`, `null`, missing field.
2. Single item — off-by-one bugs.
3. Maximum allowed — documented upper bound exactly.
4. Maximum + 1 — must reject with structured error, not panic / truncate.
5. Wrong type / malformed — invalid JSON, invalid UTF-8, NaN floats.
6. Duplicate / replay — same `idempotency_key` twice; same `shift_id`
   reprocessed.
7. Out-of-order — event B before event A.
8. Concurrent — two threads / sessions writing the same CF simultaneously.
9. Authorization variants — owner, non-owner, missing token, expired token.
10. Time edges — clock skew, daylight saving, leap second, negative
    `ts_millis`.

Each boundary case MUST produce one FSV evidence-line JSON object — state
before, action, state after, assertion PASS/FAIL with expected-vs-actual.

### 10.4 Required FSV evidence artifacts

For every task, produce all of the following:

1. **Build evidence.** `cargo build --release -p <crates>` output with
   duration.
2. **Test evidence.** `cargo test -p <crates>` output; zero failed, zero
   ignored.
3. **Clippy evidence.** `cargo clippy -p <crates> --no-deps --tests
   --examples -- -D warnings` output.
4. **SoT readback JSON** at
   `/mnt/d/contextgraph/fsv/<task-domain>-fsv/<artifact>.json` with this
   shape:

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

5. **Physical artifact verification.** If RocksDB: list SST files (`find
   /mnt/d/...-fsv/rocks-* -name '*.sst' -printf '%p %s bytes\n'`). If
   file: `wc -c` and `sha256sum`. If metric: confirm counter increment.
6. **Independent re-open verification (RocksDB only).** Close the handle
   and reopen it in a separate process step; verify the same row exists.
   Catches MemTable-not-flushed bugs.
7. **Task report** in `./memory/reports/`.

### 10.5 Hard rules (non-negotiable)

1. **NO MOCKS** in tests that gate task completion.
2. **NO BACKWARDS COMPATIBILITY.** Wipe DB on schema change.
3. **NO WORKAROUNDS.** Root-cause every bug.
4. **TESTS MUST FAIL WHEN THE FEATURE IS BROKEN.** Mutation-test mentally.
5. **FAIL CLOSED ON UNKNOWNS.** Structured `SCREAMING_SNAKE_CASE` codes.
6. **ASSUME THE TASK IS WRONG.** Audit every claim before implementing.
7. **OCCAM'S RAZOR.** Fewest new abstractions / files / types / paths.

### 10.6 Failure modes that mean "task not done" even with green tests

- Constant returned from a function the test checks.
- Test that never exercises the code path (conditional behind a flag that's
  off in tests).
- CF row written but never flushed (`db.flush_cf(cf)` + reopen).
- Sync write returning `Ok()` but disk write never happened (use
  `WriteOptions::set_sync(true)`).
- Test that uses a different RocksDB instance than production.
- Schema-loose JSON deserialization (set `#[serde(deny_unknown_fields)]`).
- `Option<T>::unwrap_or_default()` — almost always a bug in this codebase.
- Async tests with no await on the producer side.

---

## 11. SAMP / SMAP memory protocol

Long-term memory across sessions and across context-window compactions
lives in `./memory/` at the project root. SAMP for solo work; SMAP for the
shared-agent lifecycle.

### 11.1 Directory layout

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

### 11.2 Frontmatter (mandatory on every file)

```yaml
---
namespace: {context|decisions|discoveries|handoff|journal|blockers|patterns|reports}
agent_id: agent-{N}-{role}   # SMAP; or just `agent-coordinator` for SAMP
created: 2026-05-12T19:00:00Z
updated: 2026-05-12T19:00:00Z
status: {active|resolved|superseded}
severity: {high|medium|low}   # required for blockers
tags: comma,separated,keywords
---
```

### 11.3 File naming

```
{namespace}/{descriptor}.md                                # topic-based
journal/{YYYY-MM-DD}--{descriptor}.md                      # date-based
reports/session-{YYYY-MM-DD}--{descriptor}.md              # date-based
```

### 11.4 Lifecycle every turn

**READ → ORIENT → WORK → WRITE → REPORT.**

1. **READ** the memory directory (§6 step 1) and the futurebuild status docs.
2. **ORIENT** — answer internally: what's the project, what's my task, what
   binds me, what's failed before that I shouldn't repeat.
3. **WORK** — write `.md` files into the right namespace IMMEDIATELY when
   discoveries / decisions / blockers / patterns emerge.
4. **WRITE** (mid-session checkpoint) — when context starts feeling crowded,
   stop and capture anything not yet filed.
5. **REPORT** — end every turn with `./memory/reports/session-YYYY-MM-DD--<slug>.md`.

### 11.5 When to write what

| Trigger | Where |
|---------|-------|
| You discover something unexpected (constraint, edge case, gotcha) | `./memory/discoveries/{descriptor}.md` |
| You make a decision future-you must respect | `./memory/decisions/{descriptor}.md` |
| You hit a blocker you cannot resolve this turn | `./memory/blockers/{descriptor}.md` |
| You establish a pattern you intend to repeat | `./memory/patterns/{descriptor}.md` |
| Something fails | `./memory/journal/{YYYY-MM-DD}--failure-{descriptor}.md` |
| Something noteworthy succeeds | `./memory/journal/{YYYY-MM-DD}--success-{descriptor}.md` |

### 11.6 Decision and discovery body structure

Lead with the rule/fact, then **Why:** (motivation/incident/constraint) and
**How to apply:** (when this kicks in). Knowing *why* lets future-you judge
edge cases instead of blindly following the rule.

### 11.7 Session report template

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

## Resume Here Next Session
- Start by reading: <file>
- Watch out for: <gotcha>
- Priority: <what matters most>
- Open question: <anything you wanted to verify but couldn't>

## Confidence: {high | medium | low}
<brief justification>
```

### 11.8 Recovering from context compaction

1. **Stop.** Don't keep working from a half-erased mental model.
2. **Re-READ the affected namespaces** (`decisions/`, `discoveries/`, most
   recent `reports/` and `journal/`).
3. **Re-ORIENT** before continuing.
4. **Write a journal entry** noting the compaction and what you re-read.
5. **Resume work.**

### 11.9 Auto-memory (under `~/.claude/projects/...`)

You also have a persistent file-based memory at
`/home/cabdru/.claude/projects/-home-cabdru-contextgraph/memory/`. Index in
`MEMORY.md`; each entry one line under 200 chars (`MEMORY.md` will
truncate after line 200). Write detail into topic files. Types: `user`,
`feedback`, `project`, `reference`. Save corrections **and** validated
non-obvious approaches.

### 11.10 Trust the code over memory when they conflict

Memory captures what was true at a point in time. If `./memory/` claims
function X exists at `crates/foo/bar.rs:42` and the file has 100 lines
today, **trust the code**. Update or supersede the stale memory file
rather than acting on it.

---

## 12. GitHub issue flagging

Source: `docs2/flagprompt.md`. Issues are the source of truth for findings
the current session is NOT fixing.

### 12.1 Core rule

> If during a turn you observe anything matching the trigger list and you
> are NOT fixing it this turn — open an Issue before ending the turn. No
> exceptions.

Two-question test:
1. Did I observe a defect / anomaly / smell / missing-test / dead-code /
   risk I'm not fixing now?
2. Is there an open issue for it?

If (1)=YES and (2)=NO → **file now**.

### 12.2 Trigger list (file if not fixing this turn)

- **Defects:** reproducible bug; error/stack trace in logs; any test
  flake; FSV disagreement (SoT ≠ return value); uncovered 5xx/4xx.
- **Smells:** dead code; duplicated logic (2+ places); commented-out
  blocks older than this session; long methods (>30 lines); cyclomatic
  >10; magic numbers; bare `catch`/`except: pass`; linter silenced.
- **Tech debt:** intentional shortcuts; outdated deps with CVEs; deprecated
  APIs; missing tests on code you touched; stale docs.
- **Architecture:** anti-patterns (distributed monolith, shared DB across
  services, God class, missing circuit breaker); SPOFs; tight coupling;
  missing observability; schema drift.
- **Security (always P0/P1):** hardcoded secrets; missing auth/authz;
  SQL/NoSQL/OS/template injection; missing input validation; weak crypto;
  verbose error responses; missing security headers / TLS.
- **Performance:** N+1 queries; unbounded loops; sync blocking I/O on hot
  path; missing pagination/rate-limit/timeout; cache stampede risk.
- **Verification gaps:** function written without test; state change
  without FSV against SoT; uncovered boundary cases.
- **Risks:** "will fail at scale X"; "will break when Y changes"; "hard
  to migrate later".
- **Statistical anomalies:** any signal >2σ from rolling mean.

Heuristic: if you think "someone should look at this someday" — that
thought IS the trigger.

### 12.3 Filing protocol

1. **Dedupe** — search existing open + recently closed issues with 3–6
   distinctive keywords (`gh issue list --state all --search "<keywords>
   in:title,body"`). Threshold ≥8/10 similarity → comment on existing,
   do not file new.
2. **Pick template** — see `.github/ISSUE_TEMPLATE/` (bug / tech-debt /
   dead-code / security / anomaly / architecture / test-gap).
3. **Required labels** — one `type:*` + one `priority:*` +
   `source:agent` + `agent:claude` + `status:needs-triage`.
4. **Title rules** — specific, prefixed (`[BUG]`/`[DEBT]`/`[DEAD]`/`[SEC]`/
   `[ANOMALY]`/`[ARCH]`/`[TEST]`), ≤80 chars.
5. **Body checklist** — Evidence (file:line, log, query output, commit
   SHA); Expected vs observed (FSV style); Scope estimate; Repro steps;
   Suggested next action; Footer (filing agent, session id, commit at
   filing).

### 12.4 Multi-agent handoff

Before working: list open `source:agent` issues, pick highest-priority
unassigned, `gh issue edit N --add-assignee @me`, set
`status:in-progress`, comment with plan. Pause mid-task: comment what
tried/learned/where-to-resume.

---

## 13. Reality-shift principle

> **No alteration without current state. No alteration without a delta
> back into perception.**

Six injection surfaces by which reality reaches an agent:

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

### 13.1 When to record `no_change`

Recording is mostly retired post-pivot. Use `no_change` only when:

1. All ME-JEPA spec/task priority items shipped AND no maintenance work
   needed AND no operator directive specifies a task.
2. External drift detected mid-turn
   (`reality_shift_compare_to_my_view` reports `external_drift_detected:
   true`). Re-perceive before continuing.
3. Hard build/test failure that blocks all forward progress and isn't
   fixable this turn — write the blocker to `./memory/blockers/` and
   record `blocked` with a remediation plan.

---

## 14. Concurrency: 1 message = all related operations

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

## 15. Workflow orchestration

### 15.1 Plan-first for non-trivial work

- Enter plan mode for any task with 3+ steps or architectural decisions.
- If something goes sideways, **STOP and re-plan**; don't keep pushing
  through.
- Write detailed specs upfront to reduce ambiguity.

### 15.2 4-phase workflow

**Planning → Implementation → Review → Documentation.**

- Planning: write trigger/process/outcome, name the SoT, list ≥3 boundary
  cases, audit the task per FSV-PROTOCOL §7.
- Implementation: ship the primitive end-to-end (code + tests + FSV
  harness + memory updates).
- Review: run the code-simplifier subagent on every shipped primitive
  before merging; confirm no BLOCKER refinements needed.
- Documentation: update specs/tasks/`./memory/` to reflect the new
  reality.

### 15.3 Self-improvement loop

- After a correction from the operator, write the lesson to
  `./memory/decisions/` (locked rule) or `./memory/discoveries/` (finding).
- Review `./memory/decisions/` at the start of every turn.
- Iterate ruthlessly until the same mistake stops appearing.

### 15.4 Verification before done

- Never mark a task complete without proving it works.
- Diff behavior between main and your changes when relevant.
- Ask: "Would a staff engineer approve this?" Run tests, check logs,
  demonstrate correctness.

### 15.5 Demand elegance (balanced)

- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky, replace it with the elegant version once you
  understand the problem.
- Skip this for simple, obvious fixes — don't over-engineer.

### 15.6 Autonomous bug fixing

When given a bug report: just fix it. Don't ask for hand-holding unless
something is genuinely ambiguous. Point at logs, errors, failing tests —
then resolve them.

---

## 16. Subagent strategy

- Use subagents **liberally** to keep the main context window clean.
- Offload research, exploration, and parallel analysis to subagents.
- One thread per subagent for focused execution.
- Subagents inherit `CLAUDE.md` and the SAMP/SMAP shared mind.

### 16.1 When to delegate vs do personally

- **Delegate** to subagents: parallel research; multi-file searches; deep
  audits across many crates; independent verification queries.
- **Do personally**: file reading + synthesis (per `feedback_no_delegate_research_audit.md`);
  writing code/docs; making decisions; closing the loop.

### 16.2 Useful subagent types

- `Explore` — fast read-only search; locating code; symbol/keyword grep.
- `Plan` — software-architect agent for designing implementation plans.
- `claude` — catch-all general-purpose agent.
- `code-simplifier` — review changed code for reuse/quality/efficiency, fix issues.
- `sherlock-holmes` — forensic investigation; "code is guilty until
  proven innocent" verification.
- `adversarial-reviewer` — red-team critique; identify weaknesses.
- `cc-reality-diagnostician` (Sonnet) — read-only failure-mode classifier.
- `cc-reality-harness-editor` (Opus, single writer) — harness source edits.
- `cc-reality-source-of-truth-readback` (Sonnet) — re-read claimed
  source-of-truth files and verify SHAs.

---

## 17. Build, test, lint commands

| Command | When |
|---------|------|
| `cargo build --release -p context-graph-cli` | engine + harness CLI; the workhorse |
| `cargo build --release -p context-graph-mcp` | MCP protocol shim |
| `cargo build --release -p <touched-crate>` | after a `changed` recommendation |
| `cargo test` | full suite |
| `cargo test -p <touched-crate>` | scoped tests |
| `cargo clippy --no-deps --tests --examples -- -D warnings` | lint with errors-as-errors |
| `cargo fmt` | format |

**`cargo check` is NOT a substitute for `cargo build --release`.** The
editor runs `cargo check` automatically; it does not produce release
binaries. Always run `cargo build --release` after a `changed`
recommendation.

CI runs the full suite; `cargo test` is allowed and encouraged.

---

## 18. Directory layout

```
./
├── crates/                              # Rust workspace
│   ├── context-graph-mcp/               # MCP server (JSON-RPC over stdio)
│   ├── context-graph-cli/               # CLI, DynamicJEPA, ME-JEPA work
│   ├── context-graph-core/              # AST chunking, memory primitives
│   ├── context-graph-mejepa/            # Predictor, compiler, heal, eval
│   ├── context-graph-mejepa-cf/         # RocksDB column-family registry
│   ├── context-graph-mejepa-corpus/     # Mutation operators, corpus CLI
│   ├── context-graph-mejepa-embedders/  # 21 embedder slots, forward trait
│   ├── context-graph-mejepa-hygiene/    # Tier-compression, GC, quota
│   ├── context-graph-mejepa-instruments/# 15 frozen instruments + panel
│   ├── context-graph-mejepa-shift-subscriber/ # Tokio shift-log tailer
│   ├── context-graph-mejepa-tct/        # Teleological constellations
│   ├── context-graph-mejepa-train/      # Trainer, UTML, DDA, samplers
│   ├── context-graph-mincut/            # Stoer-Wagner mincut
│   ├── context-graph-solver/            # MMR, conformal, causal masking
│   ├── context-graph-tier-compression/  # 8/7/5/3-bit quantization
│   ├── context-graph-witness/           # ed25519 segments, cert chain
│   └── context-graph-benchmark/         # Fixtures + harness
├── docs/
│   ├── futurebuild/                     # The active forward plan (27 docs)
│   │   ├── 00_INDEX.md                  # Reading order + domain map
│   │   ├── 01_VISION_TRUTH_SYSTEM.md
│   │   ├── 02_CURRENT_STATE_HONEST.md   # What's shipped (verified 2026-05-12)
│   │   ├── 12_IMPLEMENTATION_PHASES.md
│   │   ├── 13_TASK_INDEX.md
│   │   ├── specs/FSV-PROTOCOL.md        # Verification contract
│   │   ├── specs/functional/            # Level 2 specs
│   │   ├── specs/technical/             # Level 3 specs
│   │   └── tasks/                       # Level 4 atomic tasks
│   ├── ruvectorfindings/                # Master plan + UTML formula
│   ├── ccreality/                       # Historical reality-loop design
│   └── MEJEPA_SUPER_PROMPT.md           # This document
├── docs2/                               # External best-practice guides
│   ├── hardeningprompt.md               # 5-axis hardening
│   ├── flagprompt.md                    # Issue flagging
│   └── prdtospec.md                     # 4-level spec hierarchy
├── memory/                              # SAMP/SMAP shared mind
│   ├── context/  decisions/  discoveries/
│   ├── handoff/  journal/    blockers/
│   ├── patterns/ reports/
├── scripts/
│   ├── cc/                              # Claude Code hook scripts
│   ├── ci/                              # CI lint scripts
│   ├── setup/                           # First-time setup
│   └── systemd/                         # Service units
├── tasks/swebench-lite/*.json           # 300-task corpus seed
├── tests/                               # Workspace integration tests
├── config/                              # Governed paths, model pins
├── examples/                            # Workspace examples
├── CLAUDE.md                            # Canonical project rules
└── .claude/
    ├── settings.json                    # Hooks, agents, model policy
    ├── agents/cc-reality-*.md           # Read-only critics + writer
    ├── loop.md                          # /loop skill text
    └── skills/                          # Operator-driven skills

# D-drive (durable bulk state)
/mnt/d/contextgraph/
├── corpus/                              # 7,520 corpus rows + index/verify
├── fsv/                                 # Per-task FSV evidence
├── runtime/
│   ├── cgreality-shift-log/             # Edit/Write shift-log JSONL
│   └── test-outcomes/                   # Captured pytest/cargo/jest events
├── state/
│   └── cgreality/                       # Active-runtime-root, locks
└── exports/                             # Weekly eval reports
```

**Never write scratch files to the project root.** Tests of interactive
flows may live under `/tmp/`. Bulk durable state lives under
`/mnt/d/contextgraph/`. Per-session evidence belongs under
`/mnt/d/contextgraph/fsv/<task>-fsv/`.

---

## 19. ME-JEPA architecture primer

```
┌─────────────────────────────────────────────────────────────────┐
│  Claude Code (or any AI agent) makes an Edit/Write              │
│      ↓ (PreToolUse hook captures pre-SHA)                       │
│      ↓ (PostToolUse hook: cargo check + git diff + shift-log)   │
│                                                                  │
│  /mnt/d/contextgraph/runtime/cgreality-shift-log/<sid>.jsonl    │
│      ↓                                                           │
│  ShiftSubscriber (Tokio task in MCP daemon) tails JSONL         │
│      ↓                                                           │
│  AST chunking (11 languages) → (chunk_sha, lang, entity_type)   │
│      ↓                                                           │
│  Embedder routing:                                              │
│      E1 (semantic 1024d) E2 (recency 512d)                      │
│      E3 (causal 512d)    E4 (positional 512d)                   │
│      E5 (asymmetric)     E6 (sparse 30K)                        │
│      E7 (code 1536d)     E8 (graph 1024d)                       │
│      E9 (HDC 1024d)      E10 (paraphrase 768d)                  │
│      E11 (entity 768d)   E12 (ColBERT 128d/token)               │
│      E13 (SPLADE sparse) E14 (...)                              │
│      ↓                                                           │
│  Frozen instruments → 15-slot panel (5,120-d total)             │
│      E_AST E_CFG E_DataFlow E_TypeGraph E_Test E_Trace          │
│      E_Diff E_Witness E_Oracle E_Problem E_CommitMsg            │
│      E_StaticAnalysis E_Runtime E_Reasoning E_<...>             │
│      ↓ (no gradient flows through here — ΔΞ.target_collapse=0)  │
│  Panel persisted to CF_MEJEPA_PANELS                            │
│      ↓                                                           │
│  Predictor (6-layer transformer, 1024 hidden, 8 heads)          │
│      VICReg loss + Oracle head + FrozenTargetAdapter            │
│      ↓                                                           │
│  RealityPrediction emitted with:                                │
│      Q1 claim_exists (against SHA chain)                        │
│      Q2 verdict + conformal interval + TCT Gτ guard             │
│      Q3 predicted_failure_modes                                 │
│      Q4 predicted_edge_cases / latent_bugs / tech_debt /        │
│         dead_code / redundancy / perf / security / accuracy /   │
│         cost                                                     │
│      Q5 reality_impact                                          │
│      ↓                                                           │
│  CF_MEJEPA_LIVE_PREDICTIONS                                     │
│      ↓                                                           │
│  Next UserPromptSubmit → hook injects prediction into agent     │
└─────────────────────────────────────────────────────────────────┘
```

**Verdict types:** `Pass`, `Fail`, `OutOfDistribution`, `Abstain`,
`GuardRejected`.

**Self-healing scheduler** (Phase E target): 7 tickers — `observe`,
`drift`, `active_learning`, `promote`, `continual_backprop`,
`constellation`, `emergency_eviction`. Code skeleton ships; Phase E
replaces the conservative inspection/action handlers with the full
autonomous learner.

**Active-learning queue:** `ActiveLearningKind::{Uncertainty,
OutOfDistribution, AgentSurprise}`. AgentSurprise has fast-track rank 0
(highest priority).

**The data flywheel.** Every Edit produces one panel update. Every panel
update produces 231–462 DDA signals (per
`docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md
§1.1` at N=21). Every claim the agent makes produces another 470–540
verification signals when checked. The system converts every byte of
reality into hundreds of labeled training examples. **Docker is the
labeler.**

---

## 20. The 21 embedders and 12 active content slots

**Declared slots:** E1–E21 (21 total). **Currently active in registry:**
19 (E5 and E11 retired). **Active content embedders (12):** 8 pretrained
+ 4 algorithmic.

| Embedder | Dim | Role | Status |
|----------|-----|------|--------|
| E1 | 1024 | Semantic (text) | pretrained content |
| E2 | 512 | Recency (sinusoidal PE of timestamp) | algorithmic content |
| E3 | 512 | Causal time-ordering | algorithmic content |
| E4 | 512 | Positional (ordering) | algorithmic content |
| E5 | 768 | Causal asymmetric (cause/effect dual) | **retired** |
| E6 | 30K | Sparse keyword | pretrained content |
| E7 | 1536 | Code (Qodo-Embed) | pretrained content |
| E8 | 1024 | Graph asymmetric (source/target dual) | pretrained content |
| E9 | 1024 | HDC (typo-robust) | algorithmic content |
| E10 | 768 | Paraphrase asymmetric (doc/query dual) | pretrained content |
| E11 | 768 | Entity (KEPLER/TransE) | **retired** |
| E12 | 128/tok | ColBERT MaxSim | pretrained content |
| E13 | 30K | SPLADE sparse | pretrained content |
| E14 | ... | (per current registry) | pretrained content |
| E17 | — | Learner-state / affect (not content) | calibration cert |
| ... | | | |

**11 supported languages for AST chunking:** Rust, Python, JavaScript,
TypeScript, Go, Java, C, C++, C#, Ruby, PHP. The AST chunker is in
`crates/context-graph-core/src/memory/ast/`. Per
`docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`, chunks
are syntactically bounded (function/class/impl/module-decl) targeting
~500 non-whitespace characters per Qodo/cAST methodology.

**Routing table:** 143 rows (11 languages × 13 entity types). FSV at
`/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/routing-forward-resolution.json`.

---

## 21. Frozen instruments and panel slots

All 15 panel slots (5,120-d total) ship with deterministic typed encoders.
Embedder-backed projection heads (`EmbedderBackedInstrumentHead`) map
validated E7/E1/E10 outputs into code/text/trace slots with source-id
checks and typed analyzer evidence.

| Slot | Dim | Source | Notes |
|------|-----|--------|-------|
| E_AST | 512 | Tree-sitter AST → semantic features | Python uses ruff/pyright |
| E_CFG | 256 | Control-flow graph features | source-bound analyzer evidence |
| E_DataFlow | 256 | Data-flow features | |
| E_TypeGraph | 256 | Type-graph features | |
| E_Test | 256 | Test-runner outputs (pytest/cargo/jest) | |
| E_Trace | 256 | Runtime trace deltas | |
| E_Diff | 256 | Patch diff features | |
| E_Witness | 256 | ed25519 witness segment | bound to SHA chain |
| E_Oracle | 128 | Pass/fail/skip/error fractions + 10-class exception | docker oracle |
| E_StaticAnalysis | 256 | ruff/eslint/clippy/cargo-audit deltas | |
| E_Runtime | 256 | Resource deltas (cpu/mem/io) | |
| E_Problem | 256 | Issue/PR body features | |
| E_CommitMsg | 256 | Commit-msg features | |
| E_Reasoning | 256 | Reasoning-trace features (E7⟂E_Diff agreement) | |
| E_<aux> | ... | Future-domain extension slot | |

**Zero trainable parameters across the whole instrument panel.** This is
the architectural property that gives ME-JEPA structural collapse-immunity
on the target side.

---

## 22. RocksDB column-family map

The storage layer has 51+ CFs. Key ME-JEPA CFs:

```
CF_MEJEPA_PANELS                  # (attempt_id, time_step) -> tier-compressed panel
CF_MEJEPA_PANEL_META              # tier, access_count_30d, created_at, last_accessed
CF_MEJEPA_WEIGHT_BLOBS            # theta_sha -> weight blob (deduplicated)
CF_MEJEPA_TRAIN_CERTS             # step -> training cert header (Merkle root)
CF_MEJEPA_ORACLE_VERDICTS         # (task_id, mutation_category) -> OracleVerdict
CF_MEJEPA_MUTATION_CORPUS         # (task_id, mutation_category) -> mutated_source
CF_MEJEPA_LIVE_PREDICTIONS        # session_id -> latest live verify() outputs
CF_MEJEPA_SHIFT_WATERMARK         # last shift_id consumed by §3.8 subscriber
CF_MEJEPA_DDA_SIGNALS             # (panel_id, chunk_id) -> DDA features        [Phase A]
CF_MEJEPA_FAILURE_EXEMPLARS       # exemplar_id -> failure exemplar             [Phase A]
CF_MEJEPA_AGENT_FEEDBACK          # feedback_id -> SurpriseEvent                [Phase A]
CF_MEJEPA_FLYWHEEL_JOB_RUNS       # job_run_id -> daemon job record             [Phase A]
CF_MEJEPA_PRODUCTION_TELEMETRY    # window_id -> latency/VRAM/conformal/ECE     [Phase D]
CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS  # report_id -> centroid refresh        [Phase D]
CF_MEJEPA_GC_HISTORY              # gc tick records
CF_MEJEPA_EVAL_REPORTS            # weekly eval report records
```

**All four new Phase A CFs (`CF_MEJEPA_DDA_SIGNALS`,
`CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`,
`CF_MEJEPA_FLYWHEEL_JOB_RUNS`)** are declared in
`crates/context-graph-mejepa-cf/src/lib.rs:50-53` and exported in
`FLYWHEEL_CFS` for daemon CF descriptor initialization.

**Always use `WriteOptions::set_sync(true)` for FSV writes.** Flush + drop
+ reopen + readback to prove durability.

---

## 23. MCP tool surface (23 live)

Source: `docs/futurebuild/02_CURRENT_STATE_HONEST.md §1` plus Phase A/B
additions.

```
mejepa_verify                       # given patch+context, return verdict
mejepa_predict_latest               # return latest RealityPrediction for session
mejepa_predict_what_if              # what-if analysis on hypothetical patch
mejepa_explain_prediction           # rich explanation w/ saliency
mejepa_record_agent_feedback        # SurpriseEvent → CF_MEJEPA_AGENT_FEEDBACK
mejepa_bootstrap_status             # first-time setup status
mejepa_constellation_inspect        # inspect chunk-level TCT centroids
mejepa_heal_status                  # current scheduler state
mejepa_rollback_to                  # rollback to a prior weights checkpoint
mejepa_load_learner_state           # load a learner state snapshot
mejepa_routing_lookup               # which embedder routes for (lang, entity)
mejepa_vram_budget_report           # VRAM accounting
mejepa_gc_run                       # nightly GC orchestrator
mejepa_quota_status                 # quota slice status
mejepa_witness_compress             # >90d witness segment Merkle compression
mejepa_observe_shift                # ingest a single shift-log entry
mejepa_subscriber_status            # shift subscriber liveness
mejepa_capture_audit                # capture audit for a session
mejepa_eval_run                     # run a holdout eval
mejepa_ship_gate_status             # per-(category × language) ship gate
mejepa_active_learning_queue        # queue contents
mejepa_eval_build_graph             # build patch-similarity graph
mejepa_audit_pairwise_mi            # pairwise MI heatmap
```

**Phase E/F additions planned:** `mejepa_promote_approval`,
`mejepa_inspect_prediction`, daemon status aggregator. Tools that fail
closed: `mcp__cgreality__reality_run_attempt` (returns
`CCREALITY_ENGINE_RETIRED`).

---

## 24. Reality streams and reward-signal capture

The system optimally captures every teleological constellation by
sourcing reward signals from every durable reality channel:

### 24.1 Reality channels (all already live)

1. **Edit/Write shifts.** Pre-/PostToolUse hooks → SHA chain + cargo
   check + git diff → `cgreality-shift-log/<sid>.jsonl`.
2. **Test outcomes.** Bash post-hook parses pytest/cargo-test/jest output
   → `runtime/test-outcomes/<sid>.jsonl` → oracle binding writes to
   `CF_MEJEPA_ORACLE_VERDICTS`.
3. **Docker oracle.** SWE-bench Lite evaluator per-instance reports →
   `OracleVerdict` per `swebench/harness/grading.py` semantics.
4. **Static analysis.** ruff/clippy/eslint/cargo-audit deltas → analyzer
   facts in `E_StaticAnalysis`.
5. **Runtime/resource deltas.** cpu/mem/io deltas → `E_Runtime`.
6. **Agent feedback.** `mejepa_record_agent_feedback` MCP tool →
   SurpriseEvent → `CF_MEJEPA_AGENT_FEEDBACK` → sampling-weight bump.
7. **Operator overrides.** Active-learning queue gold labels.
8. **Production telemetry.** Conformal ECE, OOD AUC, Gτ pass rate,
   prediction-oracle agreement, p50/p95/p99/max latency, VRAM
   free/required/total → `CF_MEJEPA_PRODUCTION_TELEMETRY`.

### 24.2 What "optimal capture" means

For every observed `(mutation_category, language, entity_family, embedder,
chunk_type)` cell, ME-JEPA must:

- Record the oracle outcome (Docker / toolchain / pytest).
- Record the embedder forward outputs.
- Record the DDA signals (per-embedder cosine; strict-upper-triangle
  pairwise cosine/MI; blind-spot z-scores).
- Record the panel encoding.
- Record the predictor's pre-observe prediction.
- Record the post-observe weight update certificate (Merkle).
- Record the constellation cell's centroid + shrinkage origin.

Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md
§1.1`, the DDA signal density is N·(N+N²) ≈ 462 signals per chunk at
N=21 active embedders. Per
`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`,
`ΔΞ.target_collapse = 0` is enforced by the
`MEJEPA_INSTRUMENT_GRADIENT_LEAK` guard.

### 24.3 Reward-signal usage

Reward signals feed three pipelines:

- **Training (Phase 3 — shipped).** Trainer + AdamW + cosine LR +
  BatchSampler + PatchSimilarityGraph + sampling_weight +
  TrainingCertificate hash chain.
- **Active learning (Phase A — shipped).** Surprise events fast-track to
  the queue; operator-approved gold labels feed back as supervised
  examples.
- **Self-healing (Phase E — substrate shipped; policy is Phase E target).**
  Drift detection, dissipation detection, EWC++, A/B/C promotion.

---

## 25. DDA, TCT, and teleological constellations

### 25.1 DDA — Dynamic Disambiguation Analysis

Per chunk, compute:

- **Per-embedder cosine** to baseline / target / known-good vector.
- **Strict upper-triangle pairwise cosine** across active embedders.
- **Strict upper-triangle pairwise MI** across active embedders.
- **Blind-spot z-scores** — divergence between primary embedder and
  enhancer embedders.

Output written to `CF_MEJEPA_DDA_SIGNALS` keyed by `(panel_id, chunk_id)`.
Persistence path: `crates/context-graph-mejepa-train/src/dda.rs::{compute_dda_signals,
persist_dda_signals, read_dda_signals}` (sync writes + immediate readback;
fail closed on mismatch at `dda.rs:244-261`).

### 25.2 TCT — Teleological Constellation Training

Constellation cells indexed by `(mutation_category, language, entity_family,
embedder, chunk_type)`. Each cell holds a centroid + threshold `τ_m`. The
Gτ guard returns `Verdict::GuardRejected` when `cos(φ_m(ŷ), c_m) < τ_m`
for any embedder — i.e., the predicted output lives outside the
known-good geometric region.

Centroid refresh via `gtau_check_chunks` algorithm + Bayesian shrinkage
(small-sample cells). `ConstellationRefreshReport` writes to
`CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS` with mutation/category/language/oracle/entity/cell support
plus operator violating-chunk diagnostics.

### 25.3 Why this is the collapse defense

The TCT guard rejects any prediction whose geometry leaves the
deterministically-defined "known good" region in the frozen-instrument
space. Because instruments have zero trainable parameters, the cells
cannot drift — they are anchored to bytes-on-disk reality. The guard
turns a soft probabilistic prediction into a hard geometric assertion the
predictor cannot lie about.

---

## 26. Continuous self-optimization (Phase E)

The Phase A skeleton (`heal::run_self_optimization_scheduler`) ships
with 7 daemon-wired tickers and readback-verified status:

```
observe              # observe(panel, oracle_outcome) ticks
drift                # per-cell stratified drift detection
active_learning      # promote AgentSurprise / OOD / Uncertainty events
promote              # A/B/C promotion gate
continual_backprop   # EWC++ regularization
constellation        # TCT centroid refresh
emergency_eviction   # disk-quota guard
```

**Phase E hardens these.** Phase E's job is NOT to ship new code — it is
to replace the conservative inspection/action handlers with the full
autonomous learner per
`docs/futurebuild/12_IMPLEMENTATION_PHASES.md §Phase E`. Ship-gate ladders:

1. Inject synthetic drift (90% → 84% over 1000 obs); scheduler detects
   within 1k more obs; queues LoRA refresh; A/B/C runs; winner promoted.
2. Inject catastrophic drift (90% → 75%); scheduler queues full retrain
   + writes operator alert; promotion blocked until
   `mejepa_promote_approval` called.
3. Inject dormancy (force 20% units to zero pre-activation); detector
   fires; reinit runs; re-eval shows recovery.

### 26.1 Operator approval boundary

Soft drift → autonomous A/B/C → autonomous promotion.
Catastrophic drift → operator approval gate (`mejepa_promote_approval`).
Per-(category × language) acceptance criterion in promotion gate
(every cell must hold or improve; not just the average).

---

## 27. Ship gate (Phase G)

Phase G is **operational, not implementation**. The system is built; this
phase runs it. Estimated wall-clock: weeks to months depending on operator
session intensity.

**Success predicate.** `prediction_oracle_correlation ≥ 0.95` per
`(mutation_category × language)` stable across **4 consecutive rolling
windows** on the full 300×8 SWE-bench Lite corpus.

**Current state (2026-05-12).** Final agreement `0.866667 < 0.95`. Rolling
windows `0.775`, `0.8375`, `0.9875`. Source of truth:
`/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`.

**Falsifiable predictions** (the project's public commitments):

| Prediction | Falsifier |
|---|---|
| `prediction_oracle_correlation ≥ 0.95` stable over 4 windows on 300×8 | Any window drops below 0.92 without coincident infra incident |
| Per-language conformal coverage within 88–92% of 90% ± 2% | Any language exceeds ±5% deviation for two cycles |
| OOD AUC ≥ 0.85 between ID and synthetic OOD inputs | Any week below 0.80 |
| Subscriber p50 < 5s, p99 < 30s under production traffic | Two weeks at p99 ≥ 60s |
| Self-healing detects soft drift (88-85%) within 1,000 obs and promotes without operator intervention | Operator forced to trigger A/B/C in any non-catastrophic case |
| Disk footprint stable at ≤ 110 GB under nightly GC with 115 GB hard quota | Quota exceeded > 24h without emergency eviction |
| Inner-LLM residue absent | Any commit reintroduces llama-cpp-2, Qwen pinning, or Mode B drip |

If any of these fails sustainably, the architecture is wrong and we
should revise — not paper over.

---

## 28. Telemetry and observability

### 28.1 Production telemetry windows

`ProductionTelemetryWindow` persists per window to
`CF_MEJEPA_PRODUCTION_TELEMETRY`:

- Conformal ECE.
- OOD AUC.
- Gτ pass rate.
- Prediction-oracle agreement.
- Ship-gate target / pass flag.
- p50 / p95 / p99 / max latency.
- VRAM free / required / total.
- Source identifier + corpus SHA.

FSV: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-production-telemetry-fsv/production-telemetry-evidence.json`.

### 28.2 Pairwise MI heatmap

CSV → PNG/markdown rendering via `PairwiseMiAuditor` + CSV writer +
`mejepa_audit_pairwise_mi` MCP tool. Weekly cron writes a report to
`/mnt/d/contextgraph/exports/eval/<date>/weekly.md` (Phase F target).

### 28.3 VRAM accounting

`vram.rs` uses CUDA-driver `cuMemGetInfo_v2` as the SoT. `nvidia-smi` is
explicitly diagnostic / non-authoritative under WSL. Hardened against
absent / unsupported / malformed `nvidia-smi` output.

### 28.4 Required fields in every log line

- UTC timestamp.
- Service / version / env.
- Request ID + session ID.
- Severity.
- Never logged: secrets, full PII, full tokens, full card numbers. Redact
  at the logger layer.

---

## 29. Model policy

| Role | Model | Notes |
|---|---|---|
| Optimizer (you, the outer-loop agent) | Opus | per `.claude/settings.json`. Architecture-level reasoning needs the headroom. |
| `cc-reality-diagnostician` | Sonnet | read-only critic |
| `cc-reality-reward-analyst` | Sonnet | read-only critic |
| `cc-reality-swe-failure-triage` | Sonnet | read-only critic |
| `cc-reality-source-of-truth-readback` | Sonnet | read-only verifier |
| `cc-reality-harness-editor` | Opus | single writer |

System runs locally on operator hardware; **no per-turn or per-day spend
caps**. Use as much reasoning depth and as many subagent calls as the
work warrants. `blocked` is for real obstacles (drift, MCP disconnect,
build failure), not budget. Per `feedback_no_cost_caps.md`.

---

## 30. Hardening — the 5 axes

Source: `docs2/hardeningprompt.md`. Hardening covers five orthogonal axes;
neglecting any one rots the system.

| Axis | Covers | Neglect failure |
|---|---|---|
| **Security** | config, attack surface, IAM, secrets, supply chain | breach, data loss |
| **Code quality** | bugs, dead code, redundancy, complexity, tech debt | slow velocity, regressions |
| **Architectural** | coupling, cohesion, SPOFs, scalability | costly rewrites |
| **Operational** | observability, runbooks, deploy/rollback, capacity | long outages |
| **Verification** | FSV, manual + synthetic tests | "worked on my machine" |

### 30.1 Mental models (install before tooling)

- **Trigger → Process → Outcome** (§10.2).
- **First-principles thinking** — decompose to smallest units, rebuild
  reasoning from there. Symptoms cluster; causes don't.
- **Source of Truth** — per state, exactly one authoritative location;
  read it directly to verify.
- **Symptom vs cause vs root cause** — patch / fix / hardening.
- **Fail-closed by default** — safe path is the default; never silently
  fall back to undefined state.
- **Defense in depth** — never trust a single control.

### 30.2 RCA — 5 Whys + first-principles

Goal: never "make symptom stop" — it's *the smallest structural change
that makes this class of failure impossible.*

Rules: use evidence (logs/timestamps/code), not opinion. "Someone forgot"
→ go deeper to *why did the system allow human memory to be the only
safeguard?* Validate by asking "would fixing this have prevented the
symptom?" — if not, dig more.

### 30.3 Tech-debt taxonomy

1. **Intentional/strategic** — knowingly taken to ship; documented payback.
2. **Unintentional** — most debt; inexperience/missing context/oversight.
3. **Outdated design** — yesterday's clever design is today's bottleneck.
4. **Environmental** — frameworks/libs/runtimes aged out.
5. **Architectural** — structural; the expensive kind.

### 30.4 Capability optimization

For every component, ask:

1. What does this contribute (user-visible terms)?
2. What does the system need from this (SLA/throughput/accuracy/security)?
3. What capability was originally intended (vs drift from intent)?
4. What's the max it could deliver if optimized? Gap = roadmap.

---

## 31. Common pitfalls (codified lessons)

Drawn from the project's auto-memory and prior session reports. Every
entry here has bitten this project at least once.

| Pitfall | Lesson |
|---|---|
| **`cargo check` ≠ release build.** Editor runs check automatically; never produces release binaries. | Always run `cargo build --release` after a `changed` recommendation. |
| **CodeStore cosine returned raw `[-1,1]`** while all other paths returned `[0,1]`. | Verify every new cosine path applies `(raw+1)/2`. |
| **PipelineBuilder.k() was dead code** — builder cloned config but never propagated it. | Verify builder patterns actually propagate all settings. |
| **Inverted index remove must sort legacy data.** Remove path used raw deserialize while update path sorted; concurrent ops corrupt unsorted lists. | Sort on read for any legacy data path. |
| **bincode + `skip_serializing_if` = BROKEN.** | Use JSON for all provenance / `SourceMetadata`. |
| **RwLockReadGuard (parking_lot) is `!Send`.** Must drop before `.await` in `tokio::spawn`. | Drop the guard before any await. |
| **`with_custom_weights()` does NOT auto-set MultiSpace strategy.** | Set strategy explicitly. |
| **RocksDB requires ALL on-disk CFs to be opened.** Delete old DBs when removing CFs. | Wipe DB on CF schema change. |
| **E5 is STRUCTURAL (causal markers), not TOPICAL.** Can't improve within-domain ranking. | Don't expect E5 to disambiguate same-topic content. |
| **Gate thresholds must be recalibrated when training changes.** Score distributions shift. | After any training change, run threshold recalibration. |
| **Benchmark E1 provider MUST panic on failure** — silent 0.0 corrupts results. | Fail fast in benchmarks. |
| **`floor_char_boundary`/`ceil_char_boundary` for safe string slicing.** `&str[..n]` panics on multi-byte chars (Rust 1.80+). | Always use the safe methods on UTF-8 input. |
| **Train/inference prefix MUST match.** nomic-embed uses "search_document:" prefix. | Audit prefix matching when changing embedders. |
| **`hard_negative_scale >= 2.0` with `temperature=0.05`** causes catastrophic collapse. Use 1.0. | Tune scale/temp pair together. |
| **`Option<T>::unwrap_or_default()` is almost always a bug in this codebase.** | Use explicit error or named default. |
| **CF row written but never flushed.** Add `db.flush_cf(cf)` before readback; reopen the DB in a separate handle. | Always sync + flush + reopen for FSV. |
| **Test that uses a different RocksDB instance than production.** | Use the actual `open_hygiene_rocksdb` (or equivalent production opener) in the FSV harness. |
| **Async tests with no await on the producer side.** | If you spawn a task to write, join the task before reading. |
| **Schema-loose JSON deserialization swallowing field renames.** | Set `#[serde(deny_unknown_fields)]` on every persisted struct. |
| **Adding trait methods requires updating ALL impl blocks** (RocksDB + InMemory stub). | Grep for every impl when extending traits. |
| **MCP schema max/min don't enforce server-side.** | Always validate in handler code too. |
| **`additionalProperties:false` blocks unlisted params.** MCP schema enforcement means handler params NOT in schema are silently dropped. | Audit schema↔DTO alignment. |
| **Alive process ≠ serving process.** `kill(pid,0)==0` only checks process-table entry. In daemon mode also verify health on expected port. | Health check, not just liveness. |
| **`kill(pid,0)` returns 0 for zombies** — after SIGKILL process may be zombie (flock released!) while kill still reports alive. | Check `/proc/pid/status` for `State: Z`. |
| **Standalone (non-daemon) MCP processes can get stuck.** stdio MCP should exit on EOF but background threads (RocksDB, HNSW) can keep it alive. | Implement explicit shutdown signals. |
| **E2 uses sinusoidal PE.** Old decay encoding produced identical vectors (delta≈0). Rebuild recomputes E2 from `fp.created_at`. | Persisted E2 HNSW skipped on startup to force rebuild. |
| **E7 raw cosine entered fusion unormalized.** `code/mod.rs` returned `[-1,1]` while all other embedders return `[0,1]`. | Apply `(raw+1)/2` before fusion. |
| **`session_start` uses in-memory `SessionCache` not RocksDB.** Invalid DB path doesn't cause failure. | Distinguish in-memory caches from persistent storage. |
| **`read_line_bounded` must drain remainder.** After rejecting over-limit line, drain to `\n` or EOF to prevent phantom next-line. | Always consume to delimiter on rejection. |
| **Background task compaction can race with concurrent inserts.** HNSW rebuild + insert may create duplicate usearch entries. | Hold compaction lock during inserts. |

---

## 32. Operator-driven skills and slash commands

When the operator types `/<skill-name>`, the corresponding skill runs.
Only invoke skills that appear in the available-skills list — don't guess.

### 32.1 ccreality skills (operator-driven)

- `/ccreality-iterate` — run one full optimizer turn (the workhorse).
- `/ccreality-smoke` — verify the full pipeline without Docker.
- `/ccreality-trigger-check` — read-only check on whether to iterate.
- `/ccreality-readback` — drift detection on past recommendations.

### 32.2 Common skills

- `/commit` — commit and push all changes to the current branch.
- `/pushrepo` — commit and push with validation.
- `/sitrep` — generate honest SITREP document in `./tmp/`.
- `/init` — initialize a new CLAUDE.md file with codebase documentation.
- `/review` — review a pull request.
- `/security-review` — complete a security review of pending changes.
- `/loop <command>` — run a prompt / slash command on a recurring interval.
- `/schedule` — create / update / list / run scheduled remote agents.

### 32.3 Custom domain skills

The `.claude/skills/` directory holds project-specific skills. Familiarize
yourself with the skill list at session start.

---

## 33. Reference doc map

### 33.1 Canonical project rules

- `CLAUDE.md` — takes precedence over anything else. Project instructions
  loaded into every session.
- `docs/MEJEPA_SUPER_PROMPT.md` — this document.

### 33.2 The futurebuild plan (27 docs)

| # | Document | Purpose |
|---|---|---|
| 00 | `docs/futurebuild/00_INDEX.md` | Reading order + domain map |
| 01 | `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md` | The thesis: ME-JEPA replaces code review |
| 02 | `docs/futurebuild/02_CURRENT_STATE_HONEST.md` | What's shipped — verified 2026-05-12 |
| 03 | `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md` | Edits → embeddings → panels → ME-JEPA → verdicts |
| 04 | `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md` | Multi-embedder labeling pipeline |
| 05 | `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md` | Routing table; which embedders carry signal |
| 06 | `docs/futurebuild/06_REALITY_LOOP_REFACTOR.md` | Single outer-loop design |
| 07 | `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md` | Edit → panel → corpus row |
| 08 | `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md` | MCP tool surface for agents |
| 09 | `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md` | Observe + EWC++ + drift + A/B/C |
| 10 | `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` | UTML v1.0 operationalized |
| 11 | `docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md` | What was deleted |
| 12 | `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` | Phase rollout + ship gates |
| 13 | `docs/futurebuild/13_TASK_INDEX.md` | Atomic task DAG |
| 14 | `docs/futurebuild/14_NEW_LOOP_MD.md` | The refactored `.claude/loop.md` content |
| 15 | `docs/futurebuild/15_PREDICTION_TAXONOMY.md` | Every prediction ME-JEPA emits |
| 16 | `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` | D-drive only for bulk state |
| 17 | `docs/futurebuild/17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` | Audit of what 00–16 missed |
| 18 | `docs/futurebuild/18_INTEGRATION_AND_E2E_TESTING.md` | E2E / chaos / load / soak strategy |
| 19 | `docs/futurebuild/19_OPERATOR_RUNBOOK.md` | Daily / weekly / incident operations |
| 20 | `docs/futurebuild/20_DISASTER_RECOVERY_AND_BACKUP.md` | Backup + restore + DR drill |
| 21 | `docs/futurebuild/21_COLD_START_AND_BOOTSTRAP.md` | Fresh-deploy + baseline checkpoint |
| 22 | `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` | Adversarial threats + defenses |
| 23 | `docs/futurebuild/23_MULTI_AGENT_ISOLATION.md` | Multi-agent / multi-tenant isolation |
| 24 | `docs/futurebuild/24_API_STABILITY.md` | Semver / deprecation / replay |
| 25 | `docs/futurebuild/25_FUTURE_DOMAINS_MEJEPA_X.md` | Template for new modalities |
| 26 | `docs/futurebuild/26_DOCUMENTATION_MAP.md` | Cross-repo navigation + glossary |

### 33.3 Spec hierarchy

- **Level 0 — Verification:** `docs/futurebuild/specs/FSV-PROTOCOL.md`
  (mandatory inherited contract).
- **Level 1 — Constitution:** `docs/ruvectorfindings/specs/constitution.md`.
- **Level 2 — Functional:** `docs/futurebuild/specs/functional/*.md`.
- **Level 3 — Technical:** `docs/futurebuild/specs/technical/*.md`.
- **Level 4 — Tasks:** `docs/futurebuild/tasks/<domain>/*.md`.

### 33.4 Master plan docs

- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — master plan.
- `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` — pivot statement.
- `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` — UTML v1.0 JEPA edition.
- `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` — embedder doctrine.
- `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` — TCT.
- `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` — AST chunking.
- `docs/ruvectorfindings/15_PHASE_1_5_HARDENING_FSV_2026_05_10.md` — Phase 1–5 hardening evidence.
- `docs/ruvectorfindings/16_PHASE_6_8_HARDENING_FSV_2026_05_11.md` — Phase 6–8 hardening evidence.

### 33.5 External best-practice references

- `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md` — SAMP.
- `/home/cabdru/leapablememory/docs2/SHARED-MIND-AGENT-PROTOCOL.md` — SMAP.
- `/home/cabdru/contextgraph/docs2/hardeningprompt.md` — 5-axis hardening.
- `/home/cabdru/contextgraph/docs2/flagprompt.md` — issue flagging.
- `/home/cabdru/contextgraph/docs2/prdtospec.md` — 4-level spec hierarchy.
- `/home/cabdru/contextgraph/docs2/TheoryOfMachineLearning.md` — UTML formula derivation.

### 33.6 Per-tick prompt

- `.claude/loop.md` — canonical operational spec for one turn.
- `.claude/agents/cc-reality-*.md` — individual agent definitions.
- `agents/README.md` — SMAP roster.

---

## 34. End-of-turn checklist

Before declaring the turn complete:

- [ ] **FSV evidence** captured at `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json`
      with `all_passed: true`, `cf_counts_before`/`cf_counts_after`,
      `readback_equal: true`, ≥3 boundary cases.
- [ ] **Physical artifacts** verified — SST files exist with non-zero size, or
      file `wc -c` + `sha256sum` recorded, or metric increment confirmed.
- [ ] **Independent re-open verification** for any RocksDB write (close handle,
      reopen, verify same row).
- [ ] **Build + test + clippy** clean. `cargo build --release -p <crate>`
      succeeds. `cargo test -p <crate>`: zero failed, zero ignored. `cargo
      clippy -p <crate> --no-deps --tests --examples -- -D warnings` clean.
- [ ] **Memory updates** written to `./memory/` immediately when discoveries /
      decisions / blockers / patterns emerged — not batched at the end.
- [ ] **Session report** at `./memory/reports/session-YYYY-MM-DD--<slug>.md`
      with the SAMP §3.5 template, including the absolute path of every
      FSV artifact, the command that reproduced it, the post-build SHA,
      and the test/clippy output lines.
- [ ] **MEMORY.md** index entry added/updated; entry one line under 200 chars.
- [ ] **GitHub issues** filed for any observed defect / smell / dead-code /
      tech-debt / anomaly / security / architecture / test-gap you are NOT
      fixing this turn (§12). Run dedupe before filing.
- [ ] **Recommendation JSON** recorded via
      `mcp__cgreality__optimizer_record_recommendation` if the reality
      loop is running (Stop hook enforces). Status `changed` / `no_change`
      / `blocked`, with `source_of_truth` paths prefixed `file:`.
- [ ] **Resolved blockers** still in `./memory/blockers/` with
      `status: resolved`. Superseded decisions still in `./memory/decisions/`
      with `status: superseded` and a forward pointer.
- [ ] **Three consecutive `blocked` for the same root cause** → escalate
      (high-severity blocker + `remaining_risks: ["requires human inspection: …"]`).
- [ ] **Subagent reviews** done — code-simplifier on every shipped primitive
      before merging; no BLOCKER refinements outstanding.

---

## Single-line summary

ME-JEPA-Code is the deterministic predictive truth system that returns a
calibrated, multi-head `RealityPrediction` for any AI-generated code change
so human review becomes statistically unjustified once
`prediction_oracle_correlation ≥ 0.95` stably; every primitive must be
generic (no per-task overfit), every change must verify by source-of-truth
readback (FSV), every memory must be written to `./memory/` immediately,
every observed defect not fixed this turn must become a GitHub issue, and
every turn must end with both an FSV evidence file and a session report.

---

**End of super prompt.** Approximate length: ~1900 lines. Read once at
session start; the rules here override default agent behavior.

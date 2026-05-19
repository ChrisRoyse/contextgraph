# CLAUDE.md — ContextGraph / ME-JEPA-Code project addendum

You are working in the **ContextGraph** repository, building **ME-JEPA-Code**: the deterministic predictive truth system that lets AI coding agents stop generating unverified claims and start producing patches a calibrated verifier can trust. This file is loaded automatically into every Claude Code session that opens this directory.

**Master rules (universal protocol):** [`docs2/AICodingAgentSuperPrompt.md`](docs2/AICodingAgentSuperPrompt.md) — the AI Coding Agent Doctrine. Read it once at session start. It is the canonical, density-first reference for memory (GitHub Issues), claiming, FSV, RCA, hardening, communication, session lifecycle, and the forensic + simplification disciplines.

**This file (CLAUDE.md)** is the project-specific addendum. When this file and the doctrine agree, follow either. When they conflict, **the doctrine wins for protocol detail** and **this file wins for project-specific invariants** (paths, crates, deny-list, current phase, build commands, MEJEPA architecture).

---

## 1. What we are building

**ME-JEPA-Code** is a calibrated multi-head `RealityPrediction` system answering five operator questions for any code change an AI agent produces:

| Q | Question | How ME-JEPA answers |
|---|----------|---------------------|
| **Q1** | Does what the AI claimed it did **EXIST**? | Claim graph reconciled against the SHA chain on disk |
| **Q2** | Does it **WORK**? | Predictor + conformal interval + TCT Gτ guard → `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}` |
| **Q3** | **WHY** would it fail? | Ranked `predicted_failure_modes`, predicted test flips, violating embedders, closest exemplars |
| **Q4** | **WHAT ELSE** could go wrong? | Always-on heads: edge cases, latent bugs, tech debt, dead code, redundancy, perf, security, accuracy, cost |
| **Q5** | How is it **IMPACTING REALITY**? | Shift log replayed against predicted side-effects → `RealityImpact` |

**Ship-gate predicate:** `prediction_oracle_correlation ≥ 0.95` stably across `(mutation_category × language)` on four consecutive rolling windows of the 300×8 SWE-bench Lite corpus. When that's met, human review of AI-generated patches is statistically unjustified — the verifier *is* the system.

ME-JEPA runs as an **MCP server** locally on operator hardware. AI agents call it via JSON-RPC stdio. Underneath, a continuous self-optimization daemon consumes every Edit shift, every test outcome, and every agent-feedback event to retrain itself online.

**Product boundary.** ME-JEPA is the product. LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`. A feature belongs in the architecture only if it captures durable reality, densifies frozen target panels, improves predictor calibration/coverage/OOD, converts prediction mistakes into replay weight, or proves to the operator (by SoT readback) that the system functions as claimed.

---

## 2. Verified phase status (2026-05-13)

**Independent audit completed 2026-05-12.** 35/35 Phase A–D items PRESENT against the live tree; 16/16 FSV evidence files present on the then-current local `/mnt/d/contextgraph/fsv/` evidence root; all 17 filed GitHub issues (#3–#19) including umbrella gate [#15](https://github.com/ChrisRoyse/contextgraph/issues/15) CLOSED. **Phase E entry is unblocked.**

The only outstanding gate is `prediction_oracle_correlation = 0.866667 < 0.95`; older local evidence cites `/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`. That is Phase G model-quality convergence, not a Phase A–D implementation gap. Phase E's job is to generate the learning pressure that closes the 0.95 gate.

**Post-migration storage correction (2026-05-18):** #363 supersedes the old D-drive durable-SoT rule for production Phase G work. Current production corpus, runtime, project-ingest, cache, model-output, and FSV artifacts must live on aiwonder under `/zfs/archive/contextgraph/...`; hot work must live under `/zfs/hot/contextgraph/...` or explicitly budgeted aiwonder scratch. Local `/mnt/d/contextgraph/...` is legacy/local calibration evidence unless a GitHub issue explicitly scopes a local-only smoke. #369 tracks revalidation of post-migration artifacts that still cite `/mnt/d`.

**Current work source.** GitHub Issues are the live source of truth for next work. Read the pinned `type:context` issues first, then `docs/futurebuild/README.md` for the current 8-doc map and `docs/futurebuild/08_roadmap_and_open_work.md` for the latest locally synced roadmap.

- #310 is the active meta/context issue for Phase G queue ordering.
- #227 is the active runtime-policy context issue for Python ME-JEPA data work.

The historical 2026-05-12 audit trail lived at `./memory/discoveries/2026-05-12--phase-a-d-independent-audit-zero-gaps.md`. That namespace is now archived under `./memory.archive/`. Any audit-trail entries needed going forward become `type:discovery` GitHub issues per the doctrine §3.

---

## 3. The crates

- **`context-graph-mcp`** — MCP server. Speaks JSON-RPC over stdio. Exposes the 28 live `mejepa_*` tools plus compatibility `mcp__cgreality__*` source-of-truth transport. Owns path-policy, edit-lock, ledger, and shift-log machinery. Fails closed on retired reality-loop calls (`CCREALITY_ENGINE_RETIRED`).
- **`context-graph-cli`** — Harness CLI, DynamicJEPA commands, governed readback helpers, ME-JEPA implementation work.
- **`context-graph-mejepa`** — Predictor, compiler, heal scheduler, eval. The trunk model.
- **`context-graph-mejepa-cf`** — RocksDB column-family registry (51+ CFs).
- **`context-graph-mejepa-corpus`** — Mutation operators (8 × 11 languages), corpus CLI, SWE-bench bridge.
- **`context-graph-mejepa-embedders`** — 21 embedder slots; `EmbedderForward` trait; 12 active content slots (8 pretrained + 4 algorithmic).
- **`context-graph-mejepa-hygiene`** — Tier compression, quota, eviction, GC, witness Merkle compression.
- **`context-graph-mejepa-instruments`** — 15 frozen instruments. The panel is an **array of 15 per-embedder vectors** (a teleological constellation), one vector per slot, each vector living in its own embedder's space. The 5,120 number is contiguous-serialization byte layout only; cross-embedder dim-wise comparison is forbidden — only same-embedder dim-wise compares (E1↔E1, E_AST↔E_AST, etc.), array-level operations on the structured array (per-slot cosine map, similarity matrices, matrix-style projections that respect slot identity), and scalar pairwise cross-slot relationships (MI, correlation, agreement).
- **`context-graph-mejepa-shift-subscriber`** — Tokio task tailing the shift log; materializes panels.
- **`context-graph-mejepa-tct`** — Teleological constellations + Gτ guard.
- **`context-graph-mejepa-train`** — Trainer + UTML formula carriers + DDA computation + samplers.
- **`context-graph-{core,mincut,solver,tier-compression,witness,benchmark}`** — Supporting primitives.

Legacy `reality-loop` binary and inner-actor LLM (Qwen3.6-27B / Hermes-2-Pro / Ollama / llama.cpp generative inference) were deleted 2026-05-09 and Phase H FSV-verified on 2026-05-13 (#134/#135). **Do not reanimate without explicit operator authorization.**

---

## 4. Memory = GitHub Issues (per doctrine §3)

**There is no `./memory/` directory.** The old SAMP/SMAP namespace is archived to `./memory.archive/` (read-only history; do not write new files there). New memory is captured exclusively in this repo's GitHub Issues:

| Old `./memory/` namespace | New home |
|---|---|
| `context/` (mission / current phase / scope) | Pinned `type:context` issue |
| `decisions/` (locked architectural choices) | Closed `type:decision` issue (ADR; doctrine §3.10) |
| `discoveries/` (constraints / gotchas / edge cases) | Closed `type:discovery` issue (doctrine §3.11) |
| `patterns/` (reusable conventions) | Closed `type:pattern` issue (or `AGENTS.md` for stable ones) |
| `blockers/` (unresolved walls) | Open issue with `status:blocked` |
| `handoff/` (deliverable for next agent) | Comment on relevant issue + assignee change |
| `journal/` (chronological execution log) | Issue comments (CLAIM / milestone / PAUSE / RESOLVED / BLOCKED) |
| `reports/` (end-of-session summary) | Final RESOLVED comment on the worked issue + PR description |

Label set: see [`.github/labels.yml`](.github/labels.yml). Bootstrap with `scripts/gh-labels-bootstrap.sh`. The six startup queries to run every turn are in [`AGENTS.md`](AGENTS.md) and doctrine §3.3.

---

## 5. Behavioral rules (always enforced; additive to doctrine §1)

1. Do what has been asked; nothing more, nothing less.
2. NEVER create files unless absolutely necessary.
3. ALWAYS prefer editing an existing file to creating a new one.
4. NEVER proactively create documentation files (`*.md`) or READMEs unless explicitly requested.
5. NEVER save working files, ad-hoc tests, or scratch markdown to the project root. Use `/tmp/` for ephemeral scratch, aiwonder `/zfs/archive/contextgraph/...` for production durable artifacts, or open a GitHub issue + comment (§7).
6. ALWAYS read a file before editing it.
7. NEVER commit secrets, credentials, or `.env` files.
8. After ANY correction from the operator, capture the lesson as a `type:decision` (locked rule) or `type:discovery` (finding) GitHub issue so future agents inherit it.
9. Do file-reading, synthesis, code edits, and decisions personally. Own every step.

---

## 6. Architectural invariants (project-specific; inviolable)

These are non-negotiable and project-specific. Doctrine universal rules still apply.

1. **Structural collapse-immunity on the target side.** Frozen instruments have ZERO trainable parameters. `ΔΞ.target_collapse = 0` by construction. Never wire a gradient into the frozen-target path; the `MEJEPA_INSTRUMENT_GRADIENT_LEAK` guard is correct behavior.
2. **Reality-shift principle.** No alteration without current state. No alteration without a delta back into perception. Pre/post-tool hooks enforce this.
3. **GENERIC-ONLY in the trained model.** Every trained primitive must generalize across every SWE-bench task, every Python repo, every bug class. Mental check: *"would this be load-bearing for `psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895` and any future repo?"* You may reason task-specifically when attempting a SWE-bench task; never bake task-specifics into the trained model.
4. **Single outer loop around the invoking agent.** No inner LLM. The agent (Claude / Codex / Cursor) IS the actor. Its Edit / Write / Bash / tests ARE the data-generating process.
5. **aiwonder only for post-migration production durable state.** Production corpora, FSV evidence, runtime shift logs, test outcomes, project-ingest outputs, and weekly exports live under `/zfs/archive/contextgraph/...` on aiwonder; hot staging lives under `/zfs/hot/contextgraph/...` or explicitly budgeted aiwonder scratch. Local `/mnt/d/contextgraph/...` is legacy/local evidence unless an issue explicitly requests a local-only smoke.
6. **No backwards compatibility.** Pre-production tree. Wipe DB on schema change; fail fast with structured errors; never write migration shims.
7. **Upstream reference-only — clean-room.** Never vendor / path-dep / npm-depend on `ruvnet/agentdb` or `ruvnet/ruvector`. Reimplement borrowed ideas in our own crates.
8. **No HTTP from action paths.** MCP server is the only network surface. `WebSearch` / `WebFetch` / `mcp__exa__*` are read-only research tools. No `curl` / `wget` / `fetch` in action code paths.
9. **No destructive git.** Forbidden: `push --force`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`. Plain `git push` (fast-forward) is allowed.
10. **Frozen embedder weights.** E1, E6, E7, E8, E10, E12, E13, E14 are frozen by design. Do not retrain in the live loop.
11. **No cost caps.** System runs locally; no per-turn or per-day spend caps. `blocked` is for real obstacles, not budget.

---

## 7. Path discipline (BEHAVIORAL — gates removed 2026-05-07)

The MCP-server-side path-policy gate and pre-edit hook deny-list were both removed by operator directive. Honor these don't-touch paths behaviorally:

- `.env` and `.env.*` anywhere
- `swebench/**` (the upstream evaluator)
- `tests/oracle_**/**` (benchmark fixtures)
- `crates/context-graph-benchmark/datasets/swe_bench_*/**`
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- Any `*.pem`, `*.key`, `**/credentials.json`

If editing one of these is genuinely the right thing for an operator-directed task, ask the operator first.

---

## 8. Directory layout

```
./crates/         — Rust workspace (the source tree)
./docs/           — futurebuild plan + ruvectorfindings + MEJEPA_SUPER_PROMPT.md (project narrative; SUPERSEDED for protocol)
./docs2/          — AICodingAgentSuperPrompt.md (master doctrine) + hardeningprompt.md (14-axis deep-dive) + flagprompt.md (SUPERSEDED issue protocol) + prdtospec.md
./scripts/        — CC hook scripts (scripts/cc/), CI lint (scripts/ci/), setup, systemd, label bootstrap (gh-labels-bootstrap.sh)
./tasks/          — workstream manifests (e.g., tasks/swebench-lite/*.json)
./config/         — governed paths, model pins
./.claude/        — settings.json, agents/, loop.md, skills/
./.github/        — workflows + labels.yml
./memory.archive/ — read-only legacy SAMP/SMAP narrative (no new entries; see STOP_READING.md)

# aiwonder production state (post-migration durable bulk state)
/zfs/archive/contextgraph/
├── corpus/                              # Production corpus/cache/fingerprint artifacts
├── fsv/                                 # Production per-task FSV evidence
├── runtime/                             # Durable runtime evidence and shift/test outcomes
├── projects/                            # Project-ingest cache/panels/predictions/reports
├── state/                               # Durable state that must survive restarts
└── exports/                             # Weekly eval reports and operator exports

# aiwonder hot state
/zfs/hot/contextgraph/
├── repo/                                # aiwonder Git checkout and release binaries
├── models/                              # Embedder model artifacts
├── corpus/                              # Stable hot inputs
└── runtime/                             # Active scheduler logs and hot run state

# local legacy/calibration evidence
/mnt/d/contextgraph/                     # Historical/local evidence only unless issue-scoped
```

CI lint `scripts/ci/check_no_c_drive_paths.sh` enforces no C-drive paths in code or FSV files.

---

## 9. Per-turn protocol (project overlay on doctrine §12)

1. **READ** (doctrine §3.3 + §12.1) — Run the six issue queries to see pinned context / in-progress / blocked / unclaimed agent queue / active decisions / discoveries touching your task area. Skim `AGENTS.md` and this file.
2. **Read current futurebuild docs.** `docs/futurebuild/README.md` maps the current 8-doc suite; read `01_overview.md`, `07_results_and_benchmarks.md`, and `08_roadmap_and_open_work.md` for current state, proof history, and open work.
3. **Check GitHub state** — `gh issue list --state open --label "status:in-progress"` (avoid these), `--label "source:agent" --search "no:assignee"` (your candidate queue).
4. **Pick** the highest-priority incomplete deliverable. Or follow operator direction.
5. **Claim** the issue (if existing) per doctrine §3.4: atomic `gh issue edit` assigning self + flipping `status:needs-triage` → `status:in-progress` + CLAIM comment listing files-touched and ETA.
6. **Audit the task** against the live tree per `docs/futurebuild/specs/FSV-PROTOCOL.md` §7. `ASSUME THE TASK IS WRONG`. Examine code BEFORE reading claims (Linear Sequential Unmasking, doctrine §2.8).
7. **Plan.** Trigger → Process → Outcome (doctrine §2.2). Name the SoT. List ≥3 boundary cases.
8. **Work.** Ship one primitive end-to-end. Comment milestone updates on the issue (doctrine §3.5) — discovery / direction-change / heartbeat / sibling-issue. Never silent.
9. **Verify** per doctrine §4: `cargo build --release -p <crate>`, `cargo test -p <crate>`, `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings`. Production FSV harnesses write to `/zfs/archive/contextgraph/fsv/<task>-fsv/<artifact>.json` on aiwonder, with independent re-open readback for any RocksDB write. `/mnt/d/contextgraph/fsv/...` is local-only unless the issue explicitly says so.
10. **Simplify** (doctrine §17.1 / per-PR gate): files ≤500 lines, functions ≤30 lines, cyclomatic ≤10; avoid nested ternaries; prefer `match` / `if let` / explicit `if / else if / else`.
11. **Forensic pass** (doctrine §11) for non-trivial primitives: Cold Read, Contradiction Engine, ≥3 hypotheses, LSU bias check.
12. **Close out** (doctrine §3.6–§3.8) — comment RESOLVED / PAUSE / BLOCKED on each worked issue. RESOLVED comment includes: fix summary (root cause + structural fix), build/test/clippy status, FSV evidence (SoT before → action → SoT after with values), regression test name, side effects observed, follow-up issues filed.
13. **Observe & file** (doctrine §3.2 / §3.12) — any defect / smell / dead-code / tech-debt / anomaly / security / architecture / test-gap you observed but are NOT fixing → open an Issue before turn ends. Dedupe first (doctrine §3.14).

**No `./memory/*.md` reads or writes.** That directory is archived.

---

## 10. FSV protocol — the verification contract

Binding contract for every task. Universal protocol: doctrine §4. Project-specific operational detail: `docs/futurebuild/specs/FSV-PROTOCOL.md`. The doctrine summary:

> *A return value is a claim. The Source of Truth is the verdict. Read the verdict.*

Before declaring a task complete:

1. What state changed?
2. Where does that state live? (CF + key / file path / queue topic / metric / external system record)
3. How do I read it back independently?
4. What is the exact expected value?

If you cannot answer all four, you don't understand the feature well enough to call it done. **No mocks** in tests that gate task completion. **No backwards compatibility.** **No workarounds.** **Tests must fail when the feature is broken.** **Fail closed on unknowns** with `SCREAMING_SNAKE_CASE` codes. **Assume the task is wrong** — audit every file path / symbol / CF reference against the live tree before following it.

Required evidence per task: build output, test output (zero failed, zero ignored), clippy output (`-D warnings`), SoT readback JSON, physical artifact verification, independent re-open verification (RocksDB only), final RESOLVED issue comment with the evidence table.

---

## 11. GitHub coordination — File rule + Claim rule (doctrine §3.2)

**Two non-negotiable rules:**

1. **File rule.** Observe a defect / smell / dead-code / tech-debt / anomaly / security / architecture / test-gap / decision / discovery / pattern you are NOT capturing in code this turn → open a GitHub Issue before turn ends. Dedupe first (3–6 distinctive keywords, `gh issue list --state all --search "<kw> in:title,body"`). Threshold ≥8/10 → comment on existing, don't file.
2. **Claim rule.** Before touching code tied to an issue: assign self, flip `status:needs-triage` → `status:in-progress`, post a CLAIM comment. Comment at every milestone (Discovery / Direction change / Sibling issue / Heartbeat). When pausing: PAUSE comment (Done / Tried-failed / Learned / Resume-at / Hypothesis). When done: RESOLVED comment (Fix summary / Verification / Side effects). Blocked: BLOCKED comment with link to blocker.

**No silent work.** Never edit files referenced by another agent's `status:in-progress` issue without commenting first (doctrine §10).

**GitHub Free-plan budget ($0/mo).** Workflows must `runs-on: ubuntu-latest` only; declare `timeout-minutes:` (≤30 default, ≤60 cap); pin Actions to SHA (not tag); cron at most weekly for SAST, daily for stale. Never enable paid features. See doctrine §3.20 for full allow/deny list.

---

## 12. Hardening — the 14 axes

Universal summary: doctrine §14. Project-specific deep-dive: [`docs2/hardeningprompt.md`](docs2/hardeningprompt.md). When the doctrine summary suffices, use it. When you need primitive lists, threat-model templates, or per-axis checklists, use hardeningprompt.md.

| # | Axis | Where |
|---|------|-------|
| 1 | Security | doctrine §14.2 / hardeningprompt §3 |
| 2 | Correctness & accuracy (FSV) | doctrine §4 (universal contract) |
| 3 | Performance | doctrine §14.4 / hardeningprompt §5 |
| 4 | Reliability (SRE) | doctrine §14.5 / hardeningprompt §6 |
| 5 | Resilience (fault tolerance) | doctrine §14.6 / hardeningprompt §7 |
| 6 | Scalability | doctrine §14.7 / hardeningprompt §8 |
| 7 | Cost efficiency | doctrine §14.8 / hardeningprompt §9 |
| 8 | Architecture | doctrine §14.9 / hardeningprompt §10-§11 |
| 9 | Code quality & maintainability | doctrine §14.10 / hardeningprompt §11 |
| 10 | Data layer (RocksDB hardening) | doctrine §14.11 / hardeningprompt §12 |
| 11 | Observability | doctrine §14.12 / hardeningprompt §13 |
| 12 | Supply chain integrity | doctrine §14.13 / hardeningprompt §14 |
| 13 | AI / ML / LLM-specific | doctrine §14.14 / hardeningprompt §15 |
| 14 | Benchmarking discipline | doctrine §14.15 / hardeningprompt §16 |

Universal mental models: Trigger → Process → Outcome; first-principles decomposition; symptom vs cause vs root cause; fail-closed not fail-open; defense in depth; asymmetry of risk; 80/20 priority (doctrine §2).

RCA: 5 Whys → Fishbone → Fault Tree Analysis → first-principles debugging (doctrine §5). Output root cause as a **system property** ("the system allowed X because Y"), not "human error."

---

## 13. Forensic + simplification disciplines

**Forensic discipline.** Universal protocol: doctrine §11. Apply when verifying claims, auditing code, validating implementations: guilty until proven innocent; abductive reasoning (≥3 hypotheses); Linear Sequential Unmasking; Cold Read; Contradiction Engine; adversarial personas; investigation tiers (Glance/Scan/Investigate/Deep Dive); self-verification before any GUILTY verdict.

**Code-simplification discipline.** Universal protocol: doctrine §16 / §17.1 / pre-completion check. Apply on every shipped primitive before merging: preserve functionality; project standards (files ≤500 lines, functions ≤30 lines, cyclomatic ≤10); reduce nesting; avoid nested ternaries; choose clarity over brevity; maintain balance.

---

## 14. Build, test, lint

```bash
cargo build --release -p context-graph-cli      # workhorse CLI
cargo build --release -p context-graph-mcp      # MCP protocol shim
cargo build --release -p <touched-crate>        # after any change
cargo test                                       # full suite
cargo test -p <touched-crate>                    # scoped
cargo clippy --no-deps --tests --examples -- -D warnings
cargo fmt
```

**`cargo check` is NOT a substitute for `cargo build --release`.** Always run release build after a `changed` recommendation.

---

## 15. Project-specific hard "never"s (additive to doctrine §15-equivalent)

These are on top of the doctrine's universal forbidden list.

1. Never reintroduce the retired inner-actor LLM stack (Qwen, Hermes, Ollama, llama.cpp generative inference) without explicit operator authorization and new FSV evidence.
2. Never call HTTP from action paths (curl / wget / fetch). MCP is the only action surface.
3. Never edit `.env`, `swebench/**`, `tests/oracle_**/**`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**` without operator OK.
4. Never use destructive git (`push --force`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`).
5. Never write `./memory/*.md` files. Memory is GitHub Issues (doctrine §3).
6. Never cater a trained primitive to a specific task (GENERIC-ONLY rule §6.3).
7. Never trust a return value as evidence of work — read the SoT (doctrine §0 / §4).
8. Never add cost caps or dollar budgets.
9. Never use mock data in tests that gate task completion.
10. Never write tests that pass when the project is broken.

Full universal list: doctrine §1, §7, §8, §15-equivalent.

---

## 16. Concurrency: 1 message = all related operations

All independent tool calls in a single response MUST be made in one message. Batch ALL todos in ONE TaskCreate call (5+ minimum if creating a batch). Batch ALL file reads/writes/edits in ONE message. Batch ALL Bash commands that don't depend on each other in ONE message. When operations have dependencies, sequence them — don't fake parallelism.

---

## 17. Core principles

- **Simplicity first.** Make every change as simple as possible. Touch minimal code.
- **No laziness.** Find root causes; no temporary fixes; senior-developer standards.
- **Minimal impact.** Changes touch only what's necessary; avoid introducing bugs in unrelated code.
- **Document failure as carefully as success — in issue comments.** Failure is the next agent's lesson.
- **GitHub Issues are the single source of truth for memory across agents** (doctrine §3). If it isn't in Issues, it dies with the session.
- **`/zfs/archive/contextgraph/` on aiwonder is the production durable SoT** for post-migration FSV evidence. Local `/mnt/d/contextgraph/` evidence does not count for production closeout unless the issue explicitly requests a local-only smoke.

---

## 18. Support

- **Doctrine (master):** [`docs2/AICodingAgentSuperPrompt.md`](docs2/AICodingAgentSuperPrompt.md)
- **Project pointers:** [`AGENTS.md`](AGENTS.md)
- **Futurebuild plan:** [`docs/futurebuild/README.md`](docs/futurebuild/README.md)
- **FSV contract:** [`docs/futurebuild/specs/FSV-PROTOCOL.md`](docs/futurebuild/specs/FSV-PROTOCOL.md)
- **14-axis hardening deep-dive:** [`docs2/hardeningprompt.md`](docs2/hardeningprompt.md)
- **Per-tick prompt:** [`.claude/loop.md`](.claude/loop.md)
- **Legacy MEJEPA narrative (SUPERSEDED for protocol; project content still valid):** [`docs/MEJEPA_SUPER_PROMPT.md`](docs/MEJEPA_SUPER_PROMPT.md)
- **Legacy issue protocol (SUPERSEDED by doctrine §3):** [`docs2/flagprompt.md`](docs2/flagprompt.md)
- **Legacy SAMP/SMAP narrative (read-only history):** [`./memory.archive/`](./memory.archive/)

---

**One-line summary.** ME-JEPA-Code is the deterministic predictive truth system that returns a calibrated multi-head `RealityPrediction` for any AI-generated code change so human review becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` stably across 4 rolling windows on the 300×8 SWE-bench Lite corpus — every primitive must be generic, every change must verify by source-of-truth readback, every memory must be captured in a GitHub Issue (claim before working — no silent work), every observed defect not fixed this turn must become a GitHub issue, every post-write must pass the code-simplification discipline, and every turn must end with both an FSV evidence file and a RESOLVED/PAUSE/BLOCKED issue comment.

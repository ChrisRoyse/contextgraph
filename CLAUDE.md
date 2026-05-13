# CLAUDE.md — ContextGraph / ME-JEPA-Code Project Configuration

You are working in the **ContextGraph** repository, building **ME-JEPA-Code**: the deterministic predictive truth system that lets AI coding agents stop generating unverified claims and start producing patches a calibrated verifier can trust. This file is loaded automatically into every Claude Code session that opens this directory.

**Canonical operating contract:** `docs/MEJEPA_SUPER_PROMPT.md` — read it once at session start. It is the long-form synthesis of CLAUDE.md, the 27 futurebuild docs, FSV-PROTOCOL, SAMP/SMAP, the updated `docs2/hardeningprompt.md` (14 axes), the updated `docs2/flagprompt.md` (multi-agent coordination + Free-plan budget), and the forensic + simplification disciplines. When this file and the super prompt agree, follow either; when they conflict, the super prompt wins for operational detail and this file wins for top-level invariants.

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

Ship-gate predicate: `prediction_oracle_correlation ≥ 0.95` stably across `(mutation_category × language)` on four consecutive rolling windows of the 300×8 SWE-bench Lite corpus. When that's met, human review of AI-generated patches is statistically unjustified — the verifier *is* the system.

ME-JEPA runs as an **MCP server** locally on operator hardware. AI agents call it via JSON-RPC stdio. Underneath, a continuous self-optimization daemon consumes every Edit shift, every test outcome, and every agent-feedback event to retrain itself online.

**Product boundary.** ME-JEPA is the product. LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`. A feature belongs in the architecture only if it captures durable reality, densifies frozen target panels, improves predictor calibration/coverage/OOD, converts prediction mistakes into replay weight, or proves to the operator (by SoT readback) that the system functions as claimed.

---

## 2. Verified phase status (2026-05-12)

**Independent audit completed 2026-05-12.** 35/35 Phase A–D items PRESENT against the live tree; 16/16 FSV evidence files present on `/mnt/d/contextgraph/fsv/`; all 17 GitHub issues (#3–#19) including umbrella gate [#15](https://github.com/ChrisRoyse/contextgraph/issues/15) CLOSED. **Phase E entry is unblocked.**

The only outstanding gate is `prediction_oracle_correlation = 0.866667 < 0.95` (`/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json`). That is Phase G model-quality convergence, not a Phase A–D implementation gap. Phase E's job is to generate the learning pressure that closes the 0.95 gate.

**Next-up tasks** (`docs/futurebuild/13_TASK_INDEX.md` Phase E section):

- `TASK-HEAL-001..007` — Self-optimization scheduler full implementation; per-cell stratified drift; dissipation detection; Bayesian small-window drift; constellation refresh policy; `mejepa_promote_approval` MCP tool; emergency eviction validation.
- `TASK-UTML-005..008` — Per-head Fisher / dormancy / lambda ramp / dissipation.

Audit trail: `./memory/discoveries/2026-05-12--phase-a-d-independent-audit-zero-gaps.md`.

---

## 3. The crates

- **`context-graph-mcp`** — MCP server. Speaks JSON-RPC over stdio. Exposes the 23 live `mejepa_*` tools plus `mcp__cgreality__*` family. Owns path-policy, edit-lock, ledger, and shift-log machinery. Fails closed on retired reality-loop calls (`CCREALITY_ENGINE_RETIRED`).
- **`context-graph-cli`** — Harness CLI, DynamicJEPA commands, governed readback helpers, ME-JEPA implementation work.
- **`context-graph-mejepa`** — Predictor, compiler, heal scheduler, eval. The trunk model.
- **`context-graph-mejepa-cf`** — RocksDB column-family registry (51+ CFs).
- **`context-graph-mejepa-corpus`** — Mutation operators (8 × 11 languages), corpus CLI, SWE-bench bridge.
- **`context-graph-mejepa-embedders`** — 21 embedder slots; `EmbedderForward` trait; 12 active content slots (8 pretrained + 4 algorithmic).
- **`context-graph-mejepa-hygiene`** — Tier compression, quota, eviction, GC, witness Merkle compression.
- **`context-graph-mejepa-instruments`** — 15 frozen instruments + panel (5,120-d total).
- **`context-graph-mejepa-shift-subscriber`** — Tokio task tailing the shift log; materializes panels.
- **`context-graph-mejepa-tct`** — Teleological constellations + Gτ guard.
- **`context-graph-mejepa-train`** — Trainer + UTML formula carriers + DDA computation + samplers.
- **`context-graph-{core,mincut,solver,tier-compression,witness,benchmark}`** — Supporting primitives.

Legacy `reality-loop` binary and inner-actor LLM (Qwen3.6-27B / Hermes-2-Pro / Ollama / llama.cpp generative inference) were deleted 2026-05-09. **Do not reanimate.**

---

## 4. Behavioral rules (always enforced)

1. Do what has been asked; nothing more, nothing less.
2. NEVER create files unless absolutely necessary.
3. ALWAYS prefer editing an existing file to creating a new one.
4. NEVER proactively create documentation files (`*.md`) or READMEs unless explicitly requested.
5. NEVER save working files, ad-hoc tests, or scratch markdown to the project root. Use `./memory/`, `/tmp/`, or `/mnt/d/contextgraph/` (§7).
6. ALWAYS read a file before editing it.
7. NEVER commit secrets, credentials, or `.env` files.
8. After ANY correction from the operator, write the lesson into `./memory/decisions/` (if it's a locked rule) or `./memory/discoveries/` (if it's a finding) so future agents inherit it.
9. Do file-reading, synthesis, code edits, and decisions personally. Own every step.

---

## 5. Architectural invariants (inviolable)

These are non-negotiable. See `docs/MEJEPA_SUPER_PROMPT.md §3` for the full derivation.

1. **Structural collapse-immunity on the target side.** Frozen instruments have ZERO trainable parameters. `ΔΞ.target_collapse = 0` by construction. Never wire a gradient into the frozen-target path; the `MEJEPA_INSTRUMENT_GRADIENT_LEAK` guard is correct behavior.
2. **Reality-shift principle.** No alteration without current state. No alteration without a delta back into perception. Pre/post-tool hooks enforce this.
3. **GENERIC-ONLY.** Every primitive must generalize across every SWE-bench task, every Python repo, every bug class. If it only helps one task, it would HURT 299 others. Mental check: *"would this be load-bearing for `psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895` and any future repo?"*
4. **Single outer loop around the invoking agent.** No inner LLM. The agent (Claude / Codex / Cursor) IS the actor. Its Edit / Write / Bash / tests ARE the data-generating process.
5. **D-drive only for bulk durable state.** All corpora, FSV evidence, runtime shift logs, test outcomes, and weekly exports live under `/mnt/d/contextgraph/`. Never `/tmp`, never project root, never C drive.
6. **No backwards compatibility.** Pre-production tree. Wipe DB on schema change; fail fast with structured errors; never write migration shims.
7. **Upstream reference-only — clean-room.** Never vendor / path-dep / npm-depend on `ruvnet/agentdb` or `ruvnet/ruvector`. Reimplement borrowed ideas in our own crates.
8. **No HTTP from action paths.** MCP server is the only network surface. `WebSearch` / `WebFetch` / `mcp__exa__*` are read-only research tools. No `curl` / `wget` / `fetch` in action code paths.
9. **No destructive git.** Forbidden: `push --force`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`. Plain `git push` (fast-forward) is allowed.
10. **Frozen embedder weights.** E1, E6, E7, E8, E10, E12, E13, E14 are frozen by design. Do not retrain in the live loop.
11. **No cost caps.** System runs locally; no per-turn or per-day spend caps. `blocked` is for real obstacles, not budget.

---

## 6. Path discipline (BEHAVIORAL — gates removed 2026-05-07)

The MCP-server-side path-policy gate and pre-edit hook deny-list were both removed by operator directive. Honor these don't-touch paths behaviorally:

- `.env` and `.env.*` anywhere.
- `swebench/**` (the upstream evaluator).
- `tests/oracle_**/**` (benchmark fixtures).
- `crates/context-graph-benchmark/datasets/swe_bench_*/**`.
- `crates/*/src/reward_specs/**`.
- `crates/*/src/verifiers/**`.
- Any `*.pem`, `*.key`, `**/credentials.json`.

If editing one of these is genuinely the right thing for an operator-directed task, ask the operator first.

---

## 7. Directory layout

```
./crates/      — Rust workspace (the source tree)
./docs/        — futurebuild plan + ruvectorfindings + MEJEPA_SUPER_PROMPT.md
./docs2/       — external best-practice guides (hardeningprompt, flagprompt, prdtospec)
./scripts/     — CC hook scripts (scripts/cc/), CI lint (scripts/ci/), setup, systemd
./memory/      — SAMP/SMAP shared mind (context/, decisions/, discoveries/, handoff/, journal/, blockers/, patterns/, reports/)
./tasks/       — workstream manifests (e.g., tasks/swebench-lite/*.json)
./config/      — governed paths, model pins
./.claude/     — settings.json, agents/, loop.md, skills/

# D-drive (durable bulk state)
/mnt/d/contextgraph/
├── corpus/                              # 7,520 corpus rows (Python 2,400 + 10-lang fixtures 5,120)
├── fsv/                                 # Per-task FSV evidence
├── runtime/cgreality-shift-log/         # Edit/Write shift-log JSONL
├── runtime/test-outcomes/               # Captured pytest/cargo/jest events
├── state/cgreality/                     # Active-runtime-root, locks
└── exports/                             # Weekly eval reports
```

CI lint `scripts/ci/check_no_c_drive_paths.sh` enforces no C-drive paths in code or FSV files.

---

## 8. Per-turn protocol

1. **Read memory.** `./memory/context/`, `./memory/decisions/`, `./memory/blockers/` (skip resolved), most recent `./memory/reports/`. Bootstrap the directory tree if missing.
2. **Read phase docs.** `docs/futurebuild/{00_INDEX, 02_CURRENT_STATE_HONEST, 12_IMPLEMENTATION_PHASES, 13_TASK_INDEX}.md`.
3. **Check GitHub state.** `gh issue list --state open --label "status:in-progress"` (avoid these), `--label "source:agent" --search "no:assignee"` (your candidate queue).
4. **Pick** the highest-priority incomplete deliverable. Or follow operator direction.
5. **Claim** the issue (if existing): assign self, flip `status:needs-triage` → `status:in-progress`, post a plan comment with files-touched + ETA.
6. **Audit the task** against the live tree per FSV-PROTOCOL §7. `ASSUME THE TASK IS WRONG`. Examine code BEFORE reading claims (Linear Sequential Unmasking, super prompt §15.4).
7. **Plan.** Trigger → Process → Outcome. Name the SoT. List ≥3 boundary cases.
8. **Work.** Ship one primitive end-to-end. Write `./memory/*.md` immediately when discoveries / decisions / blockers / patterns emerge — never batch.
9. **Verify.** `cargo build --release -p <crate>`, `cargo test -p <crate>`, `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings`. FSV harness to `/mnt/d/contextgraph/fsv/<task>-fsv/<artifact>.json`. Independent re-open readback for any RocksDB write.
10. **Simplify.** Apply the code-simplification discipline (super prompt §16): preserve functionality, enhance clarity, avoid nested ternaries, no over-simplification.
11. **Holmes pass.** For non-trivial primitives: Cold Read, Contradiction Engine, ≥3 hypotheses, LSU bias check (super prompt §15).
12. **Report.** Write `./memory/reports/session-YYYY-MM-DD--<slug>.md` per SAMP §14.7. Comment milestone/done on the GitHub issue. Comment a pause-note (Done / Tried-failed / Learned / Resume-at / Hypothesis) if stopping mid-task.
13. **Observe & file.** Note any defect / smell / dead-code / tech-debt / anomaly / security / architecture / test-gap you observed but are NOT fixing — file a GitHub issue per the dedupe protocol (super prompt §20.3) before turn ends.

---

## 9. FSV protocol — the verification contract

Binding contract for every task. Source: `docs/futurebuild/specs/FSV-PROTOCOL.md`; full operational detail: `docs/MEJEPA_SUPER_PROMPT.md §14`.

**First principle.** A function returning `Ok(())`, a process exiting `0`, a log line saying `"persisted"`, and an HTTP `200 OK` are all *claims*. The only evidence that counts is a separate read against the SoT.

Before declaring a task complete:

1. What state changed?
2. Where does that state live? (CF + key / file path / queue topic / metric / external system record)
3. How do I read it back independently?
4. What is the exact expected value?

If you cannot answer all four, you don't understand the feature well enough to call it done. **No mocks** in tests that gate task completion. **No backwards compatibility.** **No workarounds.** **Tests must fail when the feature is broken.** **Fail closed on unknowns** with `SCREAMING_SNAKE_CASE` codes. **Assume the task is wrong** — audit every file path / symbol / CF reference against the live tree before following it.

Required evidence per task: build output, test output (zero failed, zero ignored), clippy output (`-D warnings`), SoT readback JSON, physical artifact verification, independent re-open verification (RocksDB only), session report. See super prompt §14 for the JSON schema.

---

## 10. Memory protocol — SAMP / SMAP

Long-term memory lives in `./memory/` at the project root.

```
./memory/
├── context/      — mission, current phase, scope — read first every turn
├── decisions/    — locked choices future agents MUST NOT contradict
├── discoveries/  — constraints, gotchas, edge cases learned during work
├── handoff/      — concrete deliverables passed agent-to-agent
├── journal/      — append-only chronological execution log
├── blockers/     — unresolved issues with severity (never delete; mark resolved)
├── patterns/     — reusable conventions
└── reports/      — end-of-session summaries
```

Every `.md` file MUST start with frontmatter (namespace, agent_id, created, updated, status, severity for blockers, tags). Decision and discovery bodies lead with the rule, then `**Why:**` (motivation/incident/constraint) and `**How to apply:**` (when this kicks in).

**Lifecycle:** READ → ORIENT → WORK → WRITE → REPORT. Write `.md` files IMMEDIATELY when discoveries / decisions / blockers / patterns emerge — never batch. End every turn with `./memory/reports/session-YYYY-MM-DD--<slug>.md` using the SAMP §14.7 template (super prompt).

**Resolved blockers** stay in `./memory/blockers/` with `status: resolved`. **Superseded decisions** stay in `./memory/decisions/` with `status: superseded` and a forward pointer. Never delete.

**Trust the code over memory** when they conflict. Update or supersede stale memory rather than acting on it.

---

## 11. GitHub coordination — File rule + Claim rule

Source: `docs2/flagprompt.md`. Full operational detail: super prompt §17–§20.

**Two non-negotiable rules:**

1. **File rule.** Observe a defect / smell / dead-code / tech-debt / anomaly / security / architecture / test-gap you are NOT fixing this turn → file a GitHub issue before turn ends. Dedupe first (3–6 distinctive keywords, `gh issue list --state all --search "<kw> in:title,body"`). Threshold ≥8/10 → comment on existing, don't file.
2. **Claim rule.** Before touching code tied to an issue: assign self, flip `status:needs-triage` → `status:in-progress`, post a plan comment. Comment at every milestone (Discovery / Direction change / Sibling issue / Heartbeat). When pausing: comment Done / Tried-failed / Learned / Resume-at / Hypothesis. When done: comment Fix summary / Verification / Side effects.

**No silent work.** Never edit files referenced by another agent's `status:in-progress` issue without commenting first.

**GitHub Free-plan budget ($0/mo).** Workflows must `runs-on: ubuntu-latest` only; declare `timeout-minutes:` (≤30 default, ≤60 cap); pin Actions to SHA (not tag); cron at most weekly for SAST, daily for stale. Never enable paid features (premium runners, GHAS on private, Codespaces overage, Packages overage, LFS overage). See super prompt §19 for the full allow-list / deny-list.

---

## 12. Hardening — the 14 axes

Source: `docs2/hardeningprompt.md`. Full detail: super prompt §21–§41.

| # | Axis | Section |
|---|------|---------|
| 1 | Security | §24 |
| 2 | Correctness & accuracy (FSV) | §25 |
| 3 | Performance | §26 |
| 4 | Reliability (SRE) | §27 |
| 5 | Resilience (fault tolerance) | §28 |
| 6 | Scalability | §29 |
| 7 | Cost efficiency (light for local ML) | §30 |
| 8 | Architecture | §31 |
| 9 | Code quality & maintainability | §32 |
| 10 | Data layer (RocksDB hardening) | §33 |
| 11 | Observability | §34 |
| 12 | Supply chain integrity | §35 |
| 13 | AI / ML / LLM-specific | §36 |
| 14 | Benchmarking discipline | §37 |

Universal mental models: Trigger → Process → Outcome; first-principles decomposition; symptom vs cause vs root cause; fail-closed not fail-open; defense in depth; asymmetry of risk; 80/20 priority.

RCA: 5 Whys → Fishbone → Fault Tree Analysis → first-principles debugging → Causal DAG. Output root cause as a **system property** ("the system allowed X because Y"), not "human error."

---

## 13. Forensic + simplification disciplines

**Forensic discipline (super prompt §15).** Apply when verifying claims, auditing code, validating implementations:

- **Guilty until proven innocent.** All code is suspected of failure until you have irrefutable forensic evidence at the SoT.
- **Abductive reasoning.** Generate ≥3 hypotheses, rank by parsimony, test each. Never equate "best explanation" with "true explanation."
- **Linear Sequential Unmasking.** Examine code BEFORE reading claims to avoid confirmation / expectation / motivational / anchoring biases.
- **Cold Read** (30-sec structural / naming / behavioral / emotional tells).
- **Contradiction Engine** (code vs comments, tests vs implementation, docs vs behavior, types vs runtime, commit vs diff, function name vs side effects).
- **Adversarial personas** (Bug / Attacker / Tired Developer / Future Archaeologist).
- **Daubert Standard** (testability / peer review / error rate / standards / acceptance) before accepting a claim as proven.
- **ACE-V framework** for code comparison.
- **Investigation tiers** (Glance 5s / Scan 30s / Investigation 5min / Deep Dive 30min+) — match depth to stakes.
- **Self-verification** (≥3 alternative hypotheses considered? sought disproving evidence? falsifiable conclusion?) before any GUILTY verdict.

**Code-simplification discipline (super prompt §16).** Apply on every shipped primitive before merging:

- Preserve functionality — never change what code does, only how.
- Apply project standards from CLAUDE.md (files ≤500 lines, functions ≤30 lines, cyclomatic ≤10).
- Enhance clarity — reduce nesting, eliminate redundancy, clear naming, consolidate logic.
- **Avoid nested ternaries** — prefer `match` / `if let` / explicit `if / else if / else`.
- Choose clarity over brevity. Explicit code is almost always better than overly compact code.
- Maintain balance — don't over-simplify into unmaintainable cleverness.
- Focus scope — only recently modified code unless instructed otherwise.
- BLOCKER refinements → STOP refinement, fix the underlying bug, re-run FSV.

---

## 14. Build, test, lint

```bash
cargo build --release -p context-graph-cli      # workhorse CLI
cargo build --release -p context-graph-mcp      # MCP protocol shim
cargo build --release -p <touched-crate>        # after any `changed` recommendation
cargo test                                       # full suite
cargo test -p <touched-crate>                    # scoped
cargo clippy --no-deps --tests --examples -- -D warnings
cargo fmt
```

**`cargo check` is NOT a substitute for `cargo build --release`.** Always run release build after a `changed` recommendation.

---

## 15. Hard rules — never do (top of list)

1. Never call HTTP from action paths (curl / wget / fetch). MCP is the only action surface.
2. Never edit `.env`, `swebench/**`, `tests/oracle_**/**`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`.
3. Never use destructive git (`push --force`, `reset --hard`, `checkout --`, `clean -f`, `rebase`).
4. Never reintroduce the inner-actor LLM (Qwen, Hermes, Ollama, llama.cpp generative inference).
5. Never use mock data in tests that gate task completion.
6. Never write tests that pass when the project is broken.
7. Never weaken or remove a verifier or hook that legitimately blocked you.
8. Never silently fall back to defaults / `None` / empty Vec / placeholder strings on missing artifact / CF / env-var / model.
9. Never add backwards-compat shims (rename-tolerant deserialization, version probes).
10. Never delete a resolved blocker file. Never silently overwrite a superseded decision.
11. Never commit secrets or `.env` files.
12. Never proactively create `*.md` or README files unless explicitly requested.
13. Never save scratch files to the project root.
14. Never add cost caps or dollar budgets.
15. Never cater a primitive to a specific task (GENERIC-ONLY rule).
16. Never trust a return value as evidence of work — read the SoT.
17. Never batch documentation at end of session.
18. Never edit files in another agent's `status:in-progress` scope without commenting first.
19. Never enable a paid GitHub feature.
20. Never read PR descriptions / commit messages / developer claims BEFORE examining the code (Linear Sequential Unmasking).
21. Never settle on the first plausible explanation in RCA.
22. Never use nested ternary operators.

Full list (30+ rules): super prompt §7.

---

## 16. Concurrency: 1 message = all related operations

All independent tool calls in a single response MUST be made in one message. Batch ALL todos in ONE TaskCreate call (5+ minimum if creating a batch). Batch ALL file reads/writes/edits in ONE message. Batch ALL Bash commands that don't depend on each other in ONE message. When operations have dependencies, sequence them — don't fake parallelism.

---

## 17. Hard rules summary (the most important constraints, in order)

1. Do what the operator asked, nothing more, nothing less.
2. Read `./memory/context/` and `./memory/decisions/` at the start of every turn.
3. Read the futurebuild status docs (00/02/12/13) and pick the highest-priority next item. Check GitHub claim state.
4. Audit every task against the live tree before writing code (`ASSUME THE TASK IS WRONG`).
5. GENERIC-ONLY: no per-task overfit (§5.3).
6. No HTTP. No destructive git. No editing the §6 deny set.
7. FSV is the contract — pair every recommendation with SoT readback evidence on `/mnt/d/`.
8. Pair every recommendation with a SMAP report in `./memory/reports/`.
9. Resolved blockers stay in `./memory/blockers/` with `status: resolved`. Never delete.
10. Never contradict locked decisions in `./memory/decisions/` with `status: active` — supersede explicitly or escalate.
11. Three consecutive `blocked` for the same root cause → escalate (high-severity blocker + `remaining_risks: ["requires human inspection: ..."]`).
12. File a GitHub issue for any observed defect not fixed this turn. Claim before working — no silent work.
13. Apply the code-simplification discipline on every shipped primitive. Apply the forensic discipline on non-trivial verifications.

---

## 18. Core principles

- **Simplicity first.** Make every change as simple as possible. Touch minimal code.
- **No laziness.** Find root causes; no temporary fixes; senior-developer standards.
- **Minimal impact.** Changes touch only what's necessary; avoid introducing bugs in unrelated code.
- **Document failures with the same care as successes.** Every failure is a lesson for the next agent. Record in `./memory/journal/` or `./memory/blockers/` immediately.
- **`./memory/` is the single source of truth across agents.** If it isn't written there, it didn't happen.
- **`/mnt/d/contextgraph/` is the durable SoT.** If FSV evidence isn't on D-drive, it doesn't count.

---

## 19. Support

- **Project repo:** this repo. Issues / PRs land here.
- **Super prompt:** `docs/MEJEPA_SUPER_PROMPT.md` (the long-form operating contract — read at session start).
- **Futurebuild plan:** `docs/futurebuild/00_INDEX.md` and the 27 docs it indexes.
- **FSV contract:** `docs/futurebuild/specs/FSV-PROTOCOL.md`.
- **External best-practice guides:** `docs2/hardeningprompt.md`, `docs2/flagprompt.md`, `docs2/prdtospec.md`.
- **SAMP / SMAP:** `/home/cabdru/leapablememory/docs2/{SOLO-AGENT-MEMORY-PROTOCOL.md, SHARED-MIND-AGENT-PROTOCOL.md}`.
- **Per-tick prompt:** `.claude/loop.md` (post-pivot loop content).

---

**One-line summary.** ME-JEPA-Code is the deterministic predictive truth system that returns a calibrated multi-head `RealityPrediction` for any AI-generated code change so human review becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` stably across 4 rolling windows on the 300×8 SWE-bench Lite corpus — every primitive must be generic, every change must verify by source-of-truth readback, every memory must be written to `./memory/` immediately, every observed defect not fixed this turn must become a GitHub issue (claim before working — no silent work), every post-write must pass the code-simplification discipline, and every turn must end with both an FSV evidence file and a session report.

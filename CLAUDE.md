# CLAUDE.md — ContextGraph / ME-JEPA-Code project addendum

You are working in the **ContextGraph** repository, building **ME-JEPA-Code**: the deterministic **binary reality predictor** that lets AI coding agents stop generating unverified claims and start producing patches a calibrated verifier can trust. This file is loaded automatically into every Claude Code session that opens this directory.

**Master rules (universal protocol):** [`docs2/AICodingAgentSuperPrompt.md`](docs2/AICodingAgentSuperPrompt.md) — the AI Coding Agent Doctrine. Read it once at session start. It is the canonical, density-first reference for memory (GitHub Issues), claiming, FSV, RCA, hardening, communication, session lifecycle, and the forensic + simplification disciplines.

**This file (CLAUDE.md)** is the project-specific addendum. When this file and the doctrine agree, follow either. When they conflict, **the doctrine wins for protocol detail** and **this file wins for project-specific invariants** (paths, crates, deny-list, current phase, build commands, MEJEPA architecture).

---

## 1. What we are building (2026-05-19 binary-doctrine reframe)

**ME-JEPA-Code** is a **binary reality predictor**. For any AI-generated code change, it returns `Pass | Fail` against the Docker oracle, plus three additional grounded surfaces:

| Q | Question | Surface | How ME-JEPA answers |
|---|----------|---------|---------------------|
| **Q1** | Does what the AI claimed it did **EXIST**? | binary prediction | Claim graph reconciled against bytes/AST on disk → `CF_MEJEPA_CLAIM_RECONCILIATION` |
| **Q2** | Does it **WORK**? | binary prediction | Predictor + conformal interval + TCT Gτ guard → `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}`; oracle = Docker `swebench.harness.run_evaluation` |
| **Q3** | **WHY** would it fail? | explanation only | On `Fail`, attaches a **named failure mode** (hierarchical, see #404) + **closest exemplars** from `CF_MEJEPA_PATCH_SIMILARITY_META`. Q3 is NOT itself a binary prediction surface — it labels the Q2 verdict |
| **Q4** | ~~What else could go wrong?~~ | **REMOVED 2026-05-19 (#408 closed)** | Q4 ambiguous surfaces have no free binary form against reality. CFs remain readable for forensic continuity; producer track is closed. Display-only. |
| **Q5** | How is it **IMPACTING REALITY**? | binary prediction | Predicted shift events checked against the live `runtime/shift-log/*.jsonl` tail → `RealityImpact` with `matched`/`missed`/`surprises`/`not_yet_observed` |

The product surface is the **MCP server**. AI agents call it via JSON-RPC stdio. Underneath, a continuous self-optimization daemon consumes every Edit shift, every test outcome, and every agent-feedback event to retrain itself online — driven by the **online mistake-driven loop** (#406) under EWC++ Fisher protection.

**Ship-gate predicate (tightened 2026-05-19):**

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per (mutation_category × language)
  on the 300×8 Python SWE-bench Lite corpus
  using Q2 Docker-oracle agreement as the binary verdict
  AND cross-panel oracle correlation ≥ 0.95 on Panel B (#405)
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

Once met → human review of AI-generated Python patches is statistically unjustified.

**2026-05-23 measurement decomposition (3-Sherlock audit, see #712).** The `0.7942190170` rank-lift number is not the predictor's score:

| Layer | Defect | Issue |
|---|---|---|
| Predictor head | Public Rust `Trainer` hard-coded `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`; latest aiwonder FSV loaded an out-of-band task-py-g-127 checkpoint (`optimizer_steps=64`, `weight_delta_observed=true`), but that path is not unified with the trainer contract and normal compiler construction must fail closed unless `trained_checkpoint_manifest_path` is explicit | #683 |
| Substrate | `e2/e3/e4` collapsed (1 unique vector each; `chunk.time` wall-clock injection identical for 399,777 chunks); `e8 ≡ e1` byte-identical (graph projection bug fixed `ff4d4c03`, prod corpus 7d older) | #704, #670 |
| Gate metric | Rank-lift = category-local RBF exemplar similarity over calibration split; lifts raw `0.4843 → 0.7942` without Brier/ECE improvement; measures exemplars, not predictor | #695 |

Honest raw-Pearson gap = `0.95 − 0.4843 = 0.4657`. Latest FSV: `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`; checkpoint readback: `/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json`, `optimizer_steps=64`, `weight_delta_observed=true`. The `0.866667` (pre-2026-05-19) was synthetic-vector evidence and remains invalid. #392 active/abstain (`0.9526350498` on `205/240`) is selection bias, not hidden gate pass. Panel B unmeasured (#405/#521).

**Product boundary.** ME-JEPA is the product. LLM agents are untrusted edit generators. Hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`. A feature belongs in the architecture only if it captures durable reality, densifies frozen target panels, improves predictor calibration/coverage/OOD, converts prediction mistakes into replay weight, or proves to the operator (by SoT readback) that the system functions as claimed.

Beyond the binary primitive ME-JEPA exposes two derived operator-facing surfaces:

- **Hierarchical chunk↔skill linkage** (#414 / #417 / #418 — shipped) — 399,777 AST chunks ↔ 491 accepted labels ↔ 128 named skills ↔ ~37k chunk→skill memberships ↔ ~10.6k skill→chunk reverse rows. Read-only navigation surface; no new prediction head.
- **Pathway exploration** (#416 — open) — top-K (3–5) most-probable future trajectories per candidate patch, chains of Q1/Q2/Q5 binary leaves only (Q4 strictly excluded). Operator picks which future to realize through code.

---

## 2. Phase status (refreshed 2026-05-19)

**Phases A–H complete.** 35/35 Phase A–D items audit-verified 2026-05-12. Phase H (inner-actor LLM retirement) FSV-verified 2026-05-13 — `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`. The previously documented 9 `scripts/ci/phase_h_*.sh` absence-checks do **not** exist on disk; #693 tracks whether to create them or remove the historical claim everywhere.

**The only outstanding gate is Phase G model-quality convergence:** valid real-input Panel A raw baseline is `prediction_oracle_correlation = 0.4843192995 < 0.95`; the `0.7942190170` rank-lift value is diagnostic only until #695 is resolved. Latest ship-gate FSV is `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`. The #392 active/abstain diagnostic has aggregate active correlation `0.9526350498`, Brier `0.0231730901`, and ECE `0.0169885978` on `205 / 240` holdout rows, but full ship-gate remains false due per-cell and rolling-window/power blockers. Panel B (#405/#521) still has no model-backed measurement.

**Post-migration storage:** Production durable state lives on aiwonder under `/zfs/archive/contextgraph/...`; hot work under `/zfs/hot/contextgraph/...` (per #363 / #412).

**2026-05-23 3-Sherlock parallel forensic audit.** Reports: `docs/SHERLOCK_PREDICTOR_FINDINGS_2026-05-22.md`, `docs/SHERLOCK_SUBSTRATE_FINDINGS_2026-05-22.md`, `docs/SHERLOCK_INFRA_FINDINGS_2026-05-22.md`. Filed 29 new issues (#683-#711). Decomposed the `0.7942` rank-lift measurement into three structural defects (table above). New critical path replaces `08 §3.0` ordering. See #712.

**2026-05-19 cleanup pass.** Investigation reports at `./docs/INVESTIGATION_{1,2,3}_*.md` drove ~314,000 LOC removed across 4 commits, including:

- Retired crates **`context-graph-causal-agent`**, **`context-graph-graph-agent`**, and **`context-graph-benchmark`** (had no ship-gate role after the 2026-05-09 LLM cutover and ME-JEPA pivot). Workspace: 25 → 22 active members.
- Retired **BlackBoxFix** subsystem from `context-graph-cli` (~52K LOC + 26 tests + `axum`/`reqwest`/`rusqlite` deps).
- Retired **Q4 producer modules** (#408 closed): `composite_reward_signals`, `static_analysis_reward`, `telemetry_signals`, `live_session_signals`, `type_graph_signals`, `e_coverage`, `e_cyclomatic`, `e_diff_topology`, `e_embedder_heads` + their FSV examples. `q4_trust_gate.rs` (the freeze enforcement) remains. Q4 CFs are read-only forensic continuity.
- Removed `linear_probe.rs`, `stdlib_registry.rs`, pre-pivot benchmark bins (`causal_*`, `multimodal_*`, `embedder_impact_*`, `weight_profile_bench`, `e2_manual_verification`, `e10_modifier_tuning`, `e11_entity_bench`).
- Removed 42 retired Shakespeare LoRA training scripts (4 historical-only stubs retained — Phase H absence-check tracks them).
- Removed `.claude/skills/ccreality-{iterate,readback,smoke,trigger-check}/`, `docs2/contextgraphspecs/`, `docs2/realitywhitepaper/`, `backupdocs/*` numbered duplicates.
- Retired obsolete WSL/Shakespeare/EXP-2 script trees and re-pointed remaining active runtime defaults to aiwonder.
- Re-branded 7 storage-test `validator_version: "hermes-2-pro-mistral-7b-q5-km@2026-04"` strings → `"deterministic-validator-v1@2026-05"`.

**Current work source.** GitHub Issues are the live source of truth. Read pinned `type:context` issues first, then `docs/futurebuild/README.md` and `docs/futurebuild/08_roadmap_and_open_work.md`.

Active pinned `type:context`:

- **#712** — Sherlock-validated critical path (2026-05-23). **Authoritative ordering.**
- **#633** — Manual full-system FSV + hardening gate (P0 children #634-#651).
- **#310** — META Phase G queue ordering (now subordinate to #712).
- **#383** — Locked rule: gate-active issue ordering is evidence-weighted.
- **#227** — Python ME-JEPA data runtime policy.
- **#397** — Consequence-prediction truth-system contract.

**Current critical path (Sherlock-validated 2026-05-23, supersedes prior ordering):**

| Rank | Issue | Action | Expected impact |
|---|---|---|---|
| 1 | **#683** | Unify real training path with trainer contract and require production `trained_checkpoint_manifest_path` fail-closed | Largest; current checkpoint evidence is out-of-band from the public trainer |
| 2 | **#670 + #704** | Regenerate `e2/e3/e4/e8`: fix `chunk.time` injection; `ff4d4c03` already fixed graph projection; rerun prod forward cache | Recovers 4/12 panel signal-bearing dims |
| 3 | **#615** | Run delete_test_call Docker oracle batch over 1,131 candidates → rebuild AST → #614 mask → labels | Closes weak-cell ceiling (`0.69 → ≥0.85`) |
| 4 | **#618 + #619** | Per-slot OOD aggregation; slot-preserving panel projections | Removes anti-compensation cheat path |
| 5 | **#691** | Hook scripts exit non-zero on capture failure | Restores Q5 binary-prediction honesty |
| 6 | **#695** | Stop using rank-lift as gate metric; use raw Pearson + isotonic/Platt | Reveals honest gap |
| 7 | **#685/#687/#688/#690/#707** | Eliminate hardcoded passing telemetry (`forward_logits` zeros; `detach_loras` no-op; `EpochSummary` hardcoded best/worst; `retrain_triggered:true` literal; OOD strict fail-open) | Prevents Goodhart on future mistake-loop wiring |
| 8 | #392 / #511 / #405 / #379 | Trajectory ECE / entropy aux loss / Panel B / multi-file holdout | **Blocked behind 1-7** |

Closed prior critical-path items remain valid substrate: #408 Q4 freeze, #404 failure-mode hierarchy, #413 auto-label discovery, #414 skill substrate, #418 runtime ability resolver, #406+#523 mistake-loop MCP. The structural work survived the audit; only the **measurement** and **predictor head** were broken.

Historical 2026-05-12 audit trail lived at `./memory/discoveries/2026-05-12--phase-a-d-independent-audit-zero-gaps.md`. That namespace is archived under `backupdocs/memory.archive/`. Any audit-trail entries going forward become `type:discovery` GitHub issues per doctrine §3.

---

## 3. The crates (22 active workspace members)

Workspace manifest: `Cargo.toml:6-30`. `default-members` mirrors the 22 active workspace crates.

**Product surface:**

- **`context-graph-mcp`** — MCP server. Speaks JSON-RPC over stdio + Streamable HTTP. Exposes 54 live `mejepa_*` tools plus compatibility `mcp__cgreality__*` source-of-truth transport. Owns path-policy, edit-lock, ledger, and shift-log machinery. Fails closed on retired reality-loop calls (`CCREALITY_ENGINE_RETIRED`).
- **`context-graph-cli`** — Operator CLI + helpers. Two bins: `context-graph-cli` (primary), `context-graph` (short alias). Owns DynamicJEPA commands, governed readback helpers, ccreality hook surface, ME-JEPA active-learning surface. **BlackBoxFix subsystem removed 2026-05-19.**

**ME-JEPA trunk:**

- **`context-graph-mejepa`** — Predictor, compiler (`MeJepaCompiler::compile`), heal scheduler, eval. The trunk model. Bins: `mejepa`, `mejepa-heal-drill`.
- **`context-graph-mejepa-cf`** — RocksDB column-family registry (~120 ME-JEPA CFs). Zero internal deps by design.
- **`context-graph-mejepa-corpus`** — 8 mutation operators (Python first-class; other 10 languages deferred), corpus CLI, SWE-bench bridge (`run_swebench_lite_oracle()` → Docker `python -m swebench.harness.run_evaluation`).
- **`context-graph-mejepa-embedders`** — 21 embedder slots; `EmbedderForward` trait; 12 active content slots (8 pretrained + 4 algorithmic).
- **`context-graph-mejepa-hygiene`** — Tier compression, quota, eviction, GC, witness Merkle compression.
- **`context-graph-mejepa-instruments`** — Frozen instrument slots. The `Panel` is an **array of per-embedder vectors** (a teleological constellation), one vector per slot, each living in its own embedder's space. The contiguous 5,120-f32 serialization is **byte layout only**; cross-embedder dim-wise comparison is forbidden. Allowed operations: (a) same-embedder cosine (E1↔E1, E_AST↔E_AST, …), (b) array-level structured operations (per-slot cosine maps, similarity matrices, slot-identity-preserving projections), (c) scalar pairwise cross-slot relationships (MI, correlation, agreement). **Q4 producer modules and TASK-RWD signals retired 2026-05-19**; Q4 trust gate remains.
- **`context-graph-mejepa-shift-subscriber`** — Tokio task tailing the shift log; materializes panels.
- **`context-graph-mejepa-tct`** — Teleological constellations + Gτ guard + freshness audit.
- **`context-graph-mejepa-train`** — Trainer + UTML formula carriers + DDA computation + replay buffer + mistake-loop consumer (#406). Bin: `mejepa-train`.

**Supporting primitives:**

- **`context-graph-core`** — Shared domain types: `CodeEntity`, `CodeLanguage`, `LLMEdgeValidation`, `TypedEdgeTrainingRecord`, etc.
- **`context-graph-storage`** — RocksDB wrapper + 51+ CFs.
- **`context-graph-embeddings`** — E1–E14 pipeline, model loaders. Bin: `train-causal`.
- **`context-graph-cuda`** — CUDA helper kernels (HDC, k-NN).
- **`context-graph-paths`** — Fail-closed durable path resolver; rejects non-aiwonder prod targets.
- **`context-graph-witness`** — Clean-room ed25519 + Merkle + Stage 0/F certificates.
- **`context-graph-solver`** — Causal masking, MMR, conformal predictor.
- **`context-graph-mincut`** — Clean-room Stoer-Wagner global min-cut.
- **`context-graph-tier-compression`** — 8/7/5/3-bit groupwise scalar quantization with magic+version header.
- **`context-graph-dynamicjepa`** — 5090jepa MCP surface (39 tools): adapter shipping, plan trace, surprise log.
- **`context-graph-test-utils`** — Shared deterministic-by-shape, pseudo-random-by-content test helpers. Suitable for storage/serialization/shape tests only; not valid as real embedder/corpus evidence for ME-JEPA gates.

**Legacy and retired:**

- `context-graph-reality-loop` binary — deleted 2026-05-09 (Phase H). Do not reanimate without explicit operator authorization and new FSV evidence.
- `context-graph-benchmark` crate — deleted 2026-05-20 after aiwonder FSV readback found no live benchmark evidence path and the ME-JEPA ship gate no longer uses pre-pivot benchmark bins.
- Inner-actor LLM stack (Qwen3.6-27B / Hermes-2-Pro / Ollama / llama.cpp generative inference) — deleted 2026-05-09; current hard guard is the MCP `feature = "llm"` compile error. The `phase_h_*.sh` absence-check claim is unresolved docs drift tracked by #693.
- `context-graph-causal-agent`, `context-graph-graph-agent` — deleted 2026-05-19; LLM-backed callers were retired in the 2026-05-09 ME-JEPA pivot and the crates had zero workspace dependents.

---

## 4. Memory = GitHub Issues (per doctrine §3)

**There is no `./memory/` directory.** The old SAMP/SMAP namespace is archived to `backupdocs/memory.archive/` (read-only history; do not write new files there). New memory is captured exclusively in this repo's GitHub Issues:

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

1. **Binary doctrine.** ME-JEPA predicts binary outcomes against ground truth (Q1 bytes/AST, Q2 Docker oracle, Q5 shift log). Q3 is an explanation surface attached to Q2 verdicts. **Q4 is removed/frozen** as of 2026-05-19 (#408). Do not reanimate Q4 producer pipelines without new operator authorization and a doctrine change.
2. **Slot identity is sacred.** The `Panel` is an array of per-embedder vectors. NEVER perform cross-embedder dim-wise comparison. NEVER flatten the panel into a single 5,120-d vector for similarity/distance. The contiguous serialization is for transport only. See `01_overview.md §1.5ter` and `02_source_map.md §5`.
3. **Structural collapse-immunity on the target side.** Frozen instruments have ZERO trainable parameters. `ΔΞ.target_collapse = 0` by construction. Never wire a gradient into the frozen-target path; the `MEJEPA_INSTRUMENT_GRADIENT_LEAK` guard is correct behavior.
4. **Reality-shift principle.** No alteration without current state. No alteration without a delta back into perception. Pre/post-tool hooks enforce this.
5. **GENERIC-ONLY in the trained model.** Every trained primitive must generalize across every SWE-bench task, every Python repo, every bug class. Mental check: *"would this be load-bearing for `psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895` and any future repo?"* You may reason task-specifically when attempting a SWE-bench task; never bake task-specifics into the trained model.
6. **Single outer loop around the invoking agent.** No inner LLM. The agent (Claude / Codex / Cursor) IS the actor. Its Edit / Write / Bash / tests ARE the data-generating process. Current enforced guard: MCP `feature = "llm"` compile error; #693 owns the missing absence-check scripts/docs drift.
7. **aiwonder only for post-migration production durable state.** Production corpora, FSV evidence, runtime shift logs, test outcomes, project-ingest outputs, and weekly exports live under `/zfs/archive/contextgraph/...` on aiwonder; hot staging lives under `/zfs/hot/contextgraph/...` or explicitly budgeted aiwonder scratch. 
8. **No backwards compatibility.** Pre-production tree. Wipe DB on schema change; fail fast with structured errors; never write migration shims.
9. **Upstream reference-only — clean-room.** Never vendor / path-dep / npm-depend on `ruvnet/agentdb` or `ruvnet/ruvector`. Reimplement borrowed ideas in our own crates.
10. **No HTTP from action paths.** MCP server is the only network surface. `WebSearch` / `WebFetch` / `mcp__exa__*` are read-only research tools. No `curl` / `wget` / `fetch` in action code paths.
11. **No destructive git.** Forbidden: `push --force`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`. Plain `git push` (fast-forward) is allowed.
12. **Frozen embedder weights.** E1, E6, E7, E8, E10, E12, E13, E14 are frozen by design. Do not retrain in the live loop.
13. **Mistake-loop is the only online weight update path.** Trainer-side `mejepa-train/src/mistake_consumer.rs` (#406) consumes `CF_MEJEPA_MISTAKE_LOG` under EWC++ Fisher protection. No other path may update predictor weights at serving time.
14. **No cost caps.** System runs locally; no per-turn or per-day spend caps. `blocked` is for real obstacles, not budget.

---

## 7. Path discipline (BEHAVIORAL — gates removed 2026-05-07)

The MCP-server-side path-policy gate and pre-edit hook deny-list were both removed by operator directive. Honor these don't-touch paths behaviorally:

- `.env` and `.env.*` anywhere
- `swebench/**` (the upstream evaluator)
- `tests/oracle_**/**` (benchmark fixtures)
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- Any `*.pem`, `*.key`, `**/credentials.json`

If editing one of these is genuinely the right thing for an operator-directed task, ask the operator first.

---

## 8. Directory layout

```
./crates/         — Rust workspace (22 active members)
./docs/           — futurebuild plan (8-doc live suite + _archived/) + PAPER.md + investigation reports
./docs2/          — AICodingAgentSuperPrompt.md (master doctrine) + hardeningprompt.md (14-axis deep-dive) + prdtospec.md + aiwonder.md + flagprompt.md (SUPERSEDED, kept as reference)
./scripts/        — CC hook scripts (scripts/cc/), CI lint helpers (scripts/ci/), setup, swebench_lite_ops/, label bootstrap (gh-labels-bootstrap.sh)
./tasks/          — Workstream manifests (e.g., tasks/swebench-lite/*.json — 300 SWE-bench Lite Python instance manifests)
./tests/e2e/      — 13 root-level chaos/load/soak/multi-agent/restart-safety e2e tests (shim'd into crates/*/tests/)
./tests/fixtures/ — 30 adversarial diff fixtures (TASK-PY-G-060)
./config/         — default.toml, mejepa/, retention_policy.toml, cell_exemptions.toml, governed_paths.json (legacy)
./configs/        — dynamicjepa/ (domain_packs/, verification/) — note plural; tracked for collapse in tech-debt
./systemd/        — contextgraph-mcp-http.service (aiwonder paths)
./.claude/        — settings.json, agents/, loop.md, skills/, hooks/
./.github/        — workflows/ (ci, chaos, e2e, load, soak) + labels.yml
./backupdocs/memory.archive/ — read-only legacy SAMP/SMAP narrative (no new entries)
./samplecode/     — Multi-language scratch corpus referenced by ast_embedder_smoke_fsv (source files only; build caches gitignored)
./shakespear/     — complete_works.txt (LibraryId::ShakespeareCanon training data)
./reference/      — DynamicJEPA reference release JSONs (archived regression fixtures)

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

# local legacy provenance only — NOT an active data root
/zfs/archive/contextgraph/
```

CI lint `scripts/ci/check_no_c_drive_paths.sh` blocks C-drive paths in code or FSV files.

---

## 9. Per-turn protocol (project overlay on doctrine §12)

1. **READ** (doctrine §3.3 + §12.1) — Run the six issue queries to see pinned context / in-progress / blocked / unclaimed agent queue / active decisions / discoveries touching your task area. Skim `AGENTS.md` and this file.
2. **Read current futurebuild docs.** `docs/futurebuild/README.md` maps the current 8-doc suite; read `01_overview.md`, `07_results_and_benchmarks.md`, and `08_roadmap_and_open_work.md` for current state, proof history, and open work.
3. **Check GitHub state** — `gh issue list --state open --label "status:in-progress"` (avoid these), `--label "source:agent" --search "no:assignee"` (your candidate queue).
4. **Pick** the highest-priority incomplete deliverable, prioritized by the evidence-weighted ordering in `08 §3.−1`. Or follow operator direction.
5. **Claim** the issue (if existing) per doctrine §3.4: atomic `gh issue edit` assigning self + flipping `status:needs-triage` → `status:in-progress` + CLAIM comment listing files-touched and ETA.
6. **Audit the task** against the live tree per `docs/futurebuild/specs/FSV-PROTOCOL.md` §7. `ASSUME THE TASK IS WRONG`. Examine code BEFORE reading claims (Linear Sequential Unmasking, doctrine §2.8).
7. **Plan.** Trigger → Process → Outcome (doctrine §2.2). Name the SoT. List ≥3 boundary cases.
8. **Work.** Ship one primitive end-to-end. Comment milestone updates on the issue (doctrine §3.5) — discovery / direction-change / heartbeat / sibling-issue. Never silent.
9. **Verify** per doctrine §4: `cargo build --release -p <crate>`, `cargo test -p <crate>`, `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings`. Production FSV harnesses write to `/zfs/archive/contextgraph/fsv/<task>-fsv/<artifact>.json` on aiwonder, with independent re-open readback for any RocksDB write. not an active closeout target.
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
cargo build --release                            # full workspace
cargo build --release --examples                 # also compile every FSV harness
cargo check --workspace --tests --examples       # MANDATORY after changing any shared type
cargo test                                       # full suite
cargo test -p <touched-crate>                    # scoped
cargo clippy --no-deps --tests --examples -- -D warnings
cargo fmt
```

**`cargo check` is NOT a substitute for `cargo build --release`.** Always run release build after a `changed` recommendation.

**`cargo check --workspace --tests --examples` IS REQUIRED** after touching any type, trait, or function signature that crosses crate boundaries. `cargo build --release` does NOT compile test or example targets, and `cargo test -p <crate>` only recompiles tests for that crate — neither catches downstream test-compile drift in other crates. The 2026-05-20 cleanup discovered three production-crate tests had been silently broken since 2026-05-19 (`OracleVerdict::evidence_unavailable` and `ReleaseAggregateTableSpec::numeric_columns` fields added but downstream `#[cfg(test)]` call sites missed) because no workspace-wide test-compile gate was being run. **Discovery: the test contract for "compiles" is part of the test contract — a test that doesn't compile is worse than no test.**

**Self-referential static-source tests are forbidden.** A test that does `source.contains(forbidden_literal)` against its own file will self-match if the literal appears in the test itself (e.g., inside `r#"..."#` raw strings). Such tests always fail in CI and look like a real regression. If a contract must be enforced at the source level, do it via a separate tool (build script, `scripts/ci/*.sh` grep) or use a behavior test against the real error code, not the production source's substring shape.

Phase H's real hard guard is the MCP `feature = "llm"` compile error. The previously documented `scripts/ci/phase_h_*.sh` absence-check suite is missing and tracked by #693. Phase G audit harnesses live at `scripts/ci/phase_g_*.sh`.

---

## 15. Project-specific hard "never"s (additive to doctrine §15-equivalent)

These are on top of the doctrine's universal forbidden list.

1. Never reintroduce the retired inner-actor LLM stack (Qwen, Hermes, Ollama, llama.cpp generative inference) without explicit operator authorization and new FSV evidence.
2. Never reanimate the **Q4 producer track** (composite reward signals / static-analysis / telemetry / live-session / type-graph / coverage / cyclomatic / diff-topology / embedder-heads) without a documented #408 reversal. The Q4 trust gate at `mejepa/src/q4_trust_gate.rs:Q4_DOCTRINE_FREEZE_ACTIVE = true` is doctrine.
3. Never reanimate the retired BlackBoxFix subsystem (HTTP server, SQLite ledger, axum/reqwest deps) without filing a new `type:context` issue documenting product-mission alignment.
4. Never reanimate `context-graph-causal-agent` or `context-graph-graph-agent` — both deleted 2026-05-19; their LLM-backed callers have been retired since 2026-05-09.
5. Never call HTTP from action paths (curl / wget / fetch). MCP is the only action surface.
6. Never edit `.env`, `swebench/**`, `tests/oracle_**/**`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**` without operator OK.
7. Never use destructive git (`push --force`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`).
8. Never write `./memory/*.md` files. Memory is GitHub Issues (doctrine §3).
9. Never cater a trained primitive to a specific task (GENERIC-ONLY rule §6.5).
10. Never trust a return value as evidence of work — read the SoT (doctrine §0 / §4).
11. Never add cost caps or dollar budgets.
12. Never use mock data in tests that gate task completion.
13. Never write tests that pass when the project is broken.
14. Never collapse the panel into a single flat vector for similarity, distance, OOD residual, calibration, or any predictor input. Slot identity is doctrine (§6.2).

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
- **`/zfs/archive/contextgraph/` on aiwonder is the production durable SoT** for post-migration FSV evidence. 
- **Binary > calibrated.** The system was reframed 2026-05-19 from "calibrated multi-head RealityPrediction" to "binary reality predictor with labeled explanation surfaces". Q4 ambiguous heads are out of scope.

---

## 18. Support

- **Doctrine (master):** [`docs2/AICodingAgentSuperPrompt.md`](docs2/AICodingAgentSuperPrompt.md)
- **Project pointers:** [`AGENTS.md`](AGENTS.md)
- **Futurebuild plan:** [`docs/futurebuild/README.md`](docs/futurebuild/README.md)
- **FSV contract:** [`docs/futurebuild/specs/FSV-PROTOCOL.md`](docs/futurebuild/specs/FSV-PROTOCOL.md)
- **14-axis hardening deep-dive:** [`docs2/hardeningprompt.md`](docs2/hardeningprompt.md)
- **Aiwonder operations:** [`docs2/aiwonder.md`](docs2/aiwonder.md)
- **Per-tick prompt:** [`.claude/loop.md`](.claude/loop.md)
- **2026-05-19 cleanup audit:** [`docs/INVESTIGATION_1_CRATES_DEAD_CODE.md`](docs/INVESTIGATION_1_CRATES_DEAD_CODE.md), [`docs/INVESTIGATION_2_TESTS_BENCHMARKS_EXAMPLES.md`](docs/INVESTIGATION_2_TESTS_BENCHMARKS_EXAMPLES.md), [`docs/INVESTIGATION_3_SCRIPTS_DOCS_CONFIG_TOPLEVEL.md`](docs/INVESTIGATION_3_SCRIPTS_DOCS_CONFIG_TOPLEVEL.md)
- **Legacy MEJEPA narrative (SUPERSEDED for protocol; project content still valid):** [`docs/MEJEPA_SUPER_PROMPT.md`](docs/MEJEPA_SUPER_PROMPT.md)
- **Legacy issue protocol (SUPERSEDED by doctrine §3):** [`docs2/flagprompt.md`](docs2/flagprompt.md)
- **Legacy SAMP/SMAP narrative (read-only history):** [`backupdocs/memory.archive/`](backupdocs/memory.archive/)

---

**One-line summary.** ME-JEPA-Code is the **binary reality predictor** that returns `Pass | Fail` for any AI-generated code change against the Docker oracle, with named-failure-mode + closest-exemplar explanation on Fail (Q3), claim-existence (Q1) and shift-event (Q5) binary predictions as additional grounded surfaces, and Q4 ambiguous heads frozen out as of 2026-05-19 — human review becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` stably × 4 rolling windows on the 300×8 Python SWE-bench Lite corpus AND Panel B confirms within 0.05. Every primitive must be generic, every change must verify by source-of-truth readback on aiwonder, every memory must be captured in a GitHub Issue (claim before working — no silent work), every observed defect not fixed this turn must become a GitHub issue, every post-write must pass the code-simplification discipline, and every turn must end with both an FSV evidence file and a RESOLVED/PAUSE/BLOCKED issue comment.

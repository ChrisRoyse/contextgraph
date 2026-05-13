# 13 — Task Index (Atomic Task DAG)

> **Purpose.** Atomic, sequenced, traceable task list. Each TASK-* corresponds to one Level-4 task per `prdtospec.md` — sized for one focused engineer-session, with explicit definition-of-done and dependencies. Atomic task files live under [`tasks/<domain>/`](tasks/).
>
> **Authority.** `12_IMPLEMENTATION_PHASES.md` orders the phases; this document orders the tasks within and across phases.
>
> **Binding verification contract.** Every TASK-* file in this DAG inherits from [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md). The protocol is the *how*; the task file is the *what*. Picking up a task without first reading the protocol means you skip the SoT-readback, boundary-case, and Occam's-razor requirements — and the task is not done without them. **`ASSUME THE TASK IS WRONG`**: audit every file path, symbol name, and CF reference in the task against the live tree before implementing (see `FSV-PROTOCOL.md §7`).

---

## Domain prefixes

| Prefix | Domain |
|---|---|
| `TASK-FLYWHEEL-NNN` | data flywheel: daemon, scheduler, oracle binding, CFs |
| `TASK-PREDICT-NNN` | live prediction API, multi-head, RealityPrediction, hooks |
| `TASK-ENRICH-NNN` | DDA / TCT enrichment: neural embedder forwards, DDA signal compute |
| `TASK-LOOP-NNN` | loop refactor + inner-loop residue cleanup |
| `TASK-HEAL-NNN` | self-healing scheduler + drift + dissipation + promotion |
| `TASK-CORPUS-NNN` | per-language operators + 300×8 corpus run |
| `TASK-OBS-NNN` | observability: weekly eval cron, MI heatmap, GPU telemetry, CLI |
| `TASK-UTML-NNN` | learning-formula extensions: per-head deltas, cert cols, lambda ramp |

## Current gate

The A-D readiness issues are closed for the Phase E handoff. Analyzer-backed instrument parity (#4), production telemetry (#11), UTML carrier reconciliation (#13), and the umbrella readiness gate (#15) have source-of-truth FSV. The latest holdout remains `0.866667 < 0.95`; that is Phase G model-quality convergence, not an A-D substrate blocker.

---

## Phase A — Operational Unblock (12 substrate tasks + moved cleanup/observability)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-FLYWHEEL-001` | MCP daemon mode + supervised tokio tasks + systemd template + D-drive validator | A | none | complete at code/FSV level; fresh-host enablement proof remains |
| `TASK-FLYWHEEL-002` | Self-optimization scheduler skeleton (7 tickers calling shipped APIs) | A | FLYWHEEL-001 | complete |
| `TASK-FLYWHEEL-003` | Test-outcome capture hook (parse pytest/cargo-test/jest) | A | none | **complete (2026-05-12)** |
| `TASK-FLYWHEEL-004` | Captured test-outcome oracle binding | A | FLYWHEEL-003 | **complete (2026-05-12 code path)** |
| `TASK-FLYWHEEL-005` | `ActiveLearningKind::AgentSurprise` variant + queue extension | A | none | **complete (2026-05-11)** |
| `TASK-FLYWHEEL-006` | New CFs: `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK` | A | none | **complete (2026-05-11)** |
| `TASK-PREDICT-001` | UserPromptSubmit prediction-injection hook | A | FLYWHEEL-001 | **complete (2026-05-12)** |
| `TASK-PREDICT-002` | `mejepa_record_agent_feedback` MCP tool + sampling-weight bump wiring | A | FLYWHEEL-005, FLYWHEEL-006 | **complete (2026-05-12; no feedback rate limit)** |
| `TASK-LOOP-001` | Apply new `.claude/loop.md` content from `14_NEW_LOOP_MD.md` | A | FLYWHEEL-001 | **complete (already applied)** |
| `TASK-LOOP-002` | Update CLAUDE.md §11.11/§13/§3 + agent README retirements | H | LOOP-001 | moved to Phase H residue cleanup; not an A-D ME-JEPA substrate blocker |
| `TASK-LOOP-007` | Hook output paths → D-drive `cgreality-shift-log/` | A | none | **complete (2026-05-11 — code already shipped, audit doc updated)** |
| `TASK-OBS-005` | C-drive lint in CI | A | none | **complete (2026-05-12)** |
| `TASK-OBS-006` | FSV path migration sweep | A | none | **complete (2026-05-12)** |
| `TASK-OBS-007` | Per-slice quota observability extension | F | none | moved to Phase F observability; category-level quota status already ships |

**Phase A substrate status:** complete for the ME-JEPA Phase E handoff. The two moved rows remain tracked under Phase F/H where they belong.

---

## Phase B — Multi-Head Extension (core tracked tasks)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-PREDICT-003` | `mejepa_predict_what_if` MCP tool | B | PREDICT-005 | **complete (2026-05-12)** |
| `TASK-PREDICT-004` | `mejepa_explain_prediction` MCP tool | B | PREDICT-005 | **complete (2026-05-12)** |
| `TASK-PREDICT-005` | Multi-head stack (8 new heads + per-head feature projections) | B | none | **code_complete (2026-05-12; DDA feature hardening remains Phase D)** |
| `TASK-PREDICT-006` | Rich `RealityPrediction` schema | B | PREDICT-005 | **complete (2026-05-12)** |
| `TASK-PREDICT-007` | Agent claim graph extractor (deterministic) | B | none | **complete (2026-05-12)** |
| `TASK-PREDICT-008` | Reality-impact reconciler (Q5) | B | PREDICT-007 | **complete (2026-05-12)** |
| `TASK-PREDICT-LABEL-001` | Failure-mode label source (pytest traceback parser → FailureModeClass) | B | CORPUS-002 | **code_complete (2026-05-12 unified deterministic extractor)** |
| `TASK-PREDICT-LABEL-002` | Edge-case label source (coverage delta + symbolic-execution proxy + per-language pattern catalog) | B | none | **partial/code_complete: deterministic text extractor; coverage/symbolic hardening open** |
| `TASK-PREDICT-LABEL-003` | Tech-debt label source (clippy/pylint/eslint warning delta + radon/lizard) | B | none | **partial/code_complete: deterministic text extractor; analyzer deltas open** |
| `TASK-PREDICT-LABEL-004` | Dead-code label source (vulture/udeps/eslint-no-unused) | B | none | **partial/code_complete: taxonomy/schema exists; analyzer deltas open** |
| `TASK-PREDICT-LABEL-005` | Perf label source (pytest-benchmark / cargo bench / massif delta extractors) | B | none | **partial/code_complete: patch-surface heuristic; benchmark parsers open** |
| `TASK-PREDICT-LABEL-006` | Security label source (bandit/cargo-audit/eslint-security/semgrep delta) | B | none | **partial/code_complete: hardcoded secret/crypto text extractor; SAST deltas open** |
| `TASK-PREDICT-LABEL-007` | Accuracy label source (test-output metric regex extractor) | B | none | **partial/code_complete: metric keyword extractor; full regex catalog open** |
| `TASK-PREDICT-LABEL-008` | Cost label source (CI minute counter + token telemetry + dep-count delta) | B | none | **partial/code_complete: dependency-count delta; CI/token telemetry open** |
| `TASK-PREDICT-LABEL-009` | Reasoning-class weak supervisor (E_Reasoning ⟂ E_Diff agreement) | B | PREDICT-005 | **partial/code_complete: field/schema present; embedder-agreement supervisor open** |
| `TASK-PREDICT-LABEL-010` | Toolchain availability fail-closed audit | F | none | open Phase F hardening; not a Phase B schema/API blocker |
| `TASK-PREDICT-LABEL-011` | Cross-language label gating | F | PREDICT-LABEL-001..009 | open Phase F hardening; not a Phase B schema/API blocker |
| `TASK-PREDICT-009` | `ActiveLearningKind::AgentSurprise` integration (consumer side) | B | FLYWHEEL-005 | **complete (2026-05-11/12)** |
| `TASK-UTML-001` | Per-head ΔP/ΔK/ΔΞ tracking | B | PREDICT-005 | **complete/code_complete (2026-05-12)** |
| `TASK-UTML-002` | Per-head cert columns in `CF_MEJEPA_TRAIN_CERTS` | B | UTML-001 | **complete for Phase A-D (2026-05-12; cert readback + carrier fields FSV)** |
| `TASK-UTML-003` | `ΔΞ_global = min_h(ΔΞ_h)` aggregation | B | UTML-001 | **complete for persisted learning-signal carrier; BatchSampler policy belongs to Phase E** |
| `TASK-UTML-004` | `agent_surprise_severity_score` factor in sampling_weight | B | PREDICT-002 | **complete** |
| `TASK-UTML-005` | Per-head Fisher matrices | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-006` | Per-head dormancy detection | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-007` | Per-head loss coefficient λ ramp in heal scheduler | E | HEAL-001, UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-008` | Per-head dissipation detection | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-009` | Per-cell state-transfer T in eval reports | F | UTML-001 | open Phase F reporting |

**Phase B/E code/FSV status (2026-05-13):** primary prediction/API/schema/label/UTML carrier work is complete with FSV at `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json` and `/mnt/d/contextgraph/fsv/contextgraph-utml-fsv/utml-fsv-summary.json`; Phase E completes Fisher, dormancy, lambda ramp, and dissipation with evidence in `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`. Remaining quality/reporting work: per-cell state-transfer reporting, full analyzer/toolchain label deltas, and holdout metrics.

---

## Phase C — Corpus Completion (4 tasks, ops-heavy)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-CORPUS-001` | Per-language mutation operators (10 langs × 8 = 80) | C | none | **complete (2026-05-12; real-toolchain verified)** |
| `TASK-CORPUS-002` | 300×8 SWE-bench Lite Python corpus generation under `/mnt/d/contextgraph/corpus/` (Docker) | C | none | **complete (2400/2400 verified, 2026-05-12)** |
| `TASK-CORPUS-003` | Per-language fixture corpora (64 tasks per language × 8 categories) | C | CORPUS-001 | **complete (5120/5120 verified, 2026-05-12)** |
| `TASK-CORPUS-004` | Combined split applied to Python + non-Python corpora | C | CORPUS-002, CORPUS-003 | **complete (split FSV passed, 2026-05-12)** |

**Phase C status:** complete. Sources of truth: Python corpus `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`, non-Python corpus `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/`, combined evidence `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`.

---

## Phase D — Neural Embedder Forwards (7 tasks)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-ENRICH-001` | `EmbedderForward` trait + active content forwards: pretrained E1/E6/E7/E8/E10/E12/E13/E14, algorithmic E2/E3/E4/E9 | D | none | **complete (2026-05-12; full pretrained/routing FSV passed)** |
| `TASK-ENRICH-002` | DDA signal computation per chunk with per-embedder cosine, strict upper-triangle pairwise cosine/MI, blind-spot z-scores | D | ENRICH-001, FLYWHEEL-006 | **complete (2026-05-12)** |
| `TASK-ENRICH-003` | Populate the new CFs with real signals | D | ENRICH-002 | **complete (2026-05-12; live algorithmic DDA rows FSV writes/readbacks 100 shifts)** |
| `TASK-ENRICH-004` | Embedder cache layer | D | ENRICH-001 | **complete (2026-05-12; persistent cache, corrupt-entry fail-closed, LRU/telemetry evidence closed in #17)** |
| `TASK-ENRICH-005` | VRAM budget gate enforcement at startup | D | ENRICH-001 | **complete for Phase A-D (production telemetry window + VRAM readback FSV)** |
| `TASK-ENRICH-006` | Per-embedder FSV (real model file, deterministic input → expected output) | D | ENRICH-001 | **complete (2026-05-12; 8 pretrained + 4 algorithmic + 143 routing rows read back)** |
| `TASK-ENRICH-007` | Per-language stdlib registry for `ReinventedWheel` | D | none | **complete (2026-05-12)** |

**Phase D/E status (updated 2026-05-13):** enrichment substrate is code/FSV-shipped for full pretrained/routing forwards, DDA compute/store, live shift-subscriber DDA rows, persistent cache/LRU telemetry, inference CF opening, TCT refresh support, production telemetry windows, analyzer-backed semantic heads, UTML carrier evidence, and stdlib registry. Autonomous Phase E now ships on top of that substrate. The non-green 0.95 prediction-oracle threshold remains Phase G convergence work.

---

## Phase E — Continuous Self-Optimization (code/FSV shipped 2026-05-13)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-HEAL-001` | Self-optimization scheduler full implementation | E | Phase A-D substrate verified | **complete/code+FSV** |
| `TASK-HEAL-002` | Per-cell stratified drift detection | E | HEAL-001 | **complete/code+FSV** |
| `TASK-HEAL-003` | Dissipation detection | E | HEAL-001, UTML-008 | **complete/code+FSV** |
| `TASK-HEAL-004` | Bayesian small-window drift (Jeffreys/Beta posterior) | E | HEAL-002 | **complete/code+FSV** |
| `TASK-HEAL-005` | Production constellation refresh policy | E | HEAL-001 | **complete/code+FSV** |
| `TASK-HEAL-006` | `mejepa_promote_approval` MCP tool | E | HEAL-001 | **complete/code+FSV** |
| `TASK-HEAL-007` | Emergency eviction policy validation | E | none | **complete/code+FSV** |
| `TASK-HEAL-008` | Drift attribution and alert policy stream | E | HEAL-002 | **complete/code+FSV** |
| `TASK-UTML-008` | Per-head dissipation detection | E | UTML-001 | **complete/code+FSV** |

**Phase E FSV source of truth:** `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`, with durable readback from `CF_MEJEPA_MODEL_PROMOTIONS`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DRIFT_WINDOW`, and `self_optimization_status.json`.

---

## Phase F — Production Observability (10 tasks)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-OBS-001` | Weekly evaluation production feed/dashboard/runbook (daemon scheduler + writer exist) | F | HEAL-002, UTML-009 | 1d |
| `TASK-OBS-002` | Pairwise MI heatmap renderer | F | none | 0.5d |
| `TASK-OBS-003` | Daemon status aggregator MCP tool | F | FLYWHEEL-001 | 0.5d |
| `TASK-OBS-004` | Per-prediction-class running calibration tracker | F | PREDICT-005 | 1d |
| `TASK-OBS-008` | WSL GPU telemetry hardening | F | none | 1-2d |
| `TASK-OBS-009` | `mejepa_inspect_prediction` MCP tool with full provenance trace | F | PREDICT-005 | 0.5d |
| `TASK-OBS-010` | Active-learning operator CLI | F | none | 0.5d |

**Phase F total: ~6 days.**

---

## Phase H — Inner-Loop Residue Cleanup (4 tasks)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-LOOP-003` | Mark superseded SMAP decisions | H | LOOP-001 | 0.25d |
| `TASK-LOOP-004` | Retire legacy MCP tools | H | none | 0.25d |
| `TASK-LOOP-005` | Audit absence of inner-actor model files | H | none | 0.1d |
| `TASK-LOOP-006` | Delete stale `target/release/reality-loop` binary | H | none | 0.1d |

**Phase H total: ~0.7 days.**

---

## Total task count: **~115 atomic tasks across 13 domains.**

| Domain | Task count | Effort sum |
|---|---|---|
| FLYWHEEL | 7 | ~5.5 days |
| PREDICT | 12 + 11 LABEL | ~12 days |
| ENRICH | 7 | ~13 days |
| LOOP | 7 | ~1.5 days |
| HEAL | 8 | ~6.5 days |
| CORPUS | 4 | ~10 days |
| OBS | 16 | ~12 days |
| UTML | 9 | ~3.5 days |
| TESTING | 11 | ~13 days |
| RUNBOOK | 12 | ~5.5 days |
| DR | 8 | ~4 days |
| SECURITY | 8 | ~3.5 days |
| BOOTSTRAP | 3 | ~2 days |
| API | 5 | ~2.5 days |
| **Total** | **~115** | **~95 days** |

> The original 8 phases (A–H per `12_IMPLEMENTATION_PHASES.md`) absorb the new TESTING / RUNBOOK / DR / SECURITY / BOOTSTRAP / API domains: TESTING + RUNBOOK + DR map mostly to Phase F (production observability) and Phase A (basic readiness); SECURITY tasks distribute across Phases A, B, E; BOOTSTRAP fits in Phases A and C; API stretches across A and F. The total fresh implementation effort grows from ~57 to ~95 engineer-days, then Phase G runs continuously.

> Per-domain task detail: see [`tasks/<domain>/_all_tasks.md`](tasks/) for compact entries; Phase A has detailed individual task files for the 8 critical-path items.

---

## Dependency DAG (high-level)

```
FLYWHEEL-001  ──┬─→ FLYWHEEL-002 ──→ HEAL-001 ──→ HEAL-002,003,004,005,007
                │
                ├─→ PREDICT-001 (injection hook)
                │
                ├─→ LOOP-001 ──→ LOOP-002,003
                │
                └─→ LOOP-007 (hook output paths)

FLYWHEEL-005,006 ──→ PREDICT-002 ──→ UTML-004

FLYWHEEL-003 ──→ FLYWHEEL-004 (oracle binding)

PREDICT-005 (multi-head) ──┬─→ PREDICT-003,004,006,007
                            ├─→ PREDICT-LABEL-009
                            ├─→ UTML-001 ──┬─→ UTML-002,003,005,006,008
                            │              └─→ UTML-007 ←── HEAL-001
                            └─→ OBS-004,009

CORPUS-001 ──→ CORPUS-003 ──→ CORPUS-004 ←── CORPUS-002

ENRICH-001 ──→ ENRICH-002 ──→ ENRICH-003
            ──→ ENRICH-004,005,006

OBS-001 ←── HEAL-002, UTML-009

LOOP-* (cleanup) — independent except LOOP-002 ←── LOOP-001
```

---

## Parallelization opportunities

These tasks can run in parallel without conflict:

- Phase C is complete; future corpus broadening can parallelize by language if adding scraped/public CI corpora beyond the deterministic fixture seed.
- Per-embedder FSV tasks within `TASK-ENRICH-006` (Phase D) — one per embedder
- Per-head label-source tasks `TASK-PREDICT-LABEL-001..009` (Phase B) — independent
- Cleanup tasks in Phase H — all independent
- All OBS tasks (Phase F) — mostly independent

A two-engineer pairing could reduce total wall-clock to ~20-25 days from the ~57 single-engineer-day total, since most remaining Phase B label hardening, Phase D per-embedder FSVs, Phase F observability tasks, and any future corpus broadening are embarrassingly parallel.

---

## Atomic task file convention

Each `tasks/<domain>/TASK-<DOMAIN>-NNN-<slug>.md` follows the [`prdtospec.md`](../../docs2/prdtospec.md) Level-4 format:

```yaml
---
task_id: TASK-EXAMPLE-001
title: Example task title
phase: A
status: open|code_complete|complete
blocked_by: []
blocks: [TASK-EXAMPLE-002]
effort: 0.5d
spec_refs: [SPEC-EXAMPLE.md, TECH-EXAMPLE.md]
---

## Goal
...

## Inputs (read these)
- crates/context-graph-mcp/src/main.rs
- crates/context-graph-mejepa-shift-subscriber/src/lib.rs
- ...

## Outputs (write these)
- crates/context-graph-mcp/src/daemon.rs (NEW)
- crates/context-graph-mcp/src/main.rs (extended with --daemon flag)
- scripts/systemd/mejepa-daemon.service (NEW)

## Definition of done
- ...

## FSV evidence path
- /mnt/d/contextgraph/fsv/mcp-daemon-fsv/...
```

The atomic task files are under [`tasks/`](tasks/) — see the directory tree at `tasks/<domain>/`.

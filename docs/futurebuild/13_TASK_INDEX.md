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

The A-F and H implementation issues are closed for the Phase G handoff. Analyzer-backed instrument parity (#4), production telemetry (#11), UTML carrier reconciliation (#13), Phase E self-optimization, Phase F operations, and Phase H residue cleanup all have source-of-truth FSV. The latest holdout remains `0.866667 < 0.95`; that is Phase G model-quality convergence, not an A-F/H implementation blocker. The active queue is now Python-first ME-JEPA-Code convergence: close the 8 Python mutation-category cells on the 300×8 SWE-bench Lite Python corpus before deferred non-Python broadening.

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
| `TASK-FLYWHEEL-010` | Unified supervised-task health registry powering `/ready` | F | FLYWHEEL-001, OBS-014 | **complete (#87)** — `supervised_task_health.json` records heartbeat/restart health for all daemon tasks; `/ready` aggregates it |
| `TASK-PREDICT-001` | UserPromptSubmit prediction-injection hook | A | FLYWHEEL-001 | **complete (2026-05-12)** |
| `TASK-PREDICT-002` | `mejepa_record_agent_feedback` MCP tool + sampling-weight bump wiring | A | FLYWHEEL-005, FLYWHEEL-006 | **complete (2026-05-12; no feedback rate limit)** |
| `TASK-LOOP-001` | Apply new `.claude/loop.md` content from `14_NEW_LOOP_MD.md` | A | FLYWHEEL-001 | **complete (already applied)** |
| `TASK-LOOP-002` | Update CLAUDE.md §11.11/§13/§3 + agent README retirements | H | LOOP-001 | moved to Phase H residue cleanup; not an A-D ME-JEPA substrate blocker |
| `TASK-LOOP-007` | Hook output paths → D-drive `cgreality-shift-log/` | A | none | **complete (2026-05-11 — code already shipped, audit doc updated)** |
| `TASK-OBS-005` | C-drive lint in CI | A | none | **complete (2026-05-12)** |
| `TASK-OBS-006` | FSV path migration sweep | A | none | **complete (2026-05-12)** |
| `TASK-OBS-007` | Per-slice quota observability extension | F | none | **complete** — `mejepa_daemon_status` and `mejepa_quota_status` expose quota slices; storage-exhaustion/session-cleanup FSV read back quota state |

**Phase A substrate status:** complete for the ME-JEPA Phase E/G handoff. The moved Phase H residue rows are now closed.

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
| `TASK-PREDICT-LABEL-010` | Toolchain availability fail-closed audit | F | none | **complete (#61 + startup gate follow-up)** |
| `TASK-PREDICT-LABEL-011` | Cross-language label gating | F | PREDICT-LABEL-001..009 | **complete (#62)** |
| `TASK-PREDICT-009` | `ActiveLearningKind::AgentSurprise` integration (consumer side) | B | FLYWHEEL-005 | **complete (2026-05-11/12)** |
| `TASK-UTML-001` | Per-head ΔP/ΔK/ΔΞ tracking | B | PREDICT-005 | **complete/code_complete (2026-05-12)** |
| `TASK-UTML-002` | Per-head cert columns in `CF_MEJEPA_TRAIN_CERTS` | B | UTML-001 | **complete for Phase A-D (2026-05-12; cert readback + carrier fields FSV)** |
| `TASK-UTML-003` | `ΔΞ_global = min_h(ΔΞ_h)` aggregation | B | UTML-001 | **complete for persisted learning-signal carrier; BatchSampler policy belongs to Phase E** |
| `TASK-UTML-004` | `agent_surprise_severity_score` factor in sampling_weight | B | PREDICT-002 | **complete** |
| `TASK-UTML-005` | Per-head Fisher matrices | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-006` | Per-head dormancy detection | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-007` | Per-head loss coefficient λ ramp in heal scheduler | E | HEAL-001, UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-008` | Per-head dissipation detection | E | UTML-001 | **complete/code+FSV (2026-05-13)** |
| `TASK-UTML-009` | Per-cell state-transfer T in eval reports | F | UTML-001 | **complete (#63)** — weekly eval reports persist per-cell state-transfer diagnostics; FSV at `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json` |

**Phase B/E/F code/FSV status (2026-05-13):** primary prediction/API/schema/label/UTML carrier work is complete with FSV at `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json` and `/mnt/d/contextgraph/fsv/contextgraph-utml-fsv/utml-fsv-summary.json`; Phase E completes Fisher, dormancy, lambda ramp, and dissipation with evidence in `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`; Phase F completes per-cell state-transfer reporting with evidence in `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json`. Remaining quality work belongs to Phase G convergence: full analyzer/toolchain label deltas and holdout metrics.

---

## Phase C — Corpus Completion (4 tasks, ops-heavy)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-CORPUS-001` | Per-language mutation operators (10 langs × 8 = 80) | C | none | **complete (2026-05-12; real-toolchain verified)** |
| `TASK-CORPUS-002` | 300×8 SWE-bench Lite Python corpus generation under `/mnt/d/contextgraph/corpus/` (Docker) | C | none | **complete (2400/2400 verified, 2026-05-12)** |
| `TASK-CORPUS-003` | Per-language fixture corpora (64 tasks per language × 8 categories) | C | CORPUS-001 | **complete (5120/5120 verified, 2026-05-12)** |
| `TASK-CORPUS-004` | Combined split applied to Python + archived non-Python fixture corpora | C | CORPUS-002, CORPUS-003 | **complete (split FSV passed, 2026-05-12); active ship gate consumes Python only** |

**Phase C status:** complete. Sources of truth: Python corpus `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`, archived non-Python fixture corpus `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/`, combined evidence `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json`. Phase G uses the Python corpus as the active ship corpus.

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

## Phase F — Production Observability + Operator Runbook + DR + API (complete 2026-05-13)

> **Status (2026-05-13).** Phase F expanded well beyond the original 10 OBS tasks during multi-agent execution. The backlog spans CLI, observability, DR, API stability, security, integration testing, and follow-up consumer wiring. Source-of-truth tracker readback on 2026-05-13 shows **0 open Phase F implementation issues**: all 37 atomic Phase F issues plus all 15 `phase:f-followup` issues are closed with FSV evidence under `/mnt/d/contextgraph/fsv/`.

### Closed OBS tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-OBS-001` | Weekly evaluation production feed/dashboard/runbook | **complete** — daemon weekly-eval scheduler runs; writer persists `CF_MEJEPA_EVAL_REPORTS` + `/mnt/d/contextgraph/exports/eval/<date>/weekly.{md,json}` |
| `TASK-OBS-002` | Pairwise MI heatmap renderer | **complete** — `mejepa_audit_pairwise_mi` MCP tool + CSV-to-PNG/markdown renderer |
| `TASK-OBS-003` | Daemon status aggregator MCP tool | **complete** — `mejepa_daemon_status` aggregates subscriber + heal + quota + VRAM |
| `TASK-OBS-004` | Per-prediction-class running calibration tracker | **complete** — `eval::per_head_calibration_tracker.rs` exposes rolling ECE per head |
| `TASK-OBS-007` | Prediction explanation surface completion (re-scoped) | **complete (#42)** — `mejepa_explain_prediction` + `mejepa_inspect_prediction` cover the full provenance trace |
| `TASK-OBS-008` | WSL GPU telemetry hardening | **complete** — `crates/context-graph-mcp/src/telemetry/gpu_wsl.rs` with retry + N/A detection |
| `TASK-OBS-009` | `mejepa_inspect_prediction` MCP tool | **complete (#44, commit 7f7e5c1d/e4baea50)** — full provenance trace including DDA chunks |
| `TASK-OBS-010` | Active-learning operator CLI | **complete (#45, commit 1f538ce1)** — `mejepa active-learning {list,label,dismiss}` — FSV sha256:a02788b1 |
| `TASK-OBS-011` | Prediction replay tool | **complete (#46)** — `mejepa replay --prediction-id <hex>` byte-for-byte replay |
| `TASK-OBS-012` | Reasoning-trace renderer | **complete (#47, commit 5662a744/a8474ace)** — `scripts/render_reasoning_trace.py` (HTML/markdown + saliency heatmap + counterfactuals) |
| `TASK-OBS-013` | System cost telemetry (resource counters, no spend caps) | **complete (#48, commit 0379df9c)** — `crates/context-graph-mejepa/src/system_cost.rs` tracks CUDA µs / tokens / RocksDB bytes; explicitly forbids `cost_estimate_usd` |
| `TASK-OBS-014` | Health + readiness HTTP probes | **complete (#49, commit 5aebf4ac/fcd3234f)** — `crates/context-graph-mcp/src/health_probe.rs`; opt-in via `CONTEXT_GRAPH_HEALTH_PROBE=1` |
| `TASK-OBS-015` | Log rotation policy | **complete (#50, commit 366d7e2a)** — `scripts/systemd/mejepa-log-rotate.{service,timer}` daily 03:15 |
| `TASK-OBS-016` | Executive summary renderer | **complete (#51, commit 5662a744)** — `scripts/render_executive_summary.py`; ≤200 words per render |

### Closed RUN (operator runbook) tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-RUN-002` | Operator CLI: daemon command | **complete (#73, commit 4d801592)** — `mejepa daemon-status` reads `self_optimization_status.json` |
| `TASK-RUN-003` | Operator CLI: weekly report command | **complete (#74)** — `mejepa report-weekly --date <YYYY-MM-DD>` reads `CF_MEJEPA_EVAL_REPORTS` |
| `TASK-RUN-005` | Operator CLI: hygiene command | **complete (#75, commit 366d7e2a)** — `mejepa hygiene {cold-audit, aggressive-cold-tier-down}` |
| `TASK-RUN-006` | Operator CLI: rollback command | **complete (#76)** — `mejepa rollback --target-witness-offset <u64>` wraps `AbcPromoter::rollback_to` |
| `TASK-RUN-007` | Operator CLI: eval command | **complete (#77)** — `mejepa eval-ship-gate` reads latest EvalReport |
| `TASK-RUN-008` | Operator CLI: pause command | **complete (#78, commit baeef6b0)** — `mejepa pause --duration-mins <n>` + `mejepa_pause_predictions` MCP tool |
| `TASK-RUN-009` | Operator CLI: session cleanup command | **complete (#79, commit 0379df9c)** — `mejepa session-cleanup --session-id <32-hex>` |
| `TASK-RUN-012` | Incident response checklist verification | **complete (#80, commit a54803fa)** — `scripts/ci/runbook_coverage.sh` enforces `MEJEPA_*` alert ↔ code parity |

### Closed DR (disaster recovery) tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-DR-003` | Storage verify CLI | **complete (#54, commit 9ccec509)** — `mejepa storage-verify --db-path <p>` with `set_paranoid_checks(true)` + per-CF row counts |
| `TASK-DR-004` | Storage restore-cf CLI | **complete (#55)** — `mejepa storage restore-cf` replaces target CF rows from a source DB with runbook FSV readback |
| `TASK-DR-007` | Restore verification monthly cron | **complete (#56, commit f036afac)** — `scripts/dr/verify_backup.sh` + `scripts/systemd/mejepa-dr-verify.{service,timer}` (first Sunday 04:00) |
| `TASK-DR-008` | Quarterly DR drill | **complete (#57, commit c710dfc4)** — `scripts/dr/dr_drill.sh` restore + corrupt SST + re-restore + RTO budget check |

### Closed API stability tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-API-002` | Storage migrate-cf command | **complete (#58)** — `mejepa storage migrate-cf` copies rows from source to target CF with fail-closed non-empty target handling |
| `TASK-API-004` | Deprecation framework | **complete (#59, commit edf19c75)** — `crates/context-graph-mcp/src/deprecation.rs`; default warn / opt-in `CONTEXT_GRAPH_DEPRECATION_LEVEL=error` |

### Closed PREDICT + label + UTML tasks (Phase F hardening)

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-PREDICT-012` | Operator override prediction tool | **complete (#60, commit 377f0884)** — `operator_override.rs` + `CF_MEJEPA_OPERATOR_OVERRIDES` + `mejepa_operator_override_prediction` MCP tool; 6.0× sampling-weight multiplier persisted |
| `TASK-PREDICT-LABEL-010` | Toolchain availability fail-closed audit | **complete (#61, commit 0379df9c)** — `crates/context-graph-mejepa/src/toolchain_detect.rs`; 16-binary default catalogue across 11 languages |
| `TASK-PREDICT-LABEL-011` | Cross-language label gating | **complete (#62, commit 0379df9c)** — `crates/context-graph-mejepa/src/cross_language_label_gating.rs`; same-root-cause + similarity > 0.85 + correlation ≥ 0.60 ⇒ transfer at 0.5× weight |
| `TASK-PREDICT-LABEL-012` | Cross-language label gating consumer | **complete (#86)** — `label_transfer_audit` + `apply_phase_b_labels_with_transfer_gate()` + `CF_MEJEPA_LABEL_TRANSFER_DECISIONS`; weekly surfaces include accept/reject ratios per language pair |
| `TASK-UTML-009` | Per-cell state-transfer T in eval reports | **complete (#63)** — weekly eval reports carry per-cell state-transfer diagnostics; FSV sha256:63234e2d |

### Closed integration test tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-TEST-001` | End-to-end happy-path harness | **complete (#64, commit 062d413d/a0e77b38)** — `crates/context-graph-mejepa/tests/e2e.rs` + `tests/e2e/{happy_path.rs, harness.rs}` |
| `TASK-TEST-002` | Restart-safety E2E harness | **complete (#65)** — daemon restart + watermark replay FSV at `/mnt/d/contextgraph/fsv/phase-f-restart-safety-fsv/` |
| `TASK-TEST-003` | Multi-agent E2E harness | **complete (#66)** — concurrent-session isolation FSV at `/mnt/d/contextgraph/fsv/phase-f-multi-agent-fsv/` |
| `TASK-TEST-004` | Catastrophic-event E2E harness | **complete (#67)** — catastrophic-event FSV at `/mnt/d/contextgraph/fsv/phase-f-catastrophic-event-fsv/` |
| `TASK-TEST-005` | Storage-exhaustion E2E harness | **complete (#52)** — storage-exhaustion FSV at `/mnt/d/contextgraph/fsv/phase-f-storage-exhaustion-fsv/` |
| `TASK-TEST-006` | Chaos harness with failpoints | **complete (#53)** — failpoint chaos FSV at `/mnt/d/contextgraph/fsv/phase-f-chaos-failpoints-fsv/` |
| `TASK-TEST-007` | Load test harness | **complete (#68)** — load-test FSV at `/mnt/d/contextgraph/fsv/phase-f-load-test-fsv/` |
| `TASK-TEST-008` | Soak test harness + nightly cron | **complete (#69)** — soak-test FSV at `/mnt/d/contextgraph/fsv/phase-f-soak-test-fsv/` |
| `TASK-TEST-009` | Toolchain fail-closed matrix | **complete (#70, commit 237b4aef)** — `crates/context-graph-mejepa/tests/toolchain_fail_closed.rs` (5 matrix assertions across 16 binaries) |
| `TASK-TEST-010` | Adversarial corpus for security robustness | **complete (#71)** — adversarial-corpus FSV at `/mnt/d/contextgraph/fsv/phase-f-adversarial-corpus-fsv/` |
| `TASK-TEST-011` | CI workflow updates for Phase F harnesses | **complete (#72, commit a56ceda3)** — `.github/workflows/{e2e,chaos,load,soak}.yml`; ubuntu-latest only; conditional harness discovery |

### Closed Phase F follow-up hardening tasks

| ID | Title | Status / Evidence |
|---|---|---|
| `TASK-PREDICT-013` | Pause-state consumer in `MeJepaCompiler::verify` | **complete (#82)** |
| `TASK-PREDICT-014` | Operator-override BatchSampler 6x upweight consumer | **complete (#83)** |
| `TASK-OBS-017` | `SystemCostCounters` producer wiring | **complete (#84)** |
| `TASK-FLYWHEEL-009` | Toolchain-audit startup fail-closed gate | **complete (#85)** |
| `TASK-PREDICT-LABEL-012` | Cross-language gating consumer + decisions audit CF | **complete (#86)** |
| `TASK-FLYWHEEL-010` | Unified `SupervisedTaskHealth` registry powering `/ready` | **complete (#87)** |
| `TASK-API-006` | Deprecation framework call-site audit | **complete (#88)** |
| `TASK-RUN-014` | Runbook actionability lint | **complete (#89)** |
| `TASK-FLYWHEEL-011` | Witness chain auto-quarantine on integrity-check failure | **complete (#90)** |
| `TASK-RUN-013` | Session-cleanup quota/GC-history reconciliation | **complete (#91)** |
| `TASK-SEC-009` | Agent identity attestation | **complete (#92)** |
| `TASK-HYG-002` | Retention policy for feedback + override CFs | **complete (#93)** |
| `TASK-OBS-018` | Heal-scheduler ticker resource burn telemetry | **complete (#94)** |
| `TASK-TEST-012` | Pin Phase F workflow Actions to SHA | **complete (#95)** |
| `TASK-FLYWHEEL-012` | Pause + active-learning enqueue suppression | **complete (#96)** |

**Phase F closure status (2026-05-13):** closed. GitHub issue readback shows no open issues with `phase:f-followup` and no open `[Phase F]` implementation issues. Remaining open work is Phase G convergence, not Phase F implementation.

**Phase F FSV index (selected evidence files):**

- `/mnt/d/contextgraph/fsv/phase-f-active-learning-cli-fsv/active_learning_cli_fsv.json` — sha256:a02788b1
- `/mnt/d/contextgraph/fsv/phase-f-runbook-cli-fsv/runbook_cli_fsv.json` — sha256:9c6cce66a2c227ce12479b690b33bcfa12cdfdaede3e375588ef3bdde059f706 (`all_passed=true`, `readback_equal=true`)
- `/mnt/d/contextgraph/fsv/phase-f-renderers-fsv/renderers_fsv.json` — sha256:6fe3b26e9bcbb3b418e5942a4e3038187ab8af627dedd15b3c7c6f880bc8a460
- `/mnt/d/contextgraph/fsv/phase-f-deprecation-framework-fsv/deprecation_framework_fsv.json` — sha256:180bb18146e2efea31203794a8712fa616a33c9ef053c36d7c7b505d4f36f23d
- `/mnt/d/contextgraph/fsv/phase-f-health-probe-fsv/health_probe_fsv.json` — sha256:ae96204da5a9ba6529775c1fa7fa1bc4c68aa671918fa90de8d38c9c0a9d2fac
- `/mnt/d/contextgraph/fsv/phase-f-toolchain-availability-fsv/toolchain_availability_fsv.json` — sha256:02b6e6fbf9e6d647647837741a94e94d5edafd6f4fb2b537a9acdcfe18284667
- `/mnt/d/contextgraph/fsv/phase-f-cross-language-label-gating-fsv/cross_language_label_gating_fsv.json` — sha256:873764c6e7504b1626c0cbb024d8edd809036af2e7666f5c397df89bd3ccaf34
- `/mnt/d/contextgraph/fsv/phase-f-label-transfer-gating-fsv/label_transfer_consumer_fsv.json` — sha256:8edc6686832b28360b974e84eb067c0821e3731d100f5d24319259f5e52bce9a
- `/mnt/d/contextgraph/fsv/phase-f-system-cost-fsv/system_cost_fsv.json` — sha256:cb76b6ee937fc9a968511db158e400abca0aaeb344643387bc6ce0abc6212b6b
- `/mnt/d/contextgraph/fsv/phase-f-operator-override-fsv/operator_override_fsv.json` — sha256:5eea66f1744b8e954a54ab2b9ea0be334e95861e4f73fc037b6977d91a46f99a
- `/mnt/d/contextgraph/fsv/phase-f-pause-cli-fsv/pause_cli_fsv.json` — sha256:1228d14e4a56afa7a67f8c17fd2fdeb5d2103c67e052d2fcfd8729d800b6df63
- `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json` — sha256:63234e2d1b492a18ca4533d750a1aa589206b02b060e4f0c3546654f4f078e0e
- `/mnt/d/contextgraph/fsv/phase-f-restart-safety-fsv/restart_safety_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-multi-agent-fsv/multi_agent_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-catastrophic-event-fsv/catastrophic_event_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-storage-exhaustion-fsv/storage_exhaustion_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-chaos-failpoints-fsv/chaos_failpoints_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-load-test-fsv/load_test_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-soak-test-fsv/soak_test_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-adversarial-corpus-fsv/adversarial_corpus_fsv.json`
- `/mnt/d/contextgraph/fsv/phase-f-agent-identity-fsv/agent_identity_mcp_fsv.json` — sha256:1184ff5264454a1e78df6405cfd297370d0331d907be228e74e10f48e77c86d5
- `/mnt/d/contextgraph/fsv/phase-f-retention-fsv/feedback_override_retention_fsv.json` — sha256:80c285e6a2297492e5149b525ffc27bafb2170279321bba13b56115a9778f477
- `/mnt/d/contextgraph/fsv/phase-f-witness-quarantine-fsv/witness_quarantine_fsv.json`

---

## Phase G — Python ME-JEPA-Code Ship-Gate Convergence (active issue set)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-G-001` | End-to-end corpus training-pipeline orchestrator (`context-graph-cli mejepa train --corpus <root> --split <train\|calibration\|holdout>`) | G | Phase C + D substrate | **complete (#97)** — writes/readbacks `CF_MEJEPA_PANELS`, `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, and `CF_MEJEPA_TRAIN_CERTS`; FSV at `/mnt/d/contextgraph/fsv/phase-g-training-pipeline-orchestrator-fsv/training_pipeline_orchestrator_fsv.json` |
| `TASK-G-002` | Python per-mutation-category cell convergence verification driver (`context-graph-cli mejepa cell-status --threshold 0.95`) | G | `CF_MEJEPA_EVAL_REPORTS` | **complete (#98)** — reads the latest eval report, lists Python mutation-category cells with correlation, holdout count, distance to threshold, last update timestamp, and exits non-zero for below-threshold or missing cells; FSV at `/mnt/d/contextgraph/fsv/phase-g-cell-status-fsv/cell_status_fsv.json`; follow-up `TASK-PY-G-001` hardens the public surface to reject stale 80-cell gate assumptions |
| `TASK-G-003` | Targeted training-data expansion for `off_by_one`, `subtle_flip`, and `swap_variable` Python cells | G | Mutation generators | **complete (#99)** — expands source-level operators for range/slice boundaries, membership/identity/any-all flips, function parameters, tuple assignment, loop locals, and zip/f-string variable uses; FSV materializes 600 rows per target category, validates every generated module with `py_compile`, and proves original/pass vs mutated/fail through pytest at `/mnt/d/contextgraph/fsv/phase-g-targeted-mutation-expansion-fsv/targeted_mutation_expansion_fsv.json` |
| `TASK-G-004` | Conformal recalibration after each A/B/C promotion | G | Phase E promotion path | **complete (#100)** — `AbcPromoter::commit` calls `recalibrate_conformal()` before active weights flip, persists `target_coverage=0.90` and empirical coverage to `CF_MEJEPA_CALIBRATION_HISTORY`, and blocks coverage outside `[0.88, 0.92]` with `MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND`; FSV at `/mnt/d/contextgraph/fsv/phase-g-conformal-recalibration-fsv/conformal_recalibration_promotion_fsv.json` |
| `TASK-G-005` | Holdout corpus rotation policy to avoid stale holdout leakage | G | Phase E promotion path | **complete (#101)** — after every four successful promotions, deterministically moves 10% of train into holdout and 10% of holdout into train, persisting the rotation event and full after-split readback to `CF_MEJEPA_HEAL_REPORTS`; FSV at `/mnt/d/contextgraph/fsv/phase-g-holdout-rotation-fsv/holdout_rotation_fsv.json` |
| `TASK-G-006` | Four-consecutive-rolling-window stability assertion plus ship gate | G | `CF_MEJEPA_EVAL_REPORTS`, `CF_MEJEPA_MODEL_PROMOTIONS`, `mejepa_ship_gate_status` | **complete (#102)** — MCP ship-gate status now returns `consecutivePassingWindows`, resets on failing windows or model-promotion reset events, and requires four consecutive 0.95+ windows before `shipGatePassed=true`; FSV at `/mnt/d/contextgraph/fsv/phase-g-ship-gate-stability-fsv/ship_gate_stability_fsv.json` |
| `TASK-G-007` | Per-cell exemption policy for impossible cells | G | `config/cell_exemptions.toml`, `mejepa_ship_gate_status` | **complete (#103)** — MCP ship-gate status loads active operator-approved exemptions, excludes exempt cells from the effective ship-gate correlation, filters exempt per-cell failures, and still reports the exempt cell correlation plus reason for visibility; FSV at `/mnt/d/contextgraph/fsv/phase-g-cell-exemptions-fsv/cell_exemptions_fsv.json` |
| `TASK-G-008` | Ship-gate operator dashboard with Python per-cell drill-down | G | `context-graph-cli mejepa cell-status`, `mejepa_ship_gate_status` | **complete (#104)** — `scripts/render_ship_gate_dashboard.py` renders markdown/HTML from `CF_MEJEPA_EVAL_REPORTS`-backed JSON with per-cell heatmap, holdout counts, last updates, statuses, exemptions, and representative prediction links; FSV at `/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv/ship_gate_dashboard_fsv.json`; follow-up `TASK-PY-G-035` makes the final bundle Python-gate specific |
| `TASK-G-009` | Production telemetry -> training feedback loop wiring | G | `CF_MEJEPA_PRODUCTION_TELEMETRY`, `CF_MEJEPA_HEAL_REPORTS` | **complete (#105)** — the `telemetry_feedback` heal-scheduler ticker reads the latest production telemetry window, queues recalibration when conformal ECE implies coverage outside `[0.88, 0.92]`, queues a full-retrain candidate when prediction-oracle agreement is below `0.95`, and persists typed `HealReport` actions; FSV at `/mnt/d/contextgraph/fsv/phase-g-telemetry-feedback-fsv/telemetry_feedback_fsv.json` |
| `TASK-G-010` | Adversarial-corpus retrain cycle integration | G | TASK-TEST-010 adversarial fixtures, BatchSampler | **complete (#106)** — `BatchSampler` reserves the configured 20% adversarial mix-in, training certificates persist adversarial counts/indices/fallbacks, and the Phase F adversarial corpus is replayed through the retrain-cycle FSV with `non_pass_rate=1.0`; FSV at `/mnt/d/contextgraph/fsv/phase-g-adversarial-retrain-fsv/phase_g_adversarial_retrain_fsv.json` |
| `TASK-G-011` | Per-cell state-transfer T diagnostic in weekly eval | G | TASK-UTML-009 | **complete (#107)** — weekly eval markdown renders a `state_transfer_per_cell` table from persisted per-cell diagnostics, including unavailable and low-transfer cells; FSV at `/mnt/d/contextgraph/fsv/phase-g-state-transfer-weekly-markdown-fsv/state_transfer_weekly_markdown_fsv.json` |
| `TASK-G-012` | Cross-validate Python holdout correlation on a public CI corpus | G | TASK-G-001 | **complete (#108)** — `context-graph-cli mejepa cross-validate-public-ci` identifies GHALogs, ingests a 560-row public-CI subset through the same panel/DDA/prediction/oracle/cert writer, compares public-CI cells against a Lite baseline, and proves reopened CF readback; FSV at `/mnt/d/contextgraph/fsv/phase-g-public-ci-cross-validation-fsv/public_ci_cross_validation_fsv.json` |
| `TASK-G-013` | Drift attribution root-cause classification for failing cells | G | `CF_MEJEPA_EVAL_REPORTS`, TASK-G-002 | **complete (#109)** — failing eval cells are classified as insufficient training data, embedder signal gap, label noise, oracle flakiness, distribution shift, or unknown; weekly markdown renders the source-backed root-cause table; FSV at `/mnt/d/contextgraph/fsv/phase-g-failing-cell-root-cause-fsv/failing_cell_root_cause_fsv.json` |
| `TASK-G-014` | Re-baseline calibration after operator overrides | G | TASK-PREDICT-012, `SystemCostCounters`, `telemetry_feedback` | **complete (#110)** — the `telemetry_feedback` tick tracks `overrides_consumed_since_calibration` from the operator-override sampler counter, queues `TriggerReason::OperatorOverrideRecalibration` after >100 consumed overrides, dedupes the current threshold crossing, and preserves a watermark for future 101-override windows; FSV at `/mnt/d/contextgraph/fsv/phase-g-operator-override-recalibration-fsv/operator_override_recalibration_fsv.json` |
| `TASK-G-015` | Label-quality audit for active Python cells plus archived language evidence | G | `CF_MEJEPA_FAILURE_EXEMPLARS` | **complete (#111)** — `scripts/audit_label_quality.py` reads `CF_MEJEPA_FAILURE_EXEMPLARS` via `ldb scan --hex`, reports labels-per-row mean/min/max, non-trivial-signal ratio, unavailable rows, and unknown-language rows to `/mnt/d/contextgraph/exports/eval/<date>/label_quality.md`; FSV at `/mnt/d/contextgraph/fsv/phase-g-label-quality-fsv/label_quality_fsv.json`; Phase G consumes this as Python gate evidence |
| `TASK-G-016` | Per-cell promotion gate documentation + reporting | G | Phase E promotion path | **complete (#112)** — `AbcPromoter::evaluate` and the live A/B/C winner path now require every baseline holdout cell to hold-or-improve within 0.005 correlation tolerance before promotion; violating candidates fail closed with `MEJEPA_PROMOTION_CELL_REGRESSION`; FSV at `/mnt/d/contextgraph/fsv/phase-g-per-cell-promotion-gate-fsv/per_cell_promotion_gate_fsv.json` |
| `TASK-G-017` | Holdout-vs-training distribution drift monitor | G | `CF_MEJEPA_PRODUCTION_TELEMETRY`, `telemetry_feedback` | **complete (#113)** — `ProductionTelemetryWindow` persists the KL divergence between training-batch and holdout `(mutation_category, language)` distributions, validates the sustained-batch threshold, and the `telemetry_feedback` ticker emits `MEJEPA_TRAINING_HOLDOUT_DRIFT` as a durable `CF_MEJEPA_HEAL_REPORTS` action when `KL > 0.5` for at least 100 batches; FSV at `/mnt/d/contextgraph/fsv/phase-g-training-holdout-drift-fsv/training_holdout_drift_fsv.json` |
| `TASK-G-018` | Weekly eval -> operator-actionable cell triage list | G | `weekly.md`, `EvalReport.failing_cell_classifications` | **complete (#114)** — weekly eval markdown renders an `Operator Cell Triage` top-10 table ranked by distance to the 0.95 cell target and maps root causes to operator actions (`expand_corpus`, `request_adversarial_sample`, `recalibrate`, `mark_exempt`); FSV at `/mnt/d/contextgraph/fsv/phase-g-weekly-cell-triage-fsv/weekly_cell_triage_fsv.json` |
| `TASK-G-019` | Oracle determinism / flakiness audit per cell | G | TASK-G-001, corpus oracle observations | **complete (#115)** — `mejepa-corpus verify --oracle-repeat-runs 3 --oracle-repeat-output <json>` produces repeat-oracle observations, `context-graph-cli mejepa oracle-flakiness-audit` writes `config/corpus_quarantine.toml`, and train ingestion skips quarantined tasks before train/holdout SoT writes; FSV at `/mnt/d/contextgraph/fsv/phase-g-oracle-flakiness-quarantine-fsv/oracle_flakiness_quarantine_fsv.json` |
| `TASK-G-020` | Per-failure-mode-class precision/recall holdout breakdown | G | `EvalReport`, `FailureModeClass` labels | **complete (#116)** — `EvalReport.per_failure_mode_class` carries precision, recall, F1, confusion counts, thresholds, and weakness strings for all 36 `FailureModeClass` values; weekly markdown and dashboard JSON render the map; FSV at `/mnt/d/contextgraph/fsv/phase-g-failure-mode-class-metrics-fsv/failure_mode_class_metrics_fsv.json` |
| `TASK-G-021` | Convergence ETA tracker per cell | G | `CF_MEJEPA_EVAL_REPORTS` rolling windows | **complete (#117)** — `eval/convergence_tracker.rs` fits a linear model over the last N eval windows, emits `estimated_passing_window` plus a confidence interval per cell, and renders the ETA in weekly triage/dashboard/cell-status surfaces; FSV at `/mnt/d/contextgraph/fsv/phase-g-convergence-eta-fsv/convergence_eta_fsv.json` |
| `TASK-PY-G-001..TASK-PY-G-040` | Remaining Python ME-JEPA-Code convergence tasks: Python reward-signal coverage, Q1-Q5 calibration, Python training-cycle execution, final burn-in, and post-green guardrails | G | TASK-G-001 where corpus rows are required | open GitHub issues #118-#131, #148-#154, #158-#159, #162-#173, #178-#182; non-Python #160/#161 are closed as deferred |

**Phase G status (2026-05-13):** active and Python-first. The orchestrator proves corpus rows can enter the source-of-truth reward pipeline with `input_task_count = output_panel_count = logical_dda_signal_count / 462 = oracle_verdict_count`. The current blocker is model-quality convergence: `prediction_oracle_correlation = 0.866667 < 0.95`. The open issue queue now exists to make the Python SWE-bench path real end to end: Python code changes are parsed into AST chunks, exploded through frozen instruments, DDA, and TCT, trained into calibrated Q1-Q5 `RealityPrediction`, served through local MCP stdio, fed back to AI agents, and continually improved by oracle outcomes, Edit shifts, and agent/operator feedback. Non-Python fixture expansion and proof-domain harnesses are explicitly deferred until this Python gate is green.

### Python Ship-Gate Dependency Map (TASK-PY-G-001 / #158)

This is the active execution-order map for the Python ME-JEPA-Code ship gate. The path starts at a SWE-bench Lite Python row and ends at a calibrated MCP `RealityPrediction`; gridworld, proof-domain, and non-Python harnesses are not active prerequisites. Any task, command, or report that reintroduces a stale 80-cell `(mutation_category x language)` gate into this Python-only path must fail closed through `TASK-PY-G-036` before it can influence release decisions.

Trigger -> Process -> Outcome for the active gate:

```text
SWE-bench Lite Python row
  -> Python AST chunks + typed task metadata
  -> frozen target instruments (oracle, witness, type/claim/impact panels)
  -> DDA/TCT reward-signal constellation
  -> checkpointed Python training + calibration
  -> Q1-Q5 RealityPrediction heads
  -> MCP stdio prediction surface
  -> durable feedback/replay loop
  -> four-window 0.95 Python ship-gate verdict
```

Boundary invariants for the dependency map:

- Empty or missing corpus input: no downstream task may synthesize rows; the runner must fail closed before panel/DDA/TCT writes.
- Malformed Python or invalid metadata: the row must be quarantined or rejected with a typed error; it must not enter calibration as a weak label.
- Maximum batch: the active corpus gate is `300 x 8 = 2400` Python mutation rows; max+1 is an error, not truncation.
- Scope guard: non-Python rows and proof-domain fixtures are archived evidence only until `TASK-PY-G-034` accepts the Python bundle.

| Order | Task | Issue | Stage | Direct predecessors | Direct successors | Source-of-truth state |
|---:|---|---:|---|---|---|---|
| 1 | `TASK-PY-G-001` Python ship-gate dependency map | #158 | graph | `TASK-G-001..021` complete Phase G substrate | `TASK-PY-G-036`, `TASK-PY-G-037`, `TASK-PY-G-002` | `docs/futurebuild/13_TASK_INDEX.md`, GitHub issue graph |
| 2 | `TASK-PY-G-036` Reject stale 80-cell gates | #178 | scope guard | `TASK-PY-G-001` | `TASK-PY-G-037`, `TASK-PY-G-002`, `TASK-PY-G-038` | CLI/MCP gate output rejects non-Python active gates |
| 3 | `TASK-PY-G-037` Isolate proof-domain harnesses | #179 | scope guard | `TASK-PY-G-001`, `TASK-PY-G-036` | `TASK-PY-G-002`, `TASK-PY-G-038` | proof-domain reports excluded from active Python release path |
| 4 | `TASK-PY-G-002` Checkpointed Python training runner | #159 | runner | `TASK-PY-G-001`, `TASK-PY-G-036`, `TASK-PY-G-037` | `TASK-PY-G-018`, `TASK-PY-G-022`, `TASK-PY-G-023` | checkpoint files + train/calibration/holdout CF readback |
| 5 | `TASK-PY-G-018` SWE-bench Verified Python subset | #149 | corpus | `TASK-PY-G-002` | `TASK-PY-G-022`, `TASK-PY-G-017`, `TASK-PY-G-004` | Python task rows and verified subset manifest |
| 6 | `TASK-PY-G-022` Python oracle pass/fail audit | #153 | oracle | `TASK-PY-G-018`, `TASK-PY-G-002` | `TASK-PY-G-017`, `TASK-PY-G-005`, `TASK-PY-G-010` | oracle verdict rows and flake/quarantine evidence |
| 7 | `TASK-PY-G-017` Python leakage detector | #148 | corpus guard | `TASK-PY-G-022`, `TASK-PY-G-018` | `TASK-PY-G-009`, `TASK-PY-G-030`, `TASK-PY-G-012`, `TASK-PY-G-013` | leakage report and quarantined row readback |
| 8 | `TASK-PY-G-009` Frozen instrument coverage audit | #124 | target panel | `TASK-PY-G-017` | `TASK-PY-G-007`, `TASK-PY-G-008` | frozen instrument coverage rows |
| 9 | `TASK-PY-G-007` Python embedder MI rotation | #122 | embedding | `TASK-PY-G-009` | `TASK-PY-G-008`, `TASK-PY-G-039` | MI rotation report + selected embedder config |
| 10 | `TASK-PY-G-008` Python TCT signal-density audit | #123 | TCT | `TASK-PY-G-009`, `TASK-PY-G-007` | `TASK-PY-G-039`, `TASK-PY-G-019` | TCT density report over Python cells |
| 11 | `TASK-PY-G-039` TCT completeness audit | #181 | TCT | `TASK-PY-G-008`, `TASK-PY-G-007` | `TASK-PY-G-003`, `TASK-PY-G-019` | per-row TCT completeness readback |
| 12 | `TASK-PY-G-019` Python sample-efficiency analysis | #150 | data quality | `TASK-PY-G-008`, `TASK-PY-G-039` | `TASK-PY-G-004`, `TASK-PY-G-020` | sample-efficiency report by mutation category |
| 13 | `TASK-PY-G-003` Per-head Q1-Q5 distillation | #118 | heads | `TASK-PY-G-039` | `TASK-PY-G-010`, `TASK-PY-G-011`, `TASK-PY-G-012`, `TASK-PY-G-013`, `TASK-PY-G-020` | Q1-Q5 head weights and calibration rows |
| 14 | `TASK-PY-G-010` Q1 claim existence at corpus scale | #125 | Q1 | `TASK-PY-G-003`, `TASK-PY-G-022` | `TASK-PY-G-027`, `TASK-PY-G-038` | claim existence predictions vs file-system SoT |
| 15 | `TASK-PY-G-011` Q3 failure-mode calibration | #126 | Q3 | `TASK-PY-G-003`, `TASK-PY-G-022` | `TASK-PY-G-006`, `TASK-PY-G-028`, `TASK-PY-G-038` | failure-mode labels and calibrated Q3 bins |
| 16 | `TASK-PY-G-012` Q4 concern confidence calibration | #127 | Q4 | `TASK-PY-G-003`, `TASK-PY-G-017` | `TASK-PY-G-006`, `TASK-PY-G-038` | security/perf/tech-debt concern calibration |
| 17 | `TASK-PY-G-013` Q5 impact vs shift-log | #128 | Q5 | `TASK-PY-G-003`, `TASK-PY-G-017` | `TASK-PY-G-038` | shift-log impact readback vs predicted Q5 impact |
| 18 | `TASK-PY-G-020` Predictor ablation harness | #151 | diagnostics | `TASK-PY-G-003`, `TASK-PY-G-019` | `TASK-PY-G-027`, `TASK-PY-G-031` | head/instrument ablation metrics |
| 19 | `TASK-PY-G-014` Python OOD detection gate | #129 | safety | `TASK-PY-G-003`, `TASK-PY-G-039` | `TASK-PY-G-027`, `TASK-PY-G-030` | OOD score distribution and fail-closed gate |
| 20 | `TASK-PY-G-006` Python conformal RCA classifier | #121 | RCA | `TASK-PY-G-011`, `TASK-PY-G-012` | `TASK-PY-G-015`, `TASK-PY-G-028`, `TASK-PY-G-031` | conformal RCA class rows |
| 21 | `TASK-PY-G-015` Python Gtau guard audit | #130 | calibration guard | `TASK-PY-G-006`, `TASK-PY-G-003` | `TASK-PY-G-027` | Gtau guard evidence and rejected-boundary rows |
| 22 | `TASK-PY-G-021` Python E_Trace gating | #152 | trace guard | `TASK-PY-G-003`, `TASK-PY-G-006` | `TASK-PY-G-027` | E_Trace gate rows and rejected traces |
| 23 | `TASK-PY-G-028` Python error taxonomy | #166 | errors | `TASK-PY-G-006`, `TASK-PY-G-011` | `TASK-PY-G-031`, `TASK-PY-G-034` | typed Python error taxonomy and dashboard mapping |
| 24 | `TASK-PY-G-005` Catastrophic Python cell quarantine | #120 | safety | `TASK-PY-G-022`, `TASK-PY-G-017` | `TASK-PY-G-030`, `TASK-PY-G-027` | quarantined catastrophic cell rows |
| 25 | `TASK-PY-G-023` Active-learning consumer | #154 | feedback | `TASK-PY-G-002`, `TASK-PY-G-022` | `TASK-PY-G-016`, `TASK-PY-G-024` | active-learning queue events |
| 26 | `TASK-PY-G-016` Feedback cooldown for Python | #131 | feedback guard | `TASK-PY-G-023` | `TASK-PY-G-024`, `TASK-PY-G-040` | cooldown state and duplicate/replay rejection |
| 27 | `TASK-PY-G-024` Feedback retrain consumer | #163 | feedback retrain | `TASK-PY-G-023`, `TASK-PY-G-016` | `TASK-PY-G-025`, `TASK-PY-G-040` | feedback replay rows and retrain trigger |
| 28 | `TASK-PY-G-040` Feedback replay-weight FSV | #182 | feedback FSV | `TASK-PY-G-024`, `TASK-PY-G-016` | `TASK-PY-G-034` | replay weight before/after and row equality |
| 29 | `TASK-PY-G-025` Eval-after-promotion hook | #162 | promotion | `TASK-PY-G-024`, `TASK-PY-G-040` | `TASK-PY-G-026`, `TASK-PY-G-027`, `TASK-PY-G-032` | promotion eval report persisted after candidate flip |
| 30 | `TASK-PY-G-026` Checkpoint provenance | #164 | provenance | `TASK-PY-G-025` | `TASK-PY-G-027`, `TASK-PY-G-033` | checkpoint manifest, hashes, train/eval commit |
| 31 | `TASK-PY-G-027` Python 0.95 confidence gate | #165 | gate | `TASK-PY-G-010`, `TASK-PY-G-014`, `TASK-PY-G-015`, `TASK-PY-G-020`, `TASK-PY-G-021`, `TASK-PY-G-026` | `TASK-PY-G-029`, `TASK-PY-G-032`, `TASK-PY-G-033`, `TASK-PY-G-038` | Python-only confidence/correlation gate |
| 32 | `TASK-PY-G-004` Python generalization audit | #119 | generalization | `TASK-PY-G-018`, `TASK-PY-G-019` | `TASK-PY-G-034` | unseen/generalization audit report |
| 33 | `TASK-PY-G-030` Unseen Python holdout quarantine | #169 | holdout guard | `TASK-PY-G-017`, `TASK-PY-G-005`, `TASK-PY-G-014` | `TASK-PY-G-032` | holdout quarantine manifest |
| 34 | `TASK-PY-G-031` Model-quality RCA dashboard | #170 | operator RCA | `TASK-PY-G-006`, `TASK-PY-G-020`, `TASK-PY-G-028` | `TASK-PY-G-034`, `TASK-PY-G-035` | RCA dashboard JSON/markdown |
| 35 | `TASK-PY-G-029` Live Python patch backfill | #168 | live data | `TASK-PY-G-027` | `TASK-PY-G-038`, `TASK-PY-G-032`, `TASK-PY-G-034` | live patch backfill rows and outcomes |
| 36 | `TASK-PY-G-032` Four-window Python burn-in | #167 | stability | `TASK-PY-G-025`, `TASK-PY-G-027`, `TASK-PY-G-029`, `TASK-PY-G-030` | `TASK-PY-G-033`, `TASK-PY-G-034` | four rolling windows, all Python cells >= 0.95 |
| 37 | `TASK-PY-G-033` Fail-closed release switch | #171 | release gate | `TASK-PY-G-026`, `TASK-PY-G-027`, `TASK-PY-G-032` | `TASK-PY-G-038`, `TASK-PY-G-034` | release switch rejects unknown or stale gate state |
| 38 | `TASK-PY-G-038` MCP RealityPrediction FSV | #180 | MCP | `TASK-PY-G-010`, `TASK-PY-G-011`, `TASK-PY-G-012`, `TASK-PY-G-013`, `TASK-PY-G-027`, `TASK-PY-G-029`, `TASK-PY-G-033`, `TASK-PY-G-037` | `TASK-PY-G-034` | MCP stdio `RealityPrediction` readback |
| 39 | `TASK-PY-G-034` Python acceptance bundle | #172 | acceptance | `TASK-PY-G-004`, `TASK-PY-G-028`, `TASK-PY-G-029`, `TASK-PY-G-031`, `TASK-PY-G-032`, `TASK-PY-G-033`, `TASK-PY-G-038`, `TASK-PY-G-040` | `TASK-PY-G-035` | acceptance bundle with issue/FSV/doc hashes |
| 40 | `TASK-PY-G-035` Post-green drift guardrails | #173 | post-green | `TASK-PY-G-031`, `TASK-PY-G-034` | Python ship gate may stay green under drift monitors | drift guardrail alerts and rollback hooks |

---

## Phase H — Inner-Loop Residue Cleanup (expanded cleanup)

| ID | Title | Phase | Blocked by | Effort |
|---|---|---|---|---|
| `TASK-LOOP-003` | Mark superseded SMAP decisions | H | LOOP-001 | **complete (#132)** — five retired inner-actor decision files have `status: superseded` + `superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md`; FSV at `/mnt/d/contextgraph/fsv/phase-h-smap-superseded-decisions-fsv/smap_superseded_decisions_fsv.json` |
| `TASK-LOOP-004` | Retire legacy MCP tools | H | none | **complete (#133)** — five retired cgreality compatibility names are excluded from tool definitions and fail closed with `CCREALITY_ENGINE_RETIRED`; FSV at `/mnt/d/contextgraph/fsv/phase-h-retired-mcp-tools-fsv/retired_mcp_tools_fsv.json` |
| `TASK-LOOP-005` | Audit absence of inner-actor model files | H | none | **complete (#134)** — `scripts/audit/no_inner_actor_models.sh`; FSV at `/mnt/d/contextgraph/fsv/phase-h-no-inner-actor-models-fsv/no_inner_actor_models_fsv.json` |
| `TASK-LOOP-006` | Delete stale `target/release/reality-loop` binary | H | none | **complete (#135)** — `scripts/ci/phase_h_reality_loop_binary_absence_fsv.sh`; FSV at `/mnt/d/contextgraph/fsv/phase-h-reality-loop-binary-absence-fsv/reality_loop_binary_absence_fsv.json` |
| `TASK-LOOP-008` | Verify `11_DELETION_LIST_INNER_LOOP` fully applied | H | LOOP-003..006, LOOP-009..012 | **complete (#136)** — entry-by-entry audit report at `memory/reports/phase-h-deletion-list-audit.md`, retired skill cleanup, stale `memory/context/current-phase.md` rewrite, and FSV at `/mnt/d/contextgraph/fsv/phase-h-deletion-list-audit-fsv/deletion_list_audit_fsv.json` |
| `TASK-LOOP-009` | Update CLAUDE.md inner-loop references | H | none | **complete (#137)** — remaining references are retirement/deny/fail-closed guidance; FSV at `/mnt/d/contextgraph/fsv/phase-h-claude-inner-loop-reference-fsv/claude_inner_loop_reference_fsv.json` |
| `TASK-LOOP-010` | Update agents/README.md retirements | H | none | **complete (#138)** — retired agent frontmatter + active roster FSV at `/mnt/d/contextgraph/fsv/phase-h-agent-roster-retirement-fsv/agent_roster_retirement_fsv.json` |
| `TASK-LOOP-011` | `CCREALITY_ENGINE_RETIRED` gate audit | H | LOOP-004 | **complete (#139)** — all five retired endpoints have unit fail-closed coverage, one JSON-RPC round-trip structured-error test, and fmt/check/clippy FSV at `/mnt/d/contextgraph/fsv/phase-h-ccreality-engine-retired-fsv/ccreality_engine_retired_gate_fsv.json` |
| `TASK-LOOP-012` | Remove orphaned scripts/config inner-LLM references | H | LOOP-005 | **complete (#140)** — `scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh` classifies scripts/config matches as retired-audit, generated snapshot, or explicit `historical, do not reanimate`; active disallowed count 0; four retired Shakespeare local-runtime helpers abort; FSV at `/mnt/d/contextgraph/fsv/phase-h-scripts-config-inner-llm-residue-fsv/scripts_config_inner_llm_residue_fsv.json` |

**Phase H status:** complete. GitHub issue readback on 2026-05-13 showed no open `phase:h` issues after #136 and #140 closed.

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
- Future Phase G model-quality experiments by cell/language/head, if the 0.95 convergence gate needs targeted retraining pressure.
- Cleanup tasks in Phase H — all independent

Phase F and Phase H no longer have parallelizable implementation work. A two-engineer pairing is now only relevant for Phase G convergence experiments and optional future corpus broadening.

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

# Atomic task graph (futurebuild)

> See `13_TASK_INDEX.md` for the full task index with effort estimates and DAG. This file is the directory listing for the atomic task files.

## Phase A — Operational Unblock

- [`flywheel/TASK-FLYWHEEL-001-mcp-daemon-mode.md`](flywheel/TASK-FLYWHEEL-001-mcp-daemon-mode.md) — MCP daemon mode + supervised tasks + D-drive validator
- [`flywheel/TASK-FLYWHEEL-002-self-optim-scheduler-skeleton.md`](flywheel/TASK-FLYWHEEL-002-self-optim-scheduler-skeleton.md) — scheduler skeleton with 5 tickers
- [`flywheel/TASK-FLYWHEEL-003-test-outcome-capture-hook.md`](flywheel/TASK-FLYWHEEL-003-test-outcome-capture-hook.md) — pytest/cargo-test/jest output parser
- [`flywheel/TASK-FLYWHEEL-004-async-oracle-binding.md`](flywheel/TASK-FLYWHEEL-004-async-oracle-binding.md) — bind oracle outcomes to predictions in observe()
- [`flywheel/TASK-FLYWHEEL-005-active-learning-agent-surprise-kind.md`](flywheel/TASK-FLYWHEEL-005-active-learning-agent-surprise-kind.md) — `ActiveLearningKind::AgentSurprise`
- [`flywheel/TASK-FLYWHEEL-006-new-cfs.md`](flywheel/TASK-FLYWHEEL-006-new-cfs.md) — register 3 new CFs
- [`predict/TASK-PREDICT-001-injection-hook.md`](predict/TASK-PREDICT-001-injection-hook.md) — UserPromptSubmit prediction-injection hook
- [`predict/TASK-PREDICT-002-record-agent-feedback-tool.md`](predict/TASK-PREDICT-002-record-agent-feedback-tool.md) — `mejepa_record_agent_feedback` MCP tool + sampling-weight bump
- [`loop/TASK-LOOP-001-apply-new-loop-md.md`](loop/TASK-LOOP-001-apply-new-loop-md.md) — apply new `.claude/loop.md` content
- [`loop/TASK-LOOP-002-update-claude-md-and-agents-readme.md`](loop/TASK-LOOP-002-update-claude-md-and-agents-readme.md) — CLAUDE.md + agents/README.md edits
- [`loop/TASK-LOOP-007-hook-output-paths.md`](loop/TASK-LOOP-007-hook-output-paths.md) — hook output paths to D-drive
- [`observability/TASK-OBS-005-c-drive-lint.md`](observability/TASK-OBS-005-c-drive-lint.md) — CI lint
- [`observability/TASK-OBS-006-fsv-path-migration.md`](observability/TASK-OBS-006-fsv-path-migration.md) — sweep `crates/*/examples/*_fsv.rs`
- [`observability/TASK-OBS-007-per-slice-quota.md`](observability/TASK-OBS-007-per-slice-quota.md) — slice breakdown in `mejepa_quota_status`

## Phase B — Multi-Head Extension

- `predict/TASK-PREDICT-003-predict-what-if-tool.md`
- `predict/TASK-PREDICT-004-explain-prediction-tool.md`
- `predict/TASK-PREDICT-005-multi-head-stack.md`
- `predict/TASK-PREDICT-006-rich-reality-prediction-schema.md`
- `predict/TASK-PREDICT-007-claim-graph-extractor.md`
- `predict/TASK-PREDICT-008-reality-impact-reconciler.md`
- `predict/TASK-PREDICT-LABEL-001..009-{...}.md` — per-head label sources (9 small tasks)
- `predict/TASK-PREDICT-009-active-learning-consumer.md`
- `flywheel/TASK-UTML-001-per-head-deltas.md`
- `flywheel/TASK-UTML-002-per-head-cert-cols.md`
- `flywheel/TASK-UTML-003-min-head-aggregation.md`
- `flywheel/TASK-UTML-004-surprise-weight-factor.md`
- `flywheel/TASK-UTML-005-per-head-fisher.md`
- `flywheel/TASK-UTML-006-per-head-dormancy.md`
- `flywheel/TASK-UTML-009-per-cell-T.md`

## Phase C — Corpus Completion

- `corpus/TASK-CORPUS-001-per-language-operators.md`
- `corpus/TASK-CORPUS-002-300x8-corpus-run.md`
- `corpus/TASK-CORPUS-003-per-language-fixture-corpora.md`
- `corpus/TASK-CORPUS-004-mincut-split-new-corpora.md`

## Phase D — Neural Embedder Forwards

- `enrichment/TASK-ENRICH-001-neural-forward-wrappers.md`
- `enrichment/TASK-ENRICH-002-dda-signal-computation.md`
- `enrichment/TASK-ENRICH-003-populate-new-cfs.md`
- `enrichment/TASK-ENRICH-004-embedder-cache.md`
- `enrichment/TASK-ENRICH-005-vram-budget-startup-gate.md`
- `enrichment/TASK-ENRICH-006-per-embedder-fsv.md`
- `enrichment/TASK-ENRICH-007-stdlib-registry.md`

## Phase E — Continuous Self-Optimization

- `heal/TASK-HEAL-001-self-optimization-scheduler.md`
- `heal/TASK-HEAL-002-per-cell-stratified-drift.md`
- `heal/TASK-HEAL-003-dissipation-detection.md`
- `heal/TASK-HEAL-004-bayesian-small-window.md`
- `heal/TASK-HEAL-005-constellation-refresh-scheduler.md`
- `heal/TASK-HEAL-006-promote-approval-tool.md`
- `heal/TASK-HEAL-007-emergency-eviction-tick.md`
- `flywheel/TASK-UTML-007-lambda-ramp.md`
- `flywheel/TASK-UTML-008-per-head-dissipation.md`

## Phase F — Production Observability

- `observability/TASK-OBS-001-weekly-eval-cron.md`
- `observability/TASK-OBS-002-mi-heatmap-renderer.md`
- `observability/TASK-OBS-003-daemon-status-aggregator.md`
- `observability/TASK-OBS-004-per-prediction-class-calibration.md`
- `observability/TASK-OBS-008-wsl-gpu-telemetry.md`
- `observability/TASK-OBS-009-inspect-prediction-tool.md`
- `observability/TASK-OBS-010-active-learning-cli.md`

## Phase H — Inner-Loop Residue Cleanup

- `loop/TASK-LOOP-003-supersede-smap-decisions.md`
- `loop/TASK-LOOP-004-retire-legacy-mcp-tools.md`
- `loop/TASK-LOOP-005-audit-no-inner-actor-models.md`
- `loop/TASK-LOOP-006-delete-stale-binary.md`

---

For Phase A tasks, atomic files exist in this commit; for later phases the atomic files are stubs (or will be generated as needed). The `13_TASK_INDEX.md` carries the full description of every task.

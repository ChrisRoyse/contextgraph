# 07 — Training Data Flywheel

> **Purpose.** Specify how every Edit / Write / test-run / agent-feedback event becomes a labeled training example for ME-JEPA, **continuously and without operator intervention**. The flywheel is the operator's "this system can derive reward signals from durable reality. that is what we are doing, trying to capture all the reward signals as possible so we can have as much data as possible for the me jepa to be able to predict reality."
>
> Owned by [`SPEC-FLYWHEEL.md`](specs/functional/SPEC-FLYWHEEL.md) and [`TECH-FLYWHEEL.md`](specs/technical/TECH-FLYWHEEL.md).

---

## 1. Five reward channels

Per the closed-loop architecture in [`03_ARCHITECTURE_BLUEPRINT.md §4`](03_ARCHITECTURE_BLUEPRINT.md), five channels feed back into the training stream. The flywheel is the orchestration that pulls all five into a single coherent training pipeline.

| Channel | Source | Frequency | Label kind |
|---|---|---|---|
| **C1: Oracle ground truth** | Docker test runs (Phase 0 corpus + live `pytest`/`cargo test`) | per test invocation | hard label: per-test pass/fail |
| **C2: Shift channel** | every Edit / Write — pre/after SHAs + cargo check + git diff | per file mutation (~250ms latency) | soft label: panel transition for SSL panel-loss |
| **C3: Agent-feedback channel** | `mejepa_record_agent_feedback` calls from the agent | event-driven (rare; high signal) | weak label: prediction-correctness with severity |
| **C4: Operator-override channel** | active-learning queue → operator labels | event-driven; gold-standard | hard label: operator-confirmed correct/incorrect |
| **C5: Static-analysis snapshot deltas** | clippy/pylint/eslint/bandit/cargo-audit before vs after | per shift | soft label: warning class deltas drive Q4 heads |

**No new channels are needed.** All five exist in the shipped substrate. The flywheel is the orchestration that ensures they all flow continuously.

## 1.1 Reward signal completeness contract

The flywheel's job is to capture teleological constellations: the durable relationship between an intended code change, the structures it touched, the instruments that observed it, the oracle outcomes it caused, and the ways ME-JEPA's prediction was right or wrong. Every captured event must be usable as ME-JEPA training data or as evidence that a needed signal is missing.

For each event, the implementation must name and verify:

| Stage | Required evidence |
|---|---|
| Trigger | edit/write/test/feedback/operator event id, timestamp, session id, and before/after SHA when code changed |
| Process | AST chunks, active embedders, DDA rows, TCT cells, panel id, prediction id, and witness hash |
| Outcome | oracle result, static-analysis delta, runtime/resource delta, agent feedback, active-learning entry, or explicit fail-closed diagnostic |
| Source of truth | RocksDB CF row, D-backed JSONL/file, witness-chain entry, or model artifact manifest read back independently after write |

The source-of-truth rule is non-negotiable: if the output should be durable, FSV must reopen or independently read the durable artifact and prove the expected row or file exists with the expected content. A successful return value is not enough.

---

## 2. The flywheel components

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Claude Code session activity                      │
└────────┬──────────┬──────────┬──────────┬──────────┬─────────────────┘
         │          │          │          │          │
         ▼          ▼          ▼          ▼          ▼
       Edit       Write      Bash       Read      User prompt
        │          │          │          │          │
        ▼          ▼          ▼          │          ▼
   ┌──────────────────────────┐         │     ┌──────────────────┐
   │ shift-log JSONL writer   │         │     │ UserPromptSubmit │
   └────────┬─────────────────┘         │     │ injection        │
            │                            │     └──────┬───────────┘
            ▼                            │            │
   ┌────────────────────────────┐        │            │
   │ Shift subscriber (tokio)   │◄───────┘            │
   │  poll_next_line @ 250ms    │                     │
   └────────┬───────────────────┘                     │
            │                                         │
            ▼                                         │
   ┌────────────────────────────┐                     │
   │ Pipeline:                  │                     │
   │  1. AST chunk              │                     │
   │  2. Embedder forward       │                     │
   │  3. DDA signals            │                     │
   │  4. PanelBuilder           │                     │
   │  5. Persist to CFs         │                     │
   └────────┬───────────────────┘                     │
            │                                         │
            ▼                                         │
   ┌────────────────────────────┐                     │
   │ Continuous self-optim      │                     │
   │ scheduler (tokio)          │                     │
   │  - observe(panel, oracle)  │                     │
   │  - EWC++ regularization    │                     │
   │  - drift detection         │                     │
   │  - active-learning queue   │                     │
   │  - A/B/C promotion         │                     │
   │  - continual backprop      │                     │
   └────────┬───────────────────┘                     │
            │                                         │
            ▼                                         │
   ┌────────────────────────────┐                     │
   │ ME-JEPA model state        │                     │
   │ (weights, Fisher snapshot, │                     │
   │  centroids, calibration)   │                     │
   └────────┬───────────────────┘                     │
            │                                         │
            ▼                                         │
   ┌────────────────────────────┐                     │
   │ verify(patch, context)     │─────────────────────┘
   │ → RealityPrediction        │
   │ → CF_MEJEPA_LIVE_PRED      │
   └────────────────────────────┘

Plus, on the operator side:
   ┌────────────────────────────┐
   │ Weekly evaluation report   │
   │ (mejepa_eval_run cron)     │
   │ + active-learning surface  │
   └────────────────────────────┘
```

### 2.1 Component status

| Component | Shipped? | Where |
|---|---|---|
| Hook layer (PreToolUse/PostToolUse/UserPromptSubmit) | YES | `.claude/settings.json` + `scripts/cc/*.sh` |
| Shift-log JSONL writer | YES | `crates/context-graph-mcp` |
| Shift subscriber | YES (crate + daemon task) | `crates/context-graph-mejepa-shift-subscriber`; daemon task wiring in `crates/context-graph-mcp/src/daemon.rs` |
| AST chunker | YES (11 langs) | `crates/context-graph-core::memory::ast` |
| Embedder forward (algorithmic) | YES | `AlgorithmicEmbedderForward` for E2/E3/E4/E9, with Phase D FSV readback |
| Embedder forward (pretrained neural) | YES | `EmbedderForward`/`PretrainedEmbedderForward` for E1/E6/E7/E8/E10/E12/E13/E14 with SHA-pinned artifact/routing FSV; production telemetry window FSV persists latency, ship-gate agreement, and VRAM readback |
| DDA signal computation | YES | `compute_dda_signals`, `persist_dda_signals`, readback, live shift rows, and `CF_MEJEPA_DDA_SIGNALS`; trained-head quality lift and weekly MI trend reporting move to Phase E/F/G |
| PanelBuilder | YES | `crates/context-graph-mejepa-instruments` |
| Embedder-backed instrument heads | YES for Phase A-D | `EmbedderBackedInstrumentHead` exists for code/text/trace semantic slots; Python code-slot analyzer feeds use Ruff semantic facts with Ruff/Pyright/py_compile FSV at `/mnt/d/contextgraph/fsv/contextgraph-python-semantic-analyzers-fsv/python-semantic-analyzers-evidence.json`; semantic-head FSV persists/readbacks E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test, E_Trace, E_Runtime, and E_Reasoning outputs |
| Continuous self-optim scheduler | YES (Phase A skeleton) | `crates/context-graph-mejepa::heal::{scheduler,scheduler_state}`; full autonomy remains Phase E |
| `MeJepaCompiler::observe` | YES | `crates/context-graph-mejepa::compiler` |
| EWC++ | YES | `crates/context-graph-mejepa::heal::ewc` |
| Drift detector | YES | `crates/context-graph-mejepa::heal::drift` |
| Active-learning queue | YES | `crates/context-graph-mejepa::eval::queue` |
| A/B/C promotion | YES | `crates/context-graph-mejepa::heal::promote` |
| Continual backprop | YES | `crates/context-graph-mejepa-train::learning_signal::continual_backprop` |
| `verify()` → `RealityPrediction` | YES (rich Phase B schema) | `crates/context-graph-mejepa::compiler`, `types.rs`, `prediction_surfaces.rs` |
| Multi-head extension to predictor | code_complete Phase B | typed surfaces and deterministic extraction ship; trained DDA/neural-backed quality remains |
| Prediction-injection hook | YES | `scripts/cc/hook_user_prompt_submit_inject_prediction.sh` + renderer; Phase B field UX hardening remains |
| `mejepa_record_agent_feedback` MCP tool | YES | `crates/context-graph-mcp`; persists to `CF_MEJEPA_AGENT_FEEDBACK` |
| MCP daemon mode (the supervised-process container) | YES (code + template + FSV) | `--daemon-server-only --d-root`; supervisor in `crates/context-graph-mcp/src/daemon.rs`; durable `CF_MEJEPA_FLYWHEEL_JOB_RUNS`; `scripts/systemd/mejepa-daemon.service`; fresh-host operator install/runbook remains |
| Weekly evaluation cron | partial (daemon scheduler FSV exists) | weekly daemon task runs holdout eval, writes reports/status, and records job-run rows; missing populated production holdout feed + operator dashboard/runbook |

**The unshipped pieces are production hardening and model-quality work, not new Phase A-D architecture:** full self-optimization policy, weekly production evaluation/dashboard cadence, per-head holdout gates, non-Python analyzer parity expansion, Phase G convergence to the 0.95 ship gate, and operator packaging. Phase A-D now provides the durable reward-signal substrate that Phase E consumes.

---

## 3. The MCP daemon mode (shipped supervisor, not fully installed)

Git history matters here. `e628fb5b` shipped supervisor shutdown, atomic state writes, launch-lock handling, `scripts/ensure_daemon.sh`, and `scripts/systemd/mejepa-daemon.service`. `23593272` then wired the self-optimization scheduler daemon task to the heal-crate scheduler instead of the old local ticker stub.

Today, `context-graph-mcp --daemon-server-only --d-root /mnt/d/contextgraph --daemon-port 3100 --http-port 3101` validates the D-root, opens the shared RocksDB, and starts the long-running shift subscriber, self-optimization scheduler, nightly GC scheduler, and weekly eval scheduler under the daemon supervisor. Task starts, scheduled run completions/errors, and task exits are sync-written to `CF_MEJEPA_FLYWHEEL_JOB_RUNS` and read back immediately; FSV evidence lives at `/mnt/d/contextgraph/fsv/mcp-daemon-supervision-fsv/daemon-supervision-evidence.json`.

What still needs work is fresh-host packaging and production data: first-time setup docs, `systemctl --user enable --now mejepa-daemon` verification on a fresh host, dashboard/runbook commands, populated production holdout feed, and the remaining Phase E/F policy/data-flow tasks that make every edit become a labeled training example.

---

## 4. The continuous self-optimization scheduler (the heart of the flywheel)

Per `03_ARCHITECTURE_BLUEPRINT.md §4.5`, the scheduler is a tokio task with seven tickers:

| Ticker | Period | Action |
|---|---|---|
| `observe_tick` | 250 ms | drain pending shifts → call `observe(panel, oracle_outcome_or_None)` with EWC++ regularization. If oracle is unknown yet (test hasn't run), enqueue for retroactive observe when oracle arrives. |
| `drift_check_tick` | 60 s | snapshot drift window → classify Soft/Hard/Catastrophic → queue appropriate intervention |
| `active_learning_tick` | 600 s | drain top-N high-priority entries from active-learning queue → fast-track observe on agent-flagged surprises |
| `promote_tick` | 3600 s | run pending LoRA refresh / calibration recalibrate / (if scheduled) full A/B/C retrain |
| `continual_backprop_tick` | 1800 s | detect dormant units → reinit if fraction > 15% |
| `constellation_freshness_tick` | 3600 s | inspect TCT constellation CF and fail closed on stale persisted constellation |
| `emergency_eviction_tick` | 600 s | inspect hygiene quota state and invoke emergency eviction only when threshold is exceeded |

**The scheduler is the single point at which "Claude Code did something" → "ME-JEPA learned from it" closes.** The Phase A skeleton now runs and inspects the real source-of-truth CFs. What is not complete yet is the Phase E policy loop that turns every edit plus later oracle outcome into an autonomous intervention/promotion decision.

Implementation: `crates/context-graph-mejepa::heal::scheduler::run_self_optimization_scheduler` (shipped Phase A skeleton). The skeleton delegates into shipped `heal`, `eval`, `hygiene`, and `tct` modules and persists a readback-verified scheduler status file. Phase E replaces the conservative inspection/action stubs with the full per-cell stratified, dissipation-aware autonomous policy loop.

---

## 5. The shift → panel → observe path (per-event)

```
Edit fires
    │
    ▼
PostToolUse hook appends shift-log JSONL line:
    { ts, session_id, file, before_sha, after_sha, line_count_before, line_count_after,
      cargo_check_result, git_diff_summary, hook_id }
    │
    ▼ (within 250ms)
ShiftSubscriber.poll_next_line() reads the line
    │
    ▼
materialize_shift(shift) -> PanelMaterializationOutcome:
    1. read file at after_sha (storage handle)
    2. memory::ast::chunk_with_options(content, language, opts) -> Vec<Chunk>
    3. for each chunk:
        a. routing::lookup(chunk.lang, chunk.kind) -> embedder set
        b. for embedder in set: EmbedderForward::forward(chunk) -> EmbeddingVec  [Phase D for full neural]
        c. compute DDA signals (per-embedder cosine to centroid + pairwise cosine + cached MI)  [Phase D production path]
        d. persist to CF_MEJEPA_DDA_SIGNALS keyed by (panel_id, chunk_id)
    4. PanelBuilder.build(chunks, scalar_features) -> Panel
    5. persist Panel to CF_MEJEPA_PANELS keyed by (session_id, panel_id)
    6. update CF_MEJEPA_SHIFT_WATERMARK with the consumed shift_id
    │
    ▼
shift_to_inference(panel, prior_panel, action) -> RealityPrediction:
    - predictor forward → predicted_panel + per-head outputs (Q1-Q5)
    - conformal predictor wraps → confidence interval
    - TCT Gτ guard → guard violations
    - assemble RealityPrediction (per 15_PREDICTION_TAXONOMY.md §1)
    - persist to CF_MEJEPA_LIVE_PREDICTIONS keyed by session_id
    │
    ▼ (asynchronously, when oracle outcome arrives)
observe(panel_t, action, panel_t+1, oracle_outcome) -> ObserveOutcome:
    - run EWC++ regularized SGD step
    - compute ΔP / ΔK / ΔΩ / ΔΞ → L_step
    - update sampling_weight for the involved chunks
    - write a training cert to CF_MEJEPA_TRAIN_CERTS (witness-chain bound)
    - if drift detector hits Soft/Hard, queue intervention
    - if agent recorded a SurpriseEvent on this prediction, bump weight by 1+2*severity_score
```

Latency budget per `REQ-SHIFT-03`: p50 < 5s, p99 < 30s. The per-shift work is bounded by:
- AST chunking: ~50ms for typical file
- Embedder forwards (neural): ~200ms per chunk × 5 chunks parallelized = ~200ms total
- DDA signals: ~50ms (cached MI; only fresh cosines)
- Panel build + persist: ~100ms
- Predictor forward: ~50ms (CUDA, batch=1)
- Conformal + Gτ: ~10ms
- Total p50: ~500ms; p99: dominated by embedder forward tail latency

---

## 6. The replay buffer + prioritized sampling

Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §6`:

```
sampling_weight(ex) = base_weight
                    · L_step(ex)
                    · (1 + 5 · operator_override_flag)
                    · decay^age_in_days
```

with `base_weight = 1.0`, `decay = 0.995` (half-life ~140 days). Examples with `sampling_weight < 0.05` drop from the current epoch; examples with `sampling_weight > 1.0` are guaranteed inclusion.

**The replay buffer is the union of CF_MEJEPA_PANELS panels + their training certs + their agent-feedback events.** The BatchSampler reads from this CF, applies sampling weights, and produces batches for the periodic full-retrain (every 1k attempts in Phase 5; reduced to every 10k attempts in steady state per the heal scheduler).

The flywheel makes the replay buffer **self-curating**:
- High-L_step examples (where the prediction was wrong) get over-sampled.
- Operator-override examples get 6× weight.
- Agent-flagged surprise examples get 1+2*severity weight bump.
- Stale low-L_step examples decay out via the 0.995 multiplicative decay.

Result: the trained model continuously over-fits to the hard-and-recent examples, which is exactly what we want for a system that gets better the more it's used.

---

## 7. The oracle-outcome arrival path (asynchronous label binding)

Predictions are made before tests run. Oracle outcomes arrive later (sometimes seconds, sometimes hours after CI completes). The flywheel must bind them retroactively:

```
Test run completes (pytest / cargo test / jest / ...)
    │
    ▼
Test-output capture hook (PostToolUse on Bash) parses the test result and
emits a TestOutcomeEvent:
    { ts, session_id, test_id, outcome: Pass|Fail|Error|Skip, duration_ms, error_log }
    │
    ▼
TestOutcomeEvent is persisted to CF_MEJEPA_ORACLE_VERDICTS keyed by
    (session_id, test_id, ts)
    │
    ▼
Self-optim scheduler's observe_tick wakes (next 250ms tick), notices the
new oracle entry, looks up:
    - the most-recent prediction in CF_MEJEPA_LIVE_PREDICTIONS for this session
      that includes this test in its predicted_failed_tests OR predicted_passed_tests
    - the corresponding panel in CF_MEJEPA_PANELS
    │
    ▼
observe(panel, action, panel_after_test, oracle=outcome) is called
    - the prediction's correctness is computed
    - if prediction was wrong AND severity is High/Critical, an active-learning
      queue entry is created (kind=PredictionMissed)
    - the panel + outcome → training cert chain entry
```

This is **the binding from "what the prediction said" to "what reality returned."** The oracle is the labeler. Predictions that match the oracle reinforce the model; predictions that don't get over-sampled in the next epoch.

---

## 8. The agent-feedback path (closing the metacognitive loop)

```
Agent calls mejepa_record_agent_feedback(prediction_id, kind, explanation, severity, actual_outcome)
    │
    ▼
SurpriseEvent persisted to CF_MEJEPA_AGENT_FEEDBACK keyed by
    (prediction_id, agent_id, ts)
    │
    ▼
Active-learning queue entry created (kind=AgentSurprise) with high priority
    │
    ▼
On the next active_learning_tick (≤ 600s later), the scheduler:
    - reads the SurpriseEvent
    - resolves the original prediction + panel
    - calls observe() with bumped sampling weight (1 + 2 * severity_score)
    - updates the calibration-error tracking for the head that made the wrong prediction
    │
    ▼
If the SurpriseEvent has severity = Catastrophic AND the calibration error
exceeds the drift threshold, fast-track to lora_refresh OR full_retrain queue
    │
    ▼
On the operator-visible side: the SurpriseEvent appears in the weekly evaluation
report under "agent-flagged misses for the week" with a per-prediction-class breakdown
```

**This is the loop the operator's "agents need a way to feed back into the me jepa model confirmation of predictions and give information whenever surprise happens" describes.** It's a single MCP tool + a CF + the scheduler entry — no new architecture.

---

## 9. Storage hygiene (so the flywheel doesn't eat the disk)

> **D-drive policy (locked).** Per `memory/decisions/2026-05-10--d-backed-durable-data-root-and-mcp-daemon.md`, ALL bulk durable state lives under `/mnt/d/contextgraph` (the D:\ drive in the WSL host), NOT under `$HOME` or the C:\ root filesystem. The flywheel adds CFs that obey the same rule:
>
> - RocksDB store (all CFs including the new `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`): `/mnt/d/contextgraph/storage/contextgraph-rocksdb/`
> - Shift-log JSONL (the densest growing artifact): `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl`
> - Model artifacts: `/mnt/d/cabdru/contextgraph/models/`
> - Daemon log + pid files: `/mnt/d/contextgraph/logs/`, `/mnt/d/contextgraph/state/locks/`
> - FSV evidence (formerly under `/tmp/`): `/mnt/d/contextgraph/fsv/<crate>-fsv/`
> - Replay-buffer materializations: `/mnt/d/contextgraph/storage/replay/`
> - Active-learning queue exports: `/mnt/d/contextgraph/exports/active-learning/`
>
> Daemon mode (`TASK-FLYWHEEL-001`) MUST refuse to start if the D-drive root is unavailable; this is not a fallback path. `scripts/ensure_daemon.sh` and daemon D-root validation enforce this at MCP startup.

Per `06 HYGIENE` (shipped):
- Tier compression: hot 8-bit / warm 7-bit / cool 5-bit / cold 3-bit (per access frequency)
- Quota enforcement: 115 GB hard cap; emergency eviction on overflow
- Witness Merkle compression for chains > 90 days old
- Nightly GC at 03:00 local: demote + compact + evict + prune + verify; every scheduled run writes `CF_MEJEPA_FLYWHEEL_JOB_RUNS` plus `state/schedulers/nightly_gc_status.json`
- Weekly eval: read `state/gold-labels/weekly_holdout_panels.json`, run holdout eval without treating expected OOD abstention as a job crash, persist `CF_MEJEPA_EVAL_REPORTS`, and write `state/schedulers/weekly_eval_status.json` plus `exports/eval/<date>/weekly.{json,md}`

The flywheel adds three new CFs (`CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`) — each participates in the existing tier policy:

| CF | Hot policy | Warm policy | Cool policy | Cold policy |
|---|---|---|---|---|
| `CF_MEJEPA_DDA_SIGNALS` | 8-bit groupwise quantization | 7-bit | 5-bit | 3-bit; ttl 365 days |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | 8-bit | 8-bit (operator-pinned) | 5-bit | 3-bit; ttl never (operator labels are permanent) |
| `CF_MEJEPA_AGENT_FEEDBACK` | 8-bit | 8-bit | 5-bit; bound to original prediction; never aged out without operator override |

Tier transition decisions are made by `hygiene::tier::tier_for_score` based on `access_count_30d` per the existing tier policy.

---

## 10. Failure modes the flywheel must survive

| Failure | Detection | Response |
|---|---|---|
| Shift subscriber lag > 500 unprocessed shifts / 1 min | `mejepa_subscriber_status` MCP tool | log `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT`; daemon escalates to operator if lag > 5 min |
| Self-optim scheduler tokio task panics | `tokio::JoinSet` supervisor catches | log + restart with exponential backoff (cap 60s); after 5 restarts in 5 min, surface `MEJEPA_SCHEDULER_FLAPPING` |
| Drift detector reports Catastrophic | `heal::drift::classify` | trigger A/B/C retrain; alert operator via `CF_MEJEPA_HEAL_REPORTS` (severity=Catastrophic); pause prediction emission until A/B/C completes |
| Storage overflow | `hygiene::quota::quota_status` | emergency eviction kicks in; if still overflowing, alert operator + halt panel writes (predictions still served from existing weights) |
| Embedder model file corrupt / missing | startup digest check (existing) | refuse to start the daemon; surface `MEJEPA_EMBEDDER_DIGEST_MISMATCH` |
| Oracle outcome stops arriving (test runner misconfigured) | rolling-window oracle-arrival rate < threshold | fail the affected hard-label observe path with diagnostic context; do not synthesize prediction-only labels |
| RocksDB write contention | latency telemetry on observe() | fail the affected write with CF/key/latency diagnostics; alert if persistent |

---

## 11. What this flywheel commits us to

1. **MCP daemon mode** with supervised tokio tasks + systemd template. (`TASK-FLYWHEEL-001`, code shipped; operator install proof remains)
2. **Continuous self-optimization scheduler** as the orchestrator of observe/drift/active-learning/promote/continual-backprop. (`TASK-FLYWHEEL-002`, skeleton shipped; `TASK-HEAL-001` is the full autonomous policy)
3. **Test-outcome capture hook** that parses pytest/cargo-test/jest output and writes `TestOutcomeEvent`s. (`TASK-FLYWHEEL-003`)
4. **Asynchronous oracle-binding** path inside observe(). (`TASK-FLYWHEEL-004`)
5. **`mejepa_record_agent_feedback` MCP tool** + `CF_MEJEPA_AGENT_FEEDBACK` + sampling-weight bump wiring. (`TASK-PREDICT-002`, complete)
6. **Active-learning queue surfacing** of agent-flagged surprises. (`TASK-FLYWHEEL-005`, complete)
7. **Weekly evaluation report cron** that runs `mejepa_eval_run` and writes the per-(category × language) report. (`TASK-OBS-001`)
8. **CF additions** for the new flywheel CFs in the shared `context-graph-mejepa-cf` registry. (`TASK-FLYWHEEL-006`)

All listed in [`13_TASK_INDEX.md`](13_TASK_INDEX.md).

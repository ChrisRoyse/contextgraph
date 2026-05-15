# 03 — Architecture

## System diagram (in words)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  UNTRUSTED EDIT GENERATOR (Claude Code / Cursor / Aider / Devin / ...)      │
│  Emits: file_edits[], natural-language claim, optional test output          │
└──────────────────────┬──────────────────────────────────────────────────────┘
                       │ JSON-RPC stdio (MCP protocol)
                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  ME-JEPA-Code MCP SERVER  (context-graph-mcp crate, stdio transport)        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  CAPTURE SURFACE                                                       │  │
│  │   - PreToolUse / PostToolUse hooks → shift_log JSONL                  │  │
│  │   - Edit / Write events → SHA chain + AST diff                        │  │
│  │   - Bash test outputs → test_outcomes JSONL                           │  │
│  │   - UserPromptSubmit → prompt audit                                   │  │
│  │   - Stop hook → session boundary capture                              │  │
│  └─────────────────────────┬─────────────────────────────────────────────┘  │
│                            ▼                                                 │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  PREDICTOR PATH (trained, runs on every prediction request)           │  │
│  │   12 embedders → joint embedding → predictor head → RealityPrediction │  │
│  │   + conformal interval + TCT Gτ guard → Verdict + per-head outputs    │  │
│  └─────────────────────────┬─────────────────────────────────────────────┘  │
│                            ▼                                                 │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  TARGET PATH (frozen, NO trainable parameters — collapse-immune)      │  │
│  │   15 instruments (E_Oracle, E_AST, E_DataFlow, E_TypeGraph, ...)      │  │
│  │   → Panel (5,120-d) → ground-truth signal for training                │  │
│  └─────────────────────────┬─────────────────────────────────────────────┘  │
│                            ▼                                                 │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  STORAGE (RocksDB column families, 51+ CFs)                           │  │
│  │   CF_MEJEPA_PANELS, CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_TCT_CELLS,       │  │
│  │   CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_REPLAY_BUFFER, ...            │  │
│  └─────────────────────────┬─────────────────────────────────────────────┘  │
│                            ▼                                                 │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  CONTINUOUS SELF-OPTIMIZATION DAEMON                                   │  │
│  │   - Shift subscriber tails edit events                                │  │
│  │   - Prioritized replay (6× weight on Refuted feedback)                │  │
│  │   - EWC catastrophic-forgetting protection                            │  │
│  │   - Continual backprop plasticity (CBP)                               │  │
│  │   - Drift detection → emergency eviction                              │  │
│  │   - Periodic conformal recalibration                                  │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## The five subsystems

ME-JEPA-Code is one product made of five subsystems. Each is independently replaceable. They are layered.

### Subsystem 1 — Capture surface

**Job:** turn live agent activity into typed, append-only reality events.

**Components:**
- **PreToolUse / PostToolUse hooks** (`scripts/cc/hook_pre_tool_use_edit.sh`, `scripts/cc/hook_post_tool_use_edit.sh`). Fire on every Edit/Write tool the agent uses. Capture pre-edit and post-edit SHA of every touched file. Append to `runtime/cgreality-shift-log/*.jsonl`.
- **Bash test capture** (`scripts/cc/hook_post_tool_use_test_capture.sh` + `parse_test_output.py`). When the agent runs a Bash command, parse the output for pytest / cargo test / jest / unittest result markers. Append structured `TestOutcomeEvent` rows to `runtime/test-outcomes/<session>.jsonl`.
- **UserPromptSubmit injection** (`scripts/cc/hook_user_prompt_submit.sh` + `hook_user_prompt_submit_inject_prediction.sh`). When the agent receives a new prompt, the latest live `RealityPrediction` for that session is rendered into Markdown (per `15_PREDICTION_TAXONOMY.md §8`) and injected into the prompt's `additionalContext`. The agent gets ME-JEPA's verdict before acting.
- **Session boundary capture** (Stop hook). On session end, snapshot the active CFs, drop file watchers, emit `SessionBoundary` event.
- **MCP tool calls** themselves. Every `mejepa_*` tool invocation logs the `tool_use_id`, the request payload, the response, the latency.

All capture is **append-only**, **fail-closed**, and routes to **durable storage on operator hardware** (no HTTP egress). The capture surface is the *reality* in "reality-shift principle."

### Subsystem 2 — Target path (frozen)

**Job:** produce the deterministic ground-truth representation `f_target(panel)` that the predictor learns to match.

This is the **collapse-immunity guarantee**. The target representation has **zero trainable parameters**. It is composed of 15 frozen instruments, each a deterministic function of the typed events the capture surface emits. Examples:

- `E_Oracle` (128-d) — Docker SWE-bench oracle outcomes per task instance: pass_fraction, fail_fraction, has_exception flag, 10-class one-hot exception type.
- `E_AST` — AST chunk metadata: node_kind, depth, parent chain, byte span.
- `E_DataFlow` — static data-flow graph features per edited symbol.
- `E_TypeGraph` — Python type information at the call sites the edit touched.
- `E_Imports` — module import graph deltas.
- `E_TestIntent` — test files validating the touched source paths.
- `E_DDA` — DDA (Data-Driven Augmentation) signals per chunk.
- `E_Witness` — Merkle root of the chunk's content hash chain.

(Full list, slot definitions, and dimensionality breakdown in [`05_FROZEN_INSTRUMENT_PANEL.md`](05_FROZEN_INSTRUMENT_PANEL.md).)

The total target panel is **5,120 dimensions**. Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`, the collapse term `ΔΞ.target_collapse = 0` by construction because the instruments are not parameters — they are functions defined outside the training loop. This is what makes ME-JEPA a *truth* system instead of another EMA-with-stop-gradient ablation.

Implementation: `context-graph-mejepa-instruments` crate (`Instrument` trait, `InstrumentSlot` enum, `Panel` + `PanelBuilder`, fail-closed validation, JSON via `try_from` invariant gate). E_Oracle shipped first (`EOracleInstrument`, 27 tests, FSV at `/mnt/d/contextgraph/fsv/contextgraph-mejepa-instruments-fsv/`).

### Subsystem 3 — Predictor path (trained)

**Job:** produce the learned representation `f_predictor(panel_t1, claim, context)` that maps the pre-edit code + agent's claim into the same 5,120-d space the target panel lives in.

**Layer 1 — Embedders (12 active content slots):**

| Slot | Model | Dim | Type | Captures |
|---|---|---|---|---|
| E1 | intfloat/e5-large-v2 | 1024 | dense | semantic |
| E2 | algorithmic temporal_recent | 512 | sinusoidal PE | recency |
| E3 | algorithmic temporal_periodic | 512 | recurrence | cyclic patterns |
| E4 | algorithmic temporal_positional | 512 | session positional | session relative |
| E6 | naver/splade-cocondenser-ensembledistil | 1536 | sparse | keyword |
| E7 | Qodo/Qodo-Embed-1-1.5B | 1536 | dense | code-specific |
| E8 | intfloat/e5-large-v2 (graph mode) | 1024 | dense | graph-context |
| E9 | algorithmic HDC | 1024 | hyperdimensional | binary patterns |
| E10 | intfloat/e5-base-v2 multimodal | 768 | dense | contextual |
| E12 | colbert-ir/colbertv2.0 | 128/token | late-interaction | token-level match |
| E13 | prithivida/Splade_PP_en_v1 | 1536 | sparse | SPLADE keyword |
| E14 | BAAI/bge-m3 | 1024 | dense | multilingual code |

Per `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`, every chunk is the AST node + its parent chain + its byte span, not a fixed-window text slice. The embedders see structural units.

**Layer 2 — Joint embedding head:**

The per-chunk per-embedder vectors are projected and concatenated into a per-chunk joint embedding. Cross-attention between embedders is the source of the "embedder disagreement" reward signal (see [`07_TELEOLOGICAL_CONSTELLATIONS.md`](07_TELEOLOGICAL_CONSTELLATIONS.md)).

**Layer 3 — Predictor head stack:**

A multi-head MLP that emits the `RealityPrediction` structure: one head per question (Q1-Q5) with sub-heads inside Q4 for edge cases / latent bugs / tech debt / dead code / perf / security / accuracy / cost. Each head is independently trained against its instrument target.

**Layer 4 — Conformal calibration:**

Per `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md`, the predictor emits a *score* per head; conformal calibration converts the score to a *calibrated probability* + a *prediction interval*. When the calibration set's residual is small, the verdict is confident. When it's large, the system emits `Verdict::Abstain`.

**Layer 5 — TCT Gτ guard:**

The teleological constellation Gτ (gamma-tau) guard checks each prediction against the cell membership of the (chunk × mutation_category) it belongs to. If the prediction conflicts with the cell's centroid by more than a per-cell calibrated threshold, the verdict becomes `Verdict::GuardRejected`. This is the "did the prediction line up with this cell's historical reality?" check.

### Subsystem 4 — Storage

**Job:** durable, append-only, FSV-able persistence of every event, every prediction, every panel, every replay-buffer entry.

**Implementation:** RocksDB with **51+ column families** (`context-graph-mejepa-cf` crate). Layout decisions: every prediction goes to `CF_MEJEPA_LIVE_PREDICTIONS` with the `prediction_id` as the key. Every panel materialization goes to `CF_MEJEPA_PANELS`. Every DDA signal row goes to `CF_MEJEPA_DDA_SIGNALS`. Every TCT cell membership goes to `CF_MEJEPA_TCT_CELLS`. Every operator override goes to `CF_MEJEPA_OPERATOR_OVERRIDES`. Every prediction-vs-actual reconciliation goes to `CF_MEJEPA_PREDICTION_VERIFICATIONS`.

Key choices:
- **Sync writes** for anything that gates a verdict. RocksDB `sync_all=true` on each `Put`.
- **Independent re-open readback** for FSV evidence: any time a test asserts "this row exists in CF X with value Y", the test reopens the DB and reads it back through a fresh handle.
- **No backward compatibility.** Schema changes wipe the DB. Pre-production discipline.

### Subsystem 5 — Continuous self-optimization daemon

**Job:** keep the predictor measurably better the longer it runs.

**Components** (all in `context-graph-mejepa-train` + `context-graph-mejepa-shift-subscriber`):

- **Shift subscriber** — Tokio task that tails the shift_log JSONL. For each new edit event, computes the post-edit panel via the frozen instruments and emits a `PanelMaterialized` event.
- **Prioritized replay buffer** — bounded queue of (input, target, weight) tuples. `Refuted` operator-feedback entries get 6× weight. Out-of-distribution prediction mistakes get pulled into the buffer at high priority.
- **EWC (Elastic Weight Consolidation)** — per-head Fisher information matrix tracked online. Prevents catastrophic forgetting of older cells when new cells dominate the recent training stream.
- **Continual Backprop (CBP)** — per-unit utility tracking; low-utility units get reinitialized to maintain plasticity over months of online training.
- **Drift detector** — Bayesian small-window drift on the per-cell oracle distribution. When drift fires, emergency eviction marks the cell for retraining.
- **Constellation refresh policy** — TCT cells get re-centroided on a schedule + on-demand when their reward-density drops below a per-cell threshold.
- **Conformal recalibration** — after each promotion, the conformal residual distribution is re-fit on the latest holdout.

Status:
- TASK-HEAL-001..007: full implementation pending (Phase E entry).
- TASK-UTML-005..008: per-head Fisher + dormancy + lambda ramp + dissipation pending.
- Shift subscriber: shipped.
- Replay buffer: in design.

## What lives in which Rust crate

| Crate | Job |
|---|---|
| `context-graph-mcp` | MCP server, JSON-RPC stdio, 28 live `mejepa_*` tools |
| `context-graph-cli` | Harness CLI, ME-JEPA implementation work, readback helpers |
| `context-graph-mejepa` | Predictor, compiler, heal scheduler, evaluation |
| `context-graph-mejepa-cf` | RocksDB column-family registry (51+ CFs) |
| `context-graph-mejepa-corpus` | 8 mutation operators × 11 languages, corpus CLI, SWE-bench Docker bridge |
| `context-graph-mejepa-embedders` | 21 embedder slots; `EmbedderForward` trait; 12 active content slots |
| `context-graph-mejepa-hygiene` | Tier compression, quota, eviction, GC, Merkle witness compression |
| `context-graph-mejepa-instruments` | 15 frozen instruments + Panel + PanelBuilder |
| `context-graph-mejepa-shift-subscriber` | Tokio task tailing the shift log; materializes panels |
| `context-graph-mejepa-tct` | Teleological constellations + Gτ guard |
| `context-graph-mejepa-train` | Trainer + UTML formula carriers + DDA + samplers |
| `context-graph-core`, `context-graph-mincut`, `context-graph-solver`, `context-graph-tier-compression`, `context-graph-witness`, `context-graph-benchmark` | Supporting primitives |

## MCP tool surface (28 tools, summarized)

The full surface is in `crates/context-graph-mcp/src/handlers/`. Grouped by purpose:

**Prediction:**
- `mejepa_predict_latest` — return the latest live `RealityPrediction` for a session
- `mejepa_predict_what_if` — counterfactual: "if the agent applied this diff, what would the prediction be?"
- `mejepa_explain_prediction` — return the violating embedders, nearest exemplars, instrument deltas behind a prediction

**Feedback:**
- `mejepa_record_agent_feedback` — operator-supplied correction; pulled into replay buffer at 6× weight
- `mejepa_observe_shift` — explicit shift recording outside the hooks

**Constellations:**
- `mejepa_constellation_inspect` — view a TCT cell's members, centroid, threshold, last-update
- `mejepa_routing_lookup` — given a query, return the cell it routes to

**Self-optimization / heal:**
- `mejepa_heal_status` — return the daemon's drift/eviction/replay state
- `mejepa_capture_audit` — manual capture of an event the hooks didn't catch
- `mejepa_gc_run` — garbage-collect stale CFs
- `mejepa_quota_status` — disk / VRAM budget status
- `mejepa_bootstrap_status` — cold-start readiness

**Evaluation:**
- `mejepa_eval_build_graph` — assemble the ship-gate evaluation graph
- `mejepa_eval_run` — run one evaluation pass against the holdout corpus
- `mejepa_ship_gate_status` — return the current `prediction_oracle_correlation` per cell

**Active learning:**
- `mejepa_active_learning_queue` — return the cells whose reward density is below threshold and should be hand-labeled

**Audit:**
- `mejepa_subscriber_status` — shift-subscriber lag
- `mejepa_rollback_to` — rollback to a tagged commit (operator-only)
- `mejepa_witness_compress` — compress the Merkle witness chain
- `mejepa_audit_pairwise_mi` — pairwise mutual information audit across embedders
- `mejepa_vram_budget_report` — current VRAM allocation per embedder

**Health:**
- `mejepa_verify` — full FSV-style health check
- `mejepa_load_learner_state` — load a frozen learner snapshot
- `mejepa_promote_approval` — operator approval gate for promoting a new trained predictor

## Data flow for one prediction (sequence diagram in words)

1. Agent runs `Edit` on file `foo.py`. PreToolUse hook fires → records pre-edit SHA. Agent applies the edit. PostToolUse hook fires → records post-edit SHA + AST diff. Both events append to `runtime/cgreality-shift-log/<session>.jsonl`.

2. Shift subscriber sees the new event → computes the post-edit panel via the 15 frozen instruments → writes to `CF_MEJEPA_PANELS` keyed by `(session_id, file_path, post_edit_sha)`.

3. Agent calls `mejepa_predict_latest(session_id)` over MCP.

4. The predict path:
   - Reads the latest `CF_MEJEPA_PANELS` row for the session.
   - Runs the 12 embedders forward on the post-edit chunks.
   - Concatenates into joint embedding.
   - Forward through predictor head → raw scores per Q1–Q5.
   - Apply conformal calibration → calibrated probability + interval.
   - Apply TCT Gτ guard → check cell membership.
   - Emit `RealityPrediction` with `verdict`, `prediction_id`, `predicted_failure_modes`, `predicted_edge_cases`, etc.

5. The prediction is persisted to `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `prediction_id`.

6. The MCP response returns the prediction to the agent.

7. UserPromptSubmit hook fires on the next user turn → renders the prediction to Markdown → injects into agent's next prompt's `additionalContext`.

8. Agent acts on the prediction (e.g., reverts a change that ME-JEPA marked `Verdict::Fail`).

9. Operator runs the tests → outcome captured → `CF_MEJEPA_PREDICTION_VERIFICATIONS` row written → if prediction was wrong, `mejepa_record_agent_feedback` fires → replay buffer at 6× weight.

10. Next daemon retrain cycle ingests the verification → predictor weights update → conformal recalibrated.

This is the full reality-shift cycle. Every step is sourced from durable bytes-on-disk, not from any LLM's self-report.

## What's running right now

The **forward-cache run on aiwonder** is producing the per-chunk per-embedder vectors that the predictor head will be trained against. Cache root:

```
/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/
```

12 embedders × 399,777 chunks = ~4.9 million vector records. Mean throughput 206 s/shard at par=6, ~21 hour ETA. When this completes, the seed substrate for the JEPA predictor exists, and the training loop ([`09_TRAINING_LOOP.md`](09_TRAINING_LOOP.md)) can run.

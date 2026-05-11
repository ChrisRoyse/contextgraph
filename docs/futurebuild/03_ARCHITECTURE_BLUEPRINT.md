# 03 — Architecture Blueprint

> **Purpose.** The reference architecture for ME-JEPA-as-truth-system, end-to-end, with every data path, every storage CF, every MCP tool, and every closing of the feedback loop named. This blueprint is what the rest of the futurebuild docs implement against. Everything here is *additive* to the shipped Phase 0–8 substrate; it adds wiring, daemons, and the agent-feedback closure that the operator's prompt requires.

---

## 1. Single end-to-end picture

```
                                 ┌──────────────────────────────────────────────────────────┐
                                 │           Claude Code session (the agent)                │
                                 │   - reads issue, edits files, runs tests                 │
                                 │   - calls mejepa_verify / mejepa_predict_latest          │
                                 │   - calls mejepa_record_agent_feedback on surprise       │
                                 └──────────────┬───────────────────────────────────────────┘
                                                │
                                                ▼   (PreToolUse / PostToolUse / UserPromptSubmit hooks)
                  ┌──────────────────────────────────────────────────────────────────────┐
                  │   .claude hook layer  →  shift-log JSONL   →   shift subscriber      │
                  │   pre/after SHA + cargo check + git diff appended every Edit/Write   │
                  └─────────────┬───────────────────────────────────┬────────────────────┘
                                │                                   │
                                ▼                                   ▼
   ┌──────────────────────────────────────────┐    ┌──────────────────────────────────────┐
   │   AST chunker (11 languages)             │    │   Prediction injector (UserPromptSubmit) │
   │   memory::ast::chunk_with_options(...)   │    │   reads CF_MEJEPA_LIVE_PREDICTIONS   │
   │   → Vec<Chunk { chunk_id, lang, kind }>  │    │   → markdown rendered into agent ctx │
   └─────────────┬────────────────────────────┘    └──────────┬───────────────────────────┘
                 │                                             ▲
                 ▼                                             │
   ┌──────────────────────────────────────────┐    ┌───────────┴──────────────────────────┐
   │   Embedder routing table                 │    │   ME-JEPA Compiler                   │
   │   per (Language, EntityType) → set of    │    │   compile() / verify() / observe()   │
   │   active embedders out of E1, E7, E8,    │    │   → RealityPrediction (multi-head)   │
   │   E10, E12, E13, E14 + scalar feats      │    │   → CF_MEJEPA_LIVE_PREDICTIONS       │
   └─────────────┬────────────────────────────┘    └───────────▲──────────────────────────┘
                 │                                             │
                 ▼                                             │
   ┌──────────────────────────────────────────┐                │
   │   Per-chunk DDA enrichment               │                │
   │   N embedders → N + N² signals per chunk │                │
   │   per-(category × language × embedder)   │                │
   │   centroids → TCT Gτ guard on each       │                │
   └─────────────┬────────────────────────────┘                │
                 │                                             │
                 ▼                                             │
   ┌──────────────────────────────────────────┐                │
   │   PanelBuilder → Panel (5,120-d)         │                │
   │   15 instrument slots                    │                │
   │   E_AST E_CFG E_DataFlow E_TypeGraph     │                │
   │   E_Test E_Trace E_Diff E_Witness        │                │
   │   E_Oracle E_Problem E_CommitMsg         │                │
   │   E_StaticAnalysis E_Runtime E_Reasoning │                │
   │   + Scalars                              │                │
   └─────────────┬────────────────────────────┘                │
                 │                                             │
                 ▼                                             │
   ┌──────────────────────────────────────────────────────────┘
   │   Multi-head predictor (6-layer transformer trunk + heads)
   │   - panel head: predicts panel[t=2] in latent space (VICReg)
   │   - oracle head: per-test pass/fail logits + count
   │   - failure-mode head: ranked FailureMode classes per chunk
   │   - edge-case head: predicted untested edge cases per chunk
   │   - tech-debt head: smell/dead-code/redundancy logits
   │   - perf head: predicted runtime/memory regression deltas
   │   - security head: SAST-style pattern logits
   │   - cost head: predicted token/$/CI-min impact
   │   See §3 below for the head taxonomy
   └────────────────────────────────────────────────────────────┐
                                                                │
                                                                ▼
   ┌──────────────────────────────────────────────────────────────────────────┐
   │   Storage layer (RocksDB, 28+ ME-JEPA CFs, 51+ workspace CFs)            │
   │   CF_MEJEPA_PANELS · CF_MEJEPA_PANEL_META · CF_MEJEPA_WEIGHT_BLOBS       │
   │   CF_MEJEPA_TRAIN_CERTS · CF_MEJEPA_ORACLE_VERDICTS                      │
   │   CF_MEJEPA_MUTATION_CORPUS · CF_MEJEPA_LIVE_PREDICTIONS                 │
   │   CF_MEJEPA_SHIFT_WATERMARK · CF_MEJEPA_AGENT_FEEDBACK *new*             │
   │   CF_MEJEPA_FAILURE_EXEMPLARS *new* · CF_MEJEPA_DDA_SIGNALS *new*        │
   │   CF_MEJEPA_CONSTELLATION · CF_MEJEPA_FISHER_SNAPSHOTS                   │
   │   CF_MEJEPA_DRIFT_WINDOW · CF_MEJEPA_DRIFT_HISTORY                       │
   │   CF_MEJEPA_HEAL_REPORTS · CF_MEJEPA_DISTILL_STEPS                       │
   │   CF_MEJEPA_ACTIVE_POINTERS · CF_MEJEPA_EVAL_REPORTS                     │
   │   CF_MEJEPA_ACTIVE_LEARNING_QUEUE · CF_MEJEPA_PATCH_GRAPH                │
   │   CF_AUDIT_LOG · CF_TOOL_CALL_INDEX · CF_MERGE_HISTORY · CF_IMPORTANCE   │
   └──────────────────────────────────────────────────────────────────────────┘
```

The two new things relative to the shipped substrate are highlighted as *new* in the storage block: **CF_MEJEPA_AGENT_FEEDBACK** (the closed-loop channel for agent-confirmed/-corrected predictions) and **CF_MEJEPA_DDA_SIGNALS** + **CF_MEJEPA_FAILURE_EXEMPLARS** (the multi-head prediction substrate). The rest of the diagram is wiring.

---

## 2. Components inventory (shipped vs new)

| Component | Status | Crate / Module | Purpose |
|---|---|---|---|
| Hook layer (`PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`) | shipped | `.claude/settings.json` + `scripts/cc/*.sh` | bind reality changes to deterministic capture |
| Shift-log JSONL writer | shipped | `crates/context-graph-mcp/src/handlers/...` | append-only stream of (file, before_sha, after_sha, line_diff) tuples |
| Shift subscriber (Tokio task) | shipped (run via daemon mode) | `context-graph-mejepa-shift-subscriber` | tail shift-log → materialize panel → call observe()/predict() |
| AST chunker (11 languages) | shipped | `context-graph-core::memory::ast` | typed Chunk { chunk_id, lang, kind } |
| Embedder routing table | shipped (143 routes) | `context-graph-mejepa-embedders::routing` | per (Language, EntityType) → embedder set |
| Per-embedder forward wrappers (neural) | partial (deterministic placeholders) | `context-graph-mejepa-embedders::forward` (proposed) | run actual model forward passes, not deterministic encoders |
| DDA enrichment computation (N+N² signals) | new | `context-graph-mejepa-train::dda` (proposed) | compute pairwise cosine + MI per chunk per shift |
| TCT constellation + Gτ guard | shipped | `context-graph-mejepa-tct` | per-(cat × ent × lang × emb) centroids; guard test |
| 15 instrument slots → Panel(5120-d) | shipped (deterministic) / partial (semantic heads) | `context-graph-mejepa-instruments` | typed feature vectors; PanelBuilder with fail-closed dim |
| Predictor trunk (6-layer transformer) | shipped | `context-graph-mejepa::predictor` | latent-space prediction of panel[t=2] |
| Multi-head prediction stack | shipped (partial — only panel + oracle heads) | `context-graph-mejepa::heads` (proposed extension) | per-failure-class + edge-case + tech-debt + perf + security + cost heads |
| `MeJepaCompiler::compile/verify/observe` | shipped | `context-graph-mejepa::compiler` | the inference + online-update API |
| `RealityPrediction` rich struct | shipped (basic) / proposed extension | `context-graph-mejepa::types` | full Q4 surface (concerns, edge cases, etc.) |
| `mejepa_verify` / `mejepa_predict_latest` MCP tools | shipped | `context-graph-mcp::handlers::tools::mejepa_tools` | agent-facing read API |
| `mejepa_record_agent_feedback` MCP tool | new | `context-graph-mcp` (proposed) | agent-facing write API for surprise events |
| `mejepa_observe_shift` MCP tool | shipped | `context-graph-mcp` | trigger online observe() from a captured shift |
| Continuous self-optimization daemon | new | `context-graph-mejepa::heal::scheduler` (proposed) | tokio task: drift detect → A/B/C promote → continual backprop → recalibrate |
| Storage hygiene daemon | shipped (no scheduler) | `context-graph-mejepa-hygiene::gc` | nightly demote + compact + evict + Merkle compress |
| Active-learning queue surfacing | shipped | `context-graph-mejepa::eval::queue` | when conformal interval > T or surprise recorded |
| Weekly evaluation report writer | shipped | `context-graph-mejepa::eval::report` | per-(category × language) stratified ship-gate report |
| Witness chain + Stage 0/F/Merkle certs | shipped | `context-graph-witness` | cryptographic provenance of every step |
| Per-step training certificate | shipped | `context-graph-mejepa-train::cert` | hash-chained ΔP/ΔK/ΔΩ/ΔΞ + L_step per step |

---

## 3. The multi-head prediction stack

The current `MeJepaPredictor` ships with a 6-layer transformer trunk (hidden 1024, 8 heads) followed by a panel head (predicts panel[t=2] in latent space) and an Oracle head (per-test pass/fail logits + count). The full truth-system requires more heads. **Each head is a thin Linear projection on top of the shared trunk**, so adding a head costs essentially zero parameters and amortizes over the same forward pass.

| Head | Output | Loss | Label source |
|---|---|---|---|
| **Panel head** (shipped) | predicted_panel[t=2] in 5,120-d latent | MSE + cosine + VICReg | frozen-instrument target on observed panel[t=2] |
| **Oracle head** (shipped) | logits over (test_pass × test_count + 1) | binary cross-entropy + count loss | Docker oracle outcome on the patch |
| **Failure-mode head** (new) | logits over `FailureModeClass` (taxonomy in `15_PREDICTION_TAXONOMY.md`) per chunk | softmax × focal | derived from per-test failure log + patch-similarity-graph nearest-corpus exemplar |
| **Edge-case head** (new) | logits over `EdgeCaseClass` per chunk × `not_covered_by_tests` boolean | sigmoid multilabel | derived from coverage Δ + symbolic-execution proxy on AST |
| **Tech-debt head** (new) | logits over `DebtClass` per chunk (smell, complexity, dead-code, redundant) | sigmoid multilabel | derived from `clippy` / `pylint` / `eslint` static-analysis snapshot vs prior |
| **Perf head** (new) | predicted (cpu_us_delta, rss_kb_delta, alloc_count_delta) per chunk | regression with Huber loss | derived from `pytest-benchmark` / `cargo bench` / `time` capture vs baseline |
| **Security head** (new) | logits over `SecurityConcernClass` per chunk | sigmoid multilabel | derived from `bandit` / `cargo audit` / `eslint-plugin-security` snapshot |
| **Accuracy head** (new) | predicted (accuracy_metric_delta, calibration_delta) for ML/data-pipeline code | regression | derived from baseline vs new accuracy metrics in test outputs |
| **Cost head** (new) | predicted (CI_minutes_delta, token_cost_delta_for_this_codepath) | regression | derived from CI duration deltas + agent-token-usage telemetry |
| **Reasoning head** (new) | logits over `ReasoningClass` (correct, mostly_correct, plausible_but_wrong, hallucination) for the agent's accompanying explanation | softmax | weak-supervised by E_Reasoning agreement with E_Oracle |

**Architectural property.** Heads compete for limited trunk capacity, but they share the embedding space — a hard bug-prediction example also strengthens the panel-prediction objective via VICReg regularization. The ΔΞ formula remains valid because all heads are predictor-side (target side is still frozen instruments). All head losses are weighted in `L_total` per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §2.2`:

```
L_total = α·L_panel + β·L_pairwise + γ·L_action + δ·L_constellation + ε·L_anti-pole + ζ·L_uncertainty + η·L_invariant
        + λ_oracle·L_oracle + λ_fail·L_failure_mode + λ_edge·L_edge_case + λ_debt·L_tech_debt
        + λ_perf·L_perf + λ_sec·L_security + λ_acc·L_accuracy + λ_cost·L_cost + λ_reason·L_reasoning
```

with `λ_*` defaults tuned per `12_IMPLEMENTATION_PHASES.md §B.3` — initially small (0.05) on auxiliary heads, gradually ramped as label quality and head-specific holdout metric improves.

---

## 4. The closed feedback loop (the operator's "continually learn and evolve")

The system has FIVE feedback channels feeding back into ME-JEPA's training stream. Together they ensure ME-JEPA self-optimizes the more it is used.

### 4.1 Oracle channel (the ground truth)

When an agent runs tests (or CI does), the actual pass/fail outcome is captured by the existing E_Oracle instrument and persisted to `CF_MEJEPA_ORACLE_VERDICTS`. The next training step weights this example by `sampling_weight(ex) = base · L_step(ex) · (1 + 5·operator_override) · decay^age` per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §6`. **High-L_step examples (where the prediction was wrong) are over-sampled.** This is shipped today.

### 4.2 Shift channel (the deterministic delta)

Every Edit shift (file SHA delta + cargo-check result + git diff) is captured in `cgreality-shift-log/<session>.jsonl`. The shift subscriber tails this and writes (panel_t-1, action, panel_t) tuples to `CF_MEJEPA_PANELS`. **This is the densest data stream: every keystroke roughly produces a panel.** This is shipped today (subscriber crate exists; needs daemon-mode invocation).

### 4.3 Agent-feedback channel (NEW)

When the agent gets a `RealityPrediction` and acts on it, it can call `mejepa_record_agent_feedback`:

```rust
// MCP tool signature
fn mejepa_record_agent_feedback(
    prediction_id: PredictionId,            // returned by verify() / predict_latest()
    feedback_kind: FeedbackKind,            // Confirmed | Surprise | Omission | Calibration
    agent_explanation: String,              // free text + optional structured extras
    actual_outcome: Option<ActualOutcome>,  // if known (e.g., test result)
    severity: SurpriseSeverity,             // Low | Medium | High | Catastrophic
) -> Result<FeedbackRecorded>
```

The handler:
1. Persists a `SurpriseEvent { prediction_id, feedback_kind, agent_explanation, actual_outcome, severity, ts, witness_hash }` to `CF_MEJEPA_AGENT_FEEDBACK` keyed by `(prediction_id, ts)`.
2. Resolves the original `RealityPrediction` from `CF_MEJEPA_LIVE_PREDICTIONS`.
3. Computes a calibration delta: `expected_calibration_at_confidence(prediction.confidence) − observed_outcome_correctness`.
4. Multiplies `sampling_weight` for the contributing chunks by `1 + 2·severity_score` for the next epoch.
5. If `severity == Catastrophic` AND `feedback_kind == Surprise`, posts a high-severity event to the active-learning queue AND the drift detector AND the operator-visible alerts stream.
6. Writes a witness-chain entry binding the feedback to the original prediction's witness hash; both sides become permanent provenance.

**This is the operator's "agents need a way to feed back into the me jepa model confirmation of predictions and give information whenever surprise happens."** It is the closing of the loop ME-JEPA was missing.

### 4.4 Operator-override channel (the weight=6 examples)

When the operator labels an example via the active-learning queue (`mejepa_active_learning_queue` MCP tool returns the queue, operator marks an entry), the `operator_override_flag` is set on the example. Per the formula in §4.1, this multiplies sampling weight by 6×. Operator examples are gold-standard.

### 4.5 Continuous self-optimization daemon (NEW)

A new long-running tokio task (`context-graph-mejepa::heal::scheduler`) runs the following loop forever, gated by config but on by default:

```rust
loop {
    tokio::select! {
        _ = next_observe_tick() => {                                  // every 250ms
            // Drain any unprocessed shifts → observe() with EWC++ regularization
            scheduler.drain_pending_shifts(deadline_ms = 200).await?;
        }
        _ = next_drift_check_tick() => {                              // every 60s
            let snapshot = drift::DriftWindow::snapshot()?;
            match drift::classify(&snapshot) {
                DriftLevel::Soft => scheduler.queue_lora_refresh().await?,
                DriftLevel::Hard => scheduler.queue_calibration_recalibrate().await?,
                DriftLevel::Catastrophic => scheduler.queue_full_retrain_alert().await?,
                DriftLevel::None => {}
            }
        }
        _ = next_active_learning_tick() => {                          // every 600s
            let queue = eval::queue::ActiveLearningQueueState::current()?;
            for entry in queue.high_priority(top_n = 10) {
                if entry.kind == ActiveLearningKind::AgentSurprise {
                    scheduler.fast_track_observe(entry).await?;       // immediate update
                }
            }
        }
        _ = next_promote_tick() => {                                  // every 3600s
            if scheduler.has_pending_lora_refresh() {
                heal::lora_refresh::run().await?;
            }
            if scheduler.should_run_full_retrain() {
                heal::full_retrain::run_periodic().await?;            // A/B/C with promotion gate
            }
        }
        _ = next_continual_backprop_tick() => {                       // every 1800s
            let plasticity = learning_signal::continual_backprop::detect_dormant_units()?;
            if plasticity.dormant_fraction > 0.15 {
                learning_signal::continual_backprop::reinit_dormant_units(plasticity)?;
            }
        }
    }
}
```

**No human in the loop unless drift is Catastrophic.** The system observes shifts every 250ms, checks drift every minute, drains active-learning every 10 minutes, runs LoRA refresh + full-retrain hourly, and triggers continual backprop every 30 minutes. Catastrophic drift surfaces to the operator + writes a high-severity blocker; everything else is automatic.

This is the operator's "ME-JEPA system needs to continually learn and evolve and improve itself the more it is used, it continually self optimizes."

---

## 5. Storage layer extensions

Existing CFs (per `docs/ruvectorfindings/specs/_README.md` §1 and the `context-graph-mejepa-cf` crate) cover panels, weights, certs, oracle verdicts, mutation corpus, live predictions, shift watermark, constellation, Fisher snapshots, drift window/history, heal reports, distill steps, active pointers, eval reports, active-learning queue, patch graph.

**Three new CFs needed for the truth-system extension:**

| CF | Key | Value (serde JSON, 0600 perms) | Purpose |
|---|---|---|---|
| `CF_MEJEPA_AGENT_FEEDBACK` | `(prediction_id, agent_id, ts)` | `SurpriseEvent { kind, explanation, actual_outcome, severity, witness_hash, sampling_weight_bump }` | the agent-side feedback ledger; bound by witness_hash to the original prediction |
| `CF_MEJEPA_DDA_SIGNALS` | `(panel_id, chunk_id)` | `DdaSignals { per_embedder_cosine: [f32; N], pairwise_mi_to_centroid: [[f32; N]; N] }` | the n·(N+N²) DDA signal block per chunk per panel; cached for fast Gτ |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | `(failure_mode_class, embedder_violation_signature)` | `Vec<ExemplarRef { task_id, mutation_kind, chunk_id, oracle_outcome, witness_hash }>` | per-failure-mode reverse index for retrieval — when prediction says "WrongAlgorithm in nested traversal," we can show the closest 5 corpus exemplars |

Storage hygiene per `docs/ruvectorfindings/specs/_README.md` Phase 6 still applies — these CFs participate in the nightly GC, tier compression policy, and 115 GB hard quota.

---

## 6. End-to-end data flow (one Edit, soup-to-nuts)

This is the operational sequence the futurebuild plan delivers. Steps marked **NEW** are added by this directory; everything else is already shipped at code level and just needs to be wired into the daemon.

```
T+0    Agent calls Edit { file: foo.py, old_string: ..., new_string: ... }

T+1    PreToolUse hook records pre_sha for foo.py.

T+2    The Edit returns; PostToolUse hook computes after_sha,
       runs cargo check (if Rust file), git diff, appends a JSONL line
       to /tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session>.jsonl.

T+3    Shift subscriber tails the JSONL line (within 250ms — pollNoAwait).
       It reads foo.py @ after_sha, calls
       memory::ast::chunk_with_options(foo.py, Language::Python, opts)
       to produce Vec<Chunk>.

T+4    For each chunk:
         - Look up routing: routing::lookup(chunk.lang, chunk.kind) → embedder set
         - For each embedder in set, call EmbedderForward::forward(chunk)
           returning the per-embedder vector. NEW for non-deterministic
           neural embedders (currently placeholder).
         - Compute DDA signals: pairwise cosine + cached MI matrix.
           Persist to CF_MEJEPA_DDA_SIGNALS.

T+5    PanelBuilder assembles Panel(5,120-d) using all 15 instrument slots
       over the union of chunks' embedder outputs + scalar features
       (BFS depth, blame age, churn, complexity, coverage Δ, repo health).

T+6    ME-JEPA verify(panel_t-1, action=foo.py edit, panel_t):
         - predictor forward → predicted_panel + per-head outputs
         - conformal predictor wraps the prediction with a calibrated interval
         - TCT Gτ guard checks cos(φ_m(predicted_panel), c_m) ≥ τ_m for each m
         - assemble RealityPrediction { verdict, confidence_interval,
                                         predicted_failure_modes,
                                         predicted_edge_cases,
                                         predicted_tech_debt,
                                         predicted_perf_regressions,
                                         predicted_security_concerns,
                                         predicted_accuracy_degradations,
                                         predicted_cost_increases,
                                         predicted_reasoning_class,
                                         closest_exemplars,
                                         violating_embedders,
                                         violating_chunks,
                                         witness_hash }
         - persist to CF_MEJEPA_LIVE_PREDICTIONS keyed by session_id

T+7    UserPromptSubmit hook on next prompt → reads CF_MEJEPA_LIVE_PREDICTIONS
       for this session → renders markdown injection block. NEW: hook needs
       to be extended to read MCP-state-driven predictions, not just shift entries.

T+8    Agent reads the prediction in its prompt context. It either:
         (a) Acts on the prediction (patches predicted-failure path)
         (b) Asks for a fresh prediction after another edit
         (c) Disagrees → calls mejepa_record_agent_feedback. NEW MCP tool.

T+9    If (c): SurpriseEvent persisted to CF_MEJEPA_AGENT_FEEDBACK; sampling
       weight for affected chunks bumped; if severity=Catastrophic, drift
       detector and active-learning queue both notified.

T+10   When the agent runs tests (pytest, cargo test, jest...), test outcomes
       captured by E_Oracle are written to CF_MEJEPA_ORACLE_VERDICTS bound to
       the prediction_id. The continuous self-optimization daemon polls this
       CF every 250ms; on a new oracle outcome, it calls observe() with
       EWC++ regularization, computes ΔP/ΔK/ΔΩ/ΔΞ → L_step, writes a
       training cert chain entry, updates the BatchSampler weights.

T+11   At the next drift-check tick (every 60s), the drift detector compares
       the rolling 1,000-attempt accuracy window to the expected calibration.
       If degraded by >3% absolute, queue lora_refresh; >5% queue calibration
       recalibrate; >8% queue full_retrain alert.

T+12   At the next promote tick (every hour), if any pending refresh exists,
       run the appropriate heal/* module. A/B/C promotion gate is evaluated
       on a held-out window; only the winner becomes the active pointer.

T+13   The agent eventually finishes. mejepa_verify is called on the full
       patch with full context. If verdict is Pass at confidence ≥ ship-gate
       threshold AND every claim in the agent's response reconciles against
       the SHA chain AND no high-severity Q4 concerns are above threshold,
       the agent's "done" is trusted. Otherwise the system surfaces the gap.
```

Steps T+0 through T+13 are the entire operational truth system. All of T+1, T+2, T+3, T+5, T+6 (most of it), T+10, T+11, T+12 are shipped. T+4's neural forward wrappers, T+6's multi-head extension, T+7's MCP-state-driven injection, T+8c/T+9's agent-feedback channel, and the unifying continuous-self-optimization daemon are the futurebuild work.

---

## 7. What "self-healing" means concretely in this architecture

Per the operator's "self-healing and self-learning property," the following invariants must hold without operator action:

| Invariant | Mechanism | When it fires |
|---|---|---|
| Drift below threshold → automatic LoRA refresh | `heal::lora_refresh::run` invoked by the scheduler | drift detector reports Soft (88-85% accuracy band) |
| Calibration drift → automatic conformal recalibration | `heal::calibration::recalibrate` invoked by the scheduler | calibration error rises >3% |
| Plasticity loss → automatic continual backprop | `learning_signal::continual_backprop::reinit_dormant_units` invoked by the scheduler | dormant-unit fraction > 15% |
| Catastrophic drift → A/B/C retrain → operator alert | `heal::full_retrain::run_periodic` invoked + alert written to `CF_MEJEPA_HEAL_REPORTS` with `severity=Catastrophic` | drift > 8% sustained |
| Storage near quota → emergency eviction | `hygiene::quota::quota_check_and_evict` invoked nightly + on demand | disk usage > 110 GB |
| Witness-chain corruption → auto-repair attempt → operator alert if fail | `optimizer_witness_chain_repair_legacy` (existing) + escalation | hash chain validation fails |
| Dormant operator-override examples → re-evaluated | active-learning queue re-surface every 600s | gold-standard example > 30 days unconfirmed |
| Inactive embedder slots → flagged via pairwise MI audit | `mejepa_audit_pairwise_mi` MCP tool + alert | slot's pairwise MI to others > ceiling for two consecutive audits |

**No invariant requires operator action except the catastrophic-retrain alert.** The truth system runs forever on its own; the operator's role narrows to setting policy and acting on alerts.

---

## 8. What "self-learning" means concretely in this architecture

The system gets better the more it is used because of three compounding effects:

1. **Every Edit is a training example.** The shift subscriber turns every keystroke into (panel_t-1, action, panel_t) tuples. The 250ms tick rate means even a fast-typing agent contributes hundreds of tuples per session. Over weeks, the corpus grows to millions of examples beyond the seeded 2,400.

2. **Surprise events get over-sampled.** The agent-feedback channel multiplies the sampling weight of examples where the prediction was wrong. A surprised agent tells ME-JEPA "look here," and ME-JEPA's next epoch over-samples the chunks involved by 2–10× depending on severity. This is the active-learning loop closed.

3. **Operator-override examples are gold.** When the operator confirms or overrides a prediction via the active-learning queue, the example is sampled at 6× the base rate and Fisher-protected against catastrophic forgetting. Operator labels become permanent anchors.

The result: in a session-week of normal Claude Code usage, ME-JEPA accumulates roughly an order of magnitude more training data than the seeded corpus, weighted toward the examples that matter most. Per the UTML §VII consolidation integral M(t), with `k_replay ≈ 3` during periodic full-retrain phases and `τ_decay ≈ 365 days` for high-Fisher consolidated examples, the system's effective training corpus grows monotonically. The Phase 8 evaluation report tracks `prediction_oracle_correlation` per (category × language) over rolling windows; a stable monotone climb toward ≥ 0.95 is the empirical signature that self-learning is working.

---

## 9. Critical constraints (carried from CLAUDE.md and the existing constitution)

- **GENERIC-ONLY.** No per-task or per-language hard-coding in any of the new code paths. The multi-head taxonomy must work for every supported language; the agent-feedback channel must be language-agnostic.
- **No HTTP from runtime.** MCP is the only network surface. Agent feedback comes through the MCP tool, not curl/HTTP.
- **No mocks in tests.** Real synthetic data only; real Docker oracle for the corpus; real RocksDB for the storage layer; real tree-sitter for the chunker.
- **Fail-closed errors with explicit codes.** Every new code path uses the `MEJEPA_*` error namespace; no silent fallbacks.
- **Frozen instruments stay frozen.** No gradient flow through E_AST, E_CFG, E_Oracle, E_Witness, etc. The ΔΞ guarantee is structural and must not be weakened.
- **Append-only audit.** Every prediction, every feedback event, every observe() call writes a witness-chain entry. No retroactive edits. No deletes (lifecycle CFs use soft-delete + permanent merge history).
- **Self-bounded storage.** Hard 115 GB quota; nightly GC; tier compression demotion. Self-healing storage as much as self-healing model.

---

## 10. What this blueprint commits us to

Implementing this architecture means delivering, in order:

1. **Daemon mode for the MCP server** that runs the shift subscriber + continuous self-optimization scheduler + nightly GC scheduler as supervised tokio tasks.
2. **Multi-head extensions to the predictor** with the head taxonomy in §3.
3. **`mejepa_record_agent_feedback` MCP tool** + `CF_MEJEPA_AGENT_FEEDBACK` + sampling-weight bump wiring.
4. **`CF_MEJEPA_DDA_SIGNALS` + `CF_MEJEPA_FAILURE_EXEMPLARS`** for the multi-head label substrate.
5. **Prediction-injection hook** so that `RealityPrediction` is rendered into UserPromptSubmit context for the agent.
6. **Neural per-embedder forward wrappers** (replacing deterministic placeholders) per [#3](https://github.com/ChrisRoyse/contextgraph/issues/3).
7. **Per-language mutation operators** so the corpus covers all 11 languages, not just Python, per [#7](https://github.com/ChrisRoyse/contextgraph/issues/7).
8. **Full 300×8 corpus run** so the predictor trains on a real corpus, per [#5](https://github.com/ChrisRoyse/contextgraph/issues/5).
9. **Production rolling-window holdout evaluation** that drives the ship-gate ≥ 0.95 metric, per [#5](https://github.com/ChrisRoyse/contextgraph/issues/5) + [#11](https://github.com/ChrisRoyse/contextgraph/issues/11).
10. **The refactored `.claude/loop.md`** that aligns the per-tick agent behavior with this architecture (Mode A becomes "ship a futurebuild deliverable"; the dual-mode A/B split is retired).

The implementation phasing for these is in [`12_IMPLEMENTATION_PHASES.md`](12_IMPLEMENTATION_PHASES.md). The atomic task graph is in [`13_TASK_INDEX.md`](13_TASK_INDEX.md). The new loop.md content is in [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md).

# SPEC-SHIFT-SUBSCRIBER — ME-JEPA-Code Real-Time Reality-Shift → Online Observe Pipeline (Phase 7)

> **Level 2 — Functional Specification.** Defines WHAT the shift-subscriber domain (Phase 7) must do, not HOW. Implementation details live in `TECH-SHIFT-SUBSCRIBER.md`. Conflicts with the constitution resolve to the constitution.

```xml
<functional_spec id="SPEC-SHIFT-SUBSCRIBER" version="1.0">

<metadata>
  <title>ME-JEPA-Code Shift-Log Subscriber + Phase 7 MCP Tool Surface</title>
  <status>draft</status>
  <owner>Phase 7 ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>      <!-- Phase 1 panel substrate; the 15-slot panel materialized per shift -->
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>     <!-- 11-language chunker invoked when a shift changes a source file -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>        <!-- 21-embedder ensemble executed in parallel per shift -->
    <spec_ref>SPEC-CORPUS</spec_ref>           <!-- corpus_entry_added increment per oracle-verified shift -->
    <spec_ref>SPEC-PREDICTOR</spec_ref>        <!-- predict() body; this spec wires the call -->
    <spec_ref>SPEC-INFERENCE</spec_ref>        <!-- mejepa_verify body; this spec wires its MCP surface -->
    <spec_ref>SPEC-TRAINING</spec_ref>         <!-- per-step training cert when L_step ≥ 0.05 -->
    <spec_ref>SPEC-SELF-HEALING</spec_ref>     <!-- observe() body; this spec wires the call -->
    <spec_ref>SPEC-WITNESS</spec_ref>          <!-- E_Witness panel slot for each consumed shift -->
    <spec_ref>SPEC-UTML</spec_ref>             <!-- L_step gate for online observe sampling -->
    <spec_ref>SPEC-HYGIENE</spec_ref>          <!-- 4 GB instrument cache LRU policy + watermark CF tier policy -->
    <spec_ref>SPEC-TCT</spec_ref>              <!-- Gτ guard called from mejepa_verify on each shift -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.8.1 (architecture), §3.8.2 (feedback loop closure), §3.8.3 (existing artifact channels), §3.8.4 (backpressure / durability / restart safety), §3.8.5 (new MCP tools), §3.8.6 (performance budget), §3.8.7 (self-updating reality model end-state), §7 Phase 7 (DoD + verification command)</source>
    <source type="design">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md §F priorities #1, #2, #4, #6, #7 (capture every signal, primary inference path, real-time pipeline binding API), §13 (artifact layout post-pivot)</source>
    <source type="design">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §7 (per-shift latency breakdown ~80 ms with full ensemble)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (`<reality_loop_contract>` rule on subscriber latency, `<performance_budgets>` shift_subscriber_observe_latency_p50/p99 rows, `<failure_taxonomy>` MEJEPA_OBSERVE_BATCH_NAN, MEJEPA_OBSERVE_INTEGRITY_VIOLATION, CCREALITY_ENGINE_RETIRED, `<coding_standards>` mcp_tool_namespace_lock, `<anti_patterns>` no reintroduction of reality_run_attempt)</source>
    <source type="design">docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md (existing shift-log producer — read-only authority; the subscriber consumes its `cgreality-shift-log/<session_id>.jsonl` output verbatim)</source>
    <source type="memory">memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md (every outer-loop turn is a training tuple; subscriber is the binding API)</source>
  </source_documents>
</metadata>

<overview>
The shift-subscriber is the binding API between ContextGraph's deterministic reality substrate
(the PostToolUse Edit hook + `cgreality-shift-log/<session_id>.jsonl` durable stream + the
witness-chain) and ME-JEPA-Code's online learning surface. Its job is to close the four-step
loop **see edit → predict → observe outcome → update** within seconds of any harness edit, so
the predictor stays at most a few seconds behind durable reality for files the outer-loop has
touched. Without this domain, ME-JEPA is a batch-trained snapshot model; with it, ME-JEPA is a
self-updating reality compiler whose accuracy improves with every Claude Code outer-loop turn —
without re-instrumenting any existing producer of harness artifacts.

The subscriber is realized as a single Tokio task hosted inside the existing `context-graph-mcp`
crate (or, equivalently, a new `context-graph-mejepa-shift-subscriber` crate the MCP server
spawns). It tails the JSONL shift log, parses each `ShiftRecord` per the schema produced by the
PostToolUse Edit hook, materializes a panel-at-t=1 from the post-edit file SHA via a file-SHA-keyed
instrument cache, and routes the panel to one of two destinations: (1) when the same shift batch
also contains test-artifact deltas (e.g., `validation.json` shifted), the subscriber computes
panel-at-t=2 with a populated `E_Oracle` slot and calls `mejepa.observe(panel_t1, panel_t2,
oracle_outcome)` for an online weight update gated by the UTML L_step ≥ 0.05 sampling-weight
threshold; (2) when no oracle outcome accompanies the shift, the subscriber calls `mejepa.predict(
panel_t1)` and persists the live prediction into `CF_MEJEPA_LIVE_PREDICTIONS` so any subsequent
`mejepa_predict_latest` MCP call serves a fresh result. After every shift the subscriber writes
the `shift_id` of the just-consumed entry into `CF_MEJEPA_SHIFT_WATERMARK`, making restart safe:
on resume the subscriber re-tails from the last persisted watermark, never re-processes a shift,
never skips one. Per-shift end-to-end target is < 5 s p50 (cached instrument path) and < 30 s p99
(when AST/CFG/DataFlow rebuilds are needed for previously unseen file SHAs); doc 12 §7's parallel
14-embedder breakdown shows the inner work is ~80 ms when warmed, leaving the rest of the budget
for cold-cache rebuilds and disk writes.

The same subscriber owns the new ME-JEPA MCP tool surface defined in doc 09 §3.8.5 plus doc 10
§F priority #7. Five tools land under the locked `mcp__cgreality__mejepa_*` namespace:
`mejepa_predict_latest` (most recent live prediction for the active session/file set);
`mejepa_observe_shift` (idempotent reprocessing of a specific shift, used for testing and
replay); `mejepa_subscriber_status` (watermark, lag, error rate, instrument-cache hit rate, RAM
high-water mark); `mejepa_capture_audit` (for a given attempt, the complete signal capture —
every `E_*` instrument value, every aux-head prediction, every counterfactual perturbation tested,
the full auditability surface per doc 09 §3.8.5); and `mejepa_verify` (cross-cutting with
SPEC-INFERENCE — this spec wires its MCP surface, while SPEC-INFERENCE owns the seven-gate
ladder body). The retired `mcp__cgreality__reality_run_attempt` and adjacent pre-pivot tool
names MUST remain fail-closed with `CCREALITY_ENGINE_RETIRED` per constitution
`<anti_patterns>`; this spec adds new namespaced tools without revising or shimming retired ones.

Beneficiaries: (1) the operator gains a self-tuning ME-JEPA whose holdout correlation against
Docker-oracle outcomes climbs continuously rather than only after batch retrains; (2) any
external AI agent over MCP can ask `mejepa_predict_latest` for the predictor's freshest take on
the working tree; (3) the Phase 5 self-healing drift detector receives a populated rolling
1,000-attempt window of `(predicted_set, actual_oracle)` pairs without any code change in Phase 5;
(4) the Phase 8 ship gate (prediction-oracle correlation ≥ 0.95) gets its denominator from this
subscriber — every shift yields an attribution-traceable prediction-and-eventual-oracle pair.
</overview>

<user_types>

  <user_type id="UT-01" name="PostToolUse Edit hook (shift producer)">
    <description>The existing PreToolUse/PostToolUse hook pair (`scripts/cc/hook_pre_tool_use_edit.sh`, `scripts/cc/hook_post_tool_use_edit.sh`) records before/after SHAs and appends a JSONL entry per Edit/Write tool call to `cgreality-shift-log/<session_id>.jsonl`. The shift producer is read-only with respect to this spec — the subscriber consumes its output verbatim.</description>
    <permissions>WRITE-only to `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session_id>.jsonl`; never reads ME-JEPA state; not affected by subscriber backpressure (writer side is unbounded).</permissions>
    <goals>Produce a durable, ordered, append-only JSONL stream per session; tolerate subscriber downtime; never block the outer loop on subscriber consumption.</goals>
  </user_type>

  <user_type id="UT-02" name="Outer-loop optimizer (Claude Code, this agent)">
    <description>The Claude Code outer-loop session per CLAUDE.md §11. Every Edit/Write tool call this agent makes flows through the PostToolUse hook into the shift log. The optimizer also calls the new MCP tools to query subscriber state and fetch live predictions.</description>
    <permissions>READ all `mcp__cgreality__mejepa_*` tools; WRITE shift-log entries indirectly via Edit/Write tool calls; cannot mutate watermark CF directly (only the subscriber writes there).</permissions>
    <goals>Get a fresh prediction for the working-tree state via `mejepa_predict_latest` without waiting for tests to run; observe the subscriber lag via `mejepa_subscriber_status` and pause edit-heavy work when lag exceeds the alert threshold.</goals>
  </user_type>

  <user_type id="UT-03" name="Operator (human reviewer)">
    <description>The human running the system. Audits captured signal completeness via `mejepa_capture_audit`, monitors lag/error rate via `mejepa_subscriber_status`, drives replay testing via `mejepa_observe_shift`.</description>
    <permissions>READ all subscriber state; APPROVE manual watermark resets (rare, requires writing a `memory/decisions/` entry per CLAUDE.md §6); cannot silently mutate the LRU capacity, the L_step sampling threshold, or the lag alert threshold.</permissions>
    <goals>Verify the subscriber consumed every shift the producer wrote (no gaps); verify per-attempt audit bundles surface every `E_*` instrument value and every aux-head prediction with no silent drops; verify the lag alert fires within 1 minute when contrived backpressure is applied.</goals>
  </user_type>

  <user_type id="UT-04" name="External AI agent (predict/verify caller)">
    <description>Any AI agent over MCP — Claude Code in the outer loop, an external coding agent, or eventually ME-JEPA itself by gradient ascent — that asks for the predictor's current take on a session's working tree or for a verdict on a candidate patch.</description>
    <permissions>READ-only via `mcp__cgreality__mejepa_predict_latest` and `mcp__cgreality__mejepa_verify`; cannot trigger observe(), cannot write to the watermark CF, cannot mutate the instrument cache.</permissions>
    <goals>Receive `RealityPrediction` results dated within seconds of the most recent edit on the requested session; receive `VerifyVerdict::Approve` for in-distribution sharp-confidence patches per SPEC-INFERENCE.</goals>
  </user_type>

  <user_type id="UT-05" name="Phase 5 Self-Healing Loop">
    <description>The drift-detection + EWC++ + A/B/C retrain machinery per SPEC-SELF-HEALING. Reads the rolling 1,000-attempt window of `(predicted_set, actual_oracle)` pairs that the subscriber has populated into `CF_MEJEPA_LIVE_PREDICTIONS` and `CF_MEJEPA_ORACLE_VERDICTS`.</description>
    <permissions>READ `CF_MEJEPA_LIVE_PREDICTIONS`; READ `CF_MEJEPA_ORACLE_VERDICTS`; never writes to the watermark CF; owns its own `observe()` body — this spec only wires the subscriber-side call site.</permissions>
    <goals>Receive a continuously growing window with no temporal gaps; never see a prediction whose paired oracle outcome arrived but was dropped because the subscriber crashed mid-shift.</goals>
  </user_type>

  <user_type id="UT-06" name="Phase 6 Hygiene Loop">
    <description>The disk + memory budget enforcer per SPEC-HYGIENE. Owns the LRU eviction policy on the 4 GB instrument cache and the tier-compression policy on the watermark + live-prediction CFs.</description>
    <permissions>READ all subscriber state; APPLY tier transitions on cold panels; RESIZE the instrument cache capacity within constitution bounds (default 4_096 entries, max 16_384 per `<performance_budgets>`).</permissions>
    <goals>Keep the subscriber's RAM footprint under the 4 GB instrument-cache + 512 MB RSS budget; ensure watermark CF growth is bounded.</goals>
  </user_type>

  <user_type id="UT-07" name="Phase 8 Evaluation Loop">
    <description>The ship-gate evaluator per `phase_plan_summary` Phase 8. Reads paired `(prediction, oracle)` tuples to compute the rolling-window prediction-oracle correlation. The end goal (≥ 0.95 stably) is its single responsibility.</description>
    <permissions>READ `CF_MEJEPA_LIVE_PREDICTIONS` and `CF_MEJEPA_ORACLE_VERDICTS`; WRITE rolling-window correlation reports.</permissions>
    <goals>Receive an unbroken stream of paired tuples sufficient to detect a 0.95 correlation transition.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-SHIFT-01" priority="must-have">
    <narrative>
      <as_a>Outer-loop optimizer (UT-02)</as_a>
      <i_want_to>Have the subscriber consume every shift the PostToolUse Edit hook writes, in order, exactly once, even across MCP-server restarts and outer-loop pauses</i_want_to>
      <so_that>The Phase 5 drift window and the Phase 8 ship-gate evaluator have a complete, gap-free training stream</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-01-01">
        <given>A `cgreality-shift-log/<session_id>.jsonl` containing 100 shift records and an empty `CF_MEJEPA_SHIFT_WATERMARK`</given>
        <when>The subscriber Tokio task is spawned via `ShiftSubscriber::run()`</when>
        <then>All 100 shifts are consumed in order; after the last shift the watermark equals shift 100's `shift_id`; no shift is consumed twice; consumption rate satisfies the &lt; 5 s p50 / &lt; 30 s p99 budget per `<performance_budgets>` `shift_subscriber_observe_latency_p50/p99`</then>
      </criterion>
      <criterion id="AC-SHIFT-01-02">
        <given>The subscriber has consumed shifts 1-50; the host process is killed (SIGKILL) at shift 51 mid-processing</given>
        <when>The MCP server restarts and respawns the subscriber</when>
        <then>The subscriber reads the watermark, observes shift 50 as the last fully-consumed shift, and resumes from shift 51 (the in-flight shift is reprocessed exactly once because the watermark is only advanced AFTER `observe()`/`predict()` and CF persistence both succeed); no shift is silently skipped</then>
      </criterion>
      <criterion id="AC-SHIFT-01-03">
        <given>The PostToolUse hook produces shifts faster than the subscriber consumes them, accumulating backpressure</given>
        <when>The lag exceeds 500 unprocessed shifts and stays above that threshold for &gt; 1 minute</when>
        <then>The subscriber emits `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` to the structured log and the next `mejepa_subscriber_status` MCP call reports `alert_active: true`; consumption continues (no shift dropped); operator may pause outer-loop ingestion or scale the subscriber per doc 09 §3.8.4</then>
      </criterion>
      <criterion id="AC-SHIFT-01-04">
        <given>A malformed JSONL line in the shift log (truncated write, invalid JSON, missing required `shift_id` or `session_id` field)</given>
        <when>The subscriber tails to that line</when>
        <then>Returns `MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL` with the byte offset, the offending line preview (first 200 chars, sanitized), and the session_id; the subscriber skips the malformed line, records the failure to the structured log, and advances the watermark only by the parsed offset (so a fixed log can be re-tailed); no panel materialization is attempted; per-instance error_rate metric increments</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-02" priority="must-have">
    <narrative>
      <as_a>Outer-loop optimizer (UT-02)</as_a>
      <i_want_to>For every shift whose `after_sha` represents a fresh post-edit state, have a panel materialized via the 21-embedder ensemble and a prediction emitted within the latency budget</i_want_to>
      <so_that>The next `mcp__cgreality__mejepa_predict_latest` call returns a prediction reflecting the current working tree, not the pre-edit state</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-02-01">
        <given>A shift record with `after_sha` populated for a Python source file, no test-artifact delta in the same shift, instrument cache cold for this `after_sha`</given>
        <when>The subscriber processes the shift</when>
        <then>A panel-at-t=1 is materialized via parallel inference of 14 content embedders (E1, E2-E4, E5, E7, E8, E10, E11, E14) plus aux instruments (E_Witness on the witness-chain entry, E_Diff on the before/after pair) per doc 12 §7; `mejepa.predict(panel_t1)` is called; the resulting `RealityPrediction` is persisted to `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `(session_id, witness_chain_offset)`; total wall time &lt; 30 s (p99 budget cold); the watermark advances after the persistence write succeeds</then>
      </criterion>
      <criterion id="AC-SHIFT-02-02">
        <given>The same `after_sha` from AC-SHIFT-02-01 reappears in a subsequent shift (the file's bytes have not changed since the previous edit)</given>
        <when>The subscriber processes the second shift</when>
        <then>All 15 panel slots hit the file-SHA-keyed instrument cache (no AST re-parse, no embedder forward pass on cached content); total wall time &lt; 5 s (p50 budget warm); the resulting prediction differs from the previous one ONLY because temporal context (E2/E3/E4) has updated</then>
      </criterion>
      <criterion id="AC-SHIFT-02-03">
        <given>An `mcp__cgreality__mejepa_predict_latest { session_id }` call after the subscriber has consumed N shifts in the named session</given>
        <when>The MCP server dispatches the call</when>
        <then>Returns the most recent `RealityPrediction` for that session by `(witness_chain_offset DESC LIMIT 1)`; if no prediction exists for the session, returns an empty result with `last_consumed_shift_id: None` rather than an error; latency p50 &lt; 50 ms (point read on RocksDB)</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-03" priority="must-have">
    <narrative>
      <as_a>Phase 5 Self-Healing Loop (UT-05)</as_a>
      <i_want_to>For every shift that includes a test-artifact delta (e.g., `validation.json` shifted alongside the source edit), have the subscriber compute panel-at-t=2 with a populated `E_Oracle` slot and call `mejepa.observe()` for an online update</i_want_to>
      <so_that>The drift detector's rolling window grows in real time and the predictor self-tunes within seconds of every test run</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-03-01">
        <given>A shift record whose `subject` includes both a source-file delta AND a `validation.json` delta produced by the same `attempt_id`</given>
        <when>The subscriber processes the shift</when>
        <then>Panel-at-t=2 is materialized with `E_Oracle` populated from the parsed `validation.json` per SPEC-INSTRUMENTS; the UTML L_step is computed (cross-cutting with SPEC-UTML, SPEC-TRAINING); if `L_step ≥ 0.05` the subscriber calls `mejepa.observe(panel_t1, panel_t2, oracle_outcome)`; if `L_step &lt; 0.05` the subscriber emits `MEJEPA_SHIFT_OBSERVE_DROPPED` (informational, not an error) and the per-step training cert is still written for auditability; `corpus_entry_added` increments by 1 per Phase 7 DoD</then>
      </criterion>
      <criterion id="AC-SHIFT-03-02">
        <given>A shift whose `validation.json` delta would yield `L_step` with NaN or Inf during the UTML factor multiplication</given>
        <when>The subscriber attempts to compute L_step</when>
        <then>Returns `MEJEPA_OBSERVE_BATCH_NAN` per `<failure_taxonomy>`; the observe() call is skipped (NOT silently observed with a clamped L_step); the watermark still advances (the shift is consumed once) but the per-step training cert flags `numerical_health: degraded`; the prediction-only path (US-SHIFT-02) still runs so the live prediction remains current</then>
      </criterion>
      <criterion id="AC-SHIFT-03-03">
        <given>A test-artifact delta whose witness-chain offset cannot be replayed (corrupted segment)</given>
        <when>The subscriber attempts panel-at-t=2 materialization</when>
        <then>Returns `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` per `<failure_taxonomy>`; observe() is skipped; the subscriber writes a high-severity entry to `memory/blockers/` per CLAUDE.md §6 SMAP rules; the watermark advances to avoid replaying the same corruption indefinitely</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-04" priority="must-have">
    <narrative>
      <as_a>External AI agent (UT-04)</as_a>
      <i_want_to>Call `mcp__cgreality__mejepa_verify(patch, context, [include_provenance])` over MCP and receive a `VerifyVerdict::Approve` or `EscalateToHuman` per SPEC-INFERENCE's seven-gate ladder</i_want_to>
      <so_that>I can ship sharp in-distribution patches without a human in the loop, while every uncertain or out-of-distribution patch is correctly escalated</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-04-01">
        <given>The MCP server is running and the subscriber Tokio task is alive</given>
        <when>An external agent invokes `mcp__cgreality__mejepa_verify { patch, context, include_provenance: false }`</when>
        <then>The MCP layer validates the JSON-RPC envelope (additionalProperties:false, path canonicalization per SEC-MEJEPA-01), dispatches to the SPEC-INFERENCE-owned `MeJepaCompiler::verify()` body, and returns the resulting `VerifyVerdict` JSON; the seven-gate body itself is specified in SPEC-INFERENCE (REQ-INFER-09 through REQ-INFER-15) — this spec wires only the MCP shim and the namespacing</then>
      </criterion>
      <criterion id="AC-SHIFT-04-02">
        <given>An external agent invokes the retired `mcp__cgreality__reality_run_attempt` tool</given>
        <when>The MCP layer dispatches the call</when>
        <then>Returns `CCREALITY_ENGINE_RETIRED` per constitution `<anti_patterns>`; no compatibility shim is engaged; no `target/release/reality-loop` process is spawned</then>
      </criterion>
      <criterion id="AC-SHIFT-04-03">
        <given>`include_provenance: true` in the `mejepa_verify` call</given>
        <when>The verdict is computed</when>
        <then>The response carries a provenance bundle per SPEC-INFERENCE REQ-INFER-16 plus the subscriber-specific fields `last_consumed_shift_id` and `subscriber_lag_at_call_time` so the caller knows whether the prediction was made against a fresh tree</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-05" priority="must-have">
    <narrative>
      <as_a>Operator (UT-03)</as_a>
      <i_want_to>Call `mcp__cgreality__mejepa_capture_audit(attempt_id)` and receive the COMPLETE signal capture for that attempt — every `E_*` instrument value, every aux-head prediction, every counterfactual perturbation tested</i_want_to>
      <so_that>I can audit ME-JEPA's full decision surface without reverse-engineering binary blobs and without trusting summaries</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-05-01">
        <given>An `attempt_id` that was processed by the subscriber and yielded both a panel-at-t=1 and a panel-at-t=2</given>
        <when>The operator invokes `mcp__cgreality__mejepa_capture_audit { attempt_id }`</when>
        <then>The response carries: every panel slot value (15 slots × 2 timesteps); every aux-head prediction per SPEC-PREDICTOR; the `RealityPrediction` from `predict()`; the `VerifyVerdict` if `verify()` was called; the `oracle_outcome` if test artifacts shifted; the L_step value plus per-factor ΔP/ΔK/ΔΩ/ΔΞ decomposition; every counterfactual perturbation tested by the predictor (cross-cutting with SPEC-PREDICTOR); the witness-chain offsets bracketing the attempt; per-instrument cache hit/miss flags; the panel SHA-256s; the embedder version pins per E1-E21</then>
      </criterion>
      <criterion id="AC-SHIFT-05-02">
        <given>An `attempt_id` not found in any of the persisted CFs (typo, evicted by hygiene)</given>
        <when>The operator invokes `mejepa_capture_audit`</when>
        <then>Returns a structured response with `attempt_found: false` plus `nearest_neighbors: Vec<AttemptId>` (5 closest by witness_chain_offset distance) so the operator can debug; latency p50 &lt; 100 ms; never silently returns a partial bundle</then>
      </criterion>
      <criterion id="AC-SHIFT-05-03">
        <given>An attempt was processed but one of the 15 panel slots failed to materialize (e.g., E_TypeGraph unavailable because pyright was missing on that host)</given>
        <when>The operator invokes `mejepa_capture_audit`</when>
        <then>The response carries the populated 14 slots plus an explicit `missing_slots: ["E_TypeGraph"]` array with the underlying `MEJEPA_INFER_INSTRUMENT_MISSING` error payload; never silently zero-fills the missing slot; the audit therefore cannot be confused with a clean attempt</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-06" priority="must-have">
    <narrative>
      <as_a>Operator (UT-03)</as_a>
      <i_want_to>Call `mcp__cgreality__mejepa_observe_shift(shift_id)` to idempotently re-trigger consumption of a specific shift (for replay testing, drift-injection drills, debugging an observe() that misbehaved)</i_want_to>
      <so_that>I can reproduce subscriber-pipeline incidents without rolling back the watermark and re-running the entire log</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-06-01">
        <given>A `shift_id` that has already been consumed (the watermark is past it)</given>
        <when>The operator invokes `mcp__cgreality__mejepa_observe_shift { shift_id }`</when>
        <then>The subscriber re-locates the JSONL entry by `shift_id`, re-materializes the panels, re-runs predict()/observe() depending on whether oracle outcome is present, BUT DOES NOT roll the watermark back; the resulting prediction overwrites the prior `CF_MEJEPA_LIVE_PREDICTIONS` entry for the same `(session_id, witness_chain_offset)` key (idempotent); the per-step training cert chain records the replay with `replay: true` flag for auditability</then>
      </criterion>
      <criterion id="AC-SHIFT-06-02">
        <given>A `shift_id` not found in any session's JSONL</given>
        <when>The operator invokes `mejepa_observe_shift`</when>
        <then>Returns a structured response with `shift_found: false`; never silently no-ops</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-07" priority="must-have">
    <narrative>
      <as_a>Operator (UT-03) and outer-loop optimizer (UT-02)</as_a>
      <i_want_to>Query `mcp__cgreality__mejepa_subscriber_status()` and receive an honest health snapshot: watermark, lag, error rate, instrument-cache hit rate, RAM high-water mark, and last-consumed `shift_id`</i_want_to>
      <so_that>I can pause edit-heavy work when lag spikes and verify the subscriber is alive without grepping logs</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-07-01">
        <given>The subscriber Tokio task is alive and has consumed 1,000 shifts across multiple sessions</given>
        <when>The operator invokes `mcp__cgreality__mejepa_subscriber_status()`</when>
        <then>Returns: `last_watermark: ShiftId`; `lag: u64` (unprocessed shifts across all sessions); `lag_alert_active: bool`; `error_rate_per_1000: f32` (rolling 1,000-shift window); `instrument_cache_hit_rate: f32 ∈ [0,1]`; `instrument_cache_capacity: usize` and `instrument_cache_entries: usize`; `rss_bytes: u64`; `task_alive_since: Unix timestamp seconds`; `dropped_l_step_below_threshold_count: u64`; latency p50 &lt; 50 ms (in-memory atomics)</then>
      </criterion>
      <criterion id="AC-SHIFT-07-02">
        <given>The subscriber Tokio task has crashed (panic that escaped its supervisor) and has not been respawned yet</given>
        <when>The operator invokes `mejepa_subscriber_status`</when>
        <then>Returns `task_alive_since: None` plus `last_panic: Some(PanicInfo { backtrace, captured_at })`; never returns stale-but-cached state as if alive</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-08" priority="must-have">
    <narrative>
      <as_a>Phase 6 Hygiene Loop (UT-06)</as_a>
      <i_want_to>The subscriber's instrument cache to obey a 4 GB RAM budget via LRU eviction (cross-cutting with SPEC-HYGIENE), and the watermark CF to never exceed its tier-policy quota</i_want_to>
      <so_that>The subscriber stays inside its 1 dedicated CPU + 512 MB RSS + 4 GB instrument-cache budget per `<performance_budgets>` indefinitely</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-08-01">
        <given>The instrument cache approaches its 4 GB RAM budget under sustained mixed-file-SHA traffic</given>
        <when>The subscriber attempts to insert a new entry that would exceed budget</when>
        <then>The LRU eviction policy drops the least-recently-used entry; if eviction cannot free enough RAM (single panel exceeds 4 GB, structurally impossible per panel-dim 5,120 × f32 = 20 KB but defensive), returns `MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW` and the panel slot fails closed; never silently silently shrinks the panel</then>
      </criterion>
      <criterion id="AC-SHIFT-08-02">
        <given>The watermark CF has accumulated entries from 100,000 sessions over several months</given>
        <when>The hygiene loop runs its tier-policy pass</when>
        <then>Watermark CF is bounded per its tier policy in SPEC-HYGIENE (last-N-sessions retained at gold tier; older watermarks tier-compressed and pruned past 90 days); the subscriber tolerates compaction without missing shifts</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-09" priority="should-have">
    <narrative>
      <as_a>Phase 8 Evaluation Loop (UT-07)</as_a>
      <i_want_to>The subscriber to never silently drop a `(prediction, oracle)` pair so my rolling-window prediction-oracle correlation has zero observation gaps</i_want_to>
      <so_that>The Phase 8 ship gate (≥ 0.95 correlation across 8 mutation categories AND all supported languages) measures real performance, not selection-biased performance</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-09-01">
        <given>1,000 sequential shifts, 200 of which have oracle outcomes attached</given>
        <when>The subscriber runs to completion</when>
        <then>`CF_MEJEPA_LIVE_PREDICTIONS` contains 1,000 entries (one per shift); `CF_MEJEPA_ORACLE_VERDICTS` contains 200 entries; the join on `(session_id, witness_chain_offset)` recovers all 200 paired tuples; no pair is dropped because of L_step gating (the L_step gate only suppresses the OBSERVE update, never the persistence) — explicit per BR-SHIFT-04</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-SHIFT-10" priority="must-have">
    <narrative>
      <as_a>Outer-loop optimizer (UT-02)</as_a>
      <i_want_to>The full Phase 7 DoD verification command to pass: `cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./candidate.patch` reports `panel_materialization` &lt; 1 s, a `me_jepa_verify_verdict`, `me_jepa_predicts_match_oracle: true` on a 5% holdout sample, `corpus_entry_added: 1`, `total_corpus_size: previous + 1`</i_want_to>
      <so_that>End-to-end Phase 7 integration is provably wired up before Phase 8's ship-gate evaluation can run honestly</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-SHIFT-10-01">
        <given>A fresh checkout with Phase 0 corpus, Phase 1-4 weights loaded, Phase 7 subscriber alive, an unprocessed SWE-bench Lite task `astropy__astropy-12907`, a candidate patch on disk</given>
        <when>The verification CLI runs end-to-end</when>
        <then>The subscriber consumes the resulting shift entries (the patch + the test run shifts); panel materialization completes in &lt; 1 s (instruments warm); `mejepa_verify` returns a verdict; the predicted oracle outcome is checked against the actual Docker oracle on a 5% sample and matches; one new corpus entry is appended (the panel + oracle verdict tuple); the corpus size grows by 1; FSV evidence at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` covers the full round-trip</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-SHIFT-01" story_ref="US-SHIFT-01" priority="must">
    <description>The shift-subscriber SHALL be a single Tokio task (`pub async fn run(self) -> Result<(), SubscriberError>`) hosted in `crates/context-graph-mejepa-shift-subscriber/` and spawned by the `context-graph-mcp` server during MCP startup. The task tails the per-session JSONL files under `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session_id>.jsonl`, parses each `ShiftRecord` (matching the existing schema in `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs`), and dispatches per-shift via `pub async fn process_shift(&mut self, shift: ShiftEntry) -> Result<ShiftOutcome, SubscriberError>`.</description>
    <rationale>Per doc 09 §3.8.1 the subscriber is a "Tokio task in context-graph-mcp"; per doc 10 §F priority #4 it is "the binding API between the reality loop and ME-JEPA"; spawning under the MCP server keeps the lifecycle tied to the MCP singleton flock per CLAUDE.md §11.12.</rationale>
    <constraints>One Tokio task only (not a thread pool — instrument inference parallelism happens INSIDE the task via candle and tokio::spawn_blocking for AST parsing); 1 dedicated CPU + 512 MB RSS budget per `<performance_budgets>`. The task SHALL respect the Tokio cooperative scheduling (no blocking syscalls without `spawn_blocking`).</constraints>
    <verification_method>Integration test spawns the subscriber on a fixture JSONL with 100 shifts and asserts ordered consumption + watermark advancement; FSV evidence at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/watermark.json` captures the post-run watermark.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-02" story_ref="US-SHIFT-01" priority="must">
    <description>Restart safety: the watermark `(session_id, last_consumed_shift_id)` SHALL be persisted to `CF_MEJEPA_SHIFT_WATERMARK` ONLY AFTER both the panel-materialization-and-routing step AND the `CF_MEJEPA_LIVE_PREDICTIONS` (or `CF_MEJEPA_ORACLE_VERDICTS` when applicable) write succeed. On subscriber spawn, the task reads the persisted watermark per session and resumes tailing from the next byte after the last fully-consumed shift's JSONL line.</description>
    <rationale>Per doc 09 §3.8.4 watermark in `CF_MEJEPA_SHIFT_WATERMARK` is the restart-safety primitive; per BR-SHIFT-02 the watermark advances AFTER write-succeeds-or-explicit-skip to guarantee at-least-once delivery of the (panel, prediction) tuple to durable storage.</rationale>
    <constraints>Watermark monotonicity: `MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS` if a shift_id less than the persisted watermark is observed (this is structurally impossible without manual fiddling and indicates corruption). Per-session watermarks: each `<session_id>.jsonl` has its own watermark row in the CF.</constraints>
    <verification_method>Integration test simulates SIGKILL mid-shift (kill the task between panel-materialize and CF write) and asserts the next spawn re-processes the in-flight shift exactly once; FSV evidence captures pre-kill and post-restart watermark snapshots.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-03" story_ref="US-SHIFT-01" priority="must">
    <description>Lag alerting: the subscriber SHALL maintain a per-session lag counter (unprocessed shift count). When the global lag (sum across sessions) exceeds 500 unprocessed shifts AND the lag has stayed above 500 for &gt; 1 minute, the subscriber SHALL emit `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` to the structured log AND set `alert_active: true` in `mejepa_subscriber_status` responses. Consumption SHALL continue (no shift dropped); the alert is informational so the operator can pause the outer loop or scale the subscriber.</description>
    <rationale>Per doc 09 §3.8.4: "If subscriber lag exceeds 500 unprocessed shifts for &gt; 1 minute, raise an alert ... operator can pause outer-loop ingestion or scale the subscriber"; per `<performance_budgets>` the threshold is 500 / 1 minute exactly.</rationale>
    <constraints>Threshold values (500, 1 minute) MUST be configurable via `MeJepaShiftSubscriberConfig` but bounded: lag threshold ∈ [100, 5,000]; sustained-window ∈ [10 s, 5 min]. Lower bounds prevent alert spam on transient bursts; upper bounds prevent silent acceptance of true backpressure.</constraints>
    <verification_method>FSV harness drives the producer at 100 shifts/sec while the subscriber is throttled to 10 shifts/sec; asserts the alert fires within 70 seconds of lag crossing 500; FSV evidence captures alert timing.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-04" story_ref="US-SHIFT-02" priority="must">
    <description>Per-shift panel materialization: for each consumed shift, the subscriber SHALL invoke the 21-embedder ensemble per doc 12 §7 (parallel inference of E1, E5, E7, E8, E10, E11, E14 on post-edit code; parallel E2-E4 on temporal context; E_Witness on the witness-chain entry; E_Diff on the before/after pair) plus the deterministic instruments per SPEC-INSTRUMENTS to materialize panel-at-t=1. Panels SHALL be cached by `(file_sha, slot_id, code_version)` in an LRU instrument cache (per REQ-SHIFT-08).</description>
    <rationale>Per doc 09 §3.8.1 step 1: "extract panel[t=1] from after_sha (instruments rerun on touched files; cached by file_sha)"; per doc 12 §7 the inner work is ~80 ms when warmed (50 ms parallel embedder inference, 10 ms panel materialize, 10 ms ME-JEPA forward, 5 ms VICReg+L_step, 5 ms RocksDB persist).</rationale>
    <constraints>Panel dim is exactly 5,120 (15 slots × per-slot dim per SPEC-INSTRUMENTS). Cache miss SHALL trigger AST chunker + embedder forward (cross-cutting with SPEC-AST-CHUNKING + SPEC-EMBEDDERS); cache hit SHALL return the cached `Arc<Vec<f32>>` without copy. Missing instrument (e.g., pyright unavailable for E_TypeGraph on Python) SHALL surface `MEJEPA_INFER_INSTRUMENT_MISSING` per `<failure_taxonomy>`; never zero-pad.</constraints>
    <verification_method>Integration test on 100 shifts (50 fresh SHAs, 50 repeat SHAs) measures cache hit rate ≥ 50% and asserts repeat-SHA wall time &lt; 5 s p50; FSV evidence at `predict-latency-evidence.json` records p50/p99/cache-hit-rate.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-05" story_ref="US-SHIFT-02" priority="must">
    <description>Predict-only path (no oracle outcome in shift): the subscriber SHALL invoke `mejepa.predict(panel_t1)` (cross-cutting with SPEC-PREDICTOR + SPEC-INFERENCE) and persist the resulting `RealityPrediction` to `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `(session_id, witness_chain_offset)`. The persisted record SHALL include `last_consumed_shift_id` for replay attribution.</description>
    <rationale>Per doc 09 §3.8.1 step 3: "call mejepa.predict(panel[t=1]) and emit prediction (predictions persist to CF_MEJEPA_LIVE_PREDICTIONS)"; per CLAUDE.md §11.13 the artifact layout enumerates this CF.</rationale>
    <constraints>One prediction per shift (no batching across shifts to keep per-shift latency bounded). Predictions exceeding 16 KiB JSON serialize SHALL fail with `MEJEPA_INFER_DIM_MISMATCH` (defensive; per-prediction is bounded by SPEC-INFERENCE REQ-INFER-19). Phase 6 hygiene tier-compresses old predictions per SPEC-HYGIENE.</constraints>
    <verification_method>Integration test asserts every consumed shift has exactly one row in `CF_MEJEPA_LIVE_PREDICTIONS`; FSV evidence captures one example record.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-06" story_ref="US-SHIFT-03" priority="must">
    <description>Observe path (oracle outcome attached): when the shift's `subject` includes a `validation.json` delta (or any test-artifact delta resolvable to an `OracleOutcome`), the subscriber SHALL materialize panel-at-t=2 with `E_Oracle` populated, compute the UTML L_step (cross-cutting with SPEC-UTML), and: (a) if `L_step ≥ 0.05` invoke `mejepa.observe(panel_t1, panel_t2, oracle_outcome)` (cross-cutting with SPEC-SELF-HEALING) and write the per-step training cert to `CF_MEJEPA_TRAIN_CERTS` (cross-cutting with SPEC-TRAINING); (b) if `L_step &lt; 0.05` emit `MEJEPA_SHIFT_OBSERVE_DROPPED` (informational), still write the per-step cert with `sampling_decision: dropped_l_step_below_threshold` and STILL persist the prediction-oracle pair (REQ-SHIFT-09 below).</description>
    <rationale>Per doc 09 §3.8.1 step 2: "compute panel[t=2] (with E_Oracle from latest test run); call mejepa.observe(panel, oracle_outcome)"; per the prompt's per-shift workflow snippet the L_step ≥ 0.05 sampling-weight gate decides whether to update; per doc 11 §10 the gate is the explicit pruning rule.</rationale>
    <constraints>SPEC-SELF-HEALING owns the observe() body (gradient computation, EWC++ regularization, weight write); this spec only WIRES the call site. SPEC-TRAINING owns the training cert format; this spec only writes via the SPEC-TRAINING-defined writer trait.</constraints>
    <verification_method>Integration test feeds 100 oracle-attached shifts with controlled L_step distribution and asserts the observed/dropped split matches the threshold; FSV evidence at `observe-evidence.json` captures one observed and one dropped example.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-07" story_ref="US-SHIFT-04, US-SHIFT-05, US-SHIFT-06, US-SHIFT-07" priority="must">
    <description>The subscriber SHALL register exactly five new MCP tools under the locked `mcp__cgreality__mejepa_*` namespace: `mejepa_predict_latest { session_id, [limit: u32 = 10] }`, `mejepa_observe_shift { shift_id }`, `mejepa_subscriber_status { }`, `mejepa_capture_audit { attempt_id }`, `mejepa_verify { patch, context, [include_provenance: bool = false] }`. Tool registration follows the existing `crates/context-graph-mcp/src/tools/definitions/` pattern with JSON-Schema enforcement (`additionalProperties: false` on every object schema per the audit-9 lesson).</description>
    <rationale>Per doc 09 §3.8.5 + Phase 7 deliverables; per constitution `<mcp_tool_namespace_lock>`: "All MCP tools added by ME-JEPA SHALL be prefixed `mcp__cgreality__mejepa_*`. The retired `mcp__cgreality__reality_run_attempt` and friends MUST emit fail-closed `CCREALITY_ENGINE_RETIRED`".</rationale>
    <constraints>`mejepa_verify` is cross-cutting with SPEC-INFERENCE — this spec defines the MCP wire contract; SPEC-INFERENCE REQ-INFER-16 defines the verdict ladder. The retired `reality_run_attempt`, `reality_alter_*`, and any pre-pivot `reality-loop` binary CLI shim MUST remain fail-closed with `CCREALITY_ENGINE_RETIRED`.</constraints>
    <verification_method>Integration test invokes each new tool over JSON-RPC and asserts (a) schema compliance, (b) response shape, (c) `additionalProperties: false` rejection of unknown params; FSV evidence at `mcp-tool-roundtrip-evidence.json` captures one successful round-trip per tool plus a `CCREALITY_ENGINE_RETIRED` round-trip on `reality_run_attempt`.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-08" story_ref="US-SHIFT-08" priority="must">
    <description>The instrument cache SHALL be an LRU keyed by `(file_sha: [u8; 32], slot_id: InstrumentSlot, code_version: String)`. Capacity defaults to 4_096 entries; total RAM budget 4 GB per `<performance_budgets>`. Cache values are `Arc<Vec<f32>>` to avoid clone on hit. Eviction is least-recently-used. The cache exposes hit-rate and entry-count metrics to `mejepa_subscriber_status`.</description>
    <rationale>Per doc 09 §3.8.6 budget: "Instrument cache: 4 GB RAM (LRU; same as §6.1 panel cache)"; per doc 09 §3.8.1 the cache is keyed by `file_sha` so unchanged files don't re-extract. Cross-cutting with SPEC-HYGIENE for the eviction/tier policy.</rationale>
    <constraints>Capacity bounded [256, 16_384] entries via config; smaller values destroy hit rate, larger violates RAM budget. Cache invalidation on `code_version` change (the `String` is captured into the key so a redeploy automatically invalidates without operator action).</constraints>
    <verification_method>FSV harness inserts 5,000 entries (above default capacity), asserts LRU eviction kicks in and entry count stabilizes at 4_096; reports hit rate on a sequence of 1,000 lookups against a Zipfian SHA distribution.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-09" story_ref="US-SHIFT-03, US-SHIFT-09" priority="must">
    <description>Persistence-vs-update separation: the subscriber SHALL persist every `(prediction, oracle)` pair into `CF_MEJEPA_LIVE_PREDICTIONS` and `CF_MEJEPA_ORACLE_VERDICTS` REGARDLESS of whether L_step ≥ 0.05. The L_step gate suppresses ONLY the online weight update (the call to `mejepa.observe()`); it never suppresses persistence. This guarantees Phase 5 drift detection and Phase 8 evaluation observe a complete distribution of outcomes.</description>
    <rationale>Per doc 11 §10 L_step gating prunes curriculum items, NOT data points; per doc 09 §5.2 drift detection requires unbiased rolling-window observation; per US-SHIFT-09 the ship gate is invalidated by selection bias.</rationale>
    <constraints>This invariant SHALL be enforced by a structural separation in `process_shift()` — the persistence call site is BEFORE the observe call site, with a guard-test for L_step gating only on the observe branch.</constraints>
    <verification_method>Integration test feeds 1,000 shifts with controlled `L_step` distribution; asserts `CF_MEJEPA_LIVE_PREDICTIONS` and `CF_MEJEPA_ORACLE_VERDICTS` row counts equal the totals (not the observed-only count); FSV evidence at `observe-evidence.json` shows one dropped-but-persisted example.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-10" story_ref="US-SHIFT-05" priority="must">
    <description>The `mejepa_capture_audit { attempt_id }` tool SHALL return the COMPLETE signal capture for `attempt_id`: every panel slot value across all available timesteps; every aux-head prediction per SPEC-PREDICTOR; the `RealityPrediction`; the `VerifyVerdict` if `verify()` was invoked; the `oracle_outcome` if test artifacts shifted; the L_step plus per-factor ΔP/ΔK/ΔΩ/ΔΞ decomposition; counterfactual perturbations tested by the predictor; the bracketing witness-chain offsets; per-instrument cache hit/miss flags; the panel SHA-256s; the embedder version pins per E1-E21. Missing slots SHALL be surfaced explicitly via `missing_slots: Vec<InstrumentSlot>` rather than zero-filled.</description>
    <rationale>Per doc 09 §3.8.5: "for a given attempt, returns the complete signal capture — every E_* instrument value, every aux-head prediction, every counterfactual perturbation tested. Auditability of the full capture surface."</rationale>
    <constraints>Audit bundles are large (potentially several MB for a full attempt). The MCP tool MAY chunk the response into multiple JSON-RPC messages if total size exceeds 1 MB; chunking is transparent to MCP-spec-compliant clients. Bundle JSON SHALL be deterministic given the same persisted state (sorted keys, canonical float formatting) so diffs across replays are meaningful.</constraints>
    <verification_method>Integration test runs one full attempt end-to-end, invokes `mejepa_capture_audit`, and asserts every documented field is present and finite; FSV evidence captures one bundle.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-11" story_ref="US-SHIFT-06" priority="must">
    <description>Idempotent shift replay: `mejepa_observe_shift { shift_id }` SHALL re-process a specific shift (re-materialize panels, re-route to predict()/observe()) without rolling the watermark backward. Re-processing overwrites the `CF_MEJEPA_LIVE_PREDICTIONS` row for the same `(session_id, witness_chain_offset)` key. Per-step training certs from replay SHALL carry `replay: true` for auditability.</description>
    <rationale>Per doc 09 §3.8.5: "idempotently triggers consumption of specific shift (testing/replay)"; required for drift-injection drills per `<testing_requirements>` row "Drift-detection drill" and for debugging observe() incidents.</rationale>
    <constraints>Replay MUST NOT re-run the shift's side effects on disk (no re-edit of source files; the shift records bytes that already happened, replay only re-materializes the model-side response). Idempotency key is `(session_id, witness_chain_offset)`; multiple replays of the same shift overwrite atomically.</constraints>
    <verification_method>Integration test invokes `mejepa_observe_shift` 10 times on the same shift_id; asserts `CF_MEJEPA_LIVE_PREDICTIONS` has exactly one row for that `(session_id, witness_chain_offset)` and the watermark is unchanged.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-12" story_ref="US-SHIFT-07" priority="must">
    <description>The `mejepa_subscriber_status()` tool SHALL return: `last_watermark_per_session: HashMap<SessionId, ShiftId>`; global `lag: u64`; `lag_alert_active: bool`; rolling-1000-shift `error_rate_per_1000: f32`; `instrument_cache_hit_rate: f32 ∈ [0,1]`; `instrument_cache_capacity: usize`; `instrument_cache_entries: usize`; `rss_bytes: u64`; `task_alive_since: Option<i64>` (None when task dead); `dropped_l_step_below_threshold_count: u64`; `last_panic: Option<PanicInfo>`. Latency p50 &lt; 50 ms (in-memory atomics + a single CF read for watermarks).</description>
    <rationale>Per doc 09 §3.8.5: "watermark, lag, error rate, instrument cache hit rate"; the additional fields enable operator triage without grepping logs.</rationale>
    <constraints>Status is a snapshot — atomically loaded from monotonic counters, never reconstructed from log scans. `task_alive_since: None` when the supervisor has detected a panic; never returns stale-but-cached state.</constraints>
    <verification_method>Integration test runs the subscriber for 1,000 shifts, then queries status, asserts every field is populated and finite; injects a panic via test hook and asserts `task_alive_since: None` plus `last_panic: Some(_)`.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-13" story_ref="US-SHIFT-04" priority="must">
    <description>Retired tool surfaces: the MCP server SHALL continue to fail-closed `mcp__cgreality__reality_run_attempt`, `mcp__cgreality__reality_alter_*`, and any pre-pivot harness-write tools with `CCREALITY_ENGINE_RETIRED`. This spec adds new `mcp__cgreality__mejepa_*` tools but MUST NOT modify, alias, or shim any retired tool.</description>
    <rationale>Per constitution `<anti_patterns>` "No `mcp__cgreality__reality_run_attempt` revival, no `target/release/reality-loop` resurrection — the retired CLI shim must remain fail-closed with CCREALITY_ENGINE_RETIRED" and `<mcp_tool_namespace_lock>` row.</rationale>
    <verification_method>Integration test invokes each retired tool name and asserts every call returns `CCREALITY_ENGINE_RETIRED`; FSV evidence captures one round-trip for `reality_run_attempt`.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-14" story_ref="US-SHIFT-01" priority="must">
    <description>Failure-mode taxonomy: the subscriber SHALL surface the following `SubscriberError` variants with SCREAMING_SNAKE `.code()` tags per `<failure_taxonomy>`: `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` (informational; lag > 500 / 1 min); `MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL` (malformed JSONL); `MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW` (RAM budget exceeded after eviction); `MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS` (corruption / manual fiddling); `MEJEPA_SHIFT_OBSERVE_DROPPED` (informational; L_step &lt; 0.05); `MEJEPA_OBSERVE_BATCH_NAN` (delegates to `<failure_taxonomy>`); `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` (witness-chain replay failure); `CCREALITY_ENGINE_RETIRED` (retired surface called).</description>
    <rationale>Per `<failure_taxonomy>` plus the prompt's failure-mode list; per `<error_handling>` "every failure logs context (input shapes, file paths, SHAs) before returning".</rationale>
    <constraints>Each variant carries a structured payload (shift_id, session_id, byte_offset, panic info as relevant). All payloads are serializable to JSON for MCP responses without `unwrap_or` defaults.</constraints>
    <verification_method>Unit test exhaustively constructs each variant and asserts `.code()` returns the expected tag; FSV harness emits one example of each.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-15" story_ref="US-SHIFT-10" priority="must">
    <description>Phase 7 DoD verification: `cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./candidate.patch` SHALL report (per doc 09 §7 Phase 7): `panel_materialization` &lt; 1 s, `me_jepa_verify_verdict` populated (Pass/Fail/OutOfDistribution/Abstain), `me_jepa_predicts_match_oracle: true` on a 5% sample compared against the actual Docker oracle, `corpus_entry_added: 1`, `total_corpus_size: previous + 1`. Witness chain spans outer-loop evidence and ME-JEPA predictions in one shared chain segregated by `witness_type`.</description>
    <rationale>Per doc 09 §7 Phase 7 DoD verbatim; the corpus_entry_added counter increments via the subscriber's persistence path (cross-cutting with SPEC-CORPUS).</rationale>
    <verification_method>The CLI command itself is the verification; FSV evidence at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` captures the round-trip output; integration test asserts every documented metric meets bounds.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-16" story_ref="US-SHIFT-02" priority="must">
    <description>Performance budgets per `<performance_budgets>`: `shift_subscriber_observe_latency_p50 &lt; 5 s`; `shift_subscriber_observe_latency_p99 &lt; 30 s`; subscriber RSS ≤ 512 MB excluding the 4 GB instrument cache; instrument cache RAM ≤ 4 GB; `mejepa_predict_latest` MCP call latency p50 &lt; 50 ms; `mejepa_subscriber_status` p50 &lt; 50 ms; `mejepa_capture_audit` p50 &lt; 100 ms; `mejepa_observe_shift` p50 &lt; 5 s (hits the same per-shift budget).</description>
    <rationale>Per doc 09 §3.8.6 budget table verbatim; per doc 12 §7 the ~80 ms inner work leaves substantial headroom.</rationale>
    <verification_method>FSV harness `predict-latency-evidence.json` records p50/p95/p99 latencies for 1,000 shifts mixing cold and warm cache; asserts every metric meets bounds.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-17" story_ref="US-SHIFT-08" priority="should">
    <description>Configurable knobs via `MeJepaShiftSubscriberConfig`: `lag_alert_threshold_shifts: usize = 500`; `lag_alert_sustain_seconds: u64 = 60`; `instrument_cache_capacity: usize = 4_096`; `l_step_observe_threshold: f32 = 0.05`; `tail_poll_interval_ms: u64 = 100`; `max_concurrent_shifts: usize = 1` (per-task serialization preserves observe() ordering for EWC++ correctness). All knobs are bounded; out-of-bound values fail at config load time with a structured error.</description>
    <rationale>Per `<error_handling>` rule on no silent defaults; per `<anti_patterns>` rule on no `unwrap_or`; per doc 11 §11 EWC++ regularization assumes ordered observe() calls.</rationale>
    <verification_method>Unit test loads each knob at min/max/out-of-bound and asserts the appropriate behavior.</verification_method>
  </requirement>

  <requirement id="REQ-SHIFT-18" story_ref="US-SHIFT-05" priority="must">
    <description>Provenance metadata: every persisted record (predictions, oracle verdicts, watermarks, training certs written from this domain) SHALL carry: `corpus_sha: [u8; 32]`, `embedder_versions: HashMap<EmbedderId, String>`, `frozen_at: i64` (Unix seconds), `code_version: String` (`git describe --always --dirty`).</description>
    <rationale>Per `<provenance>` rule: "Every persistent artifact (panel, training cert, constellation, verdict) carries provenance metadata: corpus_sha, embedder_versions, frozen_at, code_version".</rationale>
    <verification_method>FSV harness opens a sample of persisted records and asserts every documented provenance field is populated and round-trips via `mejepa_capture_audit`.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-SHIFT-01" req_ref="REQ-SHIFT-01">
    <condition>The MCP server starts and a previous subscriber Tokio task is detected as alive (e.g., another MCP process held the singleton flock per CLAUDE.md §11.12)</condition>
    <action>The MCP singleton-flock takeover (SIGTERM(2s)→SIGKILL) evicts the prior process; the new subscriber spawns fresh, reads the watermark CF, resumes per REQ-SHIFT-02.</action>
    <exception>None. The flock contract guarantees at most one subscriber per host.</exception>
  </rule>

  <rule id="BR-SHIFT-02" req_ref="REQ-SHIFT-02">
    <condition>A shift's panel materialization succeeds but the subsequent `CF_MEJEPA_LIVE_PREDICTIONS` write fails (RocksDB transient error)</condition>
    <action>The watermark SHALL NOT advance; the shift remains "in flight"; on retry the shift is re-processed exactly once (idempotent per REQ-SHIFT-11). After 3 consecutive write failures the subscriber surfaces `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` and writes a `memory/blockers/` entry.</action>
    <exception>None. At-least-once delivery to durable storage is the structural correctness invariant; "skipped because of transient I/O" is a violation, never tolerated.</exception>
  </rule>

  <rule id="BR-SHIFT-03" req_ref="REQ-SHIFT-06">
    <condition>An oracle-attached shift has `L_step &lt; 0.05`</condition>
    <action>Suppress the call to `mejepa.observe()` (no online weight update); STILL persist the prediction-oracle pair per REQ-SHIFT-09; emit `MEJEPA_SHIFT_OBSERVE_DROPPED` informational; per-step training cert records `sampling_decision: dropped_l_step_below_threshold` with the L_step value for auditability.</action>
    <exception>None. The L_step gate is from doc 11 §10 and is non-negotiable; relaxing it would re-introduce learning on uninformative tuples and starve EWC++ regularization budget.</exception>
  </rule>

  <rule id="BR-SHIFT-04" req_ref="REQ-SHIFT-09">
    <condition>The L_step gate (BR-SHIFT-03) suppresses the observe() update</condition>
    <action>The persistence path (writes to `CF_MEJEPA_LIVE_PREDICTIONS` and, when applicable, `CF_MEJEPA_ORACLE_VERDICTS`) SHALL still execute. Phase 5 drift detection and Phase 8 evaluation MUST observe the full distribution; selection bias on observed-only tuples invalidates the ship-gate metric.</action>
    <exception>None. The structural separation between persistence and update is enforced in `process_shift()`.</exception>
  </rule>

  <rule id="BR-SHIFT-05" req_ref="REQ-SHIFT-07, REQ-SHIFT-13">
    <condition>An MCP caller invokes `mcp__cgreality__reality_run_attempt`, any `reality_alter_*`, or any retired pre-pivot tool name</condition>
    <action>The MCP layer returns `CCREALITY_ENGINE_RETIRED` per `<anti_patterns>`. The new ME-JEPA tools live exclusively under `mcp__cgreality__mejepa_*`; no compatibility shim is added.</action>
    <exception>None. The retired surface MUST remain retired; the operator's pivot intent is locked.</exception>
  </rule>

  <rule id="BR-SHIFT-06" req_ref="REQ-SHIFT-04, REQ-SHIFT-08">
    <condition>The instrument cache cannot serve a slot for a particular `(file_sha, slot_id, code_version)` and the underlying instrument is unavailable (e.g., pyright not on PATH for E_TypeGraph on Python)</condition>
    <action>Surface `MEJEPA_INFER_INSTRUMENT_MISSING` per `<failure_taxonomy>`; record the missing slot in the panel record's `missing_slots` field; the predict()/observe() call paths MAY still proceed if SPEC-PREDICTOR's policy permits partial panels (SPEC-PREDICTOR-owned decision); never zero-pad.</action>
    <exception>None. Zero-padding is forbidden by `<anti_patterns>` "no silent fallback paths".</exception>
  </rule>

  <rule id="BR-SHIFT-07" req_ref="REQ-SHIFT-03">
    <condition>The lag exceeds the alert threshold</condition>
    <action>Emit the alert (informational, log + status field); CONTINUE consuming shifts; never drop shifts to "catch up".</action>
    <exception>None. The producer side has no rate-limit hook; dropping shifts would create gaps that violate REQ-SHIFT-09 / BR-SHIFT-04.</exception>
  </rule>

  <rule id="BR-SHIFT-08" req_ref="REQ-SHIFT-11">
    <condition>An operator invokes `mejepa_observe_shift` on a shift whose `shift_id` is past the watermark</condition>
    <action>Re-process the shift (idempotent overwrite of the `CF_MEJEPA_LIVE_PREDICTIONS` row); per-step training cert flagged with `replay: true`; watermark NOT rolled back; observe() side effects are de-duplicated by EWC++'s Fisher-information accumulation.</action>
    <exception>None. Replay is a first-class operation; rolling the watermark backward would silently drop subsequent shifts.</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-SHIFT-01" req_ref="REQ-SHIFT-01" priority="critical">
    <scenario>Shift-log file rotates mid-run (the producer may rotate per session boundary or per disk-quota threshold).</scenario>
    <expected_behavior>The subscriber's tail logic detects file truncation via inode or file-size monotonicity check; on rotation it opens the new file and resumes from byte 0; no shift is silently dropped because the watermark is per-`(session_id, shift_id)` not per-byte-offset.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-02" req_ref="REQ-SHIFT-01" priority="high">
    <scenario>Shift-log JSONL line is partially written when the subscriber tails (writer mid-flush; line ends without `\n`).</scenario>
    <expected_behavior>The subscriber waits for `\n` before parsing (uses a line-buffered reader); never parses a half-written line; on reader-EOF without newline it pauses ≤ `tail_poll_interval_ms` and retries.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-03" req_ref="REQ-SHIFT-04" priority="high">
    <scenario>A shift's `after_sha` references a file that was deleted on disk by a subsequent (un-shifted, e.g., human-applied) operation.</scenario>
    <expected_behavior>AST chunker returns `MEJEPA_CHUNKER_PARSE_FAILED` per `<failure_taxonomy>`; the subscriber surfaces `MEJEPA_INFER_INSTRUMENT_MISSING` for the affected slots; predict() path proceeds with partial panel per BR-SHIFT-06; the file deletion is logged for operator inspection (this represents a reality drift the subscriber CANNOT itself fix).</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-04" req_ref="REQ-SHIFT-04" priority="critical">
    <scenario>Two shifts arrive in tight succession on the same `(file_sha, slot_id)` (different `shift_id`s but same file content).</scenario>
    <expected_behavior>Second shift hits the instrument cache; AST + embedder forward not re-run; panel materialization wall time &lt; 5 s p50.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-05" req_ref="REQ-SHIFT-06" priority="high">
    <scenario>An oracle-attached shift's `L_step` is exactly 0.05 (boundary).</scenario>
    <expected_behavior>The strict inequality `≥ 0.05` gates the observe() call (so 0.05 itself triggers observe, 0.0499 does not). Documented in FSV evidence.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-06" req_ref="REQ-SHIFT-02" priority="critical">
    <scenario>The subscriber SIGKILLs between `CF_MEJEPA_LIVE_PREDICTIONS` write and watermark advance.</scenario>
    <expected_behavior>On respawn, the same shift is re-processed; the row in `CF_MEJEPA_LIVE_PREDICTIONS` is overwritten atomically (idempotent per REQ-SHIFT-11); the per-step training cert from the prior partial run is orphaned but harmless (Phase 5 reads only the final cert per step). At-least-once delivery is the structural invariant.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-07" req_ref="REQ-SHIFT-03" priority="high">
    <scenario>Lag oscillates around the alert threshold (briefly crosses 500 every few seconds).</scenario>
    <expected_behavior>The sustained-window check (`lag_alert_sustain_seconds`, default 60) prevents alert flapping; alert fires only when lag is &gt; 500 continuously for the full window; alert clears when lag drops below threshold for the same window.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-08" req_ref="REQ-SHIFT-08" priority="medium">
    <scenario>Instrument cache is sized too small (capacity 16) for the working-set, hit rate collapses.</scenario>
    <expected_behavior>Hit rate &lt; 50% surfaces in `mejepa_subscriber_status`; latency p99 climbs above 30 s and the `<performance_budgets>` violation is logged. Operator may bump capacity via config (bounded [256, 16_384] per REQ-SHIFT-17).</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-09" req_ref="REQ-SHIFT-06" priority="high">
    <scenario>A shift attaches an oracle outcome whose `OracleOutcome` is structurally invalid (FAIL_TO_PASS field missing, exception_class is a non-canonical string).</scenario>
    <expected_behavior>Surface `MEJEPA_CORPUS_INVALID_INPUT` per `<failure_taxonomy>`; observe() path skipped; predict() path still runs (cross-cutting with SPEC-CORPUS); the malformed verdict is NOT silently coerced to a default.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-10" req_ref="REQ-SHIFT-12" priority="medium">
    <scenario>The subscriber Tokio task panics inside an instrument forward pass (e.g., candle CUDA OOM).</scenario>
    <expected_behavior>The supervisor catches the panic via `tokio::task::JoinSet`; sets `task_alive_since: None` plus `last_panic: Some(_)` in subscriber-status; the MCP server respawns the task with a clean state; the in-flight shift's watermark remains pre-advance, so it is re-processed; if the panic recurs three times on the same shift, surface `MEJEPA_OBSERVE_BATCH_NAN` and write a high-severity `memory/blockers/` entry.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-11" req_ref="REQ-SHIFT-07" priority="medium">
    <scenario>An external agent calls `mejepa_predict_latest` with a `session_id` for which no shifts have been consumed yet (first call after MCP startup, or session is brand new).</scenario>
    <expected_behavior>Return a structured response with `prediction: None, last_consumed_shift_id: None, session_known: false`; never returns a stale prediction from a different session; never errors.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-12" req_ref="REQ-SHIFT-11" priority="medium">
    <scenario>Operator invokes `mejepa_observe_shift` concurrently 10 times on the same `shift_id`.</scenario>
    <expected_behavior>The subscriber serializes shift processing per `max_concurrent_shifts: 1` (REQ-SHIFT-17); the 10 invocations run sequentially; each overwrites the same `CF_MEJEPA_LIVE_PREDICTIONS` row; the final state is byte-equal to a single invocation; per-step training certs record 10 `replay: true` entries for auditability.</expected_behavior>
  </edge_case>

  <edge_case id="EC-SHIFT-13" req_ref="REQ-SHIFT-15" priority="critical">
    <scenario>Phase 7 DoD verification command runs but the `me_jepa_predicts_match_oracle` 5% sample yields disagreement on 2 of 5 sampled tasks.</scenario>
    <expected_behavior>The command exits non-zero with `MEJEPA_PHASE7_DOD_DISAGREEMENT`; FSV evidence captures the disagreeing task IDs with predicted vs actual oracle outcomes; the operator can use this signal to decide whether to expand the corpus or re-train (cross-cutting with Phase 5).</expected_behavior>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-SHIFT-01" http_code="200" req_ref="REQ-SHIFT-03">
    <condition>Subscriber lag exceeds 500 unprocessed shifts for &gt; 1 minute (informational, not a failure).</condition>
    <user_message>ME-JEPA shift-subscriber lag is high. The model may be running behind reality. Consider pausing edit-heavy work or scaling the subscriber.</user_message>
    <internal_message>MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT: lag={N}, sustain_seconds={S}, last_watermark={shift_id}</internal_message>
    <recovery_action>Operator: pause outer-loop ingestion via `mejepa_subscriber_status` polling; or bump subscriber concurrency (currently locked at 1 per REQ-SHIFT-17 for EWC++ correctness — requires careful constitution amendment).</recovery_action>
  </error>

  <error id="ERR-SHIFT-02" http_code="500" req_ref="REQ-SHIFT-01">
    <condition>Malformed JSONL line in shift log.</condition>
    <user_message>ME-JEPA shift-subscriber encountered an unparseable shift entry. The malformed line is logged with byte offset and a sanitized preview.</user_message>
    <internal_message>MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL: session_id={...}, byte_offset={...}, line_preview={...200 chars sanitized...}, parse_error={serde_json error message}</internal_message>
    <recovery_action>Operator: inspect the producer (PostToolUse hook) — likely a partial write or a schema regression; the watermark advances past the malformed line so a fixed log can be re-tailed.</recovery_action>
  </error>

  <error id="ERR-SHIFT-03" http_code="500" req_ref="REQ-SHIFT-08">
    <condition>Instrument cache RAM budget exceeded after eviction (defensive; structurally rare).</condition>
    <user_message>ME-JEPA instrument cache cannot fit a fresh entry. This indicates a panel-dim regression or a memory-budget misconfiguration.</user_message>
    <internal_message>MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW: requested_bytes={...}, current_bytes={...}, capacity_bytes={...}</internal_message>
    <recovery_action>Operator: verify panel dim is exactly 5,120; verify cache capacity setting; verify no leak in instrument forward path.</recovery_action>
  </error>

  <error id="ERR-SHIFT-04" http_code="500" req_ref="REQ-SHIFT-02">
    <condition>Watermark moved backward (a shift_id less than persisted watermark observed).</condition>
    <user_message>ME-JEPA watermark integrity violation. Manual fiddling or storage corruption suspected. Halting the subscriber.</user_message>
    <internal_message>MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS: persisted={...}, observed={...}, session_id={...}</internal_message>
    <recovery_action>Operator: inspect `CF_MEJEPA_SHIFT_WATERMARK`; if intentionally rolled back, write a `memory/decisions/` entry justifying it; the subscriber restarts only after operator override.</recovery_action>
  </error>

  <error id="ERR-SHIFT-05" http_code="200" req_ref="REQ-SHIFT-06">
    <condition>L_step &lt; 0.05 on an oracle-attached shift (informational, not a failure).</condition>
    <user_message>ME-JEPA observe() skipped this shift because the learning signal is below the sampling-weight threshold. Persistence still occurred.</user_message>
    <internal_message>MEJEPA_SHIFT_OBSERVE_DROPPED: shift_id={...}, l_step={...}, threshold={0.05}</internal_message>
    <recovery_action>None required. Phase 8 evaluation still observes the (prediction, oracle) pair via REQ-SHIFT-09.</recovery_action>
  </error>

  <error id="ERR-SHIFT-06" http_code="500" req_ref="REQ-SHIFT-06">
    <condition>NaN/Inf in L_step computation.</condition>
    <user_message>ME-JEPA L_step computation produced a non-finite value. The shift is persisted but observe() is skipped to protect the predictor.</user_message>
    <internal_message>MEJEPA_OBSERVE_BATCH_NAN: shift_id={...}, factors={ΔP={...}, ΔK={...}, ΔΩ={...}, ΔΞ={...}}</internal_message>
    <recovery_action>SPEC-SELF-HEALING-owned recovery: rollback to prior checkpoint per `<failure_taxonomy>` row.</recovery_action>
  </error>

  <error id="ERR-SHIFT-07" http_code="409" req_ref="REQ-SHIFT-06">
    <condition>Witness chain replay broken on the segment associated with the shift.</condition>
    <user_message>ME-JEPA detected a witness-chain integrity violation on this shift. Halting observe() to protect training data.</user_message>
    <internal_message>MEJEPA_OBSERVE_INTEGRITY_VIOLATION: shift_id={...}, witness_chain_offset={...}, expected_hash={...}, observed_hash={...}</internal_message>
    <recovery_action>Operator inspect the witness chain via `mcp__cgreality__witness_chain_inspect`; restore from last verified checkpoint.</recovery_action>
  </error>

  <error id="ERR-SHIFT-08" http_code="410" req_ref="REQ-SHIFT-13">
    <condition>Retired tool surface called.</condition>
    <user_message>The requested tool has been retired in the 2026-05-09 ME-JEPA pivot. Use `mcp__cgreality__mejepa_*` tools instead.</user_message>
    <internal_message>CCREALITY_ENGINE_RETIRED: tool_name={reality_run_attempt|reality_alter_*|...}</internal_message>
    <recovery_action>Caller: migrate to the new tool surface (`mejepa_predict_latest`, `mejepa_verify`, `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`).</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="ShiftEntry">
    <field name="shift_id" type="String" required="true">
      <description>ULID-style identifier per existing `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs::ShiftRecord::new`. Format: `01J<20 uppercase hex>`.</description>
      <constraints>Bounded 23 chars; ASCII; monotonic per session.</constraints>
    </field>
    <field name="timestamp_unix_ns" type="u128" required="true">
      <description>Producer-side wall-clock timestamp in Unix nanoseconds.</description>
    </field>
    <field name="tool_name" type="String" required="true">
      <description>Name of the Edit/Write tool invocation that produced the shift.</description>
    </field>
    <field name="session_id" type="String" required="true">
      <description>Outer-loop session identifier; partitions the JSONL files.</description>
    </field>
    <field name="subject" type="serde_json::Value" required="true">
      <description>The file path or other shift target.</description>
    </field>
    <field name="before" type="serde_json::Value" required="true">
      <description>Pre-edit state including SHA-256 + line count.</description>
    </field>
    <field name="after" type="serde_json::Value" required="true">
      <description>Post-edit state including SHA-256 + line count.</description>
    </field>
    <field name="delta_summary" type="serde_json::Value" required="true">
      <description>Lines added/removed/modified.</description>
    </field>
    <field name="verification" type="serde_json::Value" required="true">
      <description>Optional verification metadata (cargo check status, etc.).</description>
    </field>
    <field name="harness_transition_path" type="Option<String>" required="false">
      <description>Path to associated `harness-transitions/round-NN.json` if any.</description>
    </field>
  </entity>

  <entity name="ShiftOutcome">
    <field name="shift_id" type="ShiftId" required="true"/>
    <field name="path" type="enum {PredictOnly, PredictAndObserve, Skipped(Reason)}" required="true">
      <description>Which subscriber path was taken.</description>
    </field>
    <field name="panel_t1_sha" type="[u8; 32]" required="true"/>
    <field name="panel_t2_sha" type="Option<[u8; 32]>" required="false"/>
    <field name="prediction_persisted_at" type="i64" required="true">
      <description>Unix seconds at the point of persistence.</description>
    </field>
    <field name="l_step" type="Option<f32>" required="false">
      <description>UTML L_step when computed (oracle-attached shifts only).</description>
    </field>
    <field name="observe_invoked" type="bool" required="true"/>
    <field name="cache_hits" type="usize" required="true">
      <description>Number of slot lookups that hit the instrument cache.</description>
    </field>
    <field name="cache_misses" type="usize" required="true"/>
    <field name="latency_ms" type="u64" required="true"/>
  </entity>

  <entity name="WatermarkRecord">
    <field name="session_id" type="String" required="true"/>
    <field name="last_consumed_shift_id" type="String" required="true"/>
    <field name="last_consumed_byte_offset" type="u64" required="true">
      <description>Byte offset in the JSONL file at which the next read starts.</description>
    </field>
    <field name="last_advanced_at" type="i64" required="true">
      <description>Unix seconds at last successful advance.</description>
    </field>
    <field name="provenance_corpus_sha" type="[u8; 32]" required="true"/>
    <field name="provenance_code_version" type="String" required="true"/>
  </entity>

  <entity name="SubscriberStatus">
    <field name="last_watermark_per_session" type="HashMap<SessionId, ShiftId>" required="true"/>
    <field name="lag" type="u64" required="true"/>
    <field name="lag_alert_active" type="bool" required="true"/>
    <field name="error_rate_per_1000" type="f32" required="true">
      <description>Rolling-1000-shift error rate.</description>
      <constraints>Finite; in [0, 1].</constraints>
    </field>
    <field name="instrument_cache_hit_rate" type="f32" required="true">
      <constraints>Finite; in [0, 1].</constraints>
    </field>
    <field name="instrument_cache_capacity" type="usize" required="true"/>
    <field name="instrument_cache_entries" type="usize" required="true"/>
    <field name="rss_bytes" type="u64" required="true"/>
    <field name="task_alive_since" type="Option<i64>" required="false"/>
    <field name="dropped_l_step_below_threshold_count" type="u64" required="true"/>
    <field name="last_panic" type="Option<PanicInfo>" required="false"/>
  </entity>

  <entity name="CaptureAuditBundle">
    <field name="attempt_id" type="String" required="true"/>
    <field name="attempt_found" type="bool" required="true"/>
    <field name="panels" type="HashMap<TimeStep, PanelSnapshot>" required="true">
      <description>Panel slots across all available timesteps (t=0, t=1, t=2 as available).</description>
    </field>
    <field name="missing_slots" type="Vec<InstrumentSlot>" required="true">
      <description>Slots that failed to materialize, with the underlying error code.</description>
    </field>
    <field name="aux_head_predictions" type="HashMap<AuxHeadId, AuxHeadPrediction>" required="true"/>
    <field name="reality_prediction" type="Option<RealityPrediction>" required="false"/>
    <field name="verify_verdict" type="Option<VerifyVerdict>" required="false"/>
    <field name="oracle_outcome" type="Option<OracleOutcome>" required="false"/>
    <field name="l_step" type="Option<f32>" required="false"/>
    <field name="utml_factors" type="Option<UtmlFactorBundle>" required="false">
      <description>ΔP, ΔK, ΔΩ, ΔΞ decomposition.</description>
    </field>
    <field name="counterfactual_perturbations" type="Vec<CounterfactualResult>" required="true"/>
    <field name="bracketing_witness_offsets" type="(u64, u64)" required="true"/>
    <field name="cache_hit_flags" type="HashMap<InstrumentSlot, bool>" required="true"/>
    <field name="panel_shas" type="HashMap<TimeStep, [u8; 32]>" required="true"/>
    <field name="embedder_versions" type="HashMap<EmbedderId, String>" required="true"/>
    <field name="nearest_neighbors" type="Vec<AttemptId>" required="true">
      <description>5 closest attempts when `attempt_found: false`; empty otherwise.</description>
    </field>
  </entity>

  <entity name="PanicInfo">
    <field name="message" type="String" required="true"/>
    <field name="backtrace" type="String" required="true"/>
    <field name="captured_at" type="i64" required="true"/>
    <field name="last_in_flight_shift" type="Option<ShiftId>" required="false"/>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-SHIFT-01" category="performance">
    <description>Per-shift end-to-end latency p50 &lt; 5 s (cached instrument path).</description>
    <metric>p50 on FSV stress test of 1,000 shifts mixing 70% warm + 30% cold cache &lt; 5 s.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-02" category="performance">
    <description>Per-shift end-to-end latency p99 &lt; 30 s (cold instrument path).</description>
    <metric>p99 on the same FSV stress test &lt; 30 s.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-03" category="performance">
    <description>`mejepa_predict_latest` MCP latency p50 &lt; 50 ms.</description>
    <metric>1,000 calls to `mejepa_predict_latest`; p50 &lt; 50 ms.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-04" category="performance">
    <description>`mejepa_subscriber_status` MCP latency p50 &lt; 50 ms.</description>
    <metric>1,000 calls; p50 &lt; 50 ms (in-memory atomics + one CF read).</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-05" category="performance">
    <description>`mejepa_capture_audit` MCP latency p50 &lt; 100 ms.</description>
    <metric>1,000 calls on populated attempt_ids; p50 &lt; 100 ms.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-06" category="reliability">
    <description>Restart-safe at-least-once delivery: every shift consumed by the producer is eventually persisted to `CF_MEJEPA_LIVE_PREDICTIONS` (and `CF_MEJEPA_ORACLE_VERDICTS` when applicable) at least once across any sequence of subscriber crashes.</description>
    <metric>Integration test runs 10 random-time SIGKILLs across 1,000 shifts; asserts 1,000 final rows in `CF_MEJEPA_LIVE_PREDICTIONS`; rows that were re-processed (≥ 1 SIGKILL between materialize and persist) are byte-equal to the final state.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-07" category="reliability">
    <description>Backpressure tolerance: producer can write at 100 shifts/sec sustained; subscriber consumes at the rate the per-shift budget allows; lag accumulates without correctness loss.</description>
    <metric>Producer-side write 1,000 shifts at 100/sec; subscriber consumes within 100 s; final `CF_MEJEPA_LIVE_PREDICTIONS` row count == 1,000.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-08" category="resource">
    <description>Subscriber RSS ≤ 512 MB excluding instrument cache; instrument cache RAM ≤ 4 GB; one dedicated CPU.</description>
    <metric>Sustained 24 h test with mixed workload; RSS sample p99 ≤ 512 MB; cache size p99 ≤ 4 GB.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-09" category="security">
    <description>Path canonicalization at every MCP tool boundary; reject `../`, control characters, symlinks pointing outside repo root per SEC-MEJEPA-01.</description>
    <metric>Unit test attempts 10 path-traversal payloads; all rejected with `MEJEPA_INFER_SOURCE_SHA_DRIFT`.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-SHIFT-10" category="auditability">
    <description>FSV evidence at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/{watermark.json, observe-evidence.json, predict-latency-evidence.json, mcp-tool-roundtrip-evidence.json}` is produced on every release-build CI run.</description>
    <metric>FSV harness writes all four files; mode 0o600 per SEC-MEJEPA-07; each file passes its own re-read-and-assert block.</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>SPEC-INSTRUMENTS / TECH-INSTRUMENTS</name>
    <description>Phase 1 panel substrate. The subscriber materializes panels via the Instrument trait per (file_sha, slot_id).</description>
    <impact>If unavailable, the subscriber cannot materialize panels; predict()/observe() paths cannot run; the Phase 7 DoD verification fails.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-AST-CHUNKING / TECH-AST-CHUNKING</name>
    <description>11-language AST chunker invoked per cache miss to produce atomic chunks for code-domain instruments.</description>
    <impact>If unavailable for the source's language, the subscriber surfaces `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` and proceeds with partial panel per BR-SHIFT-06.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-EMBEDDERS / TECH-EMBEDDERS</name>
    <description>21-embedder ensemble executed in parallel per shift per doc 12 §7.</description>
    <impact>If embedder weights cannot load (digest mismatch per SEC-MEJEPA-02), the subscriber refuses to materialize the affected slot; surfaces `MEJEPA_INFER_INSTRUMENT_MISSING`.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-PREDICTOR / TECH-PREDICTOR</name>
    <description>The predict() body that the subscriber wires its persistent path to.</description>
    <impact>If unavailable, `mejepa_predict_latest` returns empty; the subscriber falls back to persisting only oracle outcomes.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-INFERENCE / TECH-INFERENCE</name>
    <description>The verify() seven-gate body. This spec wires the MCP tool; SPEC-INFERENCE owns the verdict ladder.</description>
    <impact>Must be wired up for `mejepa_verify` to function; this spec does not duplicate the gate logic.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-SELF-HEALING / TECH-SELF-HEALING</name>
    <description>The observe() body. This spec wires the call site; SPEC-SELF-HEALING owns the gradient + EWC++ machinery.</description>
    <impact>If observe() is unavailable, the subscriber drops the observe path with `MEJEPA_SHIFT_OBSERVE_DROPPED`-style informational; persistence still runs per BR-SHIFT-04.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-TRAINING / TECH-TRAINING</name>
    <description>Per-step training cert format and writer. The subscriber writes one cert per shift (whether observed or dropped).</description>
    <impact>Required for the per-step cert chain consulted by SPEC-INFERENCE REQ-INFER-08 (ΔΩ × ΔΞ multiplicative gating).</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-WITNESS</name>
    <description>73-byte SHAKE-256 chain shipped in `context-graph-witness`. The subscriber consumes existing entries (replay-only) for E_Witness slot encoding and Gate 5 of `verify()`.</description>
    <impact>If chain integrity fails, observe() is suppressed with `MEJEPA_OBSERVE_INTEGRITY_VIOLATION`.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-UTML</name>
    <description>L_step computation; the &lt; 0.05 sampling threshold gates the observe() call per BR-SHIFT-03.</description>
    <impact>Required for the L_step gate; if L_step computation cannot run (e.g., training certs unavailable), observe() runs unconditionally with degraded_status flagged.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-HYGIENE</name>
    <description>Owns the LRU eviction policy and tier-compression policy for the watermark + live-prediction CFs.</description>
    <impact>If unavailable, the subscriber's cache and CFs grow unboundedly; eventually triggers `MEJEPA_RETRAIN_QUOTA_EXCEEDED` per `<failure_taxonomy>`.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-CORPUS</name>
    <description>The `corpus_entry_added: 1` increment per Phase 7 DoD goes through SPEC-CORPUS's writer.</description>
    <impact>Required for the Phase 7 DoD verification command to report `total_corpus_size: previous + 1`.</impact>
  </dependency>

  <dependency type="external">
    <name>cgreality-shift-log JSONL stream</name>
    <description>Existing producer at `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session_id>.jsonl` written by the PostToolUse Edit hook. Schema in `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs::ShiftRecord`.</description>
    <impact>Read-only authority; the subscriber consumes the schema verbatim. If the producer schema changes, the subscriber surfaces `MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL` rather than silently mis-interpreting.</impact>
  </dependency>

  <dependency type="external">
    <name>RocksDB column families</name>
    <description>`CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS` per CLAUDE.md §11.13 artifact layout.</description>
    <impact>Required for restart safety, drift detection rolling window, and the per-step cert chain.</impact>
  </dependency>

  <dependency type="external">
    <name>tokio runtime</name>
    <description>Tokio 1.x async runtime per constitution `<tech_stack>`.</description>
    <impact>Subscriber is a Tokio task; runtime crash is a process-level failure.</impact>
  </dependency>

</dependencies>

<out_of_scope>

  <item>Predictor architecture, forward-pass body, aux-head bodies — covered by SPEC-PREDICTOR.</item>
  <item>The seven-gate `verify()` body (OOD detection, conformal calibration, Granger F-test, witness replay, SHA pin, Gτ guard) — covered by SPEC-INFERENCE.</item>
  <item>Observe() gradient computation, EWC++ Fisher information, A/B/C retraining, drift detection thresholds — covered by SPEC-SELF-HEALING.</item>
  <item>UTML L_step factor decomposition (ΔP/ΔK/ΔΩ/ΔΞ formulas) — covered by SPEC-UTML.</item>
  <item>Per-step training cert format (Merkle root, factor table, sampling_weight schema) — covered by SPEC-TRAINING.</item>
  <item>21-embedder ensemble training, distillation, LoRA — covered by SPEC-EMBEDDERS.</item>
  <item>11-language AST chunker boundaries (function/class/impl/module-decl detection) — covered by SPEC-AST-CHUNKING.</item>
  <item>Tier-compression policy on cold panels and watermark CF — covered by SPEC-HYGIENE.</item>
  <item>TCT constellation construction, Gτ test threshold, per-cell Bayesian shrinkage — covered by SPEC-TCT.</item>
  <item>Phase 0 mutation corpus generation, Phase 1 instrument shipping list — covered by SPEC-CORPUS / SPEC-INSTRUMENTS.</item>
  <item>The PostToolUse Edit hook implementation itself (the producer side of the shift log) — read-only authority per `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`; the subscriber must not modify the producer.</item>
  <item>Reintroducing retired tools (`reality_run_attempt`, etc.) — locked by constitution `<anti_patterns>`.</item>

</out_of_scope>

<test_plan>

  <test_case id="TC-SHIFT-01" type="unit" req_ref="REQ-SHIFT-01">
    <description>Subscriber Tokio task spawns and returns the expected `Result<(), SubscriberError>` signature.</description>
    <preconditions>RocksDB instance with all required CFs created; empty shift log.</preconditions>
    <inputs>Empty JSONL.</inputs>
    <expected_result>Task awaits indefinitely on tail-poll; no error; no watermark advance.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-02" type="integration" req_ref="REQ-SHIFT-01, REQ-SHIFT-02">
    <description>End-to-end consumption of 100 shifts with watermark advancement.</description>
    <preconditions>JSONL with 100 well-formed `ShiftRecord` entries, mixed prediction-only and oracle-attached.</preconditions>
    <inputs>The 100-shift JSONL.</inputs>
    <expected_result>All 100 consumed in order; final watermark equals shift 100's `shift_id`; `CF_MEJEPA_LIVE_PREDICTIONS` has 100 rows; `CF_MEJEPA_ORACLE_VERDICTS` has the expected oracle-attached count; FSV writes `watermark.json`.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-03" type="integration" req_ref="REQ-SHIFT-02">
    <description>SIGKILL mid-shift; subscriber respawns; in-flight shift re-processed exactly once.</description>
    <preconditions>1,000-shift JSONL.</preconditions>
    <inputs>Random SIGKILL after shift 500's panel materialize but before persist.</inputs>
    <expected_result>Final `CF_MEJEPA_LIVE_PREDICTIONS` has 1,000 rows; the shift-500 row is byte-equal to the post-recovery state.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-04" type="integration" req_ref="REQ-SHIFT-03">
    <description>Lag alert fires within 70 s of crossing 500 unprocessed shifts.</description>
    <preconditions>Producer driven at 100 shifts/sec; subscriber throttled to 10 shifts/sec.</preconditions>
    <inputs>Continuous producer-side write.</inputs>
    <expected_result>`mejepa_subscriber_status.lag_alert_active` flips to true within 70 s of lag &gt; 500; consumption never stops.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-05" type="unit" req_ref="REQ-SHIFT-01">
    <description>Malformed JSONL line rejected with structured error.</description>
    <preconditions>JSONL with a truncated line at byte offset 1234.</preconditions>
    <inputs>The JSONL.</inputs>
    <expected_result>`MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL` with `byte_offset: 1234` and a sanitized line preview; subscriber advances past the malformed line.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-06" type="integration" req_ref="REQ-SHIFT-04, REQ-SHIFT-08">
    <description>Instrument cache hit rate ≥ 50% on 100-shift mix of fresh + repeat SHAs.</description>
    <preconditions>50 unique file_shas, 100 shifts (50 fresh + 50 repeat).</preconditions>
    <inputs>The shift sequence.</inputs>
    <expected_result>Cache hit rate ≥ 50%; repeat-SHA wall time &lt; 5 s p50; FSV `predict-latency-evidence.json` populated.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-07" type="integration" req_ref="REQ-SHIFT-06, REQ-SHIFT-09">
    <description>L_step gate suppresses observe() but never persistence.</description>
    <preconditions>1,000 oracle-attached shifts with controlled L_step distribution (50% above 0.05, 50% below).</preconditions>
    <inputs>The 1,000-shift sequence.</inputs>
    <expected_result>500 observe() invocations recorded; 500 `MEJEPA_SHIFT_OBSERVE_DROPPED` informational entries; `CF_MEJEPA_LIVE_PREDICTIONS` has 1,000 rows; `CF_MEJEPA_ORACLE_VERDICTS` has 1,000 rows.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-08" type="integration" req_ref="REQ-SHIFT-07">
    <description>Five new MCP tools register, respond, and reject `additionalProperties`.</description>
    <preconditions>MCP server up; subscriber alive.</preconditions>
    <inputs>One round-trip per tool; one round-trip with an unknown extra param.</inputs>
    <expected_result>Each known tool returns the documented response; the unknown-param call returns the JSON-Schema rejection error.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-09" type="integration" req_ref="REQ-SHIFT-13">
    <description>Retired `reality_run_attempt` returns `CCREALITY_ENGINE_RETIRED`.</description>
    <preconditions>MCP server up.</preconditions>
    <inputs>`mcp__cgreality__reality_run_attempt {}` JSON-RPC call.</inputs>
    <expected_result>Error response with `code: "CCREALITY_ENGINE_RETIRED"`; FSV captures the round-trip.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-10" type="e2e" req_ref="REQ-SHIFT-15">
    <description>Phase 7 DoD verification command.</description>
    <preconditions>Phase 0-4 weights loaded; SWE-bench Lite `astropy__astropy-12907` task; candidate patch on disk; subscriber alive.</preconditions>
    <inputs>`cargo run --release -p context-graph-mejepa -- verify --task astropy__astropy-12907 --patch ./candidate.patch`.</inputs>
    <expected_result>`panel_materialization &lt; 1 s`; `me_jepa_verify_verdict` populated; `me_jepa_predicts_match_oracle: true`; `corpus_entry_added: 1`; `total_corpus_size: previous + 1`; FSV evidence at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/`.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-11" type="integration" req_ref="REQ-SHIFT-11">
    <description>`mejepa_observe_shift` is idempotent on repeat calls.</description>
    <preconditions>One consumed shift with persisted prediction.</preconditions>
    <inputs>Ten concurrent invocations of `mejepa_observe_shift { shift_id }`.</inputs>
    <expected_result>One row in `CF_MEJEPA_LIVE_PREDICTIONS` for that `(session_id, witness_chain_offset)`; ten `replay: true` per-step certs; watermark unchanged.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-12" type="integration" req_ref="REQ-SHIFT-12">
    <description>`mejepa_subscriber_status` reports task-dead state honestly.</description>
    <preconditions>Subscriber alive.</preconditions>
    <inputs>Inject a panic via test hook; query status.</inputs>
    <expected_result>`task_alive_since: None`; `last_panic: Some(_)`.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-13" type="integration" req_ref="REQ-SHIFT-10">
    <description>`mejepa_capture_audit` returns complete bundle for known attempt; nearest neighbors for unknown.</description>
    <preconditions>50 attempts persisted across multiple sessions.</preconditions>
    <inputs>Two calls: one known `attempt_id`, one fabricated unknown.</inputs>
    <expected_result>Known: full bundle; every documented field populated and finite; missing slots surfaced explicitly. Unknown: `attempt_found: false`, `nearest_neighbors` of length 5.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-SHIFT-14" type="performance" req_ref="REQ-SHIFT-16">
    <description>Latency budget verification on 1,000-shift FSV stress test.</description>
    <preconditions>Mixed 70% warm + 30% cold cache.</preconditions>
    <inputs>1,000 shifts.</inputs>
    <expected_result>p50 &lt; 5 s; p99 &lt; 30 s; `mejepa_predict_latest` p50 &lt; 50 ms; `mejepa_subscriber_status` p50 &lt; 50 ms; `mejepa_capture_audit` p50 &lt; 100 ms.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-SHIFT-15" type="integration" req_ref="REQ-SHIFT-08, NFR-SHIFT-08">
    <description>Sustained 24 h test for RAM budget.</description>
    <preconditions>Mixed workload; 100,000 shifts over 24 h.</preconditions>
    <inputs>The workload.</inputs>
    <expected_result>RSS sample p99 ≤ 512 MB; cache size p99 ≤ 4 GB; no `MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW`.</expected_result>
    <priority>high</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-SHIFT-01" status="open" assignee="Phase 7 implementation team">
    <text>Should the subscriber crate be a new dedicated crate `context-graph-mejepa-shift-subscriber` (per `<directory_structure>`) or live inside `context-graph-mcp/src/handlers/tools/mejepa/`?</text>
    <context>The `<directory_structure>` listed `context-graph-mejepa-shift-subscriber/` (new crate). Doc 09 §3.8.1 said "Tokio task in context-graph-mcp". Both are consistent if the new crate is a workspace member that the MCP server pulls in. Confirm with operator on initial PR.</context>
    <resolution>(pending)</resolution>
  </question>

  <question id="Q-SHIFT-02" status="open" assignee="Phase 5/7 implementation teams">
    <text>Does the L_step gate apply per-shift or per-attempt (multi-shift batches)?</text>
    <context>Doc 11 §10 specifies L_step at the curriculum-item level. A single attempt may produce multiple shifts (edit, then test, then re-edit). The cleanest interpretation is per-shift, which this spec adopts. Confirm with SPEC-SELF-HEALING author when that spec is written.</context>
    <resolution>(pending — current spec is per-shift)</resolution>
  </question>

  <question id="Q-SHIFT-03" status="open" assignee="Phase 7 implementation team">
    <text>How does `mejepa_capture_audit` chunk responses larger than 1 MB without breaking MCP-spec clients?</text>
    <context>MCP supports streaming responses but several existing tools return single JSON-RPC messages. If audit bundles routinely exceed 1 MB, we need either streaming or a paginated "first call returns metadata + page tokens" pattern. Defer to Phase 7 implementation and surface in TECH-SHIFT-SUBSCRIBER.</context>
    <resolution>(pending)</resolution>
  </question>

  <question id="Q-SHIFT-04" status="open" assignee="Operator">
    <text>Is `max_concurrent_shifts: 1` the right default? EWC++ regularization assumes ordered observe() calls per doc 11 §11, but predict-only paths could parallelize.</text>
    <context>Sequential observe() preserves Fisher information correctness. Parallel predict-only paths could double throughput on prediction-heavy workloads. Initial implementation: serialize everything for simplicity; revisit after Phase 7 holdout evaluation.</context>
    <resolution>(pending — default = 1 in v1.0)</resolution>
  </question>

  <question id="Q-SHIFT-05" status="open" assignee="Phase 6 implementation team">
    <text>What's the tier-compression policy for `CF_MEJEPA_SHIFT_WATERMARK`?</text>
    <context>Watermarks are tiny (~64 bytes per session); even 100,000 sessions × 64 bytes = 6.4 MB. Probably no tier-compression needed; just last-N-sessions retention. Defer to SPEC-HYGIENE.</context>
    <resolution>(pending — recommend retain-all in v1.0)</resolution>
  </question>

</open_questions>

<glossary>

  <term name="ShiftRecord">JSONL entry produced by the PostToolUse Edit hook capturing one Edit/Write tool call's before/after state.</term>
  <term name="ShiftEntry">In-memory representation of a `ShiftRecord` consumed by the subscriber.</term>
  <term name="Watermark">The `(session_id, last_consumed_shift_id, byte_offset)` tuple persisted to `CF_MEJEPA_SHIFT_WATERMARK` to enable restart-safe resumption.</term>
  <term name="Subscriber lag">Number of unprocessed shifts in the JSONL stream beyond the watermark.</term>
  <term name="Instrument cache">LRU map keyed by `(file_sha, slot_id, code_version)` storing materialized panel slot vectors.</term>
  <term name="Panel-at-t=1">Post-edit panel materialized from `after_sha`.</term>
  <term name="Panel-at-t=2">Panel materialized after the test run; includes populated `E_Oracle` slot.</term>
  <term name="L_step">UTML v1.0 four-factor product ΔP × ΔK × ΔΩ × ΔΞ; `≥ 0.05` gates whether the subscriber calls observe().</term>
  <term name="Capture audit bundle">Complete signal capture for an attempt: every panel slot, every aux-head prediction, every counterfactual perturbation, returned by `mejepa_capture_audit`.</term>
  <term name="MCP namespace lock">Constitutional rule that all ME-JEPA MCP tools are prefixed `mcp__cgreality__mejepa_*` and the retired surface stays retired.</term>
  <term name="Restart-safe at-least-once delivery">The structural correctness invariant: every shift the producer wrote is eventually persisted to `CF_MEJEPA_LIVE_PREDICTIONS` exactly once across any sequence of subscriber crashes (REQ-SHIFT-02 + REQ-SHIFT-11 idempotent overwrite).</term>

</glossary>

</functional_spec>
```

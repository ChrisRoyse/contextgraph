## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory.

**Confirmed 2026-05-11:**
- `ConformalSet` SHIPPED in `crates/context-graph-mejepa/src/conformal.rs`; `OodDetector` SHIPPED in `ood.rs`.
- This task is INDEPENDENT of TASK-EVAL-004 — it works against per-prediction `ConformalSet` outputs from any source.

**Decided scope for this task:**
1. Add `EvalRunner::enqueue_uncertain(&mut self, predictions: &[(PredictionId, ConformalSet, f32)]) -> Result<usize, EvalError>` to `crates/context-graph-mejepa/src/eval/active_learning.rs` (new file). Returns the count of predictions enqueued.
2. Filter rule: enqueue iff `conformal_set.outcome_set.len() > 3 AND ood_score < 0.5` (per REQ-EVAL-05). The threshold values are LOCKED constitutionally.
3. `signal_value = conformal_set.outcome_set.len() as f32 * (1.0 - ood_score)` — ranks "high uncertainty + in-distribution" highest.
4. Enqueue into `ActiveLearningQueueState` (BinaryHeap ordered by `signal_value` descending). Capacity-eviction: if `queue.len() > config.queue_capacity`, evict lowest-signal entry; persist eviction record to `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS`.
5. Persist new entries to `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` keyed by `panel_sha`.
6. `apply_active_learning_label(&mut self, panel_sha: [u8; 32], label: ActiveLearningLabel) -> Result<(), EvalError>`: pop entry from queue, append label to `CF_MEJEPA_ACTIVE_LEARNING_LABELS`, increment corpus growth counter for the weekly report.
7. **OOD escalation routing:** when `ood_score >= 0.5`, route to `CF_MEJEPA_OOD_ESCALATIONS` (NOT the active learning queue) — per BR-EVAL-03 OOD-flagged predictions need adjacent-test re-run not human label.
8. Target: ~1-2 % corpus growth per active-learning cycle (REQ-EVAL-06).
9. NEVER `unwrap_or` on `panel_sha` existence checks.

**FSV for this task:** integration test (a) feeds 1,000 synthetic predictions with KNOWN signal-value distribution → asserts top-K queue contents match expected by `signal_value`; (b) overflows the queue → asserts oldest-lowest evicted into `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS`; (c) applies a label → asserts `CF_MEJEPA_ACTIVE_LEARNING_LABELS` row exists AND the queue is shorter by 1; (d) routes OOD-flagged into the OOD CF not the queue. Source of Truth: separate prefix scans on each CF after each scenario.

---

# TASK-EVAL-009 — Active learning loop (`enqueue_uncertain` + `apply_active_learning_label` + OOD escalation routing)

```xml
<task_spec id="TASK-EVAL-009" version="1.0">

<metadata>
  <title>Implement active-learning loop in `crates/context-graph-mejepa/src/eval/active_learning.rs` — enqueue_uncertain + apply_active_learning_label + OOD escalation routing + capacity eviction</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-EVAL-05">ActiveLearningQueue + signal_value heap + dedup + capacity eviction + 3 label methods</item>
    <item ref="REQ-EVAL-06">Active-learning corpus growth ≥ 1 % target (queue depth + labels_added_this_week roll-up)</item>
    <item ref="component:EvalRunner.enqueue_uncertain">outcome_set &gt; 3 AND ood_score &lt; 0.5 filter; signal_value computation</item>
    <item ref="component:EvalRunner.apply_active_learning_label">durable label persistence + corpus row append + queue removal</item>
    <item ref="BR-EVAL-03">OOD-uncertain panels routed to CF_MEJEPA_OOD_ESCALATIONS rather than active-learning queue</item>
    <item ref="EC-EVAL-04">Capacity overflow → evict lowest-signal_value entry to CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-002</task_ref><!-- ActiveLearningQueueState, ActiveLearningQueueEntry, ActiveLearningLabel, QueueError -->
    <task_ref>TASK-EVAL-003</task_ref><!-- CF_MEJEPA_ACTIVE_LEARNING_QUEUE/LABELS/EVICTIONS, CF_MEJEPA_OOD_ESCALATIONS, OodEscalation, serializers -->
    <task_ref>TASK-INFER-005</task_ref><!-- ConformalSet { outcome_set } -->
    <task_ref>TASK-INFER-006</task_ref><!-- OodDetector::ood_score -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (active_learning.rs); 1 modified (queue.rs to add mutating methods); 1 modified (runner.rs)</estimated_files>
</metadata>

<context>
This task lands the **active learning loop**: live predictions arrive via Phase 7 SHIFT-SUBSCRIBER (or a holdout panel during eval); for each, decide one of three routes:

1. **enqueue_uncertain**: `outcome_set.len() &gt; 3` AND `ood_score &lt; 0.5` → write to CF_MEJEPA_ACTIVE_LEARNING_QUEUE with computed `signal_value = (outcome_set.len() / max_observed_outcome_set_len) × (1 - ood_score)`. Operator labels via TASK-EVAL-014 MCP tool.
2. **OOD escalation**: `ood_score ≥ 0.5` → write to CF_MEJEPA_OOD_ESCALATIONS (rejected from queue per BR-EVAL-03; OOD panels are NOT good active-learning candidates because the model already knows it doesn't know).
3. **drop**: `outcome_set.len() ≤ 3` → not uncertain enough; do nothing (return `Err(NotUncertain)`).

`apply_active_learning_label` is the operator's labeling endpoint: takes (panel_sha, oracle_outcome, label_method, operator_id), writes a durable `ActiveLearningLabel` row to CF_MEJEPA_ACTIVE_LEARNING_LABELS, appends a corpus row through SPEC-CORPUS, removes the panel from the queue. Capacity eviction (queue full at 10 K) pops the lowest-signal_value entry to CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS for audit per EC-EVAL-04. Cross-cutting boundary: TASK-EVAL-002 declared the queue state struct shape (read-only `top_n` / `len` / `contains`); this task adds the **mutating** methods (`enqueue` / `remove` / `evict_lowest` / `load_from_storage`) since they need RocksDB store access.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_EVAL_OOD_PANEL_REJECTED)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-05; REQ-EVAL-06; AC-EVAL-04-* active-learning scenarios; BR-EVAL-03; EC-EVAL-04 eviction)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (enqueue_uncertain 9-step behavior; apply_active_learning_label 5-step behavior; OOD routing; capacity eviction via secondary heap-of-min)</file>
  <file purpose="prior_task">TASK-EVAL-002, TASK-EVAL-003</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-002 + 003 shipped: queue state struct, RocksDB CFs, serializers</check>
  <check>TASK-INFER-005 shipped: `ConformalSet { stated_alpha, outcome_set: Vec&lt;OracleOutcome&gt; }`</check>
  <check>TASK-INFER-006 shipped: `OodDetector::ood_score(&Panel) -&gt; f32`</check>
  <check>SPEC-CORPUS exposes a corpus appender callable from this crate (TASK-CORPUS-007/021 OracleStore::put_provenance + put_generated)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/active_learning.rs` with:
      - `pub fn enqueue_uncertain(queue: &mut ActiveLearningQueueState, panel: &Panel, conformal_set: &ConformalSet, ood_score: f32, storage: &RocksDbStore) -> Result<(), QueueError>` per TECH-EVALUATION 9-step behavior:
        1. **Validate finite**: if `!ood_score.is_finite()` or `outcome_set.len()` overflow → `Err(QueueError::NanDetected{field: "ood_score"})`
        2. **OOD filter**: `if ood_score &gt;= 0.5` → write `OodEscalation { panel_sha, ood_score, language, reason: "MEJEPA_EVAL_OOD_PANEL_REJECTED", escalated_at }` to CF_MEJEPA_OOD_ESCALATIONS via `serialize_ood_escalation` + `ood_escalation_key`; return `Err(QueueError::OodPanelRejected { ood_score })`
        3. **Uncertainty filter**: `if conformal_set.outcome_set.len() &lt;= 3` → `Err(QueueError::NotUncertain { outcome_set_size: conformal_set.outcome_set.len() })`
        4. **Dedup**: `if queue.contains(&panel.sha)` → `Err(QueueError::DuplicatePanel { panel_sha })`
        5. **Update max**: `queue.max_observed_outcome_set_len = max(queue.max_observed_outcome_set_len, conformal_set.outcome_set.len())`
        6. **Compute signal_value**: `let signal = (conformal_set.outcome_set.len() as f32 / queue.max_observed_outcome_set_len as f32) * (1.0 - ood_score); signal.clamp(0.0, 1.0)`
        7. **Construct entry**: `ActiveLearningQueueEntry { panel_sha: panel.sha, prediction_id, conformal_set_size: conformal_set.outcome_set.len(), ood_score, signal_value: signal, enqueued_at: now_unix(), language: panel.language, suggested_label_method: heuristic_label_method(panel) }`
        8. **Capacity check**: if `queue.heap.len() &gt;= queue.capacity`, pop lowest-signal_value via secondary `BinaryHeap` of `Reverse<(signal_value, panel_sha)>` (or alternatively scan + remove min for the rare-eviction case); write evicted to CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS; remove from `entries_by_panel_sha`
        9. **Push**: `queue.heap.push(entry.clone()); queue.entries_by_panel_sha.insert(panel.sha, entry.clone())`; persist to CF_MEJEPA_ACTIVE_LEARNING_QUEUE keyed by `active_learning_queue_key(&panel.sha)`
      - `pub fn apply_active_learning_label(queue: &mut ActiveLearningQueueState, panel_sha: &[u8; 32], oracle_outcome: &OracleOutcome, label_method: LabelMethod, operator_id: &str, provenance: LabelProvenance, storage: &RocksDbStore, corpus_appender: &dyn CorpusAppender) -> Result<Ulid, EvalError>` per TECH-EVALUATION 5-step behavior:
        1. **Validate**: panel_sha exists in queue → else `Err(EvalError::PanelNotInQueue { panel_sha })`; operator_id non-empty + no control characters; oracle_outcome well-formed
        2. **Construct label**: `let ulid = Ulid::new(); let label = ActiveLearningLabel { panel_sha, oracle_outcome, label_method, operator_id, labelled_at: now_unix(), provenance }`
        3. **Persist label**: write to CF_MEJEPA_ACTIVE_LEARNING_LABELS keyed by `active_learning_label_key(&ulid)` via `serialize_label`
        4. **Append corpus row**: call `corpus_appender.append_label_as_corpus_row(&label)` (cross-cutting with SPEC-CORPUS); errors map to `EvalError::ReportPersistFail { cf: "corpus", backend_error }`
        5. **Remove from queue**: rebuild queue heap by filtering out the labelled panel; remove from `entries_by_panel_sha`; delete CF_MEJEPA_ACTIVE_LEARNING_QUEUE row
        6. Return `Ok(ulid)`
      - `pub fn load_active_learning_queue(storage: &RocksDbStore, capacity: usize) -> Result<ActiveLearningQueueState, EvalError>`: prefix-scan CF_MEJEPA_ACTIVE_LEARNING_QUEUE; deserialize each row via `deserialize_queue_entry`; rebuild heap + entries_by_panel_sha; recompute `max_observed_outcome_set_len = entries.iter().map(|e| e.conformal_set_size).max().unwrap_or(4)`
      - `pub fn auto_route_top_n_per_day(queue: &mut ActiveLearningQueueState, top_n: usize, label_method: LabelMethod, storage: &RocksDbStore) -> Result<usize, EvalError>` (optional auto-route knob from EvalConfig.auto_route_docker_rerun): selects top-N entries; for each, queues a Docker re-run job (out-of-band) — for this task the implementation writes a CF_MEJEPA_OOD_ESCALATIONS-style job stub and returns the count; actual re-run integration is in TASK-EVAL-016 CLI / future SPEC-CORPUS extension.
      - `fn heuristic_label_method(panel: &Panel) -> LabelMethod`: simple heuristic; prefer `LabelMethod::AdjacentTestRerun` when panel has E_Test slot populated with neighboring tests; `DockerRerun` when panel has clear oracle gold available; else `Manual`.
    - Update `crates/context-graph-mejepa/src/eval/queue.rs`:
      - Move the in-memory `enqueue` / `remove` mutating helpers here (private `pub(crate)` since they need queue internals)
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire `EvalRunner::enqueue_uncertain` and `apply_active_learning_label` as `&mut self` methods that delegate into `active_learning::*`
      - Wire `EvalRunner::new` to call `load_active_learning_queue` and store into `self.active_learning_queue: Arc<Mutex<...>>`
    - Define `pub trait CorpusAppender: Send + Sync { fn append_label_as_corpus_row(&self, label: &ActiveLearningLabel) -> Result<(), CorpusAppendError>; fn current_count(&self) -> Result<u64, CorpusAppendError>; }` — interface SPEC-CORPUS implements (TASK-CORPUS-007/021 OracleStore implements this trait; this task only declares the trait + a TestCorpusAppender for tests)
    - Add unit + integration tests in `active_learning.rs` `#[cfg(test)] mod tests` for:
      - `enqueue_when_outcome_set_4_and_ood_low_succeeds` (covers AC-EVAL-04-01)
      - `enqueue_with_ood_high_routes_to_escalation` (ood_score=0.7 → CF_MEJEPA_OOD_ESCALATIONS row + Err(OodPanelRejected))
      - `enqueue_with_outcome_set_2_returns_not_uncertain`
      - `enqueue_duplicate_panel_returns_duplicate_error`
      - `enqueue_at_capacity_evicts_lowest_signal` (capacity=3, push 4 → evicted row in CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS)
      - `signal_value_correct_formula` ((4/4) × (1-0.2) = 0.8)
      - `signal_value_clamped_to_unit` (degenerate ood_score=-0.1 → signal clamps to 1.0)
      - `apply_label_persists_durably_and_removes_from_queue` (covers AC-EVAL-04-02)
      - `apply_label_panel_not_in_queue_returns_error`
      - `load_active_learning_queue_recovers_max_observed_outcome_set_len`
      - `auto_route_top_n_returns_count_and_writes_jobs`
      - `nan_ood_score_returns_nan_detected`
  </in_scope>
  <out_of_scope>
    - The MCP tool wrappers `mejepa_active_learning_queue` + `mejepa_active_learning_label` (TASK-EVAL-014)
    - Active-learning rollup into ActiveLearningSummary (TASK-EVAL-012)
    - SPEC-CORPUS's actual `append_label_as_corpus_row` body (this task declares the trait; SPEC-CORPUS implements)
    - Bayesian shrinkage (TASK-EVAL-012)
    - check_ship_gate consumption (TASK-EVAL-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/active_learning.rs">
use ulid::Ulid;
use crate::types::{Language, OracleOutcome};
use super::error::{EvalError, QueueError};
use super::queue::{ActiveLearningQueueEntry, ActiveLearningQueueState, ActiveLearningLabel, LabelProvenance};
use super::storage::RocksDbStore;
use super::types::LabelMethod;

pub trait CorpusAppender: Send + Sync {
    fn append_label_as_corpus_row(&amp;self, label: &amp;ActiveLearningLabel) -&gt; Result&lt;(), String&gt;;
    fn current_count(&amp;self) -&gt; Result&lt;u64, String&gt;;
}

pub fn enqueue_uncertain(
    queue: &amp;mut ActiveLearningQueueState,
    panel: &amp;Panel,
    conformal_set: &amp;ConformalSet,
    ood_score: f32,
    storage: &amp;RocksDbStore,
) -&gt; Result&lt;(), QueueError&gt;;

pub fn apply_active_learning_label(
    queue: &amp;mut ActiveLearningQueueState,
    panel_sha: &amp;[u8; 32],
    oracle_outcome: &amp;OracleOutcome,
    label_method: LabelMethod,
    operator_id: &amp;str,
    provenance: LabelProvenance,
    storage: &amp;RocksDbStore,
    corpus_appender: &amp;dyn CorpusAppender,
) -&gt; Result&lt;Ulid, EvalError&gt;;

pub fn load_active_learning_queue(
    storage: &amp;RocksDbStore,
    capacity: usize,
) -&gt; Result&lt;ActiveLearningQueueState, EvalError&gt;;

pub fn auto_route_top_n_per_day(
    queue: &amp;mut ActiveLearningQueueState,
    top_n: usize,
    label_method: LabelMethod,
    storage: &amp;RocksDbStore,
) -&gt; Result&lt;usize, EvalError&gt;;

fn heuristic_label_method(panel: &amp;Panel) -&gt; LabelMethod;
    </signature>
  </signatures>
  <constraints>
    - `signal_value` clamped to `[0, 1]` after multiplication to absorb floating-point edge cases
    - Capacity eviction MUST be O(N) acceptable for the rare-eviction case (capacity=10K, eviction triggered &lt; 1× per week per benchmark) — secondary min-heap is overkill; linear scan + swap-remove is fine
    - All `Err` returns wrapped with `.log_context("eval::active_learning")` per `&lt;error_handling&gt;`
    - `apply_active_learning_label` is **transactional**: if corpus append fails, the label row in CF_MEJEPA_ACTIVE_LEARNING_LABELS MUST be rolled back (delete by ULID key); if rollback also fails, log `MEJEPA_EVAL_REPORT_PERSIST_FAIL` blocker but do NOT panic
    - `operator_id` validation: reject empty + reject any byte `&lt; 0x20` (control char) per SEC-MEJEPA-01
    - `now_unix()` MUST be deterministic-injectable for tests (use `std::time::SystemTime` in production; `MockClock` in tests)
    - File length budget: &lt; 500 lines
    - Rust 2021 edition; Rust 1.84+
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 12 passing tests covering all 3 routes (enqueue / OOD escalation / drop) + label apply + load + auto-route
    - signal_value formula verified to 3 decimal places
    - Capacity eviction writes to CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS
    - Transactional rollback verified by injecting a corpus-appender failure
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/active_learning.rs:
  pub fn enqueue_uncertain(queue, panel, conformal_set, ood_score, storage) -&gt; Result&lt;(), QueueError&gt; {
      if !ood_score.is_finite() {
          return Err(QueueError::NanDetected { field: "ood_score".into() });
      }
      if ood_score &gt;= 0.5 {
          let esc = OodEscalation { /* fields */ };
          let bytes = serialize_ood_escalation(&esc).map_err(|e| QueueError::NanDetected { field: format!("serialize: {:?}", e) })?;
          let cf = storage.cf_handle(CF_MEJEPA_OOD_ESCALATIONS)?;
          storage.put_cf(&cf, &ood_escalation_key(&panel.sha), &bytes)?;
          return Err(QueueError::OodPanelRejected { ood_score });
      }
      if conformal_set.outcome_set.len() &lt;= 3 {
          return Err(QueueError::NotUncertain { outcome_set_size: conformal_set.outcome_set.len() });
      }
      if queue.contains(&panel.sha) {
          return Err(QueueError::DuplicatePanel { panel_sha: panel.sha });
      }
      let outcome_set_size = conformal_set.outcome_set.len();
      queue.max_observed_outcome_set_len = queue.max_observed_outcome_set_len.max(outcome_set_size);
      let signal = ((outcome_set_size as f32 / queue.max_observed_outcome_set_len as f32)
                    * (1.0 - ood_score)).clamp(0.0, 1.0);
      let entry = ActiveLearningQueueEntry { /* fields */ signal_value: signal };

      // capacity eviction
      if queue.heap.len() &gt;= queue.capacity {
          let lowest = queue.heap.iter().min_by(|a, b| a.signal_value.partial_cmp(&b.signal_value).unwrap_or(Ordering::Equal)).cloned();
          if let Some(evicted) = lowest {
              queue.heap.retain(|e| e.panel_sha != evicted.panel_sha);
              queue.entries_by_panel_sha.remove(&evicted.panel_sha);
              storage.put_cf(/* CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, eviction_key, serialize_queue_entry(&evicted) */)?;
              storage.delete_cf(&cf_queue, &active_learning_queue_key(&evicted.panel_sha))?;
          }
      }

      let bytes = serialize_queue_entry(&entry).map_err(|e| QueueError::NanDetected { field: format!("serialize: {:?}", e) })?;
      storage.put_cf(&cf_queue, &active_learning_queue_key(&panel.sha), &bytes)?;
      queue.heap.push(entry.clone());
      queue.entries_by_panel_sha.insert(panel.sha, entry);
      Ok(())
  }

  pub fn apply_active_learning_label(...) -&gt; Result&lt;Ulid, EvalError&gt; {
      if !queue.contains(panel_sha) {
          return Err(EvalError::PanelNotInQueue { panel_sha: *panel_sha });
      }
      // validate operator_id
      if operator_id.is_empty() || operator_id.bytes().any(|b| b &lt; 0x20) {
          return Err(EvalError::MissingProvenance { field: "operator_id".into() });
      }
      let ulid = Ulid::new();
      let label = ActiveLearningLabel { panel_sha: *panel_sha, oracle_outcome: oracle_outcome.clone(), label_method, operator_id: operator_id.into(), labelled_at: now_unix(), provenance };
      // persist label
      let bytes = serialize_label(&label)?;
      storage.put_cf(&cf_labels, &active_learning_label_key(&ulid), &bytes)?;
      // append corpus
      if let Err(e) = corpus_appender.append_label_as_corpus_row(&label) {
          // rollback
          let _ = storage.delete_cf(&cf_labels, &active_learning_label_key(&ulid));
          return Err(EvalError::ReportPersistFail { cf: "corpus".into(), backend_error: e });
      }
      // remove from queue
      queue.heap.retain(|e| &e.panel_sha != panel_sha);
      queue.entries_by_panel_sha.remove(panel_sha);
      storage.delete_cf(&cf_queue, &active_learning_queue_key(panel_sha))?;
      Ok(ulid)
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/active_learning.rs">enqueue_uncertain + apply_active_learning_label + load_active_learning_queue + auto_route_top_n_per_day + CorpusAppender trait + heuristic_label_method + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/queue.rs">add `pub(crate)` mutating helpers; expose `contains`, `len`, `iter` if not already</file>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire `EvalRunner::enqueue_uncertain` + `apply_active_learning_label` as &mut self methods; wire load_active_learning_queue in ::new</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod active_learning;` + re-export `CorpusAppender` trait</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>enqueue_uncertain succeeds when outcome_set &gt; 3 AND ood_score &lt; 0.5</criterion>
  <criterion>OOD-uncertain panels routed to CF_MEJEPA_OOD_ESCALATIONS + return Err(OodPanelRejected)</criterion>
  <criterion>outcome_set ≤ 3 returns Err(NotUncertain)</criterion>
  <criterion>Duplicate panel returns Err(DuplicatePanel)</criterion>
  <criterion>Capacity eviction writes to CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS and removes lowest-signal entry</criterion>
  <criterion>signal_value formula (outcome_set/max) × (1-ood) verified</criterion>
  <criterion>apply_active_learning_label persists label, appends corpus, removes from queue (3-way atomic)</criterion>
  <criterion>Transactional rollback on corpus-append failure verified</criterion>
  <criterion>load_active_learning_queue rebuilds heap + max_observed_outcome_set_len from CF</criterion>
  <criterion>operator_id validation rejects empty + control chars</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::active_learning</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

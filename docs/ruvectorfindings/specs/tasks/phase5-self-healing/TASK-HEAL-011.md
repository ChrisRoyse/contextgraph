# TASK-HEAL-011 — schedule_full_retrain (every 10,000 obs; +0.02 floor; immutable corpus snapshot)

```xml
<task_spec id="TASK-HEAL-011" version="1.0">

<metadata>
  <title>Periodic full-retrain scheduler in `crates/context-graph-mejepa/src/heal/full_retrain.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-HEAL-05">schedule_full_retrain every N=10,000 observations regardless of drift; +0.02 floor</item>
    <item ref="BR-HEAL-04">Witness-chain entry on every weight promotion (delegates to TASK-HEAL-009 retrain_and_promote)</item>
    <item ref="EC-HEAL-07">10,000-obs interval lands DURING an in-flight A/B/C from drift; deferred to next interval</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/full_retrain.rs); 1 modified (src/heal/mod.rs)</estimated_files>
</metadata>

<context>
Logic 6 of 10. Implements `SelfHealingPipeline::full_retrain_if_due()` — the periodic-retrain
scheduler that fires every 10,000 observations independent of drift to absorb slow distribution
shifts that don't trigger drift detection. The body delegates the heavy lifting to TASK-HEAL-009
`retrain_and_promote(TriggerReason::PeriodicFullRetrain)` but applies a STRICTER floor: the new
weights only promote iff `winner_score > current_score + 0.02` (vs A/B/C's `>= A's score`). If a
promotion is already in flight from drift, this scheduler defers to the NEXT 10,000-obs interval
without retrying mid-cycle (EC-HEAL-07).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_RETRAIN_*; `<performance_budgets>` &lt;60min on RTX 5090)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-05; AC-HEAL-06-01..03; EC-HEAL-07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts SelfHealingPipeline::full_retrain_if_due 4-step body; PromotionGate.full_retrain_floor=0.02)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError, HealReport, MeJepaHealConfig.full_retrain_period field)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_MUTATION_CORPUS snapshot reader)</file>
  <file purpose="prior_task">TASK-HEAL-009 (retrain_and_promote orchestrator + PromotionGate + try_acquire_promotion_lock)</file>
  <file purpose="cross_domain_train">crates/context-graph-mejepa-train (Phase 3 train_from_scratch entry point)</file>
  <file purpose="cross_domain_corpus">crates/context-graph-mejepa-corpus (CF_MEJEPA_MUTATION_CORPUS snapshot keyed by corpus_sha + frozen_at)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-004..005 + TASK-HEAL-009 complete</check>
  <check>HealStatus.observation_counter is incremented atomically on every successful observe()</check>
  <check>MeJepaHealConfig.full_retrain_period (default 10_000) and full_retrain_promotion_floor (default 0.02) fields exist (TASK-HEAL-004)</check>
  <check>PromotionGate.full_retrain_floor field exists (TASK-HEAL-003)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/full_retrain.rs`:
      - `pub fn full_retrain_if_due(pipeline: &mut SelfHealingPipeline) -> Result<Option<HealReport>, HealError>` per component_contracts (4-step body):
        1. Read `observation_counter` from `pipeline.status.lock().unwrap().observation_counter`
        2. If `counter % full_retrain_period == 0 AND counter > 0`:
           a. Check promotion lock: if held (concurrent A/B/C in flight), log defer-to-next-interval at `info!` level + return `Ok(None)` per EC-HEAL-07; do NOT block waiting
           b. Snapshot CF_MEJEPA_MUTATION_CORPUS immutably at the current corpus_sha (frozen_at=now_secs)
           c. Call `pipeline.abc_promoter.retrain_and_promote(TriggerReason::PeriodicFullRetrain, ...)` with a STRICTER PromotionGate: `gate.oracle_agreement_floor = current_score + 0.02` (override the default `>= A's score`)
           d. If retrain_and_promote returns Err(HoldoutRegression): persist informational HealReport with mode_winner=A_unchanged_no_floor_clearance + return Ok(None) (NO error escalation; the +0.02 floor was not cleared, A retained, no blocker)
           e. Persist successful HealReport to CF_MEJEPA_HEAL_REPORTS
           f. Return `Ok(Some(HealReport))` if promotion happened; `Ok(None)` otherwise
      - Helper: `pub fn override_promotion_gate_with_full_retrain_floor(base_gate: &PromotionGate, floor: f32) -> PromotionGate` — produces a PromotionGate with `oracle_agreement_floor = mode_a.oracle_agreement + floor` (delegated to retrain_and_promote)
      - Helper: `pub fn snapshot_corpus_for_full_retrain(storage: &RocksDbStore, corpus_sha_now: &[u8; 32]) -> Result<CorpusSnapshot, HealError>` — wraps Phase 0 snapshot logic; ensures frozen_at + sha provenance metadata captured
    - Tests:
      - `full_retrain_if_due_returns_none_when_counter_below_period`
      - `full_retrain_if_due_fires_at_exactly_10000_observations`
      - `full_retrain_if_due_fires_at_20000_30000_etc` (multiple intervals)
      - `full_retrain_if_due_returns_none_when_counter_zero` (do not fire at startup)
      - `full_retrain_if_due_defers_when_promotion_lock_held` (EC-HEAL-07; logs info + returns Ok(None); does NOT retry mid-cycle)
      - `full_retrain_if_due_uses_stricter_floor` (winner_score must exceed A by 0.02; verified via mock retrain_and_promote that returns score = A + 0.01 → no winner; A retained)
      - `full_retrain_if_due_returns_ok_none_on_holdout_regression` (NOT escalated to Err; this is informational)
      - `full_retrain_if_due_persists_a_unchanged_no_floor_clearance_on_no_winner`
      - `full_retrain_if_due_promotes_when_score_exceeds_floor`
      - `full_retrain_if_due_appends_modelpromote_with_trigger_periodic`
      - `full_retrain_if_due_propagates_quota_exceeded_as_err`
      - `full_retrain_if_due_propagates_batch_nan_as_err` (mid-Mode-B NaN bubbles up)
      - `snapshot_corpus_for_full_retrain_freezes_at_corpus_sha`
      - End-to-end with synthetic 30,000-obs run: verify exactly 3 retrain attempts at 10k/20k/30k boundaries
    - FSV evidence emission for TASK-HEAL-019: `FullRetrainEvidence { observation_counter_at_fire, mode_winner, score_a, score_winner, score_delta, promotion_floor, retrain_triggered, deferred_due_to_lock }`
  </in_scope>
  <out_of_scope>
    - retrain_and_promote body (TASK-HEAL-009)
    - rollback_to (TASK-HEAL-010)
    - Continuous calibration (TASK-HEAL-012)
    - Phase 3 train_from_scratch internals
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/full_retrain.rs">
use std::sync::{Arc, Mutex, RwLock};
use crate::heal::{pipeline::SelfHealingPipeline, promote::AbcPromoter, errors::HealError,
                  status::TriggerReason, report::HealReport};
use crate::heal::data_models::{PromotionGate, CorpusSnapshot};

pub fn full_retrain_if_due(pipeline: &amp;mut SelfHealingPipeline) -&gt; Result&lt;Option&lt;HealReport&gt;, HealError&gt;;

pub fn override_promotion_gate_with_full_retrain_floor(
    base_gate: &amp;PromotionGate,
    floor: f32,
) -&gt; PromotionGate;

pub fn snapshot_corpus_for_full_retrain(
    storage: &amp;RocksDbStore,
    corpus_sha_now: &amp;[u8; 32],
) -&gt; Result&lt;CorpusSnapshot, HealError&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullRetrainEvidence {
    pub observation_counter_at_fire: u64,
    pub mode_winner: ModeWinner,
    pub score_a: f32,
    pub score_winner: f32,
    pub score_delta: f32,
    pub promotion_floor: f32,
    pub retrain_triggered: bool,
    pub deferred_due_to_lock: bool,
}
    </signature>
  </signatures>
  <constraints>
    - The +0.02 floor is ADDITIVE on top of A's oracle_agreement (winner must exceed A by AT LEAST 0.02; winner == A + 0.015 fails)
    - Lock-held defer is NEVER an Err — it's a logged Ok(None) per EC-HEAL-07; the next 10,000-obs boundary will re-attempt
    - HoldoutRegression at the +0.02 floor is INFORMATIONAL (NOT escalated to Err) — A is retained; no operator blocker; HealReport persisted with mode_winner=A_unchanged_no_floor_clearance
    - Real escalations (BatchNan, QuotaExceeded, FisherRankDeficient, IntegrityViolation) DO bubble up as Err
    - Counter check uses `counter % period == 0 AND counter > 0` (do NOT fire at counter=0; fire at 10,000 / 20,000 / ...)
    - File &lt; 500 lines
    - tracing::info! before defer-to-next-interval; tracing::error! before every Err propagation
    - No mock data in tests; real RocksDB tempdir + real corpus snapshot fixture
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::full_retrain` shows ≥ 14 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Counter at 9,999 returns Ok(None); counter at 10,000 fires retrain
    - Lock held → Ok(None) + info-level log "deferred to next interval"
    - Score = A + 0.01 → Ok(None) + HealReport mode_winner=A_unchanged_no_floor_clearance
    - Score = A + 0.05 → Ok(Some(HealReport)) with mode_winner ∈ {B, C}
    - 30,000-obs synthetic run fires exactly 3 retrain attempts (at 10k/20k/30k)
    - WitnessChain ModelPromote entry has trigger_reason=PeriodicFullRetrain
  </verification>
</definition_of_done>

<pseudo_code>
pub fn full_retrain_if_due(pipeline: &mut SelfHealingPipeline) -> Result<Option<HealReport>, HealError> {
    let counter = pipeline.status.lock().unwrap().observation_counter;
    if counter == 0 || counter % pipeline.config.full_retrain_period != 0 {
        return Ok(None);
    }
    // EC-HEAL-07: defer if A/B/C in flight
    {
        let lock = pipeline.promotion_lock.lock().unwrap();
        if lock.held_by.is_some() {
            tracing::info!(counter, "full_retrain deferred to next interval (promotion lock held by {:?})", lock.held_by);
            return Ok(None);
        }
    }
    let corpus_sha_now = read_active_corpus_sha(&pipeline.storage)?;
    let _snapshot = snapshot_corpus_for_full_retrain(&pipeline.storage, &corpus_sha_now)?;
    let stricter_gate = override_promotion_gate_with_full_retrain_floor(
        &pipeline.abc_promoter.gate,
        pipeline.config.full_retrain_promotion_floor, // default 0.02
    );
    let result = pipeline.abc_promoter.retrain_and_promote_with_gate(
        TriggerReason::PeriodicFullRetrain,
        pipeline.predictor.clone(),
        pipeline.storage.clone(),
        pipeline.witness_chain.clone(),
        pipeline.holdout(),
        pipeline.promotion_lock.clone(),
        &pipeline.calibration_version(),
        stricter_gate,
    );
    match result {
        Ok(report) => Ok(Some(report)),
        Err(HealError::HoldoutRegression { mode_a_score, mode_b_score, mode_c_score }) => {
            // INFORMATIONAL: +0.02 floor not cleared; A retained; not an error
            let report = HealReport::a_unchanged_no_floor_clearance(mode_a_score, mode_b_score, mode_c_score, TriggerReason::PeriodicFullRetrain)?;
            pipeline.storage.put(CF_MEJEPA_HEAL_REPORTS, &encode_heal_report_key(now_secs()), &bincode::serialize(&report)?)?;
            tracing::info!(counter, score_a = mode_a_score.oracle_agreement, "full_retrain: +0.02 floor not cleared; A retained");
            Ok(None)
        }
        Err(other) => {
            tracing::error!(error_code = ?other.code(), "full_retrain_if_due propagating Err");
            Err(other)
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/full_retrain.rs">full_retrain_if_due + override_promotion_gate_with_full_retrain_floor + snapshot_corpus_for_full_retrain + FullRetrainEvidence + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod full_retrain;`; re-export full_retrain_if_due</file>
  <file path="crates/context-graph-mejepa/src/heal/promote.rs">expose retrain_and_promote_with_gate(...) variant that accepts a custom PromotionGate (or thread the override into existing retrain_and_promote)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::full_retrain shows ≥ 14 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Counter % 10,000 == 0 AND counter > 0 → retrain attempt fires</criterion>
  <criterion>Counter at 0 → no fire (do not retrain at startup)</criterion>
  <criterion>Promotion lock held → Ok(None) + info log; no retry mid-cycle (EC-HEAL-07)</criterion>
  <criterion>Score &lt; A + 0.02 → Ok(None) + A_unchanged_no_floor_clearance HealReport persisted</criterion>
  <criterion>Score &gt;= A + 0.02 → Ok(Some(HealReport)) with mode_winner ∈ {B, C}</criterion>
  <criterion>30,000-obs synthetic run triggers exactly 3 retrain attempts</criterion>
  <criterion>BatchNan / QuotaExceeded / FisherRankDeficient propagate as Err (NOT swallowed)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::full_retrain</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

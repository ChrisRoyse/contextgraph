<task_spec id="TASK-SHIFT-007" version="1.0">
<metadata>
  <title>Logic — ShiftSubscriber::process_shift core panel-materialize → predict/observe routing</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-SHIFT-04">Per-shift panel materialization (15 slots × 5,120-d)</item>
    <item ref="REQ-SHIFT-05">Predict-only path persists to CF_MEJEPA_LIVE_PREDICTIONS</item>
    <item ref="REQ-SHIFT-06">Observe path on oracle-attached shifts (L_step gated)</item>
    <item ref="REQ-SHIFT-09">Persistence-vs-update separation (BR-SHIFT-04)</item>
    <item ref="REQ-SHIFT-18">Provenance metadata in every persisted record</item>
    <item ref="BR-SHIFT-02">Watermark stays put on persistence failure</item>
    <item ref="BR-SHIFT-06">Missing instrument → MEJEPA_INFER_INSTRUMENT_MISSING; never zero-pad</item>
    <item ref="EC-SHIFT-04">Repeat-SHA cache hits keep p50 under budget</item>
    <item ref="EC-SHIFT-05">L_step boundary 0.05 strictly ≥ triggers observe</item>
    <item ref="EC-SHIFT-06">SIGKILL between persist and watermark advance is recoverable</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[ShiftSubscriber][process_shift]">behaviour</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[ShiftSubscriber][materialize_slot]">behaviour</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-003</task_ref>
    <task_ref>TASK-SHIFT-004</task_ref>
    <task_ref>TASK-SHIFT-005</task_ref>
    <task_ref>TASK-SHIFT-006</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
The heart of the subscriber. Given a `ShiftEntry` with byte_offset populated, materialize panel-at-t=1 via 15 calls to `materialize_slot`, branch on whether an oracle outcome is attached, persist `CF_MEJEPA_LIVE_PREDICTIONS` (always), persist `CF_MEJEPA_ORACLE_VERDICTS` (when applicable), then route to either `predict()` only or `predict() + observe()`. Cross-cutting calls into `MeJepaCompiler::predict/observe` (SPEC-INFERENCE/SPEC-SELF-HEALING owned bodies) and `UtmlFactorEngine::compute_l_step` (SPEC-UTML owned body) are wired here through `Arc<...>` handles passed at construction. Watermark advance is the CALLER's responsibility (TASK-SHIFT-010); this task just returns Ok or Err.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[ShiftSubscriber]</file>
  <file purpose="sequence_diagram">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#process_shift_sequence_diagram</file>
  <file purpose="implementation_notes">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (concurrency, durability, ordering, idempotency)</file>
  <file purpose="prior_data_models">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
  <file purpose="cf_keys">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-004.md</file>
  <file purpose="cache_logic">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-006.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001..006 complete</check>
  <check>`context-graph-mejepa` crate exists with `MeJepaCompiler::predict` / `observe` (cross-cutting traits OR stubs that compile)</check>
  <check>Cross-cutting traits: `MeJepaCompiler`, `UtmlFactorEngine`, `TrainCertWriter`, `OracleVerdictWriter`, `WitnessChainReader` are imported via `Arc<dyn Trait>` so this crate doesn't take a hard dep on the implementing crates</check>
</prerequisites>

<scope>
  <in_scope>
    - Define cross-cutting traits in this crate as `Arc<dyn Trait + Send + Sync>` injection points (concrete impls live elsewhere):
      - `pub trait MeJepaPredictor: Send + Sync { fn predict(&self, panel: &[f32]) -> Result<RealityPrediction, _>; fn observe(&self, t1: &[f32], t2: &[f32], oracle: &OracleOutcome) -> Result<(), _>; }`
      - `pub trait UtmlFactorEngine: Send + Sync { fn compute_l_step(...) -> Result<UtmlFactorBundle, _>; }`
      - `pub trait TrainCertWriter: Send + Sync { fn write(&self, cert: TrainCert) -> Result<(), _>; }`
      - `pub trait OracleVerdictWriter: Send + Sync { fn append(&self, task_id: &str, mut_cat: &str, verdict: &OracleOutcome) -> Result<(), _>; }`
      - `pub trait WitnessChainReader: Send + Sync { fn replay_offset(&self, offset: u64) -> Result<[u8; 32], _>; }`
      - `pub trait InstrumentSlotEncoder: Send + Sync { fn encode_for(&self, file_sha: [u8; 32], slot: InstrumentSlot, after: &ShiftSide) -> Result<Vec<f32>, _>; }`
      - `pub trait LivePredictionWriter: Send + Sync { fn put(&self, key: [u8; 10], pred: &RealityPrediction) -> Result<(), _>; }`
    - Augment `ShiftSubscriber` struct with `Arc<dyn ...>` collaborators
    - Implement `process_shift(&mut self, shift: ShiftEntry) -> Result<ShiftOutcome, SubscriberError>`
    - Implement `materialize_slot(&self, file_sha: [u8; 32], slot: InstrumentSlot, shift: &ShiftEntry) -> Result<Arc<Vec<f32>>, SubscriberError>` using `tokio::task::spawn_blocking` for CPU-bound encode
    - 15-slot inner-shift parallelism via `futures::stream::iter(slots).buffer_unordered(15).try_collect()` per implementation_notes
    - Branch on whether `shift.subject` contains a `validation.json` delta (pure-Rust JSON inspector — does NOT shell out)
    - Persistence-before-watermark order strictly enforced (per Architecture invariant)
    - Tests use trait mocks (in-test-module impls of each trait) — NOT mock data; the traits themselves are real
  </in_scope>
  <out_of_scope>
    - Watermark advance (TASK-SHIFT-008 + TASK-SHIFT-010)
    - Lag alert state machine (TASK-SHIFT-009)
    - `run()` task entry point (TASK-SHIFT-010)
    - Latency profiling export (TASK-SHIFT-012)
    - Replay flag wiring for `mejepa_observe_shift` (TASK-MCP-004)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/traits.rs">
      pub trait MeJepaPredictor: Send + Sync { ... }
      pub trait UtmlFactorEngine: Send + Sync { ... }
      pub trait TrainCertWriter: Send + Sync { ... }
      pub trait OracleVerdictWriter: Send + Sync { ... }
      pub trait WitnessChainReader: Send + Sync { ... }
      pub trait InstrumentSlotEncoder: Send + Sync { ... }
      pub trait LivePredictionWriter: Send + Sync { ... }
      // Carrier types:
      pub struct TrainCert { pub step: u64, pub replay: bool, pub sampling_decision: SamplingDecision, pub factors: UtmlFactorBundle, pub provenance: ProvenanceBundle }
      pub enum SamplingDecision { Observed, DroppedLStepBelowThreshold, ReplayObserved, NumericalHealthDegraded }
      pub struct ProvenanceBundle { pub corpus_sha: [u8; 32], pub embedder_versions: HashMap&lt;String, String&gt;, pub frozen_at: i64, pub code_version: String }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs">
      use std::sync::Arc;
      use crate::cache::InstrumentCache;
      use crate::traits::{MeJepaPredictor, UtmlFactorEngine, TrainCertWriter, OracleVerdictWriter, WitnessChainReader, InstrumentSlotEncoder, LivePredictionWriter};

      pub struct ShiftSubscriber {
          pub(crate) config: MeJepaShiftSubscriberConfig,
          pub(crate) code_version: String,
          pub(crate) instrument_cache: Arc&lt;InstrumentCache&gt;,
          pub(crate) instrument_encoder: Arc&lt;dyn InstrumentSlotEncoder&gt;,
          pub(crate) predictor: Arc&lt;dyn MeJepaPredictor&gt;,
          pub(crate) utml_engine: Arc&lt;dyn UtmlFactorEngine&gt;,
          pub(crate) train_cert_writer: Arc&lt;dyn TrainCertWriter&gt;,
          pub(crate) oracle_writer: Arc&lt;dyn OracleVerdictWriter&gt;,
          pub(crate) live_prediction_writer: Arc&lt;dyn LivePredictionWriter&gt;,
          pub(crate) witness_chain: Arc&lt;dyn WitnessChainReader&gt;,
          pub(crate) metrics: Arc&lt;SubscriberMetrics&gt;,
          pub(crate) provenance: ProvenanceBundle,
      }

      impl ShiftSubscriber {
          pub async fn process_shift(
              &amp;mut self,
              shift: ShiftEntry,
          ) -&gt; Result&lt;ShiftOutcome, SubscriberError&gt;;

          async fn materialize_slot(
              &amp;self,
              file_sha: [u8; 32],
              slot: InstrumentSlot,
              shift: &amp;ShiftEntry,
          ) -&gt; Result&lt;Arc&lt;Vec&lt;f32&gt;&gt;, SubscriberError&gt;;

          fn detect_oracle_outcome(shift: &amp;ShiftEntry) -&gt; Option&lt;OracleOutcome&gt;;

          fn assemble_panel(slots: &amp;[Arc&lt;Vec&lt;f32&gt;&gt;], missing: &amp;[InstrumentSlot]) -&gt; Vec&lt;f32&gt;;

          fn panel_sha(panel: &amp;[f32]) -&gt; [u8; 32];
      }
    </signature>
  </signatures>

  <constraints>
    - 15 slots in `InstrumentSlot::iter_all()` order (defined by SPEC-INSTRUMENTS); slot ordering is structural, never ad-hoc
    - Slot materialization uses `tokio::task::spawn_blocking` for the actual encode (AST/embedder forward is CPU-bound)
    - Inner-shift parallelism: `futures::stream::iter(slots).buffer_unordered(15).try_collect()` — keeps 15 spawn_blocking calls in flight simultaneously
    - `assemble_panel` panics-free: missing slots → zero-length contribution but `missing` field surfaced in ShiftOutcome (NOT zero-padded values; the layout is documented in SPEC-PREDICTOR which decides whether to accept partial panels)
    - Persistence order strictly: predict() → live_prediction_writer.put() → (if oracle) oracle_writer.append() → (if oracle) utml_engine.compute_l_step() → (if observe path AND L_step ≥ threshold) predictor.observe() → train_cert_writer.write() → return Ok
    - L_step gate uses `&gt;=` (strict `≥ 0.05` per EC-SHIFT-05); 0.04999... drops, 0.05 observes
    - L_step NaN/Inf surfaces SubscriberError::ObserveBatchNan; live prediction STILL persists (BR-SHIFT-04); observe is skipped
    - On `live_prediction_writer.put()` failure, propagate up; caller leaves watermark un-advanced (BR-SHIFT-02)
    - On `oracle_writer.append()` failure, same (in-flight; respawn re-processes; idempotent overwrite per REQ-SHIFT-11)
    - Provenance bundle assembled at construction; embedded in every train cert
    - File length under 500 lines; split if needed (e.g., trait declarations into `traits.rs`, body into `process_shift.rs`)
    - Tests use trait mocks defined in `#[cfg(test)] mod`, not external mock data
  </constraints>

  <verification>
    - Test 1 (predict-only): shift with no oracle delta → live_prediction_writer.put called once, oracle_writer.append NOT called, predictor.observe NOT called, ShiftOutcome::PredictOnly
    - Test 2 (observe path, L_step ≥ 0.05): full path, predictor.observe called once, train_cert with sampling_decision == Observed
    - Test 3 (observe path, L_step &lt; 0.05): predictor.observe NOT called, train_cert with sampling_decision == DroppedLStepBelowThreshold; live_prediction + oracle still persisted
    - Test 4 (NaN L_step): ObserveBatchNan surfaced; live_prediction still persisted; observe skipped
    - Test 5 (cache hit): repeat shift on same after_sha → cache_hits == 15 (all slots warm); latency &lt; 100 ms in test
    - Test 6 (missing instrument): mock encoder fails for E_TypeGraph slot → ShiftOutcome carries `missing_slots: ["E_TypeGraph"]`; predict path proceeds
    - Test 7 (live_prediction_writer fail): mock writer returns Err → process_shift returns Err; watermark advance MUST be the caller's choice (test asserts the error type)
  </verification>
</definition_of_done>

<pseudo_code>
process_shift:
  let t0 = Instant::now();
  let after_sha = shift.after.sha;
  let file_sha = after_sha;
  // (1) Materialize 15 slots in parallel
  let slots: Vec<InstrumentSlot> = InstrumentSlot::iter_all().collect();
  let materialized = futures::stream::iter(slots.clone())
    .map(|slot| async {
       let res = self.materialize_slot(file_sha, slot, &shift).await;
       (slot, res)
    })
    .buffer_unordered(15)
    .collect::<Vec<_>>()
    .await;
  let mut hits = 0; let mut misses = 0; let mut missing_slots = Vec::new();
  let mut slot_vecs: Vec<Arc<Vec<f32>>> = Vec::with_capacity(15);
  for (slot, res) in materialized.into_iter().sorted_by_slot() {
    match res {
      Ok(v) => { slot_vecs.push(v); /* hits/misses tracked inside materialize_slot via metrics */ }
      Err(SubscriberError::InstrumentMissing(_)) => { missing_slots.push(slot); }
      Err(e) => return Err(e),
    }
  }
  let panel_t1 = Self::assemble_panel(&slot_vecs, &missing_slots);
  let panel_t1_sha = Self::panel_sha(&panel_t1);
  // (2) Predict (always)
  let prediction = self.predictor.predict(&panel_t1)?;
  let h6 = session_id_hash6(&shift.session_id);
  let key = live_prediction_key(h6, witness_offset_for(&shift));
  self.live_prediction_writer.put(key, &prediction)?;
  // (3) Branch on oracle outcome
  let oracle = Self::detect_oracle_outcome(&shift);
  let (path, panel_t2_sha, l_step_opt, observe_invoked) = match oracle {
    None => (ShiftPath::PredictOnly, None, None, false),
    Some(oracle_outcome) => {
      // Persist oracle verdict (always)
      let task_id = extract_task_id(&shift)?;
      let mut_cat = extract_mut_cat(&shift)?;
      self.oracle_writer.append(&task_id, &mut_cat, &oracle_outcome)?;
      // Build panel_t2 with E_Oracle populated
      let oracle_slot = self.materialize_slot(file_sha, InstrumentSlot::E_Oracle, &shift).await?;
      let mut t2_slot_vecs = slot_vecs.clone();
      // replace E_Oracle position with oracle_slot
      t2_slot_vecs[InstrumentSlot::E_Oracle.index()] = oracle_slot;
      let panel_t2 = Self::assemble_panel(&t2_slot_vecs, &missing_slots);
      let panel_t2_sha = Some(Self::panel_sha(&panel_t2));
      // L_step
      let utml = self.utml_engine.compute_l_step(&panel_t1, &panel_t2, &oracle_outcome)?;
      utml.assert_finite_and_clamped().map_err(|_| SubscriberError::ObserveBatchNan{ shift_id: shift.shift_id.clone(), factors: utml.clone() })?;
      // Observe gated on L_step
      let (observe_invoked, sampling_decision) = if utml.l_step >= self.config.l_step_observe_threshold {
        self.predictor.observe(&panel_t1, &panel_t2, &oracle_outcome)?;
        (true, SamplingDecision::Observed)
      } else {
        self.metrics.dropped_l_step_count.fetch_add(1, Ordering::Relaxed);
        (false, SamplingDecision::DroppedLStepBelowThreshold)
      };
      // Train cert (always)
      self.train_cert_writer.write(TrainCert{ step: ..., replay: false, sampling_decision, factors: utml.clone(), provenance: self.provenance.clone() })?;
      (ShiftPath::PredictAndObserve{ l_step: utml.l_step }, panel_t2_sha, Some(utml.l_step), observe_invoked)
    }
  };
  Ok(ShiftOutcome{ shift_id: shift.shift_id, path, panel_t1_sha, panel_t2_sha, prediction_persisted_at: now_unix_seconds(), l_step: l_step_opt, observe_invoked, cache_hits: hits, cache_misses: misses, latency_ms: t0.elapsed().as_millis() as u64, missing_slots: missing_slots.into_iter().map(|s| format!("{:?}", s)).collect() })
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/traits.rs">7 cross-cutting traits + carrier types</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/process.rs">process_shift + materialize_slot + helpers (split out of subscriber.rs to keep under 500 lines)</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/process_shift_drills.rs">7 path-coverage tests using trait mocks</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs">augment struct with Arc<dyn ...> collaborators</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod traits; pub mod process;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add `futures` workspace dep</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 7 process_shift drill tests pass</criterion>
  <criterion>Persistence-before-observe order verified by call-sequence assertion in mocks</criterion>
  <criterion>L_step boundary 0.05 strictly ≥ behaviour verified</criterion>
  <criterion>NaN L_step still persists prediction; skips observe</criterion>
  <criterion>Cache hit pattern: repeat shift on same after_sha materializes in &lt; 100 ms</criterion>
  <criterion>No file exceeds 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber process::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test process_shift_drills</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>

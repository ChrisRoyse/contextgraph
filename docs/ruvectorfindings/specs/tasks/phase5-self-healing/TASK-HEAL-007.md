# TASK-HEAL-007 — DriftDetector methods (push, detect_drift; rolling 1,000-attempt window; 3-window hysteresis)

```xml
<task_spec id="TASK-HEAL-007" version="1.0">

<metadata>
  <title>DriftDetector method bodies in `crates/context-graph-mejepa/src/heal/drift.rs` (extending TASK-HEAL-002 struct shapes)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-HEAL-03">DriftDetector::push + detect_drift; rolling 1000-window; severity table; persist to CF_MEJEPA_DRIFT_WINDOW + CF_MEJEPA_DRIFT_HISTORY</item>
    <item ref="REQ-HEAL-11">Drift events surface (journal + blocker + status_change)</item>
    <item ref="EC-HEAL-03">WarmupNotReady on window.len() &lt; 1000</item>
    <item ref="EC-HEAL-04">3-window hysteresis OR ≥2-band jump skips hysteresis</item>
    <item ref="ERR-HEAL-02">MEJEPA_OBSERVE_DRIFT_DETECTED</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-002</task_ref>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified (src/heal/drift.rs)</estimated_files>
</metadata>

<context>
Logic 2 of 10. Implements `push` (append + persist + evict-on-overflow) and `detect_drift`
(empirical-coverage computation + severity-table mapping + 3-window hysteresis OR ≥2-band-jump
skip + journal-entry emission + CF_MEJEPA_DRIFT_HISTORY persistence on transition + memory/blockers/
on Hard or Catastrophic per REQ-HEAL-11). Method bodies on the structs landed in TASK-HEAL-002.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_OBSERVE_DRIFT_DETECTED)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-03, REQ-HEAL-11; AC-HEAL-02-01..04; EC-HEAL-03/04)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts DriftDetector + drift_severity_diagram)</file>
  <file purpose="prior_task">TASK-HEAL-002 (struct shapes)</file>
  <file purpose="prior_task">TASK-HEAL-004 (StatusChange enum + journal/blocker emitter helpers)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_DRIFT_WINDOW + CF_MEJEPA_DRIFT_HISTORY codecs)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-002, 004, 005 complete</check>
  <check>SMAP `memory/journal/` and `memory/blockers/` directories exist (CLAUDE.md §6 bootstrap)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement on `DriftDetector`:
      - `pub fn push(&mut self, sample: DriftSample, store: &impl DriftStore) -> Result<Option<DriftSample>, HealError>` — append to ring buffer; persist via store; return evicted sample if any.
      - `pub fn detect_drift(&mut self, store: &impl DriftHistoryStore, surface: &impl DriftSurface) -> Result<DriftSeverity, HealError>` — compute empirical_coverage; map to raw severity; apply hysteresis; on transition: persist + journal + blocker.
      - `fn map_coverage_to_raw_severity(coverage: f32, table: &SeverityTable) -> DriftSeverity` (private) — pure mapping function (no side effects); coverage ∈ [healthy_min, healthy_max] → Healthy; ∈ [soft_min, healthy_min) → Soft; ∈ [hard_min, soft_min) → Hard; &lt; hard_min → Catastrophic.
      - `fn apply_hysteresis(&mut self, raw: DriftSeverity) -> DriftSeverity` (private) — if `last_severity` is None OR `band_index` differs by ≥ 2, accept immediately; else require `hysteresis_windows` consecutive identical raw severities before transitioning.
    - Define injected traits:
      - `pub trait DriftStore { fn put_drift_sample(&self, key: [u8;8], value: &[u8]) -> Result<(), HealError>; }`
      - `pub trait DriftHistoryStore { fn put_drift_history(&self, key: [u8;8], value: &[u8]) -> Result<(), HealError>; }`
      - `pub trait DriftSurface { fn emit_journal(&self, entry: &str) -> Result<(), HealError>; fn emit_blocker(&self, severity: BlockerSeverity, body: &str) -> Result<(), HealError>; }`
      - `pub enum BlockerSeverity { Low, Medium, High, Critical }`
    - Update tests:
      - `push_evicts_oldest_at_capacity_and_returns_evicted`
      - `push_persists_via_store_trait`
      - `detect_drift_returns_warmup_not_ready_below_thousand`
      - `detect_drift_healthy_at_coverage_zero_point_nine`
      - `detect_drift_soft_at_coverage_zero_point_eight_seven`
      - `detect_drift_hard_at_coverage_zero_point_eight_three`
      - `detect_drift_catastrophic_at_coverage_zero_point_seven_five`
      - `detect_drift_hysteresis_requires_three_consecutive_for_adjacent_band`
      - `detect_drift_skips_hysteresis_for_two_band_jump` (Healthy→Hard direct)
      - `detect_drift_persists_to_history_on_transition`
      - `detect_drift_emits_journal_on_any_transition`
      - `detect_drift_emits_blocker_severity_high_on_hard`
      - `detect_drift_emits_blocker_severity_critical_on_catastrophic`
      - Synthetic 70%-coverage drift test: push 1000 deterministic samples that mismatch 30% → assert DriftSeverity::Hard within 1000 samples (canonical AC-HEAL-02-03 invariant)
  </in_scope>
  <out_of_scope>
    - DriftStore / DriftHistoryStore / DriftSurface live impls (TASK-HEAL-008)
    - Drift-detection-triggered A/B/C spawn (TASK-HEAL-008 + TASK-HEAL-009)
    - status_change stamping on RealityPrediction (TASK-HEAL-008 + cross-cutting INFERENCE)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/drift.rs">
pub trait DriftStore {
    fn put_drift_sample(&amp;self, key: [u8; 8], value: &amp;[u8]) -&gt; Result&lt;(), HealError&gt;;
}

pub trait DriftHistoryStore {
    fn put_drift_history(&amp;self, key: [u8; 8], value: &amp;[u8]) -&gt; Result&lt;(), HealError&gt;;
}

pub trait DriftSurface {
    fn emit_journal(&amp;self, entry: &amp;str) -&gt; Result&lt;(), HealError&gt;;
    fn emit_blocker(&amp;self, severity: BlockerSeverity, body: &amp;str) -&gt; Result&lt;(), HealError&gt;;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockerSeverity { Low, Medium, High, Critical }

impl DriftDetector {
    pub fn push&lt;S: DriftStore&gt;(&amp;mut self, sample: DriftSample, store: &amp;S)
        -&gt; Result&lt;Option&lt;DriftSample&gt;, HealError&gt;;

    pub fn detect_drift&lt;H: DriftHistoryStore, F: DriftSurface&gt;(
        &amp;mut self, store: &amp;H, surface: &amp;F
    ) -&gt; Result&lt;DriftSeverity, HealError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - `detect_drift` returns WarmupNotReady when `window.len() &lt; 1000` — never errors out
    - Empirical coverage = `count(actual ∈ predicted_set) / 1000` (exact integer division)
    - Severity mapping is total: every coverage ∈ [0,1] maps to exactly one DriftSeverity
    - 3-window hysteresis ONLY for adjacent-band transitions; ≥2-band jumps skip per EC-HEAL-04
    - Journal entry has SMAP frontmatter (namespace=journal, agent_id, timestamp, status=active, tags) per REQ-HEAL-11
    - Blocker entry has SMAP frontmatter (namespace=blockers, severity=high|critical) per REQ-HEAL-11
    - No mock data; deterministic synthetic OracleOutcomeRef SHAs only
    - File &lt; 500 lines (split into drift_hysteresis.rs if needed)
    - No `unwrap_or` per constitution
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::drift` shows ≥ 30 tests (16 from TASK-HEAL-002 + ≥14 new)
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Synthetic 70%-coverage drift test asserts DriftSeverity::Hard reported within 1000 samples
    - Healthy → Hard direct transition skips 3-window hysteresis (EC-HEAL-04)
    - Adjacent-band Healthy → Soft requires 3 consecutive samples
  </verification>
</definition_of_done>

<pseudo_code>
fn map_coverage_to_raw_severity(coverage: f32, table: &SeverityTable) -> DriftSeverity {
    if coverage >= table.healthy_min && coverage <= table.healthy_max { DriftSeverity::Healthy }
    else if coverage >= table.soft_min && coverage < table.healthy_min { DriftSeverity::Soft }
    else if coverage >= table.hard_min && coverage < table.soft_min { DriftSeverity::Hard }
    else { DriftSeverity::Catastrophic }
}

fn apply_hysteresis(&mut self, raw: DriftSeverity) -> DriftSeverity {
    if raw == DriftSeverity::WarmupNotReady { return raw; }
    let prev = self.last_severity.unwrap_or(raw);
    let raw_band = raw.band_index().unwrap_or(0);
    let prev_band = prev.band_index().unwrap_or(0);
    let diff = (raw_band as i32 - prev_band as i32).unsigned_abs();
    if diff >= 2 || self.last_severity.is_none() {
        self.last_severity = Some(raw); self.consecutive_at_new_severity = 0; return raw;
    }
    if raw == prev {
        self.consecutive_at_new_severity = 0; return prev;
    }
    self.consecutive_at_new_severity += 1;
    if (self.consecutive_at_new_severity as usize) >= self.hysteresis_windows {
        self.last_severity = Some(raw); self.consecutive_at_new_severity = 0; raw
    } else {
        prev
    }
}
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/drift.rs">add method bodies + DriftStore/DriftHistoryStore/DriftSurface traits + tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::drift shows ≥ 30 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Synthetic 70% coverage drift test reports DriftSeverity::Hard within 1000 samples</criterion>
  <criterion>2-band-jump Healthy → Hard transition skips hysteresis</criterion>
  <criterion>Adjacent-band transitions require 3 consecutive identical raw severities</criterion>
  <criterion>Hard transition emits SMAP blocker severity:high; Catastrophic emits severity:critical</criterion>
  <criterion>Every transition emits a journal entry with SMAP frontmatter</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::drift</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

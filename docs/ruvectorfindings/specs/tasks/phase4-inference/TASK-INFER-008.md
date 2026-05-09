# TASK-INFER-008 — Calibrated confidence widening by ΔΩ_mean × ΔΞ_mean (degraded-state fail-closed)

```xml
<task_spec id="TASK-INFER-008" version="1.0">

<metadata>
  <title>ΔΩ × ΔΞ multiplicative gate in `crates/context-graph-mejepa/src/degraded.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-INFER-08">calibrated_confidence multiplied by ΔΩ_mean × ΔΞ_mean clamped [0.10, 0.95]; degraded_status when either &lt; 0.4</item>
    <item ref="BR-INFER-03">degraded_status fails-closed when training health is poor</item>
    <item ref="EC-INFER-09">bootstrap defaults when no training certs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-002</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (degraded.rs)</estimated_files>
</metadata>

<context>
Per doc 11 §12: `calibrated_confidence = raw_conf · conv_rate · strategy_agreement · evidence_factor
· ΔΩ_mean · ΔΞ_mean`, clamped to [0.10, 0.95]. For ME-JEPA-Code, the conformal interval is
multiplied by `ΔΩ_mean × ΔΞ_mean` to fail-closed when training health is poor. ΔΩ_mean and ΔΞ_mean
are read from CF_MEJEPA_TRAIN_CERTS over the last N=128 steps. When either < 0.4, `degraded_status
= true` and the conformal outcome_set widens proportionally.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants ΔΩ × ΔΞ rule; performance_budgets)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-08; BR-INFER-03; EC-INFER-09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts MeJepaCompiler::compile step 2; degraded_state_diagram)</file>
  <file purpose="utml_authority">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §12 (calibrated_confidence formula)</file>
  <file purpose="upstream_compiler">crates/context-graph-mejepa/src/compiler.rs (TrainCertSummary trait surface; TASK-INFER-003)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-002 complete; MeJepaInferConfig has bootstrap_delta_omega/_xi + train_cert_window_steps</check>
  <check>TASK-INFER-003 declares TrainCertSummary { delta_omega: f32, delta_xi: f32 } trait surface</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/degraded.rs` with:
      - `pub struct TrainHealthSummary { pub delta_omega_mean: f32, pub delta_xi_mean: f32, pub degraded_status: bool, pub source_count: usize, pub bootstrap_used: bool }`
      - `pub fn compute_train_health(certs: &[TrainCertSummary], bootstrap_omega: f32, bootstrap_xi: f32) -> TrainHealthSummary` — empty certs → bootstrap_used=true + degraded_status=true; non-empty → mean over window + degraded if ΔΩ < 0.4 OR ΔΞ < 0.4
      - `pub fn calibrated_confidence_multiplier(omega: f32, xi: f32) -> f32` — `(omega * xi).clamp(0.10, 0.95)`
      - `pub fn calibrated_confidence(raw_conf: f32, conv_rate: f32, strategy_agreement: f32, evidence_factor: f32, omega: f32, xi: f32) -> f32` — full doc 11 §12 formula clamped [0.10, 0.95]
      - `pub fn evidence_factor(num_paths: u32, threshold: u32) -> f32` — `0.5 + 0.5 * (num_paths as f32 / threshold as f32).min(1.0)`
    - Add unit tests:
      - `compute_train_health_empty_certs_uses_bootstrap_and_marks_degraded`: certs=[], bootstrap=0.5/0.5 → bootstrap_used=true, degraded_status=true (per EC-INFER-09 the bootstrap multiplier is 0.25 clamped to 0.10)
      - `compute_train_health_healthy_window`: 128 certs all (0.8, 0.8) → degraded_status=false
      - `compute_train_health_omega_below_threshold_marks_degraded`: 128 certs (0.3, 0.7) → degraded_status=true
      - `compute_train_health_xi_below_threshold_marks_degraded`: (0.7, 0.3) → degraded_status=true
      - `compute_train_health_boundary_at_04`: (0.4, 0.4) → degraded_status=false (strict `<` per BR-INFER-03)
      - `multiplier_clamps_below_010`: omega=0.1, xi=0.1 → 0.01 clamped to 0.10
      - `multiplier_clamps_above_095`: omega=1.0, xi=1.0 → 1.0 clamped to 0.95
      - `calibrated_confidence_full_formula`: synthetic inputs with known product → expected value (within 1e-6)
      - `evidence_factor_floor`: num_paths=0 → 0.5
      - `evidence_factor_ceil`: num_paths=2*threshold → 1.0
    - Re-export from lib.rs
  </in_scope>
  <out_of_scope>
    - CF_MEJEPA_TRAIN_CERTS read implementation — Phase 3 TRAINING owns the column family; this task consumes pre-deserialized `&[TrainCertSummary]` slices from `MejepaStore::read_recent_train_certs`
    - Degraded-state propagation into RealityPrediction.degraded_status — TASK-INFER-003 wires it in compile()
    - Dynamic widening of outcome_set — TASK-INFER-007 already accepts the multiplier
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/degraded.rs">
use crate::compiler::TrainCertSummary;

#[derive(Debug, Clone, Copy)]
pub struct TrainHealthSummary {
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
    pub degraded_status: bool,
    pub source_count: usize,
    pub bootstrap_used: bool,
}

pub const HEALTH_FLOOR: f32 = 0.4;
pub const MULTIPLIER_LO: f32 = 0.10;
pub const MULTIPLIER_HI: f32 = 0.95;

pub fn compute_train_health(certs: &amp;[TrainCertSummary], bootstrap_omega: f32, bootstrap_xi: f32)
    -&gt; TrainHealthSummary;

pub fn calibrated_confidence_multiplier(omega: f32, xi: f32) -&gt; f32;

pub fn calibrated_confidence(raw_conf: f32, conv_rate: f32, strategy_agreement: f32,
                             evidence_factor: f32, omega: f32, xi: f32) -&gt; f32;

pub fn evidence_factor(num_paths: u32, threshold: u32) -&gt; f32;
    </signature>
  </signatures>
  <constraints>
    - degraded_status uses STRICT `<` 0.4 (boundary 0.4 is healthy)
    - Bootstrap used only when `certs.is_empty()`; sets degraded_status=true regardless of bootstrap values
    - Multiplier clamped [0.10, 0.95] AFTER product (so omega*xi=0.05 clamps to 0.10)
    - calibrated_confidence final clamped [0.10, 0.95]
    - All inputs validated finite; NaN/Inf treated as 0.0 contribution? — NO; assert finite, panic in debug, log+saturate to 0.0 in release with warning (constitution: no silent fallback; degraded flag propagates the warning loudly)
    - evidence_factor: num_paths=0 → 0.5 floor; threshold=0 returns f32::NAN (caller must validate)
    - Determinism: pure functions, no RNG
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa degraded` shows ≥ 10 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - empty certs → bootstrap_used=true and degraded_status=true
    - boundary (0.4, 0.4) → degraded_status=false (strict)
    - boundary (0.39, 0.41) → degraded_status=true
    - multiplier clamp [0.10, 0.95] holds
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_train_health(certs, b_omega, b_xi) -> TrainHealthSummary {
    if certs.is_empty() {
        return TrainHealthSummary {
            delta_omega_mean: b_omega.clamp(0.0, 1.0),
            delta_xi_mean: b_xi.clamp(0.0, 1.0),
            degraded_status: true,
            source_count: 0,
            bootstrap_used: true,
        };
    }
    let n = certs.len() as f32;
    let omega: f32 = certs.iter().map(|c| c.delta_omega).sum::<f32>() / n;
    let xi:    f32 = certs.iter().map(|c| c.delta_xi).sum::<f32>() / n;
    TrainHealthSummary {
        delta_omega_mean: omega.clamp(0.0, 1.0),
        delta_xi_mean:    xi.clamp(0.0, 1.0),
        degraded_status:  omega < HEALTH_FLOOR || xi < HEALTH_FLOOR,
        source_count:     certs.len(),
        bootstrap_used:   false,
    }
}

pub fn calibrated_confidence_multiplier(omega: f32, xi: f32) -> f32 {
    (omega * xi).clamp(MULTIPLIER_LO, MULTIPLIER_HI)
}

pub fn calibrated_confidence(raw, conv, agree, ev, omega, xi) -> f32 {
    let v = raw * conv * agree * ev * omega * xi;
    v.clamp(MULTIPLIER_LO, MULTIPLIER_HI)
}

pub fn evidence_factor(num_paths: u32, threshold: u32) -> f32 {
    if threshold == 0 { return f32::NAN; }
    0.5 + 0.5 * (num_paths as f32 / threshold as f32).min(1.0)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/degraded.rs">TrainHealthSummary + compute_train_health + multiplier + calibrated_confidence + evidence_factor + 10 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod degraded;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa degraded shows ≥ 10 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>compute_train_health empty certs sets bootstrap_used + degraded_status</criterion>
  <criterion>boundary (0.4, 0.4) is healthy (strict &lt;)</criterion>
  <criterion>multiplier clamps to [0.10, 0.95]</criterion>
  <criterion>calibrated_confidence formula correct to 1e-6</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa degraded</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

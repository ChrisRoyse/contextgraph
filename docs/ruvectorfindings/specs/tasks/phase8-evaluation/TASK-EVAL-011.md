# TASK-EVAL-011 — Aux-head distillation summary + State-transfer T diagnostic (Wasserstein-1 + M(t))

```xml
<task_spec id="TASK-EVAL-011" version="1.0">

<metadata>
  <title>Implement aux-head distillation summary + state-transfer T diagnostic in `crates/context-graph-mejepa/src/eval/state_transfer.rs` + `aux_head.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-EVAL-09">State-transfer T diagnostic with Wasserstein-1 + M(t) geometric mean per doc 11 §8</item>
    <item ref="REQ-EVAL-12">Aux-head distillation summary (consumer side; reads CF_MEJEPA_TRAIN_CERTS distillation steps every 1,000 steps)</item>
    <item ref="component:EvalRunner.compute_state_transfer">d_train_deploy via Wasserstein-1; T = exp(-d / lambda); M(t) geometric mean from train certs ΔΩ × ΔΞ</item>
    <item ref="component:EvalRunner.compute_aux_head_distillation_summary">aux_heads_in_consensus, aux_heads_diverged via divergence_threshold</item>
    <item ref="EC-EVAL-05">When CF_MEJEPA_LIVE_PREDICTIONS &lt; 100, state_transfer_diagnostic = None</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-001</task_ref><!-- StateTransferDiagnostic, AuxHeadDistillationSummary -->
    <task_ref>TASK-EVAL-002</task_ref><!-- EvalError, EvalConfig -->
    <task_ref>TASK-EVAL-003</task_ref><!-- CF descriptors for borrowed CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_LIVE_PREDICTIONS -->
    <task_ref>TASK-TRAIN-***</task_ref><!-- CF_MEJEPA_TRAIN_CERTS populated by Phase 3 with per-step distillation_loss + ΔΩ + ΔΞ -->
    <task_ref>TASK-SHIFT-***</task_ref><!-- CF_MEJEPA_LIVE_PREDICTIONS populated by Phase 7 (optional; degrades to None when &lt; 100) -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 created (state_transfer.rs, aux_head.rs); 1 modified (runner.rs to wire steps 6 + 7)</estimated_files>
</metadata>

<context>
This task lands two related but logically distinct diagnostics:

**Aux-head distillation summary (REQ-EVAL-12)**: Phase 3 (TRAINING) writes per-step distillation telemetry into CF_MEJEPA_TRAIN_CERTS every 1,000 steps including (a) the distillation loss between auxiliary prediction heads and the main predictor, (b) which aux heads are in consensus (cosine ≥ 0.80 with main), (c) which diverged (cosine &lt; aux_head_divergence_threshold default 0.20). This task READS the latest 1,000-step rollup and surfaces it as `AuxHeadDistillationSummary` in `EvalReport`. No retraining decisions are made here — Phase 5 SELF-HEALING reads this summary read-only and may trigger Mode A retraining if too many heads diverged.

**State-transfer T diagnostic (REQ-EVAL-09; doc 11 §8)**: predicts how performance will degrade between training distribution and deployment distribution. `d_train_deploy = wasserstein_1(holdout_panel_embeddings, live_panel_embeddings)`; `T = exp(-d / lambda)` (default lambda=1.0); `M(t)` = geometric mean of (ΔΩ × ΔΞ) across the last `m_t_window_steps` train cert entries (default 1,000); `performance_deploy_predicted = M(t) × T`. When CF_MEJEPA_LIVE_PREDICTIONS has fewer than 100 entries (Phase 7 not yet shipped or just started), the diagnostic degrades to `None` per EC-EVAL-05; ship gate skips the corresponding condition (handled in TASK-EVAL-008 Q3 conditional).

Both diagnostics are READ-ONLY consumers of upstream RocksDB CFs; this task creates no new persistence.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_EVAL_INSUFFICIENT_LIVE_DATA; doc 11 §8 state-transfer formula)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-09 state-transfer; REQ-EVAL-12 aux-head; AC-EVAL-09-* state-transfer scenarios; EC-EVAL-05)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (compute_state_transfer 6-step behavior; compute_aux_head_distillation_summary 4-step behavior; Wasserstein-1 sampled implementation; M(t) geometric mean)</file>
  <file purpose="design_authority">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §8 (state-transfer T derivation)</file>
  <file purpose="prior_task">TASK-EVAL-001..004</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-001..003 shipped: data models, error, CFs declared</check>
  <check>TASK-EVAL-004 shipped: run_holdout_eval scaffold with holdout panel embeddings accessible</check>
  <check>TASK-TRAIN-*** shipped: CF_MEJEPA_TRAIN_CERTS populated with per-step `{step, distillation_loss, aux_head_cosines: BTreeMap&lt;String, f32&gt;, delta_omega: f32, delta_xi: f32}` records</check>
  <check>Phase 7 may or may not be shipped: when not, CF_MEJEPA_LIVE_PREDICTIONS is empty, both diagnostics for state-transfer return None gracefully</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/state_transfer.rs` with:
      - `pub fn compute_state_transfer_diagnostic(holdout_embeddings: &[[f32; 512]], live_embeddings: &[[f32; 512]], train_certs: &[TrainCert], config: &EvalConfig) -> Result<Option<StateTransferDiagnostic>, EvalError>` per TECH-EVALUATION 6-step behavior:
        1. **Live data check**: if `live_embeddings.len() &lt; 100` → `Ok(None)` and emit `EvalError::InsufficientLiveData{count: live_embeddings.len(), required: 100}.log_context("eval::state_transfer")`
        2. **Wasserstein-1 sampled**: compute `d = wasserstein_1_sampled(holdout_embeddings, live_embeddings, config.state_transfer_wasserstein_samples)` (default 5,000 samples). The sampled approximation is acceptable per TECH-EVALUATION because exact Wasserstein-1 is O(N³) and we have N up to 240. Use a deterministic SplitMix64 seed from `code_version` hash; reject NaN.
        3. **T value**: `T = (-d / config.state_transfer_lambda).exp()`; clamp `[0, 1]`; lambda must be `&gt; 0` (validated in EvalConfig).
        4. **M(t)**: filter `train_certs` to the last `config.m_t_window_steps` entries (default 1,000); compute `delta_omega_xi[i] = train_certs[i].delta_omega * train_certs[i].delta_xi`; geometric mean = `exp(mean(ln(delta_omega_xi)))`; reject any non-positive value (geometric mean undefined for ≤ 0); clamp result to `[0, 1]`.
        5. **Performance deploy predicted**: `M(t) × T`; clamp `[0, 1]`.
        6. **Construct + return**: `Some(StateTransferDiagnostic { d_train_deploy: d, lambda: config.state_transfer_lambda, t_value: T, m_t_estimate: M(t), performance_deploy_predicted: M(t) * T, attribution: format!("d={:.4} via Wasserstein-1 sampled (n_samples={}); T=exp(-{}/lambda); M(t) geometric mean over {} train certs", d, samples_used, d, m_t_window), wasserstein_samples_used: samples_used })`
      - `fn wasserstein_1_sampled(p: &[[f32; 512]], q: &[[f32; 512]], n_samples: usize) -> Result<f32, EvalError>`:
        - Deterministic SplitMix64 (seed = SHA-256(p.len().to_le_bytes() ++ q.len().to_le_bytes())[..8])
        - Sample `n_samples` indices from each distribution
        - Compute pairwise Euclidean distances of the samples; sort; sum diffs (1-D Wasserstein along the dominant principal component approximation — see doc 11 §8 for the closed-form approximation when full optimal-transport solver isn't available)
        - **Implementation note**: a faithful 512-d Wasserstein-1 requires Sinkhorn or simplex solver; for the v1 of this task we use the **sliced Wasserstein** approximation: project both distributions onto K=32 random unit directions; compute 1-D Wasserstein per direction (sort + L1 of sorted diffs); average. This is well-known per Bonneel 2015 and tractable on CPU.
        - Reject NaN; clamp `[0, ∞)`
    - Create `crates/context-graph-mejepa/src/eval/aux_head.rs` with:
      - `pub fn compute_aux_head_distillation_summary(train_certs: &[TrainCert], config: &EvalConfig) -> Result<Option<AuxHeadDistillationSummary>, EvalError>` per TECH-EVALUATION 4-step behavior:
        1. **Find latest distillation step**: filter `train_certs` to entries with `aux_head_cosines.is_some()`; pick the one with the largest `step`. If none → `Ok(None)`.
        2. **Bin aux heads**: for each `(head_name, cosine)` in the latest entry's `aux_head_cosines`:
           - if `cosine &gt;= 0.80` → `aux_heads_in_consensus.push(head_name)`
           - if `cosine &lt; config.aux_head_divergence_threshold` (default 0.20) → `aux_heads_diverged.push(head_name)`
           - else (between thresholds) → neither list (silent middle ground)
        3. **Sort both lists** alphabetically by head_name for deterministic output
        4. **Construct + return**: `Some(AuxHeadDistillationSummary { last_distillation_step: entry.step, distillation_loss: entry.distillation_loss, aux_heads_in_consensus, aux_heads_diverged })`
      - `pub fn load_train_certs_window(storage: &RocksDbStore, window_steps: usize) -> Result<Vec<TrainCert>, EvalError>`: prefix-scan CF_MEJEPA_TRAIN_CERTS in step-descending order; deserialize first `window_steps` entries; reverse to ascending; reject NaN/Inf in any field.
      - **TrainCert data model**: forward-declare a minimal value type readable from CF_MEJEPA_TRAIN_CERTS:
        ```rust
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct TrainCert {
            pub step: u64,
            pub distillation_loss: f32,
            pub delta_omega: f32,
            pub delta_xi: f32,
            pub aux_head_cosines: Option<BTreeMap<String, f32>>,
        }
        ```
        SPEC-TRAINING is the writer; this task only declares the read-side struct compatible with the bincode produced upstream. If SPEC-TRAINING ships a different shape, we adapt this struct (no backwards compat per memory/feedback_no_backwards_compat).
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 6 of `run_holdout_eval` to call `compute_state_transfer_diagnostic` and store in `EvalReportBuilder.state_transfer_diagnostic`
      - Wire step 7 to call `compute_aux_head_distillation_summary` and store in `EvalReportBuilder.aux_head_distillation`
      - Both wrapped with `.map_err(|e| { e.log_context("eval::runner.state_transfer"); e })?` (but state-transfer's None-when-&lt;100 case is `Ok(None)`, NOT error)
    - Add unit + integration tests in `state_transfer.rs` `#[cfg(test)] mod tests` for:
      - `state_transfer_returns_none_when_live_under_100`
      - `wasserstein_zero_when_distributions_identical` (P==Q → d ≈ 0; T ≈ 1.0)
      - `wasserstein_positive_when_distributions_differ` (synthetic shift)
      - `t_value_decay_with_increasing_d` (T monotone-decreasing)
      - `m_t_geometric_mean_correct` (synthetic train_certs, expect computed value within 1e-4)
      - `m_t_skips_window_when_no_certs`
      - `state_transfer_full_pipeline_returns_some` (synthetic 100 holdout + 100 live + 1000 certs)
      - `wasserstein_handles_nan_via_error`
      - `m_t_negative_delta_omega_returns_error` (geometric mean undefined for non-positive)
    - Add unit + integration tests in `aux_head.rs` `#[cfg(test)] mod tests` for:
      - `aux_head_summary_returns_none_when_no_distillation_certs`
      - `aux_head_summary_bins_consensus_and_diverged_correctly` (head_a=0.85 → consensus; head_b=0.15 → diverged; head_c=0.50 → neither)
      - `aux_head_summary_uses_latest_step` (3 certs at steps 100, 500, 1000 → uses step 1000's aux_head_cosines)
      - `aux_head_summary_alphabetical_ordering` (deterministic output)
      - `load_train_certs_window_returns_last_n` (synthesize 1500 certs, window=1000, expect last 1000)
  </in_scope>
  <out_of_scope>
    - Q1/Q2/Q3 rollup (TASK-EVAL-012)
    - Phase 3 TRAINING's writer side (SPEC-TRAINING owns TrainCert producer)
    - Phase 5 SELF-HEALING's consumption of `aux_heads_diverged` (Phase 5 read-only)
    - True optimal-transport Wasserstein (deferred — sliced Wasserstein is acceptable per TECH-EVALUATION rationale)
    - check_ship_gate consumption of state_transfer (TASK-EVAL-008 doesn't check it; informational diagnostic)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/state_transfer.rs">
use super::config::EvalConfig;
use super::error::EvalError;
use super::types::StateTransferDiagnostic;
use super::aux_head::TrainCert;

pub fn compute_state_transfer_diagnostic(
    holdout_embeddings: &amp;[[f32; 512]],
    live_embeddings: &amp;[[f32; 512]],
    train_certs: &amp;[TrainCert],
    config: &amp;EvalConfig,
) -&gt; Result&lt;Option&lt;StateTransferDiagnostic&gt;, EvalError&gt;;

fn wasserstein_1_sampled(
    p: &amp;[[f32; 512]],
    q: &amp;[[f32; 512]],
    n_samples: usize,
) -&gt; Result&lt;(f32, usize), EvalError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/aux_head.rs">
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::config::EvalConfig;
use super::error::EvalError;
use super::storage::RocksDbStore;
use super::types::AuxHeadDistillationSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainCert {
    pub step: u64,
    pub distillation_loss: f32,
    pub delta_omega: f32,
    pub delta_xi: f32,
    pub aux_head_cosines: Option&lt;BTreeMap&lt;String, f32&gt;&gt;,
}

pub fn compute_aux_head_distillation_summary(
    train_certs: &amp;[TrainCert],
    config: &amp;EvalConfig,
) -&gt; Result&lt;Option&lt;AuxHeadDistillationSummary&gt;, EvalError&gt;;

pub fn load_train_certs_window(
    storage: &amp;RocksDbStore,
    window_steps: usize,
) -&gt; Result&lt;Vec&lt;TrainCert&gt;, EvalError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Wasserstein sliced approximation: K=32 random unit directions seeded from SHA-256 of `(p.len(), q.len())` — deterministic across runs
    - All f32 inputs validated finite; NaN propagates as `Err(EvalError::NanDetected{field: ..., cell: None})`
    - `t_value`, `m_t_estimate`, `performance_deploy_predicted` clamped to `[0, 1]` after computation
    - `m_t_window_steps` is a soft target — when `train_certs.len() &lt; window_steps`, use what's available; do NOT error
    - Geometric mean over `delta_omega * delta_xi`: each product MUST be `&gt; 0` (geometric mean undefined for non-positive); if any non-positive, `Err(EvalError::NanDetected{field: "delta_omega_xi_non_positive"})`
    - `aux_head_divergence_threshold` from config; default 0.20 per constitution-pinned EvalConfig
    - All Err returns wrapped with `.log_context("eval::state_transfer")` or `("eval::aux_head")`
    - File length budget: each &lt; 500 lines
    - Rust 2021 edition; Rust 1.84+
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - state_transfer: ≥ 8 passing tests
    - aux_head: ≥ 4 passing tests
    - Wasserstein zero on identical distributions (within 1e-3)
    - T monotone decreasing in d
    - M(t) geometric mean within 1e-4 of analytic ground truth on synthetic train_certs
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/state_transfer.rs:
  pub fn compute_state_transfer_diagnostic(holdout, live, train_certs, config) -&gt; Result&lt;Option&lt;StateTransferDiagnostic&gt;, EvalError&gt; {
      if live.len() &lt; 100 {
          let err = EvalError::InsufficientLiveData { count: live.len(), required: 100 };
          err.log_context("eval::state_transfer");
          return Ok(None);
      }
      let (d, samples_used) = wasserstein_1_sampled(holdout, live, config.state_transfer_wasserstein_samples)?;
      let t = (-d / config.state_transfer_lambda).exp().clamp(0.0, 1.0);

      // M(t) geometric mean
      let window: Vec&lt;_&gt; = train_certs.iter().rev().take(config.m_t_window_steps).collect();
      if window.is_empty() {
          return Err(EvalError::InsufficientLiveData { count: 0, required: 1 });
      }
      let mut log_sum = 0.0_f64;
      let mut count = 0;
      for cert in &window {
          let prod = (cert.delta_omega * cert.delta_xi) as f64;
          if !prod.is_finite() || prod &lt;= 0.0 {
              return Err(EvalError::NanDetected { field: "delta_omega_xi_non_positive".into(), cell: None });
          }
          log_sum += prod.ln();
          count += 1;
      }
      let m_t = (log_sum / count as f64).exp() as f32;
      let m_t = m_t.clamp(0.0, 1.0);
      let perf = (m_t * t).clamp(0.0, 1.0);

      Ok(Some(StateTransferDiagnostic {
          d_train_deploy: d,
          lambda: config.state_transfer_lambda,
          t_value: t,
          m_t_estimate: m_t,
          performance_deploy_predicted: perf,
          attribution: format!("d={:.4} via sliced Wasserstein-1 (samples={}); T=exp(-d/lambda); M(t) geometric mean over {} steps", d, samples_used, count),
          wasserstein_samples_used: samples_used,
      }))
  }

crates/context-graph-mejepa/src/eval/aux_head.rs:
  pub fn compute_aux_head_distillation_summary(train_certs, config) -&gt; Result&lt;Option&lt;AuxHeadDistillationSummary&gt;, EvalError&gt; {
      let latest = train_certs.iter().filter(|c| c.aux_head_cosines.is_some()).max_by_key(|c| c.step);
      let cert = match latest { Some(c) =&gt; c, None =&gt; return Ok(None) };
      let cosines = cert.aux_head_cosines.as_ref().unwrap();
      let mut consensus = Vec::new();
      let mut diverged = Vec::new();
      for (name, cos) in cosines {
          if !cos.is_finite() {
              return Err(EvalError::NanDetected { field: format!("aux_head_{}", name), cell: None });
          }
          if *cos &gt;= 0.80 { consensus.push(name.clone()); }
          else if *cos &lt; config.aux_head_divergence_threshold { diverged.push(name.clone()); }
      }
      consensus.sort();
      diverged.sort();
      Ok(Some(AuxHeadDistillationSummary {
          last_distillation_step: cert.step,
          distillation_loss: cert.distillation_loss,
          aux_heads_in_consensus: consensus,
          aux_heads_diverged: diverged,
      }))
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/state_transfer.rs">compute_state_transfer_diagnostic + wasserstein_1_sampled (sliced) + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/aux_head.rs">compute_aux_head_distillation_summary + load_train_certs_window + TrainCert read-side struct + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 6 (state-transfer) and step 7 (aux-head) of run_holdout_eval</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod state_transfer; pub mod aux_head;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>state_transfer returns None when live &lt; 100</criterion>
  <criterion>Wasserstein ≈ 0 on identical distributions</criterion>
  <criterion>T monotone-decreasing in d; clamped [0, 1]</criterion>
  <criterion>M(t) geometric mean correct within 1e-4 on synthetic certs</criterion>
  <criterion>Geometric mean rejects non-positive delta_omega × delta_xi</criterion>
  <criterion>aux_head_summary picks latest step's cosines</criterion>
  <criterion>aux_head consensus / diverged correctly binned at 0.80 / 0.20 boundaries</criterion>
  <criterion>aux_head output deterministic (alphabetical sort)</criterion>
  <criterion>load_train_certs_window returns last N entries from CF_MEJEPA_TRAIN_CERTS</criterion>
  <criterion>run_holdout_eval populates EvalReport.state_transfer_diagnostic AND .aux_head_distillation</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::state_transfer</command>
  <command>cargo test -p context-graph-mejepa eval::aux_head</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

# TASK-HEAL-006 — EwcPlusPlus methods (update_fisher_online, detect_task_boundary, snapshot_current_task, current_lambda, ewc_penalty)

```xml
<task_spec id="TASK-HEAL-006" version="1.0">

<metadata>
  <title>EWC++ method bodies in `crates/context-graph-mejepa/src/heal/ewc.rs` (extending TASK-HEAL-001 struct shapes)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-HEAL-01">EWC penalty term in observe() L_total</item>
    <item ref="REQ-HEAL-02">update_fisher_online + detect_task_boundary + snapshot_current_task + current_lambda + ewc_penalty</item>
    <item ref="AC-HEAL-01-03">10 boundaries → Fisher full-rank (rank-deficient is a bug)</item>
    <item ref="AC-HEAL-01-04">λ ∈ [2000, 15000] adaptive ramp</item>
    <item ref="EC-HEAL-01">Cold-start: zero EWC penalty on first observe; immediate task_0 snapshot after first SGD step</item>
    <item ref="EC-HEAL-02">SVD merge of oldest deltas into base when 1 base + ≤4 deltas cap exceeded</item>
    <item ref="ERR-HEAL-06">MEJEPA_HEAL_FISHER_RANK_DEFICIENT</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-001</task_ref>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 modified (src/heal/ewc.rs); ≤1 created (src/heal/ewc_svd.rs if 500-line cap requires split)</estimated_files>
</metadata>

<context>
First logic task. Implements the five EWC++ method bodies on the structs landed in TASK-HEAL-001:
diagonal-Fisher online update via running EMA τ=0.01, sona-style task-boundary detection via
Hutchinson Hessian-trace z-score against running mean/variance, task snapshot persistence with
SVD-merge-on-overflow, adaptive λ ramp `λ = clamp(initial * (1 + ln(1 + task_count)), [initial, max])`,
and the per-snapshot EWC penalty `Σ_i F_i (θ_i − θ_i*)²`. Inspired by `ruvnet/ruvector
crates/sona/src/ewc.rs at HEAD ef5274c2`; clean-room reimplementation. All paths fail-closed on
non-finite values; rank-deficient Fisher returns Err per ERR-HEAL-06.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<utml_v1_invariants>` rule 4 ΔK Fisher protection; `<failure_taxonomy>` MEJEPA_HEAL_FISHER_RANK_DEFICIENT)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-01, REQ-HEAL-02; AC-HEAL-01-03/04; EC-HEAL-01/02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts EwcPlusPlus methods + observe_sequence_diagram)</file>
  <file purpose="prior_task">TASK-HEAL-001 (struct shapes + DEFAULT_* constants)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError::FisherRankDeficient + CriticalBug variants)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_FISHER_SNAPSHOTS encoder)</file>
  <file purpose="cross_domain_predictor">crates/context-graph-mejepa-instruments/src/lib.rs (Panel layout)</file>
  <file purpose="cross_domain_predictor">crates/context-graph-mejepa/src/predictor.rs (Phase 2 Predictor::forward + parameters() iterator) — read-only, blocking dep</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-001, 004, 005 complete</check>
  <check>Phase 2 Predictor exposes a `parameters_flat() -> Vec&lt;f32&gt;` and `parameter_grad_flat() -> Vec&lt;f32&gt;` API (or equivalent ndarray accessor)</check>
  <check>An OracleOutcome type is reachable from heal/ewc.rs (re-export from mejepa-instruments)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement on `EwcPlusPlus`:
      - `pub fn update_fisher_online(&mut self, panel: &Panel, oracle_outcome: &OracleOutcome, predictor: &impl PredictorGradAccessor) -> Result<FisherUpdate, HealError>` — REQ-HEAL-02
      - `pub fn detect_task_boundary(&mut self, current_loss: f32, predictor: &impl HessianTraceProbe) -> Result<bool, HealError>` — REQ-HEAL-02 (curvature-shift heuristic)
      - `pub fn snapshot_current_task(&mut self, predictor_params: &[f32], corpus_sha: [u8; 32]) -> Result<TaskSnapshot, HealError>` — REQ-HEAL-02 (with SVD-merge-on-overflow)
      - `pub fn current_lambda(&self) -> f32` — REQ-HEAL-02 adaptive ramp
      - `pub fn ewc_penalty(&self, theta_flat: &[f32]) -> Result<f32, HealError>` — REQ-HEAL-01 scalar; vectorized `Σ F * (θ − θ*)²` over all retained snapshots
      - `pub fn fisher_rank(&self) -> usize` — convenience; reads cached `fisher_matrix.rank`
      - `fn svd_merge_oldest_into_base(&mut self) -> Result<(), HealError>` (private) — fires when `task_snapshots.len() > PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS`. Algorithm: stack the oldest delta's `(theta_star, fisher_diag)` into the base via Fisher-weighted average `θ*_base ← (F_base θ*_base + F_old θ*_old) / (F_base + F_old)`, then `F_base ← F_base + F_old`; remove the oldest from `task_snapshots`.
    - Define two trait abstractions for predictor injection (avoids forward-reference into Phase 2 internals):
      - `pub trait PredictorGradAccessor { fn forward_with_loss(&self, panel: &Panel, oracle_outcome: &OracleOutcome) -> Result<(f32, Vec<f32>), HealError>; /* returns (loss, grad_flat) */ }`
      - `pub trait HessianTraceProbe { fn hutchinson_trace(&self, num_samples: u32, panel: &Panel) -> Result<f32, HealError>; }`
    - Update `FisherMatrix::recompute_rank` to use a numerical-rank heuristic: `rank = count(diag_i > eps)` where `eps = max(diag) * f32::EPSILON * dim`. Persist the recomputed rank into `self.rank` only on call (caller controls when).
    - Implement `current_lambda` per pseudo: `clamp(initial_lambda * (1 + (1 + task_count as f32).ln()), initial_lambda, max_lambda)`.
    - Persistence wiring: `snapshot_current_task` accepts an `&impl SnapshotStore` injected trait `pub trait SnapshotStore { fn put_snapshot(&self, key: [u8;8], value: &[u8]) -> Result<(), HealError>; }` so the raw RocksDB CF write is delegated; this trait will be implemented by SelfHealingPipeline in TASK-HEAL-008. (Alternative: take the raw bytes and let the pipeline persist; either pattern is acceptable as long as no `Arc<RocksDbStore>` lives inside `EwcPlusPlus`.)
    - Add unit + integration tests:
      - `update_fisher_online_increases_step_count`
      - `update_fisher_online_rejects_non_finite_grad` (synthetic NaN injection on the grad accessor mock — note: this uses a deterministic in-test trait impl, not a "mock model"; per REQ-HEAL-16 the predictor itself is real)
      - `detect_task_boundary_returns_false_on_first_call` (running mean/var not yet stable)
      - `detect_task_boundary_returns_true_after_two_consecutive_z_above_threshold`
      - `detect_task_boundary_resets_after_positive_detection`
      - `snapshot_current_task_persists_to_store_via_trait`
      - `snapshot_current_task_triggers_svd_merge_at_cap_plus_one`
      - `svd_merge_combines_two_snapshots_correctly` (analytical check on a 4-element diagonal Fisher)
      - `current_lambda_starts_at_initial`
      - `current_lambda_ramps_with_task_count` (10 snapshots → λ around 7000)
      - `current_lambda_clamped_at_max_lambda` (1000 snapshots → λ == 15000 exactly)
      - `ewc_penalty_zero_on_first_step` (no snapshots → 0)
      - `ewc_penalty_scales_with_theta_distance` (simple closed-form check)
      - `ewc_penalty_rejects_dim_mismatch`
      - `ten_boundaries_full_rank` (REQ-HEAL-02 + AC-HEAL-01-03 invariant)
      - `lambda_in_2000_15000_after_10_boundaries` (AC-HEAL-01-04)
      - `fisher_rank_deficient_returns_err` (synthetic zero-grad regime)
    - FSV evidence emission (referenced by TASK-HEAL-019 final harness): output the per-boundary λ trajectory + per-step Fisher-rank curve to `/tmp/contextgraph-mejepa-self-healing-fsv/ewc-fisher-evidence.json` when run in test mode. The actual writer lives in TASK-HEAL-019; this task only ensures the data plumbing is exposed via `EwcPlusPlus::fsv_snapshot() -> EwcFsvEvidence`.
  </in_scope>
  <out_of_scope>
    - SelfHealingPipeline::observe orchestration (TASK-HEAL-008)
    - SnapshotStore live RocksDbStore impl (TASK-HEAL-008)
    - Predictor implementation (Phase 2)
    - Cross-cutting `signal_clarity` weighting on EWC penalty (handled at observe-step in TASK-HEAL-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/ewc.rs">
pub trait PredictorGradAccessor {
    fn forward_with_loss(&amp;self, panel: &amp;Panel, oracle_outcome: &amp;OracleOutcome)
        -&gt; Result&lt;(f32, Vec&lt;f32&gt;), HealError&gt;;
    fn parameter_count(&amp;self) -&gt; usize;
}

pub trait HessianTraceProbe {
    fn hutchinson_trace(&amp;self, num_samples: u32, panel: &amp;Panel) -&gt; Result&lt;f32, HealError&gt;;
}

pub trait SnapshotStore {
    fn put_snapshot(&amp;self, key: [u8; 8], value: &amp;[u8]) -&gt; Result&lt;(), HealError&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EwcFsvEvidence {
    pub lambda_trajectory: Vec&lt;(u64, f32)&gt;,    // (boundary_step, lambda)
    pub fisher_rank_curve: Vec&lt;(u64, usize)&gt;,  // (step_count, rank)
    pub task_snapshot_count: usize,
    pub initial_lambda: f32,
    pub max_lambda: f32,
}

impl EwcPlusPlus {
    pub fn update_fisher_online&lt;P: PredictorGradAccessor&gt;(
        &amp;mut self, panel: &amp;Panel, oracle_outcome: &amp;OracleOutcome, predictor: &amp;P
    ) -&gt; Result&lt;FisherUpdate, HealError&gt;;

    pub fn detect_task_boundary&lt;H: HessianTraceProbe&gt;(
        &amp;mut self, current_loss: f32, predictor: &amp;H, panel: &amp;Panel
    ) -&gt; Result&lt;bool, HealError&gt;;

    pub fn snapshot_current_task&lt;S: SnapshotStore&gt;(
        &amp;mut self, predictor_params: &amp;[f32], corpus_sha: [u8; 32], store: &amp;S
    ) -&gt; Result&lt;TaskSnapshot, HealError&gt;;

    pub fn current_lambda(&amp;self) -&gt; f32;

    pub fn ewc_penalty(&amp;self, theta_flat: &amp;[f32]) -&gt; Result&lt;f32, HealError&gt;;

    pub fn fisher_rank(&amp;self) -&gt; usize;

    pub fn fsv_snapshot(&amp;self) -&gt; EwcFsvEvidence;
}
    </signature>
  </signatures>
  <constraints>
    - All inputs validated finite at entry; non-finite returns Err(HealError::BatchNan { component: "ewc.update_fisher_online", witness_chain_offset: 0 })
    - `update_fisher_online` advances `step_count`; if `step_count % 1000 == 0` recomputes rank; if rank-deficient AFTER any boundary returns Err(HealError::FisherRankDeficient)
    - `current_lambda()` is monotone non-decreasing in `task_count` until it hits `max_lambda`
    - `ewc_penalty` is O(N · K) where N = predictor_dim, K = task_snapshots.len() (capped at 5)
    - SVD merge fires only when `task_snapshots.len() > PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS`; never on smaller counts
    - Atomic write semantics: snapshot_current_task stages bytes and calls SnapshotStore::put_snapshot which itself MUST be atomic (the trait contract; impl in TASK-HEAL-008 uses the shipped `write_text_checked`)
    - No `unwrap_or` on Fisher-rank or λ values per constitution `<anti_patterns>`
    - File &lt; 500 lines (split into ewc_svd.rs if needed)
    - No mock data in tests; deterministic synthetic Panel + OracleOutcome built from real bytes
    - tracing::error! before every Err return per failure_logging note (component, panel_sha, lambda, fisher_rank)
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::ewc` shows ≥ 28 tests (12 from TASK-HEAL-001 + ≥16 new)
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - 10-boundary integration test asserts `fisher_rank == predictor_dim` (full rank)
    - 10-boundary integration test asserts `λ ∈ [2000, 15000]`
    - 1000-snapshot stress test asserts `task_snapshots.len() ≤ 5` (SVD merge fires)
    - Cold-start: first observe with `task_snapshots.is_empty()` produces `ewc_penalty == 0.0` exactly
  </verification>
</definition_of_done>

<pseudo_code>
impl EwcPlusPlus {
    pub fn update_fisher_online<P: PredictorGradAccessor>(&mut self, panel, oracle_outcome, predictor) -> Result<FisherUpdate, HealError> {
        let (_loss, grad) = predictor.forward_with_loss(panel, oracle_outcome)?;
        if grad.iter().any(|x| !x.is_finite()) {
            tracing::error!(error_code = "MEJEPA_OBSERVE_BATCH_NAN", component = "ewc.update_fisher_online");
            return Err(HealError::BatchNan { component: "ewc.update_fisher_online".into(), witness_chain_offset: 0 });
        }
        if grad.len() != self.fisher_matrix.diagonal.len() {
            return Err(HealError::FisherRankDeficient { rank: 0, dim: self.fisher_matrix.diagonal.len() });
        }
        let mut delta_norm_sq = 0.0_f64;
        for (f, g) in self.fisher_matrix.diagonal.iter_mut().zip(grad.iter()) {
            let new = (1.0 - DEFAULT_FISHER_EMA_DECAY) * *f + DEFAULT_FISHER_EMA_DECAY * (g * g);
            delta_norm_sq += ((new - *f) as f64).powi(2);
            *f = new;
        }
        self.fisher_matrix.step_count += 1;
        if self.fisher_matrix.step_count % 1000 == 0 {
            self.fisher_matrix.rank = self.fisher_matrix.recompute_rank();
            if self.fisher_matrix.rank < self.fisher_matrix.diagonal.len() && !self.task_snapshots.is_empty() {
                return Err(HealError::FisherRankDeficient { rank: self.fisher_matrix.rank, dim: self.fisher_matrix.diagonal.len() });
            }
        }
        Ok(FisherUpdate { delta_norm: delta_norm_sq.sqrt() as f32, current_rank: self.fisher_matrix.rank })
    }

    pub fn current_lambda(&self) -> f32 {
        let task_count = self.task_snapshots.len() as f32;
        let raw = self.initial_lambda * (1.0 + (1.0 + task_count).ln());
        raw.clamp(self.initial_lambda, self.max_lambda)
    }

    pub fn ewc_penalty(&self, theta_flat: &[f32]) -> Result<f32, HealError> {
        if theta_flat.iter().any(|x| !x.is_finite()) {
            return Err(HealError::BatchNan { component: "ewc.ewc_penalty".into(), witness_chain_offset: 0 });
        }
        let mut total = 0.0_f64;
        for snap in &self.task_snapshots {
            if theta_flat.len() != snap.theta_star.len() {
                return Err(HealError::FisherRankDeficient { rank: theta_flat.len(), dim: snap.theta_star.len() });
            }
            for ((t, ts), f) in theta_flat.iter().zip(snap.theta_star.iter()).zip(snap.fisher_diag_at_boundary.iter()) {
                let d = (*t - *ts) as f64;
                total += (*f as f64) * d * d;
            }
        }
        Ok(total as f32)
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/ewc_svd.rs">SVD-merge helper if ewc.rs exceeds 500 lines</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/ewc.rs">add method bodies + traits + EwcFsvEvidence + tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::ewc shows ≥ 28 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>10-boundary test asserts fisher_rank == predictor_dim (Phase 5 DoD invariant)</criterion>
  <criterion>10-boundary test asserts λ ∈ [2000, 15000]</criterion>
  <criterion>1000-snapshot stress test asserts task_snapshots.len() ≤ 5 (SVD-merge cap)</criterion>
  <criterion>Cold-start ewc_penalty == 0.0</criterion>
  <criterion>NaN gradient injection returns HealError::BatchNan with .code() == "MEJEPA_OBSERVE_BATCH_NAN"</criterion>
  <criterion>Zero-rank Fisher AFTER snapshot returns HealError::FisherRankDeficient</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::ewc</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

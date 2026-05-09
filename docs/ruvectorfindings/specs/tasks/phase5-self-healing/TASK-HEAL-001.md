# TASK-HEAL-001 — EWC++ data models (EwcPlusPlus, FisherMatrix, TaskSnapshot, BoundaryDetectorState)

```xml
<task_spec id="TASK-HEAL-001" version="1.0">

<metadata>
  <title>EWC++ data models in `crates/context-graph-mejepa/src/heal/ewc.rs` (struct definitions only; method bodies in TASK-HEAL-006)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-HEAL-01">EWC-regularized SGD step requires Fisher accumulator + λ + task snapshots + boundary detector data shapes</item>
    <item ref="REQ-HEAL-02">EwcPlusPlus carries fisher_matrix, lambda, task_snapshots, boundary_detector</item>
    <item ref="data_model:EwcPlusPlus">struct fields per TECH-SELF-HEALING data_models</item>
    <item ref="data_model:FisherMatrix">diagonal Vec&lt;f32&gt; + rank + step_count</item>
    <item ref="data_model:TaskSnapshot">theta_star + fisher_diag_at_boundary + boundary_step + corpus_sha + frozen_at</item>
    <item ref="data_model:BoundaryDetectorState">EMA loss mean/var + Hessian-trace estimator + boundary_threshold</item>
    <item ref="constitution:utml_v1_invariants">EWC penalty contributes to ΔK protection; rule 4 of utml_v1_invariants</item>
  </implements>
  <depends_on><!-- foundation; cross-domain shipped (witness, storage primitives only referenced by name) --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 created (Cargo.toml augmentation, src/heal/mod.rs scaffold + src/heal/ewc.rs); 1 modified (workspace Cargo.toml)</estimated_files>
</metadata>

<context>
First foundation task for the SELF-HEALING domain. Creates the new `crates/context-graph-mejepa/`
crate skeleton (workspace member) and the `src/heal/` sub-module. Lands the public data model
contract for EWC++ (Elastic Weight Consolidation, online Fisher accumulator + λ ramp + boundary
detector). Method bodies are deferred to TASK-HEAL-006; this task only ships struct/enum
definitions, derives, validating constructors, and unit tests for invariants. Inspired by
`ruvnet/ruvector crates/sona/src/ewc.rs at HEAD ef5274c2` per upstream-reference-only locked decision
(clean-room reimplementation; no code copied).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<utml_v1_invariants>` rules 3-4; `<coding_standards>` naming + file_organization; `<failure_taxonomy>` MEJEPA_HEAL_FISHER_RANK_DEFICIENT)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-01, REQ-HEAL-02; AC-HEAL-01-03 / 01-04 Fisher rank + λ ramp invariants; EC-HEAL-01 cold start; EC-HEAL-02 SVD merge cap)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (data_models block: EwcPlusPlus, FisherMatrix, TaskSnapshot, BoundaryDetectorState; component_contracts EwcPlusPlus method signatures for type alignment)</file>
  <file purpose="provenance">memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md (clean-room rule)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files; PascalCase types; SCREAMING_SNAKE consts)</file>
  <file purpose="cross_domain_predictor">crates/context-graph-mejepa-instruments/src/lib.rs (Panel layout reference; signature alignment for forthcoming observe(panel) parameter)</file>
</input_context_files>

<prerequisites>
  <check>Workspace builds: `cargo build --release --workspace`</check>
  <check>`serde = "1"`, `serde_json = "1"`, `bincode = "1"` workspace deps present</check>
  <check>`sha2` workspace dep available (referenced for corpus_sha)</check>
  <check>The crate `crates/context-graph-mejepa/` is registered as a workspace member (this task creates it if absent)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/Cargo.toml` (workspace member; deps to context-graph-mejepa-instruments, context-graph-witness, context-graph-storage, context-graph-embeddings, serde, bincode, sha2, tracing). If the crate already exists, add the new heal/ deps without disturbing existing ones.
    - Create `crates/context-graph-mejepa/src/lib.rs` (or extend) declaring `pub mod heal;`
    - Create `crates/context-graph-mejepa/src/heal/mod.rs` declaring `pub mod {ewc, drift, promote, errors, status, calibration, integrity, lora_refresh, distill, pipeline, schedule};` (forward-declare empty modules to be filled by later tasks; this task only fills `ewc.rs` and the `mod.rs` scaffold).
    - Create `crates/context-graph-mejepa/src/heal/ewc.rs` with:
      - `FisherMatrix { diagonal: Vec<f32>, rank: usize, step_count: u64 }` (diagonal-only Fisher; `try_new_zero(dim)` constructor; `recompute_rank` method body STUB returning current `rank` field; finite-validation on diagonal mutations)
      - `TaskSnapshot { theta_star: Vec<f32>, fisher_diag_at_boundary: Vec<f32>, boundary_step: u64, corpus_sha: [u8; 32], frozen_at: i64 }` with `try_new` validating `theta_star.len() == fisher_diag_at_boundary.len()` and finite values
      - `BoundaryDetectorState { loss_running_mean: f32, loss_running_var: f32, hessian_trace_estimator: HessianTraceEstimator, boundary_threshold: f32, consecutive_above_threshold: u32 }` with `default()` returning `boundary_threshold = 3.0` (z-score) and zeros elsewhere
      - `HessianTraceEstimator { last_estimate: f32, hutchinson_samples: u32, last_update_step: u64 }` (Hutchinson-style placeholder; implementation in TASK-HEAL-006)
      - `EwcPlusPlus { fisher_matrix: FisherMatrix, lambda: f32, task_snapshots: Vec<TaskSnapshot>, boundary_detector: BoundaryDetectorState, initial_lambda: f32, max_lambda: f32, predictor_dim: usize }` with `try_new(predictor_dim, initial_lambda, max_lambda)` validating `initial_lambda <= max_lambda` and `initial_lambda >= 1.0`
      - `FisherUpdate { delta_norm: f32, current_rank: usize }` (return type for update_fisher_online; method body in TASK-HEAL-006)
      - Module-level constants: `pub const DEFAULT_INITIAL_LAMBDA: f32 = 2000.0;`, `pub const DEFAULT_MAX_LAMBDA: f32 = 15000.0;`, `pub const DEFAULT_BOUNDARY_THRESHOLD_Z: f32 = 3.0;`, `pub const DEFAULT_FISHER_EMA_DECAY: f32 = 0.01;`, `pub const PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS: usize = 5;` (1 base + 4 deltas, per EC-HEAL-02 + doc 12 §11 risk #5)
    - All types derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq` (EwcPlusPlus also `Default` is NOT derived — must use `try_new`).
    - Add unit tests in `src/heal/ewc.rs` `#[cfg(test)] mod tests`:
      - `fisher_matrix_zero_constructor_full_zeros_and_rank_zero`
      - `fisher_matrix_rejects_non_finite_in_constructor` (negative test)
      - `task_snapshot_rejects_length_mismatch`
      - `task_snapshot_rejects_non_finite_theta`
      - `boundary_detector_state_default_threshold_three`
      - `ewc_plus_plus_constructor_clamps_lambda_in_range`
      - `ewc_plus_plus_rejects_initial_above_max`
      - `ewc_plus_plus_rejects_zero_predictor_dim`
      - `serde_round_trip_fisher_matrix`
      - `serde_round_trip_task_snapshot`
      - `default_constants_match_spec` (asserts all `DEFAULT_*` constants equal the spec defaults)
      - `snapshot_cap_constant_is_five` (1 base + 4 deltas)
  </in_scope>
  <out_of_scope>
    - update_fisher_online / detect_task_boundary / snapshot_current_task / current_lambda / ewc_penalty method bodies (TASK-HEAL-006)
    - SVD merge logic (TASK-HEAL-006)
    - Hutchinson-style Hessian-trace computation (TASK-HEAL-006)
    - DriftDetector / DriftSample / SeverityTable (TASK-HEAL-002)
    - AbcPromoter / ModelHandle / EnsembleHandle / HoldoutEval / PromotionGate (TASK-HEAL-003)
    - HealError / HealReport / HealStatus (TASK-HEAL-004)
    - RocksDB CF descriptors (TASK-HEAL-005)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/ewc.rs">
use serde::{Deserialize, Serialize};

pub const DEFAULT_INITIAL_LAMBDA: f32 = 2000.0;
pub const DEFAULT_MAX_LAMBDA: f32 = 15000.0;
pub const DEFAULT_BOUNDARY_THRESHOLD_Z: f32 = 3.0;
pub const DEFAULT_FISHER_EMA_DECAY: f32 = 0.01;
pub const PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FisherMatrix {
    pub diagonal: Vec&lt;f32&gt;,
    pub rank: usize,
    pub step_count: u64,
}

impl FisherMatrix {
    pub fn try_new_zero(dim: usize) -&gt; Result&lt;Self, &amp;'static str&gt;;
    pub fn recompute_rank(&amp;self) -&gt; usize;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskSnapshot {
    pub theta_star: Vec&lt;f32&gt;,
    pub fisher_diag_at_boundary: Vec&lt;f32&gt;,
    pub boundary_step: u64,
    pub corpus_sha: [u8; 32],
    pub frozen_at: i64,
}

impl TaskSnapshot {
    pub fn try_new(theta_star: Vec&lt;f32&gt;, fisher_diag_at_boundary: Vec&lt;f32&gt;,
                   boundary_step: u64, corpus_sha: [u8; 32], frozen_at: i64)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HessianTraceEstimator {
    pub last_estimate: f32,
    pub hutchinson_samples: u32,
    pub last_update_step: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoundaryDetectorState {
    pub loss_running_mean: f32,
    pub loss_running_var: f32,
    pub hessian_trace_estimator: HessianTraceEstimator,
    pub boundary_threshold: f32,
    pub consecutive_above_threshold: u32,
}

impl Default for BoundaryDetectorState {
    fn default() -&gt; Self;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EwcPlusPlus {
    pub fisher_matrix: FisherMatrix,
    pub lambda: f32,
    pub task_snapshots: Vec&lt;TaskSnapshot&gt;,
    pub boundary_detector: BoundaryDetectorState,
    pub initial_lambda: f32,
    pub max_lambda: f32,
    pub predictor_dim: usize,
}

impl EwcPlusPlus {
    pub fn try_new(predictor_dim: usize, initial_lambda: f32, max_lambda: f32)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FisherUpdate {
    pub delta_norm: f32,
    pub current_rank: usize,
}
    </signature>
  </signatures>
  <constraints>
    - File &lt; 500 lines per constitution `<file_organization>` rule
    - All `Vec&lt;f32&gt;` constructor inputs validated finite (no NaN/Inf) before storage
    - `lambda` always clamped to `[initial_lambda, max_lambda]` at construction
    - `initial_lambda &gt;= 1.0`, `max_lambda &gt;= initial_lambda`, `predictor_dim &gt; 0`
    - `boundary_threshold` default 3.0 z-score per data_models block
    - Snapshot cap constant exposes the (1 base + ≤4 deltas) per-embedder rule from EC-HEAL-02 + doc 12 §11 risk #5
    - All public types `Debug + Clone + Serialize + Deserialize + PartialEq`
    - NO `unwrap_or` on confidence/threshold values per constitution `<anti_patterns>`
    - File begins with `// Inspired by ruvnet/RuVector crates/sona/src/ewc.rs at HEAD ef5274c2 (clean-room reimplementation; no code copied).` per constitution `<provenance>` rule
    - Crate name `context-graph-mejepa` per constitution kebab-case rule (already implied by §3 directory layout)
    - All exports go through `pub use heal::ewc::*;` to be made public from the crate root in TASK-HEAL-004 (this task only adds `pub mod ewc;` inside `src/heal/mod.rs`)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa heal::ewc` shows ≥ 12 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - All `DEFAULT_*` constants match spec values byte-for-byte
    - `EwcPlusPlus::try_new(dim=0, ...)` returns Err
    - `EwcPlusPlus::try_new(dim=N, initial_lambda=20000, max_lambda=15000)` returns Err
    - `TaskSnapshot::try_new(theta=vec_len_100, fisher=vec_len_99, ...)` returns Err
  </verification>
</definition_of_done>

<pseudo_code>
// Provenance comment at top of file
// Inspired by ruvnet/RuVector crates/sona/src/ewc.rs at HEAD ef5274c2 (clean-room reimplementation).

impl FisherMatrix {
    pub fn try_new_zero(dim: usize) -> Result<Self, &'static str> {
        if dim == 0 { return Err("dim must be > 0"); }
        Ok(Self { diagonal: vec![0.0_f32; dim], rank: 0, step_count: 0 })
    }
    pub fn recompute_rank(&self) -> usize {
        // Stub: full SVD impl in TASK-HEAL-006. For now: count non-zero entries.
        self.diagonal.iter().filter(|x| x.abs() > f32::EPSILON).count()
    }
}

impl TaskSnapshot {
    pub fn try_new(theta_star, fisher_diag_at_boundary, boundary_step, corpus_sha, frozen_at)
        -> Result<Self, &'static str>
    {
        if theta_star.len() != fisher_diag_at_boundary.len() {
            return Err("theta_star length != fisher_diag_at_boundary length");
        }
        if theta_star.iter().any(|x| !x.is_finite()) { return Err("non-finite theta_star"); }
        if fisher_diag_at_boundary.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return Err("non-finite or negative fisher_diag");
        }
        Ok(Self { theta_star, fisher_diag_at_boundary, boundary_step, corpus_sha, frozen_at })
    }
}

impl Default for BoundaryDetectorState {
    fn default() -> Self {
        Self {
            loss_running_mean: 0.0,
            loss_running_var: 0.0,
            hessian_trace_estimator: HessianTraceEstimator { last_estimate: 0.0, hutchinson_samples: 0, last_update_step: 0 },
            boundary_threshold: DEFAULT_BOUNDARY_THRESHOLD_Z,
            consecutive_above_threshold: 0,
        }
    }
}

impl EwcPlusPlus {
    pub fn try_new(predictor_dim, initial_lambda, max_lambda) -> Result<Self, &'static str> {
        if predictor_dim == 0 { return Err("predictor_dim must be > 0"); }
        if !initial_lambda.is_finite() || !max_lambda.is_finite() { return Err("non-finite lambda"); }
        if initial_lambda < 1.0 { return Err("initial_lambda must be >= 1"); }
        if max_lambda < initial_lambda { return Err("max_lambda must be >= initial_lambda"); }
        let fisher_matrix = FisherMatrix::try_new_zero(predictor_dim)?;
        Ok(Self {
            fisher_matrix,
            lambda: initial_lambda,
            task_snapshots: Vec::new(),
            boundary_detector: BoundaryDetectorState::default(),
            initial_lambda,
            max_lambda,
            predictor_dim,
        })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/Cargo.toml">workspace member metadata + deps (serde, bincode, sha2, tracing, context-graph-mejepa-instruments, context-graph-witness, context-graph-storage, context-graph-embeddings)</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">crate root with `pub mod heal;`</file>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">heal sub-module scaffold: `pub mod {ewc, drift, promote, errors, status, calibration, integrity, lora_refresh, distill, pipeline, schedule};` — empty placeholders for forthcoming tasks; this task only fills `ewc.rs`</file>
  <file path="crates/context-graph-mejepa/src/heal/ewc.rs">EwcPlusPlus + FisherMatrix + TaskSnapshot + BoundaryDetectorState + HessianTraceEstimator + FisherUpdate + DEFAULT_* constants + tests</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml (workspace root)">add `crates/context-graph-mejepa` to `[workspace.members]` if not already present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa heal::ewc shows ≥ 12 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>EwcPlusPlus::try_new clamps lambda in [initial_lambda, max_lambda]</criterion>
  <criterion>EwcPlusPlus::try_new rejects predictor_dim==0</criterion>
  <criterion>EwcPlusPlus::try_new rejects max_lambda &lt; initial_lambda</criterion>
  <criterion>TaskSnapshot::try_new rejects length mismatch</criterion>
  <criterion>TaskSnapshot::try_new rejects non-finite or negative fisher_diag</criterion>
  <criterion>BoundaryDetectorState::default() has boundary_threshold == 3.0</criterion>
  <criterion>DEFAULT_INITIAL_LAMBDA == 2000.0; DEFAULT_MAX_LAMBDA == 15000.0; DEFAULT_FISHER_EMA_DECAY == 0.01; DEFAULT_BOUNDARY_THRESHOLD_Z == 3.0; PER_EMBEDDER_SNAPSHOT_CAP_BASE_PLUS_DELTAS == 5</criterion>
  <criterion>Provenance comment present at top of `ewc.rs`</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::ewc</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

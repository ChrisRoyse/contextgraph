# TASK-PRED-001 — Crate scaffolding + PredictorConfig + TransformerLayer + MeJepaPredictor structs

```xml
<task_spec id="TASK-PRED-001" version="1.0">

<metadata>
  <title>Create context-graph-mejepa crate; declare PredictorConfig + TransformerLayer + MeJepaPredictor structs (no method bodies yet)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-PRED-01">Predictor accepts (panel_t0, panel_t1) of shape (batch, 5120); declares dim contract</item>
    <item ref="REQ-PRED-02">6-layer transformer, hidden 1024, 8 heads, GeLU; PredictorConfig defaults</item>
    <item ref="REQ-PRED-17">gradient_checkpointing config flag</item>
    <item ref="component:MeJepaPredictor">struct skeleton (fields only; method bodies in TASK-PRED-005)</item>
    <item ref="component:PredictorConfig">data model</item>
    <item ref="component:TransformerLayer">struct skeleton (forward in TASK-PRED-004)</item>
  </implements>
  <depends_on><!-- none; foundation root of phase 2 --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>5 files created, 1 modified (workspace Cargo.toml)</estimated_files>
</metadata>

<context>
This task is the structural root of Phase 2. It creates the new `context-graph-mejepa` crate
(does not yet exist on disk per `git status` and `ls crates/`) and lays down the type-level
contracts the rest of the phase will fill in: `PredictorConfig` (architecture + headroom flags),
`TransformerLayer` (one block; forward body lives in TASK-PRED-004), and `MeJepaPredictor`
(top-level container; forward body in TASK-PRED-005). The constants `PANEL_DIM = 5_120`,
`CONCAT_INPUT_DIM = 10_240`, and the VICReg / VRAM defaults are all declared here so the
remaining tasks can reference them without forward-defining anything.

No method bodies are written here apart from `MeJepaPredictor::new` (validation + construction)
and `PredictorConfig::default()`. The crate compiles green at the end of this task — every
struct has a passing unit test that asserts default values match doc 09 §2.4 + REQ-PRED-02.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; tech_stack candle 0.9.x; performance_budgets predictor_forward_p50)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-01, REQ-PRED-02, REQ-PRED-17, data_requirements PredictorConfig)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (data_models MeJepaPredictor / PredictorConfig / TransformerLayer; library_api constants)</file>
  <file purpose="upstream_panel_dim">crates/context-graph-mejepa-instruments/src/lib.rs (PANEL_DIM = 5_120 — re-exported here)</file>
  <file purpose="workspace_root">Cargo.toml (workspace members; new crate registration)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files; PascalCase types; SCREAMING_SNAKE consts; kebab-case crate)</file>
  <file purpose="reference_crate_layout">crates/context-graph-mejepa-instruments/Cargo.toml (sibling crate Cargo.toml shape)</file>
</input_context_files>

<prerequisites>
  <check>Workspace builds clean: `cargo build --release --workspace` succeeds before this task</check>
  <check>candle-core, candle-nn, candle-transformers v0.9.x are already declared in workspace `[workspace.dependencies]`</check>
  <check>cudarc is available in workspace dependencies (used by TASK-PRED-010 for cuMemGetInfo)</check>
  <check>Phase 1c TASK-INSTR-001..003 have shipped at least the trait + Panel skeleton; verify `cargo build --release -p context-graph-mejepa-instruments` works</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/Cargo.toml` (kebab-case crate name)
    - Create `crates/context-graph-mejepa/src/lib.rs` (re-exports + module declarations)
    - Create `crates/context-graph-mejepa/src/config.rs` (PredictorConfig + ActivationKind + constants + Default impls)
    - Create `crates/context-graph-mejepa/src/predictor/mod.rs` (MeJepaPredictor struct skeleton + new())
    - Create `crates/context-graph-mejepa/src/predictor/transformer_layer.rs` (TransformerLayer struct skeleton; no forward body)
    - Modify `Cargo.toml` (workspace) to register `crates/context-graph-mejepa`
    - Add validation + construction body for `MeJepaPredictor::new` (config validation + device check)
    - Add config-only unit tests (5+) verifying defaults
  </in_scope>
  <out_of_scope>
    - TransformerLayer::forward body (TASK-PRED-004)
    - MeJepaPredictor::forward body (TASK-PRED-005)
    - OracleHead struct (TASK-PRED-002)
    - PredictedPanel / OracleLogits / TargetPanel / TargetProvenance data models (TASK-PRED-002)
    - VicregLambdas / LossOutputs / errors / evidence DTOs (TASK-PRED-003)
    - FrozenTargetAdapter struct + methods (TASK-PRED-006)
    - Gradient hook (TASK-PRED-009)
    - VRAM check (TASK-PRED-010)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/config.rs">
pub const PANEL_DIM: usize = 5_120;
pub const CONCAT_INPUT_DIM: usize = 10_240;
pub const DEFAULT_NUM_LAYERS: u8 = 6;
pub const DEFAULT_HIDDEN_DIM: u32 = 1_024;
pub const DEFAULT_NUM_HEADS: u8 = 8;
pub const DEFAULT_FF_EXPANSION: u8 = 4;
pub const DEFAULT_LAMBDA_VAR: f32 = 25.0;
pub const DEFAULT_LAMBDA_COV: f32 = 1.0;
pub const DEFAULT_LAMBDA_INV: f32 = 25.0;
pub const DEFAULT_GAMMA: f32 = 1.0;
pub const VRAM_STEADY_STATE_TARGET_BYTES: u64 = 8_589_934_592;
pub const VRAM_WARN_THRESHOLD_BYTES: u64 = 9_663_676_416;
pub const GRAD_NORM_NOISE_FLOOR: f64 = 1e-12;
pub const VICREG_DEGENERATE_DIM_FRACTION: f32 = 0.5;
pub const VICREG_DEGENERATE_CONSECUTIVE_PASSES: u8 = 2;
pub const DEFAULT_LAYER_NORM_EPS: f32 = 1e-5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationKind { GeLU }

#[derive(Debug, Clone)]
pub struct PredictorConfig {
    pub num_layers: u8,
    pub hidden_dim: u32,
    pub num_heads: u8,
    pub ff_expansion: u8,
    pub activation: ActivationKind,
    pub dropout: f32,
    pub layer_norm_eps: f32,
    pub gradient_checkpointing: bool,
    pub vram_warn_threshold_bytes: u64,
    pub vicreg_variance_floor_consecutive_passes: u8,
}

impl Default for PredictorConfig { fn default() -&gt; Self; }
impl PredictorConfig {
    pub fn validate(&amp;self) -&gt; Result&lt;(), crate::error::PredictorError&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/predictor/transformer_layer.rs">
use candle_core::Tensor;
use candle_nn::{LayerNorm, Linear, VarBuilder};

pub struct TransformerLayer {
    pub(crate) self_attn_qkv: Linear,
    pub(crate) self_attn_out: Linear,
    pub(crate) ffn_up: Linear,
    pub(crate) ffn_down: Linear,
    pub(crate) attn_ln: LayerNorm,
    pub(crate) ffn_ln: LayerNorm,
    pub(crate) num_heads: u8,
    pub(crate) head_dim: u32,
    pub(crate) ff_expansion: u8,
}

impl TransformerLayer {
    pub fn new(
        hidden_dim: u32,
        num_heads: u8,
        ff_expansion: u8,
        layer_norm_eps: f32,
        vb: VarBuilder,
    ) -&gt; Result&lt;Self, crate::error::PredictorError&gt;;
    // forward(...) lives in TASK-PRED-004
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/predictor/mod.rs">
pub mod transformer_layer;
pub use transformer_layer::TransformerLayer;

use crate::config::PredictorConfig;
use crate::frozen_target::FrozenTargetAdapter;  // declared in TASK-PRED-006; module file may be a placeholder mod with no public items at this stage
use crate::loss::variance_floor::VarianceFloorHistory;  // declared in TASK-PRED-007; placeholder OK
use crate::oracle_head::OracleHead;  // declared in TASK-PRED-002

use candle_core::{DType, Device};
use candle_nn::Linear;

pub struct MeJepaPredictor {
    pub(crate) config: PredictorConfig,
    pub(crate) input_proj: Linear,
    pub(crate) layers: Vec&lt;TransformerLayer&gt;,
    pub(crate) output_proj: Linear,
    pub(crate) oracle_head: OracleHead,
    pub(crate) frozen_target_adapter: FrozenTargetAdapter,
    pub(crate) device: Device,
    pub(crate) dtype: DType,
    pub(crate) variance_floor_history: VarianceFloorHistory,
}

impl MeJepaPredictor {
    pub fn new(
        config: PredictorConfig,
        frozen_target_adapter: FrozenTargetAdapter,
        device: Device,
        num_tests: usize,
    ) -&gt; Result&lt;Self, crate::error::PredictorError&gt;;
    // forward(...) lives in TASK-PRED-005
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/lib.rs">
pub mod config;
pub mod data_models;       // declared in TASK-PRED-002 (placeholder mod here)
pub mod error;             // declared in TASK-PRED-003 (placeholder mod here)
pub mod evidence;          // declared in TASK-PRED-003 (placeholder mod here)
pub mod frozen_target;     // declared in TASK-PRED-006 (placeholder mod here)
pub mod grad_hook;         // declared in TASK-PRED-009 (placeholder mod here)
pub mod loss;              // declared in TASK-PRED-007 (placeholder mod here)
pub mod oracle_head;       // declared in TASK-PRED-002 (placeholder mod here)
pub mod predictor;
pub mod vram;              // declared in TASK-PRED-010 (placeholder mod here)

pub use config::{
    ActivationKind, PredictorConfig,
    PANEL_DIM, CONCAT_INPUT_DIM,
    DEFAULT_NUM_LAYERS, DEFAULT_HIDDEN_DIM, DEFAULT_NUM_HEADS, DEFAULT_FF_EXPANSION,
    DEFAULT_LAMBDA_VAR, DEFAULT_LAMBDA_COV, DEFAULT_LAMBDA_INV, DEFAULT_GAMMA,
    VRAM_STEADY_STATE_TARGET_BYTES, VRAM_WARN_THRESHOLD_BYTES,
    GRAD_NORM_NOISE_FLOOR,
    VICREG_DEGENERATE_DIM_FRACTION, VICREG_DEGENERATE_CONSECUTIVE_PASSES,
    DEFAULT_LAYER_NORM_EPS,
};
pub use predictor::{MeJepaPredictor, TransformerLayer};
    </signature>
    <signature file="crates/context-graph-mejepa/Cargo.toml">
[package]
name = "context-graph-mejepa"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
candle-core = { workspace = true }
candle-nn = { workspace = true }
candle-transformers = { workspace = true }
context-graph-mejepa-instruments = { path = "../context-graph-mejepa-instruments" }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha2 = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
# Unit tests are #[cfg(test)] inline; no extra deps for this task.
    </signature>
  </signatures>
  <constraints>
    - Crate name kebab-case `context-graph-mejepa` per constitution `<naming_conventions>` `<crates>`
    - Files snake_case; types PascalCase; consts SCREAMING_SNAKE per constitution `<naming_conventions>`
    - PANEL_DIM = 5_120 EXACT; CONCAT_INPUT_DIM = 10_240 EXACT (= 2 * PANEL_DIM); do not parameterize
    - Default num_layers = 6; hidden_dim = 1024; num_heads = 8; ff_expansion = 4 (FFN inner = 4096); GeLU only; dropout = 0.0; layer_norm_eps = 1e-5; gradient_checkpointing = false
    - PredictorConfig::validate enforces: num_layers ∈ [1, 12]; hidden_dim % num_heads == 0; activation == GeLU; dropout ∈ [0.0, 0.5]; vicreg_variance_floor_consecutive_passes ∈ [1, 8]
    - MeJepaPredictor::new must (a) call config.validate(), (b) refuse non-Cuda(0) device with PredictorError::DeviceUnavailable (no silent CPU fallback per CLAUDE.md §11.9), (c) set dtype = DType::BF16 on cuda, (d) construct input_proj Linear(10240, hidden_dim), output_proj Linear(hidden_dim, 5120), Vec of num_layers TransformerLayer, OracleHead, (e) assert frozen_target_adapter.parameters().is_empty() — but since FrozenTargetAdapter ships in TASK-PRED-006, the placeholder struct in this task MUST expose `pub fn parameters(&amp;self) -&gt; &amp;[&amp;candle_core::Var]` returning a static empty slice so this task's tests can call it
    - Placeholder modules (`data_models`, `error`, `evidence`, `frozen_target`, `grad_hook`, `loss`, `oracle_head`, `vram`) MUST compile but contain only `// placeholder for TASK-PRED-NNN` plus the minimum shape they need to exist:
        * `error::PredictorError` — declared as a **placeholder enum with a single variant** sufficient for `MeJepaPredictor::new` to return errors; full taxonomy in TASK-PRED-003 (the variants will be **expanded**, not renamed/removed)
        * `oracle_head::OracleHead` — placeholder unit struct + `pub fn new(num_tests: usize, _vb: VarBuilder) -&gt; Result&lt;Self, PredictorError&gt; { Ok(Self) }`; full impl in TASK-PRED-002
        * `frozen_target::FrozenTargetAdapter` — placeholder unit struct + `pub fn parameters(&amp;self) -&gt; &amp;[&amp;Var] { const E: &amp;[&amp;Var] = &amp;[]; E }`; full impl in TASK-PRED-006
        * `loss::variance_floor::VarianceFloorHistory` — placeholder struct + `pub fn new(window: usize) -&gt; Self`; full impl in TASK-PRED-007
        * Other placeholder modules contain only `pub mod foo;` markers as needed
    - No `unwrap()` on Result types in non-test code; map every error into PredictorError
    - No `any` / `dyn Any` in public surface
    - Crate registers in workspace `[workspace.members]` array (alphabetical insertion next to other context-graph-mejepa-* crates)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa` shows >= 5 tests passing (config defaults + validate edges + new() rejects non-Cuda + new() rejects num_layers=0 + PANEL_DIM/CONCAT_INPUT_DIM constants)
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - `cargo build --release --workspace` still succeeds (no other crate broken by workspace member addition)
  </verification>
</definition_of_done>

<pseudo_code>
config.rs:
    declare consts (PANEL_DIM, CONCAT_INPUT_DIM, DEFAULT_*, VRAM_*, GRAD_NORM_NOISE_FLOOR, VICREG_*, DEFAULT_LAYER_NORM_EPS)
    enum ActivationKind { GeLU }
    struct PredictorConfig { all fields per data_models }
    impl Default for PredictorConfig
        num_layers: 6, hidden_dim: 1024, num_heads: 8, ff_expansion: 4,
        activation: GeLU, dropout: 0.0, layer_norm_eps: 1e-5,
        gradient_checkpointing: false,
        vram_warn_threshold_bytes: VRAM_WARN_THRESHOLD_BYTES,
        vicreg_variance_floor_consecutive_passes: 2
    impl PredictorConfig::validate
        check num_layers in [1, 12], hidden_dim % num_heads == 0, activation == GeLU,
        dropout in [0.0, 0.5], consecutive_passes in [1, 8]
        on violation: Err(PredictorError::ConfigInvalid { detail })

predictor/transformer_layer.rs:
    struct TransformerLayer { self_attn_qkv, self_attn_out, ffn_up, ffn_down, attn_ln, ffn_ln, num_heads, head_dim, ff_expansion }
    impl TransformerLayer::new(hidden_dim, num_heads, ff_expansion, layer_norm_eps, vb)
        head_dim = hidden_dim / num_heads (validated by caller)
        ffn_inner = hidden_dim * ff_expansion as u32
        self_attn_qkv = candle_nn::linear(hidden_dim as usize, 3 * hidden_dim as usize, vb.pp("attn_qkv"))?
        self_attn_out = candle_nn::linear(hidden_dim as usize, hidden_dim as usize, vb.pp("attn_out"))?
        ffn_up = candle_nn::linear(hidden_dim as usize, ffn_inner as usize, vb.pp("ffn_up"))?
        ffn_down = candle_nn::linear(ffn_inner as usize, hidden_dim as usize, vb.pp("ffn_down"))?
        attn_ln = candle_nn::layer_norm(hidden_dim as usize, layer_norm_eps as f64, vb.pp("attn_ln"))?
        ffn_ln  = candle_nn::layer_norm(hidden_dim as usize, layer_norm_eps as f64, vb.pp("ffn_ln"))?
        Ok(Self { ... })
    // forward body intentionally absent — TASK-PRED-004

predictor/mod.rs:
    struct MeJepaPredictor { config, input_proj, layers, output_proj, oracle_head, frozen_target_adapter, device, dtype, variance_floor_history }
    impl MeJepaPredictor::new(config, frozen_target_adapter, device, num_tests)
        config.validate()?
        if !matches!(device, Device::Cuda(_)): return Err(DeviceUnavailable)
        dtype = DType::BF16
        // Verify frozen_target_adapter.parameters().is_empty() — runtime invariant per REQ-PRED-09
        debug_assert!(frozen_target_adapter.parameters().is_empty());
        let vm = VarMap::new();
        let vb = VarBuilder::from_varmap(&vm, dtype, &device);
        input_proj = candle_nn::linear(CONCAT_INPUT_DIM, config.hidden_dim as usize, vb.pp("input_proj"))?
        layers = (0..config.num_layers).map(|i| TransformerLayer::new(..., vb.pp(format!("layer_{i}"))))?
        output_proj = candle_nn::linear(config.hidden_dim as usize, PANEL_DIM, vb.pp("output_proj"))?
        oracle_head = OracleHead::new(num_tests, vb.pp("oracle_head"))?
        variance_floor_history = VarianceFloorHistory::new(config.vicreg_variance_floor_consecutive_passes as usize)
        Ok(Self { config, input_proj, layers, output_proj, oracle_head, frozen_target_adapter, device, dtype, variance_floor_history })

lib.rs: declare modules; re-export public surface

placeholder modules:
    error.rs:
        #[derive(Debug, thiserror::Error)]
        pub enum PredictorError {
            #[error("MEJEPA_PRED_CONFIG_INVALID: {detail}")] ConfigInvalid { detail: String },
            #[error("MEJEPA_PRED_DEVICE_UNAVAILABLE: {detail}")] DeviceUnavailable { detail: String },
            #[error("MEJEPA_PRED_CANDLE: {0}")] Candle(#[from] candle_core::Error),
        }
        impl PredictorError { pub fn code(&amp;self) -&gt; &amp;'static str { match ... } }
        // FULL taxonomy expanded in TASK-PRED-003 (additive, no rename)
    oracle_head.rs:
        pub struct OracleHead;
        impl OracleHead { pub fn new(_num_tests: usize, _vb: VarBuilder) -&gt; Result&lt;Self, PredictorError&gt; { Ok(Self) } }
    frozen_target.rs:
        pub struct FrozenTargetAdapter;
        impl FrozenTargetAdapter {
            pub fn empty() -&gt; Self { Self }
            pub fn parameters(&amp;self) -&gt; &amp;[&amp;candle_core::Var] { const E: &amp;[&amp;candle_core::Var] = &amp;[]; E }
        }
    loss/mod.rs: pub mod variance_floor;
    loss/variance_floor.rs:
        pub struct VarianceFloorHistory { window: usize }
        impl VarianceFloorHistory { pub fn new(window: usize) -&gt; Self { Self { window } } }

tests in config.rs / predictor/mod.rs:
    test default values (num_layers=6, hidden_dim=1024, etc.)
    test PANEL_DIM = 5120 and CONCAT_INPUT_DIM = 10240
    test validate rejects num_layers=0 and num_layers=13
    test validate rejects hidden_dim=1023 (not divisible by 8 heads)
    test new() rejects Device::Cpu with DeviceUnavailable
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/Cargo.toml">Crate manifest with workspace deps + path dep on context-graph-mejepa-instruments</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">Module declarations + public re-exports</file>
  <file path="crates/context-graph-mejepa/src/config.rs">Constants + ActivationKind + PredictorConfig + Default + validate</file>
  <file path="crates/context-graph-mejepa/src/predictor/mod.rs">MeJepaPredictor struct + new() with validation</file>
  <file path="crates/context-graph-mejepa/src/predictor/transformer_layer.rs">TransformerLayer struct + new() with weight allocation</file>
  <file path="crates/context-graph-mejepa/src/error.rs">Placeholder PredictorError enum (3 variants; expanded in TASK-PRED-003)</file>
  <file path="crates/context-graph-mejepa/src/oracle_head.rs">Placeholder OracleHead unit struct + new()</file>
  <file path="crates/context-graph-mejepa/src/frozen_target.rs">Placeholder FrozenTargetAdapter unit struct + parameters() empty slice</file>
  <file path="crates/context-graph-mejepa/src/loss/mod.rs">`pub mod variance_floor;` (rest of loss surface in TASK-PRED-007)</file>
  <file path="crates/context-graph-mejepa/src/loss/variance_floor.rs">Placeholder VarianceFloorHistory struct + new()</file>
  <file path="crates/context-graph-mejepa/src/data_models.rs">Empty placeholder file (filled in TASK-PRED-002)</file>
  <file path="crates/context-graph-mejepa/src/evidence.rs">Empty placeholder file (filled in TASK-PRED-003)</file>
  <file path="crates/context-graph-mejepa/src/grad_hook.rs">Empty placeholder file (filled in TASK-PRED-009)</file>
  <file path="crates/context-graph-mejepa/src/vram.rs">Empty placeholder file (filled in TASK-PRED-010)</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml">Add `crates/context-graph-mejepa` to `[workspace.members]` (alphabetical position next to other context-graph-mejepa-* crates)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo build --release --workspace succeeds (no regression to existing crates)</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 5 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>PredictorConfig::default() matches doc 09 §2.4 + REQ-PRED-02</criterion>
  <criterion>PredictorConfig::validate rejects out-of-range values with PredictorError::ConfigInvalid</criterion>
  <criterion>MeJepaPredictor::new rejects Device::Cpu with DeviceUnavailable (no silent fallback)</criterion>
  <criterion>FrozenTargetAdapter placeholder.parameters().is_empty() returns true (REQ-PRED-09 prerequisite)</criterion>
  <criterion>PANEL_DIM == 5_120 and CONCAT_INPUT_DIM == 10_240 are public consts</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo build --release --workspace</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

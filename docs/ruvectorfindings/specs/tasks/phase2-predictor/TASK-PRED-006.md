# TASK-PRED-006 — FrozenTargetAdapter (parameters() returns &[]; encode_target with detach; deterministic)

```xml
<task_spec id="TASK-PRED-006" version="1.0">

<metadata>
  <title>Replace placeholder FrozenTargetAdapter with full impl: zero-trainable-parameter wrapper around 15 instruments + encode_target with detach</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-PRED-09">parameters() returns &amp;[] by construction; trait API forbids &amp;mut self weight mutation</item>
    <item ref="REQ-PRED-11">Determinism: same (input, code_version, embedder_versions) → byte-equal target panel</item>
    <item ref="component:FrozenTargetAdapter">struct + new + parameters + encode_target + provenance accessors</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-002</task_ref>
    <task_ref>TASK-PRED-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified</estimated_files>
</metadata>

<context>
The FrozenTargetAdapter is the architectural keystone of `ΔΞ.target_collapse = 0 by construction`
(constitution `<utml_v1_invariants>` rule 2). It wraps a `Vec<Box<dyn Instrument + Send + Sync>>`
loaded from `context-graph-mejepa-instruments`, exposes ZERO trainable parameters
(`parameters()` returns a static empty slice), and provides `encode_target(&Panel) -> TargetPanel`
that converts a frozen-target Panel (5,120-d float vector) into a candle Tensor on the predictor's
device with `requires_grad = false` and explicit `.detach()`.

The trait API in `context-graph-mejepa-instruments::Instrument` (per Phase 1c TASK-INSTR-002 and
TECH-INSTRUMENTS) MUST already forbid `&mut self` weight-mutating methods; this task does NOT
modify that crate. We rely on the type-level guarantee plus a runtime gradient hook (TASK-PRED-009).

Defense-in-depth: `encode_target` calls `tensor.detach()?` after constructing the Tensor from the
Panel's f32 slice — even if a downstream caller managed to acquire a `&Tensor` and call
`.requires_grad_(true)`, the prior detach severs the autograd graph. This is belt-and-suspenders
with the gradient hook.

Iron Rule observed: this task does NOT call MeJepaPredictor (TASK-PRED-005), does NOT call
vicreg_loss (TASK-PRED-007), does NOT call gradient hook (TASK-PRED-009).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; anti_patterns "no learnable target encoder"; provenance rule)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-09, REQ-PRED-11; AC-PRED-03-01, AC-PRED-03-03; data_requirements TargetPanel + TargetProvenance; EC-PRED-04 detach escape hatch)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (data_models FrozenTargetAdapter / TargetProvenance; component_contracts FrozenTargetAdapter; implementation_notes "frozen-target-architecture")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-002.md (TargetPanel + TargetProvenance data models)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (PredictorError taxonomy)</file>
  <file purpose="upstream_trait">crates/context-graph-mejepa-instruments/src/lib.rs (Instrument trait + Panel + PANEL_DIM = 5_120)</file>
  <file purpose="upstream_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel::data() returns &amp;[f32] of length PANEL_DIM)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/frozen_target.rs (placeholder unit struct from TASK-PRED-001)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-002 + TASK-PRED-003 complete (data_models + error taxonomy ready)</check>
  <check>cargo build --release -p context-graph-mejepa succeeds before this task</check>
  <check>context-graph-mejepa-instruments exports Panel + Instrument trait (verified by Phase 1c TASK-INSTR-001..003)</check>
  <check>The Instrument trait has NO `&amp;mut self` weight-mutating methods (precondition; verify via Phase 1c TASK-INSTR-002)</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace placeholder `FrozenTargetAdapter` unit struct with full struct holding `instruments: Vec<Box<dyn Instrument + Send + Sync>>` + `provenance: TargetProvenance`
    - Implement `pub fn new(instruments, provenance) -> Self` (no validation needed; the trait API IS the guarantee)
    - Implement `pub fn parameters(&self) -> &[&Var]` returning a `const EMPTY: &[&Var] = &[];`
    - Implement `pub fn encode_target(&self, panel_t2: &Panel, device: &Device, dtype: DType) -> Result<TargetPanel, PredictorError>` (panel dim validation + Tensor construction + detach)
    - Implement `pub fn provenance(&self) -> &TargetProvenance` accessor
    - Implement `pub fn instrument_count(&self) -> usize` accessor
    - Implement `pub fn target_sha256(&self, panel_t2: &Panel) -> Result<String, PredictorError>` — SHA-256 of bf16 byte representation for determinism FSV
    - Add `pub fn empty_for_test() -> Self` test-helper that constructs an adapter with zero instruments and a stub TargetProvenance (used by TASK-PRED-001 already; reaffirmed here)
    - Add 7 unit tests: parameters_is_empty; encode_target preserves dim; encode_target rejects wrong panel dim; determinism (two encodes byte-equal SHA-256); detach severs grad graph (best-effort: assert tensor.detach() can be called); provenance accessor returns the value passed to new(); instrument_count
  </in_scope>
  <out_of_scope>
    - Gradient hook (TASK-PRED-009) — separate concern; this task does NOT call grad_hook code
    - Materialization of panel[t=2] from instrument outputs (Phase 1c TASK-INSTR-020 `materialize_panel`)
    - Per-instrument FSV (Phase 1c)
    - VRAM measurement (TASK-PRED-010)
    - VICReg loss (TASK-PRED-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/frozen_target.rs">
use candle_core::{DType, Device, Tensor, Var};
use context_graph_mejepa_instruments::{Instrument, Panel, PANEL_DIM as INSTR_PANEL_DIM};
use sha2::{Digest, Sha256};

use crate::config::PANEL_DIM;
use crate::data_models::{TargetPanel, TargetProvenance};
use crate::error::{NanSource, PanelSide, PredictorError};

const _: () = assert!(PANEL_DIM == INSTR_PANEL_DIM, "panel dim must match upstream instruments crate");

pub struct FrozenTargetAdapter {
    instruments: Vec&lt;Box&lt;dyn Instrument + Send + Sync&gt;&gt;,
    provenance: TargetProvenance,
}

impl FrozenTargetAdapter {
    pub fn new(instruments: Vec&lt;Box&lt;dyn Instrument + Send + Sync&gt;&gt;, provenance: TargetProvenance) -&gt; Self;

    /// Returns the static empty slice. This is the type-level guarantee that AdamW
    /// cannot register any frozen-target state for training (REQ-PRED-09).
    pub fn parameters(&amp;self) -&gt; &amp;[&amp;Var];

    pub fn provenance(&amp;self) -&gt; &amp;TargetProvenance;

    pub fn instrument_count(&amp;self) -&gt; usize;

    /// Convert a frozen-target Panel into a Tensor on the predictor's device with
    /// `requires_grad = false` and explicit `.detach()`.
    pub fn encode_target(
        &amp;self,
        panel_t2: &amp;Panel,
        device: &amp;Device,
        dtype: DType,
    ) -&gt; Result&lt;TargetPanel, PredictorError&gt;;

    /// SHA-256 hex of the encoded target's byte representation. Used by FSV (TASK-PRED-011)
    /// to verify determinism per REQ-PRED-11.
    pub fn target_sha256(
        &amp;self,
        panel_t2: &amp;Panel,
        device: &amp;Device,
        dtype: DType,
    ) -&gt; Result&lt;String, PredictorError&gt;;

    /// Test-only constructor with empty instruments + stub provenance.
    pub fn empty_for_test() -&gt; Self;
}

// NB: there are NO `&amp;mut self` methods that touch instrument weights. This is the type-level
// guarantee. Adding such a method is a CRITICAL spec violation per the constitution
// `<utml_v1_invariants>` rule 2.
    </signature>
  </signatures>
  <constraints>
    - Static empty slice: `const EMPTY: &amp;[&amp;Var] = &amp;[]; EMPTY` returned by `parameters()` — exactly per TECH-PREDICTOR's `parameters` body
    - `encode_target` validates `panel_t2.data().len() == PANEL_DIM (5120)`; on mismatch return `PredictorError::DimMismatch { detail: "panel_t2 wrong dim", observed: ..., expected_panel_dim: PANEL_DIM }`
    - Tensor construction: `Tensor::from_slice(panel_t2.data(), (1, PANEL_DIM), device)?` then `.to_dtype(dtype)?` then `.detach()?`
    - The (1, PANEL_DIM) shape is the per-sample batch=1 representation; the training loop will batch externally via `Tensor::cat` over multiple TargetPanels
    - `requires_grad = false` is the candle default for tensors-from-slice; `.detach()` is the explicit defense-in-depth call
    - `target_sha256` extracts the bf16 byte representation: `tensor.to_dtype(DType::BF16)?.to_vec1::<half::bf16>()?` then `Sha256` hash hex digest; returns `String` (lowercase hex)
    - Wait: candle `to_vec1` is for 1-D; for our (1, PANEL_DIM) we use `tensor.flatten_all()?.to_vec1::<f32>()?` (cast to f32 for stable hashing across bf16-cuda variant) — this trades exact bf16 hashing for portability; FSV evidence file records `dtype_for_hash="f32"` to make this transparent
    - PROVENANCE: `provenance` is owned; the accessor returns `&TargetProvenance` (no clone)
    - `empty_for_test()` constructs `FrozenTargetAdapter { instruments: vec![], provenance: TargetProvenance::new("test", BTreeMap::new(), "1970-01-01T00:00:00Z") }`
    - Determinism: same panel_t2.data() + same device + same dtype must produce byte-equal target_sha256; this is verified by tests
    - Compile-time check: `const _: () = assert!(PANEL_DIM == context_graph_mejepa_instruments::PANEL_DIM);` — ensures upstream crate's PANEL_DIM stays in sync
    - File length stays &lt; 500 lines (constitution `<file_organization>`)
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 33 tests passing (this task adds 7)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Static assertion `PANEL_DIM == upstream PANEL_DIM` compiles successfully (compile-time assertion failure would BLOCK build)
    - Test (cuda-gated): encode_target with a Panel of correct dim returns TargetPanel of shape (1, 5120) bf16
    - Test: encode_target with Panel.data().len() != 5120 → Err(PredictorError::DimMismatch)
    - Test: parameters().is_empty() == true; parameters().len() == 0
    - Test: two consecutive encode_target on same inputs → byte-equal target_sha256
    - Test: provenance() returns the same struct passed to new()
  </verification>
</definition_of_done>

<pseudo_code>
const EMPTY_VARS: &[&Var] = &[];

impl FrozenTargetAdapter {
    pub fn new(instruments, provenance) -&gt; Self {
        Self { instruments, provenance }
    }

    pub fn parameters(&self) -&gt; &[&Var] { EMPTY_VARS }

    pub fn provenance(&self) -&gt; &TargetProvenance { &self.provenance }

    pub fn instrument_count(&self) -&gt; usize { self.instruments.len() }

    pub fn encode_target(&self, panel_t2, device, dtype) -&gt; Result&lt;TargetPanel, PredictorError&gt; {
        let data = panel_t2.data();
        if data.len() != PANEL_DIM {
            return Err(PredictorError::DimMismatch {
                detail: format!("panel_t2 has {} dims, expected {}", data.len(), PANEL_DIM),
                observed: btreemap!{"panel_t2" =&gt; format!("[{}]", data.len())},
                expected_panel_dim: PANEL_DIM,
            });
        }
        let tensor = Tensor::from_slice(data, (1, PANEL_DIM), device)?;
        let tensor = tensor.to_dtype(dtype)?;
        let tensor = tensor.detach()?;  // belt-and-suspenders
        Ok(TargetPanel { data: tensor, provenance_ref: self.provenance.code_version.clone() })
    }

    pub fn target_sha256(&self, panel_t2, device, dtype) -&gt; Result&lt;String, PredictorError&gt; {
        let target = self.encode_target(panel_t2, device, dtype)?;
        let bytes_f32: Vec&lt;f32&gt; = target.data
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1()?;
        let mut hasher = Sha256::new();
        for f in &bytes_f32 {
            hasher.update(f.to_le_bytes());
        }
        Ok(hex::encode(hasher.finalize()))  // or manually format hex if hex crate not available
    }

    pub fn empty_for_test() -&gt; Self {
        let provenance = TargetProvenance::new("test", BTreeMap::new(), "1970-01-01T00:00:00Z");
        Self { instruments: Vec::new(), provenance }
    }
}

// tests:
//   test_parameters_is_empty
//   test_encode_target_correct_dim_returns_ok    (cuda-gated)
//   test_encode_target_wrong_dim_returns_err
//   test_target_sha256_deterministic              (cuda-gated; two calls byte-equal)
//   test_provenance_accessor_returns_struct
//   test_instrument_count
//   test_empty_for_test_constructs_zero_instruments_zero_params
</pseudo_code>

<files_to_create>
  <!-- none; modifying placeholder from TASK-PRED-001 -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/frozen_target.rs">Replace placeholder unit struct with full impl: instruments + provenance fields; new + parameters + provenance + instrument_count + encode_target + target_sha256 + empty_for_test; add 7 unit tests + compile-time PANEL_DIM cross-check</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">Add `pub use frozen_target::FrozenTargetAdapter;` to public re-exports if not already there from TASK-PRED-001</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">Add `hex = "0.4"` to [dependencies] if not already in workspace deps (used for SHA-256 hex encoding); falls back to manual hex formatting if `hex` is forbidden by workspace-deps policy</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 33 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>FrozenTargetAdapter::parameters() returns an empty slice (REQ-PRED-09)</criterion>
  <criterion>encode_target with correct-dim Panel returns TargetPanel of shape (1, 5120) bf16</criterion>
  <criterion>encode_target with wrong-dim Panel returns Err(PredictorError::DimMismatch)</criterion>
  <criterion>target_sha256 is deterministic across two calls (REQ-PRED-11)</criterion>
  <criterion>Compile-time assertion PANEL_DIM == upstream PANEL_DIM holds</criterion>
  <criterion>The struct has NO `&amp;mut self` methods that touch instrument weights (visual code review part of validation)</criterion>
  <criterion>provenance() returns &amp;TargetProvenance (no clone)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa frozen_target</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

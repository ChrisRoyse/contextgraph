// Phase 1 instruments + 5,120-d panel scaffolding for ME-JEPA-Code.
// Per `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.2`
// (instrument inventory) and `§3.5.5` (5,120-d panel after E_StaticAnalysis,
// E_Runtime, E_Reasoning are added).
//
// This crate ships:
//   - `Instrument` trait — every E_* concrete instrument implements it.
//   - `InstrumentSlot` enum — 15 named slots with byte-precise offsets
//     into the panel.
//   - `Panel` and `PanelBuilder` — a fail-closed 5,120-d joint embedding
//     assembled from individual instrument outputs with dim validation.
//   - `e_oracle::EOracleInstrument` — test-outcome + exception-class encoder.
//   - `e_witness::EWitnessInstrument` — verified witness-chain encoder.
//   - `e_static_analysis::EStaticAnalysisInstrument` — linter/type/source
//     signal encoder.
//   - `e_code`, `e_text`, `e_trace`, `e_runtime`, `e_reasoning`, and
//     `e_scalars` — deterministic typed encoders for the remaining panel
//     slots. These are real source-of-truth encoders, not silent zero-fill
//     placeholders; neural/embedder-backed replacements can ship later behind
//     the same fail-closed trait contract.
//
// Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`
// — clean-room. No upstream code copied.
//
// Fail-closed everywhere: empty input, dim mismatch on `set_slot`,
// non-finite input or output, slot-already-filled, panel-build with no
// slots, malformed persisted panel state, etc. all return
// `MEJEPA_INSTRUMENTS_INVALID_INPUT` or `_NUMERICAL_INVARIANT`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod e_code;
pub mod e_oracle;
pub mod e_reasoning;
pub mod e_runtime;
pub mod e_scalars;
pub mod e_static_analysis;
pub mod e_text;
pub mod e_trace;
pub mod e_witness;
mod features;
pub mod frozen_hook;
pub mod materialize;
pub mod panel_b;
pub mod panel_json;
pub mod panel_store;
pub mod q4_accuracy_labels;
pub mod q4_confidence_calibration;
pub mod q4_cost_labels;
pub mod q4_pass_annotations;
pub mod q4_perf_labels;
pub mod q4_reasoning_labels;
pub mod q4_security_labels;

pub type InstrumentResult<T> = Result<T, InstrumentError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum InstrumentError {
    #[error("instrument input invalid at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "instrument numerical invariant failed at {field}: {message}; remediation: {remediation}"
    )]
    NumericalInvariant {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "frozen instrument target violation at {field}: {message}; remediation: {remediation}"
    )]
    FrozenViolation {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "instrument store error during {operation} on {cf}: {message}; remediation: {remediation}"
    )]
    Store {
        operation: &'static str,
        cf: &'static str,
        message: String,
        remediation: &'static str,
    },
    /// #710: a slot vector passed to `set_slot_with_health_check` is
    /// degenerate (strict-constant, all elements equal). Defense in depth
    /// against upstream embedder cache collapse (#666 / #704 / #707).
    #[error(
        "degenerate slot vector at {field}: {message}; remediation: {remediation}"
    )]
    DegenerateSlot {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
}

impl InstrumentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_INSTRUMENTS_INVALID_INPUT",
            Self::NumericalInvariant { .. } => "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT",
            Self::FrozenViolation { .. } => "MEJEPA_INSTR_FROZEN_VIOLATION",
            Self::Store { .. } => "MEJEPA_INSTRUMENTS_STORE_ERROR",
            Self::DegenerateSlot { .. } => "DEGENERATE_SLOT_VECTOR",
        }
    }

    pub(crate) fn invalid(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::InvalidInput {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn invariant(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::NumericalInvariant {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn frozen_violation(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::FrozenViolation {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn store(
        operation: &'static str,
        cf: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::Store {
            operation,
            cf,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn degenerate_slot(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::DegenerateSlot {
            field,
            message: message.into(),
            remediation,
        }
    }
}

/// Total dimension of the joint embedding panel. Per doc 09 §3.5.5.
pub const PANEL_DIM: usize = 5_120;
const SLOT_COUNT: usize = 15;
const VALID_FILLED_MASK: u16 = (1u16 << SLOT_COUNT) - 1;

/// Named instrument slots in the panel. Dim and offset per slot are fixed
/// across every panel; the table below MUST match `Self::layout()` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentSlot {
    /// `E_AST` — tree-sitter AST encoder. 384-d.
    EAst,
    /// `E_CFG` — control-flow graph attention encoder. 256-d.
    ECfg,
    /// `E_DataFlow` — def-use graph attention encoder. 256-d.
    EDataFlow,
    /// `E_TypeGraph` — type signature + inferred-type graph encoder. 256-d.
    ETypeGraph,
    /// `E_Test` — reuses `E1` (e5-large-v2) on test descriptions. 384-d.
    ETest,
    /// `E_Trace` — execution-trace sequence transformer. 512-d.
    ETrace,
    /// `E_Diff` — AST-diff edge encoder. 256-d.
    EDiff,
    /// `E_Witness` — witness-chain entry sequence encoder. 256-d.
    EWitness,
    /// `E_Oracle` — one-hot test outcomes + exception class. 128-d.
    EOracle,
    /// `E_Problem` — reuses `E1` on the SWE-bench problem text. 1024-d.
    EProblem,
    /// `E_CommitMsg` — reuses `E1` on the candidate fix's commit msg. 384-d.
    ECommitMsg,
    /// `E_StaticAnalysis` — linter / typecheck / complexity aggregates. 256-d.
    EStaticAnalysis,
    /// `E_Runtime` — wall-clock / RSS / coverage / network signal aggregates. 256-d.
    ERuntime,
    /// `E_Reasoning` — reasoning/reflexion-trace encoder. 384-d.
    EReasoning,
    /// Scalar feature aggregates: BFS depth, blame age, churn, coverage Δ,
    /// repo-health metrics. 128-d. Per doc 09 §3.5.5.
    Scalars,
}

impl InstrumentSlot {
    /// Byte-precise layout: each slot's `(byte offset, dim)` in the panel.
    /// Order MUST match the enum-variant order so `all()` indexes line up.
    pub fn layout() -> [(Self, usize, usize); 15] {
        let mut offset = 0usize;
        let mut layout: [(Self, usize, usize); 15] = [(Self::EAst, 0, 0); 15];
        for (i, (slot, dim)) in [
            (Self::EAst, 384),
            (Self::ECfg, 256),
            (Self::EDataFlow, 256),
            (Self::ETypeGraph, 256),
            (Self::ETest, 384),
            (Self::ETrace, 512),
            (Self::EDiff, 256),
            (Self::EWitness, 256),
            (Self::EOracle, 128),
            (Self::EProblem, 1024),
            (Self::ECommitMsg, 384),
            (Self::EStaticAnalysis, 256),
            (Self::ERuntime, 256),
            (Self::EReasoning, 384),
            (Self::Scalars, 128),
        ]
        .iter()
        .enumerate()
        {
            layout[i] = (*slot, offset, *dim);
            offset += dim;
        }
        debug_assert_eq!(offset, PANEL_DIM);
        layout
    }

    /// All 15 slots in canonical doc-09 order.
    pub fn all() -> [Self; 15] {
        Self::layout().map(|(s, _, _)| s)
    }

    /// Return this slot's `(offset, dim)` in the panel.
    pub fn extent(&self) -> (usize, usize) {
        for (slot, off, dim) in Self::layout() {
            if slot == *self {
                return (off, dim);
            }
        }
        unreachable!("InstrumentSlot::layout missing variant {:?}", self);
    }

    pub fn offset(&self) -> usize {
        self.extent().0
    }

    pub fn dim(&self) -> usize {
        self.extent().1
    }

    /// Stable identifier for use in JSON, RocksDB keys, etc.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::EAst => "e_ast",
            Self::ECfg => "e_cfg",
            Self::EDataFlow => "e_data_flow",
            Self::ETypeGraph => "e_type_graph",
            Self::ETest => "e_test",
            Self::ETrace => "e_trace",
            Self::EDiff => "e_diff",
            Self::EWitness => "e_witness",
            Self::EOracle => "e_oracle",
            Self::EProblem => "e_problem",
            Self::ECommitMsg => "e_commit_msg",
            Self::EStaticAnalysis => "e_static_analysis",
            Self::ERuntime => "e_runtime",
            Self::EReasoning => "e_reasoning",
            Self::Scalars => "scalars",
        }
    }
}

/// Every E_* instrument in `crates/context-graph-mejepa-instruments` MUST
/// implement this trait. Encoded vectors are validated against the slot's
/// declared `dim` by `PanelBuilder::set_slot`.
pub trait Instrument {
    /// Concrete input type — e.g. `OracleVerdict` for `E_Oracle`.
    type Input;

    /// The slot this instrument owns.
    fn slot(&self) -> InstrumentSlot;

    /// Declared output dimension. MUST equal `self.slot().dim()`.
    fn output_dim(&self) -> usize {
        self.slot().dim()
    }

    /// Produce the latent vector for `input`. Length MUST equal
    /// `self.output_dim()`. Non-finite values MUST cause a fail-closed
    /// error.
    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>>;
}

/// One panel of the joint embedding. Materialized via `PanelBuilder` so
/// dim mismatches and double-fills surface fail-closed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PanelSerde", into = "PanelSerde")]
pub struct Panel {
    data: Vec<f32>,
    /// Bitmask: bit `i` (in `InstrumentSlot::all()` order) is set iff
    /// that slot was filled by `set_slot`. Bits not set are zero-filled.
    filled_mask: u16,
}

impl Panel {
    /// Construct a panel from persisted or externally assembled state.
    /// This is the only public constructor so every panel instance keeps
    /// the same invariants that `PanelBuilder` enforces.
    pub fn try_new(data: Vec<f32>, filled_mask: u16) -> InstrumentResult<Self> {
        validate_panel_parts(&data, filled_mask)?;
        Ok(Self { data, filled_mask })
    }

    /// Full 5,120-d panel data.
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    /// Filled-slot bitmask in `InstrumentSlot::all()` order.
    pub fn filled_mask(&self) -> u16 {
        self.filled_mask
    }

    /// View the slice belonging to a slot.
    pub fn slot(&self, slot: InstrumentSlot) -> &[f32] {
        let (offset, dim) = slot.extent();
        &self.data[offset..offset + dim]
    }

    /// Was this slot filled (via `set_slot`) when the panel was built?
    pub fn is_filled(&self, slot: InstrumentSlot) -> bool {
        let bit = slot_bit(slot);
        self.filled_mask & bit != 0
    }

    /// #710: post-hoc check — is the slot's current vector strict-constant?
    /// Useful for downstream consumers that need to verify health of a
    /// panel built via the permissive `set_slot` path. Returns `false`
    /// for unfilled slots (they hold uninitialized zeros, but the
    /// answer "is this signal" is undefined; check `is_filled` first).
    pub fn is_degenerate(&self, slot: InstrumentSlot) -> bool {
        if !self.is_filled(slot) {
            return false;
        }
        is_strict_constant_slice(self.slot(slot))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelSerde {
    data: Vec<f32>,
    filled_mask: u16,
}

impl From<Panel> for PanelSerde {
    fn from(panel: Panel) -> Self {
        Self {
            data: panel.data,
            filled_mask: panel.filled_mask,
        }
    }
}

impl TryFrom<PanelSerde> for Panel {
    type Error = InstrumentError;

    fn try_from(value: PanelSerde) -> Result<Self, Self::Error> {
        Self::try_new(value.data, value.filled_mask)
    }
}

#[derive(Debug, Clone)]
pub struct PanelBuilder {
    data: Vec<f32>,
    filled_mask: u16,
}

impl Default for PanelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelBuilder {
    pub fn new() -> Self {
        Self {
            data: vec![0.0; PANEL_DIM],
            filled_mask: 0,
        }
    }

    /// Write `vector` into `slot`. Errors fail-closed if:
    /// - `vector.len() != slot.dim()`
    /// - any element is non-finite
    /// - the slot was already filled
    pub fn set_slot(&mut self, slot: InstrumentSlot, vector: &[f32]) -> InstrumentResult<()> {
        let (offset, dim) = slot.extent();
        if vector.len() != dim {
            return Err(InstrumentError::invalid(
                "vector",
                format!(
                    "vector length {} does not match slot {:?}'s dim {}",
                    vector.len(),
                    slot,
                    dim
                ),
                "supply a vector of exactly slot.dim() elements",
            ));
        }
        for (i, v) in vector.iter().enumerate() {
            if !v.is_finite() {
                return Err(InstrumentError::invariant(
                    "vector",
                    format!("vector[{i}] is non-finite: {v} (slot {slot:?})"),
                    "scrub NaN/Inf from the instrument output before set_slot",
                ));
            }
        }
        let bit = slot_bit(slot);
        if self.filled_mask & bit != 0 {
            return Err(InstrumentError::invalid(
                "slot",
                format!("slot {slot:?} already filled"),
                "do not call set_slot twice for the same slot",
            ));
        }
        self.data[offset..offset + dim].copy_from_slice(vector);
        self.filled_mask |= bit;
        Ok(())
    }

    /// #710: defense-in-depth at the panel-build layer. Identical to
    /// `set_slot` plus a strict-constant-vector rejection. If every
    /// element of `vector` is bit-equal, returns `DEGENERATE_SLOT_VECTOR`.
    /// This catches the canonical E2/E3/E4 collapse (#666 / #704 / #707)
    /// at construction time, before any prediction is made.
    ///
    /// Use this method on the production inference path (compiler.rs);
    /// keep the permissive `set_slot` for synthetic tests that legitimately
    /// fill placeholder zeros.
    pub fn set_slot_with_health_check(
        &mut self,
        slot: InstrumentSlot,
        vector: &[f32],
    ) -> InstrumentResult<()> {
        if is_strict_constant_slice(vector) {
            return Err(InstrumentError::degenerate_slot(
                "vector",
                format!(
                    "slot {:?} vector is strict-constant ({} dims all equal {}); rejected as degenerate",
                    slot,
                    vector.len(),
                    vector.first().copied().unwrap_or(f32::NAN)
                ),
                "investigate the upstream embedder cache for collapse (#666 / #704); pass a non-constant vector",
            ));
        }
        self.set_slot(slot, vector)
    }

    /// Convenience: encode `input` via `instrument` and write the result
    /// into the matching slot.
    pub fn set_from_instrument<I, In>(&mut self, instrument: &I, input: &In) -> InstrumentResult<()>
    where
        I: Instrument<Input = In>,
    {
        let vector = instrument.encode(input)?;
        if vector.len() != instrument.output_dim() {
            return Err(InstrumentError::invariant(
                "instrument.encode",
                format!(
                    "instrument {:?} produced vector of length {} but declared output_dim = {}",
                    instrument.slot(),
                    vector.len(),
                    instrument.output_dim()
                ),
                "instrument implementations MUST honor their declared output_dim",
            ));
        }
        self.set_slot(instrument.slot(), &vector)
    }

    /// Finalize the panel. Zero-filled slots remain zero; the `filled_mask`
    /// records which slots were explicitly set. Building with no filled
    /// slots is rejected because it is indistinguishable from missing
    /// instrument evidence.
    pub fn build(self) -> InstrumentResult<Panel> {
        Panel::try_new(self.data, self.filled_mask)
    }

    /// Optional check: assert each of `required_slots` is filled. Errors
    /// fail-closed listing the first missing slot. Use this when building
    /// `panel[t=2]` per doc 09 §2.3, where every slot is expected.
    pub fn require_slots(&self, required_slots: &[InstrumentSlot]) -> InstrumentResult<()> {
        if required_slots.is_empty() {
            return Err(InstrumentError::invalid(
                "required_slots",
                "required slot list is empty",
                "pass at least one required slot or skip require_slots explicitly",
            ));
        }
        for slot in required_slots {
            let bit = slot_bit(*slot);
            if self.filled_mask & bit == 0 {
                return Err(InstrumentError::invalid(
                    "panel",
                    format!("required slot {slot:?} was not filled"),
                    "fill every required slot before calling require_slots / build",
                ));
            }
        }
        Ok(())
    }
}

fn validate_panel_parts(data: &[f32], filled_mask: u16) -> InstrumentResult<()> {
    if data.len() != PANEL_DIM {
        return Err(InstrumentError::invalid(
            "panel.data",
            format!(
                "panel data length {} does not match PANEL_DIM {PANEL_DIM}",
                data.len()
            ),
            "persist and read exactly PANEL_DIM f32 values for every ME-JEPA panel",
        ));
    }
    if filled_mask == 0 {
        return Err(InstrumentError::invalid(
            "panel.filled_mask",
            "panel has no filled instrument slots",
            "fill at least one instrument slot before persisting a panel",
        ));
    }
    let unknown_bits = filled_mask & !VALID_FILLED_MASK;
    if unknown_bits != 0 {
        return Err(InstrumentError::invalid(
            "panel.filled_mask",
            format!("filled_mask contains unknown slot bits: 0x{unknown_bits:04x}"),
            "clear bits outside InstrumentSlot::all() before persisting a panel",
        ));
    }
    for (i, value) in data.iter().enumerate() {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "panel.data",
                format!("panel.data[{i}] is non-finite: {value}"),
                "do not persist NaN/Inf in ME-JEPA panels",
            ));
        }
    }
    Ok(())
}

fn slot_bit(slot: InstrumentSlot) -> u16 {
    let idx = InstrumentSlot::all()
        .iter()
        .position(|s| *s == slot)
        .expect("InstrumentSlot::all is exhaustive");
    1u16 << idx
}

/// #710: strict-constant detector. Returns `true` iff every element of
/// `values` is bit-equal to the first element. Empty slices return
/// `false` (no constant-vector content to flag). Uses `to_bits()` so
/// NaN representations compare correctly.
fn is_strict_constant_slice(values: &[f32]) -> bool {
    let Some(first) = values.first() else {
        return false;
    };
    let first_bits = first.to_bits();
    values.iter().all(|v| v.to_bits() == first_bits) && values.len() > 1
}

#[cfg(test)]
#[path = "tests.rs"]
mod instrument_tests;

pub use e_code::{
    CodeInstrumentInput, DiffInstrumentInput, EAstInstrument, ECfgInstrument, EDataFlowInstrument,
    EDiffInstrument, ETypeGraphInstrument,
};
pub use e_oracle::{EOracleInstrument, ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome};
pub use e_reasoning::{EReasoningInstrument, ReasoningEvent, ReasoningInput};
pub use e_runtime::{ERuntimeInstrument, RuntimeInput};
pub use e_scalars::{ScalarInput, ScalarsInstrument};
pub use e_static_analysis::{
    Diagnostic, DiagnosticSeverity, EStaticAnalysisInstrument, StaticAnalysisInput,
};
pub use e_text::{ECommitMsgInstrument, EProblemInstrument, ETestInstrument, TextInstrumentInput};
pub use e_trace::{ETraceInstrument, TraceEvent, TraceEventKind, TraceInput};
pub use e_witness::{EWitnessInstrument, WitnessChainInput, CANONICAL_WITNESS_FORMAT_VERSION};
pub use frozen_hook::{hash_f32s, FrozenGuard, FrozenSnapshot};
pub use materialize::{
    active_slots, materialize_panel, vectors_from_panel, zero_slots, MaterializedPanel,
    PanelVectorInput, TimeStep,
};
pub use panel_b::{
    default_panel_b_artifact_specs, require_panel_b_artifacts, resolve_panel_b_artifact_manifest,
    validate_aiwonder_models_root, MissingPanelBArtifact, PanelBArtifact, PanelBArtifactManifest,
    PanelBArtifactSpec, PanelBSlot, PANEL_B_ARTIFACT_SCHEMA_VERSION,
};
pub use panel_json::{PanelEnvelope, PanelProvenance, PANEL_JSON_SCHEMA_VERSION};
pub use panel_store::{
    PanelKey, PanelMeta, PanelStore, CF_MEJEPA_PANELS, CF_MEJEPA_PANEL_META, MEJEPA_PANEL_CFS,
};
pub use q4_accuracy_labels::{
    extract_q4_accuracy_labels, run_q4_accuracy_tools_with_timeout, write_q4_accuracy_raw_outputs,
    PersistedQ4AccuracySignal, Q4AccuracyCommandSpec, Q4AccuracyExtraction, Q4AccuracyLabel,
    Q4AccuracyLabelKind, Q4AccuracyLabelStore, Q4AccuracyMetricKind, Q4AccuracyQuarantine,
    Q4AccuracyRawOutputPaths, Q4AccuracyRawOutputs, Q4AccuracyScanPhase, Q4AccuracySignalRecord,
    Q4AccuracySource, Q4AccuracyToolOutput, DEFAULT_Q4_ACCURACY_TIMEOUT_SECS,
};
pub use q4_confidence_calibration::{
    calibrate_q4_binary_class, q4_head_calibration_key, PersistedQ4HeadCalibration,
    Q4CalibrationCellMetric, Q4CalibrationExample, Q4CalibrationHead, Q4CalibrationMethod,
    Q4CalibrationStatus, Q4HeadCalibrationReport, Q4HeadCalibrationStore,
    Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION, Q4_CONFIDENCE_COVERAGE_HIGH,
    Q4_CONFIDENCE_COVERAGE_LOW, Q4_CONFIDENCE_HOLDOUT_FRACTION, Q4_CONFIDENCE_IMBALANCE_RATE,
    Q4_CONFIDENCE_MIN_LABELS, Q4_CONFIDENCE_TARGET_COVERAGE, Q4_CONFIDENCE_TARGET_ECE,
    Q4_CONFIDENCE_TARGET_PRECISION,
};
pub use q4_cost_labels::{
    extract_q4_cost_labels, run_q4_cost_tools_with_timeout, write_q4_cost_raw_outputs,
    PersistedQ4CostSignal, Q4CostCommandSpec, Q4CostExtraction, Q4CostKind, Q4CostLabel,
    Q4CostLabelKind, Q4CostLabelStore, Q4CostQuarantine, Q4CostRawOutputPaths, Q4CostRawOutputs,
    Q4CostScanPhase, Q4CostSignalRecord, Q4CostSource, Q4CostToolOutput,
    DEFAULT_Q4_COST_TIMEOUT_SECS,
};
pub use q4_pass_annotations::{
    evaluate_q4_pass_nontrivial, q4_pass_annotation_key, write_q4_pass_weekly_markdown,
    PersistedQ4PassAnnotation, Q4PassAnnotation, Q4PassAnnotationKind, Q4PassAnnotationStore,
    Q4PassCellMetric, Q4PassEvaluationReport, Q4PassHeadKind, Q4PassMetricStatus,
    Q4PassPredictedConcern, Q4PassSeverity, Q4_PASS_CONFIDENCE_THRESHOLD, Q4_PASS_MIN_ANNOTATIONS,
    Q4_PASS_REGRESSION_DROP_THRESHOLD, Q4_PASS_SCHEMA_VERSION, Q4_PASS_TARGET_NONTRIVIAL_RATE,
};
pub use q4_perf_labels::{
    extract_q4_perf_labels, run_q4_perf_tools_with_timeout, write_q4_perf_raw_outputs,
    PersistedQ4PerfSignal, Q4PerfCategory, Q4PerfCommandSpec, Q4PerfExtraction, Q4PerfLabel,
    Q4PerfLabelStore, Q4PerfQuarantine, Q4PerfRawOutputPaths, Q4PerfRawOutputs, Q4PerfScanPhase,
    Q4PerfSignalRecord, Q4PerfSource, Q4PerfToolKind, Q4PerfToolOutput,
    DEFAULT_Q4_PERF_TIMEOUT_SECS,
};
pub use q4_reasoning_labels::{
    attach_checkpoint_hash, extract_q4_reasoning_labels, train_q4_reasoning_head,
    write_q4_reasoning_raw_outputs, PersistedQ4ReasoningCalibration, PersistedQ4ReasoningSignal,
    Q4ReasoningCalibrationRow, Q4ReasoningClass, Q4ReasoningExtraction, Q4ReasoningFeatures,
    Q4ReasoningHeadCheckpoint, Q4ReasoningHeadStatus, Q4ReasoningLabel, Q4ReasoningLabelStore,
    Q4ReasoningOutcome, Q4ReasoningPredictionVerdict, Q4ReasoningQuarantine,
    Q4ReasoningRawOutputPaths, Q4ReasoningSignalRecord, Q4ReasoningSource,
    Q4_REASONING_MIN_HARVEST_EXAMPLES,
};
pub use q4_security_labels::{
    extract_q4_security_labels, run_q4_security_tools, run_q4_security_tools_with_timeout,
    write_q4_security_raw_outputs, PersistedQ4SecuritySignal, Q4SecurityClass, Q4SecurityDetector,
    Q4SecurityExtraction, Q4SecurityLabel, Q4SecurityLabelStore, Q4SecurityLineRange,
    Q4SecurityQuarantine, Q4SecurityRawOutputPaths, Q4SecurityRawOutputs, Q4SecurityScanPhase,
    Q4SecuritySeverity, Q4SecuritySignalRecord, Q4SecuritySource, Q4SecurityToolOutput,
    DEFAULT_Q4_SECURITY_TIMEOUT_SECS,
};

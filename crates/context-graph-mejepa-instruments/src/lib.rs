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
//   - `e_oracle::EOracleInstrument` — first concrete instrument
//     (one-hot test-outcome counts + categorical exception class). 128-d
//     output. The other 13 instruments are deferred to follow-up turns.
//
// Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`
// — clean-room. No upstream code copied.
//
// Fail-closed everywhere: empty input, dim mismatch on `set_slot`,
// non-finite input or output, slot-already-filled, panel-build with no
// slots, etc. all return `MEJEPA_INSTRUMENTS_INVALID_INPUT` or
// `_NUMERICAL_INVARIANT`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod e_oracle;

pub type InstrumentResult<T> = Result<T, InstrumentError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum InstrumentError {
    #[error("instrument input invalid at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error("instrument numerical invariant failed at {field}: {message}; remediation: {remediation}")]
    NumericalInvariant {
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
}

/// Total dimension of the joint embedding panel. Per doc 09 §3.5.5.
pub const PANEL_DIM: usize = 5_120;

/// Named instrument slots in the panel. Dim and offset per slot are fixed
/// across every panel; the table below MUST match `Self::layout()` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// `E_Reasoning` — Qwen CoT / reflexion-trace encoder. 384-d.
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
pub struct Panel {
    pub data: Vec<f32>,
    /// Bitmask: bit `i` (in `InstrumentSlot::all()` order) is set iff
    /// that slot was filled by `set_slot`. Bits not set are zero-filled.
    pub filled_mask: u16,
}

impl Panel {
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

    /// Convenience: encode `input` via `instrument` and write the result
    /// into the matching slot.
    pub fn set_from_instrument<I, In>(
        &mut self,
        instrument: &I,
        input: &In,
    ) -> InstrumentResult<()>
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
    /// records which slots were explicitly set.
    pub fn build(self) -> Panel {
        Panel {
            data: self.data,
            filled_mask: self.filled_mask,
        }
    }

    /// Optional check: assert each of `required_slots` is filled. Errors
    /// fail-closed listing the first missing slot. Use this when building
    /// `panel[t=2]` per doc 09 §2.3, where every slot is expected.
    pub fn require_slots(&self, required_slots: &[InstrumentSlot]) -> InstrumentResult<()> {
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

fn slot_bit(slot: InstrumentSlot) -> u16 {
    let idx = InstrumentSlot::all()
        .iter()
        .position(|s| *s == slot)
        .expect("InstrumentSlot::all is exhaustive");
    1u16 << idx
}

#[cfg(test)]
#[path = "tests.rs"]
mod instrument_tests;

pub use e_oracle::{EOracleInstrument, ExceptionClass, OracleVerdict, TestOutcome};

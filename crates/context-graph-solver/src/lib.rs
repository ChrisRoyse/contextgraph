// Inspired by ruvnet/RuVector crates/ruvector-solver/src/{types,forward_push,cg}.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

pub mod causal_mask;
pub mod cg;
pub mod conformal;
pub mod csr;
pub mod error;
pub mod granger;
pub mod mmr;
pub mod pagerank;

pub use causal_mask::{
    apply_mask_to_scores, build_causal_mask, retrocausal_safety_check, softmax_row, MaskStrategy,
};
pub use cg::{ConjugateGradientConfig, ConjugateGradientReport, ConjugateGradientSolver};
pub use conformal::{
    ConformalConfig, ConformalPredictor, ConformalReport, NonconformityMeasure, PredictionSet,
};
pub use csr::{CsrMatrix, MatrixKind};
pub use error::{SolverError, SolverResult};
pub use granger::{
    f_distribution_upper_tail, granger_test, regularized_incomplete_beta, GrangerConfig,
    GrangerReport,
};
pub use mmr::{
    select as mmr_select, select_with_matrix as mmr_select_with_matrix, MmrConfig, MmrSelection,
};
pub use pagerank::{ForwardPushConfig, ForwardPushReport, ForwardPushSolver, RankedNode};

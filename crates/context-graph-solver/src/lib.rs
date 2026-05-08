// Inspired by ruvnet/RuVector crates/ruvector-solver/src/{types,forward_push,cg}.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

pub mod cg;
pub mod csr;
pub mod error;
pub mod pagerank;

pub use cg::{ConjugateGradientConfig, ConjugateGradientReport, ConjugateGradientSolver};
pub use csr::{CsrMatrix, MatrixKind};
pub use error::{SolverError, SolverResult};
pub use pagerank::{ForwardPushConfig, ForwardPushReport, ForwardPushSolver, RankedNode};

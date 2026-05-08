// Inspired by ruvnet/RuVector crates/ruvector-solver/src/error.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use thiserror::Error;

pub type SolverResult<T> = Result<T, SolverError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum SolverError {
    #[error("solver input invalid at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error("solver failed to converge: {message}; remediation: {remediation}")]
    DidNotConverge {
        message: String,
        remediation: &'static str,
    },
    #[error("solver numerical invariant failed at {field}: {message}; remediation: {remediation}")]
    NumericalInvariant {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
}

impl SolverError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "CGSOLVER_INVALID_INPUT",
            Self::DidNotConverge { .. } => "CGSOLVER_DID_NOT_CONVERGE",
            Self::NumericalInvariant { .. } => "CGSOLVER_NUMERICAL_INVARIANT",
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

use std::error::Error;
use std::fmt;

use super::PathwayLeaf;

pub type PathwayResult<T> = Result<T, PathwayError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathwayError {
    detail: String,
}

impl PathwayError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn code(code: &str, detail: impl fmt::Display) -> String {
        format!("{code}: {detail}")
    }

    pub(crate) fn from_err(err: impl fmt::Display) -> Self {
        Self::new(err.to_string())
    }
}

impl fmt::Display for PathwayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pathway error: {}", self.detail)
    }
}

impl Error for PathwayError {}

pub(crate) fn require(condition: bool, detail: impl Into<String>) -> PathwayResult<()> {
    if condition {
        Ok(())
    } else {
        Err(PathwayError::new(detail))
    }
}

pub(crate) fn validate_id(field: &str, value: &str) -> PathwayResult<()> {
    require(
        !value.trim().is_empty(),
        format!("{field} must not be empty"),
    )?;
    require(
        !value.contains('\n') && !value.contains('\r'),
        format!("{field} must be single-line"),
    )
}

pub(crate) fn validate_hex(field: &str, value: &str) -> PathwayResult<()> {
    validate_id(field, value)?;
    require(
        value.len() >= 8 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        format!("{field} must be hex with at least 8 chars"),
    )
}

pub(crate) fn validate_sha(field: &str, value: &str) -> PathwayResult<()> {
    validate_id(field, value)?;
    require(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        format!("{field} must be a 64-char hex sha256"),
    )
}

pub(crate) fn validate_probability(field: &str, value: f32) -> PathwayResult<()> {
    require(
        value.is_finite() && (0.0..=1.0).contains(&value),
        format!("{field} must be finite in [0,1]"),
    )
}

pub(crate) fn first_closest_historical_pathway_id(leaves: &[PathwayLeaf]) -> Option<&String> {
    leaves
        .iter()
        .filter_map(|leaf| leaf.evidence.closest_historical_pathway_id.as_ref())
        .next()
}

pub(crate) fn invert_interval(interval: [f32; 2]) -> [f32; 2] {
    [1.0 - interval[1], 1.0 - interval[0]]
}

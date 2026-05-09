// ME-JEPA-Code Phase 0 mutation corpus generator.
//
// Per `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.1` (mutation
// corpus design) and `§7 Phase 0` (delivery contract). Inspired by the
// general approach of mutation testing (Mothra, MuJava, mutmut for Python);
// no upstream code copied. See
// `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.
//
// Eight mutation categories — each takes the canonical fix's source text
// for ONE file and returns a mutated version that should change the oracle
// outcome in a category-specific way:
//
//   1. KnownGood       — echo input unchanged                              (PASS expected)
//   2. SubtleFlip      — invert one boolean operator (`==`↔`!=`, `and`↔`or`, …) (FAIL expected)
//   3. OffByOne        — add ±1 to one numeric literal                      (FAIL expected)
//   4. SwapVariable    — rename one local variable to another in-scope name (FAIL expected)
//   5. DeleteTestCall  — delete one assertion call                          (mixed expected)
//   6. WrongFile       — apply patch text to a DIFFERENT base file          (FAIL expected)
//   7. OverEngineer    — append unused helper function                       (PASS expected)
//   8. CompileError    — introduce a Python syntax error                     (FAIL at parse)
//
// Fail-closed everywhere. An operator that cannot find a mutation site
// returns `MEJEPA_CORPUS_NO_MUTATION_SITE` rather than silently echoing
// the input. Determinism: same input + same `seed` → same mutated output.
// The `seed` selects WHICH candidate site to mutate when an operator finds
// multiple — e.g. `SubtleFlip` may find 7 boolean operators; the seed picks
// one. `KnownGood` ignores the seed entirely.
//
// Shipped alongside the operators:
//   - strict command/Docker oracle wrappers and official SWE-bench Lite bridge;
//   - crate-local RocksDB source-of-truth CFs for mutations and verdicts;
//   - split helpers for repo-atomic diagnostic splits and exact
//     instance-atomic 80/10/10 splits guarded by official test-patch SHA checks.
//
// Still operationally out of scope for this library entrypoint:
//   - the full 2,400-entry corpus generation batch (300 Lite tasks × 8 categories).

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod compile_error;
mod delete_test_call;
mod known_good;
mod off_by_one;
pub mod oracle;
mod over_engineer;
pub mod patch_mutation;
mod prng;
mod python_lex;
pub mod split;
mod subtle_flip;
mod swap_variable;
pub mod swebench;
mod util;
mod wrong_file;

#[cfg(test)]
#[path = "tests.rs"]
mod corpus_tests;

pub type MutationResult<T> = Result<T, MutationError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MutationError {
    #[error("mutation input invalid at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "mutation operator could not find a site at {field}: {message}; remediation: {remediation}"
    )]
    NoMutationSite {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error("mutation operator failed at {field}: {message}; remediation: {remediation}")]
    OperatorFailed {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
}

impl MutationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_CORPUS_INVALID_INPUT",
            Self::NoMutationSite { .. } => "MEJEPA_CORPUS_NO_MUTATION_SITE",
            Self::OperatorFailed { .. } => "MEJEPA_CORPUS_OPERATOR_FAILED",
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

    pub(crate) fn no_site(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::NoMutationSite {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn op_failed(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::OperatorFailed {
            field,
            message: message.into(),
            remediation,
        }
    }
}

/// One of the eight mutation categories per doc 09 §3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    /// Apply the official fix unchanged. Expected oracle: PASS.
    KnownGood,
    /// Invert one boolean operator. Expected oracle: FAIL.
    SubtleFlip,
    /// Add ±1 to one numeric literal. Expected oracle: FAIL.
    OffByOne,
    /// Rename one local variable to another in-scope name. Expected oracle: FAIL.
    SwapVariable,
    /// Delete one assertion call. Expected oracle: mixed (PASS the deleted
    /// test, FAIL the integration test).
    DeleteTestCall,
    /// Apply patch text to a DIFFERENT base file. Expected oracle: FAIL on
    /// test discovery or runtime.
    WrongFile,
    /// Append unused helper function. Expected oracle: PASS (correct but
    /// messy — important for "predict pass without overfitting to one
    /// syntactic shape").
    OverEngineer,
    /// Introduce a Python syntax error. Expected oracle: FAIL at parse.
    CompileError,
}

impl MutationCategory {
    /// Return all eight categories in canonical doc-09 order. Useful for
    /// the corpus generator's outer loop.
    pub fn all() -> [Self; 8] {
        [
            Self::KnownGood,
            Self::SubtleFlip,
            Self::OffByOne,
            Self::SwapVariable,
            Self::DeleteTestCall,
            Self::WrongFile,
            Self::OverEngineer,
            Self::CompileError,
        ]
    }

    /// Compact stable identifier for use in corpus filenames + JSON.
    pub fn slug(&self) -> &'static str {
        match self {
            Self::KnownGood => "known_good",
            Self::SubtleFlip => "subtle_flip",
            Self::OffByOne => "off_by_one",
            Self::SwapVariable => "swap_variable",
            Self::DeleteTestCall => "delete_test_call",
            Self::WrongFile => "wrong_file",
            Self::OverEngineer => "over_engineer",
            Self::CompileError => "compile_error",
        }
    }
}

/// Configuration for a single mutation. `seed` selects the site when an
/// operator finds multiple candidates (deterministic SplitMix64 PRNG).
/// `alternate_source` is REQUIRED for `WrongFile` and IGNORED otherwise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MutationConfig {
    pub seed: u64,
    pub alternate_source: Option<String>,
}

/// Result record for one mutation. `mutation_site` is `None` for `KnownGood`
/// and `WrongFile` (which doesn't mutate the primary source — it replaces
/// it wholesale).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationOutcome {
    pub category: MutationCategory,
    pub mutated_source: String,
    pub seed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mutation_site: Option<MutationSite>,
}

/// Byte-precise location of an in-place mutation, plus the original and
/// replacement strings so a downstream auditor can reproduce the edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationSite {
    pub byte_offset: usize,
    pub byte_length: usize,
    pub original_text: String,
    pub replacement_text: String,
    pub note: String,
}

/// Apply a mutation to `primary_source`. Errors fail-closed:
/// - `MEJEPA_CORPUS_INVALID_INPUT` if `primary_source` is empty / not UTF-8
///   sane, or `WrongFile` is requested without `alternate_source`.
/// - `MEJEPA_CORPUS_NO_MUTATION_SITE` if the operator finds no candidate
///   site (e.g. `SubtleFlip` on source with no boolean operators).
/// - `MEJEPA_CORPUS_OPERATOR_FAILED` for unexpected internal errors.
pub fn apply_mutation(
    category: MutationCategory,
    primary_source: &str,
    config: MutationConfig,
) -> MutationResult<MutationOutcome> {
    if primary_source.is_empty() {
        return Err(MutationError::invalid(
            "primary_source",
            "primary_source is empty; mutations require non-empty input",
            "supply the canonical fix's source text",
        ));
    }
    python_lex::ensure_parseable_python(primary_source, "primary_source")?;
    let site = match category {
        MutationCategory::KnownGood => {
            return known_good::apply(primary_source, config.seed);
        }
        MutationCategory::SubtleFlip => subtle_flip::apply(primary_source, config.seed)?,
        MutationCategory::OffByOne => off_by_one::apply(primary_source, config.seed)?,
        MutationCategory::SwapVariable => swap_variable::apply(primary_source, config.seed)?,
        MutationCategory::DeleteTestCall => delete_test_call::apply(primary_source, config.seed)?,
        MutationCategory::WrongFile => {
            let alternate = config.alternate_source.as_deref().ok_or_else(|| {
                MutationError::invalid(
                    "alternate_source",
                    "WrongFile mutation requires `alternate_source` to be set",
                    "pass the path/content of a different but structurally-similar file",
                )
            })?;
            python_lex::ensure_parseable_python(alternate, "alternate_source")?;
            return wrong_file::apply(primary_source, alternate, config.seed);
        }
        MutationCategory::OverEngineer => over_engineer::apply(primary_source, config.seed)?,
        MutationCategory::CompileError => compile_error::apply(primary_source, config.seed)?,
    };
    apply_site(category, primary_source, config.seed, site)
}

/// Apply a previously-located `MutationSite` to `source`, returning the
/// patched string. Crate-private — callers go through `apply_mutation`.
fn apply_site(
    category: MutationCategory,
    source: &str,
    seed: u64,
    site: MutationSite,
) -> MutationResult<MutationOutcome> {
    let end = site
        .byte_offset
        .checked_add(site.byte_length)
        .ok_or_else(|| {
            MutationError::op_failed(
                "mutation_site",
                format!(
                    "site offset {} + length {} overflowed usize",
                    site.byte_offset, site.byte_length
                ),
                "report the operator that produced the invalid site",
            )
        })?;
    if site.byte_offset > source.len() || end > source.len() {
        return Err(MutationError::op_failed(
            "mutation_site",
            format!(
                "site range {}..{} is outside source length {}",
                site.byte_offset,
                end,
                source.len()
            ),
            "report the operator that produced the invalid site",
        ));
    }
    if !source.is_char_boundary(site.byte_offset) || !source.is_char_boundary(end) {
        return Err(MutationError::op_failed(
            "mutation_site",
            format!(
                "site range {}..{} does not align to UTF-8 character boundaries",
                site.byte_offset, end
            ),
            "report the operator that produced the invalid site",
        ));
    }
    let actual = &source[site.byte_offset..end];
    if actual != site.original_text {
        return Err(MutationError::op_failed(
            "mutation_site.original_text",
            format!(
                "site original_text {:?} did not match source bytes {:?} at {}..{}",
                site.original_text, actual, site.byte_offset, end
            ),
            "report the operator that produced the invalid site",
        ));
    }

    let mut buf =
        String::with_capacity(source.len() - site.byte_length + site.replacement_text.len());
    buf.push_str(&source[..site.byte_offset]);
    buf.push_str(&site.replacement_text);
    buf.push_str(&source[end..]);
    Ok(MutationOutcome {
        category,
        mutated_source: buf,
        seed,
        mutation_site: Some(site),
    })
}

pub use prng::SplitMix64;

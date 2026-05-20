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
// The batch CLI now builds source-backed patches before invoking the Docker
// oracle, so the 300 Lite tasks × 8 categories corpus can be generated without
// depending on mutation sites being present in added diff lines.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Re-export of the canonical oracle verdict types defined by
/// `context-graph-mejepa-instruments`. Per the CORPUS integration contract,
/// this crate consumes and persists those upstream schemas; it does not
/// redefine them.
pub use context_graph_mejepa_instruments::{
    ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
};

mod compile_error;
mod delete_test_call;
pub mod docker_outcome_labels;
pub mod generate;
mod generic_language_ops;
mod known_good;
pub mod multilang_fixture_corpus;
mod off_by_one;
pub mod oracle;
mod over_engineer;
pub mod patch_mutation;
pub mod prng;
pub mod python_lex;
pub mod source_patch;
mod source_patch_support;
pub mod split;
mod subtle_flip;
pub mod timed_subprocess;

/// Integration-test access to the timed git runner. Public API surface is
/// intentionally minimal: only the regression test in
/// `tests/source_prep_timeout_test.rs` calls these. Adding a new public
/// surface here requires the FSV-PROTOCOL audit.
pub mod test_support {
    use std::path::Path;
    use std::time::Duration;

    use crate::source_patch_support::{run_git, run_git_with_timeout_for_test};
    use crate::{MutationError, MutationResult};

    pub fn run_git_for_tests(cwd: &Path, args: &[&str]) -> MutationResult<()> {
        run_git(&[cwd], args, "test_support.run_git")
    }

    pub fn run_git_with_timeout_for_tests(
        cwd: &Path,
        args: &[&str],
        timeout: Duration,
    ) -> Result<(), MutationError> {
        run_git_with_timeout_for_test(cwd, args, timeout, "test_support.run_git_with_timeout")
    }
}
mod swap_variable;
pub mod swebench;
mod util;
mod wrong_file;

pub use generate::{
    generate_swe_bench_mutation_corpus, generate_swe_bench_mutation_corpus_with_oracle,
    CorpusReport, MejepaCorpusError, Oracle, SwebenchDockerOracle,
};

#[cfg(test)]
#[path = "tests.rs"]
mod corpus_tests;

pub type MutationResult<T> = Result<T, MutationError>;

pub const MEJEPA_CORPUS_VERSION_V1: &str = "phase0-corpus-v1";
pub const MEJEPA_EMBEDDER_VERSION_V1: &str = "mejepa-embedder-v1";

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
    #[error("mutation corpus leakage detected for test_patch sha {sha}: {collisions:?}")]
    LeakageDetected {
        sha: String,
        collisions: Vec<(String, String)>,
    },
    #[error(
        "subprocess {program} timed out after {elapsed_secs}s during {operation}; \
         stdout_tail={stdout_tail}; stderr_tail={stderr_tail}; remediation: {remediation}"
    )]
    SubprocessTimeout {
        program: String,
        operation: &'static str,
        elapsed_secs: u64,
        stdout_tail: String,
        stderr_tail: String,
        remediation: &'static str,
    },
}

impl MutationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_CORPUS_INVALID_INPUT",
            Self::NoMutationSite { .. } => "MEJEPA_CORPUS_NO_MUTATION_SITE",
            Self::OperatorFailed { .. } => "MEJEPA_CORPUS_OPERATOR_FAILED",
            Self::LeakageDetected { .. } => "MEJEPA_CORPUS_LEAKAGE_DETECTED",
            Self::SubprocessTimeout { .. } => "MEJEPA_CORPUS_SUBPROCESS_TIMEOUT",
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

    pub(crate) fn leakage_detected(
        sha: impl Into<String>,
        collisions: Vec<(String, String)>,
    ) -> Self {
        Self::LeakageDetected {
            sha: sha.into(),
            collisions,
        }
    }
}

/// Corpus generation supports Python SWE-bench Lite plus Phase C fixture
/// corpora for the 10 non-Python languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
}

impl Language {
    pub const fn all() -> [Self; 11] {
        [
            Self::Rust,
            Self::Python,
            Self::JavaScript,
            Self::TypeScript,
            Self::Go,
            Self::Java,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Ruby,
            Self::Php,
        ]
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "c_sharp",
            Self::Ruby => "ruby",
            Self::Php => "php",
        }
    }

    pub fn from_slug(value: &str) -> Option<Self> {
        match value {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            "javascript" => Some(Self::JavaScript),
            "typescript" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" => Some(Self::C),
            "cpp" => Some(Self::Cpp),
            "c_sharp" => Some(Self::CSharp),
            "ruby" => Some(Self::Ruby),
            "php" => Some(Self::Php),
            _ => None,
        }
    }

    pub const fn is_supported(self) -> bool {
        true
    }
}

pub fn parse_languages(raw: &[String]) -> MutationResult<Vec<Language>> {
    if raw.is_empty() {
        return Err(MutationError::invalid(
            "languages",
            "at least one language slug is required",
            "pass one or more canonical language slugs from the 11-language mutation set",
        ));
    }
    let mut out = Vec::with_capacity(raw.len());
    for value in raw {
        let language = Language::from_slug(value).ok_or_else(|| {
            MutationError::invalid(
                "languages",
                format!("unknown language slug `{value}`"),
                "use one of: rust, python, javascript, typescript, go, java, c, cpp, c_sharp, ruby, php",
            )
        })?;
        if !out.contains(&language) {
            out.push(language);
        }
    }
    Ok(out)
}

/// Stable row material used to compute the finalized corpus hash. The hash is
/// over task/category plus persisted patch and verdict hashes, not over a
/// transient CLI return value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusProvenanceRow {
    pub task_id: String,
    pub category: MutationCategory,
    pub patch_sha256: String,
    pub oracle_verdict_sha256: String,
}

/// Final corpus-level provenance persisted after all mutation+verdict rows have
/// been written and read back from RocksDB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusProvenance {
    pub corpus_version: String,
    pub embedder_version: String,
    pub generated_at_unix_ms: u128,
    pub generator_version: String,
    pub seed: u64,
    pub languages: Vec<Language>,
    pub task_manifest_count: usize,
    pub mutation_count: usize,
    pub mutation_categories: Vec<MutationCategory>,
    pub source_patch_mode: String,
    pub split_mode: String,
    pub corpus_sha256: String,
    pub complete: bool,
}

impl CorpusProvenance {
    pub fn validate(&self) -> MutationResult<()> {
        if self.corpus_version.trim().is_empty() {
            return Err(MutationError::invalid(
                "corpus_version",
                "corpus_version is empty",
                "set an explicit immutable corpus schema/version identifier",
            ));
        }
        if self.embedder_version.trim().is_empty() {
            return Err(MutationError::invalid(
                "embedder_version",
                "embedder_version is empty",
                "record the frozen embedder or signal schema version used for this corpus",
            ));
        }
        if self.generator_version.trim().is_empty() {
            return Err(MutationError::invalid(
                "generator_version",
                "generator_version is empty",
                "record the generator package version or git revision",
            ));
        }
        if self.languages.is_empty() {
            return Err(MutationError::invalid(
                "languages",
                "languages is empty",
                "record every source language represented in the corpus",
            ));
        }
        if self
            .languages
            .iter()
            .any(|language| !language.is_supported())
        {
            return Err(MutationError::invalid(
                "languages",
                "Phase 0 provenance contains a language without implemented mutation operators",
                "use only Language::all entries; all 11 parser-backed operators are implemented",
            ));
        }
        if self.mutation_count == 0 {
            return Err(MutationError::invalid(
                "mutation_count",
                "mutation_count is zero",
                "persist at least one mutation+verdict row before writing provenance",
            ));
        }
        if !is_plain_sha256_hex(&self.corpus_sha256) {
            return Err(MutationError::invalid(
                "corpus_sha256",
                format!(
                    "corpus_sha256 must be 64 lowercase hex characters, got {:?}",
                    self.corpus_sha256
                ),
                "compute provenance with compute_corpus_sha256",
            ));
        }
        Ok(())
    }
}

pub fn compute_corpus_sha256(rows: &[CorpusProvenanceRow]) -> MutationResult<String> {
    if rows.is_empty() {
        return Err(MutationError::invalid(
            "rows",
            "cannot compute corpus hash for zero rows",
            "finalize provenance only after the corpus contains mutation+verdict rows",
        ));
    }
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| {
        a.task_id
            .cmp(&b.task_id)
            .then_with(|| a.category.slug().cmp(b.category.slug()))
    });
    let mut hasher = Sha256::new();
    for row in sorted {
        validate_hash_field("patch_sha256", &row.patch_sha256)?;
        validate_hash_field("oracle_verdict_sha256", &row.oracle_verdict_sha256)?;
        hasher.update(row.task_id.as_bytes());
        hasher.update([0]);
        hasher.update(row.category.slug().as_bytes());
        hasher.update([0]);
        hasher.update(row.patch_sha256.as_bytes());
        hasher.update([0]);
        hasher.update(row.oracle_verdict_sha256.as_bytes());
        hasher.update([0xff]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn build_corpus_provenance(
    store: &crate::oracle::OracleStore,
    embedder_version: &str,
    code_version: &str,
    language_set: &[Language],
) -> MutationResult<CorpusProvenance> {
    if embedder_version.trim().is_empty() {
        return Err(MutationError::invalid(
            "embedder_version",
            "embedder_version is empty",
            "record the frozen embedder or signal schema version used for this corpus",
        ));
    }
    if code_version.trim().is_empty() {
        return Err(MutationError::invalid(
            "code_version",
            "code_version is empty",
            "pass the current generator version or git revision",
        ));
    }
    if language_set.is_empty() {
        return Err(MutationError::invalid(
            "language_set",
            "language_set is empty",
            "record at least Language::Python for Phase 0 corpora",
        ));
    }
    if language_set.iter().any(|language| !language.is_supported()) {
        return Err(MutationError::invalid(
            "language_set",
            "unsupported language present in provenance request",
            "use only Language::all entries; all 11 parser-backed operators are implemented",
        ));
    }
    let rows = store.iter_corpus_rows().map_err(|err| {
        MutationError::op_failed(
            "oracle_store",
            format!(
                "failed to iterate corpus rows while building provenance: {err} ({})",
                err.code()
            ),
            "inspect the RocksDB corpus store before finalizing provenance",
        )
    })?;
    let mut provenance_rows = Vec::with_capacity(rows.len());
    let mut task_ids = std::collections::BTreeSet::new();
    let mut categories = Vec::new();
    for (task_id, category, outcome, verdict) in rows {
        task_ids.insert(task_id.clone());
        categories.push(category);
        provenance_rows.push(CorpusProvenanceRow {
            task_id,
            category,
            patch_sha256: sha256_text(&outcome.mutated_source),
            oracle_verdict_sha256: sha256_json_value(&serde_json::to_value(&verdict).map_err(
                |err| {
                    MutationError::op_failed(
                        "oracle_verdict",
                        format!("failed to serialize oracle verdict for provenance: {err}"),
                        "ensure OracleVerdict remains JSON-serializable",
                    )
                },
            )?),
        });
    }
    categories.sort_by_key(|category| category.slug());
    categories.dedup();
    let provenance = CorpusProvenance {
        corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
        embedder_version: embedder_version.to_string(),
        generated_at_unix_ms: unix_ms_lossy(),
        generator_version: code_version.to_string(),
        seed: 0,
        languages: language_set.to_vec(),
        task_manifest_count: task_ids.len(),
        mutation_count: provenance_rows.len(),
        mutation_categories: categories,
        source_patch_mode: "source-backed".to_string(),
        split_mode: "instance_atomic".to_string(),
        corpus_sha256: compute_corpus_sha256(&provenance_rows)?,
        complete: true,
    };
    provenance.validate()?;
    Ok(provenance)
}

pub fn finalize_provenance(
    store: &crate::oracle::OracleStore,
    embedder_version: &str,
    code_version: &str,
    language_set: &[Language],
) -> MutationResult<CorpusProvenance> {
    let provenance = build_corpus_provenance(store, embedder_version, code_version, language_set)?;
    store.put_provenance(&provenance).map_err(|err| {
        MutationError::op_failed(
            "provenance",
            format!("failed to write provenance row: {err} ({})", err.code()),
            "inspect RocksDB provenance column family and regenerate into a fresh output directory",
        )
    })?;
    Ok(provenance)
}

pub fn verify_provenance_consistent(
    store: &crate::oracle::OracleStore,
    expected: &CorpusProvenance,
) -> MutationResult<()> {
    let derived = build_corpus_provenance(
        store,
        &expected.embedder_version,
        &expected.generator_version,
        &expected.languages,
    )?;
    if derived.mutation_count != expected.mutation_count {
        return Err(MutationError::invalid(
            "mutation_count",
            format!(
                "provenance mutation_count={} but current store has {} rows",
                expected.mutation_count, derived.mutation_count
            ),
            "inspect the mutation and verdict column families for drift",
        ));
    }
    if derived.corpus_sha256 != expected.corpus_sha256 {
        return Err(MutationError::invalid(
            "corpus_sha256",
            format!(
                "provenance corpus_sha256={} but current store derives {}",
                expected.corpus_sha256, derived.corpus_sha256
            ),
            "inspect the corpus store for tampering or partial writes",
        ));
    }
    Ok(())
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_json_value(value: &serde_json::Value) -> String {
    sha256_text(&serde_json::to_string(value).expect("JSON value serialises"))
}

fn unix_ms_lossy() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn validate_hash_field(field: &'static str, value: &str) -> MutationResult<()> {
    let raw = value.strip_prefix("sha256:").unwrap_or(value);
    if is_plain_sha256_hex(raw) {
        Ok(())
    } else {
        Err(MutationError::invalid(
            field,
            format!("{field} must be sha256:<64 hex> or 64 lowercase hex, got {value:?}"),
            "store content-addressed hashes produced by the corpus generator",
        ))
    }
}

fn is_plain_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
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

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|category| category.slug() == slug)
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
    apply_mutation_for_language(Language::Python, category, primary_source, config)
}

pub fn apply_mutation_for_language(
    language: Language,
    category: MutationCategory,
    primary_source: &str,
    config: MutationConfig,
) -> MutationResult<MutationOutcome> {
    if language != Language::Python {
        return generic_language_ops::apply(language, category, primary_source, config);
    }
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

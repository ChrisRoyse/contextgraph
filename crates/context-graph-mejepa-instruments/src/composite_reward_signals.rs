// TASK-RWD-209 / TASK-PY-G-075: composite reward-signal labels.
//
// This module completes the residual #294 slice after E_Coverage,
// E_Cyclomatic, and E_DiffTopology shipped independently. It persists the five
// issue-specified signal families to separate CFs and writes raw per-row JSON
// artifacts under python-reward-signals-v1.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::{
    CF_MEJEPA_CHURN_SIGNALS, CF_MEJEPA_COMPLEXITY_SIGNALS, CF_MEJEPA_COVERAGE_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS, CF_MEJEPA_DOC_COVERAGE_SIGNALS,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::features::fnv1a64;
use crate::{ComplexityMetrics, CoverageMetrics, InstrumentError, InstrumentResult};

const COMPOSITE_REWARD_SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 10_000_000;
const MAX_REQUIREMENTS_BYTES: usize = 5_000_000;
const MAX_ADVISORY_BYTES: usize = 50_000_000;
const MAX_COMMITS: usize = 100_000;
const MAX_FUNCTIONS: usize = 100_000;
const MAX_DEPENDENCIES: usize = 100_000;
const RECENT_CHURN_WINDOW_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositeRewardSignalKind {
    Coverage,
    Complexity,
    Churn,
    DocCoverage,
    DepCve,
}

impl CompositeRewardSignalKind {
    pub fn cf(self) -> &'static str {
        match self {
            Self::Coverage => CF_MEJEPA_COVERAGE_SIGNALS,
            Self::Complexity => CF_MEJEPA_COMPLEXITY_SIGNALS,
            Self::Churn => CF_MEJEPA_CHURN_SIGNALS,
            Self::DocCoverage => CF_MEJEPA_DOC_COVERAGE_SIGNALS,
            Self::DepCve => CF_MEJEPA_DEP_CVE_SIGNALS,
        }
    }

    fn as_key_part(self) -> &'static str {
        match self {
            Self::Coverage => "coverage",
            Self::Complexity => "complexity",
            Self::Churn => "churn",
            Self::DocCoverage => "doc",
            Self::DepCve => "depcve",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub line_coverage_pct: f32,
    pub branch_coverage_pct: Option<f32>,
    pub coverage_delta: f32,
    pub newly_covered_lines: u32,
    pub newly_uncovered_lines: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComplexityLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub function_count: u32,
    pub max_cyclomatic_complexity: u32,
    pub mean_cyclomatic_complexity: f32,
    pub max_cognitive_complexity: u32,
    pub max_nesting_depth: u32,
    pub halstead_volume: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChurnCommit {
    pub commit_sha: String,
    pub author: String,
    pub committed_unix_secs: i64,
    pub path: String,
    pub touched_byte_start: u64,
    pub touched_byte_end: u64,
    pub lines_added: u32,
    pub lines_deleted: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChurnInput {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub now_unix_secs: i64,
    pub commits: Vec<ChurnCommit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChurnLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub commits_touching_range: u32,
    pub distinct_authors: u32,
    pub age_days: u32,
    pub recent_churn_fraction: f32,
    pub lines_added: u32,
    pub lines_deleted: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocCoverageInput {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub language: String,
    pub path: String,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionDocCoverage {
    pub qualified_name: String,
    pub has_docstring: bool,
    pub parameter_count: u32,
    pub documented_parameter_count: u32,
    pub doc_coverage: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocCoverageLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub function_count: u32,
    pub documented_function_count: u32,
    pub fully_documented_function_count: u32,
    pub parameter_count: u32,
    pub documented_parameter_count: u32,
    pub comment_line_count: u32,
    pub module_docstring_present: bool,
    pub doc_coverage: f32,
    pub functions: Vec<FunctionDocCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepCveInput {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub source_text: String,
    pub requirements_txt: Option<String>,
    pub pyproject_toml: Option<String>,
    pub advisory_db_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepCveLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub import_name: String,
    pub package: String,
    pub current_version: String,
    pub known_cve_count: u32,
    pub max_cvss: f32,
    pub advisory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeRewardQuarantine {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub signal_kind: CompositeRewardSignalKind,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum CompositeRewardSignalRecord {
    Coverage(CoverageLabel),
    Complexity(ComplexityLabel),
    Churn(ChurnLabel),
    DocCoverage(DocCoverageLabel),
    DepCve(DepCveLabel),
    Quarantine(CompositeRewardQuarantine),
}

impl CompositeRewardSignalRecord {
    pub fn signal_kind(&self) -> CompositeRewardSignalKind {
        match self {
            Self::Coverage(_) => CompositeRewardSignalKind::Coverage,
            Self::Complexity(_) => CompositeRewardSignalKind::Complexity,
            Self::Churn(_) => CompositeRewardSignalKind::Churn,
            Self::DocCoverage(_) => CompositeRewardSignalKind::DocCoverage,
            Self::DepCve(_) => CompositeRewardSignalKind::DepCve,
            Self::Quarantine(quarantine) => quarantine.signal_kind,
        }
    }

    fn identity_parts(&self) -> (&str, &str, &str) {
        match self {
            Self::Coverage(label) => (&label.corpus_row_id, &label.chunk_id, &label.path),
            Self::Complexity(label) => (&label.corpus_row_id, &label.chunk_id, &label.path),
            Self::Churn(label) => (&label.corpus_row_id, &label.chunk_id, &label.path),
            Self::DocCoverage(label) => (&label.corpus_row_id, &label.chunk_id, &label.path),
            Self::DepCve(label) => (&label.corpus_row_id, &label.chunk_id, &label.import_name),
            Self::Quarantine(quarantine) => (
                &quarantine.corpus_row_id,
                &quarantine.chunk_id,
                &quarantine.reason_code,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedCompositeRewardSignal {
    pub schema_version: u32,
    pub signal: CompositeRewardSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CompositeRewardExtraction {
    pub coverage: Vec<CoverageLabel>,
    pub complexity: Vec<ComplexityLabel>,
    pub churn: Vec<ChurnLabel>,
    pub doc_coverage: Vec<DocCoverageLabel>,
    pub dep_cve: Vec<DepCveLabel>,
    pub quarantines: Vec<CompositeRewardQuarantine>,
}

impl CompositeRewardExtraction {
    pub fn persisted_records(&self) -> Vec<PersistedCompositeRewardSignal> {
        let mut records = Vec::new();
        for label in &self.coverage {
            records.push(persisted(CompositeRewardSignalRecord::Coverage(
                label.clone(),
            )));
        }
        for label in &self.complexity {
            records.push(persisted(CompositeRewardSignalRecord::Complexity(
                label.clone(),
            )));
        }
        for label in &self.churn {
            records.push(persisted(CompositeRewardSignalRecord::Churn(label.clone())));
        }
        for label in &self.doc_coverage {
            records.push(persisted(CompositeRewardSignalRecord::DocCoverage(
                label.clone(),
            )));
        }
        for label in &self.dep_cve {
            records.push(persisted(CompositeRewardSignalRecord::DepCve(
                label.clone(),
            )));
        }
        for quarantine in &self.quarantines {
            records.push(persisted(CompositeRewardSignalRecord::Quarantine(
                quarantine.clone(),
            )));
        }
        records
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeRewardInput {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub coverage_metrics: Option<CoverageMetrics>,
    pub complexity_metrics: Option<ComplexityMetrics>,
    pub churn_input: Option<ChurnInput>,
    pub doc_coverage_input: Option<DocCoverageInput>,
    pub dep_cve_input: Option<DepCveInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeRewardRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub coverage_json: PathBuf,
    pub churn_json: PathBuf,
    pub complexity_json: PathBuf,
    pub doc_json: PathBuf,
    pub depcve_json: PathBuf,
    pub composite_json: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeRewardStoredKey {
    pub cf: String,
    pub key: String,
}

pub fn coverage_labels_from_metrics(
    corpus_row_id: &str,
    chunk_id: &str,
    metrics: &CoverageMetrics,
) -> InstrumentResult<Vec<CoverageLabel>> {
    validate_path_component("corpus_row_id", corpus_row_id)?;
    validate_path_component("chunk_id", chunk_id)?;
    if metrics.files.is_empty() {
        return invalid(
            "coverage.metrics.files",
            "coverage metrics contain no per-file rows",
            "derive coverage labels from coverage.py JSON with at least one file",
        );
    }
    let mut labels = Vec::new();
    for file in &metrics.files {
        let label = CoverageLabel {
            corpus_row_id: corpus_row_id.to_string(),
            chunk_id: chunk_id.to_string(),
            path: file.path.clone(),
            line_coverage_pct: file.candidate_line_coverage,
            branch_coverage_pct: None,
            coverage_delta: file.coverage_delta,
            newly_covered_lines: file.newly_covered_lines.len() as u32,
            newly_uncovered_lines: file.newly_uncovered_lines.len() as u32,
        };
        validate_coverage_label(&label)?;
        labels.push(label);
    }
    Ok(labels)
}

pub fn complexity_label_from_metrics(
    corpus_row_id: &str,
    chunk_id: &str,
    metrics: &ComplexityMetrics,
) -> InstrumentResult<ComplexityLabel> {
    validate_path_component("corpus_row_id", corpus_row_id)?;
    validate_path_component("chunk_id", chunk_id)?;
    validate_nonempty(
        "complexity.path",
        &metrics.path,
        "preserve analyzed file path",
    )?;
    let label = ComplexityLabel {
        corpus_row_id: corpus_row_id.to_string(),
        chunk_id: chunk_id.to_string(),
        path: metrics.path.clone(),
        function_count: metrics.function_count,
        max_cyclomatic_complexity: metrics.max_cyclomatic_complexity,
        mean_cyclomatic_complexity: metrics.mean_cyclomatic_complexity,
        max_cognitive_complexity: metrics.max_cognitive_complexity,
        max_nesting_depth: metrics.max_nesting_depth,
        halstead_volume: metrics.halstead_volume,
    };
    validate_complexity_label(&label)?;
    Ok(label)
}

pub fn analyze_churn(input: &ChurnInput) -> InstrumentResult<ChurnLabel> {
    validate_churn_input(input)?;
    let mut touching = Vec::new();
    for commit in &input.commits {
        if commit.path == input.path
            && ranges_overlap(
                input.byte_start,
                input.byte_end,
                commit.touched_byte_start,
                commit.touched_byte_end,
            )
        {
            touching.push(commit);
        }
    }
    if touching.is_empty() {
        return invalid(
            "churn.commits",
            "no commits touched the requested chunk byte range",
            "persist git blame/log records covering the chunk byte range before labeling churn",
        );
    }

    let mut authors = BTreeSet::new();
    let mut lines_added = 0_u32;
    let mut lines_deleted = 0_u32;
    let mut oldest = input.now_unix_secs;
    let mut recent = 0_u32;
    let recent_cutoff = input.now_unix_secs - RECENT_CHURN_WINDOW_DAYS.saturating_mul(24 * 60 * 60);
    for commit in &touching {
        authors.insert(commit.author.clone());
        lines_added = lines_added.saturating_add(commit.lines_added);
        lines_deleted = lines_deleted.saturating_add(commit.lines_deleted);
        oldest = oldest.min(commit.committed_unix_secs);
        if commit.committed_unix_secs >= recent_cutoff {
            recent = recent.saturating_add(1);
        }
    }
    let age_days = ((input.now_unix_secs - oldest) / (24 * 60 * 60)).max(0) as u32;
    let label = ChurnLabel {
        corpus_row_id: input.corpus_row_id.clone(),
        chunk_id: input.chunk_id.clone(),
        path: input.path.clone(),
        byte_start: input.byte_start,
        byte_end: input.byte_end,
        commits_touching_range: touching.len() as u32,
        distinct_authors: authors.len() as u32,
        age_days,
        recent_churn_fraction: recent as f32 / touching.len() as f32,
        lines_added,
        lines_deleted,
    };
    validate_churn_label(&label)?;
    Ok(label)
}

pub fn analyze_doc_coverage(input: &DocCoverageInput) -> InstrumentResult<DocCoverageLabel> {
    validate_doc_input(input)?;
    let parsed = ruff_python_parser::parse_module(&input.source_text).map_err(|err| {
        InstrumentError::invalid(
            "doc_coverage.source_text",
            format!("ruff-python-parser rejected source: {err}"),
            "fix Python syntax before computing TASK-RWD-209 doc-coverage signals",
        )
    })?;
    let module = parsed.into_syntax();
    let module_docstring_present = suite_docstring(&module.body, &input.source_text).is_some();
    let mut functions = Vec::new();
    collect_function_doc_coverage("module", &module.body, &input.source_text, &mut functions)?;
    if functions.len() > MAX_FUNCTIONS {
        return invalid(
            "doc_coverage.functions",
            format!(
                "function count {} exceeds max {MAX_FUNCTIONS}",
                functions.len()
            ),
            "shard very large files before computing doc-coverage signals",
        );
    }

    let function_count = functions.len() as u32;
    let documented_function_count = functions
        .iter()
        .filter(|function| function.has_docstring)
        .count() as u32;
    let fully_documented_function_count = functions
        .iter()
        .filter(|function| {
            function.has_docstring
                && function.documented_parameter_count == function.parameter_count
        })
        .count() as u32;
    let parameter_count = functions
        .iter()
        .map(|function| function.parameter_count)
        .sum::<u32>();
    let documented_parameter_count = functions
        .iter()
        .map(|function| function.documented_parameter_count)
        .sum::<u32>();
    let doc_coverage = if function_count == 0 {
        if module_docstring_present {
            1.0
        } else {
            0.0
        }
    } else {
        functions
            .iter()
            .map(|function| function.doc_coverage)
            .sum::<f32>()
            / function_count as f32
    };
    let label = DocCoverageLabel {
        corpus_row_id: input.corpus_row_id.clone(),
        chunk_id: input.chunk_id.clone(),
        path: input.path.clone(),
        function_count,
        documented_function_count,
        fully_documented_function_count,
        parameter_count,
        documented_parameter_count,
        comment_line_count: count_comment_lines(&input.source_text),
        module_docstring_present,
        doc_coverage,
        functions,
    };
    validate_doc_label(&label)?;
    Ok(label)
}

pub fn extract_dependency_cve_labels(input: &DepCveInput) -> InstrumentResult<Vec<DepCveLabel>> {
    validate_dep_cve_input(input)?;
    let dependencies = parse_dependency_sources(input)?;
    if dependencies.is_empty() {
        return invalid(
            "dep_cve.dependencies",
            "no pinned dependencies found in requirements.txt or pyproject.toml",
            "provide pinned Python dependencies before computing dependency-CVE labels",
        );
    }
    let advisory_db = parse_advisory_db(&input.advisory_db_json)?;
    let imports = parse_python_import_roots(&input.source_text)?;
    if imports.is_empty() {
        return invalid(
            "dep_cve.imports",
            "Python source contains no import statements",
            "compute dependency-CVE labels on chunks that contain dependency imports",
        );
    }
    let mut labels = Vec::new();
    for import_name in imports {
        let package = normalize_package_name(&import_name);
        let Some(version) = dependencies.get(&package) else {
            continue;
        };
        let mut advisory_ids = Vec::new();
        let mut max_cvss = 0.0_f32;
        if let Some(advisories) = advisory_db.packages.get(&package) {
            for advisory in advisories {
                if advisory.matches_version(version)? {
                    advisory_ids.push(advisory.id.clone());
                    max_cvss = max_cvss.max(advisory.cvss);
                }
            }
        }
        let label = DepCveLabel {
            corpus_row_id: input.corpus_row_id.clone(),
            chunk_id: input.chunk_id.clone(),
            path: input.path.clone(),
            import_name,
            package,
            current_version: version.clone(),
            known_cve_count: advisory_ids.len() as u32,
            max_cvss,
            advisory_ids,
        };
        validate_dep_cve_label(&label)?;
        labels.push(label);
    }
    if labels.is_empty() {
        return invalid(
            "dep_cve.labels",
            "no imports matched pinned dependency names",
            "preserve dependency names matching imported package roots",
        );
    }
    Ok(labels)
}

pub fn extract_composite_reward_signals(
    input: &CompositeRewardInput,
) -> InstrumentResult<CompositeRewardExtraction> {
    validate_path_component("corpus_row_id", &input.corpus_row_id)?;
    validate_path_component("chunk_id", &input.chunk_id)?;
    validate_nonempty(
        "path",
        &input.path,
        "preserve source file path for reward labels",
    )?;

    let mut extraction = CompositeRewardExtraction::default();

    match &input.coverage_metrics {
        Some(metrics) => {
            match coverage_labels_from_metrics(&input.corpus_row_id, &input.chunk_id, metrics) {
                Ok(mut labels) => extraction.coverage.append(&mut labels),
                Err(err) => extraction.quarantines.push(quarantine(
                    input,
                    CompositeRewardSignalKind::Coverage,
                    "COMPOSITE_COVERAGE_LABEL_FAILURE",
                    err.to_string(),
                )),
            }
        }
        None => extraction.quarantines.push(quarantine(
            input,
            CompositeRewardSignalKind::Coverage,
            "COMPOSITE_COVERAGE_MISSING",
            "coverage metrics were not supplied",
        )),
    }

    match &input.complexity_metrics {
        Some(metrics) => {
            match complexity_label_from_metrics(&input.corpus_row_id, &input.chunk_id, metrics) {
                Ok(label) => extraction.complexity.push(label),
                Err(err) => extraction.quarantines.push(quarantine(
                    input,
                    CompositeRewardSignalKind::Complexity,
                    "COMPOSITE_COMPLEXITY_LABEL_FAILURE",
                    err.to_string(),
                )),
            }
        }
        None => extraction.quarantines.push(quarantine(
            input,
            CompositeRewardSignalKind::Complexity,
            "COMPOSITE_COMPLEXITY_MISSING",
            "complexity metrics were not supplied",
        )),
    }

    match &input.churn_input {
        Some(churn_input) => match analyze_churn(churn_input) {
            Ok(label) => extraction.churn.push(label),
            Err(err) => extraction.quarantines.push(quarantine(
                input,
                CompositeRewardSignalKind::Churn,
                "COMPOSITE_CHURN_LABEL_FAILURE",
                err.to_string(),
            )),
        },
        None => extraction.quarantines.push(quarantine(
            input,
            CompositeRewardSignalKind::Churn,
            "COMPOSITE_CHURN_MISSING",
            "churn input was not supplied",
        )),
    }

    match &input.doc_coverage_input {
        Some(doc_input) => match analyze_doc_coverage(doc_input) {
            Ok(label) => extraction.doc_coverage.push(label),
            Err(err) => extraction.quarantines.push(quarantine(
                input,
                CompositeRewardSignalKind::DocCoverage,
                "COMPOSITE_DOC_COVERAGE_LABEL_FAILURE",
                err.to_string(),
            )),
        },
        None => extraction.quarantines.push(quarantine(
            input,
            CompositeRewardSignalKind::DocCoverage,
            "COMPOSITE_DOC_COVERAGE_MISSING",
            "doc coverage input was not supplied",
        )),
    }

    match &input.dep_cve_input {
        Some(dep_input) => match extract_dependency_cve_labels(dep_input) {
            Ok(mut labels) => extraction.dep_cve.append(&mut labels),
            Err(err) => extraction.quarantines.push(quarantine(
                input,
                CompositeRewardSignalKind::DepCve,
                "COMPOSITE_DEP_CVE_LABEL_FAILURE",
                err.to_string(),
            )),
        },
        None => extraction.quarantines.push(quarantine(
            input,
            CompositeRewardSignalKind::DepCve,
            "COMPOSITE_DEP_CVE_MISSING",
            "dependency-CVE input was not supplied",
        )),
    }

    Ok(extraction)
}

pub fn write_composite_reward_raw_outputs(
    root: impl AsRef<Path>,
    row_id: &str,
    extraction: &CompositeRewardExtraction,
) -> InstrumentResult<CompositeRewardRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-reward-signals-v1",
            err.to_string(),
            "ensure the D-root composite reward raw-output directory is writable",
        )
    })?;
    let paths = CompositeRewardRawOutputPaths {
        row_id: row_id.to_string(),
        root: root.clone(),
        coverage_json: root.join("coverage.json"),
        churn_json: root.join("churn.json"),
        complexity_json: root.join("complexity.json"),
        doc_json: root.join("doc.json"),
        depcve_json: root.join("depcve.json"),
        composite_json: root.join("composite.json"),
    };
    write_json_readback(&paths.coverage_json, &extraction.coverage)?;
    write_json_readback(&paths.churn_json, &extraction.churn)?;
    write_json_readback(&paths.complexity_json, &extraction.complexity)?;
    write_json_readback(&paths.doc_json, &extraction.doc_coverage)?;
    write_json_readback(&paths.depcve_json, &extraction.dep_cve)?;
    write_json_readback(&paths.composite_json, extraction)?;
    Ok(paths)
}

pub struct CompositeRewardSignalStore {
    db: DB,
}

impl CompositeRewardSignalStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors =
            std::iter::once(ColumnFamilyDescriptor::new("default", Options::default()))
                .chain(
                    composite_reward_cfs()
                        .iter()
                        .map(|cf| ColumnFamilyDescriptor::new(*cf, cf_options())),
                )
                .collect::<Vec<_>>();
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                "TASK_RWD_209_CFS",
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        for cf in composite_reward_cfs() {
            if db.cf_handle(cf).is_none() {
                return Err(InstrumentError::store(
                    "cf_handle",
                    cf,
                    "column family missing after RocksDB open",
                    "open composite reward storage with the canonical CF descriptors",
                ));
            }
        }
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &CompositeRewardExtraction,
    ) -> InstrumentResult<Vec<CompositeRewardStoredKey>> {
        let mut keys = Vec::new();
        for record in extraction.persisted_records() {
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(
        &self,
        cf: &str,
    ) -> InstrumentResult<Vec<(String, PersistedCompositeRewardSignal)>> {
        let cf = canonical_composite_reward_cf(cf).ok_or_else(|| {
            InstrumentError::store(
                "cf_handle",
                "TASK_RWD_209_CFS",
                format!("unknown composite reward column family {cf}"),
                "use one of the TASK-RWD-209 canonical CF constants",
            )
        })?;
        let cf_handle = self.cf(cf)?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    cf,
                    err.to_string(),
                    "inspect RocksDB iterator state and composite reward CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    cf,
                    err.to_string(),
                    "composite reward signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    cf,
                    err.to_string(),
                    "only mutate composite reward rows through CompositeRewardSignalStore",
                )
            })?;
            rows.push((key, record));
        }
        Ok(rows)
    }

    pub fn count_records(&self, cf: &str) -> InstrumentResult<usize> {
        Ok(self.scan_records(cf)?.len())
    }

    pub fn count_all(&self) -> InstrumentResult<BTreeMap<String, usize>> {
        let mut counts = BTreeMap::new();
        for cf in composite_reward_cfs() {
            counts.insert(cf.to_string(), self.count_records(cf)?);
        }
        Ok(counts)
    }

    pub fn flush_all(&self) -> InstrumentResult<()> {
        for cf in composite_reward_cfs() {
            self.db.flush_cf(self.cf(cf)?).map_err(|err| {
                InstrumentError::store(
                    "flush",
                    cf,
                    err.to_string(),
                    "inspect RocksDB WAL and filesystem state",
                )
            })?;
        }
        Ok(())
    }

    fn put_record(
        &self,
        record: &PersistedCompositeRewardSignal,
    ) -> InstrumentResult<CompositeRewardStoredKey> {
        let signal_kind = record.signal.signal_kind();
        let cf = signal_kind.cf();
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                cf,
                err.to_string(),
                "ensure composite reward signal records remain JSON-serializable",
            )
        })?;
        let key = composite_reward_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf(cf)?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    cf,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self
            .db
            .get_cf(self.cf(cf)?, key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get_cf",
                    cf,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
            .ok_or_else(|| {
                InstrumentError::store(
                    "read_after_write",
                    cf,
                    "composite reward row missing after put_cf",
                    "do not advance reward-signal checkpoints until the CF row is readable",
                )
            })?;
        if readback != value {
            return Err(InstrumentError::store(
                "read_after_write",
                cf,
                "readback bytes differ from written composite reward payload",
                "inspect RocksDB WAL/device integrity before continuing",
            ));
        }
        Ok(CompositeRewardStoredKey {
            cf: cf.to_string(),
            key,
        })
    }

    fn cf(&self, cf: &str) -> InstrumentResult<&rocksdb::ColumnFamily> {
        let cf = canonical_composite_reward_cf(cf).ok_or_else(|| {
            InstrumentError::store(
                "cf_handle",
                "TASK_RWD_209_CFS",
                format!("unknown composite reward column family {cf}"),
                "use one of the TASK-RWD-209 canonical CF constants",
            )
        })?;
        if !composite_reward_cfs().contains(&cf) {
            return Err(InstrumentError::store(
                "cf_handle",
                cf,
                "unknown composite reward column family",
                "use one of the TASK-RWD-209 canonical CF constants",
            ));
        }
        self.db.cf_handle(cf).ok_or_else(|| {
            InstrumentError::store(
                "cf_handle",
                cf,
                "column-family handle not found",
                "open the store through CompositeRewardSignalStore::open",
            )
        })
    }
}

pub fn composite_reward_cfs() -> &'static [&'static str] {
    &[
        CF_MEJEPA_COVERAGE_SIGNALS,
        CF_MEJEPA_COMPLEXITY_SIGNALS,
        CF_MEJEPA_CHURN_SIGNALS,
        CF_MEJEPA_DOC_COVERAGE_SIGNALS,
        CF_MEJEPA_DEP_CVE_SIGNALS,
    ]
}

fn canonical_composite_reward_cf(cf: &str) -> Option<&'static str> {
    composite_reward_cfs()
        .iter()
        .copied()
        .find(|candidate| *candidate == cf)
}

fn persisted(signal: CompositeRewardSignalRecord) -> PersistedCompositeRewardSignal {
    PersistedCompositeRewardSignal {
        schema_version: COMPOSITE_REWARD_SCHEMA_VERSION,
        signal,
    }
}

fn validate_churn_input(input: &ChurnInput) -> InstrumentResult<()> {
    validate_path_component("churn.corpus_row_id", &input.corpus_row_id)?;
    validate_path_component("churn.chunk_id", &input.chunk_id)?;
    validate_nonempty(
        "churn.path",
        &input.path,
        "preserve file path for churn labels",
    )?;
    if input.byte_end <= input.byte_start {
        return invalid(
            "churn.byte_range",
            format!(
                "invalid byte range {}..{}",
                input.byte_start, input.byte_end
            ),
            "provide a non-empty chunk byte range",
        );
    }
    if input.now_unix_secs <= 0 {
        return invalid(
            "churn.now_unix_secs",
            "now_unix_secs must be positive",
            "pass the observation timestamp as Unix seconds",
        );
    }
    if input.commits.is_empty() {
        return invalid(
            "churn.commits",
            "churn commit list is empty",
            "read git blame/log source data before computing churn labels",
        );
    }
    if input.commits.len() > MAX_COMMITS {
        return invalid(
            "churn.commits",
            format!(
                "commit count {} exceeds max {MAX_COMMITS}",
                input.commits.len()
            ),
            "shard git churn windows before computing labels",
        );
    }
    for commit in &input.commits {
        validate_nonempty("churn.commit_sha", &commit.commit_sha, "preserve commit id")?;
        validate_nonempty("churn.author", &commit.author, "preserve commit author")?;
        validate_nonempty(
            "churn.commit_path",
            &commit.path,
            "preserve commit file path",
        )?;
        if commit.committed_unix_secs <= 0 || commit.committed_unix_secs > input.now_unix_secs {
            return invalid(
                "churn.committed_unix_secs",
                format!(
                    "{} has invalid committed_unix_secs {} for now {}",
                    commit.commit_sha, commit.committed_unix_secs, input.now_unix_secs
                ),
                "persist monotonic git commit timestamps before churn labeling",
            );
        }
        if commit.touched_byte_end <= commit.touched_byte_start {
            return invalid(
                "churn.commit_byte_range",
                format!(
                    "{} has invalid byte range {}..{}",
                    commit.commit_sha, commit.touched_byte_start, commit.touched_byte_end
                ),
                "persist non-empty touched byte ranges from git blame/log analysis",
            );
        }
    }
    Ok(())
}

fn validate_doc_input(input: &DocCoverageInput) -> InstrumentResult<()> {
    validate_path_component("doc_coverage.corpus_row_id", &input.corpus_row_id)?;
    validate_path_component("doc_coverage.chunk_id", &input.chunk_id)?;
    if !input.language.trim().eq_ignore_ascii_case("python") {
        return invalid(
            "doc_coverage.language",
            format!(
                "doc coverage currently supports python only, got {}",
                input.language
            ),
            "route non-Python doc coverage through a language-specific TASK-RWD instrument",
        );
    }
    validate_nonempty(
        "doc_coverage.path",
        &input.path,
        "preserve analyzed file path",
    )?;
    validate_source_text("doc_coverage.source_text", &input.source_text)
}

fn validate_dep_cve_input(input: &DepCveInput) -> InstrumentResult<()> {
    validate_path_component("dep_cve.corpus_row_id", &input.corpus_row_id)?;
    validate_path_component("dep_cve.chunk_id", &input.chunk_id)?;
    validate_nonempty("dep_cve.path", &input.path, "preserve analyzed file path")?;
    validate_source_text("dep_cve.source_text", &input.source_text)?;
    if let Some(requirements) = &input.requirements_txt {
        validate_text_len(
            "dep_cve.requirements_txt",
            requirements,
            MAX_REQUIREMENTS_BYTES,
            "shard very large requirements files before dependency-CVE labeling",
        )?;
    }
    if let Some(pyproject) = &input.pyproject_toml {
        validate_text_len(
            "dep_cve.pyproject_toml",
            pyproject,
            MAX_REQUIREMENTS_BYTES,
            "shard very large pyproject files before dependency-CVE labeling",
        )?;
    }
    validate_text_len(
        "dep_cve.advisory_db_json",
        &input.advisory_db_json,
        MAX_ADVISORY_BYTES,
        "provide a bounded local advisory feed snapshot",
    )?;
    if input.advisory_db_json.trim().is_empty() {
        return invalid(
            "dep_cve.advisory_db_json",
            "advisory feed is empty",
            "provide a local Safety/PyUp-style advisory feed snapshot",
        );
    }
    if input.requirements_txt.is_none() && input.pyproject_toml.is_none() {
        return invalid(
            "dep_cve.dependencies",
            "neither requirements.txt nor pyproject.toml was supplied",
            "provide at least one dependency source for dependency-CVE labeling",
        );
    }
    Ok(())
}

fn validate_source_text(field: &'static str, text: &str) -> InstrumentResult<()> {
    validate_nonempty(
        field,
        text,
        "capture source text before computing reward labels",
    )?;
    validate_text_len(
        field,
        text,
        MAX_SOURCE_BYTES,
        "shard very large source files before computing reward labels",
    )?;
    if text.chars().any(|ch| ch == '\0') {
        return invalid(
            field,
            "source contains a NUL byte",
            "store parser-facing UTF-8 source without embedded NUL bytes",
        );
    }
    Ok(())
}

fn validate_text_len(
    field: &'static str,
    text: &str,
    max_len: usize,
    remediation: &'static str,
) -> InstrumentResult<()> {
    if text.len() > max_len {
        return invalid(
            field,
            format!("text length {} exceeds max {max_len} bytes", text.len()),
            remediation,
        );
    }
    Ok(())
}

fn validate_coverage_label(label: &CoverageLabel) -> InstrumentResult<()> {
    validate_ratio("coverage.line_coverage_pct", label.line_coverage_pct)?;
    if let Some(branch) = label.branch_coverage_pct {
        validate_ratio("coverage.branch_coverage_pct", branch)?;
    }
    if !label.coverage_delta.is_finite() || !(-1.0..=1.0).contains(&label.coverage_delta) {
        return invalid(
            "coverage.coverage_delta",
            format!(
                "coverage_delta must be finite in [-1,1], got {}",
                label.coverage_delta
            ),
            "inspect coverage delta aggregation",
        );
    }
    Ok(())
}

fn validate_complexity_label(label: &ComplexityLabel) -> InstrumentResult<()> {
    if !label.mean_cyclomatic_complexity.is_finite() || !label.halstead_volume.is_finite() {
        return invalid(
            "complexity.metrics",
            "complexity label contains non-finite metric",
            "inspect TASK-RWD-201 complexity metric aggregation",
        );
    }
    Ok(())
}

fn validate_churn_label(label: &ChurnLabel) -> InstrumentResult<()> {
    if label.commits_touching_range == 0 {
        return invalid(
            "churn.commits_touching_range",
            "churn label has zero touching commits",
            "do not emit churn labels without byte-range git evidence",
        );
    }
    validate_ratio("churn.recent_churn_fraction", label.recent_churn_fraction)
}

fn validate_doc_label(label: &DocCoverageLabel) -> InstrumentResult<()> {
    validate_ratio("doc_coverage.doc_coverage", label.doc_coverage)?;
    for function in &label.functions {
        validate_ratio("doc_coverage.functions.doc_coverage", function.doc_coverage)?;
    }
    Ok(())
}

fn validate_dep_cve_label(label: &DepCveLabel) -> InstrumentResult<()> {
    if !label.max_cvss.is_finite() || !(0.0..=10.0).contains(&label.max_cvss) {
        return invalid(
            "dep_cve.max_cvss",
            format!("max_cvss must be finite in [0,10], got {}", label.max_cvss),
            "inspect advisory feed CVSS normalization",
        );
    }
    Ok(())
}

fn validate_ratio(field: &'static str, value: f32) -> InstrumentResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(
            field,
            format!("ratio must be finite in [0,1], got {value}"),
            "inspect reward-signal ratio aggregation",
        );
    }
    Ok(())
}

fn collect_function_doc_coverage(
    prefix: &str,
    suite: &[Stmt],
    source: &str,
    functions: &mut Vec<FunctionDocCoverage>,
) -> InstrumentResult<()> {
    for stmt in suite {
        match stmt {
            Stmt::FunctionDef(function) => {
                functions.push(function_doc_coverage(prefix, function, source)?);
                let qualified = qualify(prefix, function.name.as_str());
                collect_function_doc_coverage(&qualified, &function.body, source, functions)?;
            }
            Stmt::ClassDef(class_def) => {
                let qualified = qualify(prefix, class_def.name.as_str());
                collect_function_doc_coverage(&qualified, &class_def.body, source, functions)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn function_doc_coverage(
    prefix: &str,
    function: &StmtFunctionDef,
    source: &str,
) -> InstrumentResult<FunctionDocCoverage> {
    let qualified_name = qualify(prefix, function.name.as_str());
    let docstring = suite_docstring(&function.body, source);
    let params = function
        .parameters
        .iter()
        .filter_map(|param| {
            let name = param.name().as_str().to_string();
            if name == "self" || name == "cls" {
                None
            } else {
                Some(name)
            }
        })
        .collect::<Vec<_>>();
    let documented_parameter_count = docstring
        .as_deref()
        .map(|doc| {
            params
                .iter()
                .filter(|param| doc_mentions_parameter(doc, param))
                .count() as u32
        })
        .unwrap_or(0);
    let parameter_count = params.len() as u32;
    let has_docstring = docstring.is_some();
    let doc_coverage = if !has_docstring {
        0.0
    } else if parameter_count == 0 {
        1.0
    } else {
        documented_parameter_count as f32 / parameter_count as f32
    };
    Ok(FunctionDocCoverage {
        qualified_name,
        has_docstring,
        parameter_count,
        documented_parameter_count,
        doc_coverage,
    })
}

fn suite_docstring(suite: &[Stmt], source: &str) -> Option<String> {
    let first = suite.first()?;
    match first {
        Stmt::Expr(stmt_expr) if matches!(&*stmt_expr.value, Expr::StringLiteral(_)) => Some(
            strip_python_string_literal(slice_range(source, stmt_expr.range())),
        ),
        _ => None,
    }
}

fn slice_range(source: &str, range: ruff_text_size::TextRange) -> &str {
    let start = range.start().to_usize().min(source.len());
    let end = range.end().to_usize().min(source.len()).max(start);
    &source[start..end]
}

fn strip_python_string_literal(raw: &str) -> String {
    let mut text = raw.trim();
    while text
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, 'r' | 'R' | 'u' | 'U' | 'f' | 'F' | 'b' | 'B'))
    {
        text = &text[1..];
    }
    for quote in ["\"\"\"", "'''", "\"", "'"] {
        if text.starts_with(quote) && text.ends_with(quote) && text.len() >= quote.len() * 2 {
            return text[quote.len()..text.len() - quote.len()].to_string();
        }
    }
    text.to_string()
}

fn doc_mentions_parameter(doc: &str, param: &str) -> bool {
    let lower_doc = doc.to_ascii_lowercase();
    let lower_param = param.to_ascii_lowercase();
    lower_doc
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|word| word == lower_param)
}

fn count_comment_lines(source: &str) -> u32 {
    source
        .lines()
        .filter(|line| line.trim_start().starts_with('#'))
        .count() as u32
}

fn parse_dependency_sources(input: &DepCveInput) -> InstrumentResult<BTreeMap<String, String>> {
    let mut deps = BTreeMap::new();
    if let Some(requirements) = &input.requirements_txt {
        parse_requirements_txt(requirements, &mut deps)?;
    }
    if let Some(pyproject) = &input.pyproject_toml {
        parse_pyproject_toml(pyproject, &mut deps)?;
    }
    if deps.len() > MAX_DEPENDENCIES {
        return invalid(
            "dep_cve.dependencies",
            format!(
                "dependency count {} exceeds max {MAX_DEPENDENCIES}",
                deps.len()
            ),
            "shard dependency manifests before CVE labeling",
        );
    }
    Ok(deps)
}

fn parse_requirements_txt(
    requirements: &str,
    deps: &mut BTreeMap<String, String>,
) -> InstrumentResult<()> {
    for (idx, line) in requirements.lines().enumerate() {
        let cleaned = line.split('#').next().unwrap_or("").trim();
        if cleaned.is_empty() || cleaned.starts_with('-') {
            continue;
        }
        parse_dependency_spec(cleaned, idx + 1, deps)?;
    }
    Ok(())
}

fn parse_pyproject_toml(
    pyproject: &str,
    deps: &mut BTreeMap<String, String>,
) -> InstrumentResult<()> {
    let parsed: toml::Value = toml::from_str(pyproject).map_err(|err| {
        InstrumentError::invalid(
            "dep_cve.pyproject_toml",
            format!("pyproject.toml parse failed: {err}"),
            "provide valid TOML before dependency-CVE labeling",
        )
    })?;
    if let Some(project_deps) = parsed
        .get("project")
        .and_then(|project| project.get("dependencies"))
        .and_then(|deps| deps.as_array())
    {
        for (idx, dep) in project_deps.iter().enumerate() {
            let Some(spec) = dep.as_str() else {
                return invalid(
                    "dep_cve.pyproject_toml.project.dependencies",
                    "dependency entry is not a string",
                    "store PEP 621 dependencies as strings",
                );
            };
            parse_dependency_spec(spec, idx + 1, deps)?;
        }
    }
    if let Some(poetry_deps) = parsed
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(|poetry| poetry.get("dependencies"))
        .and_then(|deps| deps.as_table())
    {
        for (name, version) in poetry_deps {
            if name.eq_ignore_ascii_case("python") {
                continue;
            }
            let Some(version) = version.as_str() else {
                continue;
            };
            let spec = format!("{name}=={}", version.trim_start_matches('^'));
            parse_dependency_spec(&spec, 0, deps)?;
        }
    }
    Ok(())
}

fn parse_dependency_spec(
    spec: &str,
    line_no: usize,
    deps: &mut BTreeMap<String, String>,
) -> InstrumentResult<()> {
    let normalized = spec.replace(" (", "").replace(')', "");
    let Some((name, version)) = normalized.split_once("==") else {
        return invalid(
            "dep_cve.dependency_spec",
            format!("dependency spec at line {line_no} is not pinned with ==: {spec}"),
            "pin dependencies before computing dependency-CVE labels",
        );
    };
    let package = normalize_package_name(name.split('[').next().unwrap_or(name).trim());
    let version = version
        .split([';', ' '])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    validate_nonempty(
        "dep_cve.package",
        &package,
        "preserve dependency package name",
    )?;
    validate_nonempty(
        "dep_cve.version",
        &version,
        "preserve pinned dependency version",
    )?;
    deps.insert(package, version);
    Ok(())
}

#[derive(Debug, Clone)]
struct AdvisoryDb {
    packages: BTreeMap<String, Vec<AdvisoryEntry>>,
}

#[derive(Debug, Clone)]
struct AdvisoryEntry {
    id: String,
    affected_versions: Vec<String>,
    cvss: f32,
}

impl AdvisoryEntry {
    fn matches_version(&self, version: &str) -> InstrumentResult<bool> {
        for spec in &self.affected_versions {
            if version_matches_spec(version, spec)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn parse_advisory_db(json_text: &str) -> InstrumentResult<AdvisoryDb> {
    let value: Value = serde_json::from_str(json_text).map_err(|err| {
        InstrumentError::invalid(
            "dep_cve.advisory_db_json",
            format!("advisory JSON parse failed: {err}"),
            "provide a valid local advisory feed snapshot",
        )
    })?;
    let packages_value = value.get("packages").ok_or_else(|| {
        InstrumentError::invalid(
            "dep_cve.advisory_db_json.packages",
            "advisory feed lacks packages field",
            "provide packages as an object or array in the local advisory feed snapshot",
        )
    })?;
    let mut packages = BTreeMap::new();
    match packages_value {
        Value::Object(map) => {
            for (package, advisories) in map {
                packages.insert(
                    normalize_package_name(package),
                    parse_advisory_entries(package, advisories)?,
                );
            }
        }
        Value::Array(rows) => {
            for row in rows {
                let package = row.get("package").and_then(Value::as_str).ok_or_else(|| {
                    InstrumentError::invalid(
                        "dep_cve.advisory_db_json.packages[].package",
                        "advisory package row lacks package string",
                        "preserve package names in advisory rows",
                    )
                })?;
                let advisories = row.get("vulnerabilities").ok_or_else(|| {
                    InstrumentError::invalid(
                        "dep_cve.advisory_db_json.packages[].vulnerabilities",
                        "advisory package row lacks vulnerabilities",
                        "preserve vulnerability arrays in advisory rows",
                    )
                })?;
                packages.insert(
                    normalize_package_name(package),
                    parse_advisory_entries(package, advisories)?,
                );
            }
        }
        _ => {
            return invalid(
                "dep_cve.advisory_db_json.packages",
                "packages must be an object or array",
                "provide a supported local advisory feed shape",
            );
        }
    }
    Ok(AdvisoryDb { packages })
}

fn parse_advisory_entries(package: &str, value: &Value) -> InstrumentResult<Vec<AdvisoryEntry>> {
    let rows = value.as_array().ok_or_else(|| {
        InstrumentError::invalid(
            "dep_cve.advisory_db_json.vulnerabilities",
            format!("advisories for {package} are not an array"),
            "preserve vulnerability arrays in the advisory feed",
        )
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let id = row
            .get("id")
            .or_else(|| row.get("cve"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                InstrumentError::invalid(
                    "dep_cve.advisory.id",
                    format!("advisory for {package} lacks id/cve"),
                    "preserve CVE/advisory identifiers",
                )
            })?;
        let cvss = row
            .get("cvss")
            .or_else(|| row.get("cvss_score"))
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                InstrumentError::invalid(
                    "dep_cve.advisory.cvss",
                    format!("advisory {id} lacks numeric cvss"),
                    "preserve CVSS scores in advisory rows",
                )
            })? as f32;
        if !cvss.is_finite() || !(0.0..=10.0).contains(&cvss) {
            return invalid(
                "dep_cve.advisory.cvss",
                format!("advisory {id} has invalid CVSS {cvss}"),
                "normalize advisory CVSS scores into [0,10]",
            );
        }
        let affected = row
            .get("affected_versions")
            .or_else(|| row.get("vulnerable_versions"))
            .and_then(Value::as_array)
            .ok_or_else(|| {
                InstrumentError::invalid(
                    "dep_cve.advisory.affected_versions",
                    format!("advisory {id} lacks affected version specs"),
                    "preserve affected version constraints in advisory rows",
                )
            })?;
        let affected_versions = affected
            .iter()
            .map(|item| {
                item.as_str().map(ToString::to_string).ok_or_else(|| {
                    InstrumentError::invalid(
                        "dep_cve.advisory.affected_versions[]",
                        format!("advisory {id} has non-string version spec"),
                        "store affected versions as strings",
                    )
                })
            })
            .collect::<InstrumentResult<Vec<_>>>()?;
        entries.push(AdvisoryEntry {
            id: id.to_string(),
            affected_versions,
            cvss,
        });
    }
    Ok(entries)
}

fn parse_python_import_roots(source: &str) -> InstrumentResult<Vec<String>> {
    let parsed = ruff_python_parser::parse_module(source).map_err(|err| {
        InstrumentError::invalid(
            "dep_cve.source_text",
            format!("ruff-python-parser rejected source: {err}"),
            "fix Python syntax before computing dependency-CVE labels",
        )
    })?;
    let module = parsed.into_syntax();
    let mut imports = BTreeSet::new();
    collect_imports(&module.body, &mut imports);
    Ok(imports.into_iter().collect())
}

fn collect_imports(suite: &[Stmt], imports: &mut BTreeSet<String>) {
    for stmt in suite {
        match stmt {
            Stmt::Import(stmt_import) => {
                for alias in &stmt_import.names {
                    if let Some(root) = alias.name.as_str().split('.').next() {
                        if !root.is_empty() {
                            imports.insert(root.to_string());
                        }
                    }
                }
            }
            Stmt::ImportFrom(stmt_import_from) => {
                if let Some(module) = &stmt_import_from.module {
                    if let Some(root) = module.as_str().split('.').next() {
                        if !root.is_empty() {
                            imports.insert(root.to_string());
                        }
                    }
                }
            }
            Stmt::FunctionDef(function) => collect_imports(&function.body, imports),
            Stmt::ClassDef(class_def) => collect_imports(&class_def.body, imports),
            _ => {}
        }
    }
}

fn version_matches_spec(version: &str, spec: &str) -> InstrumentResult<bool> {
    let spec = spec.trim();
    if spec.is_empty() {
        return invalid(
            "dep_cve.version_spec",
            "empty affected version spec",
            "preserve non-empty affected version specs in the advisory feed",
        );
    }
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (op, rhs) = if let Some(rhs) = part.strip_prefix("==") {
            ("==", rhs)
        } else if let Some(rhs) = part.strip_prefix("<=") {
            ("<=", rhs)
        } else if let Some(rhs) = part.strip_prefix(">=") {
            (">=", rhs)
        } else if let Some(rhs) = part.strip_prefix('<') {
            ("<", rhs)
        } else if let Some(rhs) = part.strip_prefix('>') {
            (">", rhs)
        } else {
            ("==", part)
        };
        let ordering = compare_versions(version, rhs.trim())?;
        let ok = match op {
            "==" => ordering == Ordering::Equal,
            "<=" => matches!(ordering, Ordering::Less | Ordering::Equal),
            ">=" => matches!(ordering, Ordering::Greater | Ordering::Equal),
            "<" => ordering == Ordering::Less,
            ">" => ordering == Ordering::Greater,
            _ => unreachable!(),
        };
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

fn compare_versions(left: &str, right: &str) -> InstrumentResult<Ordering> {
    let left_parts = parse_version_parts(left)?;
    let right_parts = parse_version_parts(right)?;
    let max_len = left_parts.len().max(right_parts.len());
    for idx in 0..max_len {
        let l = *left_parts.get(idx).unwrap_or(&0);
        let r = *right_parts.get(idx).unwrap_or(&0);
        match l.cmp(&r) {
            Ordering::Equal => {}
            other => return Ok(other),
        }
    }
    Ok(Ordering::Equal)
}

fn parse_version_parts(version: &str) -> InstrumentResult<Vec<u64>> {
    let mut parts = Vec::new();
    for raw in version.split('.') {
        let digits = raw
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if digits.is_empty() {
            return invalid(
                "dep_cve.version",
                format!("version part is not numeric in {version}"),
                "use normalized numeric Python package versions for CVE matching",
            );
        }
        let value = digits.parse::<u64>().map_err(|err| {
            InstrumentError::invalid(
                "dep_cve.version",
                format!("version part parse failed in {version}: {err}"),
                "use normalized numeric Python package versions for CVE matching",
            )
        })?;
        parts.push(value);
    }
    Ok(parts)
}

fn normalize_package_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('_', "-")
}

fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

fn qualify(prefix: &str, name: &str) -> String {
    if prefix == "module" {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn quarantine(
    input: &CompositeRewardInput,
    signal_kind: CompositeRewardSignalKind,
    reason_code: impl Into<String>,
    detail: impl Into<String>,
) -> CompositeRewardQuarantine {
    CompositeRewardQuarantine {
        corpus_row_id: input.corpus_row_id.clone(),
        chunk_id: input.chunk_id.clone(),
        path: input.path.clone(),
        signal_kind,
        reason_code: reason_code.into(),
        detail: detail.into(),
    }
}

fn composite_reward_record_key(record: &PersistedCompositeRewardSignal, value: &[u8]) -> String {
    let signal_kind = record.signal.signal_kind();
    let (row, chunk, subject) = record.signal.identity_parts();
    let hash = fnv1a64(value);
    format!(
        "task-rwd-209::{kind}::{row}::{chunk}::{subject}::{hash:016x}",
        kind = signal_kind.as_key_part()
    )
}

fn write_json_readback<T: Serialize>(path: &Path, value: &T) -> InstrumentResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| {
        InstrumentError::store(
            "serialize",
            "python-reward-signals-v1",
            err.to_string(),
            "ensure composite reward raw outputs remain JSON-serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write",
            "python-reward-signals-v1",
            err.to_string(),
            "ensure the D-root composite reward raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "readback",
            "python-reward-signals-v1",
            err.to_string(),
            "read raw output bytes after write before advancing checkpoints",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "readback",
            "python-reward-signals-v1",
            "raw output readback bytes differ from written bytes",
            "inspect filesystem integrity before advancing checkpoints",
        ));
    }
    Ok(())
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn validate_nonempty(
    field: &'static str,
    value: &str,
    remediation: &'static str,
) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(field, "value is empty", remediation);
    }
    if value.chars().any(|ch| ch == '\0') {
        return invalid(field, "value contains a NUL byte", remediation);
    }
    Ok(())
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_nonempty(field, value, "preserve a non-empty identifier")?;
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return invalid(
            field,
            format!("invalid path component: {value}"),
            "use opaque row/chunk identifiers without path separators",
        );
    }
    Ok(())
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> i64 {
        1_779_000_000
    }

    #[test]
    fn churn_happy_path_counts_touching_authors_age_and_recent_fraction() {
        let input = ChurnInput {
            corpus_row_id: "row1".into(),
            chunk_id: "chunk1".into(),
            path: "pkg/mod.py".into(),
            byte_start: 10,
            byte_end: 110,
            now_unix_secs: now(),
            commits: vec![
                commit("a1", "alice", 30, 10, 110),
                commit("a2", "bob", 25, 20, 80),
                commit("a3", "alice", 20, 0, 20),
                commit("a4", "bob", 15, 90, 130),
                commit("a5", "alice", 1, 10, 110),
            ],
        };
        let label = analyze_churn(&input).expect("churn label");
        assert_eq!(label.commits_touching_range, 5);
        assert_eq!(label.distinct_authors, 2);
        assert_eq!(label.age_days, 30);
        assert_eq!(label.recent_churn_fraction, 1.0);
    }

    #[test]
    fn doc_coverage_happy_path_is_full_when_doc_mentions_parameter() {
        let input = DocCoverageInput {
            corpus_row_id: "row1".into(),
            chunk_id: "chunk1".into(),
            language: "python".into(),
            path: "pkg/mod.py".into(),
            source_text: "def f(value):\n    \"\"\"Return doubled value.\n\n    Args:\n        value: number to double.\n    \"\"\"\n    return value * 2\n".into(),
        };
        let label = analyze_doc_coverage(&input).expect("doc coverage");
        assert_eq!(label.function_count, 1);
        assert_eq!(label.documented_parameter_count, 1);
        assert_eq!(label.doc_coverage, 1.0);
    }

    #[test]
    fn doc_coverage_missing_docstring_is_zero() {
        let input = DocCoverageInput {
            corpus_row_id: "row1".into(),
            chunk_id: "chunk1".into(),
            language: "python".into(),
            path: "pkg/mod.py".into(),
            source_text: "def f(value):\n    return value * 2\n".into(),
        };
        let label = analyze_doc_coverage(&input).expect("doc coverage");
        assert_eq!(label.doc_coverage, 0.0);
        assert_eq!(label.documented_function_count, 0);
    }

    #[test]
    fn dep_cve_matches_pinned_import_against_local_advisory_feed() {
        let input = DepCveInput {
            corpus_row_id: "row1".into(),
            chunk_id: "chunk1".into(),
            path: "pkg/mod.py".into(),
            source_text: "import urllib3\n".into(),
            requirements_txt: Some("urllib3==1.25.0\n".into()),
            pyproject_toml: None,
            advisory_db_json: r#"{
                "packages": {
                    "urllib3": [
                        {"id": "CVE-TEST-0001", "affected_versions": ["<=1.25.1"], "cvss": 9.8}
                    ]
                }
            }"#
            .into(),
        };
        let labels = extract_dependency_cve_labels(&input).expect("dep cve labels");
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].known_cve_count, 1);
        assert_eq!(labels[0].max_cvss, 9.8);
    }

    fn commit(sha: &str, author: &str, days_ago: i64, start: u64, end: u64) -> ChurnCommit {
        ChurnCommit {
            commit_sha: sha.into(),
            author: author.into(),
            committed_unix_secs: now() - days_ago * 24 * 60 * 60,
            path: "pkg/mod.py".into(),
            touched_byte_start: start,
            touched_byte_end: end,
            lines_added: 1,
            lines_deleted: 1,
        }
    }
}

// TASK-RWD-202: deterministic line-level coverage-delta reward signal.
// This additive instrument feeds the existing E_Runtime panel slot.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::features::{add_hashed_pair, bounded_ratio, fnv1a64, validate_finite_output};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_COVERAGE_JSON_BYTES: usize = 50_000_000;
const MAX_FILES: usize = 100_000;
const MAX_LINES_PER_FILE: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageInput {
    pub baseline: CoverageSnapshot,
    pub candidate: CoverageSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageSnapshot {
    pub tool: String,
    pub files: Vec<FileCoverageSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCoverageSnapshot {
    pub path: String,
    pub executable_lines: Vec<u32>,
    pub covered_lines: Vec<u32>,
    pub missing_lines: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageFileDelta {
    pub path: String,
    pub baseline_executable_lines: u32,
    pub candidate_executable_lines: u32,
    pub baseline_covered_lines: u32,
    pub candidate_covered_lines: u32,
    pub baseline_missing_lines: u32,
    pub candidate_missing_lines: u32,
    pub baseline_line_coverage: f32,
    pub candidate_line_coverage: f32,
    pub coverage_delta: f32,
    pub newly_covered_lines: Vec<u32>,
    pub newly_uncovered_lines: Vec<u32>,
    pub candidate_missing_line_numbers: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageMetrics {
    pub analyzer: String,
    pub file_count: u32,
    pub baseline_executable_lines: u32,
    pub candidate_executable_lines: u32,
    pub baseline_covered_lines: u32,
    pub candidate_covered_lines: u32,
    pub candidate_missing_lines: u32,
    pub baseline_line_coverage: f32,
    pub candidate_line_coverage: f32,
    pub coverage_delta: f32,
    pub files_improved: u32,
    pub files_regressed: u32,
    pub files_unchanged: u32,
    pub newly_covered_lines: u32,
    pub newly_uncovered_lines: u32,
    pub files: Vec<CoverageFileDelta>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ECoverageInstrument;

impl Instrument for ECoverageInstrument {
    type Input = CoverageInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::ERuntime
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let metrics = analyze_coverage_delta(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::ERuntime.dim()];
        out[0] = metrics.candidate_line_coverage.clamp(0.0, 1.0);
        out[1] = metrics.baseline_line_coverage.clamp(0.0, 1.0);
        out[2] = metrics.coverage_delta.clamp(-1.0, 1.0);
        out[3] = bounded_ratio(metrics.file_count as f32, 100_000.0);
        out[4] = ratio(metrics.files_improved, metrics.file_count);
        out[5] = ratio(metrics.files_regressed, metrics.file_count);
        out[6] = ratio(metrics.files_unchanged, metrics.file_count);
        out[7] = ratio(
            metrics.newly_covered_lines,
            metrics.candidate_executable_lines,
        );
        out[8] = ratio(
            metrics.newly_uncovered_lines,
            metrics
                .baseline_executable_lines
                .max(metrics.candidate_executable_lines),
        );
        out[9] = ratio(
            metrics.candidate_missing_lines,
            metrics.candidate_executable_lines,
        );
        out[10] = bounded_ratio(metrics.candidate_covered_lines as f32, 1_000_000.0);
        out[11] = bounded_ratio(metrics.baseline_covered_lines as f32, 1_000_000.0);
        out[12] = bounded_ratio(metrics.candidate_executable_lines as f32, 1_000_000.0);
        out[13] = bounded_ratio(metrics.baseline_executable_lines as f32, 1_000_000.0);

        let mut path_mix = 0_u64;
        for file in &metrics.files {
            let candidate_weight = bounded_ratio(file.candidate_executable_lines as f32, 100_000.0);
            add_hashed_pair(&mut out, &file.path, candidate_weight, 32, 64);
            if file.coverage_delta > 0.0 {
                add_hashed_pair(&mut out, &file.path, file.coverage_delta, 96, 32);
            } else if file.coverage_delta < 0.0 {
                add_hashed_pair(&mut out, &file.path, file.coverage_delta.abs(), 128, 32);
            }
            if let Some(extension) = file_extension(&file.path) {
                add_hashed_pair(&mut out, extension, candidate_weight, 160, 24);
            }
            path_mix ^= fnv1a64(file.path.as_bytes());
        }

        for (idx, value) in out.iter_mut().enumerate().skip(192) {
            let seed = (idx as u64)
                ^ ((metrics.file_count as u64) << 7)
                ^ ((metrics.candidate_executable_lines as u64) << 17)
                ^ ((metrics.candidate_covered_lines as u64) << 29)
                ^ ((metrics.newly_covered_lines as u64) << 41)
                ^ ((metrics.newly_uncovered_lines as u64) << 53)
                ^ path_mix;
            *value = (seed.rotate_left((idx % 61) as u32) as f64 / u64::MAX as f64) as f32;
        }

        validate_finite_output("e_coverage.output", &out)?;
        Ok(out)
    }
}

pub fn analyze_coverage_delta(input: &CoverageInput) -> InstrumentResult<CoverageMetrics> {
    validate_input(input)?;
    let baseline = snapshot_map(&input.baseline)?;
    let candidate = snapshot_map(&input.candidate)?;
    let mut paths = baseline.keys().cloned().collect::<BTreeSet<_>>();
    paths.extend(candidate.keys().cloned());

    let mut files = Vec::new();
    let mut files_improved = 0_u32;
    let mut files_regressed = 0_u32;
    let mut files_unchanged = 0_u32;
    let mut newly_covered_total = 0_u32;
    let mut newly_uncovered_total = 0_u32;

    for path in paths {
        let baseline_stats = baseline
            .get(&path)
            .map(FileCoverageStats::from_snapshot)
            .transpose()?
            .unwrap_or_else(FileCoverageStats::empty);
        let candidate_stats = candidate
            .get(&path)
            .map(FileCoverageStats::from_snapshot)
            .transpose()?
            .unwrap_or_else(FileCoverageStats::empty);
        let newly_covered_lines = candidate_stats
            .covered
            .difference(&baseline_stats.covered)
            .copied()
            .collect::<Vec<_>>();
        let newly_uncovered_lines = baseline_stats
            .covered
            .difference(&candidate_stats.covered)
            .copied()
            .collect::<Vec<_>>();
        let coverage_delta = candidate_stats.line_coverage - baseline_stats.line_coverage;
        if coverage_delta > f32::EPSILON {
            files_improved = files_improved.saturating_add(1);
        } else if coverage_delta < -f32::EPSILON {
            files_regressed = files_regressed.saturating_add(1);
        } else {
            files_unchanged = files_unchanged.saturating_add(1);
        }
        newly_covered_total = newly_covered_total.saturating_add(newly_covered_lines.len() as u32);
        newly_uncovered_total =
            newly_uncovered_total.saturating_add(newly_uncovered_lines.len() as u32);
        files.push(CoverageFileDelta {
            path,
            baseline_executable_lines: baseline_stats.executable.len() as u32,
            candidate_executable_lines: candidate_stats.executable.len() as u32,
            baseline_covered_lines: baseline_stats.covered.len() as u32,
            candidate_covered_lines: candidate_stats.covered.len() as u32,
            baseline_missing_lines: baseline_stats.missing.len() as u32,
            candidate_missing_lines: candidate_stats.missing.len() as u32,
            baseline_line_coverage: baseline_stats.line_coverage,
            candidate_line_coverage: candidate_stats.line_coverage,
            coverage_delta,
            newly_covered_lines,
            newly_uncovered_lines,
            candidate_missing_line_numbers: candidate_stats.missing.into_iter().collect(),
        });
    }

    build_metrics(
        files,
        files_improved,
        files_regressed,
        files_unchanged,
        newly_covered_total,
        newly_uncovered_total,
    )
}

pub fn coverage_snapshot_from_coverage_py_json(
    tool: impl Into<String>,
    json_text: &str,
) -> InstrumentResult<CoverageSnapshot> {
    validate_json_text(json_text)?;
    let value: Value = serde_json::from_str(json_text).map_err(|err| {
        InstrumentError::invalid(
            "coverage_json",
            format!("coverage.py JSON parse failed: {err}"),
            "capture valid pytest-cov/coverage.py JSON output",
        )
    })?;
    let files_obj = value
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            InstrumentError::invalid(
                "coverage_json.files",
                "coverage.py JSON does not contain an object at files",
                "run pytest-cov with --cov-report=json and preserve the files object",
            )
        })?;
    if files_obj.is_empty() {
        return Err(InstrumentError::invalid(
            "coverage_json.files",
            "coverage.py JSON files object is empty",
            "capture at least one file coverage entry",
        ));
    }

    let mut files = Vec::new();
    for (path, file_value) in files_obj {
        let covered_lines = match parse_line_array(file_value, "executed_lines")? {
            Some(lines) => lines,
            None => parse_line_array(file_value, "covered_lines")?.ok_or_else(|| {
                InstrumentError::invalid(
                    "coverage_json.files[].executed_lines",
                    format!("{path} has no executed_lines or covered_lines array"),
                    "preserve coverage.py executed_lines for line-level labels",
                )
            })?,
        };
        let missing_lines = parse_line_array(file_value, "missing_lines")?.unwrap_or_default();
        let mut executable = covered_lines
            .iter()
            .chain(missing_lines.iter())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        executable.sort_unstable();
        files.push(FileCoverageSnapshot {
            path: path.clone(),
            executable_lines: executable,
            covered_lines,
            missing_lines,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let snapshot = CoverageSnapshot {
        tool: tool.into(),
        files,
    };
    validate_snapshot("coverage_json", &snapshot)?;
    Ok(snapshot)
}

fn validate_json_text(json_text: &str) -> InstrumentResult<()> {
    if json_text.trim().is_empty() {
        return Err(InstrumentError::invalid(
            "coverage_json",
            "coverage JSON is empty",
            "capture valid pytest-cov/coverage.py JSON output",
        ));
    }
    if json_text.len() > MAX_COVERAGE_JSON_BYTES {
        return Err(InstrumentError::invalid(
            "coverage_json",
            format!(
                "coverage JSON length {} exceeds max {} bytes",
                json_text.len(),
                MAX_COVERAGE_JSON_BYTES
            ),
            "shard extremely large coverage reports before TASK-RWD-202 encoding",
        ));
    }
    if json_text.chars().any(|ch| ch == '\0') {
        return Err(InstrumentError::invalid(
            "coverage_json",
            "coverage JSON contains a NUL byte",
            "persist parser-facing UTF-8 JSON without embedded NUL bytes",
        ));
    }
    Ok(())
}

fn parse_line_array(value: &Value, field: &'static str) -> InstrumentResult<Option<Vec<u32>>> {
    let Some(array) = value.get(field) else {
        return Ok(None);
    };
    let array = array.as_array().ok_or_else(|| {
        InstrumentError::invalid(
            "coverage_json.files[].lines",
            format!("{field} is not an array"),
            "preserve coverage.py line arrays as JSON integer arrays",
        )
    })?;
    let mut lines = Vec::with_capacity(array.len());
    for item in array {
        let line = item.as_u64().ok_or_else(|| {
            InstrumentError::invalid(
                "coverage_json.files[].lines",
                format!("{field} contains a non-integer value {item}"),
                "coverage line arrays must contain positive integer line numbers",
            )
        })?;
        if line == 0 || line > u32::MAX as u64 {
            return Err(InstrumentError::invalid(
                "coverage_json.files[].lines",
                format!("{field} contains invalid line number {line}"),
                "coverage line arrays must contain positive u32 line numbers",
            ));
        }
        lines.push(line as u32);
    }
    lines.sort_unstable();
    Ok(Some(lines))
}

fn validate_input(input: &CoverageInput) -> InstrumentResult<()> {
    validate_snapshot("input.baseline", &input.baseline)?;
    validate_snapshot("input.candidate", &input.candidate)?;
    Ok(())
}

fn validate_snapshot(field: &'static str, snapshot: &CoverageSnapshot) -> InstrumentResult<()> {
    validate_text(
        field,
        &snapshot.tool,
        "store coverage tool identifiers as non-empty single-line UTF-8",
    )?;
    if snapshot.files.is_empty() {
        return Err(InstrumentError::invalid(
            field,
            "coverage snapshot has no files",
            "persist at least one file coverage row per snapshot",
        ));
    }
    if snapshot.files.len() > MAX_FILES {
        return Err(InstrumentError::invalid(
            field,
            format!(
                "coverage snapshot has {} files; max is {MAX_FILES}",
                snapshot.files.len()
            ),
            "shard very large coverage reports before encoding",
        ));
    }
    let mut seen_paths = BTreeSet::new();
    for file in &snapshot.files {
        validate_file_snapshot(field, file)?;
        if !seen_paths.insert(file.path.clone()) {
            return Err(InstrumentError::invalid(
                field,
                format!("duplicate coverage path {}", file.path),
                "deduplicate coverage rows by canonical path before encoding",
            ));
        }
    }
    Ok(())
}

fn validate_file_snapshot(
    field: &'static str,
    file: &FileCoverageSnapshot,
) -> InstrumentResult<()> {
    validate_text(
        field,
        &file.path,
        "store file paths as non-empty single-line UTF-8",
    )?;
    if file.executable_lines.is_empty() {
        return Err(InstrumentError::invalid(
            field,
            format!("{} has no executable lines", file.path),
            "coverage snapshots must preserve executable statement lines",
        ));
    }
    if file.executable_lines.len() > MAX_LINES_PER_FILE {
        return Err(InstrumentError::invalid(
            field,
            format!(
                "{} has {} executable lines; max is {MAX_LINES_PER_FILE}",
                file.path,
                file.executable_lines.len()
            ),
            "shard extremely large generated files before encoding coverage",
        ));
    }
    let executable = checked_line_set(
        field,
        &file.path,
        "executable_lines",
        &file.executable_lines,
    )?;
    let covered = checked_line_set(field, &file.path, "covered_lines", &file.covered_lines)?;
    let missing = checked_line_set(field, &file.path, "missing_lines", &file.missing_lines)?;
    if !covered.is_subset(&executable) {
        return Err(InstrumentError::invalid(
            field,
            format!(
                "{} covered_lines are not a subset of executable_lines",
                file.path
            ),
            "normalize coverage.py line arrays before encoding",
        ));
    }
    if !missing.is_subset(&executable) {
        return Err(InstrumentError::invalid(
            field,
            format!(
                "{} missing_lines are not a subset of executable_lines",
                file.path
            ),
            "normalize coverage.py line arrays before encoding",
        ));
    }
    if !covered.is_disjoint(&missing) {
        return Err(InstrumentError::invalid(
            field,
            format!("{} covered_lines and missing_lines overlap", file.path),
            "coverage line sets must be disjoint",
        ));
    }
    Ok(())
}

fn checked_line_set(
    field: &'static str,
    path: &str,
    label: &'static str,
    lines: &[u32],
) -> InstrumentResult<BTreeSet<u32>> {
    let mut set = BTreeSet::new();
    for line in lines {
        if *line == 0 {
            return Err(InstrumentError::invalid(
                field,
                format!("{path} {label} contains line 0"),
                "coverage line numbers must be one-based",
            ));
        }
        if !set.insert(*line) {
            return Err(InstrumentError::invalid(
                field,
                format!("{path} {label} contains duplicate line {line}"),
                "deduplicate coverage line arrays before encoding",
            ));
        }
    }
    Ok(set)
}

fn validate_text(
    field: &'static str,
    value: &str,
    remediation: &'static str,
) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return Err(InstrumentError::invalid(
            field,
            "text field is empty or whitespace-only",
            remediation,
        ));
    }
    if value
        .chars()
        .any(|ch| ch == '\0' || ch == '\n' || ch == '\r')
    {
        return Err(InstrumentError::invalid(
            field,
            "text field contains a control character",
            remediation,
        ));
    }
    Ok(())
}

fn snapshot_map(
    snapshot: &CoverageSnapshot,
) -> InstrumentResult<BTreeMap<String, FileCoverageSnapshot>> {
    let mut map = BTreeMap::new();
    for file in &snapshot.files {
        if map.insert(file.path.clone(), file.clone()).is_some() {
            return Err(InstrumentError::invalid(
                "coverage.snapshot",
                format!("duplicate coverage path {}", file.path),
                "deduplicate coverage rows by canonical path before encoding",
            ));
        }
    }
    Ok(map)
}

#[derive(Debug)]
struct FileCoverageStats {
    executable: BTreeSet<u32>,
    covered: BTreeSet<u32>,
    missing: BTreeSet<u32>,
    line_coverage: f32,
}

impl FileCoverageStats {
    fn from_snapshot(file: &FileCoverageSnapshot) -> InstrumentResult<Self> {
        let executable = checked_line_set(
            "coverage.file",
            &file.path,
            "executable_lines",
            &file.executable_lines,
        )?;
        let covered = checked_line_set(
            "coverage.file",
            &file.path,
            "covered_lines",
            &file.covered_lines,
        )?;
        let missing = checked_line_set(
            "coverage.file",
            &file.path,
            "missing_lines",
            &file.missing_lines,
        )?;
        let line_coverage = if executable.is_empty() {
            0.0
        } else {
            covered.len() as f32 / executable.len() as f32
        };
        Ok(Self {
            executable,
            covered,
            missing,
            line_coverage,
        })
    }

    fn empty() -> Self {
        Self {
            executable: BTreeSet::new(),
            covered: BTreeSet::new(),
            missing: BTreeSet::new(),
            line_coverage: 0.0,
        }
    }
}

fn build_metrics(
    files: Vec<CoverageFileDelta>,
    files_improved: u32,
    files_regressed: u32,
    files_unchanged: u32,
    newly_covered_lines: u32,
    newly_uncovered_lines: u32,
) -> InstrumentResult<CoverageMetrics> {
    let file_count = files.len() as u32;
    let baseline_executable_lines = files
        .iter()
        .map(|file| file.baseline_executable_lines)
        .sum::<u32>();
    let candidate_executable_lines = files
        .iter()
        .map(|file| file.candidate_executable_lines)
        .sum::<u32>();
    let baseline_covered_lines = files
        .iter()
        .map(|file| file.baseline_covered_lines)
        .sum::<u32>();
    let candidate_covered_lines = files
        .iter()
        .map(|file| file.candidate_covered_lines)
        .sum::<u32>();
    let candidate_missing_lines = files
        .iter()
        .map(|file| file.candidate_missing_lines)
        .sum::<u32>();
    let baseline_line_coverage = ratio(baseline_covered_lines, baseline_executable_lines);
    let candidate_line_coverage = ratio(candidate_covered_lines, candidate_executable_lines);
    let metrics = CoverageMetrics {
        analyzer: "contextgraph_coverage_delta_v1".to_string(),
        file_count,
        baseline_executable_lines,
        candidate_executable_lines,
        baseline_covered_lines,
        candidate_covered_lines,
        candidate_missing_lines,
        baseline_line_coverage,
        candidate_line_coverage,
        coverage_delta: candidate_line_coverage - baseline_line_coverage,
        files_improved,
        files_regressed,
        files_unchanged,
        newly_covered_lines,
        newly_uncovered_lines,
        files,
    };
    validate_metrics(&metrics)?;
    Ok(metrics)
}

fn validate_metrics(metrics: &CoverageMetrics) -> InstrumentResult<()> {
    for value in [
        metrics.baseline_line_coverage,
        metrics.candidate_line_coverage,
        metrics.coverage_delta,
    ] {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "coverage.metrics",
                format!("metric is non-finite: {value}"),
                "inspect TASK-RWD-202 coverage aggregation",
            ));
        }
    }
    if metrics.file_count == 0 || metrics.candidate_executable_lines == 0 {
        return Err(InstrumentError::invalid(
            "coverage.metrics",
            "candidate coverage contains no executable lines",
            "provide real line-level coverage for at least one candidate file",
        ));
    }
    if metrics.files_improved + metrics.files_regressed + metrics.files_unchanged
        != metrics.file_count
    {
        return Err(InstrumentError::invariant(
            "coverage.metrics.file_counts",
            "file improvement/regression/unchanged counts do not sum to file_count",
            "inspect TASK-RWD-202 file delta classification",
        ));
    }
    Ok(())
}

fn ratio(numerator: u32, denominator: u32) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn file_extension(path: &str) -> Option<&str> {
    path.rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty() && !extension.contains('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> CoverageInput {
        CoverageInput {
            baseline: coverage_snapshot_from_coverage_py_json("coverage.py", baseline_json())
                .unwrap(),
            candidate: coverage_snapshot_from_coverage_py_json("coverage.py", candidate_json())
                .unwrap(),
        }
    }

    fn baseline_json() -> &'static str {
        r#"{
  "meta": {"version": "7.5.0"},
  "files": {
    "src/a.py": {
      "executed_lines": [1, 2],
      "missing_lines": [3, 4]
    },
    "src/b.py": {
      "executed_lines": [1],
      "missing_lines": [2]
    }
  }
}"#
    }

    fn candidate_json() -> &'static str {
        r#"{
  "meta": {"version": "7.5.0"},
  "files": {
    "src/a.py": {
      "executed_lines": [1, 2, 3],
      "missing_lines": [4]
    },
    "src/b.py": {
      "executed_lines": [2],
      "missing_lines": [1]
    }
  }
}"#
    }

    #[test]
    fn parses_coverage_py_json_and_computes_line_delta() {
        let metrics = analyze_coverage_delta(&sample_input()).unwrap();
        assert_eq!(metrics.file_count, 2);
        assert_eq!(metrics.baseline_executable_lines, 6);
        assert_eq!(metrics.candidate_executable_lines, 6);
        assert_eq!(metrics.baseline_covered_lines, 3);
        assert_eq!(metrics.candidate_covered_lines, 4);
        assert!((metrics.baseline_line_coverage - 0.5).abs() < 0.0001);
        assert!((metrics.candidate_line_coverage - (4.0 / 6.0)).abs() < 0.0001);
        assert!((metrics.coverage_delta - (1.0 / 6.0)).abs() < 0.0001);
        assert_eq!(metrics.files_improved, 1);
        assert_eq!(metrics.files_regressed, 0);
        assert_eq!(metrics.files_unchanged, 1);
        assert_eq!(metrics.newly_covered_lines, 2);
        assert_eq!(metrics.newly_uncovered_lines, 1);
    }

    #[test]
    fn records_file_level_newly_covered_and_uncovered_lines() {
        let metrics = analyze_coverage_delta(&sample_input()).unwrap();
        let a = metrics
            .files
            .iter()
            .find(|file| file.path == "src/a.py")
            .unwrap();
        assert_eq!(a.newly_covered_lines, vec![3]);
        assert!(a.newly_uncovered_lines.is_empty());
        let b = metrics
            .files
            .iter()
            .find(|file| file.path == "src/b.py")
            .unwrap();
        assert_eq!(b.newly_covered_lines, vec![2]);
        assert_eq!(b.newly_uncovered_lines, vec![1]);
    }

    #[test]
    fn instrument_encodes_runtime_slot_vector() {
        let inst = ECoverageInstrument;
        let encoded = inst.encode(&sample_input()).unwrap();
        assert_eq!(encoded.len(), InstrumentSlot::ERuntime.dim());
        assert!(encoded.iter().all(|value| value.is_finite()));
        assert!(encoded.iter().any(|value| *value > 0.0));
        assert!(encoded[0] > encoded[1]);
    }

    #[test]
    fn invalid_coverage_inputs_fail_closed() {
        assert_eq!(
            coverage_snapshot_from_coverage_py_json("coverage.py", "")
                .unwrap_err()
                .code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        assert_eq!(
            coverage_snapshot_from_coverage_py_json("coverage.py", r#"{"files":{}}"#)
                .unwrap_err()
                .code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        let inst = ECoverageInstrument;
        let mut bad = sample_input();
        bad.candidate.files[0].covered_lines.push(1);
        assert_eq!(
            inst.encode(&bad).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        let mut bad = sample_input();
        bad.candidate.files[0].missing_lines.push(3);
        assert_eq!(
            inst.encode(&bad).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}

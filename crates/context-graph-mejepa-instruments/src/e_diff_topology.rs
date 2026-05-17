// TASK-RWD-203: deterministic unified-diff topology reward signal.
// This additive instrument feeds the existing E_Diff panel slot.

use serde::{Deserialize, Serialize};

use crate::features::{add_hashed_pair, bounded_ratio, fnv1a64, validate_finite_output};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

const MAX_DIFF_BYTES: usize = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffTopologyInput {
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HunkTopology {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    pub added_lines: u32,
    pub removed_lines: u32,
    pub context_lines: u32,
    pub changed_lines: u32,
    pub total_lines: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileTopology {
    pub old_path: String,
    pub new_path: String,
    pub hunk_count: u32,
    pub added_lines: u32,
    pub removed_lines: u32,
    pub context_lines: u32,
    pub changed_lines: u32,
    pub largest_hunk_changed_lines: u32,
    pub intra_file_hunk_entropy_bits: f32,
    pub hunks: Vec<HunkTopology>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffTopologyMetrics {
    pub analyzer: String,
    pub file_count: u32,
    pub hunk_count: u32,
    pub added_lines: u32,
    pub removed_lines: u32,
    pub context_lines: u32,
    pub changed_lines: u32,
    pub max_hunks_per_file: u32,
    pub mean_hunks_per_file: f32,
    pub max_hunk_changed_lines: u32,
    pub mean_hunk_changed_lines: f32,
    pub file_change_entropy_bits: f32,
    pub mean_intra_file_hunk_entropy_bits: f32,
    pub max_intra_file_hunk_entropy_bits: f32,
    pub files: Vec<FileTopology>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EDiffTopologyInstrument;

impl Instrument for EDiffTopologyInstrument {
    type Input = DiffTopologyInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EDiff
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        let metrics = analyze_diff_topology(input)?;
        let mut out = vec![0.0_f32; InstrumentSlot::EDiff.dim()];
        out[0] = bounded_ratio(metrics.file_count as f32, 10_000.0);
        out[1] = bounded_ratio(metrics.hunk_count as f32, 100_000.0);
        out[2] = bounded_ratio(metrics.added_lines as f32, 1_000_000.0);
        out[3] = bounded_ratio(metrics.removed_lines as f32, 1_000_000.0);
        out[4] = bounded_ratio(metrics.context_lines as f32, 1_000_000.0);
        out[5] = bounded_ratio(metrics.changed_lines as f32, 1_000_000.0);
        out[6] = bounded_ratio(metrics.max_hunks_per_file as f32, 10_000.0);
        out[7] = bounded_ratio(metrics.mean_hunks_per_file, 1_000.0);
        out[8] = bounded_ratio(metrics.max_hunk_changed_lines as f32, 100_000.0);
        out[9] = bounded_ratio(metrics.mean_hunk_changed_lines, 100_000.0);
        out[10] = bounded_ratio(metrics.file_change_entropy_bits, 16.0);
        out[11] = bounded_ratio(metrics.mean_intra_file_hunk_entropy_bits, 16.0);
        out[12] = bounded_ratio(metrics.max_intra_file_hunk_entropy_bits, 16.0);
        out[13] = ratio(metrics.added_lines, metrics.changed_lines);
        out[14] = ratio(metrics.removed_lines, metrics.changed_lines);
        out[15] = ratio(
            metrics.changed_lines,
            metrics.changed_lines.saturating_add(metrics.context_lines),
        );

        let mut path_mix = 0_u64;
        for file in &metrics.files {
            let changed_weight = bounded_ratio(file.changed_lines as f32, 10_000.0);
            add_hashed_pair(&mut out, effective_path(file), changed_weight, 32, 64);
            add_hashed_pair(
                &mut out,
                effective_path(file),
                bounded_ratio(file.hunk_count as f32, 1_000.0),
                96,
                64,
            );
            if let Some(extension) = file_extension(effective_path(file)) {
                add_hashed_pair(&mut out, extension, changed_weight, 160, 24);
            }
            path_mix ^= fnv1a64(effective_path(file).as_bytes());
        }

        for (idx, value) in out.iter_mut().enumerate().skip(192) {
            let seed = (idx as u64)
                ^ ((metrics.file_count as u64) << 7)
                ^ ((metrics.hunk_count as u64) << 17)
                ^ ((metrics.added_lines as u64) << 29)
                ^ ((metrics.removed_lines as u64) << 43)
                ^ path_mix;
            *value = (seed.rotate_left((idx % 61) as u32) as f64 / u64::MAX as f64) as f32;
        }

        validate_finite_output("e_diff_topology.output", &out)?;
        Ok(out)
    }
}

pub fn analyze_diff_topology(input: &DiffTopologyInput) -> InstrumentResult<DiffTopologyMetrics> {
    validate_input(input)?;
    let files = parse_unified_diff(&input.diff)?;
    build_metrics(files)
}

fn validate_input(input: &DiffTopologyInput) -> InstrumentResult<()> {
    if input.diff.trim().is_empty() {
        return Err(InstrumentError::invalid(
            "input.diff",
            "diff topology input is empty",
            "capture unified diff text before computing TASK-RWD-203 signals",
        ));
    }
    if input.diff.len() > MAX_DIFF_BYTES {
        return Err(InstrumentError::invalid(
            "input.diff",
            format!(
                "diff length {} exceeds max {} bytes",
                input.diff.len(),
                MAX_DIFF_BYTES
            ),
            "shard very large diffs before computing TASK-RWD-203 signals",
        ));
    }
    if input.diff.chars().any(|ch| ch == '\0') {
        return Err(InstrumentError::invalid(
            "input.diff",
            "diff contains a NUL byte",
            "persist parser-facing UTF-8 diff text without embedded NUL bytes",
        ));
    }
    Ok(())
}

fn parse_unified_diff(diff: &str) -> InstrumentResult<Vec<FileTopology>> {
    let mut files = Vec::new();
    let mut current_file: Option<FileAccumulator> = None;
    let mut current_hunk: Option<HunkAccumulator> = None;

    for (idx, line) in diff.lines().enumerate() {
        let line_no = idx + 1;
        if line.starts_with("diff --git ") {
            finish_hunk(&mut current_file, current_hunk.take())?;
            finish_file(&mut files, current_file.take())?;
            current_file = Some(FileAccumulator::from_diff_git(line, line_no)?);
            continue;
        }

        if line.starts_with("--- ") && !line.starts_with("----") {
            if current_hunk.is_some() {
                return invalid_line(
                    line_no,
                    "old-file header appeared inside a hunk",
                    "keep file headers before hunk bodies",
                );
            }
            let file = current_file.get_or_insert_with(FileAccumulator::default);
            file.old_path = Some(parse_file_header_path(line, "--- ", line_no)?);
            continue;
        }

        if line.starts_with("+++ ") && !line.starts_with("++++") {
            if current_hunk.is_some() {
                return invalid_line(
                    line_no,
                    "new-file header appeared inside a hunk",
                    "keep file headers before hunk bodies",
                );
            }
            let file = current_file.get_or_insert_with(FileAccumulator::default);
            file.new_path = Some(parse_file_header_path(line, "+++ ", line_no)?);
            continue;
        }

        if line.starts_with("@@ ") {
            finish_hunk(&mut current_file, current_hunk.take())?;
            if current_file.is_none() {
                return invalid_line(
                    line_no,
                    "hunk header appeared before a file header",
                    "include diff --git or ---/+++ headers before @@ hunks",
                );
            }
            current_hunk = Some(HunkAccumulator::from_header(line, line_no)?);
            continue;
        }

        if let Some(hunk) = current_hunk.as_mut() {
            classify_hunk_line(hunk, line, line_no)?;
        } else if looks_like_hunk_body(line) {
            return invalid_line(
                line_no,
                "hunk body line appeared before a hunk header",
                "include an @@ header before added/removed/context lines",
            );
        }
    }

    finish_hunk(&mut current_file, current_hunk)?;
    finish_file(&mut files, current_file)?;

    if files.is_empty() {
        return Err(InstrumentError::invalid(
            "input.diff",
            "diff contained no parseable file hunks",
            "provide unified diff text with at least one file and one @@ hunk",
        ));
    }
    Ok(files)
}

fn finish_hunk(
    current_file: &mut Option<FileAccumulator>,
    hunk: Option<HunkAccumulator>,
) -> InstrumentResult<()> {
    if let Some(hunk) = hunk {
        let finished = hunk.finish()?;
        match current_file {
            Some(file) => file.hunks.push(finished),
            None => {
                return Err(InstrumentError::invalid(
                    "input.diff",
                    "hunk completed without a file header",
                    "include diff --git or ---/+++ headers before @@ hunks",
                ));
            }
        }
    }
    Ok(())
}

fn finish_file(
    files: &mut Vec<FileTopology>,
    file: Option<FileAccumulator>,
) -> InstrumentResult<()> {
    if let Some(file) = file {
        files.push(file.finish()?);
    }
    Ok(())
}

#[derive(Default)]
struct FileAccumulator {
    old_path: Option<String>,
    new_path: Option<String>,
    hunks: Vec<HunkTopology>,
}

impl FileAccumulator {
    fn from_diff_git(line: &str, line_no: usize) -> InstrumentResult<Self> {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 4 {
            return invalid_line(
                line_no,
                "malformed diff --git header",
                "emit headers in the form: diff --git a/path b/path",
            );
        }
        Ok(Self {
            old_path: Some(normalize_path(parts[2], line_no)?),
            new_path: Some(normalize_path(parts[3], line_no)?),
            hunks: Vec::new(),
        })
    }

    fn finish(self) -> InstrumentResult<FileTopology> {
        if self.hunks.is_empty() {
            return Err(InstrumentError::invalid(
                "input.diff",
                "file diff contained no hunks",
                "provide unified diff hunks for every parsed file",
            ));
        }
        let old_path = self.old_path.unwrap_or_else(|| "/dev/null".to_string());
        let new_path = self.new_path.unwrap_or_else(|| "/dev/null".to_string());
        validate_path("file.old_path", &old_path)?;
        validate_path("file.new_path", &new_path)?;

        let added_lines = self.hunks.iter().map(|hunk| hunk.added_lines).sum();
        let removed_lines = self.hunks.iter().map(|hunk| hunk.removed_lines).sum();
        let context_lines = self.hunks.iter().map(|hunk| hunk.context_lines).sum();
        let changed_lines = self.hunks.iter().map(|hunk| hunk.changed_lines).sum();
        let largest_hunk_changed_lines = self
            .hunks
            .iter()
            .map(|hunk| hunk.changed_lines)
            .max()
            .unwrap_or(0);
        let hunk_weights = self
            .hunks
            .iter()
            .map(|hunk| hunk.changed_lines)
            .collect::<Vec<_>>();
        Ok(FileTopology {
            old_path,
            new_path,
            hunk_count: self.hunks.len() as u32,
            added_lines,
            removed_lines,
            context_lines,
            changed_lines,
            largest_hunk_changed_lines,
            intra_file_hunk_entropy_bits: shannon_entropy_bits(&hunk_weights),
            hunks: self.hunks,
        })
    }
}

struct HunkAccumulator {
    old_start: u32,
    old_len: u32,
    new_start: u32,
    new_len: u32,
    added_lines: u32,
    removed_lines: u32,
    context_lines: u32,
}

impl HunkAccumulator {
    fn from_header(line: &str, line_no: usize) -> InstrumentResult<Self> {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 3 {
            return invalid_line(
                line_no,
                "malformed hunk header",
                "emit hunk headers in the form: @@ -old,count +new,count @@",
            );
        }
        let (old_start, old_len) = parse_hunk_range(parts[1], '-', line_no)?;
        let (new_start, new_len) = parse_hunk_range(parts[2], '+', line_no)?;
        Ok(Self {
            old_start,
            old_len,
            new_start,
            new_len,
            added_lines: 0,
            removed_lines: 0,
            context_lines: 0,
        })
    }

    fn finish(self) -> InstrumentResult<HunkTopology> {
        let old_observed = self.removed_lines.saturating_add(self.context_lines);
        let new_observed = self.added_lines.saturating_add(self.context_lines);
        if old_observed != self.old_len {
            return Err(InstrumentError::invalid(
                "input.diff",
                format!(
                    "hunk old-side line count mismatch: header {} observed {}",
                    self.old_len, old_observed
                ),
                "preserve exact unified-diff hunk body lines",
            ));
        }
        if new_observed != self.new_len {
            return Err(InstrumentError::invalid(
                "input.diff",
                format!(
                    "hunk new-side line count mismatch: header {} observed {}",
                    self.new_len, new_observed
                ),
                "preserve exact unified-diff hunk body lines",
            ));
        }
        let changed_lines = self.added_lines.saturating_add(self.removed_lines);
        let total_lines = changed_lines.saturating_add(self.context_lines);
        if total_lines == 0 {
            return Err(InstrumentError::invalid(
                "input.diff",
                "hunk contains no body lines",
                "provide unified-diff hunk body lines after each @@ header",
            ));
        }
        Ok(HunkTopology {
            old_start: self.old_start,
            old_len: self.old_len,
            new_start: self.new_start,
            new_len: self.new_len,
            added_lines: self.added_lines,
            removed_lines: self.removed_lines,
            context_lines: self.context_lines,
            changed_lines,
            total_lines,
        })
    }
}

fn classify_hunk_line(
    hunk: &mut HunkAccumulator,
    line: &str,
    line_no: usize,
) -> InstrumentResult<()> {
    if line.starts_with('+') && !line.starts_with("+++") {
        hunk.added_lines = hunk.added_lines.saturating_add(1);
        Ok(())
    } else if line.starts_with('-') && !line.starts_with("---") {
        hunk.removed_lines = hunk.removed_lines.saturating_add(1);
        Ok(())
    } else if line.starts_with(' ') {
        hunk.context_lines = hunk.context_lines.saturating_add(1);
        Ok(())
    } else if line.starts_with("\\ No newline at end of file") {
        Ok(())
    } else {
        invalid_line(
            line_no,
            "unrecognized hunk body line",
            "unified-diff hunk body lines must start with space, +, -, or no-newline marker",
        )
    }
}

fn looks_like_hunk_body(line: &str) -> bool {
    line.starts_with('+') || line.starts_with('-') || line.starts_with(' ')
}

fn parse_file_header_path(
    line: &str,
    prefix: &'static str,
    line_no: usize,
) -> InstrumentResult<String> {
    let path = line
        .strip_prefix(prefix)
        .ok_or_else(|| {
            InstrumentError::invalid(
                "input.diff",
                format!("line {line_no}: malformed file header"),
                "preserve --- and +++ unified-diff file headers",
            )
        })?
        .split('\t')
        .next()
        .unwrap_or("")
        .trim();
    normalize_path(path, line_no)
}

fn normalize_path(raw: &str, line_no: usize) -> InstrumentResult<String> {
    let mut path = raw.trim();
    if path.len() >= 2 && path.starts_with('"') && path.ends_with('"') {
        path = &path[1..path.len() - 1];
    }
    if path == "/dev/null" {
        return Ok(path.to_string());
    }
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path);
    if path.trim().is_empty() {
        return invalid_line(
            line_no,
            "diff path is empty",
            "persist non-empty source paths in diff headers",
        );
    }
    validate_path("input.diff.path", path)?;
    Ok(path.to_string())
}

fn validate_path(field: &'static str, path: &str) -> InstrumentResult<()> {
    if path.trim().is_empty() {
        return Err(InstrumentError::invalid(
            field,
            "path is empty",
            "persist non-empty source paths in diff headers",
        ));
    }
    if path
        .chars()
        .any(|ch| ch == '\0' || ch == '\n' || ch == '\r')
    {
        return Err(InstrumentError::invalid(
            field,
            "path contains a control character",
            "store diff paths as single-line UTF-8",
        ));
    }
    Ok(())
}

fn parse_hunk_range(token: &str, prefix: char, line_no: usize) -> InstrumentResult<(u32, u32)> {
    let raw = token.strip_prefix(prefix).ok_or_else(|| {
        InstrumentError::invalid(
            "input.diff",
            format!("line {line_no}: hunk range {token:?} missing {prefix} prefix"),
            "preserve @@ -old,count +new,count @@ hunk ranges",
        )
    })?;
    let mut parts = raw.split(',');
    let start = parse_u32(parts.next().unwrap_or(""), line_no, "hunk start")?;
    let len = match parts.next() {
        Some(value) => parse_u32(value, line_no, "hunk length")?,
        None => 1,
    };
    if parts.next().is_some() {
        return invalid_line(
            line_no,
            "hunk range contains too many comma-separated fields",
            "emit hunk ranges as start or start,length",
        );
    }
    Ok((start, len))
}

fn parse_u32(value: &str, line_no: usize, name: &'static str) -> InstrumentResult<u32> {
    value.parse::<u32>().map_err(|err| {
        InstrumentError::invalid(
            "input.diff",
            format!("line {line_no}: invalid {name} {value:?}: {err}"),
            "preserve numeric unified-diff hunk ranges",
        )
    })
}

fn build_metrics(files: Vec<FileTopology>) -> InstrumentResult<DiffTopologyMetrics> {
    let file_count = files.len() as u32;
    let hunk_count = files.iter().map(|file| file.hunk_count).sum::<u32>();
    let added_lines = files.iter().map(|file| file.added_lines).sum::<u32>();
    let removed_lines = files.iter().map(|file| file.removed_lines).sum::<u32>();
    let context_lines = files.iter().map(|file| file.context_lines).sum::<u32>();
    let changed_lines = files.iter().map(|file| file.changed_lines).sum::<u32>();
    let max_hunks_per_file = files.iter().map(|file| file.hunk_count).max().unwrap_or(0);
    let max_hunk_changed_lines = files
        .iter()
        .map(|file| file.largest_hunk_changed_lines)
        .max()
        .unwrap_or(0);
    let mean_hunks_per_file = if file_count == 0 {
        0.0
    } else {
        hunk_count as f32 / file_count as f32
    };
    let mean_hunk_changed_lines = if hunk_count == 0 {
        0.0
    } else {
        changed_lines as f32 / hunk_count as f32
    };
    let file_weights = files
        .iter()
        .map(|file| file.changed_lines)
        .collect::<Vec<_>>();
    let mean_intra_file_hunk_entropy_bits = if file_count == 0 {
        0.0
    } else {
        files
            .iter()
            .map(|file| file.intra_file_hunk_entropy_bits)
            .sum::<f32>()
            / file_count as f32
    };
    let max_intra_file_hunk_entropy_bits = files
        .iter()
        .map(|file| file.intra_file_hunk_entropy_bits)
        .fold(0.0_f32, f32::max);
    let metrics = DiffTopologyMetrics {
        analyzer: "contextgraph_unified_diff_topology_v1".to_string(),
        file_count,
        hunk_count,
        added_lines,
        removed_lines,
        context_lines,
        changed_lines,
        max_hunks_per_file,
        mean_hunks_per_file,
        max_hunk_changed_lines,
        mean_hunk_changed_lines,
        file_change_entropy_bits: shannon_entropy_bits(&file_weights),
        mean_intra_file_hunk_entropy_bits,
        max_intra_file_hunk_entropy_bits,
        files,
    };
    validate_metrics(&metrics)?;
    Ok(metrics)
}

fn validate_metrics(metrics: &DiffTopologyMetrics) -> InstrumentResult<()> {
    for value in [
        metrics.mean_hunks_per_file,
        metrics.mean_hunk_changed_lines,
        metrics.file_change_entropy_bits,
        metrics.mean_intra_file_hunk_entropy_bits,
        metrics.max_intra_file_hunk_entropy_bits,
    ] {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "diff_topology.metrics",
                format!("metric is non-finite: {value}"),
                "inspect TASK-RWD-203 metric aggregation",
            ));
        }
    }
    if metrics.file_count == 0 || metrics.hunk_count == 0 {
        return Err(InstrumentError::invalid(
            "input.diff",
            "diff topology requires at least one file and one hunk",
            "provide unified diff text with file headers and @@ hunks",
        ));
    }
    if metrics.changed_lines == 0 {
        return Err(InstrumentError::invalid(
            "input.diff",
            "diff topology requires at least one added or removed line",
            "provide a non-empty code change diff",
        ));
    }
    Ok(())
}

fn shannon_entropy_bits(weights: &[u32]) -> f32 {
    let total = weights.iter().copied().sum::<u32>();
    if total == 0 {
        return 0.0;
    }
    let total = total as f64;
    let entropy = weights
        .iter()
        .copied()
        .filter(|weight| *weight > 0)
        .map(|weight| {
            let p = weight as f64 / total;
            -p * p.log2()
        })
        .sum::<f64>();
    entropy as f32
}

fn ratio(numerator: u32, denominator: u32) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32
    }
}

fn effective_path(file: &FileTopology) -> &str {
    if file.new_path == "/dev/null" {
        &file.old_path
    } else {
        &file.new_path
    }
}

fn file_extension(path: &str) -> Option<&str> {
    path.rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty() && !extension.contains('/'))
}

fn invalid_line<T>(
    line_no: usize,
    message: &'static str,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(
        "input.diff",
        format!("line {line_no}: {message}"),
        remediation,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(diff: &str) -> DiffTopologyInput {
        DiffTopologyInput {
            diff: diff.to_string(),
        }
    }

    fn sample_diff() -> &'static str {
        r#"diff --git a/src/a.py b/src/a.py
index 1111111..2222222 100644
--- a/src/a.py
+++ b/src/a.py
@@ -1,4 +1,5 @@
 def f():
-    return 1
+    value = 1
+    return value
 
 def g():
@@ -10,3 +11,4 @@
 def h():
-    return 2
+    value = 2
+    return value
 # end
diff --git a/tests/test_a.py b/tests/test_a.py
index 3333333..4444444 100644
--- a/tests/test_a.py
+++ b/tests/test_a.py
@@ -1,3 +1,4 @@
 from src.a import f
+from src.a import g
 
 def test_f():
"#
    }

    #[test]
    fn parses_multifile_hunk_topology() {
        let metrics = analyze_diff_topology(&input(sample_diff())).unwrap();
        assert_eq!(metrics.file_count, 2);
        assert_eq!(metrics.hunk_count, 3);
        assert_eq!(metrics.added_lines, 5);
        assert_eq!(metrics.removed_lines, 2);
        assert_eq!(metrics.changed_lines, 7);
        assert_eq!(metrics.max_hunks_per_file, 2);
        assert_eq!(metrics.max_hunk_changed_lines, 3);
        assert!((metrics.mean_hunk_changed_lines - (7.0 / 3.0)).abs() < 0.0001);
        assert!(metrics.file_change_entropy_bits > 0.5);
        assert!((metrics.mean_intra_file_hunk_entropy_bits - 0.5).abs() < 0.0001);
    }

    #[test]
    fn instrument_encodes_diff_slot_vector() {
        let inst = EDiffTopologyInstrument;
        let encoded = inst.encode(&input(sample_diff())).unwrap();
        assert_eq!(encoded.len(), InstrumentSlot::EDiff.dim());
        assert!(encoded.iter().all(|value| value.is_finite()));
        assert!(encoded.iter().any(|value| *value > 0.0));
        assert!(encoded[0] > 0.0);
    }

    #[test]
    fn supports_add_file_zero_old_range() {
        let diff = r#"diff --git a/dev/null b/new.py
--- /dev/null
+++ b/new.py
@@ -0,0 +1,2 @@
+x = 1
+y = 2
"#;
        let metrics = analyze_diff_topology(&input(diff)).unwrap();
        assert_eq!(metrics.file_count, 1);
        assert_eq!(metrics.added_lines, 2);
        assert_eq!(metrics.removed_lines, 0);
        assert_eq!(metrics.files[0].old_path, "/dev/null");
    }

    #[test]
    fn invalid_diffs_fail_closed() {
        let inst = EDiffTopologyInstrument;
        for diff in [
            "",
            "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n",
            "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1,2 +1,2 @@\n x = 1\n",
            "@@ -1,1 +1,1 @@\n-x\n+y\n",
        ] {
            assert_eq!(
                inst.encode(&input(diff)).unwrap_err().code(),
                "MEJEPA_INSTRUMENTS_INVALID_INPUT"
            );
        }
    }
}

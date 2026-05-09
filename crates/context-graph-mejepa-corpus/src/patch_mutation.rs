use crate::{MutationCategory, MutationError, MutationResult, SplitMix64};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchMutationConfig {
    pub seed: u64,
    pub alternate_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchMutationOutcome {
    pub category: MutationCategory,
    pub mutated_patch: String,
    pub seed: u64,
    pub note: String,
}

#[derive(Debug, Clone)]
struct HunkHeader {
    old_start: i64,
    new_start: i64,
    suffix: String,
}

/// Mutate a unified diff while preserving patch structure.
///
/// This is the Phase 0 bridge between official SWE-bench `model_patch`
/// strings and the source-level mutation categories. It edits only added
/// lines inside hunks and updates hunk new-file lengths when lines are added
/// or removed. Categories that cannot find a valid added-line site fail
/// closed with `MEJEPA_CORPUS_NO_MUTATION_SITE`.
pub fn mutate_unified_diff(
    category: MutationCategory,
    patch: &str,
    config: PatchMutationConfig,
) -> MutationResult<PatchMutationOutcome> {
    validate_patch(patch)?;
    let mutated_patch = match category {
        MutationCategory::KnownGood => patch.to_string(),
        MutationCategory::SubtleFlip => mutate_added_line(patch, config.seed, subtle_flip_line)?,
        MutationCategory::OffByOne => mutate_added_line(patch, config.seed, off_by_one_line)?,
        MutationCategory::SwapVariable => {
            mutate_added_line(patch, config.seed, swap_variable_line)?
        }
        MutationCategory::DeleteTestCall => delete_added_assertion_line(patch, config.seed)?,
        MutationCategory::WrongFile => wrong_file_patch(patch, config.alternate_path.as_deref())?,
        MutationCategory::OverEngineer => append_over_engineered_assignment(patch, config.seed)?,
        MutationCategory::CompileError => {
            append_added_lines(patch, config.seed, &["", "def _mejepa_compile_error("])?
        }
    };
    Ok(PatchMutationOutcome {
        category,
        mutated_patch,
        seed: config.seed,
        note: format!("unified-diff mutation category={}", category.slug()),
    })
}

fn validate_patch(patch: &str) -> MutationResult<()> {
    if patch.trim().is_empty() {
        return Err(MutationError::invalid(
            "patch",
            "unified diff is empty",
            "load the official SWE-bench patch text before mutating",
        ));
    }
    if !patch.lines().any(|line| line.starts_with("diff --git ")) {
        return Err(MutationError::invalid(
            "patch",
            "unified diff does not contain a diff --git header",
            "pass a complete git-format unified diff, not a raw source file",
        ));
    }
    if !patch.lines().any(|line| line.starts_with("@@ ")) {
        return Err(MutationError::invalid(
            "patch",
            "unified diff does not contain any hunk headers",
            "pass a patch with at least one changed hunk",
        ));
    }
    Ok(())
}

fn mutate_added_line(
    patch: &str,
    seed: u64,
    mutator: fn(&str, u64) -> Option<String>,
) -> MutationResult<String> {
    let mut lines = split_lines(patch);
    let candidates = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let content = added_line_content(line)?;
            mutator(content, seed).map(|replacement| (idx, replacement))
        })
        .collect::<Vec<_>>();
    let idx = pick_candidate(&candidates, seed, "patch.added_lines")?.0;
    let replacement = candidates
        .iter()
        .find(|(candidate_idx, _)| *candidate_idx == idx)
        .expect("candidate picked from candidates")
        .1
        .clone();
    lines[idx] = format!("+{}", replacement);
    Ok(lines.concat())
}

fn delete_added_assertion_line(patch: &str, seed: u64) -> MutationResult<String> {
    let mut lines = split_lines(patch);
    let candidates = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let content = added_line_content(line)?;
            let trimmed = content.trim_start();
            if trimmed.starts_with("assert ")
                || trimmed.contains(".assert")
                || trimmed.contains("pytest.raises")
            {
                Some(idx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let idx = *pick_candidate(&candidates, seed, "patch.assertion_added_lines")?;
    lines.remove(idx);
    update_hunk_lengths(&mut lines)?;
    Ok(lines.concat())
}

fn append_added_lines(patch: &str, seed: u64, additions: &[&str]) -> MutationResult<String> {
    let mut lines = split_lines(patch);
    let candidates = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| added_line_content(line).map(|_| idx))
        .collect::<Vec<_>>();
    let idx = *pick_candidate(&candidates, seed, "patch.added_lines")?;
    let indent = added_line_content(&lines[idx])
        .map(leading_ws)
        .unwrap_or_default();
    let insert_at = idx + 1;
    let mut insertion = Vec::new();
    for addition in additions {
        if addition.is_empty() {
            insertion.push("+\n".to_string());
        } else if addition.starts_with("    ") {
            insertion.push(format!("+{}{}\n", indent, addition));
        } else {
            insertion.push(format!("+{}\n", addition));
        }
    }
    for (offset, line) in insertion.into_iter().enumerate() {
        lines.insert(insert_at + offset, line);
    }
    update_hunk_lengths(&mut lines)?;
    Ok(lines.concat())
}

fn append_over_engineered_assignment(patch: &str, seed: u64) -> MutationResult<String> {
    let mut lines = split_lines(patch);
    let candidates = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| added_line_content(line).map(|_| idx))
        .collect::<Vec<_>>();
    let idx = *pick_candidate(&candidates, seed, "patch.added_lines")?;
    let indent = added_line_content(&lines[idx])
        .map(leading_ws)
        .unwrap_or_default();
    let insert_at = idx + 1;
    let insertion = [
        format!("+{indent}\n"),
        format!("+{indent}_mejepa_unused_helper_value = 'mejepa-over-engineered'\n"),
    ];
    for (offset, line) in insertion.into_iter().enumerate() {
        lines.insert(insert_at + offset, line);
    }
    update_hunk_lengths(&mut lines)?;
    Ok(lines.concat())
}

fn wrong_file_patch(patch: &str, alternate_path: Option<&str>) -> MutationResult<String> {
    let alternate_path = alternate_path.ok_or_else(|| {
        MutationError::invalid(
            "alternate_path",
            "WrongFile patch mutation requires alternate_path",
            "supply a different repository-relative Python file path",
        )
    })?;
    if alternate_path.trim().is_empty() || alternate_path.chars().any(char::is_control) {
        return Err(MutationError::invalid(
            "alternate_path",
            "alternate_path is empty or contains a control character",
            "supply a stable single-line repository-relative path",
        ));
    }
    if alternate_path.starts_with('/')
        || alternate_path.starts_with('\\')
        || alternate_path.contains('\\')
        || alternate_path
            .split('/')
            .any(|part| part == ".." || part.is_empty())
    {
        return Err(MutationError::invalid(
            "alternate_path",
            "alternate_path must be a clean repository-relative path",
            "supply a path like package/module.py with no absolute prefix, backslashes, empty segments, or ..",
        ));
    }
    let mut lines = split_lines(patch);
    let mut changed = false;
    for line in &mut lines {
        if line.starts_with("diff --git ") {
            *line = format!("diff --git a/{alternate_path} b/{alternate_path}\n");
            changed = true;
        } else if line.starts_with("--- a/") || line.starts_with("--- /") {
            *line = format!("--- a/{alternate_path}\n");
        } else if line.starts_with("+++ b/") || line.starts_with("+++ /") {
            *line = format!("+++ b/{alternate_path}\n");
        }
    }
    if !changed {
        return Err(MutationError::no_site(
            "patch.diff_header",
            "no diff --git header found to rewrite",
            "pass a git-format unified diff",
        ));
    }
    Ok(lines.concat())
}

fn subtle_flip_line(line: &str, _seed: u64) -> Option<String> {
    for (from, to) in [
        ("==", "!="),
        ("!=", "=="),
        (">=", "<"),
        ("<=", ">"),
        (" and ", " or "),
        (" or ", " and "),
        ("True", "False"),
        ("False", "True"),
    ] {
        if let Some(pos) = line.find(from) {
            let mut out = String::new();
            out.push_str(&line[..pos]);
            out.push_str(to);
            out.push_str(&line[pos + from.len()..]);
            return Some(out);
        }
    }
    None
}

fn off_by_one_line(line: &str, seed: u64) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let end = i;
        let prev = start.checked_sub(1).and_then(|idx| bytes.get(idx)).copied();
        let next = bytes.get(end).copied();
        let bad_prev = prev
            .map(|b| b == b'.' || b.is_ascii_alphanumeric() || b == b'_')
            .unwrap_or(false);
        let bad_next = next
            .map(|b| b == b'.' || b.is_ascii_alphanumeric() || b == b'_')
            .unwrap_or(false);
        if bad_prev || bad_next {
            continue;
        }
        let value: i64 = line[start..end].parse().ok()?;
        let replacement = if seed & 1 == 0 { value + 1 } else { value - 1 };
        let mut out = String::new();
        out.push_str(&line[..start]);
        out.push_str(&replacement.to_string());
        out.push_str(&line[end..]);
        return Some(out);
    }
    None
}

fn swap_variable_line(line: &str, seed: u64) -> Option<String> {
    let mut spans = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let token = &line[start..i];
        if !is_python_keyword(token) && token != "_" {
            spans.push((start, i, token.to_string()));
        }
    }
    spans.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
    spans.dedup_by(|a, b| a.2 == b.2);
    if spans.len() < 2 {
        return None;
    }
    let mut rng = SplitMix64::new(seed);
    let from_idx = (rng.next_u64() as usize) % spans.len();
    let mut to_idx = (rng.next_u64() as usize) % spans.len();
    if to_idx == from_idx {
        to_idx = (to_idx + 1) % spans.len();
    }
    let (start, end, _) = spans[from_idx].clone();
    let replacement = spans[to_idx].2.clone();
    let mut out = String::new();
    out.push_str(&line[..start]);
    out.push_str(&replacement);
    out.push_str(&line[end..]);
    Some(out)
}

fn is_python_keyword(token: &str) -> bool {
    matches!(
        token,
        "and"
            | "as"
            | "assert"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "False"
            | "finally"
            | "for"
            | "from"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "None"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "True"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

fn split_lines(text: &str) -> Vec<String> {
    let mut lines = text
        .split_inclusive('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if !text.ends_with('\n') {
        if let Some(last) = lines.last_mut() {
            if !last.ends_with('\n') {
                last.push('\n');
            }
        }
    }
    lines
}

fn added_line_content(line: &str) -> Option<&str> {
    if line.starts_with('+') && !line.starts_with("+++") {
        Some(line[1..].trim_end_matches('\n').trim_end_matches('\r'))
    } else {
        None
    }
}

fn leading_ws(line: &str) -> String {
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn pick_candidate<'a, T>(
    candidates: &'a [T],
    seed: u64,
    field: &'static str,
) -> MutationResult<&'a T> {
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            field,
            "no eligible unified-diff added line found for this mutation category",
            "choose a task whose official patch contains the required source construct, or extract full patched source before mutating",
        ));
    }
    let idx = (SplitMix64::new(seed).next_u64() as usize) % candidates.len();
    Ok(&candidates[idx])
}

fn update_hunk_lengths(lines: &mut [String]) -> MutationResult<()> {
    let mut hunk_start: Option<usize> = None;
    let mut old_len = 0i64;
    let mut new_len = 0i64;
    for idx in 0..=lines.len() {
        if idx == lines.len() || lines[idx].starts_with("@@ ") {
            if let Some(header_idx) = hunk_start {
                let header = parse_hunk_header(&lines[header_idx])?;
                lines[header_idx] = format!(
                    "@@ -{},{} +{},{} @@{}\n",
                    header.old_start, old_len, header.new_start, new_len, header.suffix
                );
            }
            if idx < lines.len() {
                hunk_start = Some(idx);
                old_len = 0;
                new_len = 0;
            }
            continue;
        }
        if hunk_start.is_none() {
            continue;
        }
        let line = &lines[idx];
        if line.starts_with('+') && !line.starts_with("+++") {
            new_len += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            old_len += 1;
        } else if line.starts_with(' ') || line.trim().is_empty() {
            old_len += 1;
            new_len += 1;
        }
    }
    Ok(())
}

fn parse_hunk_header(line: &str) -> MutationResult<HunkHeader> {
    let trimmed = line.trim_end();
    if !trimmed.starts_with("@@ ") {
        return Err(MutationError::invalid(
            "patch.hunk_header",
            format!("invalid hunk header: {line:?}"),
            "preserve unified-diff hunk headers beginning with @@",
        ));
    }
    let Some(end) = trimmed[3..].find(" @@") else {
        return Err(MutationError::invalid(
            "patch.hunk_header",
            format!("hunk header missing closing @@: {line:?}"),
            "preserve unified-diff hunk headers with both @@ markers",
        ));
    };
    let range_part = &trimmed[3..3 + end];
    let suffix = trimmed[3 + end + 3..].to_string();
    let mut parts = range_part.split_whitespace();
    let old = parts.next().ok_or_else(|| {
        MutationError::invalid(
            "patch.hunk_header",
            "missing old range",
            "preserve old and new hunk ranges",
        )
    })?;
    let new = parts.next().ok_or_else(|| {
        MutationError::invalid(
            "patch.hunk_header",
            "missing new range",
            "preserve old and new hunk ranges",
        )
    })?;
    let (old_start, _old_len) = parse_range(old, '-')?;
    let (new_start, _new_len) = parse_range(new, '+')?;
    Ok(HunkHeader {
        old_start,
        new_start,
        suffix,
    })
}

fn parse_range(text: &str, prefix: char) -> MutationResult<(i64, i64)> {
    let stripped = text.strip_prefix(prefix).ok_or_else(|| {
        MutationError::invalid(
            "patch.hunk_header",
            format!("range {text:?} missing prefix {prefix:?}"),
            "preserve unified-diff range prefixes",
        )
    })?;
    let mut parts = stripped.split(',');
    let start = parts
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| {
            MutationError::invalid(
                "patch.hunk_header",
                format!("range {text:?} has invalid start"),
                "preserve integer hunk range starts",
            )
        })?;
    let len = parts
        .next()
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|err| {
            MutationError::invalid(
                "patch.hunk_header",
                format!("range {text:?} has invalid length: {err}"),
                "preserve integer hunk range lengths",
            )
        })?
        .unwrap_or(1);
    Ok((start, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/pkg/mod.py b/pkg/mod.py\n--- a/pkg/mod.py\n+++ b/pkg/mod.py\n@@ -1,2 +1,4 @@\n def f(left, right):\n+    assert left == right\n+    total = 3\n     return left\n";

    #[test]
    fn compile_error_updates_hunk_count() {
        let out = mutate_unified_diff(
            MutationCategory::CompileError,
            PATCH,
            PatchMutationConfig::default(),
        )
        .unwrap();
        assert!(out.mutated_patch.contains("@@ -1,2 +1,6 @@"));
        assert!(out.mutated_patch.contains("+def _mejepa_compile_error("));
    }

    #[test]
    fn delete_test_call_removes_assertion_and_updates_count() {
        let out = mutate_unified_diff(
            MutationCategory::DeleteTestCall,
            PATCH,
            PatchMutationConfig::default(),
        )
        .unwrap();
        assert!(!out.mutated_patch.contains("assert left == right"));
        assert!(out.mutated_patch.contains("@@ -1,2 +1,3 @@"));
    }

    #[test]
    fn subtle_flip_mutates_added_lines_only() {
        let out = mutate_unified_diff(
            MutationCategory::SubtleFlip,
            PATCH,
            PatchMutationConfig::default(),
        )
        .unwrap();
        assert!(out.mutated_patch.contains("assert left != right"));
    }

    #[test]
    fn wrong_file_requires_alternate_path() {
        let err = mutate_unified_diff(
            MutationCategory::WrongFile,
            PATCH,
            PatchMutationConfig::default(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
    }

    #[test]
    fn over_engineer_preserves_added_line_indent() {
        let out = mutate_unified_diff(
            MutationCategory::OverEngineer,
            PATCH,
            PatchMutationConfig::default(),
        )
        .unwrap();
        assert!(out
            .mutated_patch
            .contains("+    _mejepa_unused_helper_value = 'mejepa-over-engineered'"));
        assert!(!out.mutated_patch.contains("+def _unused_mejepa_helper"));
        assert!(out.mutated_patch.contains("@@ -1,2 +1,6 @@"));
    }

    #[test]
    fn wrong_file_rejects_path_traversal() {
        let err = mutate_unified_diff(
            MutationCategory::WrongFile,
            PATCH,
            PatchMutationConfig {
                alternate_path: Some("../other.py".into()),
                ..PatchMutationConfig::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
    }
}

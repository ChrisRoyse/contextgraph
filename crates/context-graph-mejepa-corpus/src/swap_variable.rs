// MutationCategory::SwapVariable — rename one local variable to a different
// in-scope name. Doc 09 §3.1 row 4.
//
// Strategy:
//   1. Scan for top-level (and method-level) Python assignments matching
//      `^[ \t]*<identifier>\s*=\s*` (excluding `==`, `>=`, `<=`, `!=`,
//      `+=`, `-=`, `*=`, `/=`, etc.).
//   2. Collect distinct identifier names. Reject built-in keywords +
//      private dunders.
//   3. If fewer than 2 distinct names, fail with NoMutationSite.
//   4. Pick a target name and a different replacement name (both
//      seed-driven).
//   5. Find the FIRST word-boundaried occurrence of `target` AFTER its
//      first definition line (so we don't rename the definition itself,
//      just one downstream USE) and replace it with `replacement`.
//
// Comments and Python string literals are skipped by the shared lexical mask.
//
// This intentionally produces ONE rename, not a global rename — global
// renames typically still type-check, while a single-site rename creates
// the shape of bug we want the predictor to learn about.

use crate::prng::{pick_index_or_fail, SplitMix64};
use crate::python_lex::{python_code_mask, range_is_code};
use crate::{MutationError, MutationResult, MutationSite};

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield", "self", "cls",
];

#[derive(Debug, Clone)]
struct VarDef {
    name: String,
    /// Byte offset of the END of the definition line (so usage searches
    /// start after this point).
    def_line_end_offset: usize,
}

#[derive(Debug, Clone)]
struct SwapCandidate {
    target_idx: usize,
    usage_offset: usize,
    replacement_indices: Vec<usize>,
}

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let code_mask = python_code_mask(source)?;
    let defs = collect_var_defs(source, &code_mask);
    if defs.len() < 2 {
        return Err(MutationError::no_site(
            "var_defs",
            format!(
                "swap requires ≥2 distinct local variable definitions; found {}",
                defs.len()
            ),
            "supply source with at least two `name = ...` assignments",
        ));
    }
    let candidates = collect_swap_candidates(source, &code_mask, &defs);
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "var_usage",
            "no variable usage had a different previously-defined replacement",
            "supply source where at least two local variables are defined before a downstream use",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let candidate_idx = pick_index_or_fail(&mut rng, candidates.len())?;
    let candidate = &candidates[candidate_idx];
    let replacement_slot = pick_index_or_fail(&mut rng, candidate.replacement_indices.len())?;
    let target = &defs[candidate.target_idx];
    let replacement = &defs[candidate.replacement_indices[replacement_slot]];

    Ok(MutationSite {
        byte_offset: candidate.usage_offset,
        byte_length: target.name.len(),
        original_text: target.name.clone(),
        replacement_text: replacement.name.clone(),
        note: format!(
            "swap_variable: usage of `{}` at byte {} renamed to previously-defined `{}`",
            target.name, candidate.usage_offset, replacement.name
        ),
    })
}

fn collect_var_defs(source: &str, code_mask: &[bool]) -> Vec<VarDef> {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut defs: Vec<VarDef> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut line_start = 0;

    while line_start < n {
        let mut line_end = line_start;
        while line_end < n && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let line = &source[line_start..line_end];
        if !line_starts_in_code(source, code_mask, line_start, line_end) {
            line_start = line_end + 1;
            continue;
        }
        if let Some((names, eq_pos)) = parse_assignment_line(line) {
            if !range_is_code(code_mask, line_start, eq_pos + 1) {
                line_start = line_end + 1;
                continue;
            }
            for name in names {
                push_var_def(&mut defs, &mut seen, name, line_end);
            }
        }
        if let Some(names) = parse_function_parameters(line) {
            for name in names {
                push_var_def(&mut defs, &mut seen, name, line_end);
            }
        }
        if let Some(name) = parse_for_target(line) {
            push_var_def(&mut defs, &mut seen, name, line_end);
        }
        line_start = line_end + 1;
    }
    defs
}

fn push_var_def(
    defs: &mut Vec<VarDef>,
    seen: &mut std::collections::HashSet<String>,
    name: &str,
    def_line_end_offset: usize,
) {
    if !PYTHON_KEYWORDS.contains(&name) && !name.starts_with("__") && !seen.contains(name) {
        seen.insert(name.to_string());
        defs.push(VarDef {
            name: name.to_string(),
            def_line_end_offset,
        });
    }
}

fn collect_swap_candidates(
    source: &str,
    code_mask: &[bool],
    defs: &[VarDef],
) -> Vec<SwapCandidate> {
    defs.iter()
        .enumerate()
        .filter_map(|(target_idx, target)| {
            let usage_offset = find_first_usage_after(
                source,
                code_mask,
                &target.name,
                target.def_line_end_offset,
            )?;
            let replacement_indices = defs
                .iter()
                .enumerate()
                .filter(|(idx, replacement)| {
                    *idx != target_idx && replacement.def_line_end_offset <= usage_offset
                })
                .map(|(idx, _)| idx)
                .collect::<Vec<_>>();
            if replacement_indices.is_empty() {
                None
            } else {
                Some(SwapCandidate {
                    target_idx,
                    usage_offset,
                    replacement_indices,
                })
            }
        })
        .collect()
}

/// Parse `<indent><name>[, <name>...] = ...` assignment headers. Returns
/// Some((names, eq_pos)) where every name is a borrow into `line` and `eq_pos`
/// is the byte index of the `=`.
/// Rejects compound ops (`==`, `+=`, `-=`, `*=`, `/=`, `//=`, `%=`, `**=`,
/// `>=`, `<=`, `!=`, `:=`, `&=`, `|=`, `^=`, `>>=`, `<<=`, `@=`).
fn parse_assignment_line(line: &str) -> Option<(Vec<&str>, usize)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let lhs_start = i;
    while i < bytes.len() && bytes[i] != b'=' {
        i += 1;
    }
    if i >= bytes.len() || i == lhs_start {
        return None;
    }
    let eq_pos = i;
    let prev = previous_non_ws(bytes, eq_pos)?;
    if matches!(
        bytes[prev],
        b'=' | b'!'
            | b'<'
            | b'>'
            | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b':'
            | b'&'
            | b'|'
            | b'^'
            | b'@'
    ) || (eq_pos + 1 < bytes.len() && bytes[eq_pos + 1] == b'=')
    {
        return None;
    }
    let lhs = &line[lhs_start..eq_pos];
    let mut names = Vec::new();
    for raw in lhs.split(',') {
        let name = raw.trim().split(':').next().unwrap_or("").trim();
        if !name.is_empty() && name.as_bytes().iter().copied().all(is_ident_continue) {
            let first = name.as_bytes()[0];
            if is_ident_start(first) {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        None
    } else {
        Some((names, eq_pos))
    }
}

fn parse_function_parameters(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("def ") {
        return None;
    }
    let open = line.find('(')?;
    let close = line[open + 1..].find(')')? + open + 1;
    let params = &line[open + 1..close];
    let mut names = Vec::new();
    for raw in params.split(',') {
        let name = raw
            .trim()
            .trim_start_matches('*')
            .split('=')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("")
            .trim();
        if !name.is_empty()
            && is_ident_start(name.as_bytes()[0])
            && name.as_bytes().iter().copied().all(is_ident_continue)
        {
            names.push(name);
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

fn parse_for_target(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("for ") {
        return None;
    }
    let after_for = &trimmed[4..];
    let in_pos = after_for.find(" in ")?;
    let target = after_for[..in_pos].trim();
    if target
        .as_bytes()
        .first()
        .copied()
        .is_some_and(is_ident_start)
        && target.as_bytes().iter().copied().all(is_ident_continue)
    {
        Some(target)
    } else {
        None
    }
}

fn previous_non_ws(bytes: &[u8], offset: usize) -> Option<usize> {
    let mut i = offset;
    while i > 0 {
        i -= 1;
        if !bytes[i].is_ascii_whitespace() {
            return Some(i);
        }
    }
    None
}

fn find_first_usage_after(
    source: &str,
    code_mask: &[bool],
    name: &str,
    after_offset: usize,
) -> Option<usize> {
    let bytes = source.as_bytes();
    let name_bytes = name.as_bytes();
    let n = bytes.len();
    let nl = name_bytes.len();
    let mut i = after_offset;
    while i + nl <= n {
        if !range_is_code(code_mask, i, nl) {
            i += 1;
            continue;
        }
        if &bytes[i..i + nl] == name_bytes {
            let prev_ok = i == 0 || !is_ident_continue(bytes[i - 1]);
            let next_ok = i + nl == n || !is_ident_continue(bytes[i + nl]);
            if prev_ok && next_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn line_starts_in_code(
    source: &str,
    code_mask: &[bool],
    line_start: usize,
    line_end: usize,
) -> bool {
    let bytes = source.as_bytes();
    let mut i = line_start;
    while i < line_end && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    i < line_end && code_mask.get(i).copied().unwrap_or(false)
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

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
// Comments + same-line string literals are skipped (state machine identical
// to subtle_flip / off_by_one).
//
// This intentionally produces ONE rename, not a global rename — global
// renames typically still type-check, while a single-site rename creates
// the shape of bug we want the predictor to learn about.

use crate::prng::SplitMix64;
use crate::{MutationError, MutationResult, MutationSite};

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
    "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
    "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try",
    "while", "with", "yield", "self", "cls",
];

#[derive(Debug, Clone)]
struct VarDef {
    name: String,
    /// Byte offset of the END of the definition line (so usage searches
    /// start after this point).
    def_line_end_offset: usize,
}

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let defs = collect_var_defs(source);
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
    let mut rng = SplitMix64::new(seed);
    let target_idx = rng
        .pick_index(defs.len())
        .ok_or_else(|| MutationError::op_failed(
            "rng",
            "pick_index returned None for non-empty defs",
            "internal error",
        ))?;
    let target = &defs[target_idx];
    let mut replacement_idx = rng
        .pick_index(defs.len())
        .ok_or_else(|| MutationError::op_failed(
            "rng",
            "pick_index returned None for non-empty defs",
            "internal error",
        ))?;
    if replacement_idx == target_idx {
        replacement_idx = (replacement_idx + 1) % defs.len();
    }
    let replacement = &defs[replacement_idx];

    let usage_offset = find_first_usage_after(source, &target.name, target.def_line_end_offset)
        .ok_or_else(|| MutationError::no_site(
            "var_usage",
            format!(
                "no word-boundaried usage of `{}` found after its definition (offset {})",
                target.name, target.def_line_end_offset
            ),
            "supply source where the variable is referenced after its definition",
        ))?;

    Ok(MutationSite {
        byte_offset: usage_offset,
        byte_length: target.name.len(),
        original_text: target.name.clone(),
        replacement_text: replacement.name.clone(),
        note: format!(
            "swap_variable: usage of `{}` at byte {usage_offset} renamed to `{}`",
            target.name, replacement.name
        ),
    })
}

fn collect_var_defs(source: &str) -> Vec<VarDef> {
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
        if let Some((name, _eq_pos)) = parse_assignment_line(line) {
            if !PYTHON_KEYWORDS.contains(&name)
                && !name.starts_with("__")
                && !seen.contains(name)
            {
                seen.insert(name.to_string());
                defs.push(VarDef {
                    name: name.to_string(),
                    def_line_end_offset: line_end,
                });
            }
        }
        line_start = line_end + 1;
    }
    defs
}

/// Parse `<indent><name> = ...` assignment headers. Returns Some((name, eq_pos))
/// where `name` is a borrow into `line` and `eq_pos` is the byte index of the `=`.
/// Rejects compound ops (`==`, `+=`, `-=`, `*=`, `/=`, `//=`, `%=`, `**=`,
/// `>=`, `<=`, `!=`, `:=`, `&=`, `|=`, `^=`, `>>=`, `<<=`, `@=`).
fn parse_assignment_line(line: &str) -> Option<(&str, usize)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let name_start = i;
    if i >= bytes.len() || !is_ident_start(bytes[i]) {
        return None;
    }
    i += 1;
    while i < bytes.len() && is_ident_continue(bytes[i]) {
        i += 1;
    }
    let name_end = i;
    if name_start == name_end {
        return None;
    }
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'=' {
        return None;
    }
    // Reject compound operators by checking the byte BEFORE the `=` and
    // AFTER the `=`. The byte before should be space/tab (we already
    // skipped those — so it must be `=` directly after whitespace, not
    // after `+`, `-`, etc., because those would have failed `is_ident_*`).
    // The byte after must NOT be `=` (which would mean `==`).
    if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
        return None;
    }
    let name = &line[name_start..name_end];
    Some((name, i))
}

fn find_first_usage_after(source: &str, name: &str, after_offset: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let name_bytes = name.as_bytes();
    let n = bytes.len();
    let nl = name_bytes.len();
    let mut i = after_offset;
    let mut in_string: Option<u8> = None;
    let mut in_comment = false;
    while i + nl <= n {
        let c = bytes[i];
        if in_comment {
            if c == b'\n' {
                in_comment = false;
            }
            i += 1;
            continue;
        }
        if let Some(quote) = in_string {
            if c == b'\\' && i + 1 < n {
                i += 2;
                continue;
            }
            if c == quote {
                in_string = None;
            }
            i += 1;
            continue;
        }
        if c == b'#' {
            in_comment = true;
            i += 1;
            continue;
        }
        if c == b'"' || c == b'\'' {
            in_string = Some(c);
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

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}

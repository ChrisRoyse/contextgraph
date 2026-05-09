// MutationCategory::DeleteTestCall — delete one assertion call. Doc 09 §3.1
// row 5.
//
// We scan for lines whose first non-whitespace tokens match one of:
//   `assert ` / `assert(` ...
//   `self.assertEqual(` / `self.assertTrue(` / `self.assertFalse(` /
//     `self.assertRaises(` / any `self.assertX(`
//   `pytest.raises(` / `pytest.assume(`
//
// The whole line is removed (including trailing `\n`). Multi-line
// assertions (i.e. lines ending with `\` or with unbalanced parens that
// continue across multiple lines) are NOT supported in this turn — the
// operator skips those candidates and continues searching. If no
// single-line assertion is found, the operator returns
// `MEJEPA_CORPUS_NO_MUTATION_SITE`.
//
// `byte_offset` of the `MutationSite` is the start of the line; `byte_length`
// covers the line + its terminating `\n` (if present). `replacement_text`
// is empty.

use crate::prng::SplitMix64;
use crate::{MutationError, MutationResult, MutationSite};

#[derive(Debug, Clone)]
struct Candidate {
    line_start: usize,
    line_length: usize,
    line_text: String,
    matched_prefix: &'static str,
}

const ASSERTION_PREFIXES: &[&str] = &[
    "assert ",
    "assert(",
    "self.assert",
    "pytest.raises",
    "pytest.assume",
];

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let candidates = find_candidates(source);
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "assertion_line",
            "no single-line assertion found in source",
            "ensure source contains at least one `assert ...`, `self.assertX(...)`, or `pytest.raises(...)` line",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let pick = rng.pick_index(candidates.len()).ok_or_else(|| {
        MutationError::op_failed(
            "rng",
            "pick_index returned None for non-empty candidates",
            "internal error",
        )
    })?;
    let chosen = &candidates[pick];
    Ok(MutationSite {
        byte_offset: chosen.line_start,
        byte_length: chosen.line_length,
        original_text: chosen.line_text.clone(),
        replacement_text: String::new(),
        note: format!(
            "delete_test_call: removed line beginning with `{}`",
            chosen.matched_prefix
        ),
    })
}

fn find_candidates(source: &str) -> Vec<Candidate> {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut candidates = Vec::new();
    let mut line_start = 0;
    while line_start < n {
        let mut line_end = line_start;
        while line_end < n && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let line_with_nl_end = if line_end < n { line_end + 1 } else { line_end };
        let line = &source[line_start..line_end];
        if let Some(prefix) = matched_prefix(line) {
            if !is_multiline_continuation(line, &source[line_with_nl_end..n.min(line_with_nl_end + 200)]) {
                candidates.push(Candidate {
                    line_start,
                    line_length: line_with_nl_end - line_start,
                    line_text: source[line_start..line_with_nl_end].to_string(),
                    matched_prefix: prefix,
                });
            }
        }
        line_start = line_with_nl_end;
        if line_start == line_end {
            break;
        }
    }
    candidates
}

fn matched_prefix(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    for prefix in ASSERTION_PREFIXES {
        if trimmed.starts_with(*prefix) {
            // For "self.assert" we must additionally check that what
            // follows is an identifier-continuation char + `(` somewhere
            // before the line ends — keeps us from matching `self.assertion`
            // as a property accessor.
            if *prefix == "self.assert" {
                let rest = &trimmed["self.assert".len()..];
                if rest.bytes().take_while(|b| b.is_ascii_alphanumeric()).count() == 0 {
                    continue;
                }
                if !rest.contains('(') {
                    continue;
                }
            }
            return Some(prefix);
        }
    }
    None
}

/// Returns true if the line ends with backslash continuation OR has an
/// unbalanced open paren that the next chunk doesn't close on the same line.
/// Conservative: when in doubt, mark as multi-line so we skip it.
fn is_multiline_continuation(line: &str, _next_chunk: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.ends_with('\\') {
        return true;
    }
    let mut paren = 0i32;
    let mut in_string: Option<u8> = None;
    let mut chars = line.bytes().peekable();
    while let Some(c) = chars.next() {
        if let Some(quote) = in_string {
            if c == b'\\' {
                let _ = chars.next();
                continue;
            }
            if c == quote {
                in_string = None;
            }
            continue;
        }
        match c {
            b'#' => break,
            b'\'' | b'"' => in_string = Some(c),
            b'(' | b'[' | b'{' => paren += 1,
            b')' | b']' | b'}' => paren -= 1,
            _ => {}
        }
    }
    paren > 0
}

// MutationCategory::DeleteTestCall — delete one assertion call. Doc 09 §3.1
// row 5.
//
// We scan for lines whose first non-whitespace tokens match one of:
//   `assert ` / `assert(` ...
//   `self.assertEqual(` / `self.assertTrue(` / `self.assertFalse(` /
//     `self.assertRaises(` / any `self.assertX(`
//   `pytest.raises(` / `with pytest.raises(` / `pytest.assume(`
//
// The assertion call is replaced with a same-indentation `pass` statement
// so the mutation deletes the assertion semantics without manufacturing a
// syntax error. `with pytest.raises(...)` removes the whole indented context
// block and replaces it with one `pass`. Multi-line assertion expressions
// are skipped. If no assertion is found, the operator returns
// `MEJEPA_CORPUS_NO_MUTATION_SITE`.
//
// `byte_offset` of the `MutationSite` is the start of the line; `byte_length`
// covers the removed line or context block. `replacement_text` is the `pass`
// line that preserves Python syntax.

use crate::prng::{pick_index_or_fail, SplitMix64};
use crate::python_lex::python_code_mask;
use crate::{MutationError, MutationResult, MutationSite};

#[derive(Debug, Clone)]
struct Candidate {
    line_start: usize,
    line_length: usize,
    line_text: String,
    replacement_text: String,
    matched_prefix: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct PrefixMatch {
    prefix: &'static str,
    removes_context_block: bool,
}

const ASSERTION_PREFIXES: &[PrefixMatch] = &[
    PrefixMatch {
        prefix: "assert ",
        removes_context_block: false,
    },
    PrefixMatch {
        prefix: "assert(",
        removes_context_block: false,
    },
    PrefixMatch {
        prefix: "self.assert",
        removes_context_block: false,
    },
    PrefixMatch {
        prefix: "pytest.raises",
        removes_context_block: false,
    },
    PrefixMatch {
        prefix: "with pytest.raises",
        removes_context_block: true,
    },
    PrefixMatch {
        prefix: "pytest.assume",
        removes_context_block: false,
    },
];

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let candidates = find_candidates(source)?;
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "assertion_line",
            "no single-line assertion found in source",
            "ensure source contains at least one `assert ...`, `self.assertX(...)`, or `pytest.raises(...)` line",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let pick = pick_index_or_fail(&mut rng, candidates.len())?;
    let chosen = &candidates[pick];
    Ok(MutationSite {
        byte_offset: chosen.line_start,
        byte_length: chosen.line_length,
        original_text: chosen.line_text.clone(),
        replacement_text: chosen.replacement_text.clone(),
        note: format!(
            "delete_test_call: replaced assertion beginning with `{}` by pass",
            chosen.matched_prefix
        ),
    })
}

fn find_candidates(source: &str) -> MutationResult<Vec<Candidate>> {
    let bytes = source.as_bytes();
    let code_mask = python_code_mask(source)?;
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
        if !line_starts_in_code(source, &code_mask, line_start, line_end) {
            line_start = line_with_nl_end;
            if line_start == line_end {
                break;
            }
            continue;
        }
        if let Some(prefix) = matched_prefix(line) {
            if !is_multiline_continuation(line) {
                let line_length = if prefix.removes_context_block {
                    context_block_length(source, line_start, line_end, line_with_nl_end)
                } else {
                    line_with_nl_end - line_start
                };
                candidates.push(Candidate {
                    line_start,
                    line_length,
                    line_text: source[line_start..line_start + line_length].to_string(),
                    replacement_text: pass_line_for(line, line_end < n),
                    matched_prefix: prefix.prefix,
                });
            }
        }
        line_start = line_with_nl_end;
        if line_start == line_end {
            break;
        }
    }
    Ok(candidates)
}

fn matched_prefix(line: &str) -> Option<PrefixMatch> {
    let trimmed = line.trim_start();
    for prefix in ASSERTION_PREFIXES {
        if trimmed.starts_with(prefix.prefix) {
            // For "self.assert" we must additionally check that what
            // follows is an identifier-continuation char + `(` somewhere
            // before the line ends — keeps us from matching `self.assertion`
            // as a property accessor.
            if prefix.prefix == "self.assert" {
                let rest = &trimmed["self.assert".len()..];
                if rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric())
                    .count()
                    == 0
                {
                    continue;
                }
                if !rest.contains('(') {
                    continue;
                }
            }
            return Some(*prefix);
        }
    }
    None
}

fn context_block_length(
    source: &str,
    line_start: usize,
    line_end: usize,
    line_with_nl_end: usize,
) -> usize {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let header_indent = indent_width(&source[line_start..line_end]);
    let mut block_end = line_with_nl_end;
    while block_end < n {
        let next_start = block_end;
        let mut next_end = next_start;
        while next_end < n && bytes[next_end] != b'\n' {
            next_end += 1;
        }
        let next_with_nl_end = if next_end < n { next_end + 1 } else { next_end };
        let next_line = &source[next_start..next_end];
        if next_line.trim().is_empty() || indent_width(next_line) > header_indent {
            block_end = next_with_nl_end;
        } else {
            break;
        }
    }
    block_end - line_start
}

fn pass_line_for(line: &str, had_newline: bool) -> String {
    let indent_len = indent_width(line);
    let indent = &line[..indent_len];
    if had_newline {
        format!("{indent}pass\n")
    } else {
        format!("{indent}pass")
    }
}

fn indent_width(line: &str) -> usize {
    line.bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count()
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

/// Returns true if the line ends with backslash continuation OR has an
/// unbalanced open paren on its own. Conservative: when in doubt, mark as
/// multi-line so we skip it.
fn is_multiline_continuation(line: &str) -> bool {
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

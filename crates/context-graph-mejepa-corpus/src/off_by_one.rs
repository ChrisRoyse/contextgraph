// MutationCategory::OffByOne — add ±1 to one numeric literal. Doc 09 §3.1
// row 3.
//
// Candidate scan: contiguous decimal-digit runs that aren't part of a
// larger identifier (must be preceded by a non-word byte). Floats (`.5`,
// `3.14`) are rejected as candidates — we only mutate ints, since ±1.0
// rarely flips program semantics. Numbers inside Python string literals or
// comments are skipped by the shared lexical mask.
//
// Direction (+1 vs -1) is chosen by the LSB of the seed: even seed → +1,
// odd seed → -1. This makes the operator deterministic per (input, seed)
// AND lets the caller flip the direction by toggling the seed's LSB.

use crate::prng::{pick_index_or_fail, SplitMix64};
use crate::python_lex::python_code_mask;
use crate::{MutationError, MutationResult, MutationSite};

#[derive(Debug, Clone)]
struct Candidate {
    byte_offset: usize,
    byte_length: usize,
    original_text: String,
    replacement_text: String,
    note: String,
}

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let direction: i64 = if seed & 1 == 0 { 1 } else { -1 };
    let candidates = find_candidates(source, direction)?;
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "off_by_one_site",
            "no integer literal or range/slice boundary expression found in source",
            "ensure source contains an integer literal, range(...) bound, or slice boundary expression",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let pick = pick_index_or_fail(&mut rng, candidates.len())?;
    let chosen = &candidates[pick];
    Ok(MutationSite {
        byte_offset: chosen.byte_offset,
        byte_length: chosen.byte_length,
        original_text: chosen.original_text.clone(),
        replacement_text: chosen.replacement_text.clone(),
        note: chosen.note.clone(),
    })
}

fn find_candidates(source: &str, direction: i64) -> MutationResult<Vec<Candidate>> {
    let bytes = source.as_bytes();
    let code_mask = python_code_mask(source)?;
    let n = bytes.len();
    let mut candidates = Vec::new();
    let mut i = 0;
    let dir_label = direction_label(direction);

    while i < n {
        if !code_mask[i] {
            i += 1;
            continue;
        }
        let c = bytes[i];

        if c.is_ascii_digit() {
            // Reject if preceded by an identifier byte (e.g. `var2`, `_3`)
            // OR by a `.` (fractional part of a float like `3.14`).
            let prev_byte = if i == 0 { None } else { Some(bytes[i - 1]) };
            let prev_ok = match prev_byte {
                None => true,
                Some(b) => !is_word_byte(b) && b != b'.',
            };
            // Find run end.
            let mut j = i;
            while j < n && code_mask[j] && bytes[j].is_ascii_digit() {
                j += 1;
            }
            // Reject when the next byte is `.` (float fraction starts here)
            // OR an identifier-continuation byte (covers `e`/`E`/`j`/`_` and
            // any other identifier suffix that would mean the digits are
            // part of a larger token).
            let next_byte = bytes.get(j).copied();
            let next_is_word_or_dot = next_byte
                .map(|b| is_word_byte(b) || b == b'.')
                .unwrap_or(false);
            if prev_ok && !next_is_word_or_dot {
                let digits = source[i..j].to_string();
                let value: i64 = digits.parse().map_err(|e| {
                    MutationError::op_failed(
                        "candidate.parse",
                        format!("could not parse `{digits}` as i64: {e}"),
                        "internal error; expected only digits",
                    )
                })?;
                let new_value = value.checked_add(direction).ok_or_else(|| {
                    MutationError::op_failed(
                        "candidate.checked_add",
                        format!("integer overflow adding {direction} to {value}"),
                        "the chosen literal is at i64 boundary; pick a different seed",
                    )
                })?;
                candidates.push(Candidate {
                    byte_offset: i,
                    byte_length: j - i,
                    original_text: digits.clone(),
                    replacement_text: new_value.to_string(),
                    note: format!("off_by_one: numeric literal {digits} mutated by {dir_label}"),
                });
            }
            i = j;
            continue;
        }
        i += 1;
    }
    add_range_boundary_candidates(source, &code_mask, direction, &mut candidates);
    add_slice_boundary_candidates(source, &code_mask, direction, &mut candidates);
    Ok(candidates)
}

fn add_range_boundary_candidates(
    source: &str,
    code_mask: &[bool],
    direction: i64,
    candidates: &mut Vec<Candidate>,
) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + b"range".len() <= bytes.len() {
        if !range_is_code_local(code_mask, i, b"range".len())
            || &bytes[i..i + b"range".len()] != b"range"
            || !keyword_boundary(bytes, i, b"range".len())
        {
            i += 1;
            continue;
        }
        let Some(open) = next_non_ws(bytes, code_mask, i + b"range".len()) else {
            break;
        };
        if bytes[open] != b'(' {
            i += 1;
            continue;
        }
        let Some(close) = matching_delim(bytes, code_mask, open, b'(', b')') else {
            i += 1;
            continue;
        };
        for (arg_start, arg_end) in top_level_spans(bytes, code_mask, open + 1, close, b',') {
            if let Some((start, end)) = trim_span(source, arg_start, arg_end) {
                let original = source[start..end].to_string();
                if original.starts_with('*') {
                    continue;
                }
                candidates.push(Candidate {
                    byte_offset: start,
                    byte_length: end - start,
                    replacement_text: expression_replacement(&original, direction),
                    original_text: original,
                    note: format!(
                        "off_by_one: range(...) boundary expression mutated by {}",
                        direction_label(direction)
                    ),
                });
            }
        }
        i = close + 1;
    }
}

fn add_slice_boundary_candidates(
    source: &str,
    code_mask: &[bool],
    direction: i64,
    candidates: &mut Vec<Candidate>,
) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !code_mask[i] || bytes[i] != b'[' {
            i += 1;
            continue;
        }
        let Some(close) = matching_delim(bytes, code_mask, i, b'[', b']') else {
            i += 1;
            continue;
        };
        let spans = top_level_spans(bytes, code_mask, i + 1, close, b':');
        if spans.len() < 2 {
            i = close + 1;
            continue;
        }
        for (bound_start, bound_end) in spans.into_iter().take(2) {
            if let Some((start, end)) = trim_span(source, bound_start, bound_end) {
                let original = source[start..end].to_string();
                candidates.push(Candidate {
                    byte_offset: start,
                    byte_length: end - start,
                    replacement_text: expression_replacement(&original, direction),
                    original_text: original,
                    note: format!(
                        "off_by_one: slice boundary expression mutated by {}",
                        direction_label(direction)
                    ),
                });
            }
        }
        i = close + 1;
    }
}

fn expression_replacement(original: &str, direction: i64) -> String {
    let op = if direction > 0 { "+" } else { "-" };
    format!("({original}) {op} 1")
}

fn direction_label(direction: i64) -> &'static str {
    if direction > 0 {
        "+1"
    } else {
        "-1"
    }
}

fn matching_delim(
    bytes: &[u8],
    code_mask: &[bool],
    open_idx: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open_idx;
    while i < bytes.len() {
        if !code_mask[i] {
            i += 1;
            continue;
        }
        if bytes[i] == open {
            depth += 1;
        } else if bytes[i] == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn top_level_spans(
    bytes: &[u8],
    code_mask: &[bool],
    start: usize,
    end: usize,
    separator: u8,
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut span_start = start;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut i = start;
    while i < end {
        if !code_mask[i] {
            i += 1;
            continue;
        }
        match bytes[i] {
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'[' => bracket += 1,
            b']' => bracket = bracket.saturating_sub(1),
            b'{' => brace += 1,
            b'}' => brace = brace.saturating_sub(1),
            b if b == separator && paren == 0 && bracket == 0 && brace == 0 => {
                spans.push((span_start, i));
                span_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    spans.push((span_start, end));
    spans
}

fn trim_span(source: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    if start >= end || end > source.len() {
        return None;
    }
    let bytes = source.as_bytes();
    let mut left = start;
    let mut right = end;
    while left < right && bytes[left].is_ascii_whitespace() {
        left += 1;
    }
    while right > left && bytes[right - 1].is_ascii_whitespace() {
        right -= 1;
    }
    if left == right {
        None
    } else {
        Some((left, right))
    }
}

fn next_non_ws(bytes: &[u8], code_mask: &[bool], start: usize) -> Option<usize> {
    let mut i = start;
    while i < bytes.len() {
        if code_mask[i] && !bytes[i].is_ascii_whitespace() {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn keyword_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let prev_ok = start == 0 || !is_word_byte(bytes[start - 1]);
    let next = start + len;
    let next_ok = next >= bytes.len() || !is_word_byte(bytes[next]);
    prev_ok && next_ok
}

fn range_is_code_local(mask: &[bool], start: usize, len: usize) -> bool {
    start
        .checked_add(len)
        .and_then(|end| mask.get(start..end))
        .map(|items| items.iter().all(|is_code| *is_code))
        .unwrap_or(false)
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

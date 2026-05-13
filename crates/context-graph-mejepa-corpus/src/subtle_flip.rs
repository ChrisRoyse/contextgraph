// MutationCategory::SubtleFlip — invert one boolean operator. Doc 09 §3.1
// row 2.
//
// Candidate operators:
//   `==` ↔ `!=`
//   `<=` ↔ `>=`
//   `<`  ↔ `>`     (single-char comparison; SKIP `<<`/`>>`/`<=`/`>=`)
//   ` and ` ↔ ` or `   (word-boundaried)
//   ` True` ↔ ` False` (word-boundaried, for top-level assignments)
//
// Comments and Python string literals (including common prefixes and triple
// quotes) are skipped; mutation sites must come from executable source text.

use crate::prng::{pick_index_or_fail, SplitMix64};
use crate::python_lex::{python_code_mask, range_is_code};
use crate::{MutationError, MutationResult, MutationSite};

#[derive(Debug, Clone)]
struct Candidate {
    byte_offset: usize,
    byte_length: usize,
    original: String,
    replacement: String,
    note: String,
}

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let candidates = find_candidates(source)?;
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "boolean_operator",
            "no boolean operator found in source",
            "ensure source contains at least one of `==`, `!=`, `<`, `>`, `<=`, `>=`, ` and `, ` or `, ` True`, ` False`",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let pick = pick_index_or_fail(&mut rng, candidates.len())?;
    let chosen = &candidates[pick];
    let original_text = source
        .get(chosen.byte_offset..chosen.byte_offset + chosen.byte_length)
        .ok_or_else(|| {
            MutationError::op_failed(
                "candidate.byte_offset",
                "candidate byte range fell outside source",
                "internal error; report",
            )
        })?
        .to_string();
    if original_text != chosen.original {
        return Err(MutationError::op_failed(
            "candidate.original",
            format!(
                "candidate original {:?} did not match source {:?}",
                chosen.original, original_text
            ),
            "internal error; report the subtle-flip operator candidate",
        ));
    }
    Ok(MutationSite {
        byte_offset: chosen.byte_offset,
        byte_length: chosen.byte_length,
        original_text,
        replacement_text: chosen.replacement.clone(),
        note: chosen.note.clone(),
    })
}

fn find_candidates(source: &str) -> MutationResult<Vec<Candidate>> {
    let bytes = source.as_bytes();
    let code_mask = python_code_mask(source)?;
    let n = bytes.len();
    let mut candidates = Vec::new();
    let mut i = 0;

    while i < n {
        if !code_mask[i] {
            i += 1;
            continue;
        }
        let c = bytes[i];

        // Two-char ops: `==`, `!=`, `<=`, `>=`
        if i + 1 < n && range_is_code(&code_mask, i, 2) {
            let pair = &bytes[i..i + 2];
            match pair {
                b"==" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: "==".to_string(),
                        replacement: "!=".to_string(),
                        note: "subtle_flip: equality flipped to inequality".to_string(),
                    });
                    i += 2;
                    continue;
                }
                b"!=" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: "!=".to_string(),
                        replacement: "==".to_string(),
                        note: "subtle_flip: inequality flipped to equality".to_string(),
                    });
                    i += 2;
                    continue;
                }
                b"<=" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: "<=".to_string(),
                        replacement: ">=".to_string(),
                        note: "subtle_flip: <= flipped to >=".to_string(),
                    });
                    i += 2;
                    continue;
                }
                b">=" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: ">=".to_string(),
                        replacement: "<=".to_string(),
                        note: "subtle_flip: >= flipped to <=".to_string(),
                    });
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Single-char `<` and `>`. Skip:
        //   - `<<` / `>>` (Python bitshift) — check both neighbors.
        //   - `->` (Python type-annotation arrow) — `>` preceded by `-`.
        //   - `<-` (not Python, defensive) — `<` preceded by anything; only
        //     guarded against `<<` here since `<-` does not occur.
        let prev_byte = if i == 0 { None } else { Some(bytes[i - 1]) };
        let next_byte = bytes.get(i + 1).copied();
        if c == b'<' && next_byte != Some(b'<') && prev_byte != Some(b'<') {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: 1,
                original: "<".to_string(),
                replacement: ">".to_string(),
                note: "subtle_flip: < flipped to >".to_string(),
            });
            i += 1;
            continue;
        }
        if c == b'>'
            && next_byte != Some(b'>')
            && prev_byte != Some(b'>')
            && prev_byte != Some(b'-')
        {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: 1,
                original: ">".to_string(),
                replacement: "<".to_string(),
                note: "subtle_flip: > flipped to <".to_string(),
            });
            i += 1;
            continue;
        }

        if let Some(len) = match_compound_keyword(bytes, i, b"not", b"in") {
            if is_for_loop_membership_slot(bytes, i) {
                i += len;
                continue;
            }
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: source[i..i + len].to_string(),
                replacement: "in".to_string(),
                note: "subtle_flip: not in -> in".to_string(),
            });
            i += len;
            continue;
        }
        if let Some(len) = match_compound_keyword(bytes, i, b"is", b"not") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: source[i..i + len].to_string(),
                replacement: "is".to_string(),
                note: "subtle_flip: is not -> is".to_string(),
            });
            i += len;
            continue;
        }

        // Word-boundary keywords. We use the surrounding bytes to verify the
        // match isn't inside a longer identifier. ASCII-only check is fine
        // because Python keywords are ASCII.
        if let Some(len) = match_keyword(bytes, i, b"in") {
            if !is_for_loop_membership_slot(bytes, i) && !previous_word_is(bytes, i, b"not") {
                candidates.push(Candidate {
                    byte_offset: i,
                    byte_length: len,
                    original: "in".to_string(),
                    replacement: "not in".to_string(),
                    note: "subtle_flip: in -> not in".to_string(),
                });
            }
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"is") {
            if !next_word_is(bytes, i + len, b"not") {
                candidates.push(Candidate {
                    byte_offset: i,
                    byte_length: len,
                    original: "is".to_string(),
                    replacement: "is not".to_string(),
                    note: "subtle_flip: is -> is not".to_string(),
                });
            }
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"any") {
            if next_non_ws_is(bytes, i + len, b'(') {
                candidates.push(Candidate {
                    byte_offset: i,
                    byte_length: len,
                    original: "any".to_string(),
                    replacement: "all".to_string(),
                    note: "subtle_flip: any(...) -> all(...)".to_string(),
                });
            }
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"all") {
            if next_non_ws_is(bytes, i + len, b'(') {
                candidates.push(Candidate {
                    byte_offset: i,
                    byte_length: len,
                    original: "all".to_string(),
                    replacement: "any".to_string(),
                    note: "subtle_flip: all(...) -> any(...)".to_string(),
                });
            }
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"and") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "and".to_string(),
                replacement: "or".to_string(),
                note: "subtle_flip: and -> or".to_string(),
            });
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"or") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "or".to_string(),
                replacement: "and".to_string(),
                note: "subtle_flip: or -> and".to_string(),
            });
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"True") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "True".to_string(),
                replacement: "False".to_string(),
                note: "subtle_flip: True -> False".to_string(),
            });
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"False") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "False".to_string(),
                replacement: "True".to_string(),
                note: "subtle_flip: False -> True".to_string(),
            });
            i += len;
            continue;
        }

        i += 1;
    }
    Ok(candidates)
}

/// Match a Python-style word-boundaried keyword at byte offset `i`. Returns
/// `Some(keyword.len())` on a match, `None` otherwise. Word boundary:
/// previous and following byte (if present) must be non-word
/// (NOT `[A-Za-z0-9_]`).
fn match_keyword(bytes: &[u8], i: usize, keyword: &[u8]) -> Option<usize> {
    let kw_len = keyword.len();
    if i + kw_len > bytes.len() {
        return None;
    }
    if &bytes[i..i + kw_len] != keyword {
        return None;
    }
    let prev_ok = i == 0 || !is_word_byte(bytes[i - 1]);
    let next_ok = i + kw_len == bytes.len() || !is_word_byte(bytes[i + kw_len]);
    if prev_ok && next_ok {
        Some(kw_len)
    } else {
        None
    }
}

fn match_compound_keyword(bytes: &[u8], i: usize, first: &[u8], second: &[u8]) -> Option<usize> {
    let first_len = match_keyword(bytes, i, first)?;
    let mut j = i + first_len;
    let whitespace_start = j;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j == whitespace_start {
        return None;
    }
    let second_len = match_keyword(bytes, j, second)?;
    Some(j + second_len - i)
}

fn previous_word_is(bytes: &[u8], offset: usize, expected: &[u8]) -> bool {
    if offset == 0 {
        return false;
    }
    let mut end = offset;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    start < end && &bytes[start..end] == expected
}

fn next_word_is(bytes: &[u8], offset: usize, expected: &[u8]) -> bool {
    let mut start = offset;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    let mut end = start;
    while end < bytes.len() && is_word_byte(bytes[end]) {
        end += 1;
    }
    start < end && &bytes[start..end] == expected
}

fn is_for_loop_membership_slot(bytes: &[u8], offset: usize) -> bool {
    let line_start = bytes[..offset]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let mut start = line_start;
    while start < offset && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    start + b"for ".len() <= offset && &bytes[start..start + b"for ".len()] == b"for "
}

fn next_non_ws_is(bytes: &[u8], offset: usize, expected: u8) -> bool {
    let mut i = offset;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    bytes.get(i).copied() == Some(expected)
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

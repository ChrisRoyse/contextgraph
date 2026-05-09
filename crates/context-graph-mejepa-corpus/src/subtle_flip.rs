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
// Comments (`# ...`) and same-line single/double-quoted string literals are
// skipped (no flips inside them — that would just break the corpus entry's
// docstring instead of the program logic). Triple-quoted strings are NOT
// detected; tests should avoid placing flip candidates inside docstrings.

use crate::prng::SplitMix64;
use crate::{MutationError, MutationResult, MutationSite};

#[derive(Debug, Clone)]
struct Candidate {
    byte_offset: usize,
    byte_length: usize,
    original: &'static str,
    replacement: &'static str,
    note: &'static str,
}

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let candidates = find_candidates(source);
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "boolean_operator",
            "no boolean operator found in source",
            "ensure source contains at least one of `==`, `!=`, `<`, `>`, `<=`, `>=`, ` and `, ` or `, ` True`, ` False`",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let pick = rng.pick_index(candidates.len()).ok_or_else(|| {
        MutationError::op_failed(
            "rng",
            "SplitMix64::pick_index returned None despite non-empty candidates",
            "internal error; report",
        )
    })?;
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
    debug_assert_eq!(original_text, chosen.original);
    Ok(MutationSite {
        byte_offset: chosen.byte_offset,
        byte_length: chosen.byte_length,
        original_text,
        replacement_text: chosen.replacement.to_string(),
        note: chosen.note.to_string(),
    })
}

fn find_candidates(source: &str) -> Vec<Candidate> {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut candidates = Vec::new();
    let mut i = 0;
    let mut in_string: Option<u8> = None; // Some(b'"') or Some(b'\'')
    let mut in_comment = false;

    while i < n {
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

        // Two-char ops: `==`, `!=`, `<=`, `>=`
        if i + 1 < n {
            let pair = &bytes[i..i + 2];
            match pair {
                b"==" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: "==",
                        replacement: "!=",
                        note: "subtle_flip: equality flipped to inequality",
                    });
                    i += 2;
                    continue;
                }
                b"!=" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: "!=",
                        replacement: "==",
                        note: "subtle_flip: inequality flipped to equality",
                    });
                    i += 2;
                    continue;
                }
                b"<=" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: "<=",
                        replacement: ">=",
                        note: "subtle_flip: <= flipped to >=",
                    });
                    i += 2;
                    continue;
                }
                b">=" => {
                    candidates.push(Candidate {
                        byte_offset: i,
                        byte_length: 2,
                        original: ">=",
                        replacement: "<=",
                        note: "subtle_flip: >= flipped to <=",
                    });
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Single-char `<` and `>`. Skip `<<`/`>>` (already handled above for
        // `<=`/`>=`; these would be Python bitshift `<<`/`>>` if present).
        if c == b'<' && bytes.get(i + 1).copied() != Some(b'<') {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: 1,
                original: "<",
                replacement: ">",
                note: "subtle_flip: < flipped to >",
            });
            i += 1;
            continue;
        }
        if c == b'>' && bytes.get(i + 1).copied() != Some(b'>') {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: 1,
                original: ">",
                replacement: "<",
                note: "subtle_flip: > flipped to <",
            });
            i += 1;
            continue;
        }

        // Word-boundary keywords. We use the surrounding bytes to verify the
        // match isn't inside a longer identifier. ASCII-only check is fine
        // because Python keywords are ASCII.
        if let Some(cand) = match_keyword(bytes, i, b"and", " and ", "( and ", " and ", "subtle_flip: and -> or") {
            candidates.push(Candidate {
                byte_offset: cand.0,
                byte_length: cand.1,
                original: "and",
                replacement: "or",
                note: "subtle_flip: and -> or",
            });
            i = cand.0 + cand.1;
            continue;
        }
        if let Some(cand) = match_keyword(bytes, i, b"or", " or ", "( or ", " or ", "subtle_flip: or -> and") {
            candidates.push(Candidate {
                byte_offset: cand.0,
                byte_length: cand.1,
                original: "or",
                replacement: "and",
                note: "subtle_flip: or -> and",
            });
            i = cand.0 + cand.1;
            continue;
        }
        if let Some(cand) = match_keyword(bytes, i, b"True", " True", "(True)", " True", "subtle_flip: True -> False") {
            candidates.push(Candidate {
                byte_offset: cand.0,
                byte_length: cand.1,
                original: "True",
                replacement: "False",
                note: "subtle_flip: True -> False",
            });
            i = cand.0 + cand.1;
            continue;
        }
        if let Some(cand) = match_keyword(bytes, i, b"False", " False", "(False)", " False", "subtle_flip: False -> True") {
            candidates.push(Candidate {
                byte_offset: cand.0,
                byte_length: cand.1,
                original: "False",
                replacement: "True",
                note: "subtle_flip: False -> True",
            });
            i = cand.0 + cand.1;
            continue;
        }

        i += 1;
    }
    candidates
}

/// Match a Python-style word-boundaried keyword at byte offset `i`.
/// Returns Some((offset, length)) on a match, where (offset == i) and length
/// is the keyword's byte-length. Word boundary: previous and following byte
/// (if present) must be non-word (NOT `[A-Za-z0-9_]`).
///
/// The extra `_*` parameters are ignored (legacy from earlier API); kept
/// for stability of call sites in this module.
#[allow(clippy::too_many_arguments)]
fn match_keyword(
    bytes: &[u8],
    i: usize,
    keyword: &[u8],
    _ctx_with_spaces: &str,
    _ctx_in_parens: &str,
    _ctx_at_eol: &str,
    _note: &str,
) -> Option<(usize, usize)> {
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
        Some((i, kw_len))
    } else {
        None
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

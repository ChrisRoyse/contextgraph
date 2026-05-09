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
    let code_mask = python_code_mask(source);
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
                original: "<",
                replacement: ">",
                note: "subtle_flip: < flipped to >",
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
        if let Some(len) = match_keyword(bytes, i, b"and") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "and",
                replacement: "or",
                note: "subtle_flip: and -> or",
            });
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"or") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "or",
                replacement: "and",
                note: "subtle_flip: or -> and",
            });
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"True") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "True",
                replacement: "False",
                note: "subtle_flip: True -> False",
            });
            i += len;
            continue;
        }
        if let Some(len) = match_keyword(bytes, i, b"False") {
            candidates.push(Candidate {
                byte_offset: i,
                byte_length: len,
                original: "False",
                replacement: "True",
                note: "subtle_flip: False -> True",
            });
            i += len;
            continue;
        }

        i += 1;
    }
    candidates
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

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

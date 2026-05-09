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
    digits: String,
}

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let candidates = find_candidates(source)?;
    if candidates.is_empty() {
        return Err(MutationError::no_site(
            "numeric_literal",
            "no integer literal found in source (excluding floats and identifiers)",
            "ensure source contains at least one bare integer literal like `0`, `1`, `42`",
        ));
    }
    let mut rng = SplitMix64::new(seed);
    let pick = pick_index_or_fail(&mut rng, candidates.len())?;
    let chosen = &candidates[pick];

    let value: i64 = chosen.digits.parse().map_err(|e| {
        MutationError::op_failed(
            "candidate.parse",
            format!("could not parse `{}` as i64: {e}", chosen.digits),
            "internal error; expected only digits",
        )
    })?;
    let direction: i64 = if seed & 1 == 0 { 1 } else { -1 };
    let new_value = value.checked_add(direction).ok_or_else(|| {
        MutationError::op_failed(
            "candidate.checked_add",
            format!("integer overflow adding {direction} to {value}"),
            "the chosen literal is at i64 boundary; pick a different seed",
        )
    })?;
    let replacement = new_value.to_string();
    let dir_label = if direction > 0 { "+1" } else { "-1" };
    Ok(MutationSite {
        byte_offset: chosen.byte_offset,
        byte_length: chosen.byte_length,
        original_text: chosen.digits.clone(),
        replacement_text: replacement,
        note: format!(
            "off_by_one: numeric literal {} mutated by {dir_label}",
            chosen.digits
        ),
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
                candidates.push(Candidate {
                    byte_offset: i,
                    byte_length: j - i,
                    digits,
                });
            }
            i = j;
            continue;
        }
        i += 1;
    }
    Ok(candidates)
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

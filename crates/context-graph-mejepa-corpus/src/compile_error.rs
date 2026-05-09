// MutationCategory::CompileError — introduce a Python syntax error. Doc 09
// §3.1 row 8.
//
// Expected oracle: FAIL at parse. We produce one of three variants,
// selected by `seed % 3`:
//   0. Append `x = (` at end of file (unclosed paren).
//   1. Append a colon-less control-flow header `if True\n    pass\n`.
//   2. Append `def _bad_(:\n    pass\n` (invalid argument syntax).
//
// Always an APPEND — we don't try to identify and corrupt an existing
// statement, since that's tree-sitter territory. The append-at-EOF strategy
// is robust across any input source.
//
// `byte_offset` = end of source (after a possible trailing newline);
// `byte_length` = 0; `replacement_text` = the syntax-error snippet.

use crate::prng::SplitMix64;
use crate::util::append_insertion_offset;
use crate::{MutationResult, MutationSite};

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mut rng = SplitMix64::new(seed);
    let variant = (rng.next_u64() % 3) as u8;
    let snippet = match variant {
        0 => "\n# CompileError mutation: unclosed paren\nx = (\n",
        1 => "\n# CompileError mutation: missing colon on control-flow header\nif True\n    pass\n",
        2 => "\n# CompileError mutation: invalid def signature\ndef _bad_(:\n    pass\n",
        _ => unreachable!(),
    };
    let label = match variant {
        0 => "unclosed_paren",
        1 => "missing_colon",
        2 => "invalid_def_signature",
        _ => "unknown",
    };
    let insert_at = append_insertion_offset(source);
    Ok(MutationSite {
        byte_offset: insert_at,
        byte_length: 0,
        original_text: String::new(),
        replacement_text: snippet.to_string(),
        note: format!("compile_error: appended {label} variant at byte {insert_at}"),
    })
}

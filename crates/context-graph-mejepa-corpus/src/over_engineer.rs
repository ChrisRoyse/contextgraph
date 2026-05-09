// MutationCategory::OverEngineer — append unrelated dead code (an unused
// helper function) to the end of the source. Doc 09 §3.1 row 7.
//
// Expected oracle: PASS — the helper is unreachable, so behavior is
// unchanged. The mutation tests whether ME-JEPA correctly predicts PASS
// despite syntactic noise that adds LOC and a new symbol.
//
// The helper's name is parameterized by the seed for determinism + to keep
// multiple OverEngineer entries per task distinct.
//
// `byte_offset` is the byte index of the source's terminating newline (or
// `source.len()` if there is none). `byte_length` is 0 — pure insertion.
// `replacement_text` is the helper block (with leading `\n\n`).

use crate::prng::SplitMix64;
use crate::{MutationResult, MutationSite};

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mut rng = SplitMix64::new(seed);
    let suffix_id = rng.next_u64() & 0xFFFFFF; // 24-bit hex tag
    let helper = format!(
        "\n\ndef _unused_helper_{suffix_id:06x}() -> None:\n    \"\"\"Auto-generated dead helper for OverEngineer mutation.\"\"\"\n    return None\n"
    );
    let insert_at = trailing_newline_offset(source);
    Ok(MutationSite {
        byte_offset: insert_at,
        byte_length: 0,
        original_text: String::new(),
        replacement_text: helper,
        note: format!(
            "over_engineer: appended unused helper `_unused_helper_{suffix_id:06x}` at byte {insert_at}"
        ),
    })
}

/// Return the byte offset at which we should splice the helper. We splice
/// BEFORE any single trailing `\n` (or at the very end if the source has no
/// trailing newline) so the helper ends with its own `\n` and we don't
/// produce a doubled newline at EOF.
fn trailing_newline_offset(source: &str) -> usize {
    if source.ends_with('\n') {
        source.len() - 1
    } else {
        source.len()
    }
}

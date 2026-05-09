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
use crate::util::append_insertion_offset;
use crate::{MutationResult, MutationSite};

pub(crate) fn apply(source: &str, seed: u64) -> MutationResult<MutationSite> {
    let mut rng = SplitMix64::new(seed);
    // 48-bit hex tag. 24-bit hits 50% birthday collision around ~4,820 helpers,
    // which is below plausible Phase-1 corpus populations once re-rolls + multi-
    // helper-per-task scenarios stack. 48-bit pushes the same threshold to
    // ~19.7M helpers, comfortably above any realistic corpus.
    let suffix_id = rng.next_u64() & 0xFFFF_FFFF_FFFF;
    let helper = format!(
        "\n\ndef _unused_helper_{suffix_id:012x}() -> None:\n    \"\"\"Auto-generated dead helper for OverEngineer mutation.\"\"\"\n    return None\n"
    );
    let insert_at = append_insertion_offset(source);
    Ok(MutationSite {
        byte_offset: insert_at,
        byte_length: 0,
        original_text: String::new(),
        replacement_text: helper,
        note: format!(
            "over_engineer: appended unused helper `_unused_helper_{suffix_id:012x}` at byte {insert_at}"
        ),
    })
}

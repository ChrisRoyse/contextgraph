// MutationCategory::KnownGood — apply the official fix unchanged.
// Returns the input verbatim with no `mutation_site`. Doc 09 §3.1 row 1.

use crate::{MutationCategory, MutationOutcome, MutationResult};

pub(crate) fn apply(primary_source: &str, seed: u64) -> MutationResult<MutationOutcome> {
    Ok(MutationOutcome {
        category: MutationCategory::KnownGood,
        mutated_source: primary_source.to_string(),
        seed,
        mutation_site: None,
    })
}

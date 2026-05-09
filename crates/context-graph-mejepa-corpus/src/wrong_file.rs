// MutationCategory::WrongFile — applying the canonical fix to a different
// base file results in the alternate base file unchanged (the patch's edits
// are no longer the right edits for the new file). Doc 09 §3.1 row 6.
//
// Modeling: this operator returns `alternate_source` verbatim as the
// "mutated" output. The corpus entry's metadata records that the patch was
// "applied" to the wrong file — the oracle should fail because tests
// reference symbols that aren't where the patch put them.
//
// `mutation_site` is None because there is no in-place edit; the entire
// primary source is replaced by an unrelated file.

use crate::{MutationCategory, MutationError, MutationOutcome, MutationResult};

pub(crate) fn apply(
    primary_source: &str,
    alternate_source: &str,
    seed: u64,
) -> MutationResult<MutationOutcome> {
    if alternate_source.is_empty() {
        return Err(MutationError::invalid(
            "alternate_source",
            "alternate_source is empty; WrongFile requires non-empty alternate",
            "supply a structurally-similar but distinct file's content",
        ));
    }
    if alternate_source == primary_source {
        return Err(MutationError::invalid(
            "alternate_source",
            "alternate_source is byte-equal to primary_source; WrongFile would be a no-op",
            "supply an alternate file whose bytes differ from primary_source",
        ));
    }
    Ok(MutationOutcome {
        category: MutationCategory::WrongFile,
        mutated_source: alternate_source.to_string(),
        seed,
        mutation_site: None,
    })
}

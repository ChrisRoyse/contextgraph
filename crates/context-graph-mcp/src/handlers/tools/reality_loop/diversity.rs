// Inspired by ruvnet/RuVector crates/ruvector-core/src/advanced_features/mmr.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
// Primary algorithm reference: Carbonell & Goldstein, SIGIR 1998, "The Use of
// MMR, Diversity-Based Reranking for Reordering Documents and Producing Summaries."

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MmrSelection {
    pub index: usize,
    pub mmr_score: f64,
    pub diversity_penalty: f64,
}

pub(crate) fn mmr_select_indices(
    relevances: &[f64],
    texts: &[String],
    limit: usize,
    lambda: f64,
) -> Vec<MmrSelection> {
    debug_assert_eq!(relevances.len(), texts.len());
    if limit == 0 || relevances.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::<MmrSelection>::new();
    let mut remaining = (0..relevances.len()).collect::<Vec<_>>();
    while !remaining.is_empty() && selected.len() < limit {
        let mut best_pos = 0usize;
        let mut best = MmrSelection {
            index: remaining[0],
            mmr_score: f64::NEG_INFINITY,
            diversity_penalty: 0.0,
        };
        for (pos, idx) in remaining.iter().copied().enumerate() {
            let diversity_penalty = selected
                .iter()
                .map(|chosen| token_similarity(&texts[idx], &texts[chosen.index]))
                .fold(0.0, f64::max);
            let mmr_score = lambda * relevances[idx] - (1.0 - lambda) * diversity_penalty;
            if mmr_score > best.mmr_score || (mmr_score == best.mmr_score && idx < best.index) {
                best_pos = pos;
                best = MmrSelection {
                    index: idx,
                    mmr_score,
                    diversity_penalty,
                };
            }
        }
        remaining.remove(best_pos);
        selected.push(best);
    }
    selected
}

pub(crate) fn token_similarity(a: &str, b: &str) -> f64 {
    let a = tokens(a);
    let b = tokens(b);
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let inter = a.intersection(&b).count() as f64;
    let union = a.union(&b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

pub(crate) fn has_retrieval_tokens(text: &str) -> bool {
    !tokens(text).is_empty()
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .filter(|item| item.len() >= 2)
        .map(|item| item.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmr_prefers_novel_candidate_when_duplicate_is_redundant() {
        let texts = vec![
            "missing import module loader generic fix".to_string(),
            "missing import module loader generic fix duplicate".to_string(),
            "timeout process watchdog runtime generic fix".to_string(),
        ];
        let relevances = vec![0.70, 0.69, 0.55];
        let selected = mmr_select_indices(&relevances, &texts, 2, 0.30);
        assert_eq!(
            selected.iter().map(|item| item.index).collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert!(selected[1].diversity_penalty < token_similarity(&texts[0], &texts[1]));
    }

    #[test]
    fn mmr_lambda_one_preserves_relevance_order() {
        let texts = vec![
            "alpha".to_string(),
            "alpha beta".to_string(),
            "gamma".to_string(),
        ];
        let relevances = vec![0.4, 0.9, 0.6];
        let selected = mmr_select_indices(&relevances, &texts, 3, 1.0);
        assert_eq!(
            selected.iter().map(|item| item.index).collect::<Vec<_>>(),
            vec![1, 2, 0]
        );
    }

    #[test]
    fn token_similarity_is_deterministic() {
        let close = token_similarity("import error missing module", "missing import module");
        let far = token_similarity("import error missing module", "timeout docker failure");
        assert!(close > far);
    }
}

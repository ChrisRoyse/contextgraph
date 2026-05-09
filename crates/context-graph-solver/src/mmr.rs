// Inspired by ruvnet/RuVector crates/ruvector-core/src/advanced_features/mmr.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Algorithm reference:
//   Carbonell & Goldstein, SIGIR 1998,
//   "The Use of MMR, Diversity-Based Reranking for Reordering Documents
//    and Producing Summaries."
//
// Generic Maximal Marginal Relevance reranker. Caller supplies relevance
// scores and a similarity closure over candidate indices; the reranker is
// payload-agnostic. The optimizer's text-Jaccard-similarity caller in
// `context-graph-mcp` is one consumer; ME-JEPA-Code training-batch
// diversity sampling will be another.

use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};

/// Configuration for the generic MMR reranker.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MmrConfig {
    /// Trade-off in [0.0, 1.0]. λ=1.0 → pure relevance ordering;
    /// λ=0.0 → pure diversity (relevance ignored).
    pub lambda: f64,
    /// Hard ceiling on the number of selections returned. 0 → empty result.
    pub limit: usize,
}

impl Default for MmrConfig {
    /// Default: λ=0.5 (balanced relevance/diversity), limit=10 (typical
    /// retrieval k for human-readable result lists).
    fn default() -> Self {
        Self {
            lambda: 0.5,
            limit: 10,
        }
    }
}

/// One MMR-ranked selection with its computed score components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MmrSelection {
    /// Index into the caller's candidate slice.
    pub index: usize,
    /// `λ * relevance - (1-λ) * max_similarity_to_already_selected`.
    pub mmr_score: f64,
    /// `max_similarity_to_already_selected` at selection time.
    /// 0.0 for the first pick.
    pub diversity_penalty: f64,
}

fn validate_config(config: MmrConfig) -> SolverResult<()> {
    if !config.lambda.is_finite() || !(0.0..=1.0).contains(&config.lambda) {
        return Err(SolverError::invalid(
            "lambda",
            "MMR lambda must be a finite value in [0.0, 1.0]",
            "set lambda between 0.0 (pure diversity) and 1.0 (pure relevance)",
        ));
    }
    Ok(())
}

/// Generic MMR selection driven by a similarity closure.
///
/// `similarity(i, j)` MUST return a value in [0.0, 1.0] where 1.0 means
/// "indistinguishable" and 0.0 means "completely dissimilar." Symmetry is
/// not required by the algorithm but is strongly recommended; non-symmetric
/// similarities make the diversity penalty interpretation order-dependent.
///
/// `relevances[i]` SHOULD live in a comparable scalar range; absolute scale
/// matters only relative to similarity (both sides of `λ * rel - (1-λ) * sim`).
/// Callers that mix unbounded relevance with [0,1] similarity should
/// normalize relevance into [0,1] first, e.g. via min-max scaling.
///
/// Errors:
/// - `MmrError::DimensionMismatch` if `relevances.len()` is < the highest
///   index that the closure may be asked about (we use `relevances.len()`
///   as the candidate-set size).
pub fn select<F>(
    relevances: &[f64],
    config: MmrConfig,
    mut similarity: F,
) -> SolverResult<Vec<MmrSelection>>
where
    F: FnMut(usize, usize) -> f64,
{
    validate_config(config)?;
    if config.limit == 0 || relevances.is_empty() {
        return Ok(Vec::new());
    }
    let n = relevances.len();
    for (idx, score) in relevances.iter().enumerate() {
        if !score.is_finite() {
            return Err(SolverError::invalid(
                "relevances",
                format!("MMR relevance scores must be finite (index {idx} = {score})"),
                "filter out NaN/Inf candidates before scoring",
            ));
        }
    }

    let mut selected: Vec<MmrSelection> = Vec::with_capacity(config.limit.min(n));
    let mut remaining: Vec<usize> = (0..n).collect();

    while !remaining.is_empty() && selected.len() < config.limit {
        let mut best_pos = 0usize;
        let mut best = MmrSelection {
            index: remaining[0],
            mmr_score: f64::NEG_INFINITY,
            diversity_penalty: 0.0,
        };

        for (pos, idx) in remaining.iter().copied().enumerate() {
            let diversity_penalty = if selected.is_empty() {
                0.0
            } else {
                let mut max_sim: f64 = 0.0;
                for chosen in &selected {
                    let sim = similarity(idx, chosen.index);
                    if !sim.is_finite() || !(0.0..=1.0).contains(&sim) {
                        return Err(SolverError::invalid(
                            "similarity",
                            format!(
                                "MMR similarity closure must return a finite value in [0.0, 1.0] (similarity({idx},{}) = {sim})",
                                chosen.index
                            ),
                            "ensure similarity returns a value in [0.0, 1.0]",
                        ));
                    }
                    if sim > max_sim {
                        max_sim = sim;
                    }
                }
                max_sim
            };

            let mmr_score =
                config.lambda * relevances[idx] - (1.0 - config.lambda) * diversity_penalty;

            // Stable tiebreaker: prefer lower index. The first iteration
            // always takes (NEG_INFINITY < any finite mmr_score given
            // validated inputs above), so no sentinel clause is needed.
            let take =
                mmr_score > best.mmr_score || (mmr_score == best.mmr_score && idx < best.index);
            if take {
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

    Ok(selected)
}

/// Convenience wrapper for the common case where similarities are precomputed
/// as a row-major dense `n × n` matrix. Index pairs `(i, j)` look up
/// `similarities[i * n + j]`. Callers that already have a similarity matrix
/// avoid the closure-call overhead this way.
pub fn select_with_matrix(
    relevances: &[f64],
    similarities: &[f64],
    config: MmrConfig,
) -> SolverResult<Vec<MmrSelection>> {
    let n = relevances.len();
    if similarities.len() != n * n {
        return Err(SolverError::invalid(
            "similarities",
            format!(
                "similarity matrix must be n*n in row-major layout (got {} values, expected {})",
                similarities.len(),
                n * n
            ),
            "pass exactly relevances.len() * relevances.len() values",
        ));
    }
    for (idx, sim) in similarities.iter().enumerate() {
        if !sim.is_finite() || !(0.0..=1.0).contains(sim) {
            return Err(SolverError::invalid(
                "similarities",
                format!("similarity matrix values must be finite and in [0.0, 1.0] (index {idx} = {sim})"),
                "normalize similarities before calling MMR",
            ));
        }
    }
    select(relevances, config, |i, j| similarities[i * n + j])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jaccard_tokens(a: &str, b: &str) -> f64 {
        use std::collections::BTreeSet;
        let set_a: BTreeSet<&str> = a.split_whitespace().collect();
        let set_b: BTreeSet<&str> = b.split_whitespace().collect();
        if set_a.is_empty() && set_b.is_empty() {
            return 1.0;
        }
        let inter = set_a.intersection(&set_b).count() as f64;
        let union = set_a.union(&set_b).count() as f64;
        if union == 0.0 {
            0.0
        } else {
            inter / union
        }
    }

    #[test]
    fn empty_inputs_return_empty() {
        let out = select(&[], MmrConfig::default(), |_, _| 0.0).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn limit_zero_returns_empty() {
        let out = select(
            &[0.5, 0.4],
            MmrConfig {
                lambda: 0.5,
                limit: 0,
            },
            |_, _| 0.0,
        )
        .unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn lambda_one_yields_relevance_order() {
        let relevances = vec![0.30, 0.95, 0.60, 0.10];
        let out = select(
            &relevances,
            MmrConfig {
                lambda: 1.0,
                limit: 4,
            },
            |i, j| if i == j { 1.0 } else { 0.5 },
        )
        .unwrap();
        let order: Vec<usize> = out.iter().map(|s| s.index).collect();
        assert_eq!(order, vec![1, 2, 0, 3]);
    }

    #[test]
    fn lambda_zero_picks_pure_diversity_after_first() {
        // First pick is the lowest-index candidate (diversity penalty 0,
        // tiebreak on index). Subsequent picks minimize similarity to
        // the already-selected set.
        let relevances = vec![0.5, 0.5, 0.5];
        let sim = vec![
            1.0, 0.9, 0.1, // row 0
            0.9, 1.0, 0.2, // row 1
            0.1, 0.2, 1.0, // row 2
        ];
        let out = select_with_matrix(
            &relevances,
            &sim,
            MmrConfig {
                lambda: 0.0,
                limit: 2,
            },
        )
        .unwrap();
        assert_eq!(out[0].index, 0); // tiebreak on index
        assert_eq!(out[1].index, 2); // farthest from 0
    }

    #[test]
    fn duplicate_candidates_are_deprioritized() {
        let texts = vec![
            "missing import module loader generic fix",
            "missing import module loader generic fix duplicate",
            "timeout process watchdog runtime generic fix",
        ];
        let relevances = vec![0.70, 0.69, 0.55];
        let out = select(
            &relevances,
            MmrConfig {
                lambda: 0.30,
                limit: 2,
            },
            |i, j| jaccard_tokens(texts[i], texts[j]),
        )
        .unwrap();
        let order: Vec<usize> = out.iter().map(|s| s.index).collect();
        // Pick #0 first (highest relevance). Pick #2 second (lower relevance
        // but very low similarity to #0); skip #1 which is near-duplicate.
        assert_eq!(order, vec![0, 2]);
        assert!(out[1].diversity_penalty < 0.5);
    }

    #[test]
    fn rejects_lambda_out_of_range() {
        let err = select(
            &[0.5],
            MmrConfig {
                lambda: -0.1,
                limit: 1,
            },
            |_, _| 0.0,
        )
        .expect_err("negative lambda must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_lambda_above_one() {
        let err = select(
            &[0.5],
            MmrConfig {
                lambda: 1.5,
                limit: 1,
            },
            |_, _| 0.0,
        )
        .expect_err("lambda > 1 must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_non_finite_relevance() {
        let err = select(
            &[0.5, f64::NAN],
            MmrConfig {
                lambda: 0.5,
                limit: 2,
            },
            |_, _| 0.0,
        )
        .expect_err("NaN relevance must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_non_finite_similarity() {
        let err = select(
            &[0.5, 0.5],
            MmrConfig {
                lambda: 0.5,
                limit: 2,
            },
            |_, _| f64::INFINITY,
        )
        .expect_err("Inf similarity must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_similarity_below_zero() {
        let err = select(
            &[0.5, 0.5],
            MmrConfig {
                lambda: 0.5,
                limit: 2,
            },
            |_, _| -0.01,
        )
        .expect_err("negative similarity must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_similarity_above_one() {
        let err = select(
            &[0.5, 0.5],
            MmrConfig {
                lambda: 0.5,
                limit: 2,
            },
            |_, _| 1.01,
        )
        .expect_err("similarity above 1 must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_wrong_shape_similarity_matrix() {
        let err = select_with_matrix(
            &[0.5, 0.5],
            &[1.0, 0.5, 0.5], // 3 values, expected 4
            MmrConfig {
                lambda: 0.5,
                limit: 2,
            },
        )
        .expect_err("wrong shape must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn rejects_out_of_range_similarity_matrix_value_even_if_limit_one() {
        let err = select_with_matrix(
            &[0.5, 0.4],
            &[1.0, 1.2, 0.2, 1.0],
            MmrConfig {
                lambda: 1.0,
                limit: 1,
            },
        )
        .expect_err("matrix values outside [0,1] must error");
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }

    #[test]
    fn limit_exceeds_candidates_returns_all() {
        let relevances = vec![0.3, 0.6, 0.5];
        let out = select(
            &relevances,
            MmrConfig {
                lambda: 1.0,
                limit: 10,
            },
            |_, _| 0.0,
        )
        .unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn deterministic_under_ties() {
        // Tie on mmr_score → lower index wins.
        let relevances = vec![0.5, 0.5, 0.5, 0.5];
        let out = select(
            &relevances,
            MmrConfig {
                lambda: 1.0,
                limit: 4,
            },
            |_, _| 0.0,
        )
        .unwrap();
        let order: Vec<usize> = out.iter().map(|s| s.index).collect();
        assert_eq!(order, vec![0, 1, 2, 3]);
    }
}

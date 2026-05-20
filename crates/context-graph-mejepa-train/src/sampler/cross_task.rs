use crate::error::{TrainerError, TrainerErrorCode};
use crate::sampler::{BatchPlan, BatchSampler, PatchSimilarityGraph};
use serde_json::json;
use std::collections::BTreeSet;
use std::collections::HashSet;

/// Fail-closed gate (F-001): the maximum fraction of a batch that may be filled
/// from the (adversarial + cross-task) fallback pool before the trainer is
/// required to abort the batch.
///
/// Wired into `enforce_fallback_warning_ratio`, which returns
/// `TrainerError::MejepaTrainFallbackRatioExceeded` when the ratio exceeds this
/// threshold. The constant is `pub` so downstream operators (FSV harnesses, CI)
/// can quote the exact contract.
pub const DEFAULT_FALLBACK_WARNING_RATIO: f32 = 0.30;

pub fn build_patch_similarity_graph(
    e_diff_embeddings: &[Vec<f32>],
    cosine_threshold: f32,
) -> Result<PatchSimilarityGraph, TrainerError> {
    if !cosine_threshold.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("cosine_threshold must be finite; got {cosine_threshold}"),
        )
        .with_context(json!({
            "field": "build_patch_similarity_graph.cosine_threshold",
            "value": cosine_threshold,
            "remediation": "provide a finite cosine threshold in [-1.0, 1.0]"
        })));
    }
    let n = e_diff_embeddings.len();
    let mut neighbors = vec![Vec::new(); n];
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let sim = cosine_same_dim(
                &e_diff_embeddings[i],
                &e_diff_embeddings[j],
                "build_patch_similarity_graph",
            )?;
            if sim > cosine_threshold {
                neighbors[i].push(j);
            }
        }
    }
    Ok(PatchSimilarityGraph { neighbors })
}

pub fn related_tasks_above(graph: &PatchSimilarityGraph, task_idx: usize) -> &[usize] {
    graph
        .neighbors
        .get(task_idx)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

impl BatchSampler {
    pub fn next_batch_with_cross_task(
        &mut self,
        current_task_idx: usize,
        batch_size: usize,
    ) -> Result<BatchPlan, TrainerError> {
        let mut plan = self.force_prefix(batch_size);
        let mut used = plan.indices.iter().copied().collect::<BTreeSet<_>>();
        self.fill_adversarial_mix(&mut plan, batch_size, &mut used);
        let need = batch_size.saturating_sub(plan.indices.len());
        let regular_set: HashSet<usize> = self
            .regular_pool_excluding(&used, true)
            .into_iter()
            .collect();
        let neighbors = related_tasks_above(&self.patch_similarity_graph, current_task_idx)
            .iter()
            .copied()
            .filter(|idx| regular_set.contains(idx))
            .collect::<Vec<_>>();
        for _ in 0..need {
            let use_cross_task =
                self.rng.next_unit_f32() < self.config.cross_task_transfer_probability;
            let candidate = if use_cross_task {
                let pool = neighbors
                    .iter()
                    .copied()
                    .filter(|idx| !used.contains(idx))
                    .collect::<Vec<_>>();
                if pool.is_empty() {
                    plan.cross_task_fallback_count += 1;
                    self.weighted_pick_from_regular_excluding(&used, true)?
                } else {
                    let picked = pool[(self.rng.next_u64() as usize) % pool.len()];
                    plan.cross_task_indices.push(plan.indices.len());
                    Some(picked)
                }
            } else {
                self.weighted_pick_from_regular_excluding(&used, true)?
            };
            if let Some(idx) = candidate {
                if used.insert(idx) {
                    plan.indices.push(idx);
                    plan.regular_count += 1;
                }
            }
        }
        self.finalize_batch_plan(&mut plan, batch_size);
        enforce_fallback_warning_ratio(&plan)?;
        Ok(plan)
    }
}

/// Trainer-side gate that enforces the `DEFAULT_FALLBACK_WARNING_RATIO`
/// contract documented in F-001. Returns
/// `TrainerError::MejepaTrainFallbackRatioExceeded` (structured SoT context:
/// observed, threshold, counts) when the observed fallback ratio for the batch
/// exceeds the threshold.
pub(crate) fn enforce_fallback_warning_ratio(plan: &BatchPlan) -> Result<(), TrainerError> {
    let selected = plan.indices.len().max(1) as f32;
    let fallback_count = plan.adversarial_fallback_count + plan.cross_task_fallback_count;
    let ratio = fallback_count as f32 / selected;
    if ratio > DEFAULT_FALLBACK_WARNING_RATIO {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainFallbackRatioExceeded,
            format!(
                "sampler fallback ratio {ratio:.3} exceeds threshold {DEFAULT_FALLBACK_WARNING_RATIO:.3}"
            ),
        )
        .with_context(json!({
            "field": "sampler.fallback_ratio",
            "observed": ratio,
            "threshold": DEFAULT_FALLBACK_WARNING_RATIO,
            "adversarial_fallback_count": plan.adversarial_fallback_count,
            "cross_task_fallback_count": plan.cross_task_fallback_count,
            "selected_batch_count": plan.indices.len(),
            "remediation": "provide enough adversarial/cross-task candidates or lower requested ratios before training"
        })));
    }
    Ok(())
}

/// Fail-closed cosine between two same-dimension f32 vectors. Mirrors the
/// gold-standard `cosine_same_dim` in `dda.rs:384` — but adopts dedicated
/// `TrainerErrorCode` variants for the trainer's slot-identity / zero-norm
/// failure modes.
///
/// * dim mismatch  → `MejepaTrainPatchSimilarityDimMismatch`
/// * empty input   → `MejepaTrainPatchSimilarityDimMismatch` (dim=0 violates slot identity)
/// * zero norm     → `MejepaTrainPatchSimilarityZeroNormVector`
/// * NaN / Inf     → `MejepaTrainPatchSimilarityZeroNormVector` (zero norm folds NaN/Inf inputs into the same fail path; finiteness propagates through `dot/(na*nb)` and is caught by the post-clamp non-finite guard below)
pub(crate) fn cosine_same_dim(a: &[f32], b: &[f32], context: &str) -> Result<f32, TrainerError> {
    if a.len() != b.len() || a.is_empty() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainPatchSimilarityDimMismatch,
            format!(
                "cosine dimension mismatch for {context}: {} vs {} (empty={})",
                a.len(),
                b.len(),
                a.is_empty()
            ),
        )
        .with_context(json!({
            "context": context,
            "left_dim": a.len(),
            "right_dim": b.len(),
            "remediation": "ensure both patch-similarity input vectors share the same (>0) dimension per CLAUDE.md §6.2"
        })));
    }
    let dot = a.iter().zip(b).map(|(x, y)| *x * *y).sum::<f32>();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || !na.is_finite() || nb == 0.0 || !nb.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainPatchSimilarityZeroNormVector,
            format!("cosine received zero or non-finite norm vector for {context}"),
        )
        .with_context(json!({
            "context": context,
            "left_norm": na,
            "right_norm": nb,
            "remediation": "reject the malformed embedding upstream; do not synthesize a zero-norm patch fingerprint"
        })));
    }
    let raw = dot / (na * nb);
    if !raw.is_finite() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainPatchSimilarityZeroNormVector,
            format!("cosine produced non-finite value for {context}"),
        )
        .with_context(json!({
            "context": context,
            "dot": dot,
            "left_norm": na,
            "right_norm": nb,
            "remediation": "inspect upstream patch fingerprint pipeline for NaN/Inf contamination"
        })));
    }
    Ok(raw.clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sampler::BatchPlan;

    #[test]
    fn threshold_filters_neighbors() {
        let graph =
            build_patch_similarity_graph(&[vec![1.0, 0.0], vec![0.9, 0.1], vec![0.0, 1.0]], 0.7)
                .expect("real-data unit vectors must yield a graph");
        assert_eq!(graph.neighbors[0], vec![1]);
        assert_eq!(graph.neighbors[2], Vec::<usize>::new());
    }

    #[test]
    fn build_graph_fails_closed_on_dim_mismatch() {
        let err = build_patch_similarity_graph(&[vec![1.0, 0.0], vec![0.9, 0.1, 0.5]], 0.5)
            .expect_err("dim mismatch must fail closed");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_DIM_MISMATCH");
        assert_eq!(err.context["left_dim"], 2);
        assert_eq!(err.context["right_dim"], 3);
    }

    #[test]
    fn build_graph_fails_closed_on_zero_norm() {
        let err = build_patch_similarity_graph(&[vec![1.0, 0.0], vec![0.0, 0.0]], 0.5)
            .expect_err("zero-norm vector must fail closed");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR");
        let right_norm = err.context["right_norm"]
            .as_f64()
            .expect("right_norm in context");
        assert!((right_norm - 0.0).abs() < 1e-6);
    }

    #[test]
    fn build_graph_fails_closed_on_nan_input() {
        let err = build_patch_similarity_graph(&[vec![1.0, 0.0], vec![f32::NAN, 0.5]], 0.5)
            .expect_err("NaN input must fold into the zero-norm/non-finite cosine fail path");
        assert_eq!(err.code(), "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR");
    }

    #[test]
    fn build_graph_rejects_non_finite_threshold() {
        let err = build_patch_similarity_graph(&[vec![1.0, 0.0], vec![0.0, 1.0]], f32::NAN)
            .expect_err("non-finite threshold must fail closed");
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    fn batch_plan_with(adversarial_fb: usize, cross_task_fb: usize, indices: usize) -> BatchPlan {
        BatchPlan {
            indices: (0..indices).collect(),
            force_count: 0,
            regular_count: indices,
            adversarial_count: 0,
            adversarial_example_indices: Vec::new(),
            adversarial_fallback_count: adversarial_fb,
            cross_task_indices: Vec::new(),
            cross_task_fallback_count: cross_task_fb,
            operator_override_sampler_applied_count: 0,
            operator_override_boost_audit: Vec::new(),
            online_reward_signals_applied_count: 0,
            online_reward_boost_audit: Vec::new(),
        }
    }

    #[test]
    fn fallback_ratio_gate_passes_at_threshold() {
        // ratio = 3/10 = 0.30, exactly at the threshold -> Ok
        let plan = batch_plan_with(1, 2, 10);
        enforce_fallback_warning_ratio(&plan).expect("ratio at threshold must pass");
    }

    #[test]
    fn fallback_ratio_gate_fires_above_threshold() {
        // ratio = 4/10 = 0.40 > 0.30 -> Err
        let plan = batch_plan_with(2, 2, 10);
        let err = enforce_fallback_warning_ratio(&plan)
            .expect_err("ratio above threshold must fail closed");
        assert_eq!(err.code(), "MEJEPA_TRAIN_FALLBACK_RATIO_EXCEEDED");
        let observed = err.context["observed"]
            .as_f64()
            .expect("observed in context");
        let threshold = err.context["threshold"]
            .as_f64()
            .expect("threshold in context");
        assert!((observed - 0.4).abs() < 1e-6);
        assert!((threshold - 0.30).abs() < 1e-6);
        assert_eq!(err.context["adversarial_fallback_count"], 2);
        assert_eq!(err.context["cross_task_fallback_count"], 2);
    }

    #[test]
    fn fallback_ratio_gate_uses_dedicated_screaming_snake_code() {
        // Regression: F-001 originally collapsed to MEJEPA_TRAIN_CONFIG_INVALID.
        // The dedicated code must now be emitted.
        let plan = batch_plan_with(5, 5, 10);
        let err = enforce_fallback_warning_ratio(&plan).expect_err("must fail");
        assert_ne!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
        assert_eq!(err.code(), "MEJEPA_TRAIN_FALLBACK_RATIO_EXCEEDED");
    }

    #[test]
    fn cosine_same_dim_orthogonal_unit_vectors() {
        let v = cosine_same_dim(&[1.0, 0.0], &[0.0, 1.0], "test").expect("orthogonal");
        assert!(v.abs() < 1e-6);
    }

    #[test]
    fn cosine_same_dim_identical_unit_vectors() {
        let v = cosine_same_dim(&[1.0, 0.0], &[1.0, 0.0], "test").expect("identical");
        assert!((v - 1.0).abs() < 1e-6);
    }
}

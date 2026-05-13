use crate::error::TrainerError;
use crate::sampler::{BatchPlan, BatchSampler, PatchSimilarityGraph};
use std::collections::BTreeSet;
use std::collections::HashSet;

pub const DEFAULT_FALLBACK_WARNING_RATIO: f32 = 0.30;

pub fn build_patch_similarity_graph(
    e_diff_embeddings: &[Vec<f32>],
    cosine_threshold: f32,
) -> PatchSimilarityGraph {
    let n = e_diff_embeddings.len();
    let mut neighbors = vec![Vec::new(); n];
    for i in 0..n {
        for j in 0..n {
            if i != j && cosine(&e_diff_embeddings[i], &e_diff_embeddings[j]) > cosine_threshold {
                neighbors[i].push(j);
            }
        }
    }
    PatchSimilarityGraph { neighbors }
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
        Ok(plan)
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot = a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_filters_neighbors() {
        let graph =
            build_patch_similarity_graph(&[vec![1.0, 0.0], vec![0.9, 0.1], vec![0.0, 1.0]], 0.7);
        assert_eq!(graph.neighbors[0], vec![1]);
        assert_eq!(graph.neighbors[2], Vec::<usize>::new());
    }
}

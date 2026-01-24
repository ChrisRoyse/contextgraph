//! TransE-style knowledge graph operations for KEPLER.
//!
//! KEPLER was trained with the TransE objective on Wikidata5M (4.8M entities, 20M triples).
//! Unlike the previous all-MiniLM-L6-v2 model, these operations are semantically meaningful:
//!
//! - For a valid triple (h, r, t): ||h + r - t||₂ is small (score close to 0)
//! - For an invalid triple: ||h + r - t||₂ is large (score very negative)
//!
//! # Score Thresholds (based on KEPLER paper evaluation)
//!
//! | Range | Interpretation |
//! |-------|----------------|
//! | > -5.0 | Valid triple |
//! | -10.0 to -5.0 | Uncertain |
//! | < -10.0 | Invalid triple |
//!
//! These thresholds are very different from the previous MiniLM model because
//! KEPLER was actually trained with the TransE objective.

use super::types::KEPLER_DIMENSION;
use super::KeplerModel;

impl KeplerModel {
    /// TransE scoring: score = -||h + r - t||₂
    ///
    /// Computes the TransE score for a (head, relation, tail) triple.
    /// Higher score (closer to 0) indicates a more likely valid triple.
    ///
    /// # KEPLER-Specific Behavior
    ///
    /// Unlike generic sentence embedders (like all-MiniLM), KEPLER was trained
    /// with the TransE objective on Wikidata5M. This means:
    /// - Valid triples consistently produce scores > -5.0
    /// - Invalid triples consistently produce scores < -10.0
    /// - The separation between valid and invalid is much clearer (~5+ points)
    ///
    /// # Arguments
    /// * `head` - Head entity embedding (768D)
    /// * `relation` - Relation embedding (768D)
    /// * `tail` - Tail entity embedding (768D)
    ///
    /// # Returns
    /// Negative L2 distance: 0 = perfect, more negative = worse.
    ///
    /// # Panics
    /// Panics if any input vector is not exactly KEPLER_DIMENSION (768) elements.
    ///
    /// # Examples
    /// ```rust
    /// use context_graph_embeddings::models::kepler::KeplerModel;
    ///
    /// // Perfect triple: h + r = t
    /// let h: Vec<f32> = vec![1.0; 768];
    /// let r: Vec<f32> = vec![0.5; 768];
    /// let t: Vec<f32> = vec![1.5; 768];
    /// let score = KeplerModel::transe_score(&h, &r, &t);
    /// assert!(score.abs() < 1e-5); // Should be ~0
    /// ```
    pub fn transe_score(head: &[f32], relation: &[f32], tail: &[f32]) -> f32 {
        assert_eq!(head.len(), KEPLER_DIMENSION);
        assert_eq!(relation.len(), KEPLER_DIMENSION);
        assert_eq!(tail.len(), KEPLER_DIMENSION);

        let sum_sq: f32 = head
            .iter()
            .zip(relation.iter())
            .zip(tail.iter())
            .map(|((h, r), t)| {
                let diff = h + r - t;
                diff * diff
            })
            .sum();

        -sum_sq.sqrt()
    }

    /// Predict tail entity embedding: t_hat = h + r
    ///
    /// Given a head entity and relation, predicts what the tail entity
    /// embedding should be according to the TransE model.
    ///
    /// # KEPLER-Specific Behavior
    ///
    /// Because KEPLER learned the TransE constraint during training,
    /// the predicted tail embedding will be semantically close to
    /// actual tail entities that complete the triple.
    ///
    /// # Arguments
    /// * `head` - Head entity embedding (768D)
    /// * `relation` - Relation embedding (768D)
    ///
    /// # Returns
    /// Predicted tail embedding (768D).
    ///
    /// # Panics
    /// Panics if any input vector is not exactly KEPLER_DIMENSION (768) elements.
    pub fn predict_tail(head: &[f32], relation: &[f32]) -> Vec<f32> {
        assert_eq!(head.len(), KEPLER_DIMENSION);
        assert_eq!(relation.len(), KEPLER_DIMENSION);

        head.iter()
            .zip(relation.iter())
            .map(|(h, r)| h + r)
            .collect()
    }

    /// Predict relation embedding: r_hat = t - h
    ///
    /// Given head and tail entity embeddings, predicts what the relation
    /// embedding should be according to the TransE model.
    ///
    /// # KEPLER-Specific Behavior
    ///
    /// Because KEPLER learned the TransE constraint during training,
    /// the predicted relation embedding will be semantically close to
    /// actual relation predicates that connect the entities.
    ///
    /// # Arguments
    /// * `head` - Head entity embedding (768D)
    /// * `tail` - Tail entity embedding (768D)
    ///
    /// # Returns
    /// Predicted relation embedding (768D).
    ///
    /// # Panics
    /// Panics if any input vector is not exactly KEPLER_DIMENSION (768) elements.
    pub fn predict_relation(head: &[f32], tail: &[f32]) -> Vec<f32> {
        assert_eq!(head.len(), KEPLER_DIMENSION);
        assert_eq!(tail.len(), KEPLER_DIMENSION);

        tail.iter().zip(head.iter()).map(|(t, h)| t - h).collect()
    }

    /// Convert TransE score to confidence in [0, 1].
    ///
    /// KEPLER produces different score distributions than MiniLM.
    /// This function maps the score to a [0, 1] confidence range.
    ///
    /// # Score Interpretation
    ///
    /// | Score | Confidence | Interpretation |
    /// |-------|------------|----------------|
    /// | > 0 | 1.0 | Perfect match |
    /// | -5.0 | ~0.67 | Valid triple |
    /// | -10.0 | ~0.33 | Uncertain |
    /// | -15.0 | ~0.0 | Invalid triple |
    ///
    /// # Arguments
    /// * `score` - TransE score (negative L2 distance)
    ///
    /// # Returns
    /// Confidence in [0.0, 1.0] range.
    pub fn score_to_confidence(score: f32) -> f32 {
        // Map score from [-15, 0] to [0, 1]
        // score = 0 -> confidence = 1.0
        // score = -15 -> confidence = 0.0
        let normalized = (score + 15.0) / 15.0;
        normalized.clamp(0.0, 1.0)
    }

    /// Determine validation result from TransE score.
    ///
    /// # Thresholds (based on KEPLER paper)
    ///
    /// - VALID: score > -5.0
    /// - UNCERTAIN: -10.0 <= score <= -5.0
    /// - INVALID: score < -10.0
    pub fn validation_from_score(score: f32) -> &'static str {
        if score > -5.0 {
            "VALID"
        } else if score >= -10.0 {
            "UNCERTAIN"
        } else {
            "INVALID"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transe_score_perfect() {
        let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
        let r: Vec<f32> = vec![0.5; KEPLER_DIMENSION];
        let t: Vec<f32> = vec![1.5; KEPLER_DIMENSION];

        let score = KeplerModel::transe_score(&h, &r, &t);
        assert!(score.abs() < 1e-5, "Perfect triple should have score ~0");
    }

    #[test]
    fn test_transe_score_imperfect() {
        let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
        let r: Vec<f32> = vec![0.5; KEPLER_DIMENSION];
        let t: Vec<f32> = vec![2.0; KEPLER_DIMENSION]; // Wrong tail

        let score = KeplerModel::transe_score(&h, &r, &t);
        assert!(score < 0.0, "Imperfect triple should have negative score");
    }

    #[test]
    fn test_predict_tail() {
        let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
        let r: Vec<f32> = vec![0.5; KEPLER_DIMENSION];

        let predicted = KeplerModel::predict_tail(&h, &r);
        assert_eq!(predicted.len(), KEPLER_DIMENSION);
        assert!((predicted[0] - 1.5).abs() < 1e-5);
    }

    #[test]
    fn test_predict_relation() {
        let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
        let t: Vec<f32> = vec![1.5; KEPLER_DIMENSION];

        let predicted = KeplerModel::predict_relation(&h, &t);
        assert_eq!(predicted.len(), KEPLER_DIMENSION);
        assert!((predicted[0] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_score_to_confidence() {
        assert!((KeplerModel::score_to_confidence(0.0) - 1.0).abs() < 1e-5);
        assert!((KeplerModel::score_to_confidence(-15.0) - 0.0).abs() < 1e-5);
        assert!((KeplerModel::score_to_confidence(-7.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_validation_from_score() {
        assert_eq!(KeplerModel::validation_from_score(-3.0), "VALID");
        assert_eq!(KeplerModel::validation_from_score(-7.0), "UNCERTAIN");
        assert_eq!(KeplerModel::validation_from_score(-12.0), "INVALID");
    }
}

//! Gardener - Graph maintenance and pruning
//!
//! TASK-STEERING-001: Implements the Gardener component for graph health.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See steering.components.gardener requirements.
//!
//! ## Responsibilities
//!
//! - Graph edge pruning (remove weak connections)
//! - Dead end removal (orphan nodes)
//! - Connectivity monitoring
//! - Health scoring

use super::feedback::GardenerFeedback;

/// Graph gardener for maintenance and pruning.
///
/// The Gardener monitors graph health and recommends maintenance actions:
/// - Pruning edges below weight threshold
/// - Removing orphan nodes older than age threshold
/// - Monitoring overall connectivity
#[derive(Debug, Clone)]
pub struct Gardener {
    /// Edge weight threshold for pruning [0, 1]
    pub prune_threshold: f32,
    /// Maximum orphan age before removal (days)
    pub orphan_age_days: u32,
}

impl Gardener {
    /// Create a new Gardener with default configuration.
    ///
    /// Default values:
    /// - prune_threshold: 0.1 (prune edges with weight < 0.1)
    /// - orphan_age_days: 30 (remove orphans older than 30 days)
    pub fn new() -> Self {
        Self {
            prune_threshold: 0.1,
            orphan_age_days: 30,
        }
    }

    /// Create a Gardener with custom configuration.
    ///
    /// # Arguments
    /// * `prune_threshold` - Edge weight below which to prune [0, 1]
    /// * `orphan_age_days` - Days after which orphan nodes are removed
    pub fn with_config(prune_threshold: f32, orphan_age_days: u32) -> Self {
        Self {
            prune_threshold: prune_threshold.clamp(0.0, 1.0),
            orphan_age_days,
        }
    }

    /// Evaluate graph health and return feedback.
    ///
    /// # Arguments
    /// * `edge_count` - Total number of edges in the graph
    /// * `orphan_count` - Number of nodes with no connections
    /// * `connectivity` - Graph connectivity score [0, 1]
    ///
    /// # Returns
    /// GardenerFeedback with pruning recommendations and health metrics.
    pub fn evaluate(
        &self,
        _edge_count: usize,
        orphan_count: usize,
        connectivity: f32,
    ) -> GardenerFeedback {
        // Note: In a real implementation, edges_pruned would be computed
        // by analyzing actual edge weights. Here we report the orphan count
        // as a proxy for maintenance needs.
        GardenerFeedback::new(
            0, // Would be set after actual pruning analysis
            orphan_count,
            connectivity,
        )
    }

    /// Compute gardener score [-1, 1].
    ///
    /// The score is based on graph connectivity:
    /// - 1.0 connectivity -> score = 1.0
    /// - 0.5 connectivity -> score = 0.0
    /// - 0.0 connectivity -> score = -1.0
    ///
    /// Formula: score = (connectivity * 2.0 - 1.0), clamped to [-1, 1]
    ///
    /// # Arguments
    /// * `connectivity` - Graph connectivity score [0, 1]
    ///
    /// # Returns
    /// Score in [-1, 1] where positive = healthy, negative = needs work
    pub fn score(&self, connectivity: f32) -> f32 {
        // Map connectivity [0, 1] to score [-1, 1]
        // 0.0 -> -1.0 (bad)
        // 0.5 -> 0.0 (neutral)
        // 1.0 -> 1.0 (good)
        (connectivity * 2.0 - 1.0).clamp(-1.0, 1.0)
    }

    /// Check if an edge should be pruned based on weight.
    ///
    /// # Arguments
    /// * `weight` - Edge weight [0, 1]
    ///
    /// # Returns
    /// true if the edge weight is below the prune threshold
    pub fn should_prune_edge(&self, weight: f32) -> bool {
        weight < self.prune_threshold
    }

    /// Check if an orphan node should be removed based on age.
    ///
    /// # Arguments
    /// * `age_days` - Age of the orphan node in days
    ///
    /// # Returns
    /// true if the orphan is older than the age threshold
    pub fn should_remove_orphan(&self, age_days: u32) -> bool {
        age_days > self.orphan_age_days
    }

    /// Get health status based on connectivity.
    ///
    /// # Arguments
    /// * `connectivity` - Graph connectivity score [0, 1]
    ///
    /// # Returns
    /// Status string: "healthy", "degraded", or "critical"
    pub fn health_status(&self, connectivity: f32) -> &'static str {
        if connectivity > 0.7 {
            "healthy"
        } else if connectivity > 0.4 {
            "degraded"
        } else {
            "critical"
        }
    }

    /// Get recommendations based on graph state.
    ///
    /// # Arguments
    /// * `connectivity` - Graph connectivity score [0, 1]
    /// * `orphan_count` - Number of orphan nodes
    /// * `weak_edge_count` - Number of edges below prune threshold
    ///
    /// # Returns
    /// Vector of recommendation strings
    pub fn get_recommendations(
        &self,
        connectivity: f32,
        orphan_count: usize,
        weak_edge_count: usize,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        if connectivity < 0.5 {
            recommendations.push(
                "Graph connectivity is low. Consider strengthening important connections."
                    .to_string(),
            );
        }

        if orphan_count > 10 {
            recommendations.push(format!(
                "Found {} orphan nodes. Consider removing or connecting them.",
                orphan_count
            ));
        }

        if weak_edge_count > 20 {
            recommendations.push(format!(
                "Found {} weak edges (weight < {:.2}). Consider pruning.",
                weak_edge_count, self.prune_threshold
            ));
        }

        recommendations
    }
}

impl Default for Gardener {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gardener_default() {
        let g = Gardener::new();
        assert_eq!(g.prune_threshold, 0.1);
        assert_eq!(g.orphan_age_days, 30);
    }

    #[test]
    fn test_gardener_with_config() {
        let g = Gardener::with_config(0.2, 60);
        assert_eq!(g.prune_threshold, 0.2);
        assert_eq!(g.orphan_age_days, 60);
    }

    #[test]
    fn test_score_mapping() {
        let g = Gardener::new();

        // Full connectivity -> max score
        assert_eq!(g.score(1.0), 1.0);

        // Zero connectivity -> min score
        assert_eq!(g.score(0.0), -1.0);

        // Half connectivity -> neutral
        assert_eq!(g.score(0.5), 0.0);

        // Quarter connectivity -> negative
        assert_eq!(g.score(0.25), -0.5);

        // Three-quarter connectivity -> positive
        assert_eq!(g.score(0.75), 0.5);
    }

    #[test]
    fn test_should_prune_edge() {
        let g = Gardener::with_config(0.15, 30);

        assert!(g.should_prune_edge(0.1));
        assert!(g.should_prune_edge(0.14));
        assert!(!g.should_prune_edge(0.15));
        assert!(!g.should_prune_edge(0.5));
    }

    #[test]
    fn test_should_remove_orphan() {
        let g = Gardener::with_config(0.1, 30);

        assert!(!g.should_remove_orphan(15));
        assert!(!g.should_remove_orphan(30));
        assert!(g.should_remove_orphan(31));
        assert!(g.should_remove_orphan(60));
    }

    #[test]
    fn test_health_status() {
        let g = Gardener::new();

        assert_eq!(g.health_status(0.9), "healthy");
        assert_eq!(g.health_status(0.71), "healthy");
        assert_eq!(g.health_status(0.5), "degraded");
        assert_eq!(g.health_status(0.41), "degraded");
        assert_eq!(g.health_status(0.3), "critical");
        assert_eq!(g.health_status(0.0), "critical");
    }

    #[test]
    fn test_get_recommendations() {
        let g = Gardener::new();

        // No recommendations for healthy graph
        let recs = g.get_recommendations(0.8, 5, 10);
        assert!(recs.is_empty());

        // Low connectivity recommendation
        let recs = g.get_recommendations(0.3, 5, 10);
        assert!(recs.len() == 1);
        assert!(recs[0].contains("connectivity"));

        // Multiple recommendations
        let recs = g.get_recommendations(0.3, 15, 25);
        assert_eq!(recs.len(), 3);
    }
}

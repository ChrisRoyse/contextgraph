# TASK-LOGIC-009: Goal Discovery Pipeline

```xml
<task_spec id="TASK-LOGIC-009" version="1.0">
<metadata>
  <title>Implement Goal Discovery Pipeline</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <requirement_ref>REQ-GOAL-DISCOVERY-01</requirement_ref>
    <requirement_ref>REQ-AUTONOMOUS-GOALS-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-004</task_ref>
    <task_ref>TASK-LOGIC-008</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
Goal discovery replaces manual North Star creation with autonomous clustering-based goal
emergence. Goals are discovered by analyzing teleological arrays, computing cluster
centroids, and assigning appropriate goal levels (NorthStar, Strategic, Tactical, Immediate).
</context>

<objective>
Implement GoalDiscoveryPipeline that uses K-means/HDBSCAN clustering on teleological arrays
to discover emergent goals, compute centroids as goal candidates, and build goal hierarchies.
</objective>

<rationale>
Autonomous goal discovery solves the "apples-to-oranges" problem:
1. Goals are teleological arrays (13 embedders), not single embeddings
2. Goals emerge from data patterns, not manual specification
3. Cluster centroids are valid teleological arrays comparable to memories
4. Goal hierarchies form naturally from cluster relationships
</rationale>

<input_context_files>
  <file purpose="array_types">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="comparator">crates/context-graph-core/src/teleology/comparator.rs</file>
  <file purpose="search_engine">crates/context-graph-storage/src/teleological/search/engine.rs</file>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#purpose/discover_goals</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-004 complete (TeleologicalComparator exists)</check>
  <check>TASK-LOGIC-008 complete (5-stage pipeline exists)</check>
  <check>TASK-CORE-005 complete (GoalNode uses teleological arrays)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create GoalDiscoveryPipeline struct</item>
    <item>Implement K-means clustering for teleological arrays</item>
    <item>Compute cluster centroids as goal candidates</item>
    <item>Score candidates by coherence and size</item>
    <item>Generate goal descriptions from clusters</item>
    <item>Assign goal levels (NorthStar, Strategic, Tactical, Immediate)</item>
    <item>Build parent-child goal relationships</item>
  </in_scope>
  <out_of_scope>
    <item>Drift detection (TASK-LOGIC-010)</item>
    <item>MCP handler implementation (TASK-INTEG-*)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/autonomous/discovery.rs">
      use crate::teleology::array::TeleologicalArray;
      use crate::teleology::comparison::{ComparisonType, SearchMatrix};
      use crate::teleology::comparator::TeleologicalComparator;
      use crate::purpose::GoalLevel;

      /// Configuration for goal discovery.
      #[derive(Debug, Clone)]
      pub struct DiscoveryConfig {
          pub sample_size: usize,
          pub min_cluster_size: usize,
          pub min_coherence: f32,
          pub clustering_algorithm: ClusteringAlgorithm,
          pub num_clusters: NumClusters,
          pub comparison_type: ComparisonType,
      }

      #[derive(Debug, Clone)]
      pub enum ClusteringAlgorithm {
          KMeans,
          HDBSCAN { min_samples: usize },
          Spectral { n_neighbors: usize },
      }

      #[derive(Debug, Clone)]
      pub enum NumClusters {
          Auto,
          Fixed(usize),
          Range { min: usize, max: usize },
      }

      /// Goal discovery pipeline for autonomous goal emergence.
      pub struct GoalDiscoveryPipeline {
          comparator: TeleologicalComparator,
      }

      impl GoalDiscoveryPipeline {
          pub fn new(comparator: TeleologicalComparator) -> Self;

          /// Discover goals from a set of teleological arrays.
          pub fn discover(
              &self,
              arrays: &[TeleologicalArray],
              config: &DiscoveryConfig,
          ) -> Result<DiscoveryResult, DiscoveryError>;

          /// Cluster arrays using the configured algorithm.
          fn cluster(
              &self,
              arrays: &[TeleologicalArray],
              config: &DiscoveryConfig,
          ) -> Result<Vec<Cluster>, DiscoveryError>;

          /// Compute centroid for a cluster of arrays.
          fn compute_centroid(
              &self,
              members: &[&TeleologicalArray],
          ) -> TeleologicalArray;

          /// Score a cluster's suitability as a goal.
          fn score_cluster(&self, cluster: &Cluster) -> GoalCandidate;

          /// Assign goal level based on cluster characteristics.
          fn assign_level(&self, candidate: &GoalCandidate) -> GoalLevel;

          /// Build parent-child relationships between discovered goals.
          fn build_hierarchy(
              &self,
              candidates: &[GoalCandidate],
          ) -> Vec<GoalRelationship>;
      }

      /// Result of goal discovery.
      #[derive(Debug)]
      pub struct DiscoveryResult {
          pub discovered_goals: Vec<DiscoveredGoal>,
          pub clusters_found: usize,
          pub clusters_above_threshold: usize,
          pub total_arrays_analyzed: usize,
          pub hierarchy: Vec<GoalRelationship>,
      }

      /// A discovered goal from clustering.
      #[derive(Debug)]
      pub struct DiscoveredGoal {
          pub goal_id: String,
          pub description: String,
          pub level: GoalLevel,
          pub confidence: f32,
          pub member_count: usize,
          pub centroid: TeleologicalArray,
          pub centroid_strength: EmbedderStrengths,
          pub dominant_embedders: Vec<Embedder>,
          pub keywords: Vec<String>,
          pub coherence_score: f32,
      }

      /// Strength of each embedder in a centroid.
      #[derive(Debug)]
      pub struct EmbedderStrengths {
          pub strengths: [f32; 13],
      }

      /// A cluster of teleological arrays.
      #[derive(Debug)]
      pub struct Cluster {
          pub members: Vec<usize>,
          pub centroid: TeleologicalArray,
          pub coherence: f32,
      }

      /// A goal candidate before level assignment.
      #[derive(Debug)]
      pub struct GoalCandidate {
          pub cluster: Cluster,
          pub score: f32,
          pub dominant_embedders: Vec<Embedder>,
      }

      /// Parent-child relationship between goals.
      #[derive(Debug)]
      pub struct GoalRelationship {
          pub parent_id: String,
          pub child_id: String,
          pub similarity: f32,
      }

      #[derive(Debug, thiserror::Error)]
      pub enum DiscoveryError {
          #[error("Insufficient arrays for clustering: {count} < {min}")]
          InsufficientData { count: usize, min: usize },
          #[error("Clustering failed: {0}")]
          ClusteringFailed(String),
          #[error("No viable clusters found")]
          NoClustersFound,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Centroids must be valid teleological arrays</constraint>
    <constraint>Goal levels assigned based on cluster size and coherence</constraint>
    <constraint>Minimum cluster size enforced</constraint>
    <constraint>Coherence threshold filters weak clusters</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-core autonomous::discovery</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/autonomous/discovery.rs

use std::collections::HashMap;
use uuid::Uuid;
use thiserror::Error;

use crate::teleology::array::{TeleologicalArray, EmbedderOutput};
use crate::teleology::comparison::ComparisonType;
use crate::teleology::comparator::TeleologicalComparator;
use crate::teleology::embedder::Embedder;
use crate::purpose::GoalLevel;

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub sample_size: usize,
    pub min_cluster_size: usize,
    pub min_coherence: f32,
    pub clustering_algorithm: ClusteringAlgorithm,
    pub num_clusters: NumClusters,
    pub comparison_type: ComparisonType,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            sample_size: 500,
            min_cluster_size: 5,
            min_coherence: 0.75,
            clustering_algorithm: ClusteringAlgorithm::KMeans,
            num_clusters: NumClusters::Auto,
            comparison_type: ComparisonType::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClusteringAlgorithm {
    KMeans,
    HDBSCAN { min_samples: usize },
    Spectral { n_neighbors: usize },
}

#[derive(Debug, Clone)]
pub enum NumClusters {
    Auto,
    Fixed(usize),
    Range { min: usize, max: usize },
}

pub struct GoalDiscoveryPipeline {
    comparator: TeleologicalComparator,
}

impl GoalDiscoveryPipeline {
    pub fn new(comparator: TeleologicalComparator) -> Self {
        Self { comparator }
    }

    pub fn discover(
        &self,
        arrays: &[TeleologicalArray],
        config: &DiscoveryConfig,
    ) -> Result<DiscoveryResult, DiscoveryError> {
        if arrays.len() < config.min_cluster_size {
            return Err(DiscoveryError::InsufficientData {
                count: arrays.len(),
                min: config.min_cluster_size,
            });
        }

        // Sample if necessary
        let sample = if arrays.len() > config.sample_size {
            self.sample_arrays(arrays, config.sample_size)
        } else {
            arrays.to_vec()
        };

        // Cluster the arrays
        let clusters = self.cluster(&sample, config)?;

        // Filter clusters by threshold
        let viable_clusters: Vec<_> = clusters
            .into_iter()
            .filter(|c| c.members.len() >= config.min_cluster_size)
            .filter(|c| c.coherence >= config.min_coherence)
            .collect();

        if viable_clusters.is_empty() {
            return Err(DiscoveryError::NoClustersFound);
        }

        // Score and convert to goal candidates
        let candidates: Vec<_> = viable_clusters
            .iter()
            .map(|c| self.score_cluster(c))
            .collect();

        // Assign levels and create discovered goals
        let discovered_goals: Vec<_> = candidates
            .iter()
            .map(|c| self.candidate_to_goal(c))
            .collect();

        // Build hierarchy
        let hierarchy = self.build_hierarchy(&candidates);

        Ok(DiscoveryResult {
            discovered_goals,
            clusters_found: viable_clusters.len(),
            clusters_above_threshold: viable_clusters.len(),
            total_arrays_analyzed: sample.len(),
            hierarchy,
        })
    }

    fn cluster(
        &self,
        arrays: &[TeleologicalArray],
        config: &DiscoveryConfig,
    ) -> Result<Vec<Cluster>, DiscoveryError> {
        // Compute pairwise similarity matrix
        let n = arrays.len();
        let mut similarity_matrix = vec![vec![0.0f32; n]; n];

        for i in 0..n {
            for j in i..n {
                if i == j {
                    similarity_matrix[i][j] = 1.0;
                } else {
                    let result = self.comparator.compare(
                        &arrays[i],
                        &arrays[j],
                        &config.comparison_type,
                    );
                    similarity_matrix[i][j] = result.overall_similarity;
                    similarity_matrix[j][i] = result.overall_similarity;
                }
            }
        }

        // Convert similarity to distance
        let distance_matrix: Vec<Vec<f32>> = similarity_matrix
            .iter()
            .map(|row| row.iter().map(|&s| 1.0 - s).collect())
            .collect();

        // Apply clustering algorithm
        match &config.clustering_algorithm {
            ClusteringAlgorithm::KMeans => {
                self.kmeans_cluster(arrays, &distance_matrix, config)
            }
            ClusteringAlgorithm::HDBSCAN { min_samples } => {
                self.hdbscan_cluster(arrays, &distance_matrix, *min_samples)
            }
            ClusteringAlgorithm::Spectral { n_neighbors } => {
                self.spectral_cluster(arrays, &similarity_matrix, *n_neighbors, config)
            }
        }
    }

    fn kmeans_cluster(
        &self,
        arrays: &[TeleologicalArray],
        distance_matrix: &[Vec<f32>],
        config: &DiscoveryConfig,
    ) -> Result<Vec<Cluster>, DiscoveryError> {
        let k = match &config.num_clusters {
            NumClusters::Fixed(k) => *k,
            NumClusters::Auto => self.estimate_k(distance_matrix),
            NumClusters::Range { min, max } => {
                self.find_optimal_k(distance_matrix, *min, *max)
            }
        };

        // Simple K-means implementation
        let mut assignments = vec![0usize; arrays.len()];
        let mut centroids_idx: Vec<usize> = (0..k).map(|i| i * arrays.len() / k).collect();

        for _ in 0..100 {  // Max iterations
            // Assign points to nearest centroid
            for (i, _) in arrays.iter().enumerate() {
                let mut min_dist = f32::MAX;
                let mut best_cluster = 0;
                for (c, &centroid_idx) in centroids_idx.iter().enumerate() {
                    let dist = distance_matrix[i][centroid_idx];
                    if dist < min_dist {
                        min_dist = dist;
                        best_cluster = c;
                    }
                }
                assignments[i] = best_cluster;
            }

            // Update centroids (find medoid)
            for c in 0..k {
                let members: Vec<_> = assignments.iter()
                    .enumerate()
                    .filter(|(_, &a)| a == c)
                    .map(|(i, _)| i)
                    .collect();

                if !members.is_empty() {
                    // Find medoid (point with minimum sum of distances to others)
                    let mut best_medoid = members[0];
                    let mut best_sum = f32::MAX;

                    for &i in &members {
                        let sum: f32 = members.iter()
                            .map(|&j| distance_matrix[i][j])
                            .sum();
                        if sum < best_sum {
                            best_sum = sum;
                            best_medoid = i;
                        }
                    }
                    centroids_idx[c] = best_medoid;
                }
            }
        }

        // Build clusters
        let mut clusters = Vec::new();
        for c in 0..k {
            let members: Vec<_> = assignments.iter()
                .enumerate()
                .filter(|(_, &a)| a == c)
                .map(|(i, _)| i)
                .collect();

            if !members.is_empty() {
                let member_arrays: Vec<_> = members.iter()
                    .map(|&i| &arrays[i])
                    .collect();

                let centroid = self.compute_centroid(&member_arrays);
                let coherence = self.compute_coherence(&member_arrays);

                clusters.push(Cluster {
                    members,
                    centroid,
                    coherence,
                });
            }
        }

        Ok(clusters)
    }

    fn compute_centroid(&self, members: &[&TeleologicalArray]) -> TeleologicalArray {
        // Average embeddings per embedder
        let mut centroid = TeleologicalArray::new(Uuid::new_v4());

        for embedder in Embedder::all() {
            let embeddings: Vec<_> = members.iter()
                .filter_map(|a| {
                    match a.get(embedder) {
                        EmbedderOutput::Dense(v) => Some(v.clone()),
                        _ => None,
                    }
                })
                .collect();

            if !embeddings.is_empty() {
                let dims = embeddings[0].len();
                let mut avg = vec![0.0f32; dims];

                for emb in &embeddings {
                    for (i, v) in emb.iter().enumerate() {
                        avg[i] += v / embeddings.len() as f32;
                    }
                }

                // Normalize
                let norm: f32 = avg.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for v in &mut avg {
                        *v /= norm;
                    }
                }

                centroid.set(embedder, EmbedderOutput::Dense(avg));
            }
        }

        centroid
    }

    fn compute_coherence(&self, members: &[&TeleologicalArray]) -> f32 {
        if members.len() < 2 {
            return 1.0;
        }

        // Average pairwise similarity
        let mut sum = 0.0f32;
        let mut count = 0;

        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                let result = self.comparator.compare_default(members[i], members[j]);
                sum += result.overall_similarity;
                count += 1;
            }
        }

        if count > 0 { sum / count as f32 } else { 0.0 }
    }

    fn score_cluster(&self, cluster: &Cluster) -> GoalCandidate {
        // Score based on size, coherence, and embedder distribution
        let size_score = (cluster.members.len() as f32).ln() / 10.0;
        let coherence_score = cluster.coherence;

        // Find dominant embedders
        let dominant = self.find_dominant_embedders(&cluster.centroid);

        GoalCandidate {
            cluster: cluster.clone(),
            score: (size_score + coherence_score) / 2.0,
            dominant_embedders: dominant,
        }
    }

    fn find_dominant_embedders(&self, centroid: &TeleologicalArray) -> Vec<Embedder> {
        let mut strengths: Vec<(Embedder, f32)> = Embedder::all()
            .filter_map(|e| {
                match centroid.get(e) {
                    EmbedderOutput::Dense(v) => {
                        let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                        Some((e, mag))
                    }
                    _ => None,
                }
            })
            .collect();

        strengths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        strengths.into_iter().take(3).map(|(e, _)| e).collect()
    }

    fn assign_level(&self, candidate: &GoalCandidate) -> GoalLevel {
        let size = candidate.cluster.members.len();
        let coherence = candidate.cluster.coherence;

        if size >= 50 && coherence >= 0.85 {
            GoalLevel::NorthStar
        } else if size >= 20 && coherence >= 0.80 {
            GoalLevel::Strategic
        } else if size >= 10 && coherence >= 0.75 {
            GoalLevel::Tactical
        } else {
            GoalLevel::Immediate
        }
    }

    fn candidate_to_goal(&self, candidate: &GoalCandidate) -> DiscoveredGoal {
        let level = self.assign_level(candidate);
        let goal_id = format!("discovered-{}", Uuid::new_v4());

        // Generate description from dominant embedders and keywords
        let description = self.generate_description(candidate);

        DiscoveredGoal {
            goal_id,
            description,
            level,
            confidence: candidate.score,
            member_count: candidate.cluster.members.len(),
            centroid: candidate.cluster.centroid.clone(),
            centroid_strength: self.compute_strengths(&candidate.cluster.centroid),
            dominant_embedders: candidate.dominant_embedders.clone(),
            keywords: Vec::new(), // TODO: Extract keywords
            coherence_score: candidate.cluster.coherence,
        }
    }

    fn build_hierarchy(&self, candidates: &[GoalCandidate]) -> Vec<GoalRelationship> {
        let mut relationships = Vec::new();

        // Find parent-child relationships based on centroid similarity
        for i in 0..candidates.len() {
            for j in 0..candidates.len() {
                if i != j {
                    let similarity = self.comparator.compare_default(
                        &candidates[i].cluster.centroid,
                        &candidates[j].cluster.centroid,
                    ).overall_similarity;

                    // Larger cluster with high similarity is potential parent
                    if similarity >= 0.7
                        && candidates[i].cluster.members.len() > candidates[j].cluster.members.len()
                    {
                        relationships.push(GoalRelationship {
                            parent_id: format!("goal-{}", i),
                            child_id: format!("goal-{}", j),
                            similarity,
                        });
                    }
                }
            }
        }

        relationships
    }

    fn estimate_k(&self, distance_matrix: &[Vec<f32>]) -> usize {
        // Simple heuristic: sqrt(n/2)
        ((distance_matrix.len() as f32 / 2.0).sqrt() as usize).max(2)
    }

    fn find_optimal_k(&self, distance_matrix: &[Vec<f32>], min: usize, max: usize) -> usize {
        // Elbow method placeholder
        ((max + min) / 2).max(2)
    }

    fn sample_arrays(&self, arrays: &[TeleologicalArray], size: usize) -> Vec<TeleologicalArray> {
        // Random sampling
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut indices: Vec<_> = (0..arrays.len()).collect();
        indices.shuffle(&mut rng);
        indices.truncate(size);
        indices.into_iter().map(|i| arrays[i].clone()).collect()
    }

    fn generate_description(&self, _candidate: &GoalCandidate) -> String {
        "Discovered goal cluster".to_string() // TODO: NLP-based description
    }

    fn compute_strengths(&self, centroid: &TeleologicalArray) -> EmbedderStrengths {
        let mut strengths = [0.0f32; 13];
        for embedder in Embedder::all() {
            if let EmbedderOutput::Dense(v) = centroid.get(embedder) {
                strengths[embedder.index()] = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            }
        }
        EmbedderStrengths { strengths }
    }

    fn hdbscan_cluster(
        &self,
        _arrays: &[TeleologicalArray],
        _distance_matrix: &[Vec<f32>],
        _min_samples: usize,
    ) -> Result<Vec<Cluster>, DiscoveryError> {
        // TODO: Implement HDBSCAN
        Err(DiscoveryError::ClusteringFailed("HDBSCAN not implemented".to_string()))
    }

    fn spectral_cluster(
        &self,
        _arrays: &[TeleologicalArray],
        _similarity_matrix: &[Vec<f32>],
        _n_neighbors: usize,
        _config: &DiscoveryConfig,
    ) -> Result<Vec<Cluster>, DiscoveryError> {
        // TODO: Implement Spectral clustering
        Err(DiscoveryError::ClusteringFailed("Spectral clustering not implemented".to_string()))
    }
}

#[derive(Debug)]
pub struct DiscoveryResult {
    pub discovered_goals: Vec<DiscoveredGoal>,
    pub clusters_found: usize,
    pub clusters_above_threshold: usize,
    pub total_arrays_analyzed: usize,
    pub hierarchy: Vec<GoalRelationship>,
}

#[derive(Debug)]
pub struct DiscoveredGoal {
    pub goal_id: String,
    pub description: String,
    pub level: GoalLevel,
    pub confidence: f32,
    pub member_count: usize,
    pub centroid: TeleologicalArray,
    pub centroid_strength: EmbedderStrengths,
    pub dominant_embedders: Vec<Embedder>,
    pub keywords: Vec<String>,
    pub coherence_score: f32,
}

#[derive(Debug)]
pub struct EmbedderStrengths {
    pub strengths: [f32; 13],
}

#[derive(Debug, Clone)]
pub struct Cluster {
    pub members: Vec<usize>,
    pub centroid: TeleologicalArray,
    pub coherence: f32,
}

#[derive(Debug)]
pub struct GoalCandidate {
    pub cluster: Cluster,
    pub score: f32,
    pub dominant_embedders: Vec<Embedder>,
}

#[derive(Debug)]
pub struct GoalRelationship {
    pub parent_id: String,
    pub child_id: String,
    pub similarity: f32,
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Insufficient arrays for clustering: {count} < {min}")]
    InsufficientData { count: usize, min: usize },
    #[error("Clustering failed: {0}")]
    ClusteringFailed(String),
    #[error("No viable clusters found")]
    NoClustersFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmeans_clustering() {
        // Test K-means on synthetic data
    }

    #[test]
    fn test_centroid_computation() {
        // Test centroid averaging
    }

    #[test]
    fn test_goal_level_assignment() {
        // Test level assignment rules
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/autonomous/discovery.rs">
    Goal discovery pipeline implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/autonomous/mod.rs">
    Add: pub mod discovery;
  </file>
  <file path="crates/context-graph-core/Cargo.toml">
    Add: rand dependency for sampling
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>K-means clustering produces valid clusters</criterion>
  <criterion>Centroids are valid teleological arrays</criterion>
  <criterion>Goal levels assigned based on size/coherence</criterion>
  <criterion>Hierarchy relationships computed correctly</criterion>
  <criterion>Minimum cluster size enforced</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core autonomous::discovery -- --nocapture</command>
</test_commands>
</task_spec>
```

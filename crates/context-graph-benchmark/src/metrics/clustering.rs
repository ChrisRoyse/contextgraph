//! Clustering quality metrics: Purity, NMI, ARI, Silhouette.
//!
//! These metrics evaluate how well the clustering algorithm groups documents by topic.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Clustering quality metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusteringMetrics {
    /// Cluster purity (average max overlap with ground truth topics).
    pub purity: f64,

    /// Normalized Mutual Information.
    pub nmi: f64,

    /// Adjusted Rand Index.
    pub ari: f64,

    /// Average Silhouette coefficient.
    pub silhouette: f64,

    /// Number of predicted clusters.
    pub cluster_count: usize,

    /// Number of ground truth topics.
    pub topic_count: usize,

    /// Cluster size statistics.
    pub cluster_sizes: ClusterSizeStats,
}

/// Statistics about cluster sizes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterSizeStats {
    pub min: usize,
    pub max: usize,
    pub mean: f64,
    pub std: f64,
}

impl ClusteringMetrics {
    /// Overall clustering score (weighted combination).
    pub fn overall_score(&self) -> f64 {
        // 30% purity + 30% NMI + 25% ARI + 15% silhouette
        0.30 * self.purity + 0.30 * self.nmi + 0.25 * self.ari + 0.15 * self.silhouette.max(0.0)
    }

    /// Check if clustering is considered good.
    pub fn is_good(&self) -> bool {
        self.purity >= 0.7 && self.nmi >= 0.5 && self.ari >= 0.4
    }
}

/// Compute cluster purity.
///
/// Purity = (1/N) * sum over clusters of max(count of class c in cluster k)
///
/// # Arguments
/// * `cluster_labels` - Predicted cluster for each document
/// * `true_labels` - Ground truth label for each document
pub fn compute_purity(cluster_labels: &[usize], true_labels: &[usize]) -> f64 {
    if cluster_labels.is_empty() || cluster_labels.len() != true_labels.len() {
        return 0.0;
    }

    let n = cluster_labels.len() as f64;

    // Count documents per (cluster, true_class) pair
    let mut counts: HashMap<(usize, usize), usize> = HashMap::new();
    for (&cluster, &true_class) in cluster_labels.iter().zip(true_labels.iter()) {
        *counts.entry((cluster, true_class)).or_insert(0) += 1;
    }

    // Get unique clusters
    let mut clusters: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for &c in cluster_labels {
        clusters.insert(c);
    }

    // Sum max class count per cluster
    let purity_sum: usize = clusters
        .iter()
        .map(|&cluster| {
            counts
                .iter()
                .filter(|((c, _), _)| *c == cluster)
                .map(|(_, &count)| count)
                .max()
                .unwrap_or(0)
        })
        .sum();

    purity_sum as f64 / n
}

/// Compute Normalized Mutual Information (NMI).
///
/// NMI = 2 * I(C; K) / (H(C) + H(K))
///
/// Where I is mutual information and H is entropy.
pub fn compute_nmi(cluster_labels: &[usize], true_labels: &[usize]) -> f64 {
    if cluster_labels.is_empty() || cluster_labels.len() != true_labels.len() {
        return 0.0;
    }

    let n = cluster_labels.len() as f64;

    // Count cluster sizes and class sizes
    let mut cluster_counts: HashMap<usize, f64> = HashMap::new();
    let mut class_counts: HashMap<usize, f64> = HashMap::new();
    let mut joint_counts: HashMap<(usize, usize), f64> = HashMap::new();

    for (&cluster, &class) in cluster_labels.iter().zip(true_labels.iter()) {
        *cluster_counts.entry(cluster).or_insert(0.0) += 1.0;
        *class_counts.entry(class).or_insert(0.0) += 1.0;
        *joint_counts.entry((cluster, class)).or_insert(0.0) += 1.0;
    }

    // Compute entropies
    let h_cluster: f64 = cluster_counts
        .values()
        .map(|&count| {
            let p = count / n;
            if p > 0.0 {
                -p * p.ln()
            } else {
                0.0
            }
        })
        .sum();

    let h_class: f64 = class_counts
        .values()
        .map(|&count| {
            let p = count / n;
            if p > 0.0 {
                -p * p.ln()
            } else {
                0.0
            }
        })
        .sum();

    // Compute mutual information
    let mi: f64 = joint_counts
        .iter()
        .map(|((cluster, class), &count)| {
            let p_joint = count / n;
            let p_cluster = cluster_counts[cluster] / n;
            let p_class = class_counts[class] / n;

            if p_joint > 0.0 && p_cluster > 0.0 && p_class > 0.0 {
                p_joint * (p_joint / (p_cluster * p_class)).ln()
            } else {
                0.0
            }
        })
        .sum();

    // NMI
    let denom = h_cluster + h_class;
    if denom < f64::EPSILON {
        0.0
    } else {
        2.0 * mi / denom
    }
}

/// Compute Adjusted Rand Index (ARI).
///
/// ARI adjusts the Rand Index for chance, ranging from -1 to 1.
/// ARI = 1 means perfect agreement, 0 means random, negative means worse than random.
pub fn compute_ari(cluster_labels: &[usize], true_labels: &[usize]) -> f64 {
    if cluster_labels.is_empty() || cluster_labels.len() != true_labels.len() {
        return 0.0;
    }

    let n = cluster_labels.len();

    // Build contingency table
    let mut contingency: HashMap<(usize, usize), usize> = HashMap::new();
    for (&cluster, &class) in cluster_labels.iter().zip(true_labels.iter()) {
        *contingency.entry((cluster, class)).or_insert(0) += 1;
    }

    // Row sums (cluster sizes)
    let mut row_sums: HashMap<usize, usize> = HashMap::new();
    for &cluster in cluster_labels {
        *row_sums.entry(cluster).or_insert(0) += 1;
    }

    // Column sums (class sizes)
    let mut col_sums: HashMap<usize, usize> = HashMap::new();
    for &class in true_labels {
        *col_sums.entry(class).or_insert(0) += 1;
    }

    // Compute sum of C(n_ij, 2) for contingency values
    let sum_comb_ij: f64 = contingency.values().map(|&x| comb2(x)).sum();

    // Compute sum of C(a_i, 2) for row sums
    let sum_comb_a: f64 = row_sums.values().map(|&x| comb2(x)).sum();

    // Compute sum of C(b_j, 2) for column sums
    let sum_comb_b: f64 = col_sums.values().map(|&x| comb2(x)).sum();

    // Total C(n, 2)
    let comb_n = comb2(n);

    // Expected index
    let expected = sum_comb_a * sum_comb_b / comb_n;

    // Max index
    let max_index = 0.5 * (sum_comb_a + sum_comb_b);

    // ARI
    let denom = max_index - expected;
    if denom.abs() < f64::EPSILON {
        0.0
    } else {
        (sum_comb_ij - expected) / denom
    }
}

/// Compute C(n, 2) = n * (n-1) / 2
fn comb2(n: usize) -> f64 {
    if n < 2 {
        0.0
    } else {
        (n * (n - 1)) as f64 / 2.0
    }
}

/// Compute Silhouette coefficient for a single point.
///
/// s(i) = (b(i) - a(i)) / max(a(i), b(i))
///
/// Where a(i) is the mean distance to other points in the same cluster,
/// and b(i) is the minimum mean distance to points in any other cluster.
pub fn silhouette_coefficient(
    point_idx: usize,
    cluster_labels: &[usize],
    distance_matrix: &[Vec<f64>],
) -> f64 {
    let cluster = cluster_labels[point_idx];
    let n = cluster_labels.len();

    // Get points in same cluster and other clusters
    let mut same_cluster_dists: Vec<f64> = Vec::new();
    let mut other_cluster_dists: HashMap<usize, Vec<f64>> = HashMap::new();

    for i in 0..n {
        if i == point_idx {
            continue;
        }

        let dist = distance_matrix[point_idx][i];

        if cluster_labels[i] == cluster {
            same_cluster_dists.push(dist);
        } else {
            other_cluster_dists
                .entry(cluster_labels[i])
                .or_default()
                .push(dist);
        }
    }

    // a(i) = mean distance to same cluster points
    let a = if same_cluster_dists.is_empty() {
        0.0
    } else {
        same_cluster_dists.iter().sum::<f64>() / same_cluster_dists.len() as f64
    };

    // b(i) = minimum mean distance to any other cluster
    let b = other_cluster_dists
        .values()
        .map(|dists| dists.iter().sum::<f64>() / dists.len() as f64)
        .min_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

    // Silhouette coefficient
    let max_ab = a.max(b);
    if max_ab < f64::EPSILON {
        0.0
    } else {
        (b - a) / max_ab
    }
}

/// Compute average Silhouette coefficient for all points.
pub fn compute_silhouette(cluster_labels: &[usize], distance_matrix: &[Vec<f64>]) -> f64 {
    if cluster_labels.is_empty() {
        return 0.0;
    }

    let n = cluster_labels.len();
    let sum: f64 = (0..n)
        .map(|i| silhouette_coefficient(i, cluster_labels, distance_matrix))
        .sum();

    sum / n as f64
}

/// Compute all clustering metrics.
pub fn compute_all_metrics(
    cluster_labels: &[usize],
    true_labels: &[usize],
    distance_matrix: Option<&[Vec<f64>]>,
) -> ClusteringMetrics {
    let purity = compute_purity(cluster_labels, true_labels);
    let nmi = compute_nmi(cluster_labels, true_labels);
    let ari = compute_ari(cluster_labels, true_labels);

    let silhouette = distance_matrix
        .map(|dm| compute_silhouette(cluster_labels, dm))
        .unwrap_or(0.0);

    // Cluster statistics
    let mut cluster_sizes: HashMap<usize, usize> = HashMap::new();
    for &c in cluster_labels {
        *cluster_sizes.entry(c).or_insert(0) += 1;
    }

    let sizes: Vec<usize> = cluster_sizes.values().copied().collect();
    let cluster_count = sizes.len();
    let topic_count = true_labels
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();

    let size_stats = if sizes.is_empty() {
        ClusterSizeStats::default()
    } else {
        let min = *sizes.iter().min().unwrap();
        let max = *sizes.iter().max().unwrap();
        let mean = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
        let variance: f64 = sizes
            .iter()
            .map(|&s| (s as f64 - mean).powi(2))
            .sum::<f64>()
            / sizes.len() as f64;
        let std = variance.sqrt();

        ClusterSizeStats {
            min,
            max,
            mean,
            std,
        }
    };

    ClusteringMetrics {
        purity,
        nmi,
        ari,
        silhouette,
        cluster_count,
        topic_count,
        cluster_sizes: size_stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_purity_perfect() {
        // Perfect clustering
        let cluster_labels = vec![0, 0, 0, 1, 1, 1];
        let true_labels = vec![0, 0, 0, 1, 1, 1];

        let purity = compute_purity(&cluster_labels, &true_labels);
        assert!((purity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_purity_imperfect() {
        // One document misplaced
        let cluster_labels = vec![0, 0, 1, 1, 1, 1];
        let true_labels = vec![0, 0, 0, 1, 1, 1];

        let purity = compute_purity(&cluster_labels, &true_labels);
        // Cluster 0: 2 docs, both class 0 -> max = 2
        // Cluster 1: 4 docs, 1 class 0, 3 class 1 -> max = 3
        // Purity = (2 + 3) / 6 = 0.833
        assert!((purity - 0.833).abs() < 0.01);
    }

    #[test]
    fn test_nmi_perfect() {
        let cluster_labels = vec![0, 0, 0, 1, 1, 1];
        let true_labels = vec![0, 0, 0, 1, 1, 1];

        let nmi = compute_nmi(&cluster_labels, &true_labels);
        assert!((nmi - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_ari_perfect() {
        let cluster_labels = vec![0, 0, 0, 1, 1, 1];
        let true_labels = vec![0, 0, 0, 1, 1, 1];

        let ari = compute_ari(&cluster_labels, &true_labels);
        assert!((ari - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_silhouette() {
        // Simple 2-cluster case with known distances
        let cluster_labels = vec![0, 0, 1, 1];
        let distance_matrix = vec![
            vec![0.0, 0.1, 0.9, 0.8],
            vec![0.1, 0.0, 0.8, 0.9],
            vec![0.9, 0.8, 0.0, 0.1],
            vec![0.8, 0.9, 0.1, 0.0],
        ];

        let silhouette = compute_silhouette(&cluster_labels, &distance_matrix);
        // Should be high (close to 1) since clusters are well separated
        assert!(silhouette > 0.5);
    }
}

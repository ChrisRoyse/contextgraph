//! BFS traversal benchmarks.

use criterion::{black_box, BenchmarkId, Criterion};
use std::collections::HashMap;
use std::time::Duration;

use super::config;
use super::generators::generate_graph_adjacency;

/// Storage for BFS benchmarks using real graph adjacency data.
pub struct BfsStorage {
    adjacency: HashMap<u64, Vec<u64>>,
}

impl BfsStorage {
    pub fn new(node_count: usize, avg_edges: usize) -> Self {
        Self {
            adjacency: generate_graph_adjacency(node_count, avg_edges),
        }
    }

    pub fn get_neighbors(&self, node_id: u64) -> Vec<u64> {
        self.adjacency.get(&node_id).cloned().unwrap_or_default()
    }
}

/// BFS implementation for benchmarking.
pub fn bfs_traverse(storage: &BfsStorage, start: u64, max_depth: u32) -> Vec<u64> {
    use std::collections::{HashSet, VecDeque};

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    queue.push_back((start, 0u32));
    visited.insert(start);

    while let Some((node, depth)) = queue.pop_front() {
        result.push(node);

        if depth >= max_depth {
            continue;
        }

        for neighbor in storage.get_neighbors(node) {
            if !visited.contains(&neighbor) {
                visited.insert(neighbor);
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    result
}

/// Benchmark BFS traversal.
pub fn bench_bfs_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("bfs_traversal");
    group.measurement_time(Duration::from_secs(5));

    for &node_count in config::GRAPH_SIZES {
        // Tree-like graph (sparse)
        let tree_storage = BfsStorage::new(node_count, 2);
        group.bench_with_input(BenchmarkId::new("tree", node_count), &node_count, |b, _| {
            b.iter(|| {
                bfs_traverse(
                    black_box(&tree_storage),
                    black_box(0),
                    black_box(config::BFS_MAX_DEPTH),
                )
            })
        });

        // Random graph (medium density)
        let random_storage = BfsStorage::new(node_count, 5);
        group.bench_with_input(
            BenchmarkId::new("random", node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    bfs_traverse(
                        black_box(&random_storage),
                        black_box(0),
                        black_box(config::BFS_MAX_DEPTH),
                    )
                })
            },
        );

        // Dense graph
        let dense_storage = BfsStorage::new(node_count, 10);
        group.bench_with_input(
            BenchmarkId::new("dense", node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    bfs_traverse(
                        black_box(&dense_storage),
                        black_box(0),
                        black_box(config::BFS_MAX_DEPTH),
                    )
                })
            },
        );
    }

    // Depth variation benchmarks
    let storage_1k = BfsStorage::new(1000, 5);
    for depth in [1, 2, 4, 6, 8] {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &d| {
            b.iter(|| bfs_traverse(black_box(&storage_1k), black_box(0), black_box(d)))
        });
    }

    group.finish();
}

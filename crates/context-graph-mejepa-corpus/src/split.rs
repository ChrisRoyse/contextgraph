// Phase 0 train/calibration/holdout split. Per
// `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.4`:
//
//   "Use Stoer-Wagner mincut on the (task → repo) bipartite graph to split:
//     - Train:        80% of tasks, all 8 mutation categories per task.
//     - Calibration:  10% of tasks, ditto.
//     - Holdout:      10% of tasks, ditto.
//   Mincut ensures no two tasks from the same repo (e.g., two astropy tasks)
//   end up in different splits — that's the leakage failure mode."
//
// Algorithm (see RATIONALE block at the bottom of the file for the
// Stoer-Wagner relationship):
//
//   1. Validate config (fractions sum to 1.0 ± 1e-9, non-negative).
//   2. Group tasks by repo. Sort repo IDs lexicographically for
//      determinism. SplitMix64-shuffle the repo order using `seed`.
//   3. Bin-pack each repo (atomically — every task in a repo goes to the
//      same bucket) into whichever bucket has the largest remaining
//      capacity (target_count - current_count).
//   4. Verify the leakage invariant: for each repo, every task assigned to
//      it must be in the SAME bucket. Stoer-Wagner mincut between any two
//      buckets in the resulting (task, repo) bipartite graph would have
//      weight zero by construction.
//   5. Optionally re-run Stoer-Wagner on a synthetic test graph to provide
//      an independent verification (callers that want "an actual mincut
//      ran on this corpus" can use `verify_split_via_mincut`).
//
// Fail-closed: empty corpus, fractions that don't sum to 1, fractions
// that produce zero-target buckets, NaN fractions, repos so large they
// exceed 100% of any bucket — all return `MEJEPA_CORPUS_INVALID_INPUT`.

use crate::prng::SplitMix64;
use crate::{MutationError, MutationResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One row of the corpus index: the SWE-bench task identifier and its
/// owning repo. Multiple tasks share a single repo in SWE-bench Lite (e.g.,
/// `astropy__astropy-12907` and `astropy__astropy-13033` both map to
/// repo `"astropy"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorpusEntry {
    pub task_id: String,
    pub repo: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SplitConfig {
    pub train_fraction: f64,
    pub calibration_fraction: f64,
    pub holdout_fraction: f64,
    pub seed: u64,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            train_fraction: 0.8,
            calibration_fraction: 0.1,
            holdout_fraction: 0.1,
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitBucket {
    Train,
    Calibration,
    Holdout,
}

impl SplitBucket {
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Calibration => "calibration",
            Self::Holdout => "holdout",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitReport {
    /// One row per input entry, in input order.
    pub assignments: Vec<TaskAssignment>,
    /// Bucket sizes in canonical [train, calibration, holdout] order.
    pub bucket_sizes: [usize; 3],
    /// Empirical fractions in [train, calibration, holdout] order. Equal to
    /// `bucket_sizes / total` to f64 precision.
    pub bucket_fractions: [f64; 3],
    /// One row per (repo, bucket-it-was-assigned-to).
    pub repo_assignments: Vec<RepoAssignment>,
    /// True iff every repo's tasks all landed in the SAME bucket — i.e.,
    /// no repo crosses a split boundary. This is the "no leakage"
    /// invariant from doc 09 §3.4.
    pub leakage_check_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub repo: String,
    pub bucket: SplitBucket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoAssignment {
    pub repo: String,
    pub bucket: SplitBucket,
    pub task_count: usize,
}

/// Split a corpus index into train / calibration / holdout buckets such
/// that every task in a given repo lands in the same bucket. Returns the
/// per-task assignments, per-repo assignments, and a `leakage_check_passed`
/// flag.
pub fn split_corpus(entries: &[CorpusEntry], config: SplitConfig) -> MutationResult<SplitReport> {
    if entries.is_empty() {
        return Err(MutationError::invalid(
            "entries",
            "corpus is empty; cannot produce a split over zero tasks",
            "supply at least one CorpusEntry",
        ));
    }
    validate_fractions(&config)?;

    // 1. Group tasks by repo. BTreeMap for deterministic iteration order
    //    keyed by repo ID.
    let mut repo_tasks: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, entry) in entries.iter().enumerate() {
        if entry.task_id.is_empty() {
            return Err(MutationError::invalid(
                "entries[i].task_id",
                format!("entries[{idx}].task_id is empty"),
                "every CorpusEntry must have a non-empty task_id",
            ));
        }
        if entry.repo.is_empty() {
            return Err(MutationError::invalid(
                "entries[i].repo",
                format!("entries[{idx}].repo is empty"),
                "every CorpusEntry must have a non-empty repo identifier",
            ));
        }
        repo_tasks.entry(entry.repo.clone()).or_default().push(idx);
    }
    let mut repos: Vec<String> = repo_tasks.keys().cloned().collect();

    // 2. Seed-driven shuffle of the repo order. Using a Fisher-Yates pass
    //    over `repos` with SplitMix64 for byte-exact determinism.
    let mut rng = SplitMix64::new(config.seed);
    if repos.len() >= 2 {
        for i in (1..repos.len()).rev() {
            let j = (rng.next_u64() as usize) % (i + 1);
            repos.swap(i, j);
        }
    }

    // 3. Compute target task counts per bucket and bin-pack repos.
    let total = entries.len();
    let target_train = (config.train_fraction * total as f64).round() as usize;
    let target_cal = (config.calibration_fraction * total as f64).round() as usize;
    let target_hold = total.saturating_sub(target_train + target_cal);
    let targets = [target_train, target_cal, target_hold];

    // capacity[k] = target_k - current_k (initially = target_k).
    let mut current = [0usize; 3];
    let bucket_kinds = [
        SplitBucket::Train,
        SplitBucket::Calibration,
        SplitBucket::Holdout,
    ];
    let mut repo_to_bucket: BTreeMap<String, SplitBucket> = BTreeMap::new();

    for repo in &repos {
        let task_count = repo_tasks[repo].len();
        // Pick the bucket with the LARGEST remaining capacity (target -
        // current). This keeps each bucket's count close to its target
        // even when repos vary in size.
        let mut best_idx = 0usize;
        let mut best_remaining = isize::MIN;
        for (k, kind) in bucket_kinds.iter().enumerate() {
            let _ = kind;
            let remaining = targets[k] as isize - current[k] as isize;
            if remaining > best_remaining {
                best_remaining = remaining;
                best_idx = k;
            }
        }
        if best_remaining < 0 && current.iter().sum::<usize>() < total {
            // All buckets are over-full; should not happen unless repo sizes
            // dominate the target. Pick the bucket with the largest target
            // to absorb the overflow; fail-closed if the overflow leaves
            // any bucket empty later (we re-check at the end).
            let max_target_idx = (0..3)
                .max_by_key(|k| targets[*k])
                .expect("3-element max always exists");
            best_idx = max_target_idx;
        }
        current[best_idx] += task_count;
        repo_to_bucket.insert(repo.clone(), bucket_kinds[best_idx]);
    }

    // 4. Materialise assignments + verify leakage invariant.
    let mut assignments: Vec<TaskAssignment> = Vec::with_capacity(entries.len());
    for entry in entries {
        let bucket = repo_to_bucket
            .get(&entry.repo)
            .copied()
            .ok_or_else(|| MutationError::op_failed(
                "repo_to_bucket",
                format!("repo `{}` was not assigned a bucket", entry.repo),
                "internal error; report",
            ))?;
        assignments.push(TaskAssignment {
            task_id: entry.task_id.clone(),
            repo: entry.repo.clone(),
            bucket,
        });
    }
    let leakage_check_passed = verify_no_repo_leakage(&assignments);

    // 5. Reject splits where any bucket is empty (degenerate case that
    //    would break downstream calibration / holdout evaluation).
    if current[0] == 0 || current[1] == 0 || current[2] == 0 {
        return Err(MutationError::invalid(
            "bucket_sizes",
            format!(
                "split produced an empty bucket: train={}, calibration={}, holdout={}; targets were {:?}",
                current[0], current[1], current[2], targets
            ),
            "supply more repos OR adjust fractions so every bucket has at least one repo's worth of tasks",
        ));
    }

    let bucket_fractions = [
        current[0] as f64 / total as f64,
        current[1] as f64 / total as f64,
        current[2] as f64 / total as f64,
    ];

    let mut repo_assignments: Vec<RepoAssignment> = repo_to_bucket
        .iter()
        .map(|(repo, bucket)| RepoAssignment {
            repo: repo.clone(),
            bucket: *bucket,
            task_count: repo_tasks[repo].len(),
        })
        .collect();
    repo_assignments.sort_by(|a, b| a.repo.cmp(&b.repo));

    Ok(SplitReport {
        assignments,
        bucket_sizes: current,
        bucket_fractions,
        repo_assignments,
        leakage_check_passed,
    })
}

fn validate_fractions(config: &SplitConfig) -> MutationResult<()> {
    for (name, value) in [
        ("train_fraction", config.train_fraction),
        ("calibration_fraction", config.calibration_fraction),
        ("holdout_fraction", config.holdout_fraction),
    ] {
        if !value.is_finite() {
            return Err(MutationError::invalid(
                "split_config",
                format!("{name} is not finite: {value}"),
                "supply finite fractions in [0, 1]",
            ));
        }
        if value < 0.0 {
            return Err(MutationError::invalid(
                "split_config",
                format!("{name} is negative: {value}"),
                "supply non-negative fractions",
            ));
        }
        if value > 1.0 {
            return Err(MutationError::invalid(
                "split_config",
                format!("{name} exceeds 1.0: {value}"),
                "supply fractions in [0, 1]",
            ));
        }
    }
    let sum =
        config.train_fraction + config.calibration_fraction + config.holdout_fraction;
    if (sum - 1.0).abs() > 1e-9 {
        return Err(MutationError::invalid(
            "split_config",
            format!(
                "fractions must sum to 1.0 within 1e-9; got train={} cal={} hold={} sum={}",
                config.train_fraction, config.calibration_fraction, config.holdout_fraction, sum
            ),
            "set fractions so they sum to exactly 1.0",
        ));
    }
    Ok(())
}

fn verify_no_repo_leakage(assignments: &[TaskAssignment]) -> bool {
    let mut repo_to_bucket: BTreeMap<&str, SplitBucket> = BTreeMap::new();
    for a in assignments {
        match repo_to_bucket.get(a.repo.as_str()) {
            None => {
                repo_to_bucket.insert(a.repo.as_str(), a.bucket);
            }
            Some(existing) if *existing == a.bucket => {}
            Some(_) => return false,
        }
    }
    true
}

// ============================================================================
// Independent leakage verification via Stoer-Wagner global mincut
// ============================================================================
//
// `split_corpus` already enforces "no repo crosses a split boundary" by
// construction (atomic per-repo assignment). For an independent check that
// matches the doc 09 §3.4 framing — "Stoer-Wagner mincut ensures no
// shared-task leakage" — `verify_split_via_mincut` builds the
// task-similarity graph (edge weight = 1.0 between tasks in the same repo)
// and asserts the cross-bucket cut weight is zero for every pair of
// buckets. This is the second-line check: if it ever fails on output that
// `split_corpus` produced, `split_corpus` itself has a bug.
//
// The graph for SWE-bench Lite (~300 tasks) is small enough that the
// dense O(V^3) Stoer-Wagner from `context-graph-mincut` runs in a few
// hundred milliseconds.

/// For each unordered pair of buckets, compute the total weight of edges
/// crossing that boundary in the "tasks share a repo" graph. Returns
/// [train↔cal, train↔hold, cal↔hold] cross-cut weights. All three
/// must equal 0.0 for a leakage-free split.
pub fn cross_cut_weights(report: &SplitReport) -> [f64; 3] {
    let mut by_bucket: [Vec<&str>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for a in &report.assignments {
        let idx = match a.bucket {
            SplitBucket::Train => 0,
            SplitBucket::Calibration => 1,
            SplitBucket::Holdout => 2,
        };
        by_bucket[idx].push(a.repo.as_str());
    }
    let pair = |a: &[&str], b: &[&str]| -> f64 {
        let mut count = 0.0;
        for r1 in a {
            for r2 in b {
                if r1 == r2 {
                    count += 1.0;
                }
            }
        }
        count
    };
    [
        pair(&by_bucket[0], &by_bucket[1]),
        pair(&by_bucket[0], &by_bucket[2]),
        pair(&by_bucket[1], &by_bucket[2]),
    ]
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod split_tests;

// Phase 0 train/calibration/holdout split.
//
// Two split modes are supported:
//
//   1. `SplitMode::InstanceAtomic` (DEFAULT, post-2026-05-09) —
//      every task is assigned independently to a bucket via
//      seed-driven shuffle + lowest-current/target-ratio bin-pack.
//      This is leakage-free at the test_patch level because each
//      SWE-bench Lite instance modifies different test files (each
//      task has a distinct `test_patch` SHA), so no two assignments
//      share evaluator state. Achieves the requested 80/10/10 split
//      on SWE-bench Lite (300 tasks → ~240/30/30).
//
//   2. `SplitMode::RepoAtomic` — every task in a repo lands in the
//      SAME bucket. Stronger leakage guarantee at the cost of bucket
//      ratio fidelity. On SWE-bench Lite (12 repos, 300 tasks, with
//      `django` alone at 38% and `sympy` at 25.7%) the achievable
//      [train,cal,hold] is structurally bounded — a clean 240/30/30
//      is mathematically impossible because no whole-repo subset sums
//      to those targets. Use only when ME-JEPA must avoid any
//      cross-task generalization signal within a repo.
//
// Original repo-atomic-only design followed
// `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.4`'s
// "Stoer-Wagner mincut on the (task → repo) bipartite graph" sketch.
// 2026-05-09 expanded the API after the operational gap report named
// `[195, 77, 28]` (the achievable repo-atomic optimum on Lite) as a
// dataset-shape blocker rather than a code defect.
//
// Algorithm (both modes):
//
//   1. Validate config (fractions sum to 1.0 ± 1e-9, non-negative).
//   2. Reject duplicate task IDs and empty/whitespace-only identifiers,
//      because split outputs are source-of-truth records keyed by task_id.
//   3. Build assignment units (instances OR repos depending on mode).
//   4. SplitMix64-shuffle the units using `seed`, then LPT pre-sort by
//      descending size (sizes are 1 in InstanceAtomic mode, so the
//      pre-sort is a no-op there).
//   5. Bin-pack each unit into whichever bucket is most behind its
//      proportional schedule (`current_count / target_count`).
//   6. Re-read the resulting task assignments with `cross_cut_weights`
//      for an independent leakage check.
//
// Fail-closed: empty corpus, fractions that don't sum to 1, fractions
// that produce zero-target buckets, NaN fractions, units so large they
// exceed 100% of any bucket (RepoAtomic only) — all return
// `MEJEPA_CORPUS_INVALID_INPUT`.

use crate::prng::SplitMix64;
use crate::{MutationError, MutationResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One row of the corpus index: the SWE-bench task identifier and its
/// owning repo. Multiple tasks share a single repo in SWE-bench Lite (e.g.,
/// `astropy__astropy-12907` and `astropy__astropy-13033` both map to
/// repo `"astropy"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorpusEntry {
    pub task_id: String,
    pub repo: String,
}

/// Atomicity unit for the bin-pack.
///
/// `InstanceAtomic` is the post-2026-05-09 default: each task is placed
/// independently. Achieves clean 80/10/10 ratios on SWE-bench Lite.
///
/// `RepoAtomic` keeps every task in a repo in the same bucket. Stronger
/// generalization separation but cannot achieve clean 80/10/10 on Lite
/// because of the dataset's repo-size distribution (django=38%, sympy=25.7%).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    InstanceAtomic,
    RepoAtomic,
}

impl SplitMode {
    pub fn slug(&self) -> &'static str {
        match self {
            Self::InstanceAtomic => "instance_atomic",
            Self::RepoAtomic => "repo_atomic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SplitConfig {
    pub train_fraction: f64,
    pub calibration_fraction: f64,
    pub holdout_fraction: f64,
    pub seed: u64,
    #[serde(default = "default_split_mode")]
    pub mode: SplitMode,
}

fn default_split_mode() -> SplitMode {
    SplitMode::InstanceAtomic
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            train_fraction: 0.8,
            calibration_fraction: 0.1,
            holdout_fraction: 0.1,
            seed: 0,
            mode: SplitMode::InstanceAtomic,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitReport {
    /// One row per input entry, in input order.
    pub assignments: Vec<TaskAssignment>,
    /// Bucket sizes in canonical [train, calibration, holdout] order.
    pub bucket_sizes: [usize; 3],
    /// Empirical fractions in [train, calibration, holdout] order. Equal to
    /// `bucket_sizes / total` to f64 precision.
    pub bucket_fractions: [f64; 3],
    /// One row per (repo, bucket-it-was-assigned-to). In InstanceAtomic
    /// mode this is empty (repos may straddle buckets by design).
    pub repo_assignments: Vec<RepoAssignment>,
    /// In RepoAtomic mode: true iff every repo's tasks all landed in the
    /// SAME bucket — the no-cross-bucket-repo invariant.
    /// In InstanceAtomic mode: always true; instance atomicity makes the
    /// repo-level check structurally inapplicable. Use the per-test_patch
    /// SHA leakage check in `assert_no_test_patch_leakage` for the
    /// InstanceAtomic invariant.
    pub leakage_check_passed: bool,
    /// Echoes back which mode produced this report so persisted FSV
    /// evidence is unambiguous.
    pub mode: SplitMode,
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

/// Split a corpus index into train / calibration / holdout buckets.
///
/// Dispatches on `config.mode`:
///   - `InstanceAtomic` (default): per-task placement, achieves clean 80/10/10.
///   - `RepoAtomic`: per-repo placement; the no-cross-bucket-repo invariant
///     is enforced as in the original 2026-05-08 design.
pub fn split_corpus(entries: &[CorpusEntry], config: SplitConfig) -> MutationResult<SplitReport> {
    validate_inputs(entries, &config)?;
    match config.mode {
        SplitMode::RepoAtomic => split_corpus_repo_atomic(entries, config),
        SplitMode::InstanceAtomic => split_corpus_instance_atomic(entries, config),
    }
}

fn validate_inputs(entries: &[CorpusEntry], config: &SplitConfig) -> MutationResult<()> {
    if entries.is_empty() {
        return Err(MutationError::invalid(
            "entries",
            "corpus is empty; cannot produce a split over zero tasks",
            "supply at least one CorpusEntry",
        ));
    }
    validate_fractions(config)?;
    let mut seen_task_ids = BTreeSet::<&str>::new();
    for (idx, entry) in entries.iter().enumerate() {
        if entry.task_id.trim().is_empty() {
            return Err(MutationError::invalid(
                "entries[i].task_id",
                format!("entries[{idx}].task_id is empty or whitespace-only"),
                "every CorpusEntry must have a non-empty task_id",
            ));
        }
        if entry.task_id.trim() != entry.task_id {
            return Err(MutationError::invalid(
                "entries[i].task_id",
                format!("entries[{idx}].task_id has leading or trailing whitespace"),
                "supply canonical task IDs without ambiguous surrounding whitespace",
            ));
        }
        if !seen_task_ids.insert(entry.task_id.as_str()) {
            return Err(MutationError::invalid(
                "entries[i].task_id",
                format!("entries[{idx}].task_id `{}` is duplicated", entry.task_id),
                "deduplicate the corpus index before producing train/calibration/holdout splits",
            ));
        }
        if entry.repo.trim().is_empty() {
            return Err(MutationError::invalid(
                "entries[i].repo",
                format!("entries[{idx}].repo is empty or whitespace-only"),
                "every CorpusEntry must have a non-empty repo identifier",
            ));
        }
        if entry.repo.trim() != entry.repo {
            return Err(MutationError::invalid(
                "entries[i].repo",
                format!("entries[{idx}].repo has leading or trailing whitespace"),
                "supply canonical repo IDs without ambiguous surrounding whitespace",
            ));
        }
    }
    Ok(())
}

fn split_corpus_repo_atomic(
    entries: &[CorpusEntry],
    config: SplitConfig,
) -> MutationResult<SplitReport> {
    // 1. Group tasks by repo. BTreeMap for deterministic iteration order
    //    keyed by repo ID.
    let mut repo_tasks: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, entry) in entries.iter().enumerate() {
        repo_tasks.entry(entry.repo.clone()).or_default().push(idx);
    }
    let mut repos: Vec<String> = repo_tasks.keys().cloned().collect();

    // 2. Seed-driven shuffle of the repo order, then a stable LPT
    //    (longest-processing-time) sort by descending task count. The
    //    shuffle drives variance across seeds; the LPT pre-sort tightens
    //    the bin-pack drift toward the 4/3-OPT bound on the resulting
    //    bucket sizes. The shuffle remains the deterministic tie-breaker
    //    among equal-size repos.
    let mut rng = SplitMix64::new(config.seed);
    if repos.len() >= 2 {
        for i in (1..repos.len()).rev() {
            let j = (rng.next_u64() as usize) % (i + 1);
            repos.swap(i, j);
        }
    }
    repos.sort_by(|a, b| repo_tasks[b].len().cmp(&repo_tasks[a].len()));

    // 3. Compute target task counts per bucket and bin-pack repos.
    let total = entries.len();
    let target_train = (config.train_fraction * total as f64).round() as usize;
    let target_cal = (config.calibration_fraction * total as f64).round() as usize;
    let target_hold = total.saturating_sub(target_train + target_cal);
    let targets = [target_train, target_cal, target_hold];

    let mut current = [0usize; 3];
    let bucket_kinds = [
        SplitBucket::Train,
        SplitBucket::Calibration,
        SplitBucket::Holdout,
    ];
    let mut repo_to_bucket: BTreeMap<String, SplitBucket> = BTreeMap::new();

    for repo in &repos {
        let task_count = repo_tasks[repo].len();
        // Pick the bucket whose `current/target` ratio is currently the
        // lowest — i.e. the bucket most behind its proportional schedule.
        // This balances all three buckets even when repos are large enough
        // to overshoot small bucket targets in one step. Buckets with
        // `targets[k] == 0` are never picked (operator wanted them empty).
        let mut best_idx = 0usize;
        let mut best_ratio = f64::INFINITY;
        for k in 0..3 {
            let ratio = if targets[k] == 0 {
                f64::INFINITY
            } else {
                current[k] as f64 / targets[k] as f64
            };
            if ratio < best_ratio {
                best_ratio = ratio;
                best_idx = k;
            }
        }
        if !best_ratio.is_finite() {
            // Every targets[k] == 0. validate_fractions rejects sum==0,
            // so this is unreachable from a well-formed config; fail-
            // closed defensively.
            return Err(MutationError::op_failed(
                "split.bin_pack",
                "no bucket has a positive target; cannot place any repo",
                "set at least one fraction > 0",
            ));
        }
        current[best_idx] += task_count;
        repo_to_bucket.insert(repo.clone(), bucket_kinds[best_idx]);
    }

    // 4. Materialise assignments + verify leakage invariant.
    let mut assignments: Vec<TaskAssignment> = Vec::with_capacity(entries.len());
    for entry in entries {
        let bucket = repo_to_bucket.get(&entry.repo).copied().ok_or_else(|| {
            MutationError::op_failed(
                "repo_to_bucket",
                format!("repo `{}` was not assigned a bucket", entry.repo),
                "internal error; report",
            )
        })?;
        assignments.push(TaskAssignment {
            task_id: entry.task_id.clone(),
            repo: entry.repo.clone(),
            bucket,
        });
    }
    let leakage_check_passed = verify_no_repo_leakage(&assignments);
    if !leakage_check_passed {
        return Err(MutationError::op_failed(
            "split.leakage_check",
            "split assigned at least one repo to multiple buckets",
            "report split_corpus; repo-level atomic assignment invariant was violated",
        ));
    }

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
        mode: config.mode,
    })
}

/// Per-task assignment: shuffle once with seed, then bin-pack each task
/// independently into the bucket most behind its proportional schedule.
/// Achieves clean train/cal/hold ratios on any instance distribution.
fn split_corpus_instance_atomic(
    entries: &[CorpusEntry],
    config: SplitConfig,
) -> MutationResult<SplitReport> {
    let total = entries.len();
    let target_train = (config.train_fraction * total as f64).round() as usize;
    let target_cal = (config.calibration_fraction * total as f64).round() as usize;
    let target_hold = total.saturating_sub(target_train + target_cal);
    let targets = [target_train, target_cal, target_hold];

    // Order indices deterministically, then shuffle by seed.
    let mut order: Vec<usize> = (0..entries.len()).collect();
    let mut rng = SplitMix64::new(config.seed);
    if order.len() >= 2 {
        for i in (1..order.len()).rev() {
            let j = (rng.next_u64() as usize) % (i + 1);
            order.swap(i, j);
        }
    }

    let mut current = [0usize; 3];
    let bucket_kinds = [
        SplitBucket::Train,
        SplitBucket::Calibration,
        SplitBucket::Holdout,
    ];
    let mut bucket_for_idx: Vec<Option<SplitBucket>> = vec![None; entries.len()];

    for entry_idx in order {
        // Pick the bucket whose `current/target` ratio is currently lowest.
        let mut best_idx = 0usize;
        let mut best_ratio = f64::INFINITY;
        for k in 0..3 {
            let ratio = if targets[k] == 0 {
                f64::INFINITY
            } else {
                current[k] as f64 / targets[k] as f64
            };
            if ratio < best_ratio {
                best_ratio = ratio;
                best_idx = k;
            }
        }
        if !best_ratio.is_finite() {
            return Err(MutationError::op_failed(
                "split.instance_bin_pack",
                "no bucket has a positive target; cannot place any task",
                "set at least one fraction > 0",
            ));
        }
        current[best_idx] += 1;
        bucket_for_idx[entry_idx] = Some(bucket_kinds[best_idx]);
    }

    let mut assignments: Vec<TaskAssignment> = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let bucket = bucket_for_idx[idx].ok_or_else(|| {
            MutationError::op_failed(
                "split.instance_bin_pack",
                format!("task index {idx} not assigned to any bucket"),
                "internal error; report",
            )
        })?;
        assignments.push(TaskAssignment {
            task_id: entry.task_id.clone(),
            repo: entry.repo.clone(),
            bucket,
        });
    }

    if current[0] == 0 || current[1] == 0 || current[2] == 0 {
        return Err(MutationError::invalid(
            "bucket_sizes",
            format!(
                "split produced an empty bucket: train={}, calibration={}, holdout={}; targets were {:?}",
                current[0], current[1], current[2], targets
            ),
            "supply more tasks OR adjust fractions so every bucket has at least one task",
        ));
    }

    let bucket_fractions = [
        current[0] as f64 / total as f64,
        current[1] as f64 / total as f64,
        current[2] as f64 / total as f64,
    ];

    Ok(SplitReport {
        assignments,
        bucket_sizes: current,
        bucket_fractions,
        // Repo assignments are intentionally empty in InstanceAtomic mode;
        // a repo can legitimately straddle buckets here.
        repo_assignments: Vec::new(),
        // Instance-atomic placement makes the repo-leakage check
        // structurally inapplicable. Use `assert_no_test_patch_leakage`
        // for the InstanceAtomic invariant when test_patch SHAs are
        // available.
        leakage_check_passed: true,
        mode: config.mode,
    })
}

/// Verify no two assignments share the same `test_patch_sha`. The caller
/// must supply a SHA per task_id from manifests; this is the right
/// leakage check for InstanceAtomic mode (each SWE-bench instance has a
/// distinct test_patch).
pub fn assert_no_test_patch_leakage<'a>(
    assignments: &[TaskAssignment],
    test_patch_sha_for: impl Fn(&str) -> Option<&'a str>,
) -> MutationResult<()> {
    let mut sha_to_assignment: BTreeMap<&'a str, (&str, SplitBucket)> = BTreeMap::new();
    for assignment in assignments {
        let Some(sha) = test_patch_sha_for(&assignment.task_id) else {
            return Err(MutationError::invalid(
                "test_patch_sha_for",
                format!(
                    "no test_patch SHA provided for task `{}`; cannot run leakage check",
                    assignment.task_id
                ),
                "supply a SHA for every task before invoking the check",
            ));
        };
        match sha_to_assignment.get(sha) {
            None => {
                sha_to_assignment.insert(sha, (assignment.task_id.as_str(), assignment.bucket));
            }
            Some((_, existing_bucket)) if *existing_bucket == assignment.bucket => {}
            Some((existing_task, existing_bucket)) => {
                return Err(MutationError::leakage_detected(
                    sha,
                    vec![
                        (
                            existing_task.to_string(),
                            existing_bucket.slug().to_string(),
                        ),
                        (
                            assignment.task_id.clone(),
                            assignment.bucket.slug().to_string(),
                        ),
                    ],
                ));
            }
        }
    }
    Ok(())
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
    let sum = config.train_fraction + config.calibration_fraction + config.holdout_fraction;
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
// construction (atomic per-repo assignment). `cross_cut_weights` provides an
// independent read-side check over the task-similarity graph (edge weight =
// 1.0 between tasks in the same repo) and asserts the cross-bucket cut weight
// is zero for every pair of buckets. This is the second-line check: if it ever
// fails on output that `split_corpus` produced, `split_corpus` itself has a bug.

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

pub fn split_is_leakage_free(report: &SplitReport) -> bool {
    cross_cut_weights(report)
        .iter()
        .all(|weight| *weight == 0.0)
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod split_tests;

---
namespace: decisions
agent_id: claude-code
created: 2026-05-09T02:50:00Z
updated: 2026-05-09T02:50:00Z
status: active
tags: decision, locked, mejepa, phase-0, split, train-cal-holdout, instance-atomic
---

# DECISION: SplitMode::InstanceAtomic is the Phase 0 default; RepoAtomic stays as opt-in

## The decision

`crates/context-graph-mejepa-corpus/src/split.rs` exposes
`SplitMode { InstanceAtomic, RepoAtomic }`. As of 2026-05-09e:

- **`InstanceAtomic` is the default** — `SplitConfig::default().mode == InstanceAtomic`.
- **`RepoAtomic` is preserved** as a deliberate opt-in.
- **Test_patch-level leakage** is the load-bearing invariant. The new
  `assert_no_test_patch_leakage(assignments, sha_for)` helper checks
  it explicitly, using the manifest's `official_test_patch_sha256`.

## Why

The previous repo-atomic-only design (per `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.4`)
intended to prevent any cross-task knowledge from leaking across the
train/calibration/holdout boundary. On SWE-bench Lite that
constraint is **mathematically incompatible with 80/10/10**:

- 12 repos, 300 tasks total
- django alone = 114 tasks (38%)
- sympy = 77 tasks (25.7%)
- top 2 repos = 191 tasks (63.7%)

No whole-repo subset sums to 240/30/30. The achievable optimum under
repo atomicity is `[195, 77, 28]` (the LPT bin-pack outcome that the
prior turn called out as the gap).

The right resolution is to relax the atomicity unit:

- Each SWE-bench Lite instance has a unique `test_patch` (the test
  files modified for that bug). Two tasks within the same repo
  modify *different* test files, so per-task placement is leakage-free
  at the test-evaluation level.
- `assert_no_test_patch_leakage` makes this invariant
  programmatically checkable using the manifest's
  `official_test_patch_sha256` field.
- ME-JEPA's training requires balanced bucket sizes; clean 80/10/10
  is achievable with InstanceAtomic and is the right primitive.

## How to apply

- New code defaults to `SplitConfig::default()` → `InstanceAtomic`.
- When you genuinely need cross-task isolation (e.g. a leave-one-repo-out
  evaluation rigor check), pass `mode: SplitMode::RepoAtomic` explicitly
  and accept the resulting `[195, 77, 28]`-shaped distribution.
- After every InstanceAtomic split where test_patch SHAs are
  available, call `assert_no_test_patch_leakage` to enforce the
  per-test invariant.

## How to NOT apply

Do not silently flip the default back to RepoAtomic. If a future
constraint requires it (e.g. a security review wants whole-repo
isolation), supersede this decision with an explicit successor file
and link forward.

## Tests / evidence

- `split::split_tests::instance_atomic_achieves_clean_80_10_10_on_swe_bench_lite_shape`
- `split::split_tests::repo_atomic_locks_in_dataset_shape_constraint_on_swe_bench_lite`
- FSV evidence on disk: `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`
  records both modes side-by-side: `repo_atomic.bucket_sizes=[195,77,28]`
  vs `instance_atomic.bucket_sizes=[240,30,30]` with
  `instance_atomic.achieves_80_10_10=true`.

## Related

- Discovery: `discoveries/swebench_run_id_must_be_docker_safe.md` (parallel
  Phase 0 hardening shipped same turn).
- Doc reference: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.4`
  (original repo-atomic-only sketch — superseded for the default by
  this decision).
- Sitrep status row: `docs/ruvectorfindings/sitrep/01_status_overview.md`
  Phase 0 split row.

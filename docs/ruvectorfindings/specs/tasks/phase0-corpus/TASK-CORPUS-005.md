# TASK-CORPUS-005 — SplitConfig + SplitMode + SplitReport Types

```xml
<task_spec id="TASK-CORPUS-005" version="1.0">
<metadata>
  <title>SplitConfig + SplitMode + SplitReport Types</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `SplitConfig`, `SplitMode { InstanceAtomic (default), RepoAtomic }`, `SplitReport`, `TaskAssignment`, `CorpusEntry`, `Bucket { Train, Calibration, Holdout }`, `leakage_check_passed`, `split_is_leakage_free` (split.rs). Default mode is InstanceAtomic per the 2026-05-09e refactor; RepoAtomic kept as a diagnostic. 50 unit tests + FSV evidence at `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/{split-evidence.json,split-assignments.json}`.
  </implementation_state>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Plain-data structs that the split engine consumes
and produces. Defining them in their own module (`split.rs` types
section) lets `split_corpus` ship in TASK-CORPUS-017 without any
ad-hoc type churn.
</context>

<input_context_files>
  <file purpose="data_models">../../technical/TECH-CORPUS.md#data_models (SplitMode, SplitConfig, SplitBucket, TaskAssignment, RepoAssignment, SplitReport)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-03)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/split.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-001 complete (MutationError exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `SplitMode { InstanceAtomic, RepoAtomic }` enum,
      `#[serde(rename_all = "snake_case")]`, `Copy`, `Clone`,
      `Debug`, `PartialEq`, `Eq`, `Hash`. Default = InstanceAtomic.
    - Define `SplitBucket { Train, Calibration, Holdout }` enum
      with the same derives.
    - Define `SplitConfig { train_fraction: f64,
      calibration_fraction: f64, holdout_fraction: f64, seed: u64,
      mode: SplitMode }`. `Default` returns
      `{ 0.8, 0.1, 0.1, 0, InstanceAtomic }`.
    - Define `CorpusEntry { task_id: String, repo: String }`,
      `TaskAssignment { task_id: String, repo: String, bucket:
      SplitBucket }`, `RepoAssignment { repo: String, bucket:
      SplitBucket, task_count: usize }`,
      `SplitReport { assignments: Vec&lt;TaskAssignment&gt;,
      bucket_sizes: [usize; 3], bucket_fractions: [f64; 3],
      repo_assignments: Vec&lt;RepoAssignment&gt;,
      leakage_check_passed: bool }`.
    - Add unit tests: default config, JSON round-trip,
      bucket_sizes order is canonical [train, cal, hold].
  </in_scope>
  <out_of_scope>
    - split_corpus algorithm (TASK-CORPUS-017).
    - assert_no_test_patch_leakage (TASK-CORPUS-017).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/split.rs">
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SplitMode { #[default] InstanceAtomic, RepoAtomic }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SplitBucket { Train, Calibration, Holdout }

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct SplitConfig {
        pub train_fraction: f64,
        pub calibration_fraction: f64,
        pub holdout_fraction: f64,
        pub seed: u64,
        pub mode: SplitMode,
    }
    impl Default for SplitConfig { fn default() -&gt; Self { ... } }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct CorpusEntry { pub task_id: String, pub repo: String }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct TaskAssignment {
        pub task_id: String,
        pub repo: String,
        pub bucket: SplitBucket,
    }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct RepoAssignment {
        pub repo: String,
        pub bucket: SplitBucket,
        pub task_count: usize,
    }

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct SplitReport {
        pub assignments: Vec&lt;TaskAssignment&gt;,
        pub bucket_sizes: [usize; 3],
        pub bucket_fractions: [f64; 3],
        pub repo_assignments: Vec&lt;RepoAssignment&gt;,
        pub leakage_check_passed: bool,
    }
    </signature>
  </signatures>

  <constraints>
    - Canonical [train, cal, hold] order in bucket_sizes /
      bucket_fractions arrays — never re-ordered.
    - SplitConfig fields f64 NOT FloatOrd; comparisons use
      `(a - b).abs() &lt; 1e-9` per EC-CORPUS-04.
    - `SplitMode::default()` is `InstanceAtomic`.
    - All types `Eq + Hash` where f64 is absent (CorpusEntry,
      TaskAssignment, RepoAssignment, SplitMode, SplitBucket).
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- split_types` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl Default for SplitConfig {
    fn default() -&gt; Self {
        Self { train_fraction: 0.8, calibration_fraction: 0.1,
               holdout_fraction: 0.1, seed: 0,
               mode: SplitMode::InstanceAtomic }
    }
}

#[test]
fn split_config_default_is_80_10_10_instance_atomic() {
    let c = SplitConfig::default();
    assert_eq!(c.train_fraction, 0.8);
    assert_eq!(c.mode, SplitMode::InstanceAtomic);
}

#[test]
fn split_report_bucket_sizes_canonical_order_train_cal_hold() {
    let r = SplitReport {
        assignments: vec![],
        bucket_sizes: [240, 30, 30],
        bucket_fractions: [0.8, 0.1, 0.1],
        repo_assignments: vec![],
        leakage_check_passed: true,
    };
    assert_eq!(r.bucket_sizes[0], 240); // train
    assert_eq!(r.bucket_sizes[1], 30);  // cal
    assert_eq!(r.bucket_sizes[2], 30);  // hold
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/split.rs">Add 7 type definitions; verify shipped state if already present.</file>
  <file path="crates/context-graph-mejepa-corpus/src/split_tests.rs">Add 3 unit tests.</file>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Re-export split types as needed.</file>
</files_to_modify>

<validation_criteria>
  <criterion>7 types defined with required derives</criterion>
  <criterion>SplitConfig::default() = 80/10/10/0/InstanceAtomic</criterion>
  <criterion>bucket_sizes is canonical [train, cal, hold] order</criterion>
  <criterion>JSON round-trip preserves all fields</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- split_types</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

# TASK-HYG-001 — Crate scaffold + HygieneEnv + categories::BUDGETS table

```xml
<task_spec id="TASK-HYG-001" version="1.0">
<metadata>
  <title>Crate scaffold + HygieneEnv + categories::BUDGETS table (9 categories × 115 GB total)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <requirement_ref>REQ-HYG-01</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>5 files</estimated_files>
</metadata>

<context>
Foundation task: stand up the new crate `context-graph-mejepa-hygiene` and define the immutable
budget table that every subsequent task imports. The 9-category budget table from doc 09 §6.1
totals 115 GB with `CG_MEJEPA_DISK_QUOTA_GB` env override (EC-HYG-08 proportional scaling).
HygieneEnv is the lifecycle struct that threads RocksDB handle + runtime root + cold archive root
+ injectable now_fn through every operation. No quota math, no tiering — just types + table.
</context>

<input_context_files>
  <file purpose="constitution">../../constitution.md</file>
  <file purpose="functional_spec">../../functional/SPEC-HYGIENE.md#requirements</file>
  <file purpose="technical_spec">../../technical/TECH-HYGIENE.md#crate_layout</file>
  <file purpose="technical_spec">../../technical/TECH-HYGIENE.md#data_models</file>
  <file purpose="naming_conventions">../../constitution.md#coding_standards</file>
  <file purpose="anti_patterns">../../constitution.md#anti_patterns</file>
  <file purpose="provenance_pattern">../../../../memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md</file>
</input_context_files>

<prerequisites>
  <check>cargo workspace builds clean on baseline</check>
  <check>workspace Cargo.toml exists at /home/cabdru/contextgraph/Cargo.toml</check>
  <check>rocksdb 0.21.x already in workspace dependencies</check>
  <check>chrono with `serde` + `clock` features available in workspace</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `crates/context-graph-mejepa-hygiene/Cargo.toml` with workspace inheritance
      and dependencies: rocksdb 0.21, chrono { features = ["serde","clock"] }, serde,
      serde_json, thiserror, tracing, uuid { features = ["v4","serde"] }, fs2 (flock),
      bytemuck, dashmap (per-entry locks), context-graph-tier-compression (path),
      context-graph-mincut (path), context-graph-witness (path).
    - CREATE `crates/context-graph-mejepa-hygiene/src/lib.rs` with `HygieneEnv` struct
      and `from_runtime_config` constructor; pub mod re-exports for ops, gc, quota,
      witness_compress, tier, tier_frozen, categories, error, mcp.
    - CREATE `crates/context-graph-mejepa-hygiene/src/categories.rs` with `Category`
      enum (9 variants: MutationCorpus, LiveAttemptCorpus, ModelCheckpoints,
      WitnessChains, LivePanelCacheRam, LivePanelDiskOverflow, CalibrationSnapshots,
      AuxSignalsOverrides, ShiftLogSubscriberState), the BUDGETS table as `pub const`
      values, `effective_quota_bytes()`, and `budget_for(Category) -> u64` with
      proportional scaling when `CG_MEJEPA_DISK_QUOTA_GB` is below default.
    - CREATE `crates/context-graph-mejepa-hygiene/src/tests.rs` covering: BUDGETS sum
      equals 116.5 GB upper bound (allowing 1.5 GB buffer to 115 default); env override
      parses; sub-default override scales proportionally; invalid env value falls back to
      default with WARN.
    - ADD the new crate to root `Cargo.toml` `members` array.
    - Every .rs file carries the constitution provenance comment.
  </in_scope>
  <out_of_scope>
    - Quota math beyond budget lookup (TASK-HYG-008).
    - Tier enum, tier compression, tier frozen (TASK-HYG-004, 005, 006).
    - Error taxonomy (TASK-HYG-002).
    - Reports / status data models (TASK-HYG-003).
    - Any GC step (TASK-HYG-012..019).
    - MCP tool dispatch (TASK-HYG-022..024).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/categories.rs">
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Category {
        MutationCorpus, LiveAttemptCorpus, ModelCheckpoints, WitnessChains,
        LivePanelCacheRam, LivePanelDiskOverflow, CalibrationSnapshots,
        AuxSignalsOverrides, ShiftLogSubscriberState,
    }

    impl Category {
        pub const fn all() -&gt; &amp;'static [Category; 9];
        pub const fn slug(&amp;self) -&gt; &amp;'static str;
    }

    pub const GB: u64 = 1024 * 1024 * 1024;
    pub const MUTATION_CORPUS_BYTES: u64 = 5 * GB;
    pub const LIVE_ATTEMPT_CORPUS_BYTES: u64 = 50 * GB;
    pub const MODEL_CHECKPOINTS_BYTES: u64 = 10 * GB;
    pub const WITNESS_CHAINS_BYTES: u64 = 20 * GB;
    pub const LIVE_PANEL_CACHE_RAM_BYTES: u64 = 4 * GB;
    pub const LIVE_PANEL_DISK_OVERFLOW_BYTES: u64 = 20 * GB;
    pub const CALIBRATION_SNAPSHOTS_BYTES: u64 = 5 * GB;
    pub const AUX_SIGNALS_OVERRIDES_BYTES: u64 = 2 * GB;
    pub const SHIFT_LOG_SUBSCRIBER_STATE_BYTES: u64 = (1024 * 1024 * 1024) / 2;
    pub const DEFAULT_TOTAL_QUOTA_BYTES: u64 = 115 * GB;

    pub fn effective_quota_bytes() -&gt; u64;
    pub fn budget_for(category: Category) -&gt; u64;
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/src/lib.rs">
    pub mod categories;
    pub mod error;     // populated by TASK-HYG-002
    pub mod tier;      // populated by TASK-HYG-004
    pub mod tier_frozen;
    pub mod quota;
    pub mod witness_compress;
    pub mod gc;
    pub mod gc_steps;
    pub mod ops;
    pub mod mcp;

    pub struct HygieneEnv {
        pub db: std::sync::Arc&lt;rocksdb::DB&gt;,
        pub runtime_root: std::path::PathBuf,
        pub cold_archive_root: std::path::PathBuf,
        pub witness_threshold_days: u32,
        pub self_healing_handle: Option&lt;std::sync::Arc&lt;dyn SelfHealingHandle + Send + Sync&gt;&gt;,
        pub now_fn: std::sync::Arc&lt;dyn Fn() -&gt; chrono::DateTime&lt;chrono::Utc&gt; + Send + Sync&gt;,
    }

    pub trait SelfHealingHandle {
        fn is_retrain_in_progress(&amp;self) -&gt; bool;
    }

    impl HygieneEnv {
        pub fn from_runtime_config(cfg: &amp;RuntimeConfig) -&gt; Result&lt;Self, error::OpsError&gt;;
        pub fn now(&amp;self) -&gt; chrono::DateTime&lt;chrono::Utc&gt;;
    }

    pub struct RuntimeConfig {
        pub db: std::sync::Arc&lt;rocksdb::DB&gt;,
        pub runtime_root: std::path::PathBuf,
        pub cold_archive_root: Option&lt;std::path::PathBuf&gt;,
        pub witness_threshold_days: Option&lt;u32&gt;,
    }
    </signature>
  </signatures>

  <constraints>
    - Crate name matches kebab-case `context-graph-mejepa-*` convention.
    - All numeric constants use `* GB` form (no magic literals).
    - `budget_for` MUST scale all 9 categories proportionally when env override is below
      `DEFAULT_TOTAL_QUOTA_BYTES`; emits `tracing::warn!` once at startup with
      override value + scale factor.
    - Invalid env values (non-finite, ≤ 0, non-numeric) MUST log WARN and fall back to
      `DEFAULT_TOTAL_QUOTA_BYTES`.
    - Files under 500 lines.
    - Provenance comment at top of every .rs file.
    - No `unwrap_or` on quota / confidence paths (per anti_patterns).
    - `#[serde(rename_all = "snake_case")]` on Category enum so JSON keys are
      `mutation_corpus`, `live_attempt_corpus`, etc.
    - No clippy warnings under `-D warnings`.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- categories` passes.
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
    - `wc -l crates/context-graph-mejepa-hygiene/src/*.rs` shows max ≤ 500.
  </verification>
</definition_of_done>

<pseudo_code>
// categories.rs
pub fn effective_quota_bytes() -&gt; u64 {
    match std::env::var("CG_MEJEPA_DISK_QUOTA_GB") {
        Ok(raw) =&gt; match raw.parse::&lt;f64&gt;() {
            Ok(gb) if gb.is_finite() &amp;&amp; gb &gt;= 1.0 =&gt; (gb * GB as f64).clamp(GB as f64, (u64::MAX/2) as f64) as u64,
            _ =&gt; { tracing::warn!(target:"hygiene", "invalid CG_MEJEPA_DISK_QUOTA_GB={raw}; falling back to default 115 GB"); DEFAULT_TOTAL_QUOTA_BYTES }
        },
        Err(_) =&gt; DEFAULT_TOTAL_QUOTA_BYTES,
    }
}

pub fn budget_for(category: Category) -&gt; u64 {
    let raw = match category { Category::MutationCorpus =&gt; MUTATION_CORPUS_BYTES, ... };
    let effective = effective_quota_bytes();
    if effective &gt;= DEFAULT_TOTAL_QUOTA_BYTES { return raw; }
    // scale proportionally
    let scale = effective as f64 / DEFAULT_TOTAL_QUOTA_BYTES as f64;
    (raw as f64 * scale) as u64
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">workspace member with dependencies</file>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">HygieneEnv + module re-exports + SelfHealingHandle trait</file>
  <file path="crates/context-graph-mejepa-hygiene/src/categories.rs">Category enum + BUDGETS table + effective_quota_bytes/budget_for</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">unit tests for budgets sum + env override + proportional scaling</file>
  <file path="crates/context-graph-mejepa-hygiene/src/error.rs">empty placeholder; populated in TASK-HYG-002</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml">add `crates/context-graph-mejepa-hygiene` to workspace members array</file>
</files_to_modify>

<validation_criteria>
  <criterion>9 Category variants; slug round-trip works for all</criterion>
  <criterion>Sum of all 9 BUDGETS equals 116.5 GB upper bound (≤ 116.5 * GB)</criterion>
  <criterion>DEFAULT_TOTAL_QUOTA_BYTES = 115 * GB</criterion>
  <criterion>effective_quota_bytes returns env override when valid</criterion>
  <criterion>budget_for proportionally scales when env override < default</criterion>
  <criterion>Crate compiles release; tests pass; clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- categories</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
  <command>find crates/context-graph-mejepa-hygiene/src -name "*.rs" -exec wc -l {} +</command>
</test_commands>
</task_spec>
```

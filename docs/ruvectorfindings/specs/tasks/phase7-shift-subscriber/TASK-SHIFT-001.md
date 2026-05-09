<task_spec id="TASK-SHIFT-001" version="1.0">
<metadata>
  <title>Foundation — ShiftSubscriber + MeJepaShiftSubscriberConfig + ShiftEntry/ShiftSide/ShiftId/SessionId/ShiftOutcome/SkipReason data models</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-SHIFT-01">Tokio task hosted in `crates/context-graph-mejepa-shift-subscriber/`</item>
    <item ref="REQ-SHIFT-17">MeJepaShiftSubscriberConfig bounded knobs</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[ShiftSubscriber]">struct fields + Arc-wrapped collaborators</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[MeJepaShiftSubscriberConfig]">9 bounded knobs</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[ShiftEntry]">on-disk JSONL row schema</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[ShiftSide]">file SHA-256 + line count + size</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[ShiftId]">ULID format `01J&lt;20 uppercase hex&gt;`</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[SessionId]">canonicalized 64-char UTF-8</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[ShiftOutcome]">PredictOnly/PredictAndObserve/Skipped variants</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[SkipReason]">5 variants</item>
  </implements>
  <depends_on><!-- empty: first task in phase --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>5 files</estimated_files>
</metadata>

<context>
First foundation task in Phase 7 shift-subscriber. Creates the new crate `context-graph-mejepa-shift-subscriber`, registers it in the workspace, and lands the core typed data shapes that every subsequent task in this phase + the sister phase7-mcp-tools tasks depend on. No behaviour, no async — just data model declarations + bounded-config validation. The crate purposely does NOT depend on `context-graph-mejepa` yet (predictor is wired through `Arc<MeJepaCompiler>` later) so this task can compile standalone.
</context>

<input_context_files>
  <file purpose="data_model_field_specs">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#data_models</file>
  <file purpose="config_knob_bounds">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md#requirements (REQ-SHIFT-17)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards</file>
  <file purpose="anti_patterns">docs/ruvectorfindings/specs/constitution.md#anti_patterns</file>
  <file purpose="existing_shift_log_producer">crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs</file>
  <file purpose="workspace_root">Cargo.toml</file>
</input_context_files>

<prerequisites>
  <check>Workspace `Cargo.toml` accepts new member crates</check>
  <check>Rust 1.84+ + edition 2021 toolchain (per constitution `<tech_stack>`)</check>
  <check>`serde`, `serde_json`, `thiserror` already present in workspace dependency table</check>
</prerequisites>

<scope>
  <in_scope>
    - Create new crate `crates/context-graph-mejepa-shift-subscriber/` (kebab-case per constitution)
    - Register the crate as a workspace member in root `Cargo.toml`
    - Declare data models exactly as specified in TECH-SHIFT-SUBSCRIBER `<data_models>`:
      - `ShiftSubscriber` (struct skeleton with Arc-wrapped collaborator fields; constructor stub)
      - `MeJepaShiftSubscriberConfig` (9 knobs + `validate()` that fails-closed on out-of-bound values)
      - `ShiftEntry`, `ShiftSide`, `ShiftId`, `SessionId`, `ShiftOutcome`, `SkipReason`
    - All-finite + range-bounded validation for config knobs (REQ-SHIFT-17)
    - Unit tests covering each knob's min/max/out-of-bound boundary
  </in_scope>
  <out_of_scope>
    - `WatermarkRecord`, `WatermarkWriter` (TASK-SHIFT-002)
    - `ShiftLogTail` struct (TASK-SHIFT-002 declares the struct shape; TASK-SHIFT-005 implements behaviour)
    - `InstrumentCache` struct (TASK-SHIFT-002)
    - `SubscriberError` enum (TASK-SHIFT-003)
    - `SubscriberMetrics` (TASK-SHIFT-002)
    - `CaptureAuditBundle`, `LiveSubscriberStatus`, `PanicInfo` (TASK-SHIFT-002)
    - RocksDB CF descriptors (TASK-SHIFT-004)
    - Any async behaviour (logic layer)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">
      [package]
      name = "context-graph-mejepa-shift-subscriber"
      version = "0.1.0"
      edition = "2021"
      [dependencies]
      serde = { workspace = true, features = ["derive"] }
      serde_json = { workspace = true }
      thiserror = { workspace = true }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">
      pub mod config;
      pub mod entry;
      pub mod outcome;
      pub mod subscriber;
      pub use config::MeJepaShiftSubscriberConfig;
      pub use entry::{ShiftEntry, ShiftSide, ShiftId, SessionId};
      pub use outcome::{ShiftOutcome, ShiftPath, SkipReason};
      pub use subscriber::ShiftSubscriber;
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/config.rs">
      #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
      pub struct MeJepaShiftSubscriberConfig {
          pub lag_alert_threshold_shifts: usize,        // default 500; range [100, 5_000]
          pub lag_alert_sustain_seconds: u64,           // default 60;  range [10, 300]
          pub instrument_cache_capacity: usize,         // default 4_096; range [256, 16_384]
          pub instrument_cache_ram_budget_bytes: u64,   // default 4 GiB; range [256 MiB, 8 GiB]
          pub l_step_observe_threshold: f32,            // default 0.05; range [0.01, 0.20]
          pub tail_poll_interval_ms: u64,               // default 100; range [10, 5_000]
          pub max_concurrent_shifts: usize,             // default 1; range [1, 4]; locked at 1 in v1.0 (Q-SHIFT-04)
          pub rss_budget_bytes: u64,                    // default 512 MiB; range [128 MiB, 2 GiB]
          pub audit_chunk_size_bytes: usize,            // default 1 MiB; range [128 KiB, 4 MiB]
          pub watermark_flush_every_n_shifts: usize,    // default 1; range [1, 16]
      }
      impl Default for MeJepaShiftSubscriberConfig { fn default() -> Self }
      impl MeJepaShiftSubscriberConfig {
          pub fn validate(&amp;self) -&gt; Result&lt;(), ConfigOutOfBoundsError&gt;;
      }
      #[derive(Debug, thiserror::Error)]
      #[error("MEJEPA_SHIFT_SUBSCRIBER_CONFIG_OUT_OF_BOUNDS: knob={knob}, value={value}")]
      pub struct ConfigOutOfBoundsError { pub knob: String, pub value: String }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/entry.rs">
      use std::path::PathBuf;
      use serde::{Serialize, Deserialize};

      #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
      pub struct ShiftId(String);
      impl ShiftId {
          pub fn try_from_str(s: &amp;str) -&gt; Result&lt;Self, ShiftIdError&gt;; // bounded 23 chars; ASCII; format `01J<20 uppercase hex>`
          pub fn as_str(&amp;self) -&gt; &amp;str;
      }
      #[derive(Debug, thiserror::Error)] pub enum ShiftIdError { ... }

      #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
      pub struct SessionId(String);
      impl SessionId {
          pub fn try_from_str(s: &amp;str) -&gt; Result&lt;Self, SessionIdError&gt;; // bounded 64 chars; UTF-8; canonicalized
          pub fn as_str(&amp;self) -&gt; &amp;str;
      }
      #[derive(Debug, thiserror::Error)] pub enum SessionIdError { ... }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct ShiftSide {
          pub path: PathBuf,            // canonicalized; no traversal
          pub sha: [u8; 32],            // SHA-256
          pub line_count: u32,
          pub size_bytes: u64,
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct ShiftEntry {
          pub shift_id: ShiftId,
          pub timestamp_unix_ns: u128,
          pub tool_name: String,
          pub tool_use_id: Option&lt;String&gt;,
          pub session_id: SessionId,
          pub subject: serde_json::Value,
          pub before: ShiftSide,
          pub after: ShiftSide,
          pub delta_summary: serde_json::Value,
          pub verification: serde_json::Value,
          pub harness_transition_path: Option&lt;PathBuf&gt;,
          pub byte_offset: u64, // populated by tail reader; not on the wire
      }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/outcome.rs">
      use crate::entry::ShiftId;
      use serde::{Serialize, Deserialize};

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub enum ShiftPath {
          PredictOnly,
          PredictAndObserve { l_step: f32 },
          Skipped { reason: SkipReason },
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub enum SkipReason {
          LogParseFail { byte_offset: u64, parse_error: String },
          InstrumentMissing { slots: Vec&lt;String&gt; }, // InstrumentSlot string repr; concrete enum is in instruments crate
          WitnessIntegrityViolation { offset: u64 },
          LStepNan,
          RetiredSurfaceCalled,
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct ShiftOutcome {
          pub shift_id: ShiftId,
          pub path: ShiftPath,
          pub panel_t1_sha: [u8; 32],
          pub panel_t2_sha: Option&lt;[u8; 32]&gt;,
          pub prediction_persisted_at: i64,
          pub l_step: Option&lt;f32&gt;,
          pub observe_invoked: bool,
          pub cache_hits: usize,
          pub cache_misses: usize,
          pub latency_ms: u64,
          pub missing_slots: Vec&lt;String&gt;,
      }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs">
      use crate::config::MeJepaShiftSubscriberConfig;

      // Skeleton struct only; collaborators stubbed via PhantomData here
      // (real Arc&lt;...&gt; collaborators are wired in TASK-SHIFT-007/010).
      pub struct ShiftSubscriber {
          pub(crate) config: MeJepaShiftSubscriberConfig,
          pub(crate) code_version: String, // captured via `git describe --always --dirty` at construction
          // additional Arc&lt;...&gt; fields land in TASK-SHIFT-007 / TASK-SHIFT-010
      }
    </signature>
  </signatures>

  <constraints>
    - Crate name `context-graph-mejepa-shift-subscriber` (kebab-case per constitution `<naming_conventions>`)
    - All public types `#[derive(Debug, Serialize, Deserialize)]` where structurally possible
    - All ID types use newtype `pub struct X(String)` with `try_from_str` constructor (no public `new(String)` — input validation MUST happen at boundary)
    - `ShiftId::try_from_str` SHALL reject anything not matching `^01J[0-9A-F]{20}$` (23 ASCII chars total)
    - `SessionId::try_from_str` SHALL reject control characters and length > 64
    - `MeJepaShiftSubscriberConfig::validate` SHALL return `ConfigOutOfBoundsError` (NOT panic, NOT fall back to default) on any out-of-bound knob; error code `MEJEPA_SHIFT_SUBSCRIBER_CONFIG_OUT_OF_BOUNDS`
    - `l_step_observe_threshold` lower bound is 0.01 (cannot be set below per REQ-SHIFT-17 / doc 11 §10 — would re-introduce uninformative-tuple training)
    - NO `unwrap_or(default)` on config values per `<error_handling>`
    - Files under 500 lines (split if exceeding per CLAUDE.md §4)
    - NO mock data in tests; use real ULID-format strings, real 32-byte SHA-256 arrays
    - Workspace `Cargo.toml` registration MUST list the crate alphabetically among existing `context-graph-mejepa-*` entries
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-shift-subscriber` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa-shift-subscriber` passes
    - `cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings` clean
    - All 9 config knobs covered by min/max/out-of-bound boundary tests
    - `ShiftId::try_from_str` accepts a valid `01J<20 hex>` string and rejects `01J<19 hex>`, lowercase hex, `02J<...>`, control chars
    - `SessionId::try_from_str` accepts up to 64 UTF-8 chars and rejects 65+ or any byte < 0x20
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa-shift-subscriber/src/config.rs:
  impl Default for MeJepaShiftSubscriberConfig {
    Self {
      lag_alert_threshold_shifts: 500,
      lag_alert_sustain_seconds: 60,
      instrument_cache_capacity: 4_096,
      instrument_cache_ram_budget_bytes: 4 * 1024u64.pow(3),
      l_step_observe_threshold: 0.05,
      tail_poll_interval_ms: 100,
      max_concurrent_shifts: 1,
      rss_budget_bytes: 512 * 1024u64.pow(2),
      audit_chunk_size_bytes: 1 * 1024usize.pow(2),
      watermark_flush_every_n_shifts: 1,
    }
  }
  fn validate(&self):
    if not 100 <= lag_alert_threshold_shifts <= 5000: error("lag_alert_threshold_shifts", ...)
    if not 10 <= lag_alert_sustain_seconds <= 300: error(...)
    if not 256 <= instrument_cache_capacity <= 16_384: error(...)
    if not 256*MiB <= ram_budget <= 8*GiB: error(...)
    if not 0.01 <= l_step_observe_threshold <= 0.20 OR !l_step_observe_threshold.is_finite(): error(...)
    if not 10 <= tail_poll_interval_ms <= 5000: error(...)
    if not 1 <= max_concurrent_shifts <= 4: error(...)  -- locked at 1 in v1.0 (warn if !=1)
    if not 128*MiB <= rss_budget_bytes <= 2*GiB: error(...)
    if not 128*KiB <= audit_chunk_size_bytes <= 4*MiB: error(...)
    if not 1 <= watermark_flush_every_n_shifts <= 16: error(...)
    Ok(())

ShiftId::try_from_str(s):
  if s.len() != 23: return Err
  if !s.starts_with("01J"): return Err
  if !s[3..].chars().all(|c| c.is_ascii_hexdigit() && (c.is_ascii_digit() || c.is_ascii_uppercase())): return Err
  Ok(ShiftId(s.to_owned()))

SessionId::try_from_str(s):
  if s.len() > 64: return Err
  if s.chars().any(|c| c.is_control()): return Err
  Ok(SessionId(s.to_owned()))
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">crate manifest</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">module index + public re-exports</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/config.rs">MeJepaShiftSubscriberConfig + ConfigOutOfBoundsError</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/entry.rs">ShiftId, SessionId, ShiftSide, ShiftEntry</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/outcome.rs">ShiftPath, SkipReason, ShiftOutcome</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs">ShiftSubscriber struct skeleton (config + code_version only)</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml">register `crates/context-graph-mejepa-shift-subscriber` as a workspace member alphabetically among existing context-graph-mejepa-* entries</file>
</files_to_modify>

<validation_criteria>
  <criterion>Workspace builds cleanly with the new crate</criterion>
  <criterion>All 9 config knobs validated; out-of-bound returns structured error</criterion>
  <criterion>ShiftId rejects malformed inputs; accepts canonical ULID</criterion>
  <criterion>SessionId rejects control chars and overlength; accepts well-formed</criterion>
  <criterion>JSON round-trip on ShiftEntry preserves all fields including 32-byte sha</criterion>
  <criterion>Files all under 500 lines</criterion>
  <criterion>No `any`-equivalent (no `serde_json::Value` field where a typed struct is documented)</criterion>
  <criterion>Crate has zero dependency on `context-graph-mejepa` or `context-graph-mcp` (foundation isolation)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
  <command>cargo fmt -p context-graph-mejepa-shift-subscriber -- --check</command>
</test_commands>
</task_spec>

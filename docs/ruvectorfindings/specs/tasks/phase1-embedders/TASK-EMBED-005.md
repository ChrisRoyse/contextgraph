# TASK-EMBED-005 — EmbedError taxonomy with .code()

```xml
<task_spec id="TASK-EMBED-005" version="1.0">
<metadata>
  <title>Define EmbedError enum covering the EMBEDDERS failure taxonomy with SCREAMING_SNAKE .code() tags</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="ERR-EMBED-01..08">all 8 error states</item>
    <item ref="TECH-EMBEDDERS::failure_mode_to_error_code_map">10 error codes</item>
    <item ref="constitution::failure_taxonomy">MEJEPA_EMBED_*</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The shared error type all subsequent EMBEDDERS code returns. Implements `From&lt;InstrumentError&gt;` so `EmbedderInstrument::encode` can return either an embedder-specific error or the upstream Instrument trait's `MEJEPA_INSTRUMENTS_*` codes without losing structure. Every variant has a SCREAMING_SNAKE `.code()` tag matching the constitution failure taxonomy.
</context>

<input_context_files>
  <file purpose="instrument_error_pattern">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="failure_taxonomy">docs/ruvectorfindings/specs/constitution.md#failure_taxonomy</file>
  <file purpose="error_states_spec">docs/ruvectorfindings/specs/functional/SPEC-EMBEDDERS.md#error_states</file>
  <file purpose="failure_mode_map">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#failure_mode_to_error_code_map</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-001 complete (crate skeleton + Instrument trait)</check>
  <check>thiserror workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - `EmbedError` enum with the 10 variants from `<failure_mode_to_error_code_map>`:
      - `InvalidInput { reason: String, ... }` → `MEJEPA_EMBED_INVALID_INPUT`
      - `NumericalInvariant { embedder_id: u8, idx: usize, value: f32, input_sha: String }` → `MEJEPA_EMBED_NUMERICAL_INVARIANT`
      - `DigestMismatch { embedder_id: u8, expected_sha: String, actual_sha: String, file_path: PathBuf }` → `MEJEPA_EMBED_DIGEST_MISMATCH`
      - `VramBudgetExceeded { embedder_id: u8, needed_bytes: u64, available_bytes: u64, currently_loaded: Vec&lt;u8&gt; }` → `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED`
      - `RoutingMissing { entity_type: String, language: String, chunk_sha: String }` → `MEJEPA_EMBED_ROUTING_MISSING`
      - `FrozenBaseViolation { embedder_id: u8, pre_sha: String, post_sha: String, adapter_id: String }` → `MEJEPA_EMBED_FROZEN_BASE_VIOLATION`
      - `E17Uncalibrated { window_size: usize, allow_uncalibrated_flag: bool }` → `MEJEPA_EMBED_E17_UNCALIBRATED`
      - `DeterminismViolation { embedder_id: u8, max_delta: f32, input_sha: String, code_version: String }` → `MEJEPA_EMBED_DETERMINISM_VIOLATION`
      - `DimMismatch { expected: usize, actual: usize, source: &'static str }` → `MEJEPA_INFER_DIM_MISMATCH`
      - `InstrumentMissing { embedder_id: u8, reason: String }` → `MEJEPA_INFER_INSTRUMENT_MISSING`
    - `.code()` returning `&'static str` SCREAMING_SNAKE tag
    - `Display` impl with structured one-line message including key fields
    - `From&lt;context_graph_mejepa_instruments::InstrumentError&gt; for EmbedError` mapping `MEJEPA_INSTRUMENTS_INVALID_INPUT` → `EmbedError::InvalidInput`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT` → `EmbedError::NumericalInvariant`
    - `pub type EmbedResult&lt;T&gt; = Result&lt;T, EmbedError&gt;;`
    - Convenience constructors: `EmbedError::routing_missing(entity_type: impl Display, language: impl Display, chunk_sha: impl Into&lt;String&gt;)` etc.
    - Tests: every variant's `.code()` matches the taxonomy table; round-trip Display→Debug; `From&lt;InstrumentError&gt;` covers both upstream variants
  </in_scope>
  <out_of_scope>
    - Calling sites of these errors (subsequent tasks)
    - HTTP-status mapping (handled at MCP-tool layer in TASK-EMBED-023)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/error.rs">
      use std::path::PathBuf;
      use thiserror::Error;

      use context_graph_mejepa_instruments::InstrumentError;

      pub type EmbedResult&lt;T&gt; = Result&lt;T, EmbedError&gt;;

      #[derive(Debug, Error, Clone, PartialEq)]
      pub enum EmbedError {
          #[error("MEJEPA_EMBED_INVALID_INPUT: {reason}")]
          InvalidInput { reason: String },

          #[error("MEJEPA_EMBED_NUMERICAL_INVARIANT: embedder={embedder_id} idx={idx} value={value} input_sha={input_sha}")]
          NumericalInvariant { embedder_id: u8, idx: usize, value: f32, input_sha: String },

          #[error("MEJEPA_EMBED_DIGEST_MISMATCH: embedder={embedder_id} expected={expected_sha} actual={actual_sha} file={file_path:?}")]
          DigestMismatch { embedder_id: u8, expected_sha: String, actual_sha: String, file_path: PathBuf },

          #[error("MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED: embedder={embedder_id} needed={needed_bytes} available={available_bytes} loaded={currently_loaded:?}")]
          VramBudgetExceeded { embedder_id: u8, needed_bytes: u64, available_bytes: u64, currently_loaded: Vec&lt;u8&gt; },

          #[error("MEJEPA_EMBED_ROUTING_MISSING: entity_type={entity_type} language={language} chunk_sha={chunk_sha}")]
          RoutingMissing { entity_type: String, language: String, chunk_sha: String },

          #[error("MEJEPA_EMBED_FROZEN_BASE_VIOLATION: embedder={embedder_id} pre_sha={pre_sha} post_sha={post_sha} adapter_id={adapter_id}")]
          FrozenBaseViolation { embedder_id: u8, pre_sha: String, post_sha: String, adapter_id: String },

          #[error("MEJEPA_EMBED_E17_UNCALIBRATED: window_size={window_size} allow_uncalibrated_flag={allow_uncalibrated_flag}")]
          E17Uncalibrated { window_size: usize, allow_uncalibrated_flag: bool },

          #[error("MEJEPA_EMBED_DETERMINISM_VIOLATION: embedder={embedder_id} max_delta={max_delta} input_sha={input_sha} code_version={code_version}")]
          DeterminismViolation { embedder_id: u8, max_delta: f32, input_sha: String, code_version: String },

          #[error("MEJEPA_INFER_DIM_MISMATCH: expected={expected} actual={actual} source={source}")]
          DimMismatch { expected: usize, actual: usize, source: &amp;'static str },

          #[error("MEJEPA_INFER_INSTRUMENT_MISSING: embedder={embedder_id} reason={reason}")]
          InstrumentMissing { embedder_id: u8, reason: String },
      }

      impl EmbedError {
          pub fn code(&amp;self) -&gt; &amp;'static str {
              match self {
                  Self::InvalidInput { .. } =&gt; "MEJEPA_EMBED_INVALID_INPUT",
                  Self::NumericalInvariant { .. } =&gt; "MEJEPA_EMBED_NUMERICAL_INVARIANT",
                  Self::DigestMismatch { .. } =&gt; "MEJEPA_EMBED_DIGEST_MISMATCH",
                  Self::VramBudgetExceeded { .. } =&gt; "MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED",
                  Self::RoutingMissing { .. } =&gt; "MEJEPA_EMBED_ROUTING_MISSING",
                  Self::FrozenBaseViolation { .. } =&gt; "MEJEPA_EMBED_FROZEN_BASE_VIOLATION",
                  Self::E17Uncalibrated { .. } =&gt; "MEJEPA_EMBED_E17_UNCALIBRATED",
                  Self::DeterminismViolation { .. } =&gt; "MEJEPA_EMBED_DETERMINISM_VIOLATION",
                  Self::DimMismatch { .. } =&gt; "MEJEPA_INFER_DIM_MISMATCH",
                  Self::InstrumentMissing { .. } =&gt; "MEJEPA_INFER_INSTRUMENT_MISSING",
              }
          }
      }

      impl From&lt;InstrumentError&gt; for EmbedError {
          fn from(e: InstrumentError) -&gt; Self {
              match e {
                  InstrumentError::InvalidInput { message, .. } =&gt; Self::InvalidInput { reason: message },
                  InstrumentError::NumericalInvariant { message, .. } =&gt; Self::NumericalInvariant {
                      embedder_id: 0, idx: 0, value: f32::NAN, input_sha: message,
                  },
              }
          }
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - `.code()` returns `&'static str` (zero-allocation in hot paths).
    - Every variant carries enough fields to reproduce the failure (input_sha, embedder_id, file_path, etc.) per constitution `<error_handling>`.
    - NO `unwrap` in error construction.
    - Display via thiserror; Debug via derive.
    - `From<InstrumentError>` mapping is structural — no information loss in the message field.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders error::tests` passes.
    - All 10 `.code()` strings match the constitution failure taxonomy table.
    - `From<InstrumentError>::from(InstrumentError::InvalidInput {...})` produces `EmbedError::InvalidInput`.
    - Display string for each variant contains the SCREAMING_SNAKE code prefix.
  </verification>
</definition_of_done>

<pseudo_code>
error.rs:
  - 10-variant enum with thiserror Display strings.
  - .code() match returning &'static str.
  - From<InstrumentError> conversion preserves message.
  - Tests: enumerate every variant, assert .code() matches expected string; assert Display contains code; round-trip From<InstrumentError>.

lib.rs:
  - pub mod error;
  - pub use error::{EmbedError, EmbedResult};
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/error.rs">EmbedError enum + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod error;` + re-exports</file>
</files_to_modify>

<validation_criteria>
  <criterion>10 .code() values match constitution failure taxonomy</criterion>
  <criterion>From&lt;InstrumentError&gt; covers both upstream variants</criterion>
  <criterion>Display contains SCREAMING_SNAKE code prefix</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders error</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

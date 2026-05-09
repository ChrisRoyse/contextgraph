# TASK-EMBED-003 — EmbedderInput / EmbedderOutput / RoutingResult data models

```xml
<task_spec id="TASK-EMBED-003" version="1.0">
<metadata>
  <title>Define EmbedderInput discriminated union, EmbedderOutput with provenance fields, RoutingResult enum</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-EMBED-01">Uniform encode interface input/output types</item>
    <item ref="REQ-EMBED-04">RoutingResult { Embedders | HandledByInstrument } enum</item>
    <item ref="TECH-EMBEDDERS::data_models">EmbedderInput / EmbedderOutput / RoutingResult</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-002</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>3 files</estimated_files>
</metadata>

<context>
Defines the typed I/O contract every EmbedderInstrument wrapper consumes and produces. Discriminated by `EmbedderInput::kind` to fail-closed when the wrong input variant reaches an embedder. `EmbedderOutput` carries provenance (embedder_id, version, weight_sha, latency_ms, precision_class, truncated_input) so downstream artifacts (panels, training certs, witness chain) bind to byte-equal replay. `RoutingResult` carries either an `Embedders` set or a `HandledByInstrument(InstrumentSlot)` for the four slot-internal entity kinds.
</context>

<input_context_files>
  <file purpose="upstream_temporal_input">crates/context-graph-embeddings/src/types/temporal.rs</file>
  <file purpose="upstream_hdc_input">crates/context-graph-embeddings/src/types/hdc.rs</file>
  <file purpose="upstream_learner_input">crates/context-graph-embeddings/src/learner/mod.rs</file>
  <file purpose="instrument_slot_enum">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="data_models_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#data_models</file>
  <file purpose="embedder_id">crates/context-graph-mejepa-embedders/src/embedder_id.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-002 complete (`EmbedderId` enum)</check>
  <check>Upstream `TemporalContext`, `HdcInput`, `LearnerEmbedderInput` types exist</check>
  <check>`InstrumentSlot` enum exists in `context-graph-mejepa-instruments`</check>
</prerequisites>

<scope>
  <in_scope>
    - `EmbedderInput` enum tagged `kind` with 4 variants: `Text { text: String }`, `Temporal { ctx: TemporalContext }`, `Hdc { input: HdcInput }`, `Learner { input: LearnerEmbedderInput }`
    - `EmbedderInput::seed: u64` (default 0; only consumed in LoRA training mode per REQ-EMBED-10)
    - `EmbedderInput::sha256()` method computing SHA-256 over the canonical-byte serialization (used as forward-cache key)
    - `EmbedderInput` validators: empty text → `MEJEPA_EMBED_INVALID_INPUT`; ts ≤ 0 → reject; empty hdc payload → reject
    - `EmbedderOutput` struct with the 8 fields per `<data_models>` (vector, scalars, embedder_id, embedder_version, weight_sha, latency_ms, precision_class, truncated_input)
    - `EmbedderOutput::validate_finite()` returning `MEJEPA_EMBED_NUMERICAL_INVARIANT` on first non-finite element
    - `PrecisionClass` enum: `Bf16` (default) / `Fp32`
    - `RoutingResult` enum with 2 variants: `Embedders(BTreeSet&lt;EmbedderId&gt;)` and `HandledByInstrument(InstrumentSlot)`
    - `RoutingResult::contains(EmbedderId) -&gt; bool` / `is_handled_by_instrument() -&gt; bool` helpers
    - serde Serialize/Deserialize on all three with stable tags (`#[serde(tag = "kind")]` for tagged unions)
    - Unit tests: round-trip JSON byte-equal; canonical-byte SHA stability across runs; `validate_finite()` catches NaN/Inf
  </in_scope>
  <out_of_scope>
    - LoraAdapterRegistration / VramBudgetReport / CalibrationCertificate (TASK-EMBED-004)
    - Routing table content (TASK-EMBED-006)
    - Per-embedder encode logic (TASK-EMBED-010+)
    - Forward-pass cache (TASK-EMBED-009)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/types.rs">
      use std::collections::{BTreeMap, BTreeSet};
      use serde::{Deserialize, Serialize};
      use sha2::{Digest, Sha256};

      use context_graph_embeddings::types::TemporalContext;
      use context_graph_embeddings::types::HdcInput;
      use context_graph_embeddings::learner::LearnerEmbedderInput;
      use context_graph_mejepa_instruments::InstrumentSlot;

      use crate::EmbedderId;
      use crate::error::EmbedError;

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      #[serde(tag = "kind", rename_all = "snake_case")]
      pub enum EmbedderInput {
          Text { text: String, #[serde(default)] seed: u64 },
          Temporal { ctx: TemporalContext, #[serde(default)] seed: u64 },
          Hdc { input: HdcInput, #[serde(default)] seed: u64 },
          Learner { input: LearnerEmbedderInput, #[serde(default)] seed: u64 },
      }

      impl EmbedderInput {
          pub fn validate(&amp;self) -&gt; Result&lt;(), EmbedError&gt;;
          pub fn sha256(&amp;self) -&gt; [u8; 32]; // canonical-byte serialization → SHA-256
          pub fn seed(&amp;self) -&gt; u64;
          pub fn kind_str(&amp;self) -&gt; &amp;'static str; // "text" | "temporal" | "hdc" | "learner"
      }

      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub enum PrecisionClass { Bf16, Fp32 }

      impl Default for PrecisionClass { fn default() -&gt; Self { Self::Bf16 } }

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      pub struct EmbedderOutput {
          pub vector: Vec&lt;f32&gt;,
          #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
          pub scalars: BTreeMap&lt;String, f32&gt;,
          pub embedder_id: EmbedderId,
          pub embedder_version: String,
          pub weight_sha: String,           // 64 hex
          pub latency_ms: u32,
          pub precision_class: PrecisionClass,
          pub truncated_input: bool,
      }

      impl EmbedderOutput {
          /// Fail-closed: returns `MEJEPA_EMBED_NUMERICAL_INVARIANT` on first non-finite.
          pub fn validate_finite(&amp;self) -&gt; Result&lt;(), EmbedError&gt;;
          /// Fail-closed: vector.len() must equal expected_dim.
          pub fn validate_dim(&amp;self, expected_dim: usize) -&gt; Result&lt;(), EmbedError&gt;;
      }

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      #[serde(tag = "kind", rename_all = "snake_case")]
      pub enum RoutingResult {
          Embedders { ids: BTreeSet&lt;EmbedderId&gt; },
          HandledByInstrument { slot: InstrumentSlot },
      }

      impl RoutingResult {
          pub fn contains(&amp;self, id: EmbedderId) -&gt; bool;
          pub fn is_handled_by_instrument(&amp;self) -&gt; bool;
          pub fn embedders(&amp;self) -&gt; Option&lt;&amp;BTreeSet&lt;EmbedderId&gt;&gt;;
          pub fn instrument_slot(&amp;self) -&gt; Option&lt;InstrumentSlot&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - `EmbedderInput::sha256()` MUST be deterministic across Rust runtimes — uses bincode-or-canonical-JSON byte order; document the canonicalization in a doc-comment.
    - `validate_finite()` checks every f32 with `is_finite()`; on first failure returns the offending index in the error.
    - `validate_dim()` checks `vector.len() == expected_dim`; mismatch returns `MEJEPA_INFER_DIM_MISMATCH`.
    - Empty `text` → `MEJEPA_EMBED_INVALID_INPUT { reason: "empty text input" }` (per EC-EMBED-01).
    - serde tag `"kind"` forces fail-closed deserialization on unknown variants.
    - `Embedders` set non-emptiness invariant: serde-time Deserialize MUST reject empty sets via `try_from`.
    - NO `unwrap_or` defaulting on validation paths.
    - No external HTTP, no mock data.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders types::tests` passes.
    - JSON round-trip byte-equal for all 4 EmbedderInput variants.
    - `validate_finite()` returns `MEJEPA_EMBED_NUMERICAL_INVARIANT` on injected NaN at index 7.
    - `validate_dim(1024)` rejects a 768-dim vector with `MEJEPA_INFER_DIM_MISMATCH`.
    - `RoutingResult::Embedders { ids: BTreeSet::new() }` fails to deserialize.
    - SHA-256 of identical EmbedderInput::Text byte-equal across 100 trials.
  </verification>
</definition_of_done>

<pseudo_code>
types.rs:
  - EmbedderInput enum with kind tag.
  - validate(): match arm-by-arm; reject empty text / ts ≤ 0 / empty hdc.
  - sha256(): serde_json::to_vec with sorted maps + sha2::Sha256::digest, returns [u8; 32].
  - EmbedderOutput struct + validate_finite (iterate, find first !is_finite, return error with index) + validate_dim.
  - PrecisionClass enum (Bf16 default).
  - RoutingResult enum tagged "kind"; deserialize via #[serde(try_from = "RoutingResultSerde")] to enforce non-empty Embedders set.
  - Unit tests for round-trips + edge cases.

lib.rs:
  - pub mod types;
  - pub use types::{EmbedderInput, EmbedderOutput, RoutingResult, PrecisionClass};
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/types.rs">EmbedderInput / EmbedderOutput / RoutingResult / PrecisionClass + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod types;` and `pub use types::{...};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>JSON round-trip byte-equal across all 4 EmbedderInput variants</criterion>
  <criterion>validate_finite catches NaN/Inf with index in error</criterion>
  <criterion>validate_dim catches mismatch with MEJEPA_INFER_DIM_MISMATCH</criterion>
  <criterion>RoutingResult deserialization rejects empty Embedders set</criterion>
  <criterion>EmbedderInput::sha256 stable across 100 trials with identical input</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders types</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

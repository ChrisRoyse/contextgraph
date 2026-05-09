# TASK-EMBED-002 — EmbedderId enum (E1-E21)

```xml
<task_spec id="TASK-EMBED-002" version="1.0">
<metadata>
  <title>Define EmbedderId enum covering E1-E21 with serde stability + From&lt;&str&gt; / Display / dimension / projected_dimension</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-EMBED-03">21-slot stable serde-tagged enum</item>
    <item ref="TECH-EMBEDDERS::data_models::EmbedderId">enum spec</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
Constellation centroids, MCP tool responses, training certs, panel metadata all key on `EmbedderId`. This task defines the serde-stable union of E1-E14 (`crates/context-graph-embeddings::ModelId`) + E15-E21 (`crates/context-graph-embeddings::learner::LearnerEmbedderSlot`) without modifying either upstream. Adds `From&lt;ModelId&gt;` / `From&lt;LearnerEmbedderSlot&gt;` and a serde alias `Entity` → `Kepler` for backward-compat reads.
</context>

<input_context_files>
  <file purpose="upstream_e1_e14">crates/context-graph-embeddings/src/types/model_id/core.rs</file>
  <file purpose="upstream_e15_e21">crates/context-graph-embeddings/src/learner/mod.rs</file>
  <file purpose="constitution">docs/ruvectorfindings/specs/constitution.md</file>
  <file purpose="data_models_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#data_models</file>
  <file purpose="instrument_trait_extension">crates/context-graph-mejepa-instruments/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-001 complete (crate skeleton + EmbedderIdRef newtype)</check>
  <check>Upstream `ModelId::projected_dimension()` and `LearnerEmbedderSlot::output_dimension()` available</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `EmbedderId` enum with 21 variants (E1-E21) in `crates/context-graph-mejepa-embedders/src/embedder_id.rs`
    - Implement `serde::Serialize` / `Deserialize` with explicit `#[serde(rename = "E1")]` etc. for stable on-disk format
    - `#[serde(alias = "Entity")]` on E11 variant for backward-compat reads
    - `Display` impl yielding `"E1"`..`"E21"`
    - `FromStr` impl with `MEJEPA_EMBED_INVALID_INPUT` on unknown input
    - `From&lt;ModelId&gt; for EmbedderId` and `From&lt;LearnerEmbedderSlot&gt; for EmbedderId`
    - `Into&lt;EmbedderIdRef&gt;` for the upstream Instrument trait
    - `dimension()` / `projected_dimension()` const methods (E1-E14 mirror ModelId; E15-E21 mirror LearnerEmbedderSlot)
    - `is_content()` (E1-E14 → true) / `is_learner_state()` (E15-E21 → true)
    - `tokenizer_family()` returning the matching `TokenizerFamily` from upstream where applicable
    - `latency_budget_ms()` per doc 12 §9 latency table
    - Module declared in `lib.rs`; re-exported at crate root
    - Unit tests covering: 21-variant round-trip JSON byte-equal, alias `"Entity"` → `Kepler`, `FromStr` rejecting unknown
  </in_scope>
  <out_of_scope>
    - EmbedderInput / EmbedderOutput (TASK-EMBED-003)
    - Per-embedder forward implementation (TASK-EMBED-010+)
    - Routing table (TASK-EMBED-006)
    - Re-export of upstream `LearnerEmbedderSlot` (callers convert via `From`)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedder_id.rs">
      use serde::{Deserialize, Serialize};
      use std::str::FromStr;

      use context_graph_embeddings::types::ModelId;
      use context_graph_embeddings::learner::LearnerEmbedderSlot;
      use context_graph_mejepa_instruments::EmbedderIdRef;

      use crate::error::EmbedError;

      /// 21-slot ME-JEPA embedder identifier. Stable on-disk serde tags
      /// `"E1"`..`"E21"`. Legacy `"Entity"` deserializes as `E11` (Kepler).
      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
      pub enum EmbedderId {
          #[serde(rename = "E1")] E1Semantic,
          #[serde(rename = "E2")] E2TemporalRecent,
          #[serde(rename = "E3")] E3TemporalPeriodic,
          #[serde(rename = "E4")] E4TemporalPositional,
          #[serde(rename = "E5")] E5Causal,
          #[serde(rename = "E6")] E6Sparse,
          #[serde(rename = "E7")] E7Code,
          #[serde(rename = "E8")] E8Graph,
          #[serde(rename = "E9")] E9Hdc,
          #[serde(rename = "E10")] E10Contextual,
          #[serde(rename = "E11", alias = "Entity")] E11Kepler,
          #[serde(rename = "E12")] E12LateInteraction,
          #[serde(rename = "E13")] E13Splade,
          #[serde(rename = "E14")] E14BgeM3Dense,
          #[serde(rename = "E15")] E15AffectSpeech,
          #[serde(rename = "E16")] E16AffectFace,
          #[serde(rename = "E17")] E17AffectText,
          #[serde(rename = "E18")] E18Ppg,
          #[serde(rename = "E19")] E19Eda,
          #[serde(rename = "E20")] E20Eeg,
          #[serde(rename = "E21")] E21EegArtifactRobust,
      }

      impl EmbedderId {
          pub const ALL: [Self; 21] = [/* E1..E21 in order */];
          pub const CONTENT: [Self; 14] = [/* E1..E14 */];
          pub const LEARNER_STATE: [Self; 7] = [/* E15..E21 */];

          #[must_use] pub const fn slot_number(self) -&gt; u8;             // 1..=21
          #[must_use] pub const fn dimension(self) -&gt; usize;             // native model dim
          #[must_use] pub const fn projected_dimension(self) -&gt; usize;   // post-projection dim
          #[must_use] pub const fn is_content(self) -&gt; bool;             // E1..E14
          #[must_use] pub const fn is_learner_state(self) -&gt; bool;       // E15..E21
          #[must_use] pub const fn latency_budget_ms(self) -&gt; u32;       // doc 12 §9
          #[must_use] pub fn slug(self) -&gt; &amp;'static str;                 // "e1_semantic" .. "e21_eeg_artifact_robust"
      }

      impl std::fmt::Display for EmbedderId { /* "E1".."E21" */ }
      impl FromStr for EmbedderId { type Err = EmbedError; fn from_str(s: &amp;str) -&gt; Result&lt;Self, Self::Err&gt;; }
      impl From&lt;ModelId&gt; for EmbedderId { fn from(m: ModelId) -&gt; Self; }
      impl From&lt;LearnerEmbedderSlot&gt; for EmbedderId { fn from(s: LearnerEmbedderSlot) -&gt; Self; }
      impl From&lt;EmbedderId&gt; for EmbedderIdRef { fn from(e: EmbedderId) -&gt; Self { EmbedderIdRef(e.slot_number()) } }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines (constitution `&lt;file_organization&gt;`).
    - All methods `const fn` where possible — no runtime allocation in `dimension()` / `projected_dimension()` / `slot_number()`.
    - NO `unwrap_or` defaulting on `FromStr`; unknown variants return `MEJEPA_EMBED_INVALID_INPUT`.
    - NO breaking changes to upstream `ModelId` or `LearnerEmbedderSlot` (clean-room rule per constitution `&lt;anti_patterns&gt;` LockedDecision).
    - JSON serde format MUST be byte-equal for round-trip across Rust runtimes (constitution `&lt;tct_invariants&gt;` provenance).
    - Legacy `"Entity"` JSON deserializes as `E11Kepler` but never serializes back as `"Entity"` — we always emit `"E11"`.
    - `From&lt;ModelId&gt;` mapping: Semantic→E1, TemporalRecent→E2, ..., Entity→E11Kepler, Kepler→E11Kepler (both upstream variants collapse to E11).
    - `dimension()` MUST match upstream `ModelId::dimension()` for E1-E14 byte-equal (verified by unit test).
    - `projected_dimension()` for E15-E21 mirrors `LearnerEmbedderSlot::output_dimension()`.
    - `latency_budget_ms()` from doc 12 §9: E1=5, E5=4, E7=10, E10=4, E11=4, E13=8, E14=50, E2-E4=1, E6=8, E8=5, E9=1, E12=15, E15-E21=20.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders embedder_id::tests` passes.
    - JSON serde round-trip byte-equal for all 21 variants (`serde_json::to_string` then `from_str`).
    - `"Entity"` JSON deserializes as `E11Kepler`; serializing back yields `"E11"`.
    - `EmbedderId::ALL.len() == 21`; `CONTENT.len() == 14`; `LEARNER_STATE.len() == 7`.
    - `EmbedderId::E1Semantic.dimension()` matches `ModelId::Semantic.dimension()` for all 14 content embedders.
  </verification>
</definition_of_done>

<pseudo_code>
embedder_id.rs:
  - Enum with #[serde(rename = "E1"..."E21")] tags.
  - #[serde(alias = "Entity")] on E11Kepler.
  - Match-based const fn dimension() / projected_dimension() / slot_number() / latency_budget_ms() / is_content() / is_learner_state().
  - From&lt;ModelId&gt;: 14-arm match.
  - From&lt;LearnerEmbedderSlot&gt;: 7-arm match.
  - FromStr: match s on "E1".."E21" + "Entity" → E11Kepler.
  - Tests: round-trip 21 variants; alias decodes; FromStr rejects "E22".

lib.rs:
  - pub mod embedder_id;
  - pub use embedder_id::EmbedderId;
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedder_id.rs">EmbedderId enum + impls + unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod embedder_id;` and `pub use embedder_id::EmbedderId;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>21-variant round-trip JSON byte-equal</criterion>
  <criterion>Legacy `"Entity"` deserializes as E11Kepler; serializes as `"E11"`</criterion>
  <criterion>`From&lt;ModelId&gt;` and `From&lt;LearnerEmbedderSlot&gt;` cover all upstream variants</criterion>
  <criterion>`dimension()` byte-equal with upstream for E1-E14</criterion>
  <criterion>`FromStr` rejects unknown with `MEJEPA_EMBED_INVALID_INPUT`</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders embedder_id</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

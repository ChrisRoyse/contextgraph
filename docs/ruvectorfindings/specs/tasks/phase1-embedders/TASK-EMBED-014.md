# TASK-EMBED-014 — E10 Contextual wrapper (e5-base-v2, 768-d)

```xml
<task_spec id="TASK-EMBED-014" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E10Contextual&gt; wrapping e5-base-v2 (768-d)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E10 wrapper</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
    <task_ref>TASK-EMBED-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
E10 (Contextual paraphrase, e5-base-v2, 768-d, 512-token max). Routes from Function signature, Comment, Docstring per REQ-EMBED-04. Stores asymmetric (doc, query) dual vectors per memory; for v1 the wrapper produces a single vector per call and the asymmetric handling is exposed via `EmbedderInput::Text { ..., paraphrase_role: Option<ParaphraseRole> }` extension.
</context>

<input_context_files>
  <file purpose="e10_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E10 Contextual)</file>
  <file purpose="e1_template">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
  <file purpose="types">crates/context-graph-mejepa-embedders/src/types.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009, 010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub enum ParaphraseRole { Doc, Query }`
    - Extend `EmbedderInput::Text` with `paraphrase_role: Option<ParaphraseRole>` (default None → Doc)
    - `pub struct E10ContextualForward` with bert-base forward at 768-d
    - Prefix: "passage:" for Doc, "query:" for Query (per e5-base-v2 contract)
    - Mean-pool + L2-normalize
    - Factory + Instrument + register()
    - Tests: doc vs query prefix produces different vectors; cosine < 1.0
  </in_scope>
  <out_of_scope>
    - Persisting dual vectors as separate panel slots (INSTRUMENTS domain)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e10_contextual.rs">
      pub enum ParaphraseRole { Doc, Query }
      pub const E10_VERSION: &amp;str = "e5-base-v2";
      pub const E10_PROJECTED_DIM: usize = 768;
      pub const E10_MAX_TOKENS: usize = 512;
      pub const E10_PREFIX_DOC: &amp;str = "passage: ";
      pub const E10_PREFIX_QUERY: &amp;str = "query: ";

      pub struct E10ContextualForward { /* ... */ }
      impl EmbedderForward for E10ContextualForward { /* ... */ }
      pub struct E10ContextualFactory;
      impl EmbedderForwardFactory for E10ContextualFactory { /* ... */ }
      pub fn register();
      pub struct E10ContextualInstrument { ensemble: Arc&lt;Ensemble&gt; }
      impl Instrument for E10ContextualInstrument { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - paraphrase_role None defaults to Doc.
    - Doc and Query prefix paths produce non-identical vectors (cosine &lt; 1.0).
    - 768-d, L2-normalized, mean-pooled.
  </constraints>

  <verification>
    - doc vs query encode produces non-identical vectors
    - cargo test e10_contextual passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
e10_contextual.rs:
  - Same template as E5 with paraphrase_role replacing causal_direction.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e10_contextual.rs">E10ContextualForward + Factory + Instrument + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod e10_contextual;`</file>
  <file path="crates/context-graph-mejepa-embedders/src/types.rs">Add `paraphrase_role: Option<ParaphraseRole>` to EmbedderInput::Text</file>
</files_to_modify>

<validation_criteria>
  <criterion>doc vs query prefix produces non-identical vectors</criterion>
  <criterion>vector len 768, finite, L2-normalized</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e10_contextual</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

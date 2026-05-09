# TASK-EMBED-018 — E6 Sparse + E9 HDC + E12 ColBERT + E13 SPLADE wrappers (one task, all 4)

```xml
<task_spec id="TASK-EMBED-018" version="1.0">
<metadata>
  <title>Wrappers for E6 (Sparse cocondenser, 30K→1536-d), E9 (HDC 10K-bit→1024-d), E12 (ColBERTv2 128/token), E13 (SPLADE v3 30K→1536-d)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E6/E9/E12/E13 wrappers</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Four "non-standard-dense" embedders grouped into one task: E6 (sparse SPLADE-cocondenser), E9 (hyperdimensional 10K-bit), E12 (ColBERTv2 late-interaction 128-d/token), E13 (SPLADE v3). Each has unique forward semantics but the wrapper pattern is identical. Each projects to a fixed-length dense vector via documented projection: E6 → 1536-d, E9 → 1024-d (10K-bit hash), E12 → 128-d single pooled, E13 → 1536-d.
</context>

<input_context_files>
  <file purpose="sparse_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E6 Sparse, E13 Splade)</file>
  <file purpose="hdc_pattern">crates/context-graph-embeddings/src/types/hdc.rs</file>
  <file purpose="colbert_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E12 LateInteraction)</file>
  <file purpose="e1_template">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E6SparseForward { tokenizer, model: SpladeCocondenserModel, projection: Linear(30522, 1536) }` — sparse 30K projected to 1536-d dense
    - `pub struct E9HdcForward { hdc_basis: Arc<Vec<u64>> }` — 10K-bit HDC vector, projected via fixed random matrix to 1024-d
    - `pub struct E12LateInteractionForward { tokenizer, colbert_model }` — produces per-token 128-d vectors, then mean-pooled to 128-d single output for the wrapper API (multi-vector mode out of scope here)
    - `pub struct E13SpladeForward { tokenizer, splade_v3_model, projection: Linear(30522, 1536) }`
    - `impl EmbedderForward` for each
    - 4 Factories + 4 register fns + `register_all()` group helper
    - 4 Instrument structs
    - validate_finite + validate_dim per output dim
    - Tests: vector len matches projected_dimension; deterministic; empty/invalid input rejected
  </in_scope>
  <out_of_scope>
    - Native sparse representation persistence (E6/E13 multi-vector storage); v1 ships dense projection only
    - Multi-vector ColBERT MaxSim retrieval (out of scope for this wrapper; INSTRUMENTS / search domain)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/sparse_hdc_colbert_splade.rs">
      pub const E6_VERSION: &amp;str = "splade-cocondenser-v1";
      pub const E6_PROJECTED_DIM: usize = 1536;

      pub const E9_VERSION: &amp;str = "hdc-10k-bit-v1";
      pub const E9_PROJECTED_DIM: usize = 1024;

      pub const E12_VERSION: &amp;str = "colbertv2";
      pub const E12_PROJECTED_DIM: usize = 128;

      pub const E13_VERSION: &amp;str = "splade-v3";
      pub const E13_PROJECTED_DIM: usize = 1536;

      pub struct E6SparseForward { /* ... */ }
      pub struct E9HdcForward { /* ... */ }
      pub struct E12LateInteractionForward { /* ... */ }
      pub struct E13SpladeForward { /* ... */ }

      impl EmbedderForward for E6SparseForward { /* ... */ }
      impl EmbedderForward for E9HdcForward { /* ... */ }
      impl EmbedderForward for E12LateInteractionForward { /* ... */ }
      impl EmbedderForward for E13SpladeForward { /* ... */ }

      pub struct E6SparseFactory;
      pub struct E9HdcFactory;
      pub struct E12LateInteractionFactory;
      pub struct E13SpladeFactory;

      impl EmbedderForwardFactory for E6SparseFactory { /* ... */ }
      impl EmbedderForwardFactory for E9HdcFactory { /* ... */ }
      impl EmbedderForwardFactory for E12LateInteractionFactory { /* ... */ }
      impl EmbedderForwardFactory for E13SpladeFactory { /* ... */ }

      pub fn register_all();

      pub struct E6SparseInstrument { ensemble: Arc&lt;Ensemble&gt; }
      pub struct E9HdcInstrument { ensemble: Arc&lt;Ensemble&gt; }
      pub struct E12LateInteractionInstrument { ensemble: Arc&lt;Ensemble&gt; }
      pub struct E13SpladeInstrument { ensemble: Arc&lt;Ensemble&gt; }

      impl Instrument for E6SparseInstrument { /* ... */ }
      impl Instrument for E9HdcInstrument { /* ... */ }
      impl Instrument for E12LateInteractionInstrument { /* ... */ }
      impl Instrument for E13SpladeInstrument { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines (split into 4 sub-files inside `embedders/sparse_hdc/...` if 500 line limit is tight; otherwise single file).
    - E6/E13 expects Text input.
    - E9 expects Hdc input (HdcInput payload).
    - E12 expects Text; pools per-token vectors to single 128-d.
    - validate_dim per concrete projected dim.
    - All forwards stamp embedder_id, version, weight_sha, latency_ms, precision_class (Bf16 default).
    - register_all() registers all 4 factories.
    - All E1-pattern constraints apply (no mocks-while-broken; provenance comment).
  </constraints>

  <verification>
    - E6 vector len 1536, E9 len 1024, E12 len 128, E13 len 1536
    - Deterministic across trials
    - Empty/invalid input rejected
    - cargo test sparse_hdc_colbert_splade passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
sparse_hdc_colbert_splade.rs:
  - 4 forward structs + 4 factories + 4 instruments.
  - E6/E13 share the SPLADE pattern (logits → ReLU → log1p → projection to 1536-d dense).
  - E9: hash text bytes into 10K-bit vector via XOR-and-rotate; project via fixed random matrix to 1024-d.
  - E12: tokenize → BERT forward → linear → 128 per token → mean-pool to 128.
  - register_all() registers all four.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/sparse_hdc_colbert_splade.rs">All 4 wrappers + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod sparse_hdc_colbert_splade;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>E6/E9/E12/E13 vector lens match projected_dimension</criterion>
  <criterion>Deterministic output across trials</criterion>
  <criterion>register_all() wires all 4 factories</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders sparse_hdc_colbert_splade</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

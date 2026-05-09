# TASK-EMBED-010 — E1 Semantic Instrument wrapper (e5-large-v2, 1024-d)

```xml
<task_spec id="TASK-EMBED-010" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E1Semantic&gt; wrapping the shipped e5-large-v2 forward path; registers EmbedderForwardFactory</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-EMBED-01">Instrument trait wrapper</item>
    <item ref="REQ-EMBED-10">deterministic encode under fixed (input, weight_sha, code_version)</item>
    <item ref="EC-EMBED-01">empty input → MEJEPA_EMBED_INVALID_INPUT</item>
    <item ref="EC-EMBED-02">truncate to max_tokens with truncated_input flag</item>
    <item ref="TECH-EMBEDDERS::component_contracts::EmbedderInstrument">instrument.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
First per-embedder wrapper. Establishes the pattern every other E1-E14 wrapper follows: implement `EmbedderForward` for the forward pass, register an `EmbedderForwardFactory`, and provide an `EmbedderInstrument<E1>` that implements the upstream `Instrument` trait so the panel pipeline can call `.encode()`. E1 (Semantic, e5-large-v2, 1024-d, 512-token max, "search_document:" prefix) is the most-used content embedder.
</context>

<input_context_files>
  <file purpose="upstream_e1_implementation">crates/context-graph-embeddings/src/types/model_id/repository.rs</file>
  <file purpose="upstream_e5_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs</file>
  <file purpose="ensemble_traits">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="instrument_trait">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="data_models">crates/context-graph-mejepa-embedders/src/types.rs</file>
  <file purpose="instrument_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#component_contracts (EmbedderInstrument)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete (`Ensemble`, `EmbedderForward`, `EmbedderForwardFactory`)</check>
  <check>candle-transformers BERT/RoBERTa support available</check>
  <check>tokenizers crate available</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E1SemanticForward { tokenizer: Tokenizer, model: BertModel, device: Device, weight_sha: String, version: String }`
    - `impl EmbedderForward for E1SemanticForward { fn forward(...) -> EmbedResult<EmbedderOutput> { ... } }`
      1. Validate `EmbedderInput::Text { text }` (else InvalidInput)
      2. Reject empty text (EC-EMBED-01)
      3. Apply "search_document:" prefix (per E5/e5-large-v2 train contract; lesson from MEMORY.md)
      4. Tokenize with `truncation = true; max_length = 512`; record `truncated_input` flag
      5. Forward through BertModel at bf16 on candle-cuda (or fp32 under `--feature strict-fp32-replay`)
      6. Mean-pool over token embeddings → 1024-d
      7. L2-normalize
      8. validate_finite() + validate_dim(1024)
      9. Build EmbedderOutput { vector, scalars: empty, embedder_id: E1Semantic, version: "e5-large-v2-2024-11-15", weight_sha, latency_ms, precision_class, truncated_input }
    - `pub struct E1SemanticFactory;` implementing `EmbedderForwardFactory` — `register_factory(Box::new(E1SemanticFactory))` invoked from a `pub fn register_e1()` exported and called by Ensemble or by an `inventory::submit!` ctor
    - `pub struct EmbedderInstrument<const SLOT_NUM: u8> { ensemble: Arc<Ensemble>, embedder_id: EmbedderId, slot: InstrumentSlot, output_dim: usize }` (generic version) OR a concrete `pub struct E1SemanticInstrument { ensemble: Arc<Ensemble> }` per-embedder
    - `impl Instrument for E1SemanticInstrument { type Input = EmbedderInput; fn slot(&self) -> InstrumentSlot { InstrumentSlot::EProblem (or another slot per panel layout) }; fn encode(&self, input: &EmbedderInput) -> InstrumentResult<Vec<f32>> { self.ensemble.encode(EmbedderId::E1Semantic, input).map(|out| out.vector).map_err(|e| e.into_instrument_error()) }; fn embedder_id(&self) -> Option<EmbedderIdRef> { Some(EmbedderId::E1Semantic.into()) }; fn version(&self) -> &str { ... }; fn weight_sha(&self) -> Option<&str> { ... } }`
    - Tests:
      - empty text → InvalidInput
      - text ≤ 512 tokens → vector len 1024, all finite, truncated_input=false
      - text >> 512 tokens → truncated_input=true, vector len 1024
      - two encodes of identical text byte-equal in fp32 path; bf16 path 1-ULP-tolerant (gated test)
      - factory registration round-trip via Ensemble.load_one(E1Semantic) succeeds against tempfile fixture
      - encode after lora-refresh path bumps weight_sha (deferred; tested in TASK-EMBED-020 integration)
  </in_scope>
  <out_of_scope>
    - Real model weights (FSV uses real weights when available; CI uses fixture stub via factory env override)
    - LoRA adapter on E1 (E1 not in REQ-EMBED-06 scope; LoRA wires E5/E11/E14)
    - Multi-batch forward (Phase 1b is single-shot; batching is Phase 2 predictor concern)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs">
      use std::sync::Arc;
      use candle_core::{Device, DType, Tensor};
      use candle_nn::VarBuilder;
      use tokenizers::Tokenizer;

      use context_graph_mejepa_instruments::{Instrument, InstrumentSlot, InstrumentResult, EmbedderIdRef};

      use crate::EmbedderId;
      use crate::ensemble::{Ensemble, EmbedderForward, EmbedderForwardFactory};
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::EmbedderRegistration;
      use crate::types::{EmbedderInput, EmbedderOutput, PrecisionClass};

      pub const E1_VERSION: &amp;str = "e5-large-v2-2024-11-15";
      pub const E1_PROJECTED_DIM: usize = 1024;
      pub const E1_MAX_TOKENS: usize = 512;
      pub const E1_PREFIX: &amp;str = "search_document: ";

      pub struct E1SemanticForward {
          tokenizer: Tokenizer,
          /* candle BertModel handle */
          weight_sha: String,
          version: String,
          device: Device,
      }

      impl EmbedderForward for E1SemanticForward {
          fn forward(&amp;self, input: &amp;EmbedderInput) -&gt; EmbedResult&lt;EmbedderOutput&gt;;
      }

      pub struct E1SemanticFactory;
      impl EmbedderForwardFactory for E1SemanticFactory {
          fn embedder_id(&amp;self) -&gt; EmbedderId { EmbedderId::E1Semantic }
          fn build(&amp;self, device: &amp;Device, registration: &amp;EmbedderRegistration) -&gt; EmbedResult&lt;Box&lt;dyn EmbedderForward + Send + Sync&gt;&gt;;
      }

      pub fn register() {
          crate::ensemble::register_factory(Box::new(E1SemanticFactory));
      }

      pub struct E1SemanticInstrument {
          ensemble: Arc&lt;Ensemble&gt;,
      }

      impl E1SemanticInstrument {
          pub fn new(ensemble: Arc&lt;Ensemble&gt;) -&gt; Self;
      }

      impl Instrument for E1SemanticInstrument {
          type Input = EmbedderInput;
          fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EProblem }
          fn output_dim(&amp;self) -&gt; usize { E1_PROJECTED_DIM }
          fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;
          fn embedder_id(&amp;self) -&gt; Option&lt;EmbedderIdRef&gt; { Some(EmbedderId::E1Semantic.into()) }
          fn version(&amp;self) -&gt; &amp;str { E1_VERSION }
          fn weight_sha(&amp;self) -&gt; Option&lt;&amp;str&gt; { /* read from ensemble.loaded[E1].weight_sha */ }
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - "search_document:" prefix per E5/e5-large-v2 training contract (per MEMORY.md lesson `Train/inference prefix MUST match`).
    - Mean-pool over non-padded tokens (mask-aware); L2-normalize at the end.
    - bf16 default; fp32 under `--feature strict-fp32-replay`.
    - validate_finite + validate_dim(1024) before returning.
    - tokenizer truncation strategy: head (per EC-EMBED-02 for E1).
    - Empty text → `EmbedError::InvalidInput { reason: "empty E1 text input" }`.
    - Forward returns `EmbedderOutput { embedder_id: E1Semantic, embedder_version: E1_VERSION, weight_sha: self.weight_sha.clone(), latency_ms, precision_class, truncated_input, ... }`.
    - The Instrument impl maps `EmbedError -> InstrumentError` via thiserror conversion (or a `try_into_instrument_error()` helper on EmbedError).
    - NO mock data in tests; CI uses tempfile-backed stub factory (registered via `register_e1_test()` in `#[cfg(test)] mod tests`).
    - The Instrument's `slot()` is `InstrumentSlot::EProblem` per panel layout (1024-d slot); other E1-using slots (E_Test=384-d, E_CommitMsg=384-d) project via downstream instrument-side projection heads (out of scope; INSTRUMENTS domain).
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders e1_semantic::tests` passes.
    - empty text → MEJEPA_EMBED_INVALID_INPUT.
    - 512+-token text → truncated_input=true; vector len 1024.
    - two identical encodes byte-equal under fp32; bf16 within 1-ULP.
    - Factory registration: `register()` then `Ensemble::load_one(E1Semantic)` builds the LoadedEmbedder.
    - cargo clippy -- -D warnings clean.
  </verification>
</definition_of_done>

<pseudo_code>
e1_semantic.rs:
  - E1SemanticForward holds tokenizer + BertModel.
  - forward():
      let text = match input { EmbedderInput::Text { text, .. } => text, _ => return InvalidInput };
      if text.is_empty() { return InvalidInput; }
      let prefixed = format!("{}{}", E1_PREFIX, text);
      let encoding = self.tokenizer.encode(prefixed, true)?;
      let (ids, mask, truncated) = (encoding.get_ids(), encoding.get_attention_mask(), encoding.get_overflowing().len() > 0);
      let ids_t = Tensor::new(ids, &self.device)?;
      let mask_t = Tensor::new(mask, &self.device)?;
      let hidden = self.model.forward(&ids_t, &mask_t, ...)?;
      let pooled = mask_aware_mean_pool(&hidden, &mask_t)?;
      let normalized = l2_normalize(&pooled)?;
      let vector: Vec<f32> = normalized.to_vec1()?;
      let out = EmbedderOutput { vector, scalars: BTreeMap::new(), embedder_id: E1Semantic, embedder_version: E1_VERSION.into(), weight_sha: self.weight_sha.clone(), latency_ms: ..., precision_class: PrecisionClass::Bf16, truncated_input: truncated };
      out.validate_finite()?;
      out.validate_dim(E1_PROJECTED_DIM)?;
      Ok(out)
  - E1SemanticFactory::build: load tokenizer from registration.weight_files[0].parent()/tokenizer.json; load BertModel via VarBuilder::from_safetensors(... in bf16 dtype).
  - E1SemanticInstrument::encode delegates to ensemble.encode(E1Semantic, input).

lib.rs:
  - pub mod embedders { pub mod e1_semantic; }
  - re-export E1SemanticInstrument
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Module declaration + per-embedder pub mods (initially e1_semantic; subsequent tasks extend)</file>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs">E1SemanticForward + Factory + Instrument + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod embedders;` + re-export E1SemanticInstrument</file>
</files_to_modify>

<validation_criteria>
  <criterion>empty text rejected with MEJEPA_EMBED_INVALID_INPUT</criterion>
  <criterion>vector len 1024, finite, L2-normalized</criterion>
  <criterion>truncated_input flag set on >512-token input</criterion>
  <criterion>two identical encodes byte-equal under fp32</criterion>
  <criterion>Factory registers and Ensemble::load_one(E1Semantic) succeeds</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e1_semantic</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

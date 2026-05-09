# TASK-EMBED-011 — E5 Causal wrapper (nomic-embed, 768-d, asymmetric cause/effect)

```xml
<task_spec id="TASK-EMBED-011" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E5Causal&gt; with asymmetric cause/effect dual-vector storage; nomic-embed-text-v1.5 (768-d)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E5 wrapper</item>
    <item ref="REQ-EMBED-06">E5 LoRA-target embedder (REQ-EMBED-06 ships LoRA mgr; this task ensures forward path supports adapter-modulated forward)</item>
    <item ref="TECH-EMBEDDERS::component_contracts::EmbedderInstrument">E5 specifics</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
E5 is the causal embedder (nomic-embed-text-v1.5, 768-d, capped at 512 tokens for causal use). Asymmetric: stores dual cause/effect vectors. The `EmbedderInput::Text` variant carries an extension `causal_direction: Option<CausalDirection>` (Cause | Effect) so the same input is encoded with the appropriate prefix ("Represent the cause:" vs "Represent the effect:"). E5 is a LoRA-target embedder (REQ-EMBED-06) — the forward path supports adapter modulation when `LoadedEmbedder.lora_active = Some(adapter)`.
</context>

<input_context_files>
  <file purpose="e5_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E5 Causal)</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="types">crates/context-graph-mejepa-embedders/src/types.rs</file>
  <file purpose="e1_pattern">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete</check>
  <check>TASK-EMBED-010 complete (E1 establishes the wrapper pattern)</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub enum CausalDirection { Cause, Effect }` (or re-import from upstream if exists)
    - Extend `EmbedderInput::Text` with optional metadata field `causal_direction: Option<CausalDirection>` — backward-compat default None means "encode as Cause"
    - `pub struct E5CausalForward { tokenizer, model: NomicBertModel, weight_sha, version, device }`
    - `impl EmbedderForward for E5CausalForward`:
      1. Validate Text input + non-empty
      2. Determine prefix from causal_direction: "Represent the cause:" or "Represent the effect:"
      3. Tokenize (max 512); head-truncate
      4. Forward; mean-pool; L2-normalize
      5. validate_finite + validate_dim(768)
      6. Stamp `scalars: { "causal_direction_score_cause": 1.2, "causal_direction_score_effect": 0.8 }` per memory `E5 causal direction: cause→effect 1.2x, effect→cause 0.8x`
    - `pub struct E5CausalFactory; pub fn register();`
    - `pub struct E5CausalInstrument { ensemble: Arc<Ensemble> }`
    - `impl Instrument for E5CausalInstrument` returning `InstrumentSlot::EReasoning` (or appropriate causal slot — coordinate via per-instrument routing in INSTRUMENTS domain; for E5 wrapper, slot is informational since E5 feeds the routing table, not directly into a panel slot here)
    - `pub fn forward_with_lora(&self, input, adapter: &LoraAdapter) -> EmbedResult<EmbedderOutput>` — LoRA-modulated forward (used by TASK-EMBED-020 fine-tune); base buffer SHA-checked pre/post by caller
    - Tests:
      - cause vs effect prefix produces different vectors (cosine < 1.0)
      - Empty text rejected
      - 512+-token truncation flag set
      - Two encodes byte-equal under fp32 with same direction
      - forward_with_lora invocation runs without writing back to base (verified via SHA-check stub)
  </in_scope>
  <out_of_scope>
    - Actual LoRA training step (TASK-EMBED-020)
    - Causal gate threshold tuning (out of EMBEDDERS scope; TCT/INFERENCE domain)
    - E5 as a panel slot owner (panel slot wiring is in INSTRUMENTS domain)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e5_causal.rs">
      use std::sync::Arc;
      use candle_core::Device;
      use serde::{Deserialize, Serialize};
      use tokenizers::Tokenizer;

      use context_graph_mejepa_instruments::{Instrument, InstrumentSlot, InstrumentResult, EmbedderIdRef};

      use crate::EmbedderId;
      use crate::ensemble::{Ensemble, EmbedderForward, EmbedderForwardFactory};
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::EmbedderRegistration;
      use crate::types::{EmbedderInput, EmbedderOutput, PrecisionClass};

      #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub enum CausalDirection { Cause, Effect }

      pub const E5_VERSION: &amp;str = "nomic-embed-text-v1.5";
      pub const E5_PROJECTED_DIM: usize = 768;
      pub const E5_MAX_TOKENS: usize = 512;
      pub const E5_PREFIX_CAUSE: &amp;str = "Represent the cause: ";
      pub const E5_PREFIX_EFFECT: &amp;str = "Represent the effect: ";

      pub struct E5CausalForward { /* ... */ }

      impl EmbedderForward for E5CausalForward {
          fn forward(&amp;self, input: &amp;EmbedderInput) -&gt; EmbedResult&lt;EmbedderOutput&gt;;
      }

      impl E5CausalForward {
          pub fn forward_with_lora(&amp;self, input: &amp;EmbedderInput, adapter: &amp;LoraAdapterHandle) -&gt; EmbedResult&lt;EmbedderOutput&gt;;
      }

      pub struct LoraAdapterHandle; // populated by TASK-EMBED-020

      pub struct E5CausalFactory;
      impl EmbedderForwardFactory for E5CausalFactory { /* ... */ }
      pub fn register();

      pub struct E5CausalInstrument { ensemble: Arc&lt;Ensemble&gt; }

      impl Instrument for E5CausalInstrument {
          type Input = EmbedderInput;
          fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EReasoning } // informational
          fn output_dim(&amp;self) -&gt; usize { E5_PROJECTED_DIM }
          fn encode(&amp;self, input: &amp;EmbedderInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;
          fn embedder_id(&amp;self) -&gt; Option&lt;EmbedderIdRef&gt; { Some(EmbedderId::E5Causal.into()) }
          fn version(&amp;self) -&gt; &amp;str { E5_VERSION }
          fn weight_sha(&amp;self) -&gt; Option&lt;&amp;str&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - causal_direction defaults to Cause if absent (backward-compat).
    - cause prefix and effect prefix produce vectors with cosine &lt; 1.0 — verify in tests.
    - 768-d output, L2-normalized, mean-pooled.
    - forward_with_lora MUST take a `&LoraAdapterHandle` (defined as opaque placeholder; full type lands in TASK-EMBED-020).
    - The test verifying cause ≠ effect uses canned tokenizer outputs (not real model weights) — sufficient to prove prefix wiring.
    - All other constraints from E1 wrapper apply (file size, validate_finite, validate_dim, no mock-when-real-data-needed).
  </constraints>

  <verification>
    - cause prefix vs effect prefix produces non-identical vectors.
    - Empty text → InvalidInput.
    - Truncation flag set on > 512-token input.
    - cargo test e5_causal passes.
    - cargo clippy -- -D warnings clean.
  </verification>
</definition_of_done>

<pseudo_code>
e5_causal.rs:
  - Forward struct + factory + Instrument as in E1, with causal_direction prefix dispatch.
  - LoraAdapterHandle is a `pub struct LoraAdapterHandle { adapter_id: Uuid, ... }` placeholder; concrete fields filled in TASK-EMBED-020.
  - Tests using a stubbed tokenizer + small synthetic forward (no real model needed for prefix-difference test).

lib.rs:
  - pub mod e5_causal in embedders/.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e5_causal.rs">E5CausalForward + Factory + Instrument + LoraAdapterHandle placeholder + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod e5_causal;`</file>
  <file path="crates/context-graph-mejepa-embedders/src/types.rs">Add `causal_direction: Option<CausalDirection>` to EmbedderInput::Text variant (default None)</file>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Re-export E5CausalInstrument</file>
</files_to_modify>

<validation_criteria>
  <criterion>cause vs effect prefix produces non-identical vectors</criterion>
  <criterion>vector len 768, finite, L2-normalized</criterion>
  <criterion>causal_direction None defaults to Cause</criterion>
  <criterion>forward_with_lora signature accepts LoraAdapterHandle (placeholder)</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e5_causal</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

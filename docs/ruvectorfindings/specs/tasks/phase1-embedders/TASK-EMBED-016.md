# TASK-EMBED-016 — E14 BgeM3Dense wrapper (BGE-M3, 1024-d, 8K-token, multilingual)

```xml
<task_spec id="TASK-EMBED-016" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E14BgeM3Dense&gt; wrapping BGE-M3 dense head (XLM-RoBERTa-Large, 1024-d, 8192-token context)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>16</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E14 wrapper</item>
    <item ref="REQ-EMBED-06">E14 LoRA-target embedder</item>
    <item ref="EC-EMBED-02">truncate to 8192 tokens (E14-specific)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
E14 (BGE-M3 dense head, XLM-RoBERTa-Large base, 1024-d, 8192-token context, multilingual). Heaviest content embedder after E7 (~2.2 GB bf16). Routes from Function bodies per REQ-EMBED-04 (alongside E7). LoRA-target. Multilingual support critical for cross-language SWE-bench.
</context>

<input_context_files>
  <file purpose="bge_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E14 BgeM3Dense)</file>
  <file purpose="e7_template">crates/context-graph-mejepa-embedders/src/embedders/e7_code.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete</check>
  <check>TASK-EMBED-011 complete (LoraAdapterHandle)</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E14BgeM3DenseForward` wrapping XLM-RoBERTa-Large dense head
    - 1024-d, L2-normalised, mean-pooled
    - 8192-token max, head truncation
    - forward_with_lora support
    - Factory + register + Instrument
    - Tests: multilingual round-trip (English + non-English text encode without panicking); truncation; finite; deterministic fp32
  </in_scope>
  <out_of_scope>
    - Sparse + ColBERT BGE-M3 heads (out of scope; E14 ships dense head only)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e14_bge_m3.rs">
      pub const E14_VERSION: &amp;str = "bge-m3-2024";
      pub const E14_PROJECTED_DIM: usize = 1024;
      pub const E14_MAX_TOKENS: usize = 8192;

      pub struct E14BgeM3DenseForward { /* ... */ }
      impl EmbedderForward for E14BgeM3DenseForward { /* ... */ }
      impl E14BgeM3DenseForward { pub fn forward_with_lora(&amp;self, input, adapter: &amp;LoraAdapterHandle) -&gt; EmbedResult&lt;EmbedderOutput&gt;; }

      pub struct E14BgeM3DenseFactory;
      impl EmbedderForwardFactory for E14BgeM3DenseFactory { /* ... */ }
      pub fn register();

      pub struct E14BgeM3DenseInstrument { ensemble: Arc&lt;Ensemble&gt; }
      impl Instrument for E14BgeM3DenseInstrument { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - 1024-d, L2-normalised, mean-pool over non-padded tokens.
    - 8192-token max; head truncation; truncated_input flag.
    - Multilingual safety: tokenizer MUST accept UTF-8 non-ASCII without panicking (per BR-EMBED-06 determinism).
    - All E1-pattern constraints apply.
  </constraints>

  <verification>
    - English + non-English encode succeeds
    - vector len 1024, finite
    - truncation flag accurate
    - cargo test e14_bge_m3 passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
e14_bge_m3.rs:
  - Same template as E7 with 1024-d, 8192 tokens, multilingual tokenizer.
  - forward_with_lora parallel to E5/E11.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e14_bge_m3.rs">E14BgeM3DenseForward + Factory + Instrument + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod e14_bge_m3;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>vector len 1024, finite, L2-normalized</criterion>
  <criterion>multilingual UTF-8 input encodes without panic</criterion>
  <criterion>truncated_input flag set on > 8192-token input</criterion>
  <criterion>forward_with_lora signature accepts LoraAdapterHandle</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e14_bge_m3</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

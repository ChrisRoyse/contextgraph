# TASK-EMBED-015 — E11 Kepler wrapper (RoBERTa+TransE, 768-d)

```xml
<task_spec id="TASK-EMBED-015" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E11Kepler&gt; wrapping KEPLER (RoBERTa-base + TransE, 768-d)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E11 wrapper</item>
    <item ref="REQ-EMBED-03">Legacy `Entity` alias maps to Kepler</item>
    <item ref="REQ-EMBED-06">E11 LoRA-target embedder</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
E11 (Kepler, RoBERTa-base + TransE on Wikidata5M, 768-d, 512-token max). Replaces legacy MiniLM-384D (E11=Entity). Routes from Class, Struct, Import per REQ-EMBED-04. LoRA-target. Forward path supports adapter modulation (forward_with_lora) used by TASK-EMBED-020.
</context>

<input_context_files>
  <file purpose="e11_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E11 Kepler)</file>
  <file purpose="e5_template">crates/context-graph-mejepa-embedders/src/embedders/e5_causal.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete</check>
  <check>TASK-EMBED-011 complete (LoraAdapterHandle placeholder)</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E11KeplerForward` with RoBERTa-base + TransE entity head
    - 768-d output, L2-normalised
    - forward_with_lora support (LoRA target)
    - Factory + register + Instrument
    - Tests: empty rejected; truncation flag; vector len 768; deterministic under fp32
  </in_scope>
  <out_of_scope>
    - Wikidata5M entity-grounding lookup integration
    - LoRA training step
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e11_kepler.rs">
      pub const E11_VERSION: &amp;str = "kepler-roberta-base-transe-wikidata5m";
      pub const E11_PROJECTED_DIM: usize = 768;
      pub const E11_MAX_TOKENS: usize = 512;

      pub struct E11KeplerForward { /* ... */ }
      impl EmbedderForward for E11KeplerForward { /* ... */ }
      impl E11KeplerForward { pub fn forward_with_lora(&amp;self, input, adapter: &amp;LoraAdapterHandle) -&gt; EmbedResult&lt;EmbedderOutput&gt;; }

      pub struct E11KeplerFactory;
      impl EmbedderForwardFactory for E11KeplerFactory { /* ... */ }
      pub fn register();

      pub struct E11KeplerInstrument { ensemble: Arc&lt;Ensemble&gt; }
      impl Instrument for E11KeplerInstrument { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - 768-d, L2-normalized, mean-pooled.
    - forward_with_lora parallel to E5.
    - All E1-pattern constraints apply.
  </constraints>

  <verification>
    - vector len 768, finite, L2-normalized
    - cargo test e11_kepler passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
Same template as E5 minus causal_direction; supports LoRA via forward_with_lora.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e11_kepler.rs">E11KeplerForward + Factory + Instrument + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod e11_kepler;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>vector len 768, finite, L2-normalized</criterion>
  <criterion>forward_with_lora signature accepts LoraAdapterHandle</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e11_kepler</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

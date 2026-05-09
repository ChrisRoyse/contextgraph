# TASK-EMBED-012 — E7 Code wrapper (Qodo-Embed-1-1.5B, 1536-d)

```xml
<task_spec id="TASK-EMBED-012" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E7Code&gt; wrapping Qodo-Embed-1-1.5B forward path (1536-d native, 32K-token context)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E7 wrapper</item>
    <item ref="EC-EMBED-02">truncate to 32K tokens (E7-specific)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
E7 (Code, Qodo-Embed-1-1.5B, 1536-d native, 32768-token context) is the heaviest single embedder (~2.9 GB bf16). Code-domain specialization. Per routing table E7 routes to Function bodies, Function tests, Class, Struct. Critical for E_Code panel slot input. No prefix; raw code text directly tokenized.
</context>

<input_context_files>
  <file purpose="qodo_pattern">crates/context-graph-embeddings/src/types/model_id/core.rs (E7 Code)</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="e1_template">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009, 010 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E7CodeForward { tokenizer: Tokenizer, model: QodoModel, weight_sha, version, device }`
    - `impl EmbedderForward` validating Text input, truncating to 32768 tokens (head), mean-pooling, L2-normalizing → 1536-d
    - `pub const E7_VERSION: &str = "qodo-embed-1-1.5b"; E7_PROJECTED_DIM = 1536; E7_MAX_TOKENS = 32_768;`
    - `pub struct E7CodeFactory; pub fn register();`
    - `pub struct E7CodeInstrument { ensemble: Arc<Ensemble> }` with Instrument impl returning slot `EAst` (informational; E_AST consumes E7 + projection in INSTRUMENTS domain)
    - Tests: empty rejected; truncation flag for >32K-token input; vector len 1536, finite; 2x identical encode byte-equal fp32
  </in_scope>
  <out_of_scope>
    - Code-aware tokenization beyond Qodo's default (out of scope; trusts upstream tokenizer)
    - LoRA on E7 (E7 not a LoRA target per REQ-EMBED-06)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e7_code.rs">
      pub const E7_VERSION: &amp;str = "qodo-embed-1-1.5b";
      pub const E7_PROJECTED_DIM: usize = 1536;
      pub const E7_MAX_TOKENS: usize = 32_768;

      pub struct E7CodeForward { /* ... */ }
      impl EmbedderForward for E7CodeForward { /* validates + tokenizes + forwards + pools + L2 normalises */ }

      pub struct E7CodeFactory;
      impl EmbedderForwardFactory for E7CodeFactory { /* ... */ }
      pub fn register();

      pub struct E7CodeInstrument { ensemble: Arc&lt;Ensemble&gt; }
      impl Instrument for E7CodeInstrument {
          type Input = EmbedderInput;
          fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EAst } // informational
          fn output_dim(&amp;self) -&gt; usize { E7_PROJECTED_DIM }
          fn encode(&amp;self, input: &amp;EmbedderInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;
          fn embedder_id(&amp;self) -&gt; Option&lt;EmbedderIdRef&gt; { Some(EmbedderId::E7Code.into()) }
          fn version(&amp;self) -&gt; &amp;str { E7_VERSION }
          fn weight_sha(&amp;self) -&gt; Option&lt;&amp;str&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - 32K-token max; head truncation; flag truncated_input.
    - validate_finite + validate_dim(1536) before returning.
    - All E1-pattern constraints apply (provenance, no mocks-while-broken, no unwrap).
  </constraints>

  <verification>
    - empty text → InvalidInput
    - vector len 1536, finite
    - truncated_input flag accurate
    - cargo test e7_code passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
Same template as E1 with prefix=None (raw code) and dim=1536, max_tokens=32_768.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e7_code.rs">E7CodeForward + Factory + Instrument + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod e7_code;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>vector len 1536, finite, L2-normalized</criterion>
  <criterion>truncation flag set on > 32K-token input</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e7_code</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

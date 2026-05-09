# TASK-INSTR-010 — EDataFlowInstrument neural backing (E11 Kepler + 2-layer graph-attention over def-use)

```xml
<task_spec id="TASK-INSTR-010" version="1.0">

<metadata>
  <title>EDataFlowInstrument neural backing — frozen E11 Kepler + 2-layer graph-attention over def-use edges</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-INSTR-03">EDataFlowInstrument 256-d (frozen E11 + graph-attn)</item>
    <item ref="REQ-INSTR-19">AST-perfect chunk consumption (NOT raw text)</item>
    <item ref="component:EDataFlowInstrument">encode(CodeInstrumentInput) -&gt; 256 f32</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen — MEJEPA_INSTR_FROZEN_VIOLATION on gradient detection</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
    <task_ref>TASK-INSTR-004</task_ref>
    <task_ref>TASK-AST-022</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file modified, 0-1 file created</estimated_files>
</metadata>

<context>
Replaces shipped deterministic EDataFlowInstrument with neural backing per REQ-INSTR-03: extract
def-use edges per language (rust-analyzer SSA / Python def-use via tree-sitter / TSC TypeScript
flow / etc.), encode variables via frozen E11 Kepler (RoBERTa+TransE, 768-d), 2-layer
graph-attention head over (variable_embeddings, def_use_edges) -&gt; 256-d. Frozen invariant per
constitution `<utml_v1_invariants>` rule 2.

Per-language def-use extractors are language-specific and may delegate to the language toolchain
(rust-analyzer for Rust; ast.parse + tree-sitter for Python; tsc for TypeScript flow analysis;
javac for Java; gcc -fdump for C/C++; etc.). When the toolchain is unavailable, fail closed with
MEJEPA_INFER_INSTRUMENT_MISSING.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-03, REQ-INSTR-19)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EDataFlowInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_code.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs (TASK-INSTR-004)</file>
  <file purpose="ast_chunker_facade">crates/context-graph-core/src/memory/ast_chunker/ (TASK-AST-022)</file>
  <file purpose="design_doc_embedders">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §3 row E_DataFlow</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, TASK-INSTR-004 done</check>
  <check>TASK-AST-022 done</check>
  <check>E11 Kepler weights pinned in models_config.toml with SHA-256 digest</check>
  <check>2-layer graph-attention head weights frozen</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace EDataFlowInstrument deterministic encode with neural backing
    - Per-language def-use edge extraction
    - E11 Kepler forward over variable contexts (no_grad scope)
    - 2-layer graph-attention head over (variable_embeddings, def_use_edges) -&gt; 256-d
    - `EDataFlowInstrument::new_with_weights(kepler: &amp;Kepler, gat: &amp;GraphAttention2Layer, guard: &amp;mut FrozenGuard)` constructor
    - Fail-closed when E11 unavailable: MEJEPA_INFER_INSTRUMENT_MISSING
    - Fail-closed when language toolchain unavailable for def-use extraction
    - File-SHA-keyed cache
    - Per-language tests: 11 languages
    - Determinism + frozen-gradient tests
  </in_scope>
  <out_of_scope>
    - Training E11 or graph-attention weights
    - Type lattice (ETypeGraphInstrument's job, TASK-INSTR-011)
    - Linear-probe gate (TASK-INSTR-021)
    - Panel JSON persistence (TASK-INSTR-022)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_code.rs">
pub struct EDataFlowInstrument {
    kepler: Arc&lt;Kepler&gt;,           // E11 frozen
    gat: Arc&lt;GraphAttention2Layer&gt;,
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl EDataFlowInstrument {
    pub fn new_with_weights(
        kepler: Arc&lt;Kepler&gt;,
        gat: Arc&lt;GraphAttention2Layer&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for EDataFlowInstrument {
    type Input = CodeInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EDataFlow }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 finite f32
    - Frozen — FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - No raw String input
    - Per-language coverage: 11 languages
    - Latency budget: &lt; 10 ms cold (NFR-INSTR-02 E11/Kepler-derived)
    - Deterministic
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_data_flow::
    - cargo test --release -p context-graph-mejepa-instruments e_data_flow_per_language
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
e_code.rs:

  pub struct EDataFlowInstrument { kepler, gat, cache, embedder_versions }

  impl Instrument for EDataFlowInstrument {
      fn encode(&amp;self, input: &amp;CodeInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          let (variables, def_use_edges) = extract_def_use_per_language(input.language, &amp;input.chunks)?;
          if variables.is_empty() {
              return Err(InstrumentError::invalid("input.chunks", "no def-use chains extracted", "..."));
          }
          let var_embeddings: Vec&lt;Vec&lt;f32&gt;&gt; = variables.iter().map(|v| {
              let key = (v.context_sha256, self.embedder_versions.e11.clone());
              if let Some(emb) = self.cache.lock().get(&amp;key) { emb.clone() }
              else {
                  let emb = self.kepler.forward_no_grad(&amp;v.context_text)?; // 768-d
                  self.cache.lock().put(key, emb.clone());
                  emb
              }
          }).collect::&lt;Result&lt;_, _&gt;&gt;()?;
          let pooled = self.gat.forward_no_grad(&amp;var_embeddings, &amp;def_use_edges)?; // 256-d
          validate_finite_output("e_data_flow.output", &amp;pooled)?;
          Ok(pooled)
      }
  }

tests:
  - encode_empty_chunks_rejected
  - encode_per_language_def_use
  - encode_byte_equal_across_calls
  - encode_fails_when_e11_missing
  - encode_output_finite_and_dim_256
</pseudo_code>

<files_to_create>
  <!-- optional split if exceeded -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_code.rs">Replace EDataFlowInstrument deterministic encode with neural backing</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 256 finite f32</criterion>
  <criterion>11 languages encode end-to-end</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency p99 &lt; 10 ms cold</criterion>
  <criterion>Empty chunks rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT</criterion>
  <criterion>Missing E11 weights -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_data_flow::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments e_data_flow_per_language</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

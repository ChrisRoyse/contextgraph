# TASK-INSTR-009 — ECfgInstrument neural backing (E8 + 2-layer graph-attention)

```xml
<task_spec id="TASK-INSTR-009" version="1.0">

<metadata>
  <title>ECfgInstrument neural backing — frozen E8 Graph forward + 2-layer graph-attention head over basic blocks</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-INSTR-02">ECfgInstrument 256-d (frozen E8 Graph + graph-attention)</item>
    <item ref="REQ-INSTR-19">AST-perfect chunk consumption (NOT raw text)</item>
    <item ref="component:ECfgInstrument">encode(CodeInstrumentInput) -&gt; 256 f32</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen — gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION</item>
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
The shipped `ECfgInstrument` in `e_code.rs` is a deterministic typed encoder. This task replaces
it (behind the same `Instrument` trait) with the canonical neural backing per REQ-INSTR-02:
extract a control-flow graph from each AST-perfect chunk bounded by Function/Method/Impl/Closure,
encode each basic block via frozen E8 Graph, and run a 2-layer graph-attention head over
(block_embeddings, edges) -&gt; 256-d.

Per doc 12 §3 row E_CFG, E8 Graph is the substrate. Per constitution `<frozen_invariant>`, all
weights load with `requires_grad = false`; FrozenGuard registration at construction.

CFG extraction is per-language: tree-sitter block-statement maps yield basic blocks and edges
(branch, fallthrough, return, throw). Per doc 14 §3.2, raw text is rejected fail-closed.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-02, REQ-INSTR-19)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ECfgInstrument; data_models CodeInstrumentInput, AstPerfectChunk)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_code.rs (current deterministic encoder)</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs (TASK-INSTR-004)</file>
  <file purpose="ast_chunker_facade">crates/context-graph-core/src/memory/ast_chunker/ (TASK-AST-022)</file>
  <file purpose="design_doc_embedders">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §3 row E_CFG</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done</check>
  <check>TASK-INSTR-004 done (FrozenGuard primitive)</check>
  <check>TASK-AST-022 done (multi-language AstChunk API)</check>
  <check>E8 Graph weights pinned in models_config.toml with SHA-256 digest (Phase 1b embedders)</check>
  <check>2-layer graph-attention head weights frozen + checked into operator-controlled keyring</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace ECfgInstrument's deterministic encode with neural backing
    - Per-language CFG extraction over chunks bounded by Function/Method/Impl/Closure entities
    - Encode each basic block via frozen E8 Graph
    - 2-layer graph-attention head over (block_embeddings, edges) -&gt; 256-d
    - `ECfgInstrument::new_with_weights(e8: &amp;E8Graph, gat: &amp;GraphAttention2Layer, guard: &amp;mut FrozenGuard)` constructor
    - No-grad scope around forward pass
    - Fail-closed when E8 weights unavailable: MEJEPA_INFER_INSTRUMENT_MISSING
    - Fail-closed when chunk has no Function/Method/Impl/Closure entity: MEJEPA_INSTRUMENTS_INVALID_INPUT
    - File-SHA-keyed cache: `(chunk.sha256, embedder_version)`
    - Per-language tests: 11 languages, one real source file each
    - Determinism test: same CFG -&gt; byte-equal latent
    - Frozen-gradient test
  </in_scope>
  <out_of_scope>
    - Training E8 or graph-attention weights (frozen at construction)
    - Type lattice extraction (ETypeGraphInstrument's job, TASK-INSTR-011)
    - Linear-probe gate (TASK-INSTR-021)
    - Panel JSON persistence (TASK-INSTR-022)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_code.rs">
pub struct ECfgInstrument {
    e8: Arc&lt;E8Graph&gt;,
    gat: Arc&lt;GraphAttention2Layer&gt;,
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl ECfgInstrument {
    pub fn new_with_weights(
        e8: Arc&lt;E8Graph&gt;,
        gat: Arc&lt;GraphAttention2Layer&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for ECfgInstrument {
    type Input = CodeInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::ECfg }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 finite f32
    - Frozen: all params requires_grad=false; FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - No raw String input (REQ-INSTR-19)
    - Per-language coverage: 11 languages tested
    - Latency budget: &lt; 20 ms cold; &lt; 5 ms warm via file-SHA cache (NFR-INSTR-02)
    - Deterministic
    - File &lt; 500 lines (split if exceeded)
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_cfg::
    - cargo test -p context-graph-mejepa-instruments e_cfg_per_language --release
    - Frozen-gradient test (Phase 3 hook)
    - Latency profile FSV
  </verification>
</definition_of_done>

<pseudo_code>
e_code.rs:

  pub struct ECfgInstrument { e8, gat, cache, embedder_versions }

  impl Instrument for ECfgInstrument {
      fn encode(&amp;self, input: &amp;CodeInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.chunks.is_empty() {
              return Err(InstrumentError::invalid("input.chunks", "no chunks", "..."));
          }
          let mut block_embeddings: Vec&lt;Vec&lt;f32&gt;&gt; = vec![];
          let mut edges: Vec&lt;(usize, usize, EdgeKind)&gt; = vec![];
          for chunk in &amp;input.chunks {
              if !chunk.entity_type.is_function_or_method_or_impl_or_closure() { continue; }
              let cfg = extract_cfg_per_language(input.language, chunk)?;
              for block in &amp;cfg.blocks {
                  let key = (block.sha256, self.embedder_versions.e8.clone());
                  let v = if let Some(v) = self.cache.lock().get(&amp;key) { v.clone() } else {
                      let v = self.e8.forward_no_grad(&amp;block.text)?;
                      self.cache.lock().put(key, v.clone());
                      v
                  };
                  block_embeddings.push(v);
              }
              edges.extend(cfg.edges_with_offset(block_embeddings.len() - cfg.blocks.len()));
          }
          if block_embeddings.is_empty() {
              return Err(InstrumentError::invalid("input.chunks", "no executable basic blocks", "..."));
          }
          let pooled = self.gat.forward_no_grad(&amp;block_embeddings, &amp;edges)?; // 256-d
          validate_finite_output("e_cfg.output", &amp;pooled)?;
          Ok(pooled)
      }
  }

tests:
  - encode_empty_chunks_rejected
  - encode_per_language_cfg
  - encode_byte_equal_across_calls
  - encode_caches_repeated_block
  - encode_fails_when_e8_missing
  - encode_output_finite_and_dim_256
</pseudo_code>

<files_to_create>
  <!-- optional: crates/context-graph-mejepa-instruments/src/e_cfg/ if file exceeds 500 lines -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_code.rs">Replace ECfgInstrument deterministic encode with neural backing; wire CFG extraction + graph-attention; FrozenGuard registration</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 256 finite f32</criterion>
  <criterion>11 languages encode end-to-end</criterion>
  <criterion>Encode is deterministic (byte-equal across calls)</criterion>
  <criterion>Cache hit on repeat block skips E8 forward</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release -p context-graph-mejepa-instruments clean</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-instruments -- -D warnings clean</criterion>
  <criterion>Latency p50 &lt; 5 ms warm, p99 &lt; 20 ms cold</criterion>
  <criterion>Empty chunks input rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT</criterion>
  <criterion>Missing E8 weights -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_cfg::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments e_cfg_per_language</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

# TASK-INSTR-008 — EAstInstrument neural backing (Qodo + 1536→384 projection)

```xml
<task_spec id="TASK-INSTR-008" version="1.0">

<metadata>
  <title>EAstInstrument neural backing — frozen E7 Qodo forward + linear projection 1536→384</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-INSTR-01">EAstInstrument 384-d (frozen E7 Qodo + projection)</item>
    <item ref="REQ-INSTR-19">AST-perfect chunk consumption (NOT raw text)</item>
    <item ref="EC-INSTR-09">tree-sitter ERROR/MISSING node → MEJEPA_CHUNKER_PARSE_FAILED</item>
    <item ref="component:EAstInstrument">encode(CodeInstrumentInput) -> 384 f32</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen — gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
    <task_ref>TASK-INSTR-004</task_ref>
    <!-- Cross-domain: requires multi-language AST chunker (Phase 1a) -->
    <task_ref>TASK-AST-022</task_ref>
    <!-- Cross-domain: requires E7 frozen forward (Phase 1b embeddings) -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file modified, 0-1 file created</estimated_files>
</metadata>

<context>
The shipped `EAstInstrument` in `e_code.rs` is a deterministic typed encoder that consumes a
simplified `CodeInstrumentInput { language, path, source }`. This task replaces it (behind the
same trait) with the canonical neural backing per REQ-INSTR-01: forward each AST-perfect chunk
through frozen E7 Qodo (1536-d), mean-pool weighted by non-whitespace char count, project
1536→384 with a frozen linear layer, validate finite, return.

The shipped trait signature stays. The Input type changes from the simplified struct to the
multi-language `AstChunk`-bearing struct (or wraps it). The deterministic encoder remains as the
documented fallback when E7 weights are unavailable (`MEJEPA_INFER_INSTRUMENT_MISSING`).

Per constitution `<frozen_invariant>` note + REQ-INSTR-18, this task wires `FrozenGuard`
registration: at construction, the EAstInstrument registers its E7 weights + projection layer
weights with the FrozenGuard so any subsequent gradient flow detected halts training.

Per doc 14 §3.2 + REQ-INSTR-19, the input MUST be a typed AST-chunk struct (NOT raw String). The
chunker (TASK-AST-022, Phase 1a) provides the multi-language `AstChunk` API consumed here.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-01, REQ-INSTR-19, EC-INSTR-09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EAstInstrument; data_models CodeInstrumentInput, AstPerfectChunk; implementation_notes frozen_invariant + ast_chunk_consumption + performance)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_code.rs (current deterministic encoder)</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs (TASK-INSTR-004)</file>
  <file purpose="ast_chunker_facade">crates/context-graph-core/src/memory/ast_chunker/ (TASK-AST-022)</file>
  <file purpose="design_doc_chunking">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §3.2 §4</file>
  <file purpose="design_doc_embedders">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §3 row E_AST</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done (Panel/PanelBuilder verified)</check>
  <check>TASK-INSTR-004 done (FrozenGuard primitive)</check>
  <check>TASK-AST-022 done (multi-language AstChunk API + per-language FSV)</check>
  <check>E7 Qodo weights pinned in models_config.toml with SHA-256 digest (Phase 1b embedders)</check>
  <check>Frozen 1536→384 linear projection weights file checked into operator-controlled keyring (or generated deterministically with documented seed)</check>
</prerequisites>

<scope>
  <in_scope>
    - Update `CodeInstrumentInput` to the typed AST-chunk-bearing struct per TECH-INSTRUMENTS data_models (`language: ChunkerLanguage`, `chunks: Vec&lt;AstPerfectChunk&gt;`)
    - Replace EAstInstrument's deterministic encode with: per-chunk E7 Qodo forward, mean-pool weighted by non-whitespace char count, project 1536→384, validate finite
    - Add `EAstInstrument::new_with_weights(qodo: &amp;Qodo, projection: &amp;LinearProjection, guard: &amp;mut FrozenGuard)` constructor that registers parameters with the FrozenGuard
    - Implement no-grad scope around the forward pass (candle equivalent of `torch.no_grad()`)
    - Fail-closed when E7 weights unavailable: `MEJEPA_INFER_INSTRUMENT_MISSING`
    - Fail-closed when chunker produced ERROR/MISSING node: caller's responsibility (chunker rejects); EAstInstrument relies on already-validated chunks
    - File-SHA-keyed cache: lookup `(chunk.sha256, embedder_version)` before E7 forward; cache hits skip the GPU forward
    - Per-language tests: 11 languages, one real source file each, encoded via the live chunker
    - Determinism test: same input + same code_version + same embedder_versions → byte-equal output
    - Frozen-gradient test (with FrozenGuard): registered parameters report zero gradient post-backward
  </in_scope>
  <out_of_scope>
    - Training E7 (it's frozen)
    - Training the 1536→384 projection (frozen at construction; weights pinned)
    - Per-language toolchain validation (E_TypeGraph's job, TASK-INSTR-011)
    - Linear-probe gate (TASK-INSTR-021)
    - Panel JSON persistence (TASK-INSTR-022)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_code.rs">
use context_graph_core::memory::ast_chunker::{AstPerfectChunk, ChunkerLanguage};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeInstrumentInput {
    pub language: ChunkerLanguage,
    pub chunks: Vec&lt;AstPerfectChunk&gt;,
}

pub struct EAstInstrument {
    qodo: Arc&lt;Qodo&gt;,                         // E7 frozen forward
    projection: Arc&lt;LinearProjection&gt;,       // 1536-&gt;384 frozen
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,  // chunk_sha256 + embedder_version -&gt; latent
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl EAstInstrument {
    pub fn new_with_weights(
        qodo: Arc&lt;Qodo&gt;,
        projection: Arc&lt;LinearProjection&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for EAstInstrument {
    type Input = CodeInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EAst }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;  // 384 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 384 finite f32
    - Frozen: candle Var::set_requires_grad(false) on every parameter; FrozenGuard registration at construction; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - No raw String input (REQ-INSTR-19); compile-time-typed AstChunk
    - Per-language coverage: 11 languages tested
    - Latency budget: &lt; 20 ms cold per attempt with up to 8 chunks (NFR-INSTR-02); &lt; 5 ms warm via file-SHA cache
    - Deterministic
    - File-SHA cache keyed by (chunk.sha256, embedder_version)
    - When E7 unavailable: MEJEPA_INFER_INSTRUMENT_MISSING fail-closed
    - File &lt; 500 lines (split across e_code/ subdirectory if needed)
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_ast::
    - cargo test -p context-graph-mejepa-instruments e_ast_per_language --release  # all 11 languages
    - Frozen-gradient test (TASK-INSTR-021's training-loop fixture)
    - Latency profile FSV writes p50/p99 to evidence dir
  </verification>
</definition_of_done>

<pseudo_code>
e_code.rs:

  pub struct EAstInstrument { qodo, projection, cache, embedder_versions }

  impl EAstInstrument {
      pub fn new_with_weights(...) -&gt; InstrumentResult&lt;Self&gt; {
          // verify weights' SHA matches models_config.toml pin
          // construct ParameterRefs with requires_grad=false for every Var
          // guard.register_instrument(InstrumentSlot::EAst, &amp;params)?
          // Return Self
      }
  }

  impl Instrument for EAstInstrument {
      fn encode(&amp;self, input: &amp;CodeInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.chunks.is_empty() {
              return Err(InstrumentError::invalid("input.chunks", "no chunks", "supply at least one AST-perfect chunk"));
          }
          let mut acc = vec![0.0_f32; 1536];
          let mut total_weight = 0.0_f32;
          for chunk in &amp;input.chunks {
              let key = (chunk.sha256, self.embedder_versions.e7.clone());
              let chunk_vec = if let Some(v) = self.cache.lock().get(&amp;key) {
                  v.clone()
              } else {
                  // no_grad scope
                  let v = self.qodo.forward_no_grad(chunk.text.as_str())?;
                  self.cache.lock().put(key, v.clone());
                  v
              };
              let weight = non_whitespace_char_count(&amp;chunk.text) as f32;
              for (i, x) in chunk_vec.iter().enumerate() { acc[i] += weight * x; }
              total_weight += weight;
          }
          if total_weight == 0.0 {
              return Err(InstrumentError::invalid("input.chunks", "all chunks have zero non-whitespace chars", "supply non-empty chunks"));
          }
          for x in &amp;mut acc { *x /= total_weight; }
          let projected = self.projection.forward_no_grad(&amp;acc)?;  // 384-d
          validate_finite_output("e_ast.output", &amp;projected)?;
          Ok(projected)
      }
  }

tests:
  - encode_empty_chunks_rejected
  - encode_per_language_python_rust_js_ts_go_java_c_cpp_csharp_ruby_php
  - encode_byte_equal_across_calls
  - encode_caches_repeated_chunk
  - encode_fails_when_qodo_missing (with mock missing weight)
  - encode_output_finite_and_dim_384
</pseudo_code>

<files_to_create>
  <!-- optionally: crates/context-graph-mejepa-instruments/src/e_ast/ if file exceeds 500 lines after this work -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_code.rs">Replace EAstInstrument's deterministic encode with neural backing; update CodeInstrumentInput to typed AstChunk; wire FrozenGuard registration</file>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Re-export updated CodeInstrumentInput / AstPerfectChunk if surface change is required</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 384 finite f32</criterion>
  <criterion>11 languages encode end-to-end (Python, Rust, JS, TS, Go, Java, C, C++, C#, Ruby, PHP)</criterion>
  <criterion>Encode is deterministic (byte-equal across calls)</criterion>
  <criterion>Cache hit on repeat chunk skips Qodo forward (verify via cache hit counter)</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>Phase 3 gradient hook reports zero gradient through E7 + projection weights</criterion>
  <criterion>cargo build --release -p context-graph-mejepa-instruments clean</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-instruments -- -D warnings clean</criterion>
  <criterion>Latency p50 &lt; 5 ms warm, p99 &lt; 20 ms cold (per NFR-INSTR-02)</criterion>
  <criterion>Empty chunks input rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT</criterion>
  <criterion>Missing E7 weights → MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_ast::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments e_ast_per_language</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

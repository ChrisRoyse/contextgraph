# TASK-INSTR-014 — EDiffInstrument neural backing (Qodo + diff-edge encoder)

```xml
<task_spec id="TASK-INSTR-014" version="1.0">

<metadata>
  <title>EDiffInstrument neural backing — frozen E7 Qodo + diff-edge encoder over (before, after, edges) -&gt; 256-d</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-INSTR-07">EDiffInstrument 256-d (frozen Qodo + diff-edge encoder)</item>
    <item ref="REQ-INSTR-19">AST-perfect chunk consumption</item>
    <item ref="component:EDiffInstrument">encode(DiffInstrumentInput) -&gt; 256 f32</item>
    <item ref="data_model:DiffInstrumentInput">before, after, edges</item>
    <item ref="data_model:DiffEdge">added/removed/modified node events</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
    <task_ref>TASK-INSTR-004</task_ref>
    <task_ref>TASK-INSTR-008</task_ref>
    <task_ref>TASK-AST-022</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file modified, 0-1 file created</estimated_files>
</metadata>

<context>
Per REQ-INSTR-07, EDiffInstrument consumes a (pre-patch chunk set, post-patch chunk set) pair plus
computed AST-diff edges (added/removed/modified node events) and emits a 256-d latent. Backed by
frozen E7 Qodo (the same as E_AST) plus a diff-edge encoder. Mean-pool over edge embeddings.

Reuses the Qodo forward path from TASK-INSTR-008 (EAstInstrument); the diff-edge encoder is the
incremental neural component. The cache (file-SHA-keyed) is shared with EAstInstrument across
chunks: a pre-patch chunk encoded by EAstInstrument is cached and reused here.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-07, REQ-INSTR-19)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EDiffInstrument; data_models DiffInstrumentInput, DiffEdge)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_code.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
  <file purpose="qodo_forward_path">crates/context-graph-mejepa-instruments/src/e_code.rs (EAstInstrument; TASK-INSTR-008)</file>
  <file purpose="ast_chunker_facade">crates/context-graph-core/src/memory/ast_chunker/</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>TASK-INSTR-008 done (Qodo forward path reusable)</check>
  <check>TASK-AST-022 done</check>
  <check>E7 Qodo weights pinned</check>
  <check>Frozen diff-edge encoder weights checked into operator-controlled keyring</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace EDiffInstrument deterministic encode with neural backing
    - Encode before/after chunks via cached E7 Qodo (no_grad) -&gt; 1536-d each
    - For each diff edge (added/removed/modified node), compute (edge kind one-hot + before/after embedding diff)
    - Diff-edge encoder: linear 3 × 1536 -&gt; 256 -&gt; mean-pool over edges -&gt; 256-d
    - Empty `edges` allowed when before == after; emit zeroes (still finite); but must reject when edges and chunks are both empty
    - `EDiffInstrument::new_with_weights(qodo: &amp;Qodo, diff_encoder: &amp;DiffEdgeEncoder, guard: &amp;mut FrozenGuard)` constructor
    - File-SHA-keyed cache shared (or compatible) with EAstInstrument
    - Per-language tests
    - Determinism + frozen-gradient tests
  </in_scope>
  <out_of_scope>
    - Computing the AST diff (AST chunker / TASK-AST-022 produces edges)
    - Training Qodo or diff-edge encoder weights
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_code.rs">
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffInstrumentInput {
    pub before: CodeInstrumentInput,
    pub after: CodeInstrumentInput,
    pub edges: Vec&lt;DiffEdge&gt;,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffEdge {
    pub kind: DiffEdgeKind,         // Added | Removed | Modified
    pub before_chunk_idx: Option&lt;usize&gt;,
    pub after_chunk_idx: Option&lt;usize&gt;,
}

pub struct EDiffInstrument {
    qodo: Arc&lt;Qodo&gt;,
    diff_encoder: Arc&lt;DiffEdgeEncoder&gt;,
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl EDiffInstrument {
    pub fn new_with_weights(
        qodo: Arc&lt;Qodo&gt;,
        diff_encoder: Arc&lt;DiffEdgeEncoder&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for EDiffInstrument {
    type Input = DiffInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EDiff }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 finite f32
    - Frozen — FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Reject when before AND after are empty (no chunks)
    - Allow before == after with empty edges (emit deterministic zero-edge fallback latent; still finite; flagged in tests)
    - Latency budget: &lt; 20 ms cold per NFR-INSTR-02
    - Deterministic
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_diff::
    - cargo test --release -p context-graph-mejepa-instruments e_diff_per_language
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
e_code.rs:

  pub struct EDiffInstrument { qodo, diff_encoder, cache, embedder_versions }

  impl Instrument for EDiffInstrument {
      fn encode(&amp;self, input: &amp;DiffInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.before.chunks.is_empty() &amp;&amp; input.after.chunks.is_empty() {
              return Err(InstrumentError::invalid("input.{before,after}", "empty", "..."));
          }
          let before_embs: Vec&lt;Vec&lt;f32&gt;&gt; = encode_chunks_via_cache(&amp;input.before.chunks, &amp;self.qodo, &amp;self.cache, &amp;self.embedder_versions)?;
          let after_embs: Vec&lt;Vec&lt;f32&gt;&gt; = encode_chunks_via_cache(&amp;input.after.chunks, &amp;self.qodo, &amp;self.cache, &amp;self.embedder_versions)?;
          let edge_embeddings: Vec&lt;Vec&lt;f32&gt;&gt; = input.edges.iter().map(|e| {
              let before_v = e.before_chunk_idx.and_then(|i| before_embs.get(i)).cloned().unwrap_or_else(|| vec![0.0; 1536]);
              let after_v = e.after_chunk_idx.and_then(|i| after_embs.get(i)).cloned().unwrap_or_else(|| vec![0.0; 1536]);
              let kind_oh = encode_diff_edge_kind(e.kind); // 3-d
              self.diff_encoder.forward_no_grad(&amp;kind_oh, &amp;before_v, &amp;after_v) // 256-d
          }).collect::&lt;Result&lt;_, _&gt;&gt;()?;
          let pooled = if edge_embeddings.is_empty() {
              // before == after; deterministic zero-edge fallback
              vec![0.0_f32; 256]
          } else {
              mean_pool(&amp;edge_embeddings)
          };
          validate_finite_output("e_diff.output", &amp;pooled)?;
          Ok(pooled)
      }
  }

tests:
  - encode_empty_before_and_after_rejected
  - encode_no_edges_zero_pooled
  - encode_added_only
  - encode_removed_only
  - encode_modified_only
  - encode_byte_equal_across_calls
  - encode_output_finite_and_dim_256
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_code.rs">Replace EDiffInstrument deterministic encode with neural backing; share Qodo cache with EAstInstrument</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 256 finite f32</criterion>
  <criterion>before AND after empty rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency p99 &lt; 20 ms cold</criterion>
  <criterion>Cache hit on repeated chunk skips Qodo forward (shared across E_AST and E_Diff)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_diff::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments e_diff_per_language</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

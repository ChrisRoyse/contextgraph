# TASK-EMBED-013 — E8 Graph wrapper (e5-large-v2 graph use, 1024-d)

```xml
<task_spec id="TASK-EMBED-013" version="1.0">
<metadata>
  <title>EmbedderInstrument&lt;E8Graph&gt; sharing E1's e5-large-v2 weights but typed as the graph-domain embedder</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-EMBED-01">E8 wrapper</item>
    <item ref="TECH-EMBEDDERS::vram_budget_table (E8 reuses E1)">~44 MB delta when E1 already loaded</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
    <task_ref>TASK-EMBED-010</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
E8 (Graph, e5-large-v2 graph use, 1024-d) is structurally identical to E1 forward but typed as a separate EmbedderId so panel-slot wiring (E_Module = E1+E8 routing per REQ-EMBED-04) and routing logic stay clean. To avoid VRAM doubling, E8's factory checks if E1 is already loaded and reuses the underlying model handle (~44 MB delta vs ~650 MB fresh load).
</context>

<input_context_files>
  <file purpose="e1_template">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="vram_table">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#vram_budget_table</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-010 complete (E1 wrapper)</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E8GraphForward { e1_handle: Arc<E1SemanticForward>, /* shares weights */ }`
    - `impl EmbedderForward for E8GraphForward` — delegates to E1 forward but stamps `embedder_id: E8Graph`, `version: e5-large-v2-2024-11-15-graph-domain`
    - `E8GraphFactory::build` — checks `ensemble.loaded[E1Semantic]`; if present, clone Arc and reuse; if absent, load fresh
    - VRAM budget: when reusing, registered as `vram_estimate_bytes ≈ 44 MB` (delta only); when fresh, ~650 MB
    - Instrument impl with `slot() = EProblem` (informational)
    - Tests: when E1 resident, E8 load doesn't double VRAM; when E1 absent, E8 loads fresh and uses ~650 MB
  </in_scope>
  <out_of_scope>
    - Actual graph-context features (graph adjacency injection); v1 ships with same forward as E1 — graph specialisation is a future LoRA delta
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/e8_graph.rs">
      pub const E8_VERSION: &amp;str = "e5-large-v2-2024-11-15-graph";
      pub const E8_PROJECTED_DIM: usize = 1024;

      pub struct E8GraphForward { /* shares E1 model handle */ }
      impl EmbedderForward for E8GraphForward { /* delegates to E1 with E8 stamping */ }

      pub struct E8GraphFactory;
      impl EmbedderForwardFactory for E8GraphFactory {
          /* build checks ensemble.loaded for E1Semantic; reuses if present */
      }
      pub fn register();

      pub struct E8GraphInstrument { ensemble: Arc&lt;Ensemble&gt; }
      impl Instrument for E8GraphInstrument {
          fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EProblem }
          fn output_dim(&amp;self) -&gt; usize { E8_PROJECTED_DIM }
          /* ... */
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - When E1 resident, E8's vram_estimate_bytes accounted as ~44 MB; otherwise ~650 MB.
    - The forward path is structurally identical to E1 — only stamping changes.
    - VRAM accounting MUST reflect the delta correctly; tests verify reserve(E8) returns 44 MB when E1 resident.
    - NO duplicate weight load.
  </constraints>

  <verification>
    - When E1 resident, E8 load reserves ~44 MB delta.
    - Vector output identical to E1 on same input but stamped with E8 embedder_id.
    - cargo test e8_graph passes.
    - cargo clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
e8_graph.rs:
  - E8GraphForward holds Arc<E1SemanticForward>.
  - forward delegates; rewrites stamping fields (embedder_id, version, weight_sha unchanged).
  - Factory::build inspects ensemble.loaded async, reuses Arc if present.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/e8_graph.rs">E8GraphForward + Factory + Instrument + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod e8_graph;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>E8 forward output structurally identical to E1 on same input</criterion>
  <criterion>Stamping fields differ: E8 vs E1 in embedder_id</criterion>
  <criterion>VRAM delta when E1 resident is ~44 MB not ~650 MB</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders e8_graph</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

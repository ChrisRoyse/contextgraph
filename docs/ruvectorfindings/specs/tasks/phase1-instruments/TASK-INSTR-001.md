# TASK-INSTR-001 — Verify InstrumentSlot enum + layout (15 slots, 5,120-d)

```xml
<task_spec id="TASK-INSTR-001" version="1.0">

<metadata>
  <title>Verify InstrumentSlot enum + layout() (15 slots summing to 5,120-d)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-INSTR-15">Panel layout invariant: PANEL_DIM = 5,120 = sum of 15 slot dims</item>
    <item ref="component:InstrumentSlot">enum + layout() + all() + extent() + offset() + dim() + slug()</item>
    <item ref="AC-INSTR-01-01">Slot extents match canonical table</item>
    <item ref="EC-INSTR-02">filled_mask outside 15-slot mask rejected</item>
  </implements>
  <depends_on><!-- none; foundation --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
The `InstrumentSlot` enum and its `layout()` table are the canonical authority for the 5,120-d
panel byte layout. The shipped code (`crates/context-graph-mejepa-instruments/src/lib.rs` lines
104-216) ships 15 variants — `EAst, ECfg, EDataFlow, ETypeGraph, ETest, ETrace, EDiff, EWitness,
EOracle, EProblem, ECommitMsg, EStaticAnalysis, ERuntime, EReasoning, Scalars` — with dims summing
to exactly 5,120 (debug_assert in `layout()`). This task is verification + invariant
documentation: confirm signatures, dim sum, slug uniqueness, debug_assert hits, and add a
runtime-checked unit test (the existing `debug_assert_eq!(offset, PANEL_DIM)` only fires under
`debug_assertions`).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants, frozen-instrument anti_pattern)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-15, AC-INSTR-01-01, EC-INSTR-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (data_models InstrumentSlot, panel_layout_diagram)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/lib.rs (lines 96-216)</file>
  <file purpose="shipped_tests">crates/context-graph-mejepa-instruments/src/tests.rs (existing layout/slot tests)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files, PascalCase types)</file>
</input_context_files>

<prerequisites>
  <check>Crate `context-graph-mejepa-instruments` builds with `cargo build --release -p context-graph-mejepa-instruments`</check>
  <check>27 shipped tests pass with `cargo test -p context-graph-mejepa-instruments`</check>
  <check>No outstanding clippy warnings on the crate</check>
</prerequisites>

<scope>
  <in_scope>
    - Read `crates/context-graph-mejepa-instruments/src/lib.rs` lines 96-216
    - Confirm 15 enum variants in canonical doc-09 order
    - Confirm `layout()` returns `[(Self, usize, usize); 15]` with sum-of-dims == 5,120
    - Confirm `slug()` returns unique snake_case strings for every variant
    - Add a release-mode unit test that asserts `layout()` sum == `PANEL_DIM`
    - Add a release-mode unit test that asserts every slug is distinct
    - Add a release-mode unit test that asserts `from_slug()` round-trips every variant (introduce `InstrumentSlot::from_slug(&str) -> Option<Self>` if not present)
  </in_scope>
  <out_of_scope>
    - Adding new instrument variants (constitution version bump required per implementation_notes evolution rule)
    - Changing dims or order (would require predictor architecture changes)
    - JSON serialization round-trip (covered in TASK-INSTR-022)
    - PanelBuilder validation (covered in TASK-INSTR-003)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/lib.rs">
pub const PANEL_DIM: usize = 5_120;
const SLOT_COUNT: usize = 15;
const VALID_FILLED_MASK: u16 = (1u16 &lt;&lt; SLOT_COUNT) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentSlot {
    EAst, ECfg, EDataFlow, ETypeGraph, ETest, ETrace, EDiff, EWitness,
    EOracle, EProblem, ECommitMsg, EStaticAnalysis, ERuntime, EReasoning, Scalars,
}

impl InstrumentSlot {
    pub fn layout() -&gt; [(Self, usize, usize); 15];
    pub fn all() -&gt; [Self; 15];
    pub fn extent(&amp;self) -&gt; (usize, usize);
    pub fn offset(&amp;self) -&gt; usize;
    pub fn dim(&amp;self) -&gt; usize;
    pub fn slug(&amp;self) -&gt; &amp;'static str;
    pub fn from_slug(slug: &amp;str) -&gt; Option&lt;Self&gt;; // NEW
}
    </signature>
  </signatures>
  <constraints>
    - PANEL_DIM remains exactly 5,120; do not edit
    - 15 variants in the exact canonical order: EAst, ECfg, EDataFlow, ETypeGraph, ETest, ETrace, EDiff, EWitness, EOracle, EProblem, ECommitMsg, EStaticAnalysis, ERuntime, EReasoning, Scalars
    - Slot dim table: 384, 256, 256, 256, 384, 512, 256, 256, 128, 1024, 384, 256, 256, 384, 128 (sum = 5,120)
    - Slug strings: e_ast, e_cfg, e_data_flow, e_type_graph, e_test, e_trace, e_diff, e_witness, e_oracle, e_problem, e_commit_msg, e_static_analysis, e_runtime, e_reasoning, scalars
    - `from_slug` is an exhaustive `match` (no String allocation, no HashMap)
    - Existing 27 tests continue to pass; new tests are additive
    - Constitution coding_standards: snake_case files, PascalCase types, SCREAMING_SNAKE consts, no `unwrap_or` on confidence
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-instruments` succeeds
    - `cargo test -p context-graph-mejepa-instruments` shows tests >= 27 + 3 new = 30 passed
    - `cargo clippy -p context-graph-mejepa-instruments -- -D warnings` clean
    - New release-mode test `layout_sums_to_panel_dim` runs with `cargo test --release -p context-graph-mejepa-instruments`
  </verification>
</definition_of_done>

<pseudo_code>
add to lib.rs (or extend impl InstrumentSlot block):

  pub fn from_slug(slug: &amp;str) -&gt; Option&lt;Self&gt; {
      // Exhaustive match over slug strings
      match slug {
          "e_ast" =&gt; Some(Self::EAst),
          "e_cfg" =&gt; Some(Self::ECfg),
          ...
          "scalars" =&gt; Some(Self::Scalars),
          _ =&gt; None,
      }
  }

add to tests.rs:

  #[test]
  fn layout_sums_to_panel_dim_release() {
      // duplicates the debug_assert in layout() but runs in release mode
      let total: usize = InstrumentSlot::layout().iter().map(|(_, _, d)| *d).sum();
      assert_eq!(total, PANEL_DIM);
      assert_eq!(total, 5_120);
  }

  #[test]
  fn slugs_are_unique() {
      let mut slugs: Vec&lt;&amp;'static str&gt; = InstrumentSlot::all().iter().map(|s| s.slug()).collect();
      slugs.sort_unstable();
      let n = slugs.len();
      slugs.dedup();
      assert_eq!(slugs.len(), n, "duplicate slug detected");
  }

  #[test]
  fn from_slug_round_trip() {
      for slot in InstrumentSlot::all() {
          assert_eq!(InstrumentSlot::from_slug(slot.slug()), Some(slot));
      }
      assert_eq!(InstrumentSlot::from_slug("nonexistent"), None);
      assert_eq!(InstrumentSlot::from_slug(""), None);
  }
</pseudo_code>

<files_to_create>
  <!-- none; verification + small additive changes to existing files -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Add `pub fn from_slug(slug: &amp;str) -&gt; Option&lt;Self&gt;` to `impl InstrumentSlot` (insert after `slug()` method circa line 215)</file>
  <file path="crates/context-graph-mejepa-instruments/src/tests.rs">Add three release-mode tests: `layout_sums_to_panel_dim_release`, `slugs_are_unique`, `from_slug_round_trip`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-instruments succeeds with no warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-instruments shows >= 30 passing tests</criterion>
  <criterion>cargo test --release -p context-graph-mejepa-instruments shows the new release-mode tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-instruments -- -D warnings is clean</criterion>
  <criterion>InstrumentSlot::layout() returns a 15-tuple summing to PANEL_DIM in release mode</criterion>
  <criterion>InstrumentSlot::from_slug round-trips all 15 slugs and rejects unknown slugs</criterion>
  <criterion>No edits to PANEL_DIM, SLOT_COUNT, VALID_FILLED_MASK, or existing variant order</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments</command>
  <command>cargo test --release -p context-graph-mejepa-instruments layout_sums_to_panel_dim_release slugs_are_unique from_slug_round_trip</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

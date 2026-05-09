# TASK-INSTR-003 — Verify Panel + PanelBuilder + PanelSerde fail-closed gate

```xml
<task_spec id="TASK-INSTR-003" version="1.0">

<metadata>
  <title>Verify Panel + PanelBuilder + PanelSerde invariant gate (fail-closed at every JSON ingress)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-INSTR-15">PanelBuilder fail-closed (dim mismatch, NaN/Inf, double-fill, empty-panel, malformed persisted state)</item>
    <item ref="component:Panel">try_new, data, filled_mask, slot, is_filled</item>
    <item ref="component:PanelBuilder">new, set_slot, set_from_instrument, build, require_slots</item>
    <item ref="component:PanelSerde">try_from invariant gate</item>
    <item ref="BR-INSTR-03">double-fill rejected</item>
    <item ref="BR-INSTR-04">non-finite rejected</item>
    <item ref="BR-INSTR-06">filled_mask reflects only explicitly-set slots</item>
    <item ref="EC-INSTR-01">empty panel rejected</item>
    <item ref="EC-INSTR-02">unknown filled_mask bits rejected</item>
    <item ref="ERR-INSTR-08">data length mismatch rejected</item>
    <item ref="ERR-INSTR-09">double-fill rejected</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-001</task_ref>
    <task_ref>TASK-INSTR-002</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
`Panel`, `PanelBuilder`, and `PanelSerde` are shipped (`lib.rs` lines 240-465). The serde shim
uses `#[serde(try_from = "PanelSerde", into = "PanelSerde")]` so deserialization always re-runs
`validate_panel_parts` — the single chokepoint preventing malformed JSON from constructing a
Panel via raw deserialize. This task is verification + invariant documentation: confirm signatures
+ behavior + serde gate, and add tests for cases NOT yet covered in the shipped 27 tests
(double-fill regression, NaN-on-build, JSON deserialize with bad mask, JSON serialize/deserialize
round-trip with all-15-filled panel).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_INFER_DIM_MISMATCH)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-15, BR-INSTR-03/04/06, EC-INSTR-01/02, ERR-INSTR-01/02/08/09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts Panel/PanelBuilder)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/lib.rs (lines 240-465)</file>
  <file purpose="shipped_tests">crates/context-graph-mejepa-instruments/src/tests.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-001 + TASK-INSTR-002 verified (slot enum + trait stable)</check>
  <check>27 shipped tests pass</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped Panel/PanelBuilder/PanelSerde signatures match TECH-INSTRUMENTS exactly
    - Verify `serde(try_from = "PanelSerde", into = "PanelSerde")` invariant gate is wired
    - Add unit test: malicious JSON `{"data": [...5120 zeros...], "filled_mask": 0xFFFF}` rejected (bit 15 invalid)
    - Add unit test: malicious JSON with NaN inside data rejected
    - Add unit test: JSON with len != 5,120 rejected
    - Add unit test: empty filled_mask (0) rejected
    - Add unit test: full-panel set + serialize + deserialize round-trip preserves data + filled_mask
    - Add unit test: PanelBuilder.set_slot called twice for same slot returns InvalidInput with code MEJEPA_INSTRUMENTS_INVALID_INPUT
    - Add unit test: PanelBuilder.set_slot with NaN returns NumericalInvariant
    - Add unit test: PanelBuilder.build() with zero filled slots returns InvalidInput
    - Add unit test: require_slots reports the FIRST missing slot in canonical order
  </in_scope>
  <out_of_scope>
    - Changing serde shim shape (would break stored panels)
    - Provenance metadata fields (TASK-INSTR-022)
    - Tier compression (Phase 6)
    - Per-instrument encode tests (TASK-INSTR-005..019)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/lib.rs">
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PanelSerde", into = "PanelSerde")]
pub struct Panel { data: Vec&lt;f32&gt;, filled_mask: u16 }

impl Panel {
    pub fn try_new(data: Vec&lt;f32&gt;, filled_mask: u16) -&gt; InstrumentResult&lt;Self&gt;;
    pub fn data(&amp;self) -&gt; &amp;[f32];
    pub fn filled_mask(&amp;self) -&gt; u16;
    pub fn slot(&amp;self, slot: InstrumentSlot) -&gt; &amp;[f32];
    pub fn is_filled(&amp;self, slot: InstrumentSlot) -&gt; bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PanelSerde { data: Vec&lt;f32&gt;, filled_mask: u16 }

#[derive(Debug, Clone)]
pub struct PanelBuilder { data: Vec&lt;f32&gt;, filled_mask: u16 }

impl PanelBuilder {
    pub fn new() -&gt; Self;
    pub fn set_slot(&amp;mut self, slot: InstrumentSlot, vector: &amp;[f32]) -&gt; InstrumentResult&lt;()&gt;;
    pub fn set_from_instrument&lt;I, In&gt;(&amp;mut self, instrument: &amp;I, input: &amp;In) -&gt; InstrumentResult&lt;()&gt;
        where I: Instrument&lt;Input = In&gt;;
    pub fn build(self) -&gt; InstrumentResult&lt;Panel&gt;;
    pub fn require_slots(&amp;self, required_slots: &amp;[InstrumentSlot]) -&gt; InstrumentResult&lt;()&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - try_from gate runs on every JSON deserialize (no raw deserialize bypass)
    - empty filled_mask (0) is rejected — prevents indistinguishable "missing" panels
    - filled_mask &amp; !VALID_FILLED_MASK == 0 (no bits beyond the 15-slot mask)
    - data.len() == PANEL_DIM (5,120)
    - every element finite
    - double-fill returns InvalidInput with field "slot"
    - NaN/Inf returns NumericalInvariant
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa-instruments panel_round_trip_all_filled`
    - `cargo test -p context-graph-mejepa-instruments panel_serde_rejects_bad_mask`
    - `cargo test -p context-graph-mejepa-instruments panel_serde_rejects_nan`
    - `cargo test -p context-graph-mejepa-instruments panel_serde_rejects_wrong_length`
    - `cargo test -p context-graph-mejepa-instruments panel_builder_double_fill_rejected`
  </verification>
</definition_of_done>

<pseudo_code>
add to tests.rs:

  #[test]
  fn panel_serde_rejects_bad_mask() {
      let json = format!(
          r#"{{"data":[{}], "filled_mask": 65535}}"#,  // bit 15 = invalid
          (0..5120).map(|_| "0.0").collect::&lt;Vec&lt;_&gt;&gt;().join(","),
      );
      let err = serde_json::from_str::&lt;Panel&gt;(&amp;json).unwrap_err();
      assert!(format!("{err}").contains("filled_mask"));
  }

  #[test]
  fn panel_serde_rejects_nan() {
      let mut data = vec![0.0_f32; 5120];
      data[42] = f32::NAN;
      let panel_serde = PanelSerde { data, filled_mask: 1 };
      let err = Panel::try_from(panel_serde).unwrap_err();
      assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT");
  }

  #[test]
  fn panel_serde_rejects_wrong_length() {
      let panel_serde = PanelSerde { data: vec![0.0; 5119], filled_mask: 1 };
      let err = Panel::try_from(panel_serde).unwrap_err();
      assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
  }

  #[test]
  fn panel_builder_double_fill_rejected() {
      let mut b = PanelBuilder::new();
      b.set_slot(InstrumentSlot::EOracle, &amp;vec![0.0; 128]).unwrap();
      let err = b.set_slot(InstrumentSlot::EOracle, &amp;vec![1.0; 128]).unwrap_err();
      assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
      assert!(format!("{err}").contains("already filled"));
  }

  #[test]
  fn panel_builder_empty_panel_rejected() {
      let b = PanelBuilder::new();
      let err = b.build().unwrap_err();
      assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
      assert!(format!("{err}").contains("filled_mask"));
  }

  #[test]
  fn panel_round_trip_all_filled() {
      let mut b = PanelBuilder::new();
      for slot in InstrumentSlot::all() {
          let v = vec![0.5_f32; slot.dim()];
          b.set_slot(slot, &amp;v).unwrap();
      }
      let panel = b.build().unwrap();
      let json = serde_json::to_string(&amp;panel).unwrap();
      let panel2: Panel = serde_json::from_str(&amp;json).unwrap();
      assert_eq!(panel.data(), panel2.data());
      assert_eq!(panel.filled_mask(), panel2.filled_mask());
      assert_eq!(panel.filled_mask(), VALID_FILLED_MASK);
  }

  #[test]
  fn require_slots_reports_first_missing_in_canonical_order() {
      let mut b = PanelBuilder::new();
      b.set_slot(InstrumentSlot::EOracle, &amp;vec![0.0; 128]).unwrap();
      let err = b.require_slots(&amp;InstrumentSlot::all()).unwrap_err();
      assert!(format!("{err}").contains("EAst"));  // first in canonical order
  }
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/tests.rs">Add 6 panel/builder validation tests covering serde gate + double-fill + empty panel + round-trip + require_slots ordering</file>
</files_to_modify>

<validation_criteria>
  <criterion>JSON ingress always runs validate_panel_parts</criterion>
  <criterion>Empty filled_mask rejected at build()</criterion>
  <criterion>Bits >= 15 rejected on deserialize</criterion>
  <criterion>NaN/Inf rejected at every entry point (set_slot, try_new, deserialize)</criterion>
  <criterion>Double-fill rejected with "already filled" message</criterion>
  <criterion>Full-panel JSON round-trip is byte-equal in data + filled_mask</criterion>
  <criterion>require_slots reports first missing in canonical order</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments panel_</command>
  <command>cargo test -p context-graph-mejepa-instruments require_slots_reports_first_missing_in_canonical_order</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

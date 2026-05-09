# TASK-INSTR-006 — EWitnessInstrument verify + edge-case audit

```xml
<task_spec id="TASK-INSTR-006" version="1.0">

<metadata>
  <title>EWitnessInstrument verify (shipped) + edge-case audit (empty chain, format mismatch, replay break)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-INSTR-08">EWitnessInstrument 256-d</item>
    <item ref="component:EWitnessInstrument">encode(WitnessChainInput) -> 256 f32</item>
    <item ref="BR-INSTR-05">verify_chain_bytes failure → encode refuses</item>
    <item ref="EC-INSTR-05">empty chain rejected</item>
    <item ref="EC-INSTR-06">format_version mismatch rejected</item>
    <item ref="ERR-INSTR-06">chain verification failure → InvalidInput</item>
    <item ref="ERR-INSTR-07">format_version != CANONICAL_WITNESS_FORMAT_VERSION → InvalidInput</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
`EWitnessInstrument` is shipped at `e_witness.rs`. It calls
`context_graph_witness::verify_chain_bytes` so missing provenance cannot masquerade as an
all-zero slot. Constants: `CANONICAL_WITNESS_FORMAT_VERSION = 1`, `MAX_WITNESS_ENTRIES = 65_536`.
This task verifies the shipped contract + adds tests for edge cases not yet exhaustively covered.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-08, BR-INSTR-05, EC-INSTR-05/06, ERR-INSTR-06/07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EWitnessInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_witness.rs</file>
  <file purpose="witness_primitive">crates/context-graph-witness/src/lib.rs (verify_chain_bytes, WitnessEntry, WITNESS_ENTRY_SIZE = 73, HASH_SIZE)</file>
  <file purpose="constitution">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_OBSERVE_INTEGRITY_VIOLATION)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003 done</check>
  <check>context-graph-witness builds; verify_chain_bytes available</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `pub struct EWitnessInstrument` + `WitnessChainInput { format_version: u8, chain_bytes: Vec&lt;u8&gt; }`
    - Verify `pub const CANONICAL_WITNESS_FORMAT_VERSION: u8 = 1`
    - Verify `MAX_WITNESS_ENTRIES = 65_536` enforcement
    - Add test: empty chain_bytes rejected (EC-INSTR-05)
    - Add test: format_version = 0 rejected (EC-INSTR-06)
    - Add test: format_version = 2 rejected
    - Add test: chain_bytes len not multiple of WITNESS_ENTRY_SIZE rejected
    - Add test: corrupted chain (last byte flipped) rejected by verify_chain_bytes (ERR-INSTR-06)
    - Add test: chain with 65,536 entries accepted; 65,537 entries rejected
    - Add test: deterministic encode (same input twice → byte-equal)
    - Add test: WitnessChainInput::from_entries with empty entries returns InvalidInput
  </in_scope>
  <out_of_scope>
    - Modifying verify_chain_bytes behavior (lives in context-graph-witness)
    - Stage 0 / Stage F / Merkle cert wiring (already shipped via context-graph-witness)
    - linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_witness.rs">
pub const CANONICAL_WITNESS_FORMAT_VERSION: u8 = 1;
const MAX_WITNESS_ENTRIES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessChainInput {
    pub format_version: u8,
    pub chain_bytes: Vec&lt;u8&gt;,
}

impl WitnessChainInput {
    pub fn from_entries(entries: &amp;[WitnessEntry]) -&gt; InstrumentResult&lt;Self&gt;;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EWitnessInstrument;

impl Instrument for EWitnessInstrument {
    type Input = WitnessChainInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EWitness }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;;  // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 f32, finite
    - Reject empty chain
    - Reject format_version != CANONICAL_WITNESS_FORMAT_VERSION
    - Reject chain_bytes.len() % WITNESS_ENTRY_SIZE != 0
    - Reject &gt; MAX_WITNESS_ENTRIES entries
    - Refuse if verify_chain_bytes returns Err
    - Deterministic
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_witness:: passes >= 8 tests
  </verification>
</definition_of_done>

<pseudo_code>
add to e_witness.rs tests module:

  use context_graph_witness::{WitnessEntry, WITNESS_ENTRY_SIZE};

  fn build_valid_chain(n: usize) -&gt; Vec&lt;u8&gt; {
      // construct n valid 73-byte entries chained via prev_hash; reuse helpers from context-graph-witness tests
      ...
  }

  #[test]
  fn empty_chain_rejected() {
      let input = WitnessChainInput { format_version: 1, chain_bytes: vec![] };
      let err = EWitnessInstrument.encode(&amp;input).unwrap_err();
      assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
  }

  #[test]
  fn format_version_zero_rejected() {
      let bytes = build_valid_chain(2);
      let input = WitnessChainInput { format_version: 0, chain_bytes: bytes };
      assert!(EWitnessInstrument.encode(&amp;input).is_err());
  }

  #[test]
  fn format_version_two_rejected() {
      let bytes = build_valid_chain(2);
      let input = WitnessChainInput { format_version: 2, chain_bytes: bytes };
      assert!(EWitnessInstrument.encode(&amp;input).is_err());
  }

  #[test]
  fn chain_bytes_misaligned_rejected() {
      let mut bytes = build_valid_chain(2);
      bytes.push(0u8);  // off by one byte; not multiple of 73
      let input = WitnessChainInput { format_version: 1, chain_bytes: bytes };
      assert!(EWitnessInstrument.encode(&amp;input).is_err());
  }

  #[test]
  fn corrupted_chain_rejected_by_verify() {
      let mut bytes = build_valid_chain(3);
      let last = bytes.len() - 1;
      bytes[last] ^= 0xFF;
      let input = WitnessChainInput { format_version: 1, chain_bytes: bytes };
      assert!(EWitnessInstrument.encode(&amp;input).is_err());
  }

  #[test]
  fn max_entries_boundary() {
      let bytes_ok = build_valid_chain(MAX_WITNESS_ENTRIES);
      let input_ok = WitnessChainInput { format_version: 1, chain_bytes: bytes_ok };
      assert!(EWitnessInstrument.encode(&amp;input_ok).is_ok());
      let bytes_over = build_valid_chain(MAX_WITNESS_ENTRIES + 1);
      let input_over = WitnessChainInput { format_version: 1, chain_bytes: bytes_over };
      assert!(EWitnessInstrument.encode(&amp;input_over).is_err());
  }

  #[test]
  fn deterministic_encode_byte_equal() {
      let bytes = build_valid_chain(5);
      let input = WitnessChainInput { format_version: 1, chain_bytes: bytes };
      let a = EWitnessInstrument.encode(&amp;input).unwrap();
      let b = EWitnessInstrument.encode(&amp;input).unwrap();
      assert_eq!(a, b);
  }

  #[test]
  fn from_entries_empty_rejected() {
      let entries: Vec&lt;WitnessEntry&gt; = vec![];
      assert!(WitnessChainInput::from_entries(&amp;entries).is_err());
  }
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_witness.rs">Add 8 edge-case tests + chain-builder test helper</file>
</files_to_modify>

<validation_criteria>
  <criterion>Empty chain rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT</criterion>
  <criterion>format_version 0 and 2 rejected</criterion>
  <criterion>Misaligned chain bytes rejected</criterion>
  <criterion>Corrupted chain rejected by verify_chain_bytes</criterion>
  <criterion>65,536 entries pass; 65,537 entries fail</criterion>
  <criterion>Output is 256 finite f32 on valid input</criterion>
  <criterion>Encode is deterministic</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_witness::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

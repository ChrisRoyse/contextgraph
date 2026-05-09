# TASK-EMBED-006 — RoutingTable + route_for_entity_type

```xml
<task_spec id="TASK-EMBED-006" version="1.0">
<metadata>
  <title>Const-static RoutingTable mapping (EntityType, Option&lt;Language&gt;) → RoutingResult; fail-closed lookup</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-EMBED-04">per-chunk-type routing table</item>
    <item ref="BR-EMBED-02">fail closed on missing entry</item>
    <item ref="TECH-EMBEDDERS::component_contracts::RoutingTable">routing.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-002</task_ref>
    <task_ref>TASK-EMBED-003</task_ref>
    <task_ref>TASK-EMBED-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The static dispatch table mapping AST `EntityType` → active embedder set. Critical-path fail-closed: any unknown variant returns `MEJEPA_EMBED_ROUTING_MISSING`. No silent default. Looked up O(1) via const lookup-table backed by a sorted const slice. The `Language` parameter allows future per-language deviations (e.g., Python-specific routes); the default table applies when language is None.
</context>

<input_context_files>
  <file purpose="entity_type_enum">crates/context-graph-core/src/memory/ast_chunker.rs</file>
  <file purpose="instrument_slot">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="routing_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#component_contracts</file>
  <file purpose="must_cover_table">docs/ruvectorfindings/specs/functional/SPEC-EMBEDDERS.md#requirements (REQ-EMBED-04)</file>
  <file purpose="embedder_id">crates/context-graph-mejepa-embedders/src/embedder_id.rs</file>
  <file purpose="types">crates/context-graph-mejepa-embedders/src/types.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-002, 003, 005 complete</check>
  <check>`EntityType` enum exposed by `context-graph-core`</check>
  <check>`Language` enum present (Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php — 11 variants)</check>
</prerequisites>

<scope>
  <in_scope>
    - `EntityTypeKey` enum mirroring AST chunker variants but normalised (Function {is_test, signature_only}, Class, Struct, Module, Import, Comment, Docstring, TestAssertion, DiffHunk, AstNodeSequence, CfgBasicBlock, DefUseEdge — 13 keys total)
    - `Language` re-exported from `context-graph-core`
    - `const ROUTING_TABLE: &[(EntityTypeKey, Option<Language>, RoutingResult)]` populated with the 13 must-cover entries from REQ-EMBED-04 in canonical order
    - `pub fn route_for_entity_type(et: &EntityType, lang: Option<Language>) -> EmbedResult<RoutingResult>` performing exact-match-first then `(key, None)` fallback then `MEJEPA_EMBED_ROUTING_MISSING`
    - `pub fn coverage_audit() -> Vec<(EntityTypeKey, Language)>` returning all uncovered (key, language) pairs (used by FSV harness in TASK-EMBED-024)
    - Unit tests:
      - {Function body non-test} → {E7, E14}
      - {Function signature only} → {E1, E10}
      - {Function is_test=true} → {E1, E7}
      - {Class} → {E11, E7}; {Struct} → {E11, E7}
      - {Module} → {E1, E8}; {Import} → {E11, E13}
      - {Comment} → {E1, E10}; {Docstring} → {E1, E10}
      - {TestAssertion} → {E5, E1}
      - {DiffHunk, AstNodeSequence, CfgBasicBlock, DefUseEdge} → HandledByInstrument(EDiff/EAst/ECfg/EDataFlow)
      - Unknown EntityType variant → MEJEPA_EMBED_ROUTING_MISSING
      - Property test: every (EntityTypeKey × Language) pair returns Embedders or HandledByInstrument; never None
    - Coverage table emitted as JSON when `--features routing-coverage-dump` is set (used by FSV)
  </in_scope>
  <out_of_scope>
    - Per-language deviations (the table accepts `Some(Language)` keys but the v1 spec ships only `None` defaults)
    - Wiring the routing table into the panel materialization pipeline (TASK-EMBED-009 owns Ensemble; routing is consumed downstream)
    - The MCP tool `mejepa_routing_lookup` (TASK-EMBED-023)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/routing.rs">
      use std::collections::BTreeSet;

      use context_graph_core::ast_chunker::{EntityType, Language};
      use context_graph_mejepa_instruments::InstrumentSlot;

      use crate::EmbedderId;
      use crate::error::{EmbedError, EmbedResult};
      use crate::types::RoutingResult;

      #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
      pub enum EntityTypeKey {
          FunctionBody,         // is_test=false, signature_only=false
          FunctionSignature,    // signature_only=true
          FunctionTest,         // is_test=true
          Class,
          Struct,
          Module,
          Import,
          Comment,
          Docstring,
          TestAssertion,
          DiffHunk,
          AstNodeSequence,
          CfgBasicBlock,
          DefUseEdge,
      }

      impl EntityTypeKey {
          pub fn from_entity_type(et: &amp;EntityType) -&gt; EmbedResult&lt;Self&gt;;
      }

      /// 13 canonical routes, language-agnostic. Per REQ-EMBED-04.
      pub const ROUTING_TABLE: &amp;[(EntityTypeKey, Option&lt;Language&gt;, RoutingTableEntry)] = &amp;[
          (EntityTypeKey::FunctionBody,      None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E7Code, EmbedderId::E14BgeM3Dense])),
          (EntityTypeKey::FunctionSignature, None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E1Semantic, EmbedderId::E10Contextual])),
          (EntityTypeKey::FunctionTest,      None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E1Semantic, EmbedderId::E7Code])),
          (EntityTypeKey::Class,             None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E11Kepler, EmbedderId::E7Code])),
          (EntityTypeKey::Struct,            None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E11Kepler, EmbedderId::E7Code])),
          (EntityTypeKey::Module,            None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E1Semantic, EmbedderId::E8Graph])),
          (EntityTypeKey::Import,            None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E11Kepler, EmbedderId::E13Splade])),
          (EntityTypeKey::Comment,           None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E1Semantic, EmbedderId::E10Contextual])),
          (EntityTypeKey::Docstring,         None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E1Semantic, EmbedderId::E10Contextual])),
          (EntityTypeKey::TestAssertion,     None, RoutingTableEntry::Embedders(&amp;[EmbedderId::E5Causal, EmbedderId::E1Semantic])),
          (EntityTypeKey::DiffHunk,          None, RoutingTableEntry::HandledByInstrument(InstrumentSlot::EDiff)),
          (EntityTypeKey::AstNodeSequence,   None, RoutingTableEntry::HandledByInstrument(InstrumentSlot::EAst)),
          (EntityTypeKey::CfgBasicBlock,     None, RoutingTableEntry::HandledByInstrument(InstrumentSlot::ECfg)),
          (EntityTypeKey::DefUseEdge,        None, RoutingTableEntry::HandledByInstrument(InstrumentSlot::EDataFlow)),
      ];

      pub enum RoutingTableEntry {
          Embedders(&amp;'static [EmbedderId]),
          HandledByInstrument(InstrumentSlot),
      }

      pub fn route_for_entity_type(et: &amp;EntityType, lang: Option&lt;Language&gt;) -&gt; EmbedResult&lt;RoutingResult&gt;;

      pub fn coverage_audit() -&gt; Vec&lt;(EntityTypeKey, Option&lt;Language&gt;)&gt;;
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - Routing table is `const` / `static` — no runtime construction, no env var override (per REQ-EMBED-04 constraints).
    - Lookup MUST try (key, Some(lang)) first, then (key, None), then fail closed.
    - Missing-key returns `MEJEPA_EMBED_ROUTING_MISSING { entity_type, language, chunk_sha = "" }` — chunk_sha may be filled by caller.
    - `EntityTypeKey::from_entity_type` MUST handle every variant of upstream `EntityType` enum; new variants without a route → `MEJEPA_EMBED_ROUTING_MISSING`.
    - Embedder set construction returns `BTreeSet&lt;EmbedderId&gt;` for canonical ordering (deterministic encoding).
    - NO silent default fallback to "all embedders" or "E1 only".
    - Coverage audit walks all `(EntityTypeKey × Language)` pairs and returns ones not in the table — used by FSV.
    - All BTreeSet contents non-empty (validated at build via debug_assert).
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders routing::tests` passes.
    - 13 must-cover routes return expected sets/instruments byte-equal.
    - Property test: every EntityTypeKey resolves to a non-None RoutingResult.
    - Unknown entity variant returns `MEJEPA_EMBED_ROUTING_MISSING`.
    - `cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
routing.rs:
  - EntityTypeKey enum with 14 variants (13 routes + future-extensible).
  - from_entity_type: match upstream EntityType arms; map to EntityTypeKey or return MEJEPA_EMBED_ROUTING_MISSING.
  - ROUTING_TABLE const array with the 13 routes.
  - route_for_entity_type:
    1. key = EntityTypeKey::from_entity_type(et)?
    2. iter() find (key, Some(lang))
    3. else find (key, None)
    4. else return MEJEPA_EMBED_ROUTING_MISSING
    5. convert RoutingTableEntry → RoutingResult (BTreeSet from slice)
  - coverage_audit: iterate all 14 EntityTypeKey × 11 Language pairs; return ones not in ROUTING_TABLE (with None fallback considered).
  - Tests cover all 13 routes + unknown + property test.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/routing.rs">RoutingTable + route_for_entity_type + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod routing;` + `pub use routing::{route_for_entity_type, EntityTypeKey, ROUTING_TABLE};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>13 must-cover routes byte-equal with REQ-EMBED-04 table</criterion>
  <criterion>Unknown EntityType returns MEJEPA_EMBED_ROUTING_MISSING</criterion>
  <criterion>Property test: every EntityTypeKey × Language resolves to RoutingResult</criterion>
  <criterion>Routing table is `const` (compile-time constant)</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders routing</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```

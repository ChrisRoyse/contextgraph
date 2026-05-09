# TASK-AST-003 — Define `AstChunk` + `ParseDiagnostic` structs with serde

```xml
<task_spec id="TASK-AST-003" version="1.0">
<metadata>
  <title>Define AstChunk + ParseDiagnostic structs with deterministic serde serialization</title>
  <status>blocked</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-AST-02">9-field chunk metadata tuple</item>
    <item ref="REQ-AST-08">ParseDiagnostic shape — kind/is_error/is_missing/byte/line/column/excerpt</item>
    <item ref="REQ-AST-13">Deterministic JSON serialization (BTreeSet ordering)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-001</task_ref>
    <task_ref>TASK-AST-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
AstChunk is the public output of the chunker, consumed by panel materializer, mutation
corpus, TCT centroid builder, verify(), and shift-subscriber. ParseDiagnostic surfaces
tree-sitter ERROR/MISSING nodes with byte-offset precision. Both serialize to canonical
JSON for FSV evidence (REQ-AST-07) and RocksDB persistence. Must be deterministic across
runs — no HashMap, no system time, no PID.
</context>

<input_context_files>
  <file purpose="data_model_spec">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (AstChunk + ParseDiagnostic models)</file>
  <file purpose="legacy_diagnostic">crates/context-graph-core/src/memory/ast_chunker.rs (lines 53-76; legacy AstParseDiagnostic to mirror exactly except renamed to ParseDiagnostic)</file>
  <file purpose="embedder_id_type">crates/context-graph-embeddings/src/types/model_id/core.rs (existing ModelId enum — reuse as EmbedderId or alias for now; full re-architecture in TASK-EMBED-* phase)</file>
  <file purpose="anti_pattern_btreemap">docs/ruvectorfindings/specs/constitution.md (Determinism rule — BTreeMap only)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-001 complete: Language enum exists at memory/ast_chunker/language.rs</check>
  <check>TASK-AST-002 complete: EntityType enum exists at memory/ast_chunker/entity_type.rs</check>
  <check>EmbedderId type is importable (define a minimal type-alias placeholder if cross-crate dependency is not ready: `pub type EmbedderId = u8;` documented as "TODO replace with SPEC-EMBEDDERS enum in TASK-EMBED-001"; this lets the AST-CHUNKING phase ship without blocking on EMBEDDERS)</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `pub struct AstChunk` with 9 fields per REQ-AST-02
    - Define `pub struct ParseDiagnostic` with 10 fields per REQ-AST-08
    - serde::Serialize / serde::Deserialize on both, with `#[serde(rename_all = "snake_case")]` for canonical JSON
    - sha256 field serializes as hex string (not base64) per FSV harness convention
    - content field serializes as Vec<u8> via serde_bytes (efficient base64 in JSON)
    - Add a placeholder `pub type EmbedderId = u8;` in `mod.rs` (or behind a `#[cfg(not(feature="embedders-spec"))]` shim) so the BTreeSet<EmbedderId> field type-checks without TASK-EMBED-001
    - Unit tests: AstChunk and ParseDiagnostic round-trip via serde_json with stable ordering
    - Const `pub const MAX_PARSE_DIAGNOSTICS: usize = 32;` re-exported
  </in_scope>
  <out_of_scope>
    - Constructors / builders (caller-facing API; TASK-AST-007 / TASK-AST-021)
    - sha256 computation (TASK-AST-006)
    - embedder_routing_set lookup (TASK-AST-005)
    - Replacing the legacy CodeChunk type (TASK-AST-021 backwards compat shim)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/types.rs">
      use std::collections::BTreeSet;
      use serde::{Deserialize, Serialize};
      use super::{Language, EntityType};

      pub const MAX_PARSE_DIAGNOSTICS: usize = 32;

      /// EmbedderId placeholder — replaced with SPEC-EMBEDDERS enum in TASK-EMBED-001.
      pub type EmbedderId = u8;

      /// Output unit of the multi-language AST chunker.
      #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub struct AstChunk {
          pub language: Language,
          pub entity_type: EntityType,
          pub parent_chain: String,
          pub line_start: u32,
          pub line_end: u32,
          #[serde(with = "hex_array_32")]
          pub sha256: [u8; 32],
          pub parse_diagnostics: Vec<ParseDiagnostic>,
          #[serde(with = "serde_bytes")]
          pub content: Vec<u8>,
          pub embedder_routing_set: BTreeSet<EmbedderId>,
      }

      /// Byte-precise tree-sitter parse diagnostic.
      #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub struct ParseDiagnostic {
          pub kind: String,
          pub is_error: bool,
          pub is_missing: bool,
          pub start_byte: usize,
          pub end_byte: usize,
          pub start_line: u32,
          pub start_column: u32,
          pub end_line: u32,
          pub end_column: u32,
          pub excerpt: String,
      }

      mod hex_array_32 {
          // serde helper: serialize [u8; 32] as 64-char hex string
      }
    </signature>
  </signatures>

  <constraints>
    - sha256 serializes as 64-char lowercase hex string (NOT base64, NOT array of u8) for human
      readability in FSV evidence files
    - content serializes via serde_bytes (compact base64 in JSON, raw bytes in bincode/MessagePack)
    - parse_diagnostics field length 0..=MAX_PARSE_DIAGNOSTICS (caller enforces; struct does not)
    - embedder_routing_set is BTreeSet (NOT HashSet) for deterministic JSON ordering — this is
      the constitution determinism requirement
    - No `Default` impl — chunks must be explicitly constructed; partial defaults are silent bugs
    - File under 500 lines; this file should be < 250 lines including hex_array_32 helper module
    - Add serde_bytes = "0.11" as workspace dep if not present (workspace Cargo.toml level)
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::types` all pass (≥4 tests)
    - JSON round-trip test: build a fixture AstChunk, serialize to string, parse back, assert equal
    - sha256 hex format test: `[0xab, 0xcd, ...]` → `"abcd..."` (no `0x` prefix, no separators)
    - Determinism test: serialize the same chunk twice; assert byte-equal output (BTreeSet ordering)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
types.rs:
  use std::collections::BTreeSet;
  use serde::{Deserialize, Serialize};
  use super::{Language, EntityType};

  pub const MAX_PARSE_DIAGNOSTICS: usize = 32;
  pub type EmbedderId = u8;  // TODO TASK-EMBED-001: replace with proper enum

  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub struct AstChunk {
      pub language: Language,
      pub entity_type: EntityType,
      pub parent_chain: String,
      pub line_start: u32,
      pub line_end: u32,
      #[serde(with = "hex_array_32")] pub sha256: [u8; 32],
      pub parse_diagnostics: Vec<ParseDiagnostic>,
      #[serde(with = "serde_bytes")] pub content: Vec<u8>,
      pub embedder_routing_set: BTreeSet<EmbedderId>,
  }

  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub struct ParseDiagnostic { /* 10 fields per spec */ }

  mod hex_array_32 {
      use serde::{Deserializer, Serializer, de::Error as DeError};
      pub fn serialize<S: Serializer>(v: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
          let hex: String = v.iter().map(|b| format!("{:02x}", b)).collect();
          s.serialize_str(&hex)
      }
      pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
          let s: String = serde::Deserialize::deserialize(d)?;
          if s.len() != 64 { return Err(D::Error::custom("expected 64 hex chars")); }
          let mut out = [0u8; 32];
          for i in 0..32 {
              out[i] = u8::from_str_radix(&s[2*i..2*i+2], 16).map_err(D::Error::custom)?;
          }
          Ok(out)
      }
  }

  #[cfg(test)] mod tests {
      use super::*;
      use std::collections::BTreeSet;
      fn fixture() -> AstChunk { AstChunk {
          language: Language::Rust, entity_type: EntityType::Function,
          parent_chain: "foo::bar".into(), line_start: 1, line_end: 5,
          sha256: [0xab; 32], parse_diagnostics: vec![],
          content: b"fn foo() {}".to_vec(),
          embedder_routing_set: BTreeSet::from([7, 14]),
      } }
      #[test] fn round_trip_json() {
          let f = fixture();
          let json = serde_json::to_string(&f).unwrap();
          let back: AstChunk = serde_json::from_str(&json).unwrap();
          assert_eq!(f, back);
      }
      #[test] fn sha256_serializes_hex() {
          let f = fixture();
          let json = serde_json::to_string(&f).unwrap();
          assert!(json.contains("\"sha256\":\"abababababababab"));  // 32 × "ab"
      }
      #[test] fn deterministic_serialization() {
          let f = fixture();
          let j1 = serde_json::to_string(&f).unwrap();
          let j2 = serde_json::to_string(&f).unwrap();
          assert_eq!(j1, j2);
      }
      #[test] fn diagnostic_round_trip() {
          let d = ParseDiagnostic {
              kind: "ERROR".into(), is_error: true, is_missing: false,
              start_byte: 0, end_byte: 10, start_line: 1, start_column: 0,
              end_line: 1, end_column: 10, excerpt: "fn () {}".into(),
          };
          let json = serde_json::to_string(&d).unwrap();
          let back: ParseDiagnostic = serde_json::from_str(&json).unwrap();
          assert_eq!(d, back);
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/types.rs">AstChunk + ParseDiagnostic + EmbedderId placeholder + hex_array_32 serde helper + 4 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod types;` + `pub use types::{AstChunk, ParseDiagnostic, EmbedderId, MAX_PARSE_DIAGNOSTICS};`</file>
  <file path="Cargo.toml (workspace)">Add `serde_bytes = "0.11"` to workspace dependencies if not present; reference in context-graph-core Cargo.toml</file>
</files_to_modify>

<validation_criteria>
  <criterion>AstChunk has all 9 fields with correct types per REQ-AST-02</criterion>
  <criterion>ParseDiagnostic has all 10 fields per REQ-AST-08</criterion>
  <criterion>sha256 round-trips as 64-char hex string</criterion>
  <criterion>content round-trips via serde_bytes (works for non-UTF-8 bytes)</criterion>
  <criterion>BTreeSet ordering is preserved across serialize cycles (determinism)</criterion>
  <criterion>MAX_PARSE_DIAGNOSTICS = 32 const exported</criterion>
  <criterion>EmbedderId placeholder type alias exists with TODO comment referencing TASK-EMBED-001</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::types</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```

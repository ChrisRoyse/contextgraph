# TASK-CORPUS-004 — MutationConfig + MutationSite + MutationOutcome Types

```xml
<task_spec id="TASK-CORPUS-004" version="1.0">
<metadata>
  <title>MutationConfig + MutationSite + MutationOutcome Types</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Concrete data types consumed by every mutation
operator AND by `apply_mutation`. Defining these before the
operators ship lets the operators compile against a single
stable contract (TECH-CORPUS `<data_models>` rows
MutationConfig, MutationSite, MutationOutcome).
</context>

<input_context_files>
  <file purpose="data_model">../../technical/TECH-CORPUS.md#data_models</file>
  <file purpose="error_taxonomy">../../technical/TECH-CORPUS.md#error_taxonomy</file>
  <file purpose="constitution_naming">../../constitution.md#coding_standards</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-001 complete (MutationError variants exist)</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `MutationConfig { seed: u64, alternate_source:
      Option&lt;String&gt; }` with `Default`, `Serialize`,
      `Deserialize`, `Debug`, `Clone`, `PartialEq`, `Eq`.
    - Define `MutationSite { byte_offset: usize, byte_length: usize,
      original_text: String, replacement_text: String, note: String }`
      with the same derives.
    - Define `MutationOutcome { category: MutationCategory,
      mutated_source: String, seed: u64, mutation_site:
      Option&lt;MutationSite&gt; }` with the same derives plus
      `#[serde(skip_serializing_if = "Option::is_none")]` on
      mutation_site (per TECH-CORPUS data model).
    - Add unit tests covering: default seed=0, JSON serialization
      omits None mutation_site, JSON round-trip preserves all fields.
  </in_scope>
  <out_of_scope>
    - Mutation operator logic (TASK-CORPUS-012..016).
    - apply_mutation dispatcher (TASK-CORPUS-016).
    - apply_site validator implementation (also TASK-CORPUS-016).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    pub struct MutationConfig {
        pub seed: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub alternate_source: Option&lt;String&gt;,
    }

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct MutationSite {
        pub byte_offset: usize,
        pub byte_length: usize,
        pub original_text: String,
        pub replacement_text: String,
        pub note: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct MutationOutcome {
        pub category: MutationCategory,
        pub mutated_source: String,
        pub seed: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mutation_site: Option&lt;MutationSite&gt;,
    }
    </signature>
  </signatures>

  <constraints>
    - All structs in `lib.rs` near the existing public surface
      (or a dedicated `types.rs` module if `lib.rs` exceeds 500
      lines per constitution `<file_organization>`).
    - serde rename_all NOT applied at the struct level; field names
      stay snake_case.
    - `MutationConfig::default()` returns `{ seed: 0,
      alternate_source: None }`.
    - No `unwrap()` in production code paths.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- mutation_types` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` succeeds.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
#[test]
fn mutation_config_default_seed_is_zero() {
    let c = MutationConfig::default();
    assert_eq!(c.seed, 0);
    assert!(c.alternate_source.is_none());
}

#[test]
fn mutation_outcome_omits_none_mutation_site() {
    let o = MutationOutcome {
        category: MutationCategory::KnownGood,
        mutated_source: "x".into(),
        seed: 0,
        mutation_site: None,
    };
    let s = serde_json::to_string(&amp;o).unwrap();
    assert!(!s.contains("mutation_site"));
}

#[test]
fn mutation_outcome_json_round_trip_with_site() {
    let site = MutationSite {
        byte_offset: 3, byte_length: 1,
        original_text: "&gt;".into(),
        replacement_text: "&lt;".into(),
        note: "subtle_flip swap".into(),
    };
    let o = MutationOutcome {
        category: MutationCategory::SubtleFlip,
        mutated_source: "a&lt;b".into(),
        seed: 42,
        mutation_site: Some(site),
    };
    let s = serde_json::to_string(&amp;o).unwrap();
    let back: MutationOutcome = serde_json::from_str(&amp;s).unwrap();
    assert_eq!(o, back);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add 3 structs.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 3 round-trip tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>3 structs defined with required derives</criterion>
  <criterion>JSON serialization of MutationOutcome omits None mutation_site</criterion>
  <criterion>JSON round-trip preserves all fields</criterion>
  <criterion>MutationConfig::default() returns seed=0, alternate_source=None</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- mutation_types</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

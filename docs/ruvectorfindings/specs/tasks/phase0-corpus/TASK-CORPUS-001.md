# TASK-CORPUS-001 — Error Taxonomy + MutationCategory Enum

```xml
<task_spec id="TASK-CORPUS-001" version="1.0">
<metadata>
  <title>Error Taxonomy + MutationCategory Enum</title>
  <status>partial</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `MutationCategory` (8 variants, `all() -> [Self; 8]`, `slug()`, `#[serde(rename_all = "snake_case")]`); `MutationError` with 3 variants (`InvalidInput`, `NoMutationSite`, `OperatorFailed`) + `code()` mapping; `OracleError` with 7 variants + `code()` mapping. See `crates/context-graph-mejepa-corpus/src/lib.rs:66-191` and `oracle.rs:31-177`.
    Pending: add `MutationError::LeakageDetected` variant + `MEJEPA_CORPUS_LEAKAGE_DETECTED` code; add `OracleError::DockerRunIdUnsafe` variant + `MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE` code (validator already exists inline at `swebench.rs:552-581` but raises generic `InvalidInput`); add `MutationCategory::from_slug` reverse helper; add `error_codes_match_taxonomy` + `mutation_category_slug_round_trip` unit tests.
  </implementation_state>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-02</requirement_ref>
    <requirement_ref>REQ-CORPUS-08</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Defines `MutationError`, `OracleError`, and the
`MutationCategory` enum that keys both the corpus and the verdicts
column families. Every other task in this phase imports these types,
so this MUST land first. Verifies the shipped state (8 categories,
2 fallible-result aliases) and adds the leakage error variant + the
`MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE` variant per `<error_taxonomy>`.
</context>

<input_context_files>
  <file purpose="constitution">../../constitution.md</file>
  <file purpose="error_taxonomy_spec">../../technical/TECH-CORPUS.md#error_taxonomy</file>
  <file purpose="data_model_spec">../../technical/TECH-CORPUS.md#data_models</file>
  <file purpose="failure_taxonomy_root">../../constitution.md#failure_taxonomy</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>cargo build --release -p context-graph-mejepa-corpus succeeds on baseline</check>
  <check>thiserror is in workspace dependencies</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `MutationError` enum has variants
      InvalidInput, NoMutationSite, OperatorFailed.
    - ADD `LeakageDetected` variant carrying `sha`, `collisions:
      Vec<(String, String)>` (task_id, bucket).
    - ADD `MutationError::code()` arm for LeakageDetected returning
      `MEJEPA_CORPUS_LEAKAGE_DETECTED`.
    - Verify shipped `OracleError` (in `oracle.rs` / `swebench.rs`)
      has variants InvalidInput, Io, CommandFailed, Timeout,
      JsonParse, ReportInvalid, Store.
    - ADD `OracleError::DockerRunIdUnsafe { run_id, char, position }`
      and `.code()` returning
      `MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE`.
    - Verify shipped `MutationCategory` enum has the 8 doc 09 §3.1
      variants in canonical order; add `MutationCategory::all() ->
      &'static [MutationCategory; 8]`, `slug() -> &'static str`,
      `from_slug(&str) -> Option<Self>`.
    - Add `Hash + Eq + Copy` derives if missing.
    - Add unit tests in `src/tests.rs` for `code()` mapping
      (one per variant) and for slug round-trip on all 8 categories.
  </in_scope>
  <out_of_scope>
    - Concrete oracle / split / mutation implementations
      (TASK-CORPUS-010 through TASK-CORPUS-022).
    - Re-export of OracleVerdict (TASK-CORPUS-003).
    - Language enum (TASK-CORPUS-002).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    pub type MutationResult&lt;T&gt; = Result&lt;T, MutationError&gt;;

    #[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
    pub enum MutationError {
        InvalidInput { field: &amp;'static str, message: String, remediation: &amp;'static str },
        NoMutationSite { field: &amp;'static str, message: String, remediation: &amp;'static str },
        OperatorFailed { field: &amp;'static str, message: String, remediation: &amp;'static str },
        LeakageDetected { sha: String, collisions: Vec&lt;(String, String)&gt; },
    }

    impl MutationError {
        pub fn code(&amp;self) -&gt; &amp;'static str;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum MutationCategory {
        KnownGood, SubtleFlip, OffByOne, SwapVariable,
        DeleteTestCall, WrongFile, OverEngineer, CompileError,
    }

    impl MutationCategory {
        pub const fn all() -&gt; &amp;'static [MutationCategory; 8];
        pub const fn slug(&amp;self) -&gt; &amp;'static str;
        pub fn from_slug(slug: &amp;str) -&gt; Option&lt;Self&gt;;
    }
    </signature>
    <signature file="crates/context-graph-mejepa-corpus/src/oracle.rs">
    pub type OracleResult&lt;T&gt; = Result&lt;T, OracleError&gt;;

    #[derive(Debug, thiserror::Error)]
    pub enum OracleError {
        InvalidInput { field: &amp;'static str, message: String, remediation: &amp;'static str },
        Io { context: String, source: std::io::Error },
        CommandFailed { command: Vec&lt;String&gt;, exit_code: Option&lt;i32&gt;, stdout_tail: String, stderr_tail: String },
        Timeout { context: &amp;'static str, elapsed: std::time::Duration },
        JsonParse { context: &amp;'static str, source: serde_json::Error },
        ReportInvalid { field: &amp;'static str, message: String },
        Store { operation: &amp;'static str, cf: &amp;'static str, message: String },
        DockerRunIdUnsafe { run_id: String, character: char, position: usize },
    }

    impl OracleError {
        pub fn code(&amp;self) -&gt; &amp;'static str;
    }
    </signature>
  </signatures>

  <constraints>
    - All variants follow constitution naming: SCREAMING_SNAKE
      `.code()` tags exactly per TECH-CORPUS `<error_taxonomy>`.
    - Source-of-truth file paths in error messages start with `file:`.
    - No `Box&lt;dyn Error&gt;` — typed variants only.
    - `MutationCategory` MUST `#[serde(rename_all = "snake_case")]`
      so JSON values match `known_good`, `subtle_flip`, etc.
    - No clippy warnings.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-corpus` succeeds.
    - `cargo test -p context-graph-mejepa-corpus -- error_codes` passes.
    - `cargo test -p context-graph-mejepa-corpus -- mutation_category_slug_round_trip` passes.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
// lib.rs
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum MutationError {
    #[error("mutation input invalid at {field}: {message}; remediation: {remediation}")]
    InvalidInput { ... },
    #[error("mutation operator could not find a site at {field}: ...")]
    NoMutationSite { ... },
    #[error("mutation operator failed at {field}: ...")]
    OperatorFailed { ... },
    #[error("test-patch leakage: sha {sha} appears in multiple buckets ({collisions:?})")]
    LeakageDetected { sha: String, collisions: Vec<(String, String)> },
}

impl MutationCategory {
    pub const fn all() -&gt; &amp;'static [MutationCategory; 8] {
        &amp;[KnownGood, SubtleFlip, OffByOne, SwapVariable,
          DeleteTestCall, WrongFile, OverEngineer, CompileError]
    }
    pub const fn slug(&amp;self) -&gt; &amp;'static str {
        match self {
            Self::KnownGood =&gt; "known_good", ...
        }
    }
}

#[test]
fn error_codes_match_taxonomy() {
    assert_eq!(MutationError::InvalidInput { ... }.code(), "MEJEPA_CORPUS_INVALID_INPUT");
    assert_eq!(MutationError::LeakageDetected { ... }.code(), "MEJEPA_CORPUS_LEAKAGE_DETECTED");
    assert_eq!(OracleError::DockerRunIdUnsafe { ... }.code(),
               "MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE");
}

#[test]
fn mutation_category_slug_round_trip() {
    for c in MutationCategory::all() {
        assert_eq!(MutationCategory::from_slug(c.slug()), Some(*c));
    }
}
</pseudo_code>

<files_to_create>
  <!-- All edits land in existing files; nothing new. -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add LeakageDetected variant + .code() arm; verify MutationCategory has all() / slug() / from_slug().</file>
  <file path="crates/context-graph-mejepa-corpus/src/oracle.rs">Add DockerRunIdUnsafe variant + .code() arm.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add error_codes_match_taxonomy + mutation_category_slug_round_trip tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 8 MutationCategory variants exist + slug round-trips</criterion>
  <criterion>MutationError has 4 variants (InvalidInput, NoMutationSite, OperatorFailed, LeakageDetected) with correct .code() tags</criterion>
  <criterion>OracleError has 8 variants with correct .code() tags including DockerRunIdUnsafe</criterion>
  <criterion>cargo build --release succeeds; cargo test passes; clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- error_codes_match_taxonomy</command>
  <command>cargo test -p context-graph-mejepa-corpus -- mutation_category_slug_round_trip</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

# TASK-AST-004 — Define `AstChunkerError` taxonomy (5 variants + error codes)

```xml
<task_spec id="TASK-AST-004" version="1.0">
<metadata>
  <title>Define AstChunkerError taxonomy aligned with constitution failure_taxonomy</title>
  <status>blocked</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-AST-08">ParseFailed → MEJEPA_CHUNKER_PARSE_FAILED</item>
    <item ref="REQ-AST-09">UnsupportedLanguage → MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE</item>
    <item ref="REQ-AST-10">EmptySource → MEJEPA_CHUNKER_EMPTY_SOURCE</item>
    <item ref="REQ-AST-11">MixedLanguage → MEJEPA_CHUNKER_MIXED_LANGUAGE</item>
    <item ref="ERR-AST-05">LanguageSetFailed (defense-in-depth)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-001</task_ref>
    <task_ref>TASK-AST-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
Every fail-closed boundary in the chunker maps to one of 5 structured error variants.
Each variant carries structured fields (file_path, language, diagnostics, etc.) per
constitution `<error_handling>` rule "Error context (file_path, language, byte/line ranges)
MUST be in the error variant's structured fields, NOT formatted into the message string."
Each variant's `.code()` returns the SCREAMING_SNAKE error code for log emission.
</context>

<input_context_files>
  <file purpose="error_taxonomy">docs/ruvectorfindings/specs/constitution.md#failure_taxonomy (MEJEPA_CHUNKER_* error codes)</file>
  <file purpose="data_model_spec">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#data_models (AstChunkerError model)</file>
  <file purpose="legacy_error">crates/context-graph-core/src/memory/ast_chunker.rs (lines 29-51; legacy AstChunkerError to be replaced — note legacy uses UnsupportedExtension; new uses UnsupportedLanguage)</file>
  <file purpose="thiserror_pattern">crates/context-graph-mejepa-corpus/src/lib.rs (existing thiserror pattern to mirror)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-001 complete: Language enum available</check>
  <check>TASK-AST-003 complete: ParseDiagnostic struct available</check>
  <check>thiserror 1.x in workspace deps</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `pub enum AstChunkerError` with 5 variants:
      ParseFailed { file_path, language, reason, diagnostics }
      UnsupportedLanguage { requested }
      EmptySource
      MixedLanguage { primary, embedded }
      LanguageSetFailed { language, reason }
    - `#[derive(Debug, Error)]` via thiserror; `#[error("...")]` messages MUST NOT contain
      ${file_path} or other potentially-large structured payloads — those go in struct fields
    - `pub fn code(&self) -> &'static str` returns the SCREAMING_SNAKE code per the
      constitution failure_taxonomy
    - Unit tests: each variant's `.code()` matches the constitution; each Display impl renders
      a concise message
  </in_scope>
  <out_of_scope>
    - Logging adapter (caller responsibility: `tracing::error!(code = err.code(), …, "…")`)
    - Recovery actions (caller responsibility per ERR-AST-* recovery_action fields)
    - Mapping to ContextGraphError parent type (deferred to TASK-AST-021 if needed)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/error.rs">
      use thiserror::Error;
      use super::{Language, ParseDiagnostic};

      #[derive(Debug, Error)]
      pub enum AstChunkerError {
          #[error("Failed to parse {language:?} source {file_path}: {reason}")]
          ParseFailed {
              file_path: String,
              language: Language,
              reason: String,
              diagnostics: Vec<ParseDiagnostic>,
          },

          #[error("Unsupported language: {requested}")]
          UnsupportedLanguage { requested: String },

          #[error("Source code is empty or contains only whitespace")]
          EmptySource,

          #[error("Mixed-language source not supported: primary={primary:?}, embedded={embedded}")]
          MixedLanguage { primary: Language, embedded: String },

          #[error("Failed to set tree-sitter language for {language}: {reason}")]
          LanguageSetFailed { language: String, reason: String },
      }

      impl AstChunkerError {
          pub fn code(&self) -> &'static str {
              match self {
                  Self::ParseFailed { .. } => "MEJEPA_CHUNKER_PARSE_FAILED",
                  Self::UnsupportedLanguage { .. } => "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
                  Self::EmptySource => "MEJEPA_CHUNKER_EMPTY_SOURCE",
                  Self::MixedLanguage { .. } => "MEJEPA_CHUNKER_MIXED_LANGUAGE",
                  Self::LanguageSetFailed { .. } => "MEJEPA_CHUNKER_PARSE_FAILED",
              }
          }
      }
    </signature>
  </signatures>

  <constraints>
    - Exactly 5 variants. Adding a 6th requires constitution failure_taxonomy bump.
    - Per ERR-AST-05 recovery_action note: LanguageSetFailed maps to MEJEPA_CHUNKER_PARSE_FAILED
      (with reason="language_set_failed") — this is documented as defense-in-depth
    - Display strings are concise (< 100 chars typical) to keep log lines greppable
    - Each variant's structured fields preserve the diagnostic context per
      constitution `<error_handling>`
    - No `From<std::io::Error>` etc. — chunker does no I/O
    - File under 500 lines; this file should be < 150 lines
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::error` all pass (≥6 tests, one per variant + code() roundtrip)
    - `AstChunkerError::EmptySource.code()` returns `"MEJEPA_CHUNKER_EMPTY_SOURCE"` exactly
    - Display output for ParseFailed contains the language and file path
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
error.rs:
  use thiserror::Error;
  use super::{Language, ParseDiagnostic};

  #[derive(Debug, Error)]
  pub enum AstChunkerError {
      #[error("Failed to parse {language:?} source {file_path}: {reason}")]
      ParseFailed { file_path: String, language: Language, reason: String, diagnostics: Vec<ParseDiagnostic> },
      #[error("Unsupported language: {requested}")]
      UnsupportedLanguage { requested: String },
      #[error("Source code is empty or contains only whitespace")]
      EmptySource,
      #[error("Mixed-language source not supported: primary={primary:?}, embedded={embedded}")]
      MixedLanguage { primary: Language, embedded: String },
      #[error("Failed to set tree-sitter language for {language}: {reason}")]
      LanguageSetFailed { language: String, reason: String },
  }

  impl AstChunkerError {
      pub fn code(&self) -> &'static str {
          match self {
              Self::ParseFailed { .. } => "MEJEPA_CHUNKER_PARSE_FAILED",
              Self::UnsupportedLanguage { .. } => "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
              Self::EmptySource => "MEJEPA_CHUNKER_EMPTY_SOURCE",
              Self::MixedLanguage { .. } => "MEJEPA_CHUNKER_MIXED_LANGUAGE",
              Self::LanguageSetFailed { .. } => "MEJEPA_CHUNKER_PARSE_FAILED",
          }
      }
  }

  #[cfg(test)] mod tests {
      use super::*;
      #[test] fn empty_source_code() {
          assert_eq!(AstChunkerError::EmptySource.code(), "MEJEPA_CHUNKER_EMPTY_SOURCE");
      }
      #[test] fn unsupported_lang_code() {
          let e = AstChunkerError::UnsupportedLanguage { requested: "kotlin".into() };
          assert_eq!(e.code(), "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE");
      }
      #[test] fn mixed_lang_code() {
          let e = AstChunkerError::MixedLanguage { primary: Language::Php, embedded: "html".into() };
          assert_eq!(e.code(), "MEJEPA_CHUNKER_MIXED_LANGUAGE");
      }
      #[test] fn parse_failed_code_and_display() {
          let e = AstChunkerError::ParseFailed {
              file_path: "/tmp/x.rs".into(), language: Language::Rust,
              reason: "tree-sitter ERROR node at line 5".into(), diagnostics: vec![],
          };
          assert_eq!(e.code(), "MEJEPA_CHUNKER_PARSE_FAILED");
          let s = format!("{}", e);
          assert!(s.contains("Rust") && s.contains("/tmp/x.rs"));
      }
      #[test] fn language_set_failed_collapses_to_parse_failed_code() {
          let e = AstChunkerError::LanguageSetFailed { language: "rust".into(), reason: "abi mismatch".into() };
          assert_eq!(e.code(), "MEJEPA_CHUNKER_PARSE_FAILED");
      }
      #[test] fn all_codes_screaming_snake() {
          for code in [
              "MEJEPA_CHUNKER_PARSE_FAILED",
              "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
              "MEJEPA_CHUNKER_EMPTY_SOURCE",
              "MEJEPA_CHUNKER_MIXED_LANGUAGE",
          ] {
              assert!(code.chars().all(|c| c.is_ascii_uppercase() || c == '_'));
          }
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/error.rs">AstChunkerError + code() + 6 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod error;` + `pub use error::AstChunkerError;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>5 variants present matching TECH-AST-CHUNKING.md AstChunkerError model</criterion>
  <criterion>code() returns the exact SCREAMING_SNAKE codes per constitution failure_taxonomy</criterion>
  <criterion>LanguageSetFailed.code() returns MEJEPA_CHUNKER_PARSE_FAILED per ERR-AST-05</criterion>
  <criterion>Display impls preserve file_path and language for triage</criterion>
  <criterion>All 6 unit tests pass</criterion>
  <criterion>No clippy warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::error</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```

# TASK-AST-009 — Implement `MixedLanguageDetector` (PHP+HTML / JS+HTML)

```xml
<task_spec id="TASK-AST-009" version="1.0">
<metadata>
  <title>Implement detect_mixed_language with PHP+HTML and JS+HTML refusal heuristics</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-AST-11">Mixed-language → MEJEPA_CHUNKER_MIXED_LANGUAGE fail-closed</item>
    <item ref="EC-AST-05">PHP files with `?>...&lt;?php` blocks return Err(MixedLanguage)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-004</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file (new)</estimated_files>
</metadata>

<context>
HTML+JS, PHP+HTML, JSX+CSS-in-style-blocks all break the single-language assumption.
Per doc 14 §3.3 first-pass scope, we reject these fail-closed and surface
MEJEPA_CHUNKER_MIXED_LANGUAGE. Future Phase 1+ work could route per-block.
The detector is intentionally conservative — only obvious markers trigger detection
(no false positives on string literals).
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (MixedLanguageDetector)</file>
  <file purpose="error_state">docs/ruvectorfindings/specs/functional/SPEC-AST-CHUNKING.md (REQ-AST-11, EC-AST-05)</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-004 complete: AstChunkerError + Language available</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn detect_mixed_language(source: &[u8], language: Language) -> Result<(), AstChunkerError>`
    - PHP heuristic: scan for `?>` followed by any non-whitespace bytes followed by `<?php`
      → `Err(MixedLanguage { primary: Php, embedded: "html".into() })`
    - JS / TS heuristic: scan for `<script` or `<style` outside string-literal context
      (heuristic: no preceding unmatched single/double/backtick quote on the same line);
      → `Err(MixedLanguage { primary, embedded: "html".into() })`
    - All other languages (Rust, Python, Go, Java, C, Cpp, CSharp, Ruby): return `Ok(())` (no
      detection)
    - Unit tests: ≥6 cases covering: pure PHP (Ok), PHP with HTML (Err), JS with `<script` tag
      (Err), JS string containing "<script" (Ok — false-positive avoidance), pure JS (Ok),
      pure Rust (Ok)
  </in_scope>
  <out_of_scope>
    - Per-block language routing (deferred Phase 1+)
    - HTML/CSS chunking (no tree-sitter-html / -css in workspace)
    - Markdown / RST detection (out of scope)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/mixed_lang.rs">
      use super::{AstChunkerError, Language};

      /// Detect embedded foreign-language regions per the per-language heuristic.
      ///
      /// Returns Ok(()) for pure-language source; Err(MixedLanguage) when an embedded
      /// HTML / template region is detected. Conservative — only obvious markers trigger.
      pub fn detect_mixed_language(source: &[u8], language: Language) -> Result<(), AstChunkerError> { /* ... */ }

      fn scan_php_html(source: &[u8]) -> bool { /* ... */ }
      fn scan_js_html(source: &[u8]) -> bool { /* ... */ }
      fn line_has_unmatched_string_open_before(line: &[u8], offset: usize) -> bool { /* ... */ }
    </signature>
  </signatures>

  <constraints>
    - PHP heuristic: find `?>` byte sequence, skip subsequent whitespace bytes, then expect
      either non-whitespace bytes followed by `<?php` (mixed) OR EOF/whitespace-only (pure PHP
      with closing tag — Ok)
    - JS / TS heuristic: scan line-by-line; for each `<script` or `<style` substring, check the
      same line's preceding bytes do NOT contain an unmatched `"` / `'` / `` ` `` — if matched
      string region, the marker is inside a literal and ignored (false-positive avoidance)
    - Conservative bias: if uncertain, return Ok(()) (let parser handle it; ParseFailed will
      surface real syntax issues). The detector is the FIRST line of defense, not the only one.
    - File under 500 lines; this file should be < 200 lines incl tests
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-core` succeeds
    - `cargo test -p context-graph-core memory::ast_chunker::mixed_lang` all pass (≥6 tests)
    - Pure PHP `<?php echo 1; ?>` → Ok(())
    - Mixed PHP `<?php echo 1; ?><h1>x</h1><?php echo 2; ?>` → Err(MixedLanguage)
    - JS with `<script>` tag at column 0 → Err(MixedLanguage)
    - JS string `const x = "<script>";` → Ok(()) (false-positive avoidance)
    - Pure Rust source → Ok(())
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
mixed_lang.rs:
  use super::{AstChunkerError, Language};

  pub fn detect_mixed_language(source: &[u8], language: Language) -> Result<(), AstChunkerError> {
      match language {
          Language::Php => {
              if scan_php_html(source) {
                  return Err(AstChunkerError::MixedLanguage {
                      primary: Language::Php, embedded: "html".into(),
                  });
              }
          }
          Language::JavaScript | Language::TypeScript => {
              if scan_js_html(source) {
                  return Err(AstChunkerError::MixedLanguage {
                      primary: language, embedded: "html".into(),
                  });
              }
          }
          _ => {}
      }
      Ok(())
  }

  fn scan_php_html(source: &[u8]) -> bool {
      // Look for `?>` followed eventually by `<?php` (with non-whitespace bytes between)
      let mut i = 0;
      while i + 1 < source.len() {
          if source[i] == b'?' && source[i+1] == b'>' {
              // Skip whitespace
              let mut j = i + 2;
              while j < source.len() && source[j].is_ascii_whitespace() { j += 1; }
              // If there's any non-PHP-token content between ?> and a subsequent <?php, mixed
              let mut found_nonws = false;
              while j + 4 < source.len() {
                  if source[j] == b'<' && &source[j..j.min(j+5)] == b"<?php" {
                      if found_nonws { return true; }
                      break;
                  }
                  if !source[j].is_ascii_whitespace() { found_nonws = true; }
                  j += 1;
              }
              i = j;
          } else { i += 1; }
      }
      false
  }

  fn scan_js_html(source: &[u8]) -> bool {
      // Line-by-line scan
      for line in source.split(|&b| b == b'\n') {
          for marker in [&b"<script"[..], &b"<style"[..]] {
              if let Some(off) = find_subslice(line, marker) {
                  if !line_has_unmatched_string_open_before(line, off) {
                      return true;
                  }
              }
          }
      }
      false
  }

  fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
      haystack.windows(needle.len()).position(|w| w == needle)
  }

  fn line_has_unmatched_string_open_before(line: &[u8], offset: usize) -> bool {
      // Count unescaped quotes of each type before offset; odd count = unmatched-open.
      let mut single = 0u32;
      let mut double = 0u32;
      let mut back = 0u32;
      let mut esc = false;
      for (i, &b) in line.iter().enumerate() {
          if i >= offset { break; }
          if esc { esc = false; continue; }
          match b {
              b'\\' => esc = true,
              b'\'' => single ^= 1,
              b'"' => double ^= 1,
              b'`' => back ^= 1,
              _ => {}
          }
      }
      single == 1 || double == 1 || back == 1
  }

  #[cfg(test)] mod tests {
      use super::*;
      #[test] fn pure_php_ok() {
          let s = b"<?php echo 1; ?>";
          assert!(detect_mixed_language(s, Language::Php).is_ok());
      }
      #[test] fn php_with_inline_html_err() {
          let s = b"<?php echo 1; ?><h1>x</h1><?php echo 2; ?>";
          assert!(matches!(detect_mixed_language(s, Language::Php),
                           Err(AstChunkerError::MixedLanguage { .. })));
      }
      #[test] fn js_with_script_tag_err() {
          let s = b"<script>alert(1)</script>";
          assert!(matches!(detect_mixed_language(s, Language::JavaScript),
                           Err(AstChunkerError::MixedLanguage { .. })));
      }
      #[test] fn js_string_with_script_text_ok() {
          let s = b"const x = \"<script>\";\n";
          assert!(detect_mixed_language(s, Language::JavaScript).is_ok());
      }
      #[test] fn pure_js_ok() {
          let s = b"const x = 1;\nfunction f() { return x; }";
          assert!(detect_mixed_language(s, Language::JavaScript).is_ok());
      }
      #[test] fn rust_skipped() {
          let s = b"fn x() { /* <script> */ 0 }";
          assert!(detect_mixed_language(s, Language::Rust).is_ok());
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mixed_lang.rs">detect_mixed_language + 3 helper fns + 6 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod mixed_lang;` + `pub use mixed_lang::detect_mixed_language;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>Pure PHP source → Ok(())</criterion>
  <criterion>PHP with `?>...&lt;?php` blocks → Err(MixedLanguage)</criterion>
  <criterion>JS with `&lt;script&gt;` tag → Err(MixedLanguage)</criterion>
  <criterion>JS string literal containing `&lt;script&gt;` → Ok(()) (false-positive avoidance)</criterion>
  <criterion>Rust / Python / Go / Java / C / C++ / C# / Ruby → always Ok(()) (no detection)</criterion>
  <criterion>All 6 unit tests pass</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-core</command>
  <command>cargo test -p context-graph-core memory::ast_chunker::mixed_lang</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```

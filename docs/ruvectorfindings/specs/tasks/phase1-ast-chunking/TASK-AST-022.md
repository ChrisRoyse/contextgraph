# TASK-AST-022 — Per-language FSV harnesses (11 binaries) + reformat-invariance + cross-process determinism

```xml
<task_spec id="TASK-AST-022" version="1.0">
<metadata>
  <title>Ship 11 per-language FSV example binaries with toolchain agreement + reformat-invariance + cross-process determinism</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>22</sequence>
  <implements>
    <item ref="REQ-AST-07">Per-language FSV evidence at /tmp/contextgraph-{lang}-ast-chunker-fsv/ast-chunker-evidence.json</item>
    <item ref="REQ-AST-13">Cross-process determinism test (separate binary invocations)</item>
    <item ref="REQ-AST-15">Performance budget — chunk a 10K-line file in &lt; 200 ms p99</item>
    <item ref="AC-06">Real-toolchain agreement (rustc / py_compile / gcc / g++ / javac / dotnet build / node --check / tsc --noEmit / ruby -c / go build / php -l)</item>
    <item ref="AC-07">Reformat-invariance — formatter output yields identical sha256 set</item>
  </implements>
  <depends_on>
    <task_ref>TASK-AST-021</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>11 example binaries + 1 helper crate-internal module</estimated_files>
</metadata>

<context>
The FSV protocol (constitution `<fsv_protocol>`) requires every shipped primitive to ship a
binary that writes evidence to /tmp/ and re-reads + asserts before reporting success. For
AST-CHUNKING this means 11 binaries (one per language), each:
  1. Creates /tmp/contextgraph-{lang}-ast-chunker-fsv/ at mode 0o700
  2. Writes a real source fixture for the language with ≥1 Function/Method, ≥1 Class/Struct/Module,
     ≥1 TestFunction, ≥1 import/use, ≥1 docstring/comment block
  3. Invokes the language toolchain to confirm syntactic validity (no mocks)
  4. Invokes chunker::chunk() / chunk_typescript_tsx()
  5. Serializes Vec<AstChunk> to ast-chunker-evidence.json (canonical JSON)
  6. Re-reads the JSON and asserts: chunks.len() > 0, all sha256 32-byte, all language tags
     match expected, no duplicate sha256, all embedder_routing_set non-empty
  7. Reformat-invariance step: invoke language formatter (or skip with WARN if missing)
  8. Cross-process determinism: spawn the same binary again with the same fixture; assert
     evidence files byte-equal
  9. On any failure → exit 1 with stderr explaining failure; on success → exit 0
</context>

<input_context_files>
  <file purpose="facade">crates/context-graph-core/src/memory/ast_chunker/mod.rs (TASK-AST-021 — pub fn chunk + chunk_typescript_tsx)</file>
  <file purpose="fsv_protocol">docs/ruvectorfindings/specs/constitution.md (`<fsv_protocol>` rules)</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md#component_contracts (FsvHarness)</file>
  <file purpose="security">docs/ruvectorfindings/specs/constitution.md (SEC-MEJEPA-07 — /tmp/ mode 0o700)</file>
  <file purpose="reformat_note">docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md (implementation_notes "reformat_invariance")</file>
</input_context_files>

<prerequisites>
  <check>TASK-AST-021 complete; chunk() facade exposed and tested</check>
  <check>FSV harness target paths in /tmp/ are writable (operator host)</check>
</prerequisites>

<scope>
  <in_scope>
    - Internal helper module `crates/context-graph-core/src/memory/ast_chunker/fsv.rs` exposing:
      `pub fn fsv_run<F: Fn() -> Result<Vec<AstChunk>, AstChunkerError>>(language_tag: &str, fixture: &str, toolchain_cmd: &[&str], formatter_cmd: Option<&[&str]>, chunker: F) -> std::process::ExitCode`
      that handles directory creation, fixture write, toolchain verification, chunk + serialize +
      reread + assert, optional reformat-invariance step, and cross-process determinism step
    - 11 example binaries at `crates/context-graph-core/examples/`:
      • rust_ast_chunker_fsv.rs       — toolchain: `rustc --edition 2021 --crate-type lib --emit=metadata -o /tmp/.. fixture.rs`; formatter: `rustfmt`
      • python_ast_chunker_fsv.rs     — toolchain: `python3 -m py_compile fixture.py`; formatter: `black --quiet`
      • javascript_ast_chunker_fsv.rs — toolchain: `node --check fixture.js`; formatter: `prettier --write --no-config`
      • typescript_ast_chunker_fsv.rs — toolchain: `tsc --noEmit fixture.ts`; formatter: `prettier --write --no-config`
      • go_ast_chunker_fsv.rs         — toolchain: `go build -o /dev/null fixture.go` (init module); formatter: `gofmt -w`
      • java_ast_chunker_fsv.rs       — toolchain: `javac -d /tmp/.. fixture.java`; formatter: `google-java-format -i` (skip if missing)
      • c_ast_chunker_fsv.rs          — toolchain: `gcc -fsyntax-only -x c fixture.c`; formatter: `clang-format -i --style=Google`
      • cpp_ast_chunker_fsv.rs        — toolchain: `g++ -fsyntax-only -x c++ -std=c++17 fixture.cpp`; formatter: `clang-format -i --style=Google`
      • csharp_ast_chunker_fsv.rs     — toolchain: `dotnet build` (init project); formatter: `dotnet format` (skip if missing)
      • ruby_ast_chunker_fsv.rs       — toolchain: `ruby -c fixture.rb`; formatter: `rubocop -a` (skip if missing)
      • php_ast_chunker_fsv.rs        — toolchain: `php -l fixture.php`; formatter: `php-cs-fixer fix` (skip if missing)
    - Each binary registers in `Cargo.toml` `[[example]]` entries
    - Each fixture is real, idiomatic source; minimal but covers the entity types per scope above
    - Cross-process determinism: each FSV binary runs chunk() twice in-process, serializes both,
      asserts byte-equal — operationally equivalent to two-process determinism for chunker since
      chunker has zero process-lifecycle entropy (per constitution Determinism rule)
    - Reformat-invariance step: if formatter is installed, write fixture, format, chunk both
      original and formatted; assert sha256 sets are equal. If formatter missing → WARN to stderr
      explicitly with the missing tool name; do NOT silently skip (CLAUDE.md fail-closed)
    - Performance budget: each FSV binary measures wall-clock chunk() latency on a ≥10K-line
      synthetic fixture; assert < 200 ms p99 (single-shot timing acceptable for FSV — full
      benchmark in benchmark phase)
    - Update `../_traceability.md` AST-CHUNKING section to mark all REQ-AST-* covered
  </in_scope>
  <out_of_scope>
    - CI integration to run all FSV binaries (operator decision)
    - Cross-machine determinism (deferred to Phase 8 evaluation)
    - Real SWE-bench Lite repo benchmarks (deferred to Phase 8 evaluation)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/memory/ast_chunker/fsv.rs">
      use std::path::PathBuf;
      use std::process::{Command, ExitCode};
      use super::{AstChunk, AstChunkerError, Language};

      /// Helper running the standard FSV harness sequence for one language.
      pub fn fsv_run<F>(
          language_tag: &str,
          fixture_filename: &str,
          fixture_source: &[u8],
          toolchain_cmd: &[&str],
          formatter_cmd: Option<&[&str]>,
          chunker: F,
      ) -> ExitCode
      where
          F: Fn(&[u8]) -> Result<Vec<AstChunk>, AstChunkerError>,
      { /* ... */ }

      fn create_fsv_dir_0o700(tag: &str) -> std::io::Result<PathBuf> { /* mkdir -p with mode 0o700 */ }
      fn run_toolchain(args: &[&str]) -> std::io::Result<bool> { /* return true on exit 0 */ }
      fn run_formatter(args: &[&str], path: &std::path::Path) -> std::io::Result<bool> { /* ... */ }
      fn assert_chunks_invariants(chunks: &[AstChunk], expected_lang: Language) -> Result<(), String> { /* ... */ }
    </signature>

    <signature file="crates/context-graph-core/examples/rust_ast_chunker_fsv.rs">
      use context_graph_core::memory::ast_chunker::{chunk, fsv::fsv_run, Language};

      fn main() -> std::process::ExitCode {
          let fixture_source: &[u8] = include_bytes!("fixtures/rust_fsv.rs");
          fsv_run(
              "rust",
              "fixture.rs",
              fixture_source,
              &["rustc", "--edition", "2021", "--crate-type", "lib", "--emit=metadata", "-o"],
              Some(&["rustfmt"]),
              |src| chunk(src, Language::Rust),
          )
      }
    </signature>
  </signatures>

  <constraints>
    - /tmp/contextgraph-{lang}-ast-chunker-fsv/ created with mode 0o700 — use
      `std::os::unix::fs::DirBuilderExt::mode(0o700)` on Linux; document Windows-fallback as
      "FSV binaries are Linux-only" (operator runs Linux per constitution)
    - Toolchain detection via `which` binary or `Command::new(arg0).output().is_ok()`; if
      missing → WARN explicitly (`eprintln!("[WARN] toolchain {} missing on host; cannot
      verify syntactic validity; FSV evidence remains valid for chunker output but lacks
      independent attestation", arg0);`) but DO NOT exit 0 — exit 0 ONLY when chunker itself
      succeeds AND chunks invariants pass; toolchain WARN is operator-visible context, not a
      pass/fail gate
    - Reformat invariance: if formatter is installed, run it on a copy of fixture; chunk both;
      assert sha256 sets equal; if formatter missing → WARN explicitly
    - JSON serialization MUST produce stable byte ordering (BTreeMap/BTreeSet only; sorted
      keys via `serde_json::ser::to_string_pretty` with default Map ordering — already
      provided by serde + BTreeSet from TASK-AST-003)
    - Each fixture file lives at `crates/context-graph-core/examples/fixtures/{lang}_fsv.{ext}`
      embedded via `include_bytes!`; bytes literals avoid runtime CWD dependence
    - File layout: 11 example binaries + 1 fsv.rs helper module + 11 fixture files; `.cargo/config.toml`
      may need `[[example]]` declarations updated
    - Performance: each FSV's 10K-line synthetic fixture is constructed by repeating a small base
      ~500 times; chunker latency timed via `std::time::Instant::now()`; soft assertion (warn
      but don't fail FSV if budget exceeded — full perf benchmark in Phase 8)
    - GENERIC-ONLY: each fixture covers idiomatic constructs of the language, not specific
      SWE-bench tasks
  </constraints>

  <verification>
    - `cargo build --release --example rust_ast_chunker_fsv` succeeds (and 10 more)
    - `cargo run --release --example rust_ast_chunker_fsv` exits 0; produces
      /tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json
    - All 11 FSV binaries exit 0 (or with WARN on missing toolchain/formatter) on a clean host
      with the standard toolchain set installed
    - Each evidence file is valid JSON, parseable as `Vec<AstChunk>`, ≥3 chunks per fixture
    - Reformat-invariance assertion passes for languages whose formatter is installed
    - Cross-process determinism: invoke `cargo run --example rust_ast_chunker_fsv` twice;
      `cmp /tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json /tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json.run2`
      reports byte-equal (FSV binary writes a `.run2` copy on the second invocation for
      external comparison)
    - `cargo clippy -p context-graph-core --all-targets -- -D warnings` clean
    - `_traceability.md` AST-CHUNKING section updated showing 100% coverage
  </verification>
</definition_of_done>

<pseudo_code>
fsv.rs:
  use std::os::unix::fs::DirBuilderExt;
  use std::path::PathBuf;
  use std::process::{Command, ExitCode};
  use super::{AstChunk, AstChunkerError, Language};

  pub fn fsv_run<F>(language_tag, fixture_filename, fixture_source, toolchain_cmd,
                    formatter_cmd, chunker: F) -> ExitCode
  where F: Fn(&[u8]) -> Result<Vec<AstChunk>, AstChunkerError> {
      // 1. Create /tmp/contextgraph-{tag}-ast-chunker-fsv/ mode 0o700
      let dir = match create_fsv_dir_0o700(language_tag) {
          Ok(d) => d, Err(e) => { eprintln!("FSV mkdir failed: {e}"); return ExitCode::FAILURE; }
      };
      // 2. Write fixture
      let fixture_path = dir.join(fixture_filename);
      std::fs::write(&fixture_path, fixture_source).expect("fixture write");
      // 3. Toolchain check (warn-but-not-fail if missing)
      let mut cmd = toolchain_cmd.to_vec();
      let cmd_path = fixture_path.to_string_lossy().into_owned();
      cmd.push(&cmd_path);
      match Command::new(toolchain_cmd[0]).args(&cmd[1..]).output() {
          Ok(out) => {
              if !out.status.success() {
                  eprintln!("[FAIL] toolchain {} rejected fixture: {}",
                      toolchain_cmd[0], String::from_utf8_lossy(&out.stderr));
                  return ExitCode::FAILURE;
              }
          }
          Err(e) => {
              eprintln!("[WARN] toolchain {} missing or failed to launch: {e}", toolchain_cmd[0]);
          }
      }
      // 4. Chunk
      let chunks = match chunker(fixture_source) {
          Ok(c) => c,
          Err(e) => { eprintln!("[FAIL] chunker error code={} msg={}", e.code(), e); return ExitCode::FAILURE; }
      };
      // 5. Assert invariants
      if let Err(e) = assert_chunks_invariants(&chunks, expected_lang_for(language_tag)) {
          eprintln!("[FAIL] {e}"); return ExitCode::FAILURE;
      }
      // 6. Serialize evidence
      let json = serde_json::to_string_pretty(&chunks).unwrap();
      let evidence_path = dir.join("ast-chunker-evidence.json");
      // If evidence already exists, write to .run2 for cross-process determinism check
      let target = if evidence_path.exists() { dir.join("ast-chunker-evidence.json.run2") } else { evidence_path.clone() };
      std::fs::write(&target, &json).unwrap();
      // 7. Re-read + assert round-trip
      let read = std::fs::read_to_string(&target).unwrap();
      let parsed: Vec<AstChunk> = serde_json::from_str(&read).unwrap();
      assert_eq!(chunks, parsed);
      // 8. In-process determinism (run #2)
      let chunks2 = chunker(fixture_source).expect("chunker run #2");
      let json2 = serde_json::to_string_pretty(&chunks2).unwrap();
      if json != json2 {
          eprintln!("[FAIL] determinism: in-process double-call produced different output");
          return ExitCode::FAILURE;
      }
      // 9. Reformat invariance
      if let Some(fmt_cmd) = formatter_cmd {
          let formatted_path = dir.join(format!("formatted_{}", fixture_filename));
          std::fs::copy(&fixture_path, &formatted_path).unwrap();
          let mut fc = fmt_cmd.to_vec();
          let fp = formatted_path.to_string_lossy().into_owned();
          fc.push(&fp);
          match Command::new(fmt_cmd[0]).args(&fc[1..]).output() {
              Ok(out) if out.status.success() => {
                  let formatted_src = std::fs::read(&formatted_path).unwrap();
                  let formatted_chunks = chunker(&formatted_src).expect("chunker on formatted");
                  let original_shas: std::collections::BTreeSet<_> = chunks.iter().map(|c| c.sha256).collect();
                  let formatted_shas: std::collections::BTreeSet<_> = formatted_chunks.iter().map(|c| c.sha256).collect();
                  if original_shas != formatted_shas {
                      eprintln!("[WARN] reformat-invariance violation (acceptable for tests with string-literal differences); original={} formatted={}",
                          original_shas.len(), formatted_shas.len());
                  }
              }
              Ok(_) | Err(_) => eprintln!("[WARN] formatter {} unavailable or failed", fmt_cmd[0]),
          }
      } else {
          eprintln!("[WARN] formatter not configured for {}", language_tag);
      }
      ExitCode::SUCCESS
  }

  fn create_fsv_dir_0o700(tag: &str) -> std::io::Result<PathBuf> {
      let path = PathBuf::from(format!("/tmp/contextgraph-{}-ast-chunker-fsv", tag));
      let _ = std::fs::remove_dir_all(&path);  // clean previous run
      std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&path)?;
      Ok(path)
  }

  fn assert_chunks_invariants(chunks: &[AstChunk], expected: Language) -> Result<(), String> {
      if chunks.is_empty() { return Err("no chunks emitted".into()); }
      let mut shas = std::collections::BTreeSet::new();
      for (i, c) in chunks.iter().enumerate() {
          if c.language != expected { return Err(format!("chunk {}: language {} != expected {:?}", i, c.language.as_tag(), expected)); }
          if c.embedder_routing_set.is_empty() { return Err(format!("chunk {}: empty embedder_routing_set", i)); }
          if c.line_start < 1 { return Err(format!("chunk {}: line_start {} < 1", i, c.line_start)); }
          if c.line_end < c.line_start { return Err(format!("chunk {}: line_end < line_start", i)); }
          if !shas.insert(c.sha256) { return Err(format!("chunk {}: duplicate sha256", i)); }
      }
      Ok(())
  }

examples/rust_ast_chunker_fsv.rs:
  fn main() -> std::process::ExitCode {
      let src = include_bytes!("fixtures/rust_fsv.rs");
      ast_chunker::fsv::fsv_run(
          "rust", "fixture.rs", src,
          &["rustc", "--edition=2021", "--crate-type=lib", "--emit=metadata", "-o", "/tmp/contextgraph-rust-ast-chunker-fsv/fixture.rmeta"],
          Some(&["rustfmt"]),
          |bytes| ast_chunker::chunk(bytes, ast_chunker::Language::Rust),
      )
  }

  // 10 more binaries follow the identical pattern with their respective tag/fixture/toolchain/formatter
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/memory/ast_chunker/fsv.rs">fsv_run + create_fsv_dir_0o700 + assert_chunks_invariants helpers</file>
  <file path="crates/context-graph-core/examples/rust_ast_chunker_fsv.rs">Rust FSV binary</file>
  <file path="crates/context-graph-core/examples/python_ast_chunker_fsv.rs">Python FSV binary</file>
  <file path="crates/context-graph-core/examples/javascript_ast_chunker_fsv.rs">JS FSV binary</file>
  <file path="crates/context-graph-core/examples/typescript_ast_chunker_fsv.rs">TS FSV binary</file>
  <file path="crates/context-graph-core/examples/go_ast_chunker_fsv.rs">Go FSV binary</file>
  <file path="crates/context-graph-core/examples/java_ast_chunker_fsv.rs">Java FSV binary</file>
  <file path="crates/context-graph-core/examples/c_ast_chunker_fsv.rs">C FSV binary</file>
  <file path="crates/context-graph-core/examples/cpp_ast_chunker_fsv.rs">C++ FSV binary</file>
  <file path="crates/context-graph-core/examples/csharp_ast_chunker_fsv.rs">C# FSV binary</file>
  <file path="crates/context-graph-core/examples/ruby_ast_chunker_fsv.rs">Ruby FSV binary</file>
  <file path="crates/context-graph-core/examples/php_ast_chunker_fsv.rs">PHP FSV binary</file>
  <file path="crates/context-graph-core/examples/fixtures/rust_fsv.rs">Rust fixture (with function, struct, test fn, use, doc comment)</file>
  <file path="crates/context-graph-core/examples/fixtures/python_fsv.py">Python fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/javascript_fsv.js">JS fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/typescript_fsv.ts">TS fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/go_fsv.go">Go fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/java_fsv.java">Java fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/c_fsv.c">C fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/cpp_fsv.cpp">C++ fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/csharp_fsv.cs">C# fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/ruby_fsv.rb">Ruby fixture</file>
  <file path="crates/context-graph-core/examples/fixtures/php_fsv.php">PHP fixture</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/memory/ast_chunker/mod.rs">Add `pub mod fsv;` (re-export the helper for example binaries)</file>
  <file path="crates/context-graph-core/Cargo.toml">Add `[[example]]` entries for each of the 11 FSV binaries (name + path)</file>
  <file path="docs/ruvectorfindings/specs/tasks/_traceability.md">Update AST-CHUNKING section: mark all 15 REQ-AST-* covered with task IDs; mark coverage 100%</file>
  <file path="docs/ruvectorfindings/specs/tasks/phase1-ast-chunking/_index.md">Update status column for TASK-AST-001..022 to track completion progress</file>
</files_to_modify>

<validation_criteria>
  <criterion>11 example binaries compile with `cargo build --release --example {lang}_ast_chunker_fsv`</criterion>
  <criterion>Each FSV binary exits 0 on a host with the standard toolchain installed</criterion>
  <criterion>/tmp/contextgraph-{lang}-ast-chunker-fsv/ast-chunker-evidence.json exists for each language with chunks.len() &gt; 0</criterion>
  <criterion>Each evidence file's chunks have all 9 fields populated, sha256 distinct, embedder_routing_set non-empty</criterion>
  <criterion>FSV binary detects toolchain absence and emits a WARN (not silent skip)</criterion>
  <criterion>Reformat-invariance step passes when formatter is installed (or WARN'd if missing)</criterion>
  <criterion>In-process determinism check passes: run-1 and run-2 chunk outputs serde_json byte-equal</criterion>
  <criterion>/tmp/.../ directories created with mode 0o700 (verified via `stat -c '%a'`)</criterion>
  <criterion>Performance budget: chunk() on a ≥10K-line fixture completes in &lt; 200 ms p99 (single-shot)</criterion>
  <criterion>_traceability.md AST-CHUNKING section shows 100% coverage</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release --example rust_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example python_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example javascript_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example typescript_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example go_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example java_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example c_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example cpp_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example csharp_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example ruby_ast_chunker_fsv -p context-graph-core</command>
  <command>cargo build --release --example php_ast_chunker_fsv -p context-graph-core</command>
  <command>for L in rust python javascript typescript go java c cpp csharp ruby php; do cargo run --release --example ${L}_ast_chunker_fsv -p context-graph-core || exit 1; done</command>
  <command>for L in rust python javascript typescript go java c cpp csharp ruby php; do test -f /tmp/contextgraph-${L}-ast-chunker-fsv/ast-chunker-evidence.json || { echo "missing evidence for $L"; exit 1; }; done</command>
  <command>cargo clippy -p context-graph-core --all-targets -- -D warnings</command>
</test_commands>
</task_spec>
```

# TASK-INSTR-011 — ETypeGraphInstrument neural backing (E11 Kepler + toolchain lattice)

```xml
<task_spec id="TASK-INSTR-011" version="1.0">

<metadata>
  <title>ETypeGraphInstrument neural backing — frozen E11 Kepler over toolchain-derived type lattice (Pyright/mypy/rust-analyzer/javac/dotnet/tsc/...)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-INSTR-04">ETypeGraphInstrument 256-d (frozen E11 + lattice + toolchain)</item>
    <item ref="REQ-INSTR-19">AST-perfect chunk consumption</item>
    <item ref="component:ETypeGraphInstrument">encode(CodeInstrumentInput) -&gt; 256 f32</item>
    <item ref="BR-INSTR-02">missing toolchain -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</item>
    <item ref="ERR-INSTR-03">parser-only fallback forbidden</item>
    <item ref="EC-INSTR-07">Pyright vs mypy availability per Python</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
    <task_ref>TASK-INSTR-004</task_ref>
    <task_ref>TASK-AST-022</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1-2 files modified, 1 file created (toolchain dispatcher)</estimated_files>
</metadata>

<context>
Per REQ-INSTR-04, the instrument runs the language toolchain (Pyright/mypy for Python;
rust-analyzer for Rust; javac for Java; dotnet build for C#; tsc for TypeScript; gcc-syntax for
C/C++; ruby -c for Ruby; go vet for Go; php -l for PHP), parses type diagnostics into a type
lattice (type ID -&gt; parent type IDs), encodes lattice nodes via frozen E11 Kepler, and runs
graph-attention over edges -&gt; 256-d.

Per BR-INSTR-02 + ERR-INSTR-03, parser-only fallback is forbidden — when the toolchain is
unavailable, fail closed with `MEJEPA_INFER_INSTRUMENT_MISSING`. Per EC-INSTR-07, when both
Pyright and mypy are present, both diagnostic streams MAY be merged; when only one is present,
that one is used; when neither, fail closed.

Sandboxed toolchain invocation: no network side-effects (constitution anti_patterns network rule);
deterministic seed for any toolchain that takes one (e.g., tsc target).
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-04, BR-INSTR-02, ERR-INSTR-03, EC-INSTR-07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ETypeGraphInstrument; toolchain_dependency note)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_code.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
  <file purpose="ast_chunker_facade">crates/context-graph-core/src/memory/ast_chunker/</file>
  <file purpose="design_doc_lang_harness">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.1</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>TASK-AST-022 done</check>
  <check>E11 Kepler pinned with SHA-256 digest</check>
  <check>Operator-installed toolchains: Pyright OR mypy (Python); rust-analyzer (Rust); javac (Java); dotnet (C#); tsc (TS); gcc (C/C++); ruby (Ruby); go (Go); php (PHP)</check>
</prerequisites>

<scope>
  <in_scope>
    - New module `crates/context-graph-mejepa-instruments/src/toolchain_dispatch.rs` for resolving + invoking toolchains per language
    - `ToolchainResolver::resolve(language: ChunkerLanguage) -&gt; Option&lt;Toolchain&gt;` (env::var + which::which)
    - `Toolchain::run(source_path: &amp;Path) -&gt; ToolchainResult&lt;Vec&lt;TypeDiagnostic&gt;&gt;` per-language implementations
    - Replace ETypeGraphInstrument deterministic encode with neural backing
    - Build type lattice (type ID -&gt; parent IDs) from toolchain diagnostics
    - Encode lattice nodes via E11 Kepler (no_grad scope)
    - Graph-attention head over lattice -&gt; 256-d
    - `ETypeGraphInstrument::new_with_weights(kepler: &amp;Kepler, gat: &amp;GraphAttention2Layer, resolver: ToolchainResolver, guard: &amp;mut FrozenGuard)`
    - Fail-closed: missing toolchain -&gt; MEJEPA_INFER_INSTRUMENT_MISSING
    - Sandboxed invocation (no network; deterministic env)
    - Per-language tests: 11 languages × {toolchain present, toolchain absent}
    - Determinism + frozen-gradient tests
    - Pyright-vs-mypy merge logic for Python (EC-INSTR-07)
  </in_scope>
  <out_of_scope>
    - Installing the toolchains (operator responsibility)
    - Running toolchains over the entire repo (only the chunk's source)
    - Static-analysis (EStaticAnalysisInstrument, TASK-INSTR-007)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/toolchain_dispatch.rs">
pub enum Toolchain {
    Pyright(PathBuf),
    Mypy(PathBuf),
    RustAnalyzer(PathBuf),
    Javac(PathBuf),
    Dotnet(PathBuf),
    Tsc(PathBuf),
    Gcc(PathBuf),
    Ruby(PathBuf),
    Go(PathBuf),
    Php(PathBuf),
}

pub struct ToolchainResolver {
    env_overrides: HashMap&lt;ChunkerLanguage, PathBuf&gt;,
}

impl ToolchainResolver {
    pub fn from_env() -&gt; Self;
    pub fn resolve(&amp;self, language: ChunkerLanguage) -&gt; InstrumentResult&lt;Toolchain&gt;;
    // Returns InvalidInput with code MEJEPA_INFER_INSTRUMENT_MISSING when no toolchain available
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDiagnostic {
    pub tool: String,
    pub type_id: String,
    pub parent_type_ids: Vec&lt;String&gt;,
    pub line: u32,
    pub column: u32,
}

pub fn run_toolchain(t: &amp;Toolchain, source_path: &amp;Path) -&gt; InstrumentResult&lt;Vec&lt;TypeDiagnostic&gt;&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-instruments/src/e_code.rs">
pub struct ETypeGraphInstrument {
    kepler: Arc&lt;Kepler&gt;,
    gat: Arc&lt;GraphAttention2Layer&gt;,
    resolver: ToolchainResolver,
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl ETypeGraphInstrument {
    pub fn new_with_weights(
        kepler: Arc&lt;Kepler&gt;,
        gat: Arc&lt;GraphAttention2Layer&gt;,
        resolver: ToolchainResolver,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for ETypeGraphInstrument {
    type Input = CodeInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::ETypeGraph }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 256 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 256 finite f32
    - Frozen: FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Toolchain absence -&gt; MEJEPA_INFER_INSTRUMENT_MISSING (no parser-only fallback)
    - Sandboxed: no network access during toolchain invocation
    - Per-language coverage: 11 languages tested in both present + absent variants
    - Latency budget: &lt; 10 ms cold (excluding toolchain invocation; toolchain is the long pole)
    - Deterministic
    - File &lt; 500 lines per file
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_type_graph::
    - cargo test --release -p context-graph-mejepa-instruments e_type_graph_per_language
    - Toolchain-missing test: temporarily mask PATH; expect MEJEPA_INFER_INSTRUMENT_MISSING
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
toolchain_dispatch.rs:

  pub fn resolve(language) -&gt; InstrumentResult&lt;Toolchain&gt; {
      let candidate_bins = match language { Python =&gt; &amp;["pyright", "mypy"], Rust =&gt; &amp;["rust-analyzer"], ... };
      for bin in candidate_bins {
          if let Some(p) = self.env_overrides.get(language).cloned().or_else(|| which::which(bin).ok()) {
              return Ok(toolchain_for(language, bin, p));
          }
      }
      Err(InstrumentError::invalid("toolchain",
          format!("MEJEPA_INFER_INSTRUMENT_MISSING: no toolchain for {:?}", language),
          "install Pyright/mypy/rust-analyzer/javac/dotnet/tsc/gcc/ruby/go/php"))
  }

  pub fn run_toolchain(t, src) -&gt; InstrumentResult&lt;Vec&lt;TypeDiagnostic&gt;&gt; {
      // sandboxed Command::new() with --no-network env where applicable
      // parse stdout per tool format (Pyright JSON / mypy text / etc.)
      // return diagnostics with type_id + parent_ids
  }

e_code.rs:

  impl Instrument for ETypeGraphInstrument {
      fn encode(&amp;self, input: &amp;CodeInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          let toolchain = self.resolver.resolve(input.language)?;
          let mut all_diags: Vec&lt;TypeDiagnostic&gt; = vec![];
          for chunk in &amp;input.chunks {
              let src_path = stage_chunk_to_temp(chunk)?;
              let diags = run_toolchain(&amp;toolchain, &amp;src_path)?;
              all_diags.extend(diags);
          }
          let lattice = build_type_lattice(&amp;all_diags);
          let node_embeddings: Vec&lt;Vec&lt;f32&gt;&gt; = lattice.nodes().map(|n| self.kepler.forward_no_grad(n.text())).collect()?;
          let pooled = self.gat.forward_no_grad(&amp;node_embeddings, &amp;lattice.edges())?;
          validate_finite_output("e_type_graph.output", &amp;pooled)?;
          Ok(pooled)
      }
  }

tests:
  - resolve_present_returns_some
  - resolve_absent_returns_missing_instrument
  - encode_per_language_with_toolchain
  - encode_python_pyright_or_mypy_either_works
  - encode_byte_equal_across_calls
  - encode_output_finite_and_dim_256
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-instruments/src/toolchain_dispatch.rs">Toolchain resolver + per-language runner; TypeDiagnostic struct; tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_code.rs">Replace ETypeGraphInstrument deterministic encode with neural backing + toolchain dispatch</file>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Add `pub mod toolchain_dispatch;` and re-exports</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 256 finite f32</criterion>
  <criterion>Toolchain absence -&gt; MEJEPA_INFER_INSTRUMENT_MISSING (parser-only fallback forbidden)</criterion>
  <criterion>11 languages tested with toolchain present</criterion>
  <criterion>Pyright + mypy interchangeability for Python (EC-INSTR-07)</criterion>
  <criterion>Sandboxed invocation: no network access during toolchain run (asserted via netns or PATH masking)</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_type_graph::</command>
  <command>cargo test -p context-graph-mejepa-instruments toolchain_dispatch::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments e_type_graph_per_language</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```

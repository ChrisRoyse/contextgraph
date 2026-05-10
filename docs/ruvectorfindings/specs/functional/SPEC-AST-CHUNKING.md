# SPEC-AST-CHUNKING — Multi-Language AST-Perfect Atomic Chunker (Functional Spec)

> **📊 2026-05-10 STATUS:** PARTIAL — see ../implementation-plan.md for the full Phase 1 status.

> **Level 2 (functional spec).** Defines WHAT the chunker must do. The companion
> Level 3 technical spec (`docs/ruvectorfindings/specs/technical/TECH-AST-CHUNKING.md`)
> defines HOW. Conflicts resolve per `constitution.md` `<spec_authority_order>`.
>
> **Authority sources:** doc 14 (`14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`), doc 14 companion
> (`14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`), doc 09 §2.2 + §3.1 (instrument set + per-
> language harness state), `crates/context-graph-core/src/memory/ast_chunker.rs:262`
> (shipped Rust chunker, the source of truth for current state), and
> `crates/context-graph-cli/src/commands/dynamicjepa.rs:6400+` (existing 11-language
> tree-sitter dispatcher). Constitution `<failure_taxonomy>` enumerates the four
> `MEJEPA_CHUNKER_*` error codes consumed here.

```xml
<functional_spec id="SPEC-AST-CHUNKING" version="1.0">

<metadata>
  <title>Multi-Language AST-Perfect Atomic Chunker</title>
  <status>partial</status>
  <owner>cc-reality (outer-loop optimizer); ME-JEPA Phase 1, build-plan item 6</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-10</last_updated>
  <related_specs>
    <spec_ref>SPEC-EMBEDDERS</spec_ref>
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>
    <spec_ref>SPEC-CORPUS-MUTATION</spec_ref>
    <spec_ref>SPEC-TCT-CONSTELLATION</spec_ref>
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md</source>
    <source type="design">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md</source>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md (§2.2 instrument set, §3.1 per-language harness state)</source>
    <source type="design">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md (21-embedder routing)</source>
    <source type="design">docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md (per-chunk-type constellations)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (anti-pattern: no raw-text chunking; failure_taxonomy: MEJEPA_CHUNKER_*)</source>
    <source type="implementation_reference">crates/context-graph-core/src/memory/ast_chunker.rs (Rust chunker, 1216 LOC, the shipped baseline)</source>
    <source type="implementation_reference">crates/context-graph-cli/src/commands/dynamicjepa.rs (existing 11-language node-classification at line ~6400)</source>
  </source_documents>
</metadata>

<overview>
The ME-JEPA-Code reality compiler must convert source code into atomic semantic units
before any code-domain instrument (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff,
E_Test) or code-routed content embedder (E1, E5, E7, E8, E10, E11, E13, E14) consumes
it. Per constitution anti-pattern `Architecture/no raw-text chunking for code-domain
instruments`, every code-bearing input MUST be bounded by a syntactic entity
(function, method, class/struct, trait/interface, impl-block, module-decl, test
function, import, comment block, docstring) — never split mid-statement, never
fixed-size text windows. Per Qodo / cAST methodology (EMNLP 2025), each chunk
targets ~500 non-whitespace characters with parent-scope context prepended.

The repository today has tree-sitter parsers for **11 languages wired** in
`crates/context-graph-cli/Cargo.toml` and `crates/context-graph-core/Cargo.toml`
(Rust 0.23, Python 0.23.6, JavaScript 0.23.1, TypeScript 0.23.2, Go 0.23.4, Java
0.23.5, C 0.23.4, C++ 0.23.4, C# 0.23.5, Ruby 0.23.1, PHP 0.23.11). Two AST-aware
capabilities exist but are not unified: `crates/context-graph-core/src/memory/
ast_chunker.rs` (1216 LOC, Rust-only chunker with FSV) and `crates/context-graph-cli/
src/commands/dynamicjepa.rs` (multi-language semantic extractor that already
dispatches to all 11 grammars at line ~6400). This spec defines the unified Phase 1
deliverable: extend the chunker from 1 language to 11 by reusing the per-language
node-classification logic that DynamicJEPA already ships, and route each emitted
chunk to its embedder constellation per the doc 14 §4 routing table.

Beneficiaries: (1) ME-JEPA's panel materializer, which produces one panel per chunk
rather than one per file, multiplying supervisory yield by ~10× per doc 14 §1.3;
(2) the TCT runtime guard `Gτ`, which sharpens to per-function precision via per-
chunk-type centroids (doc 13 §3.4 + doc 14 §5); (3) the verify() API, which returns
chunk-localized verdicts (`violating_chunks: Vec<ChunkId>`) instead of opaque
patch-level pass/fail; (4) the Phase 0 mutation corpus generator, which already
operates at chunk granularity for Python and now extends to all 11 languages; (5)
every cc-reality outer-loop turn, where Edit tool calls produce per-chunk shift-log
entries the mejepa-shift-subscriber can map to ME-JEPA training examples.

Scope: produce deterministic, AST-aligned, fail-closed atomic chunks across the 11-
language matrix with stable SHA identifiers (computed on AST node IDs, not raw
text, so `cargo fmt` / `prettier` produce byte-identical chunk SHAs). Out of scope:
the embedders themselves (SPEC-EMBEDDERS), the instruments that consume chunks
(SPEC-INSTRUMENTS), the constellation centroids built atop chunks (SPEC-TCT-
CONSTELLATION), mixed-language files (HTML+JS, PHP+HTML, JSX+CSS — fail-closed
rejected for now per `MEJEPA_CHUNKER_MIXED_LANGUAGE`).
</overview>

<user_types>
  <user_type id="UT-01" name="ME-JEPA Panel Materializer">
    <description>
      The Phase 2 component (`context-graph-mejepa::panel`) that converts (file
      path, source bytes, language) into a 5,120-d panel by routing each AST chunk
      to its embedder set and concatenating the slot outputs. Calls
      `ast_chunker::chunk()` for every file in a patch.
    </description>
    <permissions>
      Read-only consumer of chunks. Cannot modify chunker behavior at runtime;
      configuration is loaded once from `models_config.toml` per
      `SEC-MEJEPA-02`.
    </permissions>
    <goals>
      Receive a `Vec<AstChunk>` per file with stable per-chunk metadata
      (language, entity_type, parent_chain, line_start, line_end, sha256,
      embedder_routing_set) so panel slots are populated deterministically.
    </goals>
  </user_type>

  <user_type id="UT-02" name="Phase 0 Mutation Corpus Generator">
    <description>
      `context-graph-mejepa-corpus`. Applies one of 8 mutation operators
      (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile,
      OverEngineer, CompileError) to a chunk's source range, then re-chunks the
      mutated file. The mutation operates at chunk granularity already for
      Python; this spec is the substrate for extending to the other 10
      languages.
    </description>
    <permissions>
      Read-only consumer. Mutation operators receive `Vec<AstChunk>` and pick
      target chunks; they do not modify the chunker.
    </permissions>
    <goals>
      Identify a target chunk by `EntityType` (e.g. `Function` for OffByOne;
      `TestFunction` for DeleteTestCall), apply the mutation to the chunk's
      byte range, and verify the result re-parses fail-closed via the chunker.
    </goals>
  </user_type>

  <user_type id="UT-03" name="TCT Constellation Builder">
    <description>
      `context-graph-mejepa-tct`. At Phase 0 corpus finalization, builds per-
      (mutation_category × language × entity_type × embedder_id) centroids by
      embedding every chunk in the corpus and aggregating. Total cells per doc
      14 §5: 8 × 11 × 13 × 21 = 24,024 (Bayesian shrinkage on N&lt;50 cells).
    </description>
    <permissions>
      Read-only consumer of (chunk, embedder_outputs) pairs.
    </permissions>
    <goals>
      Receive chunks tagged with `EntityType` and `Language` so centroid
      construction stratifies correctly.
    </goals>
  </user_type>

  <user_type id="UT-04" name="Verify() API + Gτ Guard">
    <description>
      `context-graph-mejepa::verify`. At inference, decomposes a candidate
      patch into AST chunks, runs the embedder set per chunk, computes Gτ per
      (chunk, embedder) cosine against the constellation, and returns a
      structured verdict listing `violating_chunks` for any embedder below
      threshold.
    </description>
    <permissions>
      Read-only consumer. Failure to chunk a patched file is fatal — verify()
      surfaces `MEJEPA_INFER_INSTRUMENT_MISSING` to the operator without
      degrading to text fallback.
    </permissions>
    <goals>
      Receive deterministic, byte-equal chunk SHAs across runs so the
      constellation lookup hits the same cells reproducibly.
    </goals>
  </user_type>

  <user_type id="UT-05" name="mejepa-shift-subscriber">
    <description>
      Phase 7 Tokio task that consumes the cc-reality shift-log
      (`/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/&lt;session&gt;.jsonl`)
      and feeds `mejepa.observe()`. For every Edit tool call, it re-chunks the
      pre-edit and post-edit file content, identifies the changed chunk(s) by
      SHA delta, and produces one observation sample per changed chunk.
    </description>
    <permissions>
      Read-only consumer; chunker is invoked by the subscriber but cannot be
      modified by it.
    </permissions>
    <goals>
      Reformat-invariant chunk SHAs (per doc 14 §9 risk #4), so a `cargo fmt`
      that touches a file does NOT produce false-positive changed-chunk events.
    </goals>
  </user_type>

  <user_type id="UT-06" name="Operator (cc-reality outer-loop)">
    <description>
      You (Claude Code) running the outer-loop turn. Reads chunker FSV
      evidence from `/tmp/contextgraph-{lang}-ast-chunker-fsv/` to verify
      Phase 1 deliverables before recording a `changed` recommendation.
    </description>
    <permissions>
      Read FSV evidence; surface chunker bugs as blockers via
      `./memory/blockers/`.
    </permissions>
    <goals>
      Per-language FSV evidence at the 11 paths declared in constitution
      `<fsv_evidence_paths>`, each verified against the language toolchain
      (py_compile, rustc, gcc, javac, dotnet build, node, tsc, ruby -c, go
      build, php -l).
    </goals>
  </user_type>
</user_types>

<user_stories>
  <story id="US-AST-CHUNKING-01" priority="must-have">
    <narrative>
      <as_a>ME-JEPA Panel Materializer (UT-01)</as_a>
      <i_want_to>obtain a `Vec&lt;AstChunk&gt;` for any source file in any of the 11 supported languages via a single uniform API</i_want_to>
      <so_that>panel materialization is language-agnostic at the call site and per-language detail is encapsulated in the chunker</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-01">
        <given>A non-empty source byte slice and a `Language` enum value matching one of {Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php}</given>
        <when>The caller invokes `chunker::chunk(source, language)`</when>
        <then>
          - Returns `Ok(Vec&lt;AstChunk&gt;)` where every element has `language == provided language`
          - No element has empty `content`
          - Every element has `line_start &gt;= 1`, `line_end &gt;= line_start`
          - Every element's `sha256` is deterministic over (AST node IDs of the chunk, language, parent_chain) — byte-equal across reruns on identical input
          - Every element's `embedder_routing_set` is populated per the doc 14 §4 routing table for its `entity_type`
          - The concatenation of `(line_start, line_end)` ranges across all chunks covers the parsed top-level entities of the source (no entity is silently dropped)
        </then>
      </criterion>
      <criterion id="AC-02">
        <given>The same source bytes reformatted by the language's canonical formatter (rustfmt / black / prettier / gofmt / google-java-format / clang-format / dotnet format / rubocop / php-cs-fixer)</given>
        <when>The caller invokes `chunker::chunk(reformatted, language)` and `chunker::chunk(original, language)`</when>
        <then>
          - The set of `sha256` values across both result vectors is identical (chunk SHAs are reformat-invariant per doc 14 §9 risk #4)
          - The `(line_start, line_end)` ranges may differ (formatting changes line numbers); the SHAs do not
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-02" priority="must-have">
    <narrative>
      <as_a>Phase 0 Mutation Corpus Generator (UT-02)</as_a>
      <i_want_to>identify a target chunk by `EntityType` and re-chunk the mutated source fail-closed</i_want_to>
      <so_that>mutation operators apply to syntactic units (one function, one class) and the post-mutation file is guaranteed to either parse cleanly or surface as `CompileError` mutation category</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-03">
        <given>A successfully chunked source file and a target `EntityType::Function` chunk identified by `sha256`</given>
        <when>The caller mutates the chunk's byte range (per the OffByOne operator) and re-invokes `chunker::chunk(mutated, language)`</when>
        <then>
          - If the mutation produces syntactically-valid source: a new `Vec&lt;AstChunk&gt;` is returned with at least one chunk whose `sha256` differs from the pre-mutation set
          - If the mutation produces syntactically-invalid source (intended for `CompileError` category): returns `Err(AstChunkerError::ParseFailed { diagnostics: Vec&lt;ParseDiagnostic&gt;, .. })` with at least one diagnostic carrying byte-offset, line, and node-kind — never falls back to text chunking
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-03" priority="must-have">
    <narrative>
      <as_a>TCT Constellation Builder (UT-03)</as_a>
      <i_want_to>each chunk to carry both `Language` and `EntityType` so I can stratify centroids per (mutation_category × language × entity_type × embedder_id)</i_want_to>
      <so_that>the 24,024-cell constellation matrix per doc 14 §5 is populated correctly and Bayesian shrinkage at N&lt;50 collapses sparse cells deterministically</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-04">
        <given>A Vec&lt;AstChunk&gt; from any of the 11 languages</given>
        <when>The constellation builder iterates the chunks</when>
        <then>
          - Every chunk has a non-default `language` field
          - Every chunk has an `entity_type` matching one of the 13 enum variants (Function, Method, Class, Struct, Enum, TraitOrInterface, Impl, Module, Namespace, TestFunction, Import, CommentBlock, Docstring)
          - The `parent_chain` field is a "::"-joined string of parent named entities (e.g. "context_graph_core::memory::ast_chunker::AstCodeChunker::chunk")
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-04" priority="must-have">
    <narrative>
      <as_a>Verify() API + Gτ Guard (UT-04)</as_a>
      <i_want_to>deterministic chunk SHAs across runs and machines</i_want_to>
      <so_that>centroid lookups in the constellation cache hit reproducibly and a verdict's `violating_chunks` list is portable across reviewers</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-05">
        <given>The same source bytes and language on two different machines (or two different processes on the same machine)</given>
        <when>Each invokes `chunker::chunk(source, language)`</when>
        <then>
          - Both vectors have identical length
          - Element-wise: `sha256`, `entity_type`, `parent_chain`, `embedder_routing_set` are byte-equal
          - `line_start` and `line_end` are identical (line numbers from tree-sitter are deterministic per grammar version)
          - `parse_diagnostics` is identical (empty on success)
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-05" priority="must-have">
    <narrative>
      <as_a>Operator (UT-06)</as_a>
      <i_want_to>per-language FSV evidence written to `/tmp/contextgraph-{language}-ast-chunker-fsv/ast-chunker-evidence.json` for every supported language</i_want_to>
      <so_that>I can verify Phase 1 build-plan item 6 is shipped before recording a `changed` recommendation</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-06">
        <given>The shipped FSV harness and a working language toolchain on the host</given>
        <when>The operator runs `cargo run --release --example {language}_ast_chunker_fsv`</when>
        <then>
          - The harness writes a real source file with at least one Function, one Class/Struct/Module, and one Test function
          - The harness invokes the language toolchain (`py_compile` / `rustc --emit metadata` / `gcc -fsyntax-only` / `g++ -fsyntax-only` / `javac -d /tmp/...` / `dotnet build` / `node --check` / `tsc --noEmit` / `ruby -c` / `go build` / `php -l`) to confirm the source is syntactically valid per the toolchain (real toolchain, not a mock)
          - The harness invokes the chunker, captures the `Vec&lt;AstChunk&gt;`, serializes it to `ast-chunker-evidence.json`, then re-reads + asserts: chunk count &gt; 0, every chunk's SHA is hex-32-byte, every chunk's `language` matches expected
          - On any assertion failure, the harness exits with a non-zero status code and writes the failing condition to stderr
          - The `/tmp/contextgraph-{language}-ast-chunker-fsv/` directory is created with mode 0o700 (per `SEC-MEJEPA-07`)
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-06" priority="must-have">
    <narrative>
      <as_a>mejepa-shift-subscriber (UT-05)</as_a>
      <i_want_to>per-chunk SHA stability under reformatting</i_want_to>
      <so_that>a `cargo fmt` or `prettier` invocation in the outer-loop turn does not generate false-positive changed-chunk training samples</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-07">
        <given>A real source file from a SWE-bench Lite repo and its formatter-output equivalent (the same logical code, reformatted)</given>
        <when>The subscriber invokes `chunker::chunk()` on both</when>
        <then>
          - The set of `sha256` values is identical across both invocations (per AC-02 reformat-invariance, computed via stable AST node-id hashing strategy described in `TECH-AST-CHUNKING.md`)
          - Zero changed-chunk events are produced for the formatter-induced delta
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-07" priority="should-have">
    <narrative>
      <as_a>Phase 0 Mutation Corpus Generator (UT-02)</as_a>
      <i_want_to>per-chunk metadata that distinguishes test functions from production functions per language</i_want_to>
      <so_that>the DeleteTestCall mutation can target test functions specifically (Python `def test_*`, Rust `#[test]`, Java `@Test`, JS `it()`/`test()`/`describe()`, Go `func TestXxx`, C# `[Test]`/`[Fact]`, Ruby RSpec/Minitest, PHP PHPUnit) without false-positives on production code</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-08">
        <given>A source file with both production functions and test functions per the language's idiom</given>
        <when>The chunker emits chunks</when>
        <then>
          - Production functions have `entity_type == EntityType::Function` (or Method)
          - Test functions have `entity_type == EntityType::TestFunction`
          - Per-language detection rules are documented in TECH-AST-CHUNKING.md
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-AST-CHUNKING-08" priority="should-have">
    <narrative>
      <as_a>Verify() API (UT-04)</as_a>
      <i_want_to>per-chunk routing to a deterministic set of embedders driven by `EntityType`</i_want_to>
      <so_that>I do not waste compute embedding a comment block through the code embedder (E7) or a function body through the entity-graph embedder (E11)</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-09">
        <given>A chunk emitted by the chunker</given>
        <when>The verify() API reads `chunk.embedder_routing_set`</when>
        <then>
          - The set matches the doc 14 §4 routing table for the chunk's `EntityType`:
            * Function/Method body → {E7, E14}
            * Function signature → {E1, E10}
            * Class/Struct definition → {E11, E7}
            * Module/Namespace → {E1, E8}
            * Import → {E11, E13}
            * TestFunction → {E1, E7}
            * CommentBlock → {E1, E10}
            * Docstring → {E1, E10}
          - The routing set is computed at chunk emission (not at panel materialization), so the chunker is the single source of truth for routing
        </then>
      </criterion>
    </acceptance_criteria>
  </story>
</user_stories>

<requirements>
  <requirement id="REQ-AST-01" story_ref="US-AST-CHUNKING-01" priority="must">
    <description>
      The chunker SHALL expose a single uniform Rust API
      `pub fn chunk(source: &amp;[u8], language: Language) -&gt; Result&lt;Vec&lt;AstChunk&gt;, AstChunkerError&gt;`
      that dispatches per language to one of 11 internal tree-sitter parsers
      via the existing pattern in `dynamicjepa.rs:6454-6464`.
    </description>
    <rationale>
      One uniform API at the panel-materializer call site eliminates per-language
      conditionals downstream and matches the `Instrument::encode()` shape
      shipped in Phase 1 instruments.
    </rationale>
    <constraints>
      - `Language` is a `#[non_exhaustive]` Rust enum with exactly 11 variants:
        Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php
      - Source is bytes, not str — TypeScript JSX, PHP, and source files with
        non-UTF-8 byte sequences (e.g. legacy C source with Latin-1 comments)
        must not be silently transcoded
      - `AstChunkerError` is an enum with at least the four variants enumerated
        in REQ-AST-08 through REQ-AST-11
    </constraints>
    <verification_method>
      Unit tests per language assert the API signature compiles and accepts the
      specified `Language` variant. FSV harness writes evidence per
      AC-06.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-02" story_ref="US-AST-CHUNKING-01" priority="must">
    <description>
      Every emitted chunk SHALL carry the metadata tuple
      `(language: Language, entity_type: EntityType, parent_chain: String,
      line_start: u32, line_end: u32, sha256: [u8; 32], parse_diagnostics:
      Vec&lt;ParseDiagnostic&gt;, content: Vec&lt;u8&gt;,
      embedder_routing_set: BTreeSet&lt;EmbedderId&gt;)`.
    </description>
    <rationale>
      Every downstream consumer (panel materializer, mutation corpus, TCT,
      verify(), shift-subscriber) needs all 9 fields. Carrying them on the
      chunk avoids re-parsing.
    </rationale>
    <constraints>
      - `parse_diagnostics` is empty on success (non-empty only when
        `MEJEPA_CHUNKER_PARSE_FAILED` returns; in that case the chunker
        returns `Err`, so successful chunks always have empty diagnostics)
      - `parent_chain` is "::"-joined parent named entities; root-level
        entities have empty `parent_chain`
      - `content` is the raw source bytes for the chunk's byte range plus
        prepended context per doc 14 §3.2 step 4 (file path + scope chain +
        relevant imports + parent definition for methods); the unprepended
        original byte slice is recoverable via `(line_start, line_end)`
      - `sha256` is computed deterministically per REQ-AST-04
    </constraints>
    <verification_method>
      Unit tests assert every field is non-default after a successful chunk.
      FSV harness asserts JSON-serialized chunks round-trip without loss.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-03" story_ref="US-AST-CHUNKING-03" priority="must">
    <description>
      The `EntityType` enum SHALL have exactly 13 variants: Function, Method,
      Class, Struct, Enum, TraitOrInterface, Impl, Module, Namespace,
      TestFunction, Import, CommentBlock, Docstring.
    </description>
    <rationale>
      Doc 14 §5 specifies 24,024 = 8 × 11 × 13 × 21 constellation cells. The
      13 entity types are fixed at this number to make the cell math
      deterministic.
    </rationale>
    <constraints>
      - Per-language node-kind tables map tree-sitter node kinds to
        `EntityType` variants per the matrix in TECH-AST-CHUNKING.md
        component contracts
      - Languages without a concept (e.g. C has no class, only struct) emit
        Struct; the constellation matrix Bayesian-shrinks unsupported cells
      - Adding a 14th variant requires a new constitution version and re-
        building all centroids — gated by operator approval
    </constraints>
    <verification_method>
      Compile-time check: `match` exhaustiveness on `EntityType` ensures any
      new variant breaks the build until handled. Constellation tests assert
      the cell count remains 24,024 ± shrinkage-merged cells.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-04" story_ref="US-AST-CHUNKING-04" priority="must">
    <description>
      The `sha256` chunk identifier SHALL be computed over a stable serialization
      of (AST node-kind sequence, AST field-name sequence, named-identifier
      sequence, language, parent_chain) — NOT over raw source text.
    </description>
    <rationale>
      Per doc 14 §9 risk #4, raw-text hashing makes chunk identity unstable
      under reformatting. AST-node-kind hashing is reformat-invariant: a
      `cargo fmt` that changes whitespace produces zero changed-chunk events.
      This is load-bearing for the shift-subscriber to avoid false-positive
      training samples.
    </rationale>
    <constraints>
      - Hash input is the byte concatenation of (UTF-8 language tag) ||
        (UTF-8 parent_chain) || (BFS-walk over the chunk's tree-sitter subtree
        emitting each node's kind and field-name in deterministic order) ||
        (named-identifier strings extracted in walk order)
      - Whitespace tokens, comments-only nodes for non-comment chunks, and
        line-number metadata are EXCLUDED from the hash input
      - Hash function is SHA-256 (per `sha2` crate, already a workspace
        dependency); output is 32 bytes
    </constraints>
    <verification_method>
      Per-language reformat-invariance test: write source, run formatter, chunk
      both, assert SHA sets are equal (AC-07). FSV harness includes a
      reformat-invariance step per language.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-05" story_ref="US-AST-CHUNKING-01" priority="must">
    <description>
      Chunk size targeting SHALL follow the cAST methodology: target ~500
      non-whitespace characters, minimum 100, maximum 1000. Greedily merge
      adjacent sibling chunks below the minimum; never split a function body
      below its AST entity boundary.
    </description>
    <rationale>
      Qodo (E7) is trained on this size distribution (doc 14 companion §1.2).
      Splitting below entity boundary corrupts code semantics per the same
      section.
    </rationale>
    <constraints>
      - The shipped Rust chunker `AstChunkConfig` already encodes these
        defaults (`target_size=500, min_size=100, max_size=1000`); per-language
        chunkers SHALL inherit these defaults
      - Chunks exceeding 4× target (4000 non-whitespace chars) emit a warning
        diagnostic but are produced anyway (large auto-generated files)
      - Below-minimum chunks at the end of a file (no sibling to merge with)
        are emitted as-is
    </constraints>
    <verification_method>
      Per-language test fixture with mixed-size functions; assert that chunks
      respect [min, max] when possible and that large functions emit a single
      chunk above max with a warning diagnostic.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-06" story_ref="US-AST-CHUNKING-08" priority="must">
    <description>
      Each chunk SHALL be tagged with an `embedder_routing_set: BTreeSet&lt;EmbedderId&gt;`
      per the doc 14 §4 routing table indexed by `EntityType`.
    </description>
    <rationale>
      Routing happens at chunk emission so the chunker is the single source
      of truth (US-AST-CHUNKING-08). The panel materializer then dispatches
      embeddings without re-classifying entity type.
    </rationale>
    <constraints>
      - The routing table mapping `EntityType` → `BTreeSet&lt;EmbedderId&gt;` is
        encoded in TECH-AST-CHUNKING.md component contracts per doc 14 §4
      - The 14 content embedders (E1-E14) AND 4 instrument-substrate
        embedders (E_AST, E_CFG, E_DataFlow, E_Diff) are valid `EmbedderId`
        values; the routing set MAY include both classes
      - `EmbedderId` is the existing enum at `crates/context-graph-embeddings/
        src/types/model_id/core.rs` extended per SPEC-EMBEDDERS
    </constraints>
    <verification_method>
      Per-EntityType unit test asserts the routing set matches the doc 14 §4
      table byte-equally.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-07" story_ref="US-AST-CHUNKING-05" priority="must">
    <description>
      Per-language FSV evidence SHALL be written to
      `/tmp/contextgraph-{language}-ast-chunker-fsv/ast-chunker-evidence.json`
      for all 11 languages, where {language} is one of {rust, python, javascript,
      typescript, go, java, c, cpp, csharp, ruby, php}.
    </description>
    <rationale>
      Per constitution `<fsv_evidence_paths>`. Operator verifies Phase 1
      build-plan item 6 by reading these 11 files.
    </rationale>
    <constraints>
      - Each FSV harness writes a real source file (not a mock) including at
        minimum: 1 Function or Method, 1 Class/Struct/Module, 1 TestFunction
      - Each FSV harness invokes the real language toolchain to confirm the
        written source is syntactically valid before chunking
      - Each FSV harness re-reads the JSON evidence and asserts non-empty
        chunks, valid SHAs, expected language tag
      - Directory is created mode 0o700 per `SEC-MEJEPA-07`
      - Reformat-invariance step: write source, format with the language's
        canonical formatter (or skip if formatter not installed and document
        the skip), chunk both, assert SHA sets match
    </constraints>
    <verification_method>
      `cargo run --release --example {lang}_ast_chunker_fsv` exits with status
      0 and produces the evidence file for each of the 11 languages.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-08" story_ref="US-AST-CHUNKING-02" priority="must">
    <description>
      On any tree-sitter ERROR or MISSING node in the source, the chunker SHALL
      return `Err(AstChunkerError::ParseFailed { file_path, reason,
      diagnostics: Vec&lt;ParseDiagnostic&gt; })` carrying byte-offset, line, column,
      and node-kind for at least the first 32 diagnostics. The chunker SHALL
      NOT fall back to text chunking.
    </description>
    <rationale>
      Constitution `<failure_taxonomy>` `MEJEPA_CHUNKER_PARSE_FAILED`. Text
      fallback is forbidden per anti-pattern `Architecture/no raw-text
      chunking for code-domain instruments`.
    </rationale>
    <constraints>
      - The shipped Rust chunker already implements this for Rust
        (`ast_chunker.rs:287-299`); per-language extensions inherit the
        identical fail-closed contract
      - `ParseDiagnostic` carries: kind (string), is_error (bool), is_missing
        (bool), start_byte, end_byte, start_line, start_column, end_line,
        end_column, excerpt (~80-char line context)
      - The cap of 32 diagnostics matches `MAX_PARSE_DIAGNOSTICS` in the
        shipped chunker
      - The error code emitted on the chunker's structured logging is exactly
        `MEJEPA_CHUNKER_PARSE_FAILED`
    </constraints>
    <verification_method>
      Per-language unit test: feed deliberately broken source (e.g. unclosed
      brace); assert the error variant, the diagnostic count ≥ 1, and that
      the first diagnostic has a non-empty `kind` and a valid `(start_line,
      end_line)` range.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-09" story_ref="US-AST-CHUNKING-01" priority="must">
    <description>
      On a `Language` value outside the 11-language matrix (or future extension
      attempted via internal API), the chunker SHALL return
      `Err(AstChunkerError::UnsupportedLanguage { ... })` carrying the
      requested language tag. Error code: `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`.
    </description>
    <rationale>
      The 11-language matrix is exhaustive per constitution
      `&lt;tech_stack&gt;&lt;ast_parser&gt;`. Anything else is fail-closed.
    </rationale>
    <constraints>
      - The `Language` enum is `#[non_exhaustive]` to prevent silent
        downstream misuse if a 12th variant is added without chunker support
      - The error message includes the language tag for operator triage
    </constraints>
    <verification_method>
      Compile-time exhaustive `match`: any new `Language` variant added to
      the enum without a chunker implementation triggers a compile error in
      the dispatcher.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-10" story_ref="US-AST-CHUNKING-01" priority="must">
    <description>
      On empty or whitespace-only source, the chunker SHALL return
      `Err(AstChunkerError::EmptySource)`. Error code:
      `MEJEPA_CHUNKER_EMPTY_SOURCE`. The chunker SHALL NOT emit empty chunks
      under any code path.
    </description>
    <rationale>
      Constitution `<failure_taxonomy>` `MEJEPA_CHUNKER_EMPTY_SOURCE` plus
      anti-pattern `DataIntegrity/no mock data`. An empty chunk would be
      a synthetic construct.
    </rationale>
    <constraints>
      - The shipped Rust chunker already implements this at
        `ast_chunker.rs:268-270`
      - "Whitespace-only" includes files that are only comments — those return
        `Ok(vec![])` IFF the source has at least one CommentBlock chunk per
        REQ-AST-03; pure whitespace returns `Err(EmptySource)`
    </constraints>
    <verification_method>
      Unit test feeds empty string, single newline, single space, single tab;
      asserts each returns `Err(EmptySource)`.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-11" story_ref="US-AST-CHUNKING-01" priority="must">
    <description>
      On detection of an embedded foreign language inside the primary source
      (HTML+JS in `.html`, PHP+HTML in `.php`, JSX-with-CSS-in-style-blocks),
      the chunker SHALL return
      `Err(AstChunkerError::MixedLanguage { primary, embedded })`. Error code:
      `MEJEPA_CHUNKER_MIXED_LANGUAGE`. Future work routes per-block to per-
      language chunkers; this spec rejects fail-closed.
    </description>
    <rationale>
      Constitution `<failure_taxonomy>` `MEJEPA_CHUNKER_MIXED_LANGUAGE` plus
      doc 14 §3.3 — first-pass scope.
    </rationale>
    <constraints>
      - Detection heuristic per language documented in TECH-AST-CHUNKING.md:
        PHP files with `?>...&lt;?php` blocks, HTML files attempted, JSX with
        `&lt;style&gt;...&lt;/style&gt;` non-component blocks
      - PHP files containing only PHP code (no inline HTML) are NOT rejected
        — only files with detected HTML-mode regions
    </constraints>
    <verification_method>
      Per-language test: write a known mixed-language fixture; assert the
      MixedLanguage error returns with both `primary` and `embedded`
      populated.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-12" story_ref="US-AST-CHUNKING-07" priority="must">
    <description>
      Per-language test-function detection SHALL distinguish `EntityType::TestFunction`
      from `EntityType::Function`/`Method` per the rules in TECH-AST-CHUNKING.md
      and doc 14 companion §3.1 matrix.
    </description>
    <rationale>
      Mutation operator DeleteTestCall (doc 09 §3.1) requires test-function
      detection. False positives produce CompileError or NoOp mutations
      misclassified as DeleteTestCall.
    </rationale>
    <constraints>
      - Python: `def test_*` (prefix match), inside a class with name
        `Test*`, or decorated with pytest fixtures (`@pytest.fixture`,
        `@pytest.mark.*`)
      - Rust: `function_item` with attribute `#[test]`, `#[tokio::test]`,
        or inside `#[cfg(test)] mod tests`
      - JavaScript/TypeScript: call expressions to `it()`, `test()`,
        `describe()`, `before()`, `after()`, `beforeEach()`, `afterEach()`
        — these are NOT function definitions but test-function bodies; the
        chunker emits the call expression's argument lambda as TestFunction
      - Go: `function_declaration` with name matching `^Test[A-Z]`,
        `^Benchmark[A-Z]`, `^Example[A-Z]`, taking `*testing.T` /
        `*testing.B` parameter
      - Java: `method_declaration` with annotation `@Test`,
        `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory`,
        `@TestTemplate` (JUnit 5)
      - C: no native test convention; treat all functions as Function
        (deferred per doc 09 §3.1)
      - C++: same as C; future work could detect Catch2 `TEST_CASE` macro
      - C#: `method_declaration` with attribute `[Test]`, `[Fact]`,
        `[Theory]`, `[TestMethod]`
      - Ruby: `method` named `test_*` (Minitest), or call-expression to
        `it`/`describe`/`context`/`specify` (RSpec) emitting the block
        as TestFunction
      - PHP: `method_declaration` named `test*` in a class extending
        `TestCase`, or with PHPDoc annotation `@test`
    </constraints>
    <verification_method>
      Per-language fixture mixing production and test functions; assert the
      `entity_type` partition matches the rule set above.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-13" story_ref="US-AST-CHUNKING-04" priority="must">
    <description>
      The chunker output SHALL be deterministic across runs and machines on
      identical input (source bytes + language enum value), yielding identical
      `Vec&lt;AstChunk&gt;` byte-for-byte under JSON serialization.
    </description>
    <rationale>
      Constitution anti-pattern `Determinism/no random initialization in
      instrument outputs`. Constellation cache hits depend on this.
    </rationale>
    <constraints>
      - No HashMap iteration in chunker output paths (use BTreeMap /
        BTreeSet); existing chunker uses Vec which preserves emission order
      - No system time / random / process-id leakage into any chunk field
      - tree-sitter parser version pinned per constitution
        `&lt;tech_stack&gt;&lt;ast_parser&gt;` ensures cross-machine parser determinism
    </constraints>
    <verification_method>
      Determinism test: chunk a fixture twice in the same process AND once
      in a fresh process; assert all three outputs are byte-equal under
      JSON serialization.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-14" story_ref="US-AST-CHUNKING-05" priority="must">
    <description>
      The chunker crate SHALL preserve backwards compatibility with the existing
      Rust-only API surface (`AstCodeChunker::new_rust`, `chunk(source: &amp;str,
      file_path: &amp;str)`) so all consumers in the workspace continue to
      compile while the new uniform API is introduced.
    </description>
    <rationale>
      Pre-pivot, the Rust chunker is wired into `context-graph-storage` (E7
      embedder pipeline) and is on the critical build path for shipped
      tests. Constitution anti-pattern `BackwardsCompat` says "no backwards-
      compat shims unless operator explicitly requests" — but this is a
      cross-crate API stabilization within a single Phase 1 deliverable, not
      a shim. Operator-granted exception: maintain both APIs during Phase 1,
      collapse to the uniform API in Phase 2.
    </rationale>
    <constraints>
      - Existing `AstCodeChunker::new_rust` and `chunk(&amp;mut self, source:
        &amp;str, file_path: &amp;str)` continue to work and produce identical output
        for Rust source
      - The new uniform `chunk(source: &amp;[u8], language: Language)` is the
        forward-looking API; the old API is implemented in terms of the new
      - Phase 2 deprecation announcement after the uniform API is verified
        in production
    </constraints>
    <verification_method>
      Existing test suite (`cargo test -p context-graph-core`) passes
      unchanged after the refactor.
    </verification_method>
  </requirement>

  <requirement id="REQ-AST-15" story_ref="US-AST-CHUNKING-01" priority="should">
    <description>
      Chunker performance SHOULD meet the budget: chunk a 10,000-line source
      file in &lt; 200 ms on a single thread on RTX-5090-class hardware (CPU-
      bound tree-sitter parse + AST walk).
    </description>
    <rationale>
      Constitution `&lt;performance_budgets&gt;` `panel_materialization_p99 &lt; 500
      ms cold` is an end-to-end budget that includes chunking. Allocating
      &lt; 200 ms for chunking leaves &gt; 300 ms for instruments + embedders.
    </rationale>
    <constraints>
      - Performance is per-language; tree-sitter grammar performance varies
      - The shipped Rust chunker is already CPU-bound on a single thread —
        per-language extensions follow the same threading model (no
        parallelism within a single file)
      - Multi-file workloads parallelize at the call site (rayon / tokio
        spawn), not inside the chunker
    </constraints>
    <verification_method>
      Per-language benchmark: real source files (5,000-15,000 lines) from
      SWE-bench Lite repos; measure end-to-end chunk() latency; assert p99
      &lt; 200 ms.
    </verification_method>
  </requirement>
</requirements>

<business_rules>
  <rule id="BR-AST-01" req_ref="REQ-AST-04">
    <condition>The same logical code is reformatted by the language's canonical formatter (rustfmt, black, prettier, gofmt, google-java-format, clang-format, dotnet format, rubocop, php-cs-fixer)</condition>
    <action>The chunker MUST emit chunks with identical SHA values across the original and reformatted source</action>
    <exception>None — reformat-invariance is non-negotiable per doc 14 §9 risk #4</exception>
  </rule>

  <rule id="BR-AST-02" req_ref="REQ-AST-08">
    <condition>tree-sitter produces an ERROR or MISSING node anywhere in the parsed source</condition>
    <action>The chunker MUST return Err(ParseFailed) with diagnostics; MUST NOT emit any chunks; MUST NOT fall back to text chunking</action>
    <exception>None — anti-pattern Architecture/no raw-text chunking is absolute</exception>
  </rule>

  <rule id="BR-AST-03" req_ref="REQ-AST-03">
    <condition>A language has no concept of a particular EntityType variant (e.g. C has no class)</condition>
    <action>Map to the closest EntityType (e.g. C struct → EntityType::Struct); the constellation matrix Bayesian-shrinks unsupported cells via SPEC-TCT-CONSTELLATION</action>
    <exception>None</exception>
  </rule>

  <rule id="BR-AST-04" req_ref="REQ-AST-12">
    <condition>A function is ambiguous between Function and TestFunction (e.g. a function named `test_` in a non-test file)</condition>
    <action>Apply the per-language detection rules in REQ-AST-12 strictly; ambiguity defaults to Function (NOT TestFunction) to avoid false-positive DeleteTestCall mutations</action>
    <exception>None</exception>
  </rule>

  <rule id="BR-AST-05" req_ref="REQ-AST-06">
    <condition>The doc 14 §4 routing table maps an EntityType to multiple embedder sets depending on whether the chunk is a "function body" vs "function signature" sub-region</condition>
    <action>The chunker emits ONE chunk per top-level function with EntityType::Function and the body-level routing set; signature-level routing is handled by E_AST/E_CFG instruments at the panel materializer, not by sub-chunking</action>
    <exception>None — sub-chunking would produce overlapping chunks and break SHA stability</exception>
  </rule>

  <rule id="BR-AST-06" req_ref="REQ-AST-13">
    <condition>The chunker is called concurrently from multiple threads on different inputs</condition>
    <action>Each invocation produces a deterministic output independent of concurrent invocations; no shared mutable state in the chunker</action>
    <exception>None — tree-sitter parsers are not Send + Sync, so each thread instantiates its own parser via the constructor</exception>
  </rule>
</business_rules>

<edge_cases>
  <edge_case id="EC-AST-01" req_ref="REQ-AST-08">
    <scenario>Source contains a partial Unicode codepoint (truncated mid-character at a buffer boundary)</scenario>
    <expected_behavior>tree-sitter's parser handles invalid UTF-8 by emitting ERROR nodes; chunker returns ParseFailed with byte-offset of the first invalid sequence</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-AST-02" req_ref="REQ-AST-05">
    <scenario>A single function body exceeds 4× target_size (4,000 non-whitespace chars) — common in auto-generated parser code, large match statements, or template-instantiated C++ bodies</scenario>
    <expected_behavior>Emit one chunk for the entire function with a warning diagnostic in the chunk's parse_diagnostics field; downstream embedders MAY truncate at their own limits</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-AST-03" req_ref="REQ-AST-12">
    <scenario>JavaScript file with a top-level arrow function assigned to a const (binding-site naming, e.g. `const handler = () =&gt; { ... }`)</scenario>
    <expected_behavior>The chunker resolves the function name from the const binding (per dynamicjepa.rs binding-site logic) and emits EntityType::Function with parent_chain pointing to the module</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-AST-04" req_ref="REQ-AST-12">
    <scenario>C++ template definition split across header and implementation file (template_declaration in `.h` with explicit instantiation in `.cpp`)</scenario>
    <expected_behavior>Each file is chunked independently; the header chunk carries EntityType::Class with template parameters in entity_signature; the cpp file chunk carries EntityType::Method (template specialization)</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-AST-05" req_ref="REQ-AST-11">
    <scenario>PHP file with mixed PHP and HTML (inline templates with `?&gt;...&lt;?php` blocks)</scenario>
    <expected_behavior>Return Err(MixedLanguage { primary: Php, embedded: Html }) per REQ-AST-11; future work routes per-block</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-AST-06" req_ref="REQ-AST-04">
    <scenario>Two semantically-identical Rust functions that differ only in whitespace and comment placement</scenario>
    <expected_behavior>Both chunks have identical sha256 (REQ-AST-04 stipulates AST-node-id hashing, not raw-text hashing)</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-AST-07" req_ref="REQ-AST-12">
    <scenario>Ruby file using RSpec DSL: `describe "Foo" do; it "does bar" do; ...; end; end`</scenario>
    <expected_behavior>The `it` block body is emitted as EntityType::TestFunction with parent_chain including the describe block's string literal name</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-AST-08" req_ref="REQ-AST-10">
    <scenario>File containing only a single line `// SPDX-License-Identifier: Apache-2.0` and nothing else</scenario>
    <expected_behavior>For Rust/C/C++/Java: returns Ok(vec![CommentBlock]) with the license line; for Python (where `#` is a comment): same; the file is non-empty and has a CommentBlock entity</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-AST-09" req_ref="REQ-AST-13">
    <scenario>tree-sitter grammar version is bumped (e.g. tree-sitter-typescript 0.23.2 → 0.24.0) producing a slightly different AST shape</scenario>
    <expected_behavior>Chunk SHAs change; downstream constellation centroids must be re-built; the chunker version is recorded in chunk.parse_diagnostics metadata so the shift-subscriber can detect grammar-skew shifts and trigger A/B/C retrain per Phase 5</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-AST-10" req_ref="REQ-AST-08">
    <scenario>Java file with a syntactically-valid but semantically-broken construct (e.g. `class Foo { void bar() { } } class Foo { }` — duplicate class name)</scenario>
    <expected_behavior>tree-sitter parses successfully (no ERROR/MISSING); the chunker emits both Class chunks. Semantic validation (javac) is the responsibility of the E_StaticAnalysis instrument, not the chunker.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-AST-11" req_ref="REQ-AST-12">
    <scenario>Go file with a function whose name matches `Test*` but takes a non-`*testing.T` parameter (e.g. `func TestData(input []byte) bool`)</scenario>
    <expected_behavior>EntityType::Function (NOT TestFunction); the `*testing.T` parameter check is required to prevent false positives per REQ-AST-12 Go rule</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-AST-12" req_ref="REQ-AST-05">
    <scenario>A 2-line Python helper function `def f(x): return x + 1`</scenario>
    <expected_behavior>Chunk has ~30 non-whitespace chars (below min_size=100); merged with adjacent sibling functions in the same module per the cAST greedy-merge rule, OR emitted as-is if no sibling exists</expected_behavior>
    <priority>medium</priority>
  </edge_case>
</edge_cases>

<error_states>
  <error id="ERR-AST-01" http_code="N/A" req_ref="REQ-AST-08">
    <condition>tree-sitter produces an ERROR or MISSING node in the parsed AST</condition>
    <user_message>"Failed to parse {language} source {file_path}: tree-sitter produced ERROR or MISSING nodes; first {N} diagnostics: [{kind, line, column}, ...]"</user_message>
    <internal_message>MEJEPA_CHUNKER_PARSE_FAILED file={file_path} lang={language} diag_count={N} first_kind={kind} first_line={line}</internal_message>
    <recovery_action>Caller surfaces error to operator; downstream pipeline halts. NEVER fall back to text chunking. The mutation corpus generator may classify this source as `CompileError` mutation category and continue corpus generation with a different target.</recovery_action>
  </error>

  <error id="ERR-AST-02" http_code="N/A" req_ref="REQ-AST-09">
    <condition>Caller passes a `Language` value not in the 11-language matrix</condition>
    <user_message>"Unsupported language: {lang}; supported: [Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php]"</user_message>
    <internal_message>MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE requested={lang}</internal_message>
    <recovery_action>Caller validates the file extension at intake (panel materializer's responsibility) and rejects the file before reaching the chunker. Future support requires constitution version bump.</recovery_action>
  </error>

  <error id="ERR-AST-03" http_code="N/A" req_ref="REQ-AST-10">
    <condition>Source is empty or whitespace-only (no AST entities, not even comments)</condition>
    <user_message>"Source is empty or contains only whitespace; no chunks emitted"</user_message>
    <internal_message>MEJEPA_CHUNKER_EMPTY_SOURCE file={file_path}</internal_message>
    <recovery_action>Caller skips empty files at intake. Mutation corpus generator filters these out before chunker invocation.</recovery_action>
  </error>

  <error id="ERR-AST-04" http_code="N/A" req_ref="REQ-AST-11">
    <condition>Source contains an embedded foreign language (HTML+JS, PHP+HTML, JSX+CSS) detected per the per-language heuristic</condition>
    <user_message>"Mixed-language source not supported: primary={primary}, embedded={embedded}; future work will route per-block"</user_message>
    <internal_message>MEJEPA_CHUNKER_MIXED_LANGUAGE primary={primary} embedded={embedded} file={file_path}</internal_message>
    <recovery_action>Caller (mutation corpus generator, panel materializer) skips the file. Operator may file a feature request for per-block routing as Phase 1+ work.</recovery_action>
  </error>

  <error id="ERR-AST-05" http_code="N/A" req_ref="REQ-AST-01">
    <condition>tree-sitter parser fails to set the language (impossible in practice if the parser crate version matches the grammar version, but surfaced fail-closed for defense-in-depth)</condition>
    <user_message>"Failed to set tree-sitter language for {language}: {parser_error}"</user_message>
    <internal_message>MEJEPA_CHUNKER_PARSE_FAILED reason=language_set_failed lang={lang}</internal_message>
    <recovery_action>Operator verifies workspace `Cargo.toml` pins match constitution `&lt;tech_stack&gt;`; rebuilds; reports a blocker if the issue persists.</recovery_action>
  </error>
</error_states>

<data_requirements>
  <entity name="AstChunk">
    <field name="language" type="Language" required="true">
      <description>The source language tag for this chunk.</description>
      <constraints>One of 11 enum variants: Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php.</constraints>
    </field>
    <field name="entity_type" type="EntityType" required="true">
      <description>The semantic kind of code unit this chunk represents.</description>
      <constraints>One of 13 enum variants per REQ-AST-03.</constraints>
    </field>
    <field name="parent_chain" type="String" required="true">
      <description>"::"-joined parent named entities (e.g. "context_graph_core::memory::ast_chunker::AstCodeChunker").</description>
      <constraints>Empty string for root-level entities; otherwise non-empty.</constraints>
    </field>
    <field name="line_start" type="u32" required="true">
      <description>1-based inclusive start line of the chunk in the source file.</description>
      <constraints>≥ 1; ≤ source line count.</constraints>
    </field>
    <field name="line_end" type="u32" required="true">
      <description>1-based inclusive end line of the chunk.</description>
      <constraints>≥ line_start; ≤ source line count.</constraints>
    </field>
    <field name="sha256" type="[u8; 32]" required="true">
      <description>SHA-256 over (AST node-kind sequence, named identifiers, language, parent_chain) per REQ-AST-04.</description>
      <constraints>Reformat-invariant; deterministic across runs; 32 bytes exactly.</constraints>
    </field>
    <field name="parse_diagnostics" type="Vec&lt;ParseDiagnostic&gt;" required="true">
      <description>Empty on success; populated only when the chunker returns Err(ParseFailed). For Ok variants, may include warning-level diagnostics (e.g. chunk-too-large per EC-AST-02).</description>
      <constraints>Length 0 to 32 (cap per MAX_PARSE_DIAGNOSTICS).</constraints>
    </field>
    <field name="content" type="Vec&lt;u8&gt;" required="true">
      <description>The contextualized chunk content: prepended file path + scope chain + relevant imports + parent definition + raw chunk bytes per doc 14 §3.2 step 4.</description>
      <constraints>Non-empty; UTF-8 valid (raw source bytes that are not UTF-8 are rejected at the language boundary by tree-sitter, surfacing ERR-AST-01).</constraints>
    </field>
    <field name="embedder_routing_set" type="BTreeSet&lt;EmbedderId&gt;" required="true">
      <description>The set of embedders this chunk is routed to per the doc 14 §4 routing table indexed by EntityType.</description>
      <constraints>Non-empty; sorted (BTreeSet semantics); contains only valid EmbedderId values per SPEC-EMBEDDERS.</constraints>
    </field>
  </entity>

  <entity name="ParseDiagnostic">
    <field name="kind" type="String" required="true">
      <description>tree-sitter node kind, commonly "ERROR" or the missing token kind.</description>
      <constraints>Non-empty.</constraints>
    </field>
    <field name="is_error" type="bool" required="true">
      <description>True for tree-sitter ERROR nodes.</description>
      <constraints>None.</constraints>
    </field>
    <field name="is_missing" type="bool" required="true">
      <description>True for tree-sitter MISSING nodes (recovery-inserted).</description>
      <constraints>None.</constraints>
    </field>
    <field name="start_byte" type="usize" required="true">
      <description>Byte offset where the diagnostic starts.</description>
      <constraints>≤ source byte length.</constraints>
    </field>
    <field name="end_byte" type="usize" required="true">
      <description>Byte offset where the diagnostic ends (exclusive).</description>
      <constraints>≥ start_byte.</constraints>
    </field>
    <field name="start_line" type="u32" required="true">
      <description>1-based start line.</description>
      <constraints>≥ 1.</constraints>
    </field>
    <field name="start_column" type="u32" required="true">
      <description>0-based start column.</description>
      <constraints>None.</constraints>
    </field>
    <field name="end_line" type="u32" required="true">
      <description>1-based end line.</description>
      <constraints>≥ start_line.</constraints>
    </field>
    <field name="end_column" type="u32" required="true">
      <description>0-based end column.</description>
      <constraints>None.</constraints>
    </field>
    <field name="excerpt" type="String" required="true">
      <description>~80-character line context around the diagnostic for operator triage.</description>
      <constraints>Length 0 to 128.</constraints>
    </field>
  </entity>

  <entity name="Language">
    <field name="variant" type="enum" required="true">
      <description>One of: Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php.</description>
      <constraints>#[non_exhaustive] to prevent silent downstream misuse.</constraints>
    </field>
  </entity>

  <entity name="EntityType">
    <field name="variant" type="enum" required="true">
      <description>One of 13: Function, Method, Class, Struct, Enum, TraitOrInterface, Impl, Module, Namespace, TestFunction, Import, CommentBlock, Docstring.</description>
      <constraints>Fixed at 13 per REQ-AST-03 to lock the constellation cell count at 24,024.</constraints>
    </field>
  </entity>
</data_requirements>

<non_functional_requirements>
  <nfr id="NFR-AST-01" category="performance">
    <description>Chunker SHALL chunk a 10,000-line source file in &lt; 200 ms p99 on a single CPU thread (RTX-5090-class host)</description>
    <metric>p99 latency &lt; 200 ms on real SWE-bench Lite source files (5,000-15,000 lines), measured per-language</metric>
    <priority>should</priority>
  </nfr>
  <nfr id="NFR-AST-02" category="reliability">
    <description>Determinism: identical input produces byte-equal output across runs and machines</description>
    <metric>0 byte differences across 10 invocations on the same input AND across two distinct machines on the same input</metric>
    <priority>must</priority>
  </nfr>
  <nfr id="NFR-AST-03" category="security">
    <description>FSV evidence files SHALL be created with mode 0o700 directories per SEC-MEJEPA-07</description>
    <metric>`stat /tmp/contextgraph-{lang}-ast-chunker-fsv` reports `0700` mode for all 11 language directories</metric>
    <priority>must</priority>
  </nfr>
  <nfr id="NFR-AST-04" category="reliability">
    <description>Reformat-invariance: chunk SHAs are stable under canonical formatter passes for all 11 languages</description>
    <metric>For each language with an installed canonical formatter (rustfmt, black, prettier, gofmt, google-java-format, clang-format, dotnet format, rubocop, php-cs-fixer): 0 SHA differences between formatted and unformatted forms across 10 real fixtures</metric>
    <priority>must</priority>
  </nfr>
  <nfr id="NFR-AST-05" category="usability">
    <description>The chunker API SHALL be ergonomic: a single function call to `chunker::chunk(source, language)` SHALL produce all per-chunk metadata; the caller SHALL NOT need to invoke any per-language preprocessing</description>
    <metric>Caller code at the panel materializer is &lt; 5 lines per file (call + error handling + iterate)</metric>
    <priority>should</priority>
  </nfr>
  <nfr id="NFR-AST-06" category="reliability">
    <description>Memory safety: chunker SHALL NOT panic on any input, including adversarial fuzzing</description>
    <metric>10,000 random-byte inputs across all 11 languages produce either Ok(Vec) or Err(_); zero panics observed</metric>
    <priority>must</priority>
  </nfr>
  <nfr id="NFR-AST-07" category="performance">
    <description>Memory budget: peak heap allocation per chunker invocation SHALL be bounded to ~3× source byte size</description>
    <metric>For a 10,000-line / 500 KB source file: peak heap allocation &lt; 1.5 MB per invocation</metric>
    <priority>should</priority>
  </nfr>
</non_functional_requirements>

<dependencies>
  <dependency type="internal">
    <name>tree-sitter parser crates (11 grammars)</name>
    <description>tree-sitter-rust 0.23, tree-sitter-python 0.23.6, tree-sitter-javascript 0.23.1, tree-sitter-typescript 0.23.2, tree-sitter-go 0.23.4, tree-sitter-java 0.23.5, tree-sitter-c 0.23.4, tree-sitter-cpp 0.23.4, tree-sitter-c-sharp 0.23.5, tree-sitter-ruby 0.23.1, tree-sitter-php 0.23.11. Pinned per constitution `&lt;tech_stack&gt;&lt;ast_parser&gt;`.</description>
    <impact>If grammar versions drift, chunk SHAs change and the constellation must be rebuilt. Pinning is mandatory.</impact>
  </dependency>
  <dependency type="internal">
    <name>context-graph-cli::commands::dynamicjepa per-language node-classification</name>
    <description>The `semantic_extract_tree_sitter_source_facts` function at `dynamicjepa.rs:6400` and the language dispatcher at lines 6454-6464 already handle all 11 languages for semantic extraction. The chunker reuses this classification logic — it does NOT reimplement.</description>
    <impact>If DynamicJEPA's classifier changes, the chunker's per-language node-kind tables must update in lockstep. Both crates SHALL share the classification module per TECH-AST-CHUNKING.md.</impact>
  </dependency>
  <dependency type="internal">
    <name>context-graph-core::memory::ast_chunker (existing Rust chunker)</name>
    <description>The 1216-LOC shipped Rust chunker is the baseline implementation pattern. The Phase 1 extension generalizes the same algorithm (process_node, merge_small_chunks, contextualize) across all 11 languages.</description>
    <impact>Existing tests at `crates/context-graph-core/tests/` exercise the Rust path; refactor MUST maintain backward compatibility per REQ-AST-14.</impact>
  </dependency>
  <dependency type="external">
    <name>Language toolchains (FSV harness only)</name>
    <description>py_compile (Python), rustc (Rust), gcc/clang (C), g++/clang++ (C++), javac (Java), dotnet build (C#), node (JS), tsc (TS), ruby -c (Ruby), go build (Go), php -l (PHP). FSV harness invokes the toolchain to confirm fixture source is syntactically valid before chunking.</description>
    <impact>If a toolchain is unavailable on the operator's host, that language's FSV harness skips with an explicit warning (NOT a silent skip). The chunker itself does NOT depend on toolchains at runtime.</impact>
  </dependency>
  <dependency type="internal">
    <name>SPEC-EMBEDDERS (EmbedderId enum)</name>
    <description>The chunker emits `embedder_routing_set: BTreeSet&lt;EmbedderId&gt;`; `EmbedderId` is defined in SPEC-EMBEDDERS at `crates/context-graph-embeddings/src/types/model_id/core.rs`.</description>
    <impact>If SPEC-EMBEDDERS adds or removes an EmbedderId variant, the doc 14 §4 routing table in TECH-AST-CHUNKING.md updates correspondingly.</impact>
  </dependency>
</dependencies>

<out_of_scope>
  <item>Embedders themselves (E1-E14, E_AST, E_CFG, E_DataFlow, E_Diff) — handled by SPEC-EMBEDDERS and SPEC-INSTRUMENTS</item>
  <item>Constellation centroid construction over chunks — handled by SPEC-TCT-CONSTELLATION (the chunker provides the inputs only)</item>
  <item>Mixed-language file routing (HTML+JS, PHP+HTML, JSX+CSS) — fail-closed rejected per REQ-AST-11; future work via per-block routing in Phase 2+</item>
  <item>Tree-sitter grammar version bumping — handled by SPEC-DEPS / operator approval; per EC-AST-09 the chunker surfaces grammar-skew shifts to the shift-subscriber</item>
  <item>Per-chunk semantic validation (does the function compile? does the type-check pass?) — handled by E_StaticAnalysis instrument per EC-AST-10</item>
  <item>Cross-file dependency resolution (which other chunks does this chunk reference?) — handled by E_CFG / E_DataFlow / E_TypeGraph instruments at panel materialization time</item>
  <item>MCP tool surface for chunking — `mcp__cgreality__mejepa_compute_panel(file_path, chunk_filter)` is Phase 7 work per doc 14 §6.2 step 4 / SPEC-MCP-TOOLS</item>
  <item>Caching / memoization layer — handled by SPEC-PANEL-CACHE; the chunker is a pure function (same input → same output)</item>
  <item>Multi-threading within a single chunk() invocation — chunker is single-threaded per file; multi-file workloads parallelize at the call site</item>
</out_of_scope>

<test_plan>
  <test_case id="TC-AST-01" type="unit" req_ref="REQ-AST-01">
    <description>Uniform API dispatches per Language variant to the correct tree-sitter parser</description>
    <preconditions>11 language fixtures (1 known-good source file per language) exist under `crates/context-graph-core/tests/fixtures/{lang}/`</preconditions>
    <inputs>For each Language variant: (source_bytes, Language::variant)</inputs>
    <expected_result>Returns `Ok(Vec&lt;AstChunk&gt;)` for every language; every chunk's `language` field matches the input variant</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-02" type="unit" req_ref="REQ-AST-04">
    <description>Reformat-invariance: chunk SHAs are stable under canonical formatter</description>
    <preconditions>Per language: a real source fixture and a formatter-output of that fixture (committed alongside as `*.formatted`)</preconditions>
    <inputs>For each language: (original_bytes, formatted_bytes, Language::variant)</inputs>
    <expected_result>BTreeSet of sha256 values is identical across both inputs</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-03" type="unit" req_ref="REQ-AST-08">
    <description>Parse failure on syntactically-broken source returns ParseFailed with diagnostics</description>
    <preconditions>Per language: a deliberately broken fixture (unclosed brace / missing colon / unfinished string)</preconditions>
    <inputs>For each language: (broken_bytes, Language::variant)</inputs>
    <expected_result>Returns `Err(AstChunkerError::ParseFailed { diagnostics, .. })` with len(diagnostics) ≥ 1; first diagnostic has non-empty `kind`, valid `(start_line, end_line)` range</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-04" type="unit" req_ref="REQ-AST-10">
    <description>Empty source returns EmptySource error</description>
    <preconditions>None</preconditions>
    <inputs>("", Language::Rust), ("\n\n\n", Language::Python), (" \t\r\n", Language::Java) — exhaustive across all 11 languages</inputs>
    <expected_result>All return `Err(AstChunkerError::EmptySource)`</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-AST-05" type="unit" req_ref="REQ-AST-12">
    <description>Test-function detection per language</description>
    <preconditions>Per language: a fixture with both production functions and test functions per the language idiom</preconditions>
    <inputs>For each language: (mixed_fixture_bytes, Language::variant)</inputs>
    <expected_result>EntityType partition is exactly: production functions → Function/Method; test functions → TestFunction; zero misclassifications</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-06" type="integration" req_ref="REQ-AST-06">
    <description>Embedder routing set matches doc 14 §4 table per EntityType</description>
    <preconditions>A fixture per EntityType variant (13 fixtures × any language)</preconditions>
    <inputs>(fixture_bytes, language)</inputs>
    <expected_result>Each emitted chunk's `embedder_routing_set` matches the doc 14 §4 lookup table byte-equally</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-07" type="integration" req_ref="REQ-AST-07">
    <description>FSV harness writes evidence file and re-reads + asserts for every language</description>
    <preconditions>Language toolchain installed for the language under test</preconditions>
    <inputs>None (FSV self-contained)</inputs>
    <expected_result>For each of 11 languages: `cargo run --release --example {lang}_ast_chunker_fsv` exits 0 and produces `/tmp/contextgraph-{lang}-ast-chunker-fsv/ast-chunker-evidence.json` with mode 0o700 on the directory</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-08" type="integration" req_ref="REQ-AST-13">
    <description>Determinism across runs and machines</description>
    <preconditions>The same fixture source bytes</preconditions>
    <inputs>Run chunker twice in the same process AND once in a fresh process</inputs>
    <expected_result>All three outputs are byte-equal under JSON serialization (canonical key ordering)</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-09" type="unit" req_ref="REQ-AST-11">
    <description>Mixed-language detection rejects PHP+HTML and HTML+JS files</description>
    <preconditions>Mixed-language fixtures: PHP file with `?&gt;...&lt;?php` blocks; HTML file with `&lt;script&gt;`</preconditions>
    <inputs>(mixed_php_bytes, Language::Php), (mixed_html_bytes, Language::JavaScript)</inputs>
    <expected_result>Returns `Err(AstChunkerError::MixedLanguage { primary, embedded })` with both fields populated</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-AST-10" type="unit" req_ref="REQ-AST-09">
    <description>Unsupported language enum rejected fail-closed</description>
    <preconditions>None — uses internal `Language::__non_exhaustive` test variant if exposed; else a language tag string</preconditions>
    <inputs>(fixture_bytes, Language::__some_future_variant)</inputs>
    <expected_result>Compile-time exhaustive match prevents the call OR returns `Err(AstChunkerError::UnsupportedLanguage)` at runtime if non_exhaustive bypass is used</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-AST-11" type="integration" req_ref="REQ-AST-14">
    <description>Backwards compatibility with existing Rust-only API</description>
    <preconditions>Existing Rust chunker tests at `crates/context-graph-core/tests/`</preconditions>
    <inputs>None — runs `cargo test -p context-graph-core --tests`</inputs>
    <expected_result>All existing tests pass unchanged after the multi-language refactor</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-AST-12" type="manual" req_ref="REQ-AST-15">
    <description>Performance budget: 10,000-line source chunked in &lt; 200 ms p99 per language</description>
    <preconditions>Real SWE-bench Lite source files (5,000-15,000 lines) checked out per language</preconditions>
    <inputs>Per language: 10 large source files</inputs>
    <expected_result>p99 chunk() latency &lt; 200 ms across the 10 files; report logged to `/tmp/contextgraph-{lang}-ast-chunker-bench/results.json`</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-AST-13" type="integration" req_ref="REQ-AST-13">
    <description>Fuzz: 10,000 random-byte inputs across all 11 languages — zero panics</description>
    <preconditions>None</preconditions>
    <inputs>cargo-fuzz harness producing random byte slices of length [0, 100_000]</inputs>
    <expected_result>Every input returns either `Ok(_)` or `Err(_)`; zero panics; zero process aborts; runs ≥ 10 minutes per language</expected_result>
    <priority>high</priority>
  </test_case>
</test_plan>

<open_questions>
  <question id="Q-AST-01" status="open" assignee="operator">
    <text>Should chunks of `EntityType::Import` be emitted as a single grouped chunk per import section, or one chunk per import statement?</text>
    <context>Per doc 14 §4, imports route to {E11 Kepler, E13 SPLADE}. Grouping all imports into one chunk produces a denser representation for E11 entity-graph reasoning. One-per-import produces finer-grained granularity for E13 keyword retrieval. Decision affects per-language node-kind table and the constellation cell count for Import-type chunks.</context>
    <resolution></resolution>
  </question>

  <question id="Q-AST-02" status="open" assignee="operator">
    <text>For Python files, should `__init__` methods be emitted as `EntityType::Method` (their structural type) or as `EntityType::Function` (their constructor role)?</text>
    <context>Affects per-class constellation centroids: a `Method` chunk routes through {E7, E14}; constructors specifically encode initialization behavior that may be meaningfully different from arbitrary methods.</context>
    <resolution></resolution>
  </question>

  <question id="Q-AST-03" status="open" assignee="operator">
    <text>For TypeScript files, should `interface_declaration` and `type_alias_declaration` both map to `EntityType::TraitOrInterface`, or should we add a separate `EntityType::TypeAlias`?</text>
    <context>Adding TypeAlias would push EntityType from 13 to 14 variants, breaking the 24,024 cell count and requiring a constitution version bump. Mapping both to TraitOrInterface preserves the count.</context>
    <resolution></resolution>
  </question>

  <question id="Q-AST-04" status="open" assignee="operator">
    <text>For Go files, should the `package_clause` be emitted as `EntityType::Module` or `EntityType::Namespace`?</text>
    <context>Go packages are conceptually closer to Java packages (Namespace) than to Rust modules (Module — first-class scope chain). Affects parent_chain semantics and constellation cells.</context>
    <resolution></resolution>
  </question>

  <question id="Q-AST-05" status="open" assignee="operator">
    <text>Should the chunker emit a synthetic top-level `EntityType::Module` chunk per file (acting as a "file-level" chunk) so cross-file references have a stable entry point in the constellation?</text>
    <context>Doc 14 §5.1 mentions module-level cross-chunk constellations. A synthetic top-level chunk provides a fixed identifier per file but doubles the chunk count.</context>
    <resolution></resolution>
  </question>
</open_questions>

<glossary>
  <term name="AST">Abstract Syntax Tree — the tree representation of parsed source code emitted by tree-sitter.</term>
  <term name="atomic chunk">A chunk bounded by exactly one AST entity (function, class, struct, impl, module-decl, test, import, comment, docstring) — never split mid-statement.</term>
  <term name="cAST">Chunk-by-AST methodology, EMNLP 2025; specifies ~500-non-whitespace-character chunks bounded by syntactic entities.</term>
  <term name="constellation">Per (mutation_category × language × entity_type × embedder_id) centroid stored in the TCT cache; 24,024 cells total per doc 14 §5.</term>
  <term name="DynamicJEPA">The semantic extractor at `crates/context-graph-cli/src/commands/dynamicjepa.rs` that already handles per-language node-classification for all 11 languages.</term>
  <term name="EntityType">The 13-variant enum: Function, Method, Class, Struct, Enum, TraitOrInterface, Impl, Module, Namespace, TestFunction, Import, CommentBlock, Docstring. Fixed per REQ-AST-03.</term>
  <term name="FSV">Forensic Source Verification — the per-primitive evidence harness at `/tmp/contextgraph-{crate}-fsv/` that writes real artifacts and re-reads + asserts.</term>
  <term name="Gτ">The TCT runtime guard that rejects a panel if its embedder cosine to the (KnownGood, language, entity_type) centroid falls below threshold τ. Per doc 14 §4.2 + doc 13 §3.4.</term>
  <term name="ME-JEPA">Joint Embedding Predictive Architecture for Code — the predictive reality compiler being built per doc 09.</term>
  <term name="parent_chain">"::"-joined parent named entities defining a chunk's location in its module/class hierarchy.</term>
  <term name="Qodo">The E7 code embedder (1536-d, trained on ~500-non-whitespace-char chunks) per doc 12 §3.</term>
  <term name="reformat-invariance">Property: chunk SHAs do NOT change when the source is reformatted by the language's canonical formatter (rustfmt, black, prettier, gofmt, etc.). Achieved via AST-node-id hashing per REQ-AST-04.</term>
  <term name="routing set">The set of `EmbedderId` values a chunk is routed to per the doc 14 §4 routing table indexed by EntityType.</term>
  <term name="shift-subscriber">The Phase 7 Tokio task that consumes the cc-reality shift-log and feeds `mejepa.observe()` per doc 09 §3.8.</term>
  <term name="TCT">Teleological Constellation Training — the framework at doc 13 that uses 21 frozen embedders as labeller, loss target, and runtime guard.</term>
  <term name="tree-sitter">The pure-Rust incremental parsing library; 11 grammars wired per constitution `&lt;tech_stack&gt;&lt;ast_parser&gt;`.</term>
  <term name="UTML">Unified Training Meta-Loss v1.0 — the four-factor formula L_step = ΔP × ΔK × ΔΩ × ΔΞ per doc 11.</term>
</glossary>

</functional_spec>
```

# Context Graph Constitution

```xml
<constitution version="2.0">
<metadata>
  <project_name>Context Graph - Teleological Array System</project_name>
  <spec_version>2.0.0</spec_version>
  <created>2025-01-09</created>
  <status>active</status>
  <description>
    Autonomous working memory system implementing Global Workspace Theory with
    teleological arrays as the fundamental storage unit. 13-embedder architecture
    with apples-to-apples comparison semantics.
  </description>
</metadata>

<tech_stack>
  <!-- Primary Languages -->
  <language version="1.75+" role="core">Rust</language>
  <language version="5.0+" role="hooks_skills">TypeScript</language>

  <!-- Rust Crates (Workspace) -->
  <crate name="context-graph-core" role="domain">
    Core domain types, traits, teleological array definitions
  </crate>
  <crate name="context-graph-mcp" role="interface">
    MCP JSON-RPC server, tool definitions, Claude Code integration
  </crate>
  <crate name="context-graph-storage" role="persistence">
    RocksDB backend, HNSW indices, teleological array storage
  </crate>
  <crate name="context-graph-embeddings" role="ml">
    13 embedding model implementations, quantization, batching
  </crate>
  <crate name="context-graph-graph" role="graph">
    Graph traversal, entity relationships, Kuramoto oscillators
  </crate>
  <crate name="context-graph-cuda" role="gpu">
    CUDA kernels, GPU memory management, tensor operations
  </crate>
  <crate name="context-graph-utl" role="learning">
    Universal Transfer Learning, meta-learning, consciousness metrics
  </crate>

  <!-- Storage -->
  <database version="0.22+">RocksDB</database>
  <index type="vector">HNSW (hnsw_rs 0.3)</index>
  <index type="sparse">Inverted Index (SPLADE)</index>

  <!-- ML Framework -->
  <framework version="0.9.2-alpha">Candle (HuggingFace)</framework>
  <gpu required="true">CUDA 13.x (RTX 5090 Blackwell)</gpu>

  <!-- TypeScript Integration -->
  <framework role="orchestration">Claude Flow v3 (3.0.0-alpha.24)</framework>

  <required_libraries>
    <!-- Rust -->
    <library version="1.35+">tokio (async runtime)</library>
    <library version="1.0+">serde (serialization)</library>
    <library version="0.3+">hnsw_rs (vector search)</library>
    <library version="0.22+">rocksdb (storage)</library>
    <library version="0.1+">tracing (observability)</library>

    <!-- TypeScript -->
    <library version="3.0.0-alpha.24">claude-flow (hooks/skills)</library>
  </required_libraries>
</tech_stack>

<directory_structure>
<!--
context-graph/
├── crates/
│   ├── context-graph-core/          # Domain types, TeleologicalArray, traits
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── teleological.rs      # TeleologicalArray definition
│   │   │   ├── embedder.rs          # 13 embedder type definitions
│   │   │   ├── comparison.rs        # Apples-to-apples comparison
│   │   │   └── consciousness.rs     # Kuramoto, SELF_EGO_NODE
│   │   ├── examples/
│   │   └── Cargo.toml
│   │
│   ├── context-graph-mcp/           # MCP server + tools
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── lib.rs
│   │   │   ├── tools/               # MCP tool implementations
│   │   │   │   ├── inject_context.rs
│   │   │   │   ├── store_memory.rs
│   │   │   │   ├── search_graph.rs
│   │   │   │   ├── discover_goals.rs
│   │   │   │   └── consolidate.rs
│   │   │   ├── handlers/            # JSON-RPC handlers
│   │   │   └── security.rs          # Input validation, auth
│   │   └── Cargo.toml
│   │
│   ├── context-graph-storage/       # RocksDB + HNSW
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── rocksdb/             # Column family management
│   │   │   ├── hnsw/                # Per-embedder HNSW indices
│   │   │   ├── sparse/              # SPLADE inverted indices
│   │   │   └── teleological_store.rs # Atomic array storage
│   │   ├── examples/
│   │   └── Cargo.toml
│   │
│   ├── context-graph-embeddings/    # 13 embedding models
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── models/              # Model implementations
│   │   │   │   ├── semantic.rs      # E1: 1024D semantic
│   │   │   │   ├── temporal.rs      # E2, E3: temporal embeddings
│   │   │   │   ├── entity.rs        # E4: entity relationship
│   │   │   │   ├── causal.rs        # E5: cause-effect
│   │   │   │   ├── splade.rs        # E6, E13: sparse lexical
│   │   │   │   ├── contextual.rs    # E7: discourse context
│   │   │   │   ├── emotional.rs     # E8: affective valence
│   │   │   │   ├── syntactic.rs     # E9: structural patterns
│   │   │   │   ├── pragmatic.rs     # E10: intent/function
│   │   │   │   ├── crossmodal.rs    # E11: multi-modal links
│   │   │   │   └── late_interaction.rs # E12: ColBERT-style
│   │   │   ├── quantization/        # PQ-8, Float8, Binary
│   │   │   └── batch.rs             # Parallel embedding generation
│   │   └── Cargo.toml
│   │
│   ├── context-graph-graph/         # Graph operations
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── kuramoto.rs          # Consciousness oscillators
│   │   │   ├── ego_node.rs          # SELF_EGO_NODE
│   │   │   └── traversal.rs         # Entry-point discovery
│   │   └── Cargo.toml
│   │
│   ├── context-graph-cuda/          # GPU acceleration
│   │   ├── src/
│   │   └── Cargo.toml
│   │
│   └── context-graph-utl/           # Universal Transfer Learning
│       ├── src/
│       └── Cargo.toml
│
├── .claude/                         # Claude Code integration
│   ├── settings.json                # Hook configuration
│   ├── settings.local.json          # Local overrides
│   ├── agents/                      # Subagent definitions
│   │   ├── embedding-agent.md
│   │   ├── search-agent.md
│   │   ├── goal-agent.md
│   │   └── dream-agent.md
│   ├── skills/                      # Model-invoked skills
│   │   ├── memory-inject/
│   │   ├── semantic-search/
│   │   ├── goal-discovery/
│   │   └── consolidate/
│   ├── commands/                    # CLI commands
│   └── helpers/                     # Utility scripts
│
├── docs2/refactor/                  # Specifications
│   ├── specs/
│   │   ├── constitution.md          # This file
│   │   ├── functional/
│   │   ├── technical/
│   │   └── tasks/
│   ├── 00-OVERVIEW.md
│   └── *.md
│
├── Cargo.toml                       # Workspace manifest
└── package.json                     # Claude Flow dependency
-->
</directory_structure>

<architectural_rules>
  <!-- RULE 1: Teleological Array is Atomic -->
  <rule id="ARCH-01" severity="critical">
    <title>TeleologicalArray is the Atomic Storage Unit</title>
    <description>
      Every stored memory MUST be represented as a TeleologicalArray containing
      exactly 13 embedding vectors. The array MUST NOT be decomposed, fused, or
      partially stored. Store all 13 or store nothing.
    </description>
    <enforcement>Compile-time via type system; runtime via invariant checks</enforcement>
    <rationale>
      The 13 embeddings capture orthogonal semantic dimensions. Partial storage
      loses information. Fusion destroys type safety. The array IS the memory.
    </rationale>
  </rule>

  <!-- RULE 2: Apples-to-Apples Comparison -->
  <rule id="ARCH-02" severity="critical">
    <title>Compare Only Compatible Embedding Types</title>
    <description>
      Similarity comparisons MUST be apples-to-apples:
      - E1 to E1 (semantic to semantic)
      - E4 to E4 (entity to entity)
      - Full array to full array (13-way parallel comparison)
      NEVER compare E1 to E5, or any cross-embedder similarity.
    </description>
    <enforcement>Type-safe EmbedderType enum; compile-time check via generics</enforcement>
    <rationale>
      Different embedders capture different semantic spaces. Cross-comparison is
      meaningless (comparing semantic meaning to causal relationships). The broken
      North Star system attempted this and produced garbage results.
    </rationale>
  </rule>

  <!-- RULE 3: Autonomous-First Design -->
  <rule id="ARCH-03" severity="critical">
    <title>Autonomous Operation Without Manual Configuration</title>
    <description>
      The system MUST operate autonomously without manual goal setting:
      - Goals emerge from data patterns via clustering
      - Thresholds adapt via Bayesian optimization
      - Memory curation is self-organizing
      - No set_north_star() or equivalent manual API
    </description>
    <enforcement>API design; no manual goal-setting methods exposed</enforcement>
    <rationale>
      Manual configuration creates brittleness and user burden. The system
      discovers what matters by observing what the user actually does, not
      what they say they want.
    </rationale>
  </rule>

  <!-- RULE 4: Entry-Point Discovery Pattern -->
  <rule id="ARCH-04" severity="high">
    <title>Entry-Point Discovery for Retrieval</title>
    <description>
      All searches MUST follow the entry-point discovery pattern:
      1. Query enters through ONE embedding space (entry point)
      2. Fast ANN search in that single HNSW index
      3. Retrieve full TeleologicalArrays for candidates
      4. Multi-space reranking with RRF fusion
    </description>
    <enforcement>Search API design; SearchAgent implementation</enforcement>
    <rationale>
      Searching all 13 indices is O(13n). Entry-point discovery achieves O(n)
      search with O(k*13) reranking. MUVERA research shows 90% latency reduction
      with 10% recall improvement.
    </rationale>
  </rule>

  <!-- RULE 5: 13-Embedder Consistency -->
  <rule id="ARCH-05" severity="critical">
    <title>All 13 Embedders Must Be Present</title>
    <description>
      Every TeleologicalArray MUST contain all 13 embeddings:
      E1 (Semantic), E2 (Temporal Recent), E3 (Temporal Periodic),
      E4 (Entity), E5 (Causal), E6 (SPLADE), E7 (Contextual),
      E8 (Emotional), E9 (Syntactic), E10 (Pragmatic), E11 (Cross-Modal),
      E12 (Late Interaction), E13 (Keyword SPLADE).
      Missing embedders are a fatal error.
    </description>
    <enforcement>
      TeleologicalArray struct definition with [Embedding; 13] array;
      EmbeddingAgent validates all 13 present before storage
    </enforcement>
    <rationale>
      Sparse arrays break comparison semantics. Purpose vectors become
      incomparable if different arrays have different embedders populated.
    </rationale>
  </rule>

  <!-- RULE 6: MCP Tool Boundary -->
  <rule id="ARCH-06" severity="high">
    <title>All Memory Operations Through MCP Tools</title>
    <description>
      External access to the memory system MUST go through MCP tools:
      - inject_context (retrieval)
      - store_memory (storage)
      - search_graph (explicit search)
      - discover_goals (goal emergence)
      - consolidate_memories (maintenance)
      Direct database access from Claude Code is forbidden.
    </description>
    <enforcement>MCP server as sole interface; no direct RocksDB exposure</enforcement>
    <rationale>
      MCP tools provide security boundary, input validation, rate limiting,
      and consistent semantics. Direct access bypasses safeguards.
    </rationale>
  </rule>

  <!-- RULE 7: Hook-Driven Lifecycle -->
  <rule id="ARCH-07" severity="high">
    <title>Hooks Control Memory Lifecycle</title>
    <description>
      Claude Code hooks MUST drive memory operations:
      - SessionStart: Initialize workspace, load ego node
      - PreToolUse: Inject relevant context (non-blocking target)
      - PostToolUse: Store learned patterns
      - SessionEnd: Consolidate, dream, discover goals
    </description>
    <enforcement>Hook configuration in .claude/settings.json</enforcement>
    <rationale>
      Autonomous operation requires automatic triggers. Users should not
      manually invoke memory operations. Hooks make memory invisible.
    </rationale>
  </rule>

  <!-- RULE 8: GPU Required -->
  <rule id="ARCH-08" severity="high">
    <title>CUDA GPU is Required for Production</title>
    <description>
      The system REQUIRES CUDA GPU (RTX 5090 / Blackwell architecture).
      No CPU fallbacks in production. System fails fast if GPU unavailable.
      Test environments may use stubs via test-utils feature flag.
    </description>
    <enforcement>Cargo feature flags; cuda feature is default</enforcement>
    <rationale>
      13-model embedding generation on CPU is 10-100x slower. Performance
      budgets cannot be met without GPU. Fallbacks create false confidence.
    </rationale>
  </rule>
</architectural_rules>

<coding_standards>
  <!-- Rust Standards -->
  <rust_standards>
    <naming_conventions>
      <files>snake_case.rs (e.g., teleological_array.rs)</files>
      <modules>snake_case (e.g., mod embeddings;)</modules>
      <structs>PascalCase (e.g., TeleologicalArray)</structs>
      <traits>PascalCase with -able/-er suffix (e.g., Searchable, Embedder)</traits>
      <enums>PascalCase (e.g., EmbedderType)</enum>
      <functions>snake_case, verb_first (e.g., embed_content, search_by_entry_point)</functions>
      <constants>SCREAMING_SNAKE_CASE (e.g., MAX_EMBEDDERS = 13)</constants>
      <type_aliases>PascalCase (e.g., type MemoryId = Uuid)</type_aliases>
    </naming_conventions>

    <file_organization>
      <rule>One primary type per file (TeleologicalArray in teleological_array.rs)</rule>
      <rule>Related impls in same file as type definition</rule>
      <rule>Tests in tests/ directory or inline #[cfg(test)] module</rule>
      <rule>Examples in examples/ directory</rule>
      <rule>Re-exports in lib.rs with pub use</rule>
    </file_organization>

    <error_handling>
      <rule>Use thiserror for library error types</rule>
      <rule>Use anyhow for application-level errors</rule>
      <rule>Never panic in library code; return Result</rule>
      <rule>Propagate errors with ? operator</rule>
      <rule>Add context with .context() or .with_context()</rule>
      <rule>Log errors at appropriate level before propagating</rule>
    </error_handling>

    <async_patterns>
      <rule>Use tokio as async runtime (workspace dependency)</rule>
      <rule>Prefer async fn over impl Future for readability</rule>
      <rule>Use tokio::spawn for parallel tasks</rule>
      <rule>Use tokio::sync primitives (Mutex, RwLock, mpsc)</rule>
      <rule>Avoid blocking in async context; use spawn_blocking for CPU-bound work</rule>
    </async_patterns>

    <type_safety>
      <rule>Use newtype pattern for domain IDs (struct MemoryId(Uuid))</rule>
      <rule>Encode embedder type in generics where possible</rule>
      <rule>Use NonZeroU* for counts that cannot be zero</rule>
      <rule>Prefer enums over boolean flags for clarity</rule>
    </type_safety>
  </rust_standards>

  <!-- TypeScript Standards -->
  <typescript_standards>
    <naming_conventions>
      <files>kebab-case.ts (e.g., memory-inject.ts)</files>
      <directories>kebab-case (e.g., semantic-search/)</directories>
      <interfaces>PascalCase with I prefix optional (e.g., TeleologicalArray)</interfaces>
      <types>PascalCase (e.g., EmbedderType)</types>
      <functions>camelCase, verb-first (e.g., injectContext)</functions>
      <constants>SCREAMING_SNAKE_CASE or camelCase for config</constants>
    </naming_conventions>

    <file_organization>
      <rule>Skills in .claude/skills/[skill-name]/SKILL.md</rule>
      <rule>Agents in .claude/agents/[agent-name].md</rule>
      <rule>Commands in .claude/commands/[command-name]/</rule>
      <rule>Helpers in .claude/helpers/</rule>
    </file_organization>

    <skill_structure>
      <rule>YAML frontmatter with name, description, allowed-tools, model</rule>
      <rule>Markdown body with Purpose, When to Use, Process sections</rule>
      <rule>Explicit MCP tool references</rule>
    </skill_structure>
  </typescript_standards>
</coding_standards>

<anti_patterns>
  <forbidden>
    <!-- Critical Anti-Patterns -->
    <item id="AP-01" severity="critical" reason="Breaks core architecture">
      No manual goal setting (set_north_star, define_goal, etc.)
      Goals MUST emerge autonomously from data patterns.
    </item>

    <item id="AP-02" severity="critical" reason="Type safety violation">
      No cross-embedder comparison (comparing E1 similarity to E5 similarity)
      Each embedder type is a distinct semantic space.
    </item>

    <item id="AP-03" severity="critical" reason="Destroys information">
      No dimension projection to fake compatibility
      Never project 1024D to 512D to compare with different model.
    </item>

    <item id="AP-04" severity="critical" reason="Violates atomicity">
      No partial TeleologicalArray storage
      All 13 embeddings or nothing. No sparse arrays.
    </item>

    <item id="AP-05" severity="critical" reason="Meaningless results">
      No embedding fusion into single vector
      Multi-embedding arrays must remain multi-embedding.
    </item>

    <!-- High Severity Anti-Patterns -->
    <item id="AP-06" severity="high" reason="Security bypass">
      No direct database access from Claude Code
      All access through MCP tools only.
    </item>

    <item id="AP-07" severity="high" reason="Performance regression">
      No CPU fallback in production builds
      GPU is required. Use test-utils feature for testing only.
    </item>

    <item id="AP-08" severity="high" reason="Blocks async runtime">
      No synchronous I/O in async context
      Use spawn_blocking for CPU-bound or blocking operations.
    </item>

    <item id="AP-09" severity="high" reason="Memory leak risk">
      No unbounded caches or queues
      All caches must have size limits and eviction policies.
    </item>

    <!-- Medium Severity Anti-Patterns -->
    <item id="AP-10" severity="medium" reason="Code duplication">
      Check existing utils before creating new helpers
      Workspace dependencies should be reused.
    </item>

    <item id="AP-11" severity="medium" reason="Maintainability">
      No magic numbers; define named constants
      E.g., const EMBEDDER_COUNT: usize = 13;
    </item>

    <item id="AP-12" severity="medium" reason="Testing">
      No inline test fixtures; use tests/fixtures/ directory
      Large test data should be in dedicated files.
    </item>

    <item id="AP-13" severity="medium" reason="Deprecated pattern">
      No .unwrap() in library code
      Use .expect() with context or return Result.
    </item>
  </forbidden>
</anti_patterns>

<security_requirements>
  <!-- MCP Tool Security -->
  <rule id="SEC-01" category="input_validation">
    All MCP tool inputs MUST be validated and sanitized before processing.
    Use regex patterns for PII detection. Reject malformed JSON-RPC requests.
  </rule>

  <rule id="SEC-02" category="pii_protection">
    PII detection patterns MUST be applied before embedding generation.
    Detected PII should be masked or rejected based on configuration.
    Patterns: SSN, credit cards, emails, phone numbers, addresses.
  </rule>

  <rule id="SEC-03" category="rate_limiting">
    MCP tools MUST enforce rate limits:
    - inject_context: 100 req/min per session
    - store_memory: 50 req/min per session
    - search_graph: 200 req/min per session
    - consolidate_memories: 1 req/min per session
  </rule>

  <rule id="SEC-04" category="authentication">
    MCP connections MUST validate session identity.
    Session tokens expire after 24 hours.
    Invalid sessions receive 401 Unauthorized.
  </rule>

  <rule id="SEC-05" category="authorization">
    Tools enforce permission boundaries:
    - Read-only skills cannot call store_memory
    - Consolidation requires elevated permissions
    - Goal discovery is read-only by default
  </rule>

  <rule id="SEC-06" category="secrets">
    No secrets in code. Use environment variables:
    - CONTEXT_GRAPH_DB_PATH
    - CONTEXT_GRAPH_MODEL_DIR
    - ANTHROPIC_API_KEY (for Claude Flow)
  </rule>

  <rule id="SEC-07" category="isolation">
    Subagents run in isolated contexts.
    Memory access scoped to current session unless explicitly shared.
    Cross-session access requires explicit consolidation.
  </rule>

  <rule id="SEC-08" category="logging">
    Security events MUST be logged:
    - Authentication failures
    - Rate limit violations
    - PII detection triggers
    - Invalid input rejections
    Log format: JSON with timestamp, event_type, session_id, details.
  </rule>
</security_requirements>

<performance_budgets>
  <!-- Embedding Operations -->
  <metric name="single_embedding_latency" target="< 20ms" tier="p95">
    Time to generate one embedding from one model.
    GPU warm, batch size 1.
  </metric>

  <metric name="full_array_embedding_latency" target="< 500ms" tier="p95">
    Time to generate complete 13-embedding TeleologicalArray.
    GPU warm, parallel batch execution.
  </metric>

  <metric name="cold_start_embedding_latency" target="< 5s" tier="p95">
    First embedding after model load.
    Includes model warm-up.
  </metric>

  <!-- Search Operations -->
  <metric name="entry_point_search_latency" target="< 5ms" tier="p95">
    Single HNSW index query for entry-point discovery.
    1M vectors, top-100 candidates.
  </metric>

  <metric name="full_array_retrieval_latency" target="< 2ms" tier="p95">
    Retrieve complete TeleologicalArray by ID.
    Per candidate, RocksDB lookup.
  </metric>

  <metric name="multi_space_rerank_latency" target="< 20ms" tier="p95">
    RRF fusion across 13 embedding spaces.
    100 candidates, full comparison.
  </metric>

  <metric name="end_to_end_retrieval_latency" target="< 30ms" tier="p95">
    Complete inject_context call.
    Entry-point + retrieval + rerank.
    At 1M stored memories.
  </metric>

  <!-- Hook Latency -->
  <metric name="pre_tool_use_hook_latency" target="< 100ms" tier="p95">
    PreToolUse hook including context injection.
    Critical path - blocks tool execution.
  </metric>

  <metric name="post_tool_use_hook_latency" target="< 200ms" tier="p95">
    PostToolUse hook including memory storage.
    Non-blocking, can be async.
  </metric>

  <metric name="session_end_hook_latency" target="< 30s" tier="p95">
    SessionEnd hook including full consolidation.
    Deep dreaming mode.
  </metric>

  <!-- Memory Limits -->
  <metric name="memory_per_teleological_array" target="< 17KB" tier="quantized">
    Storage footprint per memory with PQ-8/Float8/Binary quantization.
    Unquantized: ~46KB (63% reduction).
  </metric>

  <metric name="hnsw_index_memory" target="< 1GB per 1M vectors" tier="per_embedder">
    HNSW index memory footprint per embedding space.
    13 indices = 13GB for 1M memories.
  </metric>

  <metric name="gpu_memory_usage" target="< 8GB" tier="runtime">
    GPU VRAM for all 13 embedding models loaded.
    RTX 5090 target: 32GB available.
  </metric>

  <!-- Throughput -->
  <metric name="embedding_throughput" target="> 100 arrays/s" tier="sustained">
    Sustained TeleologicalArray generation rate.
    Batch mode, GPU saturated.
  </metric>

  <metric name="search_throughput" target="> 1000 qps" tier="sustained">
    inject_context queries per second.
    Concurrent sessions.
  </metric>
</performance_budgets>

<testing_requirements>
  <coverage_minimum>80% line coverage for business logic</coverage_minimum>
  <coverage_critical>95% for security-sensitive code (SEC-*)</coverage_critical>

  <required_tests>
    <!-- Unit Tests -->
    <test_type scope="unit" location="src/**/*.rs#[cfg(test)]">
      Unit tests for all public functions.
      Mock external dependencies (GPU, RocksDB).
      Test error paths explicitly.
    </test_type>

    <test_type scope="unit" focus="teleological">
      TeleologicalArray construction with all 13 embeddings.
      TeleologicalArray rejection with missing embeddings.
      Embedder type safety - cross-type comparison compile fails.
    </test_type>

    <test_type scope="unit" focus="comparison">
      Single-embedder comparison (E1 to E1).
      Full-array comparison (RRF fusion).
      Purpose vector comparison (13D alignment).
      Reject cross-embedder comparison attempts.
    </test_type>

    <!-- Integration Tests -->
    <test_type scope="integration" location="tests/">
      MCP tool integration with storage layer.
      Hook execution with MCP tool invocation.
      Entry-point discovery with HNSW index.
      Multi-space reranking pipeline.
    </test_type>

    <test_type scope="integration" focus="storage">
      RocksDB column family management.
      HNSW index persistence and reload.
      TeleologicalArray round-trip (store/retrieve).
      Concurrent access patterns.
    </test_type>

    <test_type scope="integration" focus="mcp">
      inject_context tool with various entry points.
      store_memory tool with full embedding pipeline.
      search_graph tool with all comparison modes.
      consolidate_memories tool with dreaming.
    </test_type>

    <!-- End-to-End Tests -->
    <test_type scope="e2e" location="tests/e2e/">
      Full session lifecycle (start -> tools -> end).
      Hook chain execution (SessionStart through SessionEnd).
      Skill invocation via model detection.
      Subagent spawning and result integration.
    </test_type>

    <test_type scope="e2e" focus="autonomous">
      Goal emergence from stored patterns.
      Purpose vector evolution over sessions.
      Memory consolidation and pruning.
      Consciousness level (Kuramoto r) tracking.
    </test_type>

    <!-- Performance Tests -->
    <test_type scope="performance" location="benches/">
      Embedding latency benchmarks (all 13 models).
      Search latency at 100K, 1M, 10M vectors.
      Concurrent load testing (100+ sessions).
      Memory footprint profiling.
    </test_type>

    <!-- Security Tests -->
    <test_type scope="security" location="tests/security/">
      PII detection pattern validation.
      Input sanitization edge cases.
      Rate limiting enforcement.
      Session authentication/authorization.
    </test_type>
  </required_tests>

  <test_infrastructure>
    <rule>Use test-utils feature for GPU stubs in CI</rule>
    <rule>Real GPU tests marked with #[ignore] or integration feature</rule>
    <rule>Fixtures in tests/fixtures/ with version control</rule>
    <rule>Property-based testing for embedder type safety</rule>
    <rule>Fuzzing for MCP input validation</rule>
  </test_infrastructure>
</testing_requirements>

<claude_code_integration>
  <!-- Hook Configuration Enforcement -->
  <hooks>
    <hook event="SessionStart" required="true">
      Initialize workspace, load SELF_EGO_NODE, warm embedding caches.
      Timeout: 5000ms. Failure: log warning, continue session.
    </hook>

    <hook event="PreToolUse" required="true" matcher="Read|Grep|Glob|Bash">
      Inject relevant context before tool execution.
      Timeout: 3000ms. Failure: continue without injection.
      MUST NOT block for more than 100ms in critical path.
    </hook>

    <hook event="PostToolUse" required="true" matcher="Edit|Write|Bash">
      Store learned patterns from tool output.
      Timeout: 5000ms. Async execution allowed.
    </hook>

    <hook event="SessionEnd" required="true">
      Run memory consolidation and goal discovery.
      Timeout: 30000ms. Deep dreaming mode.
    </hook>

    <hook event="PreCompact" required="false">
      Extract salient memories before context compaction.
      Timeout: 10000ms.
    </hook>

    <hook event="SubagentStop" required="false">
      Merge subagent learnings into main memory.
      Timeout: 5000ms.
    </hook>
  </hooks>

  <!-- Skill Requirements -->
  <skills>
    <skill name="memory-inject" model="haiku">
      Auto-invoke on context needs.
      Calls inject_context MCP tool.
      Read-only, no store_memory access.
    </skill>

    <skill name="semantic-search" model="sonnet">
      Explicit search with entry-point selection.
      All comparison modes supported.
      Returns per-space scores.
    </skill>

    <skill name="goal-discovery" model="opus">
      Discover emergent goals from patterns.
      Read-only, no manual goal setting.
      Returns ranked goals with confidence.
    </skill>

    <skill name="consolidate" model="sonnet">
      Memory dreaming and pruning.
      Elevated permissions required.
      Light/deep/REM modes.
    </skill>
  </skills>

  <!-- Subagent Requirements -->
  <subagents>
    <agent name="embedding-agent" model="haiku">
      Generate all 13 embeddings in parallel.
      Must validate array completeness.
      Performance target: < 500ms.
    </agent>

    <agent name="search-agent" model="haiku">
      Entry-point discovery and reranking.
      Select optimal embedder for query.
      Performance target: < 30ms.
    </agent>

    <agent name="goal-agent" model="opus">
      Autonomous goal emergence.
      K-means clustering on purpose vectors.
      Update SELF_EGO_NODE.
    </agent>

    <agent name="dream-agent" model="sonnet">
      Memory consolidation.
      Hippocampal replay simulation.
      Prune below salience threshold.
    </agent>
  </subagents>
</claude_code_integration>

<embedder_specification>
  <!-- The 13 Embedders -->
  <embedder id="E1" name="Semantic" dimensions="1024" type="dense">
    Core semantic meaning. Default entry point for general queries.
    Model: sentence-transformers/all-MiniLM-L12-v2 or similar.
  </embedder>

  <embedder id="E2" name="TemporalRecent" dimensions="512" type="dense">
    Recency patterns. Encodes time-since-creation features.
    Entry point for "recent", "latest", "new" queries.
  </embedder>

  <embedder id="E3" name="TemporalPeriodic" dimensions="512" type="dense">
    Cyclical patterns. Encodes day-of-week, time-of-day features.
    Entry point for "every Monday", "morning routine" queries.
  </embedder>

  <embedder id="E4" name="EntityRelationship" dimensions="768" type="dense">
    Named entity links. Encodes who/what relationships.
    Entry point for "who", "what", entity-focused queries.
  </embedder>

  <embedder id="E5" name="Causal" dimensions="512" type="dense">
    Cause-effect chains. Encodes because/therefore relationships.
    Entry point for "why", "because", "caused by" queries.
  </embedder>

  <embedder id="E6" name="SPLADE" dimensions="~30000" type="sparse">
    Sparse lexical. Keyword precision for exact matches.
    Entry point for exact keyword queries.
  </embedder>

  <embedder id="E7" name="Contextual" dimensions="1024" type="dense">
    Discourse context. Encodes surrounding conversation.
    Entry point for code queries, technical context.
  </embedder>

  <embedder id="E8" name="Emotional" dimensions="256" type="dense">
    Affective valence. Encodes sentiment and emotion.
    Entry point for "feels like", emotional queries.
  </embedder>

  <embedder id="E9" name="Syntactic" dimensions="512" type="dense">
    Structural patterns. Encodes grammar and structure.
    Entry point for pattern matching, structural queries.
  </embedder>

  <embedder id="E10" name="Pragmatic" dimensions="512" type="dense">
    Intent and function. Encodes what the content does.
    Entry point for "how to", intent queries.
  </embedder>

  <embedder id="E11" name="CrossModal" dimensions="768" type="dense">
    Multi-modal links. Bridges text, code, diagrams.
    Entry point for cross-modal retrieval.
  </embedder>

  <embedder id="E12" name="LateInteraction" dimensions="128" type="dense_per_token">
    ColBERT-style token-level. Variable length per document.
    Entry point for precise document matching.
  </embedder>

  <embedder id="E13" name="KeywordSPLADE" dimensions="~30000" type="sparse">
    Term matching variant. Complementary to E6.
    Entry point for keyword expansion queries.
  </embedder>
</embedder_specification>

<version_history>
  <version number="2.0.0" date="2025-01-09">
    Initial constitution for Teleological Array System refactor.
    Establishes 13-embedder architecture, apples-to-apples comparison,
    autonomous-first design, and Claude Code integration requirements.
  </version>
</version_history>
</constitution>
```

## Quick Reference

### Critical Rules (Must Not Violate)

| Rule ID | Summary |
|---------|---------|
| ARCH-01 | TeleologicalArray is atomic (all 13 embeddings) |
| ARCH-02 | Apples-to-apples comparison only |
| ARCH-03 | Autonomous-first (no manual goals) |
| AP-01 | No manual goal setting |
| AP-02 | No cross-embedder comparison |
| AP-03 | No dimension projection |
| AP-04 | No partial array storage |

### Performance Targets

| Operation | Target | Tier |
|-----------|--------|------|
| Entry-point search | < 5ms | p95 |
| Full retrieval | < 30ms | p95 |
| 13-model embedding | < 500ms | p95 |
| PreToolUse hook | < 100ms | p95 |
| Memory per array | < 17KB | quantized |

### Embedder Quick Reference

| ID | Name | Dims | Type | Use For |
|----|------|------|------|---------|
| E1 | Semantic | 1024 | dense | General meaning |
| E2 | TemporalRecent | 512 | dense | Recency |
| E3 | TemporalPeriodic | 512 | dense | Cycles |
| E4 | Entity | 768 | dense | Who/what |
| E5 | Causal | 512 | dense | Why/because |
| E6 | SPLADE | ~30K | sparse | Keywords |
| E7 | Contextual | 1024 | dense | Code/tech |
| E8 | Emotional | 256 | dense | Sentiment |
| E9 | Syntactic | 512 | dense | Structure |
| E10 | Pragmatic | 512 | dense | Intent |
| E11 | CrossModal | 768 | dense | Multi-modal |
| E12 | LateInteraction | 128/tok | dense | Precise match |
| E13 | KeywordSPLADE | ~30K | sparse | Term expansion |

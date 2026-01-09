# TASK-INTEG-006: File Operation Hooks Implementation

```xml
<task_spec>
  <task_id>TASK-INTEG-006</task_id>
  <title>File Operation Hooks (PreFileRead, PostFileRead)</title>
  <status>pending</status>

  <objective>
    Implement PreFileRead and PostFileRead hook handlers that provide context
    preloading and automatic content embedding when files are accessed during
    Claude Code sessions.
  </objective>

  <rationale>
    File read hooks enable:
    1. PreFileRead: Preload related context before file access
    2. PostFileRead: Automatically embed file content as teleological arrays
    3. Context augmentation: Inject relevant memories alongside file content
    4. Access tracking: Build understanding of file access patterns

    These hooks transform passive file reads into active context building.
  </rationale>

  <dependencies>
    <dependency type="required">TASK-INTEG-004</dependency>    <!-- Hook protocol -->
    <dependency type="required">TASK-CORE-003</dependency>     <!-- TeleologicalArray type -->
    <dependency type="required">TASK-LOGIC-001</dependency>    <!-- Search engine -->
  </dependencies>

  <input_context_files>
    <file purpose="hook_protocol">crates/context-graph-mcp/src/hooks/protocol.rs</file>
    <file purpose="search_engine">crates/context-graph-storage/src/teleological/search/engine.rs</file>
    <file purpose="embedder_pipeline">crates/context-graph-core/src/teleology/embedder.rs</file>
    <file purpose="store">crates/context-graph-storage/src/teleological/store.rs</file>
  </input_context_files>

  <output_artifacts>
    <artifact type="source">crates/context-graph-mcp/src/hooks/file_read.rs</artifact>
    <artifact type="config">.claude/hooks/pre_file_read.sh</artifact>
    <artifact type="config">.claude/hooks/post_file_read.sh</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/file_read_hooks_test.rs</artifact>
  </output_artifacts>

  <definition_of_done>
    <criterion id="1">PreFileRead handler searches for related context before reads</criterion>
    <criterion id="2">PostFileRead handler embeds file content as teleological array</criterion>
    <criterion id="3">Context injection provides relevant memories to augment file content</criterion>
    <criterion id="4">File access patterns tracked for future preloading optimization</criterion>
    <criterion id="5">Configurable file extensions and size limits</criterion>
    <criterion id="6">Hook latency under 50ms for pre-read operations</criterion>
    <criterion id="7">Deduplication prevents re-embedding unchanged files</criterion>
    <criterion id="8">Shell scripts work with Claude Code's file read workflow</criterion>
  </definition_of_done>

  <estimated_complexity>Medium</estimated_complexity>

  <pseudo_code>
    <section name="PreFileReadHandler">
```rust
// crates/context-graph-mcp/src/hooks/file_read.rs

use crate::hooks::protocol::{HookEvent, HookPayload, HookResponse, HookStatus};
use context_graph_storage::teleological::{TeleologicalStore, SearchEngine, SearchQuery};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Pre-file-read hook handler for context preloading
pub struct PreFileReadHandler {
    /// Teleological store for searching
    store: Arc<TeleologicalStore>,
    /// Search engine for finding related content
    search_engine: Arc<SearchEngine>,
    /// File access tracker
    access_tracker: Arc<FileAccessTracker>,
    /// Configuration
    config: PreFileReadConfig,
}

#[derive(Debug, Clone)]
pub struct PreFileReadConfig {
    /// Maximum time to spend on preloading (ms)
    pub max_preload_latency_ms: u64,
    /// Number of related memories to fetch
    pub context_limit: usize,
    /// Minimum similarity score for context injection
    pub min_context_similarity: f32,
    /// File extensions to process
    pub process_extensions: HashSet<String>,
    /// Entry points to use for search
    pub search_entry_points: Vec<EmbedderType>,
    /// Whether to include file history
    pub include_history: bool,
}

impl Default for PreFileReadConfig {
    fn default() -> Self {
        let mut extensions = HashSet::new();
        extensions.extend([
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java",
            "md", "json", "yaml", "yml", "toml", "html", "css"
        ].iter().map(|s| s.to_string()));

        Self {
            max_preload_latency_ms: 50,
            context_limit: 5,
            min_context_similarity: 0.6,
            process_extensions: extensions,
            search_entry_points: vec![
                EmbedderType::Semantic,
                EmbedderType::Code,
                EmbedderType::Entity,
            ],
            include_history: true,
        }
    }
}

/// Payload for file read hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadPayload {
    /// Path to file being read
    pub file_path: String,
    /// Requested line range (if partial read)
    pub line_range: Option<(usize, usize)>,
    /// Tool initiating the read
    pub source_tool: String,
}

/// Context to inject alongside file content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectedContext {
    /// Related memories
    pub related_memories: Vec<RelatedMemory>,
    /// File history entries
    pub file_history: Vec<FileHistoryEntry>,
    /// Active goals related to this file
    pub related_goals: Vec<RelatedGoal>,
    /// Suggestions based on context
    pub suggestions: Vec<ContextSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedMemory {
    pub memory_id: Uuid,
    pub content_preview: String,
    pub similarity_score: f32,
    pub memory_type: MemoryType,
    pub entry_points_used: Vec<EmbedderType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHistoryEntry {
    pub edit_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub change_summary: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedGoal {
    pub goal_id: String,
    pub label: String,
    pub alignment_score: f32,
    pub level: GoalLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSuggestion {
    pub suggestion_type: SuggestionType,
    pub message: String,
    pub relevance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    ReviewRelated,
    ConsiderPattern,
    CheckAlignment,
    RecentChange,
}

impl PreFileReadHandler {
    pub fn new(
        store: Arc<TeleologicalStore>,
        search_engine: Arc<SearchEngine>,
        access_tracker: Arc<FileAccessTracker>,
        config: PreFileReadConfig,
    ) -> Self {
        Self {
            store,
            search_engine,
            access_tracker,
            config,
        }
    }

    /// Handle PreFileRead event
    pub async fn handle(&self, payload: FileReadPayload) -> Result<HookResponse, HookError> {
        let start = Instant::now();

        // 1. Check if we should process this file
        if !self.should_process(&payload.file_path) {
            return Ok(HookResponse::skip("File type not configured for preprocessing"));
        }

        // 2. Record access (async, don't wait)
        let tracker = self.access_tracker.clone();
        let file_path = payload.file_path.clone();
        tokio::spawn(async move {
            let _ = tracker.record_access(&file_path).await;
        });

        // 3. Search for related context with timeout
        let context = timeout(
            Duration::from_millis(self.config.max_preload_latency_ms),
            self.fetch_related_context(&payload),
        )
        .await
        .unwrap_or_else(|_| Ok(InjectedContext::empty()))
        .unwrap_or_else(|_| InjectedContext::empty());

        // 4. Build response
        let has_context = !context.related_memories.is_empty()
            || !context.file_history.is_empty()
            || !context.related_goals.is_empty();

        Ok(HookResponse {
            status: HookStatus::Proceed,
            message: if has_context {
                Some(format!(
                    "Found {} related memories, {} history entries",
                    context.related_memories.len(),
                    context.file_history.len()
                ))
            } else {
                None
            },
            warnings: vec![],
            injected_context: Some(serde_json::to_value(&context)?),
            metrics: HookMetrics {
                latency_ms: start.elapsed().as_millis() as u64,
                embedders_used: self.config.search_entry_points.len(),
                goals_checked: context.related_goals.len(),
            },
        })
    }

    fn should_process(&self, file_path: &str) -> bool {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        self.config.process_extensions.contains(extension)
    }

    async fn fetch_related_context(&self, payload: &FileReadPayload) -> Result<InjectedContext, HookError> {
        // 1. Search for related memories using file path as query
        let related_memories = self.search_related_memories(&payload.file_path).await?;

        // 2. Fetch file history if enabled
        let file_history = if self.config.include_history {
            self.fetch_file_history(&payload.file_path).await?
        } else {
            vec![]
        };

        // 3. Find related goals
        let related_goals = self.find_related_goals(&payload.file_path).await?;

        // 4. Generate suggestions
        let suggestions = self.generate_suggestions(
            &related_memories,
            &file_history,
            &related_goals,
        );

        Ok(InjectedContext {
            related_memories,
            file_history,
            related_goals,
            suggestions,
        })
    }

    async fn search_related_memories(&self, file_path: &str) -> Result<Vec<RelatedMemory>, HookError> {
        // Build search query from file path
        let query = SearchQuery {
            text: file_path.to_string(),
            strategy: SearchStrategy::EmbedderGroup {
                embedders: self.config.search_entry_points.clone(),
            },
            limit: self.config.context_limit,
            threshold: self.config.min_context_similarity,
            namespace: None, // Search all namespaces
            filters: Some(SearchFilters {
                memory_types: Some(vec![
                    MemoryType::CodeContext,
                    MemoryType::CodeEdit,
                    MemoryType::Documentation,
                ]),
                ..Default::default()
            }),
        };

        let results = self.search_engine.search(query).await?;

        Ok(results.memories
            .into_iter()
            .map(|m| RelatedMemory {
                memory_id: m.memory_id,
                content_preview: truncate_content(&m.content, 200),
                similarity_score: m.overall_similarity,
                memory_type: m.memory_type,
                entry_points_used: m.entry_point_hits
                    .keys()
                    .cloned()
                    .collect(),
            })
            .collect())
    }

    async fn fetch_file_history(&self, file_path: &str) -> Result<Vec<FileHistoryEntry>, HookError> {
        // Query for recent edits to this file
        let edits = self.store.query_by_metadata(
            "file_path",
            file_path,
            QueryOptions {
                limit: 10,
                order: QueryOrder::TimestampDesc,
                memory_types: Some(vec![MemoryType::CodeEdit]),
            },
        ).await?;

        Ok(edits
            .into_iter()
            .filter_map(|e| {
                let metadata: serde_json::Value = e.metadata?;
                Some(FileHistoryEntry {
                    edit_id: e.memory_id,
                    timestamp: e.timestamp,
                    change_summary: metadata.get("diff")
                        .and_then(|d| d.as_str())
                        .map(|d| summarize_diff(d))
                        .unwrap_or_else(|| "Unknown change".to_string()),
                    session_id: metadata.get("session_id")
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                })
            })
            .collect())
    }

    async fn find_related_goals(&self, file_path: &str) -> Result<Vec<RelatedGoal>, HookError> {
        let goals = self.store.search_goals_by_file(file_path, 3).await?;

        Ok(goals
            .into_iter()
            .map(|g| RelatedGoal {
                goal_id: g.goal_id,
                label: g.label,
                alignment_score: 0.0, // Will be calculated when file is read
                level: g.level,
            })
            .collect())
    }

    fn generate_suggestions(
        &self,
        memories: &[RelatedMemory],
        history: &[FileHistoryEntry],
        goals: &[RelatedGoal],
    ) -> Vec<ContextSuggestion> {
        let mut suggestions = Vec::new();

        // Suggest reviewing related memories
        if !memories.is_empty() {
            let top_memory = &memories[0];
            if top_memory.similarity_score > 0.8 {
                suggestions.push(ContextSuggestion {
                    suggestion_type: SuggestionType::ReviewRelated,
                    message: format!(
                        "Highly related content found ({}% similar). Consider reviewing.",
                        (top_memory.similarity_score * 100.0) as i32
                    ),
                    relevance: top_memory.similarity_score,
                });
            }
        }

        // Suggest checking recent changes
        if let Some(recent) = history.first() {
            let age = Utc::now() - recent.timestamp;
            if age.num_hours() < 24 {
                suggestions.push(ContextSuggestion {
                    suggestion_type: SuggestionType::RecentChange,
                    message: format!(
                        "File was modified recently: {}",
                        recent.change_summary
                    ),
                    relevance: 0.7,
                });
            }
        }

        // Suggest checking goal alignment
        if !goals.is_empty() {
            suggestions.push(ContextSuggestion {
                suggestion_type: SuggestionType::CheckAlignment,
                message: format!(
                    "This file is related to goal: '{}'",
                    goals[0].label
                ),
                relevance: 0.6,
            });
        }

        suggestions
    }
}

/// File access tracker for pattern learning
pub struct FileAccessTracker {
    /// Store for persisting access patterns
    store: Arc<TeleologicalStore>,
    /// In-memory access cache
    access_cache: Arc<RwLock<LruCache<String, Vec<AccessRecord>>>>,
    /// Access pattern analyzer
    pattern_analyzer: AccessPatternAnalyzer,
}

#[derive(Debug, Clone)]
pub struct AccessRecord {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub source_tool: String,
}

impl FileAccessTracker {
    /// Record file access
    pub async fn record_access(&self, file_path: &str) -> Result<(), HookError> {
        let record = AccessRecord {
            timestamp: Utc::now(),
            session_id: get_current_session_id(),
            source_tool: "Read".to_string(),
        };

        // Update cache
        {
            let mut cache = self.access_cache.write().await;
            let records = cache.entry(file_path.to_string())
                .or_insert_with(Vec::new);
            records.push(record.clone());

            // Keep only last 100 accesses per file
            if records.len() > 100 {
                records.remove(0);
            }
        }

        // Analyze patterns periodically
        if self.should_analyze_patterns().await {
            self.analyze_and_store_patterns().await?;
        }

        Ok(())
    }

    /// Get frequently accessed files
    pub async fn get_frequent_files(&self, limit: usize) -> Vec<(String, usize)> {
        let cache = self.access_cache.read().await;

        let mut counts: Vec<_> = cache.iter()
            .map(|(path, records)| (path.clone(), records.len()))
            .collect();

        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts.truncate(limit);

        counts
    }

    /// Get co-accessed files (files often accessed together)
    pub async fn get_co_accessed(&self, file_path: &str, limit: usize) -> Vec<(String, f32)> {
        self.pattern_analyzer.find_co_accessed(file_path, limit).await
    }

    async fn should_analyze_patterns(&self) -> bool {
        // Analyze every 100 accesses
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed) % 100 == 0
    }

    async fn analyze_and_store_patterns(&self) -> Result<(), HookError> {
        let patterns = self.pattern_analyzer.analyze_current_session().await?;

        for pattern in patterns {
            self.store.store_access_pattern(pattern).await?;
        }

        Ok(())
    }
}
```
    </section>

    <section name="PostFileReadHandler">
```rust
/// Post-file-read hook handler for content embedding
pub struct PostFileReadHandler {
    /// Teleological store for persisting embeddings
    store: Arc<TeleologicalStore>,
    /// Embedding pipeline
    embedder_pipeline: Arc<EmbedderPipeline>,
    /// Content hasher for deduplication
    content_hasher: ContentHasher,
    /// Configuration
    config: PostFileReadConfig,
}

#[derive(Debug, Clone)]
pub struct PostFileReadConfig {
    /// Maximum content size to embed (bytes)
    pub max_content_size: usize,
    /// Minimum content size to embed (bytes)
    pub min_content_size: usize,
    /// File extensions to embed
    pub embed_extensions: HashSet<String>,
    /// Whether to deduplicate by content hash
    pub deduplicate: bool,
    /// Hash TTL for deduplication (seconds)
    pub dedup_ttl_seconds: u64,
}

impl Default for PostFileReadConfig {
    fn default() -> Self {
        let mut extensions = HashSet::new();
        extensions.extend([
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java",
            "md", "json", "yaml", "yml", "toml"
        ].iter().map(|s| s.to_string()));

        Self {
            max_content_size: 100_000, // 100KB
            min_content_size: 50,
            embed_extensions: extensions,
            deduplicate: true,
            dedup_ttl_seconds: 3600, // 1 hour
        }
    }
}

/// Payload for post-file-read hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostFileReadPayload {
    /// File path that was read
    pub file_path: String,
    /// Content that was read
    pub content: String,
    /// Lines that were read (if partial)
    pub line_range: Option<(usize, usize)>,
    /// Source tool
    pub source_tool: String,
}

impl PostFileReadHandler {
    pub fn new(
        store: Arc<TeleologicalStore>,
        embedder_pipeline: Arc<EmbedderPipeline>,
        config: PostFileReadConfig,
    ) -> Self {
        Self {
            store,
            embedder_pipeline,
            content_hasher: ContentHasher::new(),
            config,
        }
    }

    /// Handle PostFileRead event
    pub async fn handle(&self, payload: PostFileReadPayload) -> Result<HookResponse, HookError> {
        let start = Instant::now();

        // 1. Check if we should embed this file
        if !self.should_embed(&payload) {
            return Ok(HookResponse::skip("File not eligible for embedding"));
        }

        // 2. Check for duplicate content
        let content_hash = self.content_hasher.hash(&payload.content);
        if self.config.deduplicate {
            if let Some(existing_id) = self.check_duplicate(&content_hash).await? {
                return Ok(HookResponse {
                    status: HookStatus::Success,
                    message: Some(format!("Content already embedded as {}", existing_id)),
                    warnings: vec![],
                    injected_context: Some(serde_json::json!({
                        "deduplicated": true,
                        "existing_memory_id": existing_id,
                    })),
                    metrics: HookMetrics::quick(start.elapsed().as_millis() as u64),
                });
            }
        }

        // 3. Generate teleological array
        let teleological_array = self.embedder_pipeline
            .generate_full(&payload.content)
            .await
            .map_err(HookError::EmbeddingFailed)?;

        // 4. Determine memory type and namespace
        let memory_type = self.classify_content(&payload.file_path, &payload.content);
        let namespace = self.derive_namespace(&payload.file_path);

        // 5. Store in teleological store
        let memory_id = self.store.inject(InjectionRequest {
            content: payload.content.clone(),
            teleological_array,
            memory_type,
            namespace,
            metadata: serde_json::json!({
                "file_path": payload.file_path,
                "content_hash": content_hash,
                "line_range": payload.line_range,
                "source_tool": payload.source_tool,
                "read_at": Utc::now().to_rfc3339(),
            }),
        }).await.map_err(HookError::StorageError)?;

        // 6. Record hash for deduplication
        if self.config.deduplicate {
            self.record_hash(&content_hash, memory_id).await?;
        }

        Ok(HookResponse {
            status: HookStatus::Success,
            message: Some(format!("File content embedded as memory {}", memory_id)),
            warnings: vec![],
            injected_context: Some(serde_json::json!({
                "memory_id": memory_id,
                "content_hash": content_hash,
                "embedders_generated": 13,
                "memory_type": memory_type,
            })),
            metrics: HookMetrics {
                latency_ms: start.elapsed().as_millis() as u64,
                embedders_used: 13,
                goals_checked: 0,
            },
        })
    }

    fn should_embed(&self, payload: &PostFileReadPayload) -> bool {
        // Check content size
        let size = payload.content.len();
        if size < self.config.min_content_size || size > self.config.max_content_size {
            return false;
        }

        // Check file extension
        let extension = std::path::Path::new(&payload.file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        self.config.embed_extensions.contains(extension)
    }

    fn classify_content(&self, file_path: &str, content: &str) -> MemoryType {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "md" | "txt" | "rst" => MemoryType::Documentation,
            "json" | "yaml" | "yml" | "toml" => MemoryType::Configuration,
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" => {
                // Check if it's a test file
                if file_path.contains("test") || file_path.contains("spec") {
                    MemoryType::TestCode
                } else {
                    MemoryType::CodeContext
                }
            }
            _ => MemoryType::General,
        }
    }

    fn derive_namespace(&self, file_path: &str) -> String {
        let path = std::path::Path::new(file_path);

        // Use top-level directory or "root"
        path.components()
            .skip(1) // Skip root
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("default")
            .to_string()
    }

    async fn check_duplicate(&self, content_hash: &str) -> Result<Option<Uuid>, HookError> {
        self.store.query_by_metadata(
            "content_hash",
            content_hash,
            QueryOptions {
                limit: 1,
                order: QueryOrder::TimestampDesc,
                ..Default::default()
            },
        )
        .await
        .map(|results| results.first().map(|m| m.memory_id))
        .map_err(HookError::StorageError)
    }

    async fn record_hash(&self, content_hash: &str, memory_id: Uuid) -> Result<(), HookError> {
        // Store in hash cache with TTL
        self.store.cache_hash(
            content_hash,
            memory_id,
            Duration::from_secs(self.config.dedup_ttl_seconds),
        ).await.map_err(HookError::StorageError)
    }
}

/// Content hasher for deduplication
pub struct ContentHasher {
    hasher_type: HasherType,
}

impl ContentHasher {
    pub fn new() -> Self {
        Self {
            hasher_type: HasherType::Blake3,
        }
    }

    pub fn hash(&self, content: &str) -> String {
        match self.hasher_type {
            HasherType::Blake3 => {
                let hash = blake3::hash(content.as_bytes());
                hash.to_hex().to_string()
            }
            HasherType::Xxh3 => {
                let hash = xxhash_rust::xxh3::xxh3_64(content.as_bytes());
                format!("{:016x}", hash)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HasherType {
    Blake3,
    Xxh3,
}
```
    </section>

    <section name="ShellScripts">
```bash
#!/bin/bash
# .claude/hooks/pre_file_read.sh
# PreFileRead hook - preloads related context before file reads

set -e

# Read input from stdin (JSON payload)
INPUT=$(cat)

# Extract file path
FILE_PATH=$(echo "$INPUT" | jq -r '.file_path')
SOURCE_TOOL=$(echo "$INPUT" | jq -r '.source_tool // "Read"')

# Call MCP tool for context preloading
RESPONSE=$(echo '{
  "jsonrpc": "2.0",
  "id": "pre-file-read-'$(date +%s)'",
  "method": "hooks/pre_file_read",
  "params": {
    "file_path": "'"$FILE_PATH"'",
    "source_tool": "'"$SOURCE_TOOL"'"
  }
}' | nc -U /tmp/contextgraph.sock 2>/dev/null || echo '{"result":{}}')

# Extract injected context
CONTEXT=$(echo "$RESPONSE" | jq -r '.result.injected_context // empty')

# Output context to stdout for Claude Code to consume
if [ -n "$CONTEXT" ] && [ "$CONTEXT" != "null" ]; then
  RELATED_COUNT=$(echo "$CONTEXT" | jq -r '.related_memories | length // 0')
  HISTORY_COUNT=$(echo "$CONTEXT" | jq -r '.file_history | length // 0')

  if [ "$RELATED_COUNT" -gt 0 ] || [ "$HISTORY_COUNT" -gt 0 ]; then
    echo "Context preloaded: $RELATED_COUNT related memories, $HISTORY_COUNT history entries" >&2

    # Output suggestions if any
    SUGGESTIONS=$(echo "$CONTEXT" | jq -r '.suggestions[]?.message // empty')
    if [ -n "$SUGGESTIONS" ]; then
      echo "Suggestions:" >&2
      echo "$SUGGESTIONS" | while read -r suggestion; do
        echo "  - $suggestion" >&2
      done
    fi
  fi
fi

# Always proceed
exit 0
```

```bash
#!/bin/bash
# .claude/hooks/post_file_read.sh
# PostFileRead hook - embeds file content as teleological array

set -e

# Read input from stdin (JSON payload)
INPUT=$(cat)

# Extract file path and content
FILE_PATH=$(echo "$INPUT" | jq -r '.file_path')
CONTENT=$(echo "$INPUT" | jq -r '.content')
SOURCE_TOOL=$(echo "$INPUT" | jq -r '.source_tool // "Read"')

# Check content size (skip very large files)
CONTENT_SIZE=${#CONTENT}
MAX_SIZE=100000

if [ "$CONTENT_SIZE" -gt "$MAX_SIZE" ]; then
  echo "Skipping embedding: content too large ($CONTENT_SIZE bytes)" >&2
  exit 0
fi

# Check minimum size
MIN_SIZE=50
if [ "$CONTENT_SIZE" -lt "$MIN_SIZE" ]; then
  echo "Skipping embedding: content too small ($CONTENT_SIZE bytes)" >&2
  exit 0
fi

# Call MCP tool to embed content
RESPONSE=$(echo '{
  "jsonrpc": "2.0",
  "id": "post-file-read-'$(date +%s)'",
  "method": "hooks/post_file_read",
  "params": {
    "file_path": "'"$FILE_PATH"'",
    "content": '"$(echo "$CONTENT" | jq -Rs .)"',
    "source_tool": "'"$SOURCE_TOOL"'"
  }
}' | nc -U /tmp/contextgraph.sock 2>/dev/null || echo '{"error":{"message":"Connection failed"}}')

# Check for errors
ERROR=$(echo "$RESPONSE" | jq -r '.error.message // empty')
if [ -n "$ERROR" ]; then
  echo "Warning: PostFileRead hook failed: $ERROR" >&2
  # Don't block, just log
  exit 0
fi

# Log success
MEMORY_ID=$(echo "$RESPONSE" | jq -r '.result.injected_context.memory_id // empty')
DEDUPLICATED=$(echo "$RESPONSE" | jq -r '.result.injected_context.deduplicated // false')

if [ "$DEDUPLICATED" = "true" ]; then
  echo "Content already indexed (deduplicated)" >&2
elif [ -n "$MEMORY_ID" ]; then
  echo "File content embedded as memory: $MEMORY_ID" >&2
fi

exit 0
```
    </section>

    <section name="Utilities">
```rust
// Utility functions

fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...", &content[..max_len - 3])
    }
}

fn summarize_diff(diff: &str) -> String {
    let lines: Vec<_> = diff.lines().collect();
    let additions = lines.iter().filter(|l| l.starts_with('+')).count();
    let deletions = lines.iter().filter(|l| l.starts_with('-')).count();

    format!("+{} -{} lines", additions, deletions)
}

fn get_current_session_id() -> String {
    SESSION_CONTEXT.with(|ctx| ctx.borrow().session_id.clone())
        .unwrap_or_else(|| format!("session-{}", Utc::now().format("%Y%m%d-%H%M%S")))
}

impl InjectedContext {
    pub fn empty() -> Self {
        Self {
            related_memories: vec![],
            file_history: vec![],
            related_goals: vec![],
            suggestions: vec![],
        }
    }
}

impl HookResponse {
    pub fn skip(reason: &str) -> Self {
        Self {
            status: HookStatus::Proceed,
            message: Some(reason.to_string()),
            warnings: vec![],
            injected_context: None,
            metrics: HookMetrics::default(),
        }
    }
}

impl HookMetrics {
    pub fn quick(latency_ms: u64) -> Self {
        Self {
            latency_ms,
            embedders_used: 0,
            goals_checked: 0,
        }
    }
}
```
    </section>
  </pseudo_code>
</task_spec>
```

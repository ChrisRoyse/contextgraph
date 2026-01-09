# Storage Layer Refactor Specification

## 1. Overview

The storage layer is responsible for persisting and retrieving `TeleologicalArray` instances as **atomic units**. This refactor ensures that:

1. Every stored item is a complete 13-embedder array (atomic storage)
2. Each of the 13 embedders has its own dedicated HNSW index (apples-to-apples search)
3. Storage operations maintain type safety and atomicity
4. Batch operations are optimized for autonomous agent throughput
5. No cross-embedder comparisons in index operations (never mix embedding types)
6. **Hook-triggered storage** for automatic memory capture on tool use
7. **Storage skills** for batch operations and index management
8. **Storage subagents** for background embedding and optimization

### 1.1 Design Principles

**Atomic Unit Storage**: A `TeleologicalArray` is always stored and retrieved as a single atomic unit containing all 13 embedder outputs. Partial arrays are invalid.

**Apples-to-Apples Indexing**: Each of the 13 embedders has its own dedicated index. When searching by E1 (Semantic), we only compare E1 vectors to E1 vectors. Never mix embedding types.

**Separation of Storage and Index**: Raw array data is stored separately from indices. This enables:
- Atomic writes of complete arrays
- Parallel index updates
- Index rebuilds without data loss
- Per-embedder index optimization

**Hook-Driven Architecture**: Storage operations integrate with Claude Code hooks for automatic memory capture, session consolidation, and proactive optimization.

### 1.2 Target Architecture Pipeline

The storage layer implements the following pipeline for autonomous AI agent memory:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    TELEOLOGICAL STORAGE PIPELINE                             │
│         Memory Injection → Embedding → Storage → Discovery → Emergence       │
└──────────────────────────────────────────────────────────────────────────────┘

    ┌─────────────────────────────────────────────────────────────────────────┐
    │ STAGE 1: MEMORY INJECTION (Claude Code Hooks)                          │
    │                                                                         │
    │   PostToolUse ──┬── Edit/Write events ──▶ File change memories          │
    │                 ├── Bash events ─────────▶ Command/output memories      │
    │                 └── Read events ─────────▶ File context memories        │
    │                                                                         │
    │   SessionEnd ────── Conversation end ────▶ Session consolidation        │
    │   PreCompact ────── RocksDB compact ─────▶ Purpose-aligned prioritize   │
    └─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
    ┌─────────────────────────────────────────────────────────────────────────┐
    │ STAGE 2: AUTONOMOUS EMBEDDING (13 Models in Parallel)                  │
    │                                                                         │
    │   Pending Queue ──▶ EmbeddingProcessorAgent ──▶ TeleologicalArray      │
    │                                                                         │
    │   ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐  │
    │   │ E1  │ E2  │ E3  │ E4  │ E5  │ E6  │ E7  │ E8  │ E9  │ E10 │ E11 │  │
    │   │Sem. │Temp │Temp │Temp │Caus │SPLA │Code │Graph│ HDC │Multi│Entity  │
    │   │     │Rec. │Per. │Pos. │     │DE   │     │     │     │modal│     │  │
    │   └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘  │
    │                         │ E12: LateInteraction │ E13: SpladeKeyword │  │
    │                         └─────────────────────┴───────────────────┘    │
    └─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
    ┌─────────────────────────────────────────────────────────────────────────┐
    │ STAGE 3: TELEOLOGICAL ARRAY STORAGE                                    │
    │                                                                         │
    │   ┌─────────────────────┐     ┌──────────────────────────────────────┐ │
    │   │  RocksDB Primary    │     │  13 Per-Embedder HNSW Indices        │ │
    │   │  (Atomic Arrays)    │◀───▶│  (Apples-to-Apples Comparison)       │ │
    │   │                     │     │                                      │ │
    │   │  cf_arrays          │     │  e01_semantic/   ─▶ HNSW 1024D       │ │
    │   │  cf_metadata        │     │  e02_temporal/   ─▶ HNSW 512D        │ │
    │   │  cf_sessions        │     │  e06_splade/     ─▶ Inverted Index   │ │
    │   │  cf_tiers           │     │  e12_late/       ─▶ Token Index      │ │
    │   │  cf_id_map          │     │  ...             ─▶ (13 total)       │ │
    │   └─────────────────────┘     └──────────────────────────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
    ┌─────────────────────────────────────────────────────────────────────────┐
    │ STAGE 4: ENTRY-POINT DISCOVERY (Any of 13 Spaces)                      │
    │                                                                         │
    │   Query ──▶ Route to embedder(s) ──▶ Per-embedder HNSW search          │
    │                                                                         │
    │   Search Modes:                                                         │
    │   • Single Embedder: Query E1 space only (fastest)                      │
    │   • Multi-Embedder:  Query E1+E6+E12, RRF aggregate                    │
    │   • Full Array:      Query all 13, weighted fusion                     │
    │                                                                         │
    │   Entry Points: Find closest in ANY space, return FULL array           │
    └─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
    ┌─────────────────────────────────────────────────────────────────────────┐
    │ STAGE 5: AUTONOMOUS GOAL EMERGENCE (Clustering)                        │
    │                                                                         │
    │   Retrieved Arrays ──▶ Purpose Evaluator ──▶ Emergent Goals            │
    │                                                                         │
    │   • Cluster by purpose_vector similarity                                │
    │   • Identify dominant themes across 13D purpose space                   │
    │   • No manual North Star - goals emerge from data patterns              │
    │   • Tier migration based on purpose alignment                           │
    └─────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Research-Backed Decisions

Based on 2025 best practices for multi-vector storage:

| Decision | Rationale | Source |
|----------|-----------|--------|
| 13 separate HNSW indices | Multi-field vector indexing with no overhead from searching multiple graphs | [Vespa Multi-Vector HNSW](https://blog.vespa.ai/semantic-search-with-multi-vector-indexing/) |
| RocksDB as primary store | Robust KV store for graph/vector metadata with efficient compaction | [CoreNN Architecture](https://blog.wilsonl.in/corenn/) |
| Mutable HNSW graphs | One graph per embedder per content node | [Weaviate Vector Indexing](https://docs.weaviate.io/weaviate/concepts/vector-index) |
| WriteBatch for atomicity | Atomic multi-statement operations | [Amazon Keyspaces Logged Batches](https://aws.amazon.com/blogs/database/amazon-keyspaces-now-supports-logged-batches-for-atomic-multi-statement-operations/) |
| Tiered storage (hot/warm/cold) | Memory-SSD-object architecture by access frequency | [Vector Search Cloud Architectures](https://arxiv.org/html/2601.01937) |

---

## 2. Hook-Triggered Storage

Claude Code hooks provide automatic storage triggers that capture memories without explicit user intervention. This section details how each hook integrates with the storage layer.

### 2.1 Hook Integration Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                    CLAUDE CODE HOOKS → STORAGE INTEGRATION                 │
└────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────┐     ┌─────────────────┐     ┌──────────────────────────┐
  │ Claude Code  │────▶│  PostToolUse    │────▶│ Immediate Memory Queue   │
  │ Tool Execute │     │  Hook Handler   │     │ (Pending Embedding)      │
  └──────────────┘     └─────────────────┘     └──────────────────────────┘
                                                          │
  ┌──────────────┐     ┌─────────────────┐                │
  │ Conversation │────▶│  SessionStart   │────────────────┤
  │ Begin        │     │  Hook Handler   │                │
  └──────────────┘     └─────────────────┘                │
                                                          ▼
  ┌──────────────┐     ┌─────────────────┐     ┌──────────────────────────┐
  │ Conversation │────▶│  SessionEnd     │────▶│ Consolidation Pipeline   │
  │ End          │     │  Hook Handler   │     │ (Flush + Merge + Tier)   │
  └──────────────┘     └─────────────────┘     └──────────────────────────┘
                                                          │
  ┌──────────────┐     ┌─────────────────┐                │
  │ RocksDB      │────▶│  PreCompact     │────────────────┤
  │ Compaction   │     │  Hook Handler   │                │
  └──────────────┘     └─────────────────┘                │
                                                          ▼
                                               ┌──────────────────────────┐
                                               │ TeleologicalArrayStore   │
                                               │ (13-Index Storage)       │
                                               └──────────────────────────┘
```

### 2.2 PostToolUse Hook for Memory Storage

The `PostToolUse` hook fires after any tool completes, enabling automatic memory capture.

#### Configuration in `.claude/settings.json`

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": {
          "tool_name": "Edit|Write|MultiEdit"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type edit --file \"$FILE_PATH\" --session \"$SESSION_ID\"",
            "timeout": 5000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "Bash"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type command --cmd \"$COMMAND\" --exit-code $EXIT_CODE",
            "timeout": 3000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "Read"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph index-file --path \"$FILE_PATH\" --context \"$CONVERSATION_CONTEXT\"",
            "timeout": 10000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "WebFetch|WebSearch"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type web --url \"$URL\" --summary \"$RESPONSE_SUMMARY\"",
            "timeout": 5000,
            "run_in_background": true
          }
        ]
      }
    ]
  }
}
```

#### Implementation

```rust
/// PostToolUse hook handler for automatic memory storage.
///
/// Triggers on: Edit, Write, Bash, Read, WebFetch, WebSearch (configurable)
/// Captures: tool context, file content, conversation state
/// Action: Queue for 13-model embedding pipeline
pub struct PostToolUseStorageHook {
    /// Pending memories awaiting batch embedding
    pending_queue: Arc<RwLock<Vec<PendingMemory>>>,

    /// Storage batch configuration
    batch_config: BatchConfig,

    /// Embedding service client
    embedder: Arc<dyn EmbeddingService>,

    /// Primary store
    store: Arc<dyn TeleologicalArrayStore>,

    /// Hook dispatcher for chaining
    hook_dispatcher: Arc<HookDispatcher>,
}

/// Pending memory awaiting embedding.
#[derive(Clone, Debug)]
pub struct PendingMemory {
    /// Unique identifier
    pub id: Uuid,

    /// Raw content to embed
    pub content: String,

    /// Memory source metadata
    pub source: MemorySource,

    /// Conversation context
    pub context: MemoryContext,

    /// Processing priority
    pub priority: Priority,

    /// Session ID for grouping
    pub session_id: String,
}

/// Source of a memory capture.
#[derive(Clone, Debug)]
pub enum MemorySource {
    /// Captured from a hook
    Hook {
        hook_type: HookType,
        tool_name: String,
        triggered_at: DateTime<Utc>,
    },
    /// Manually injected
    Manual {
        injected_by: String,
        injected_at: DateTime<Utc>,
    },
    /// Imported from external source
    Import {
        source_path: PathBuf,
        imported_at: DateTime<Utc>,
    },
}

impl PostToolUseStorageHook {
    /// Handle tool completion event.
    ///
    /// 1. Extract relevant content from tool output
    /// 2. Create pending memory with context
    /// 3. Queue for batch embedding (non-blocking)
    /// 4. Trigger batch flush if threshold reached
    pub async fn on_tool_complete(&self, event: ToolCompleteEvent) -> Result<(), HookError> {
        // Skip non-content-producing tools
        if !self.should_capture(&event.tool_name) {
            return Ok(());
        }

        // Extract memory-worthy content
        let content = self.extract_content(&event)?;
        if content.is_empty() {
            return Ok(());
        }

        // Create pending memory with hook context
        let pending = PendingMemory {
            id: Uuid::new_v4(),
            content,
            source: MemorySource::Hook {
                hook_type: HookType::PostToolUse,
                tool_name: event.tool_name.clone(),
                triggered_at: Utc::now(),
            },
            context: self.build_context(&event),
            priority: self.calculate_priority(&event),
            session_id: event.session_id.clone(),
        };

        // Add to queue (non-blocking)
        {
            let mut queue = self.pending_queue.write().await;
            queue.push(pending);
        }

        // Check batch threshold
        if self.should_flush_batch().await {
            self.trigger_batch_embedding().await?;
        }

        Ok(())
    }

    /// Determine if tool output should be captured.
    fn should_capture(&self, tool_name: &str) -> bool {
        matches!(tool_name,
            "Edit" | "Write" | "Bash" | "Read" | "MultiEdit" |
            "NotebookEdit" | "WebFetch" | "WebSearch" | "Grep" | "Glob"
        )
    }

    /// Extract memory content from tool event.
    fn extract_content(&self, event: &ToolCompleteEvent) -> Result<String, HookError> {
        match event.tool_name.as_str() {
            "Edit" | "Write" | "MultiEdit" => {
                // Extract file content and change summary
                let file_path = event.params.get("file_path")
                    .ok_or(HookError::MissingParam("file_path"))?;
                let new_content = event.params.get("new_string")
                    .or_else(|| event.params.get("content"))
                    .unwrap_or(&String::new());

                Ok(format!(
                    "File: {}\nChange: {}\nContext: {}",
                    file_path,
                    summarize_change(new_content),
                    event.conversation_context
                ))
            }
            "Bash" => {
                // Extract command and output
                let command = event.params.get("command").unwrap_or(&String::new());
                let output = event.output.as_ref().map(|o| &o.stdout).unwrap_or(&String::new());

                Ok(format!(
                    "Command: {}\nOutput: {}\nContext: {}",
                    command,
                    truncate(output, 1000),
                    event.conversation_context
                ))
            }
            "Read" => {
                // Extract file content summary
                let file_path = event.params.get("file_path").unwrap_or(&String::new());
                let content = event.output.as_ref()
                    .map(|o| summarize_file(&o.content))
                    .unwrap_or_default();

                Ok(format!(
                    "Read: {}\nSummary: {}",
                    file_path,
                    content
                ))
            }
            "WebFetch" | "WebSearch" => {
                let url = event.params.get("url").unwrap_or(&String::new());
                let response = event.output.as_ref()
                    .map(|o| truncate(&o.content, 2000))
                    .unwrap_or_default();

                Ok(format!(
                    "URL: {}\nContent: {}",
                    url,
                    response
                ))
            }
            _ => Ok(String::new()),
        }
    }

    /// Build context from tool event for later retrieval.
    fn build_context(&self, event: &ToolCompleteEvent) -> MemoryContext {
        MemoryContext {
            conversation_id: event.conversation_id.clone(),
            session_id: event.session_id.clone(),
            user_query: event.user_query.clone(),
            tool_chain: event.tool_chain.clone(),
            timestamp: Utc::now(),
        }
    }

    /// Calculate priority based on tool importance.
    fn calculate_priority(&self, event: &ToolCompleteEvent) -> Priority {
        match event.tool_name.as_str() {
            "Edit" | "Write" | "MultiEdit" => Priority::High,
            "Bash" if event.exit_code == Some(0) => Priority::Normal,
            "Bash" => Priority::Low, // Failed commands
            "Read" => Priority::Low,
            _ => Priority::Normal,
        }
    }

    /// Check if batch should be flushed.
    async fn should_flush_batch(&self) -> bool {
        let queue = self.pending_queue.read().await;
        queue.len() >= self.batch_config.flush_threshold
            || self.time_since_last_flush() > self.batch_config.max_delay
    }

    /// Trigger background batch embedding.
    async fn trigger_batch_embedding(&self) -> Result<(), HookError> {
        // Signal the EmbeddingProcessorAgent
        self.hook_dispatcher.dispatch(HookEvent::BatchEmbeddingRequested {
            queue_size: self.pending_queue.read().await.len(),
        }).await;
        Ok(())
    }
}
```

### 2.3 SessionStart Hook for Context Initialization

```rust
/// SessionStart hook handler for memory context initialization.
///
/// Fires when a new Claude Code session begins.
/// Initializes session context and preloads relevant memories.
pub struct SessionStartHook {
    store: Arc<dyn TeleologicalArrayStore>,
    session_manager: Arc<SessionManager>,
}

impl SessionStartHook {
    /// Handle session start event.
    pub async fn on_session_start(&self, event: SessionStartEvent) -> Result<(), HookError> {
        // Create session record
        let session = Session {
            id: event.session_id.clone(),
            started_at: Utc::now(),
            working_directory: event.working_directory.clone(),
            initial_context: event.initial_context.clone(),
        };

        self.session_manager.create_session(session).await?;

        // Preload relevant memories for this context
        if let Some(context) = &event.initial_context {
            self.preload_relevant_memories(&event.session_id, context).await?;
        }

        Ok(())
    }

    /// Preload memories relevant to the session context.
    async fn preload_relevant_memories(
        &self,
        session_id: &str,
        context: &str,
    ) -> Result<(), HookError> {
        // This will be handled by the search layer
        // Just emit event for now
        Ok(())
    }
}
```

### 2.4 SessionEnd Hook for Consolidation

The `SessionEnd` hook triggers memory consolidation when a Claude Code session ends.

#### Configuration

```json
{
  "hooks": {
    "SessionEnd": [
      {
        "matcher": {},
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph consolidate-session --session-id \"$SESSION_ID\" --export-metrics",
            "timeout": 60000
          }
        ]
      }
    ]
  }
}
```

#### Implementation

```rust
/// SessionEnd hook handler for memory consolidation.
///
/// Performs end-of-session operations:
/// 1. Flush pending embeddings
/// 2. Consolidate related memories (merge similar)
/// 3. Update temporal indices
/// 4. Optimize hot memories (tier promotion)
/// 5. Export session metrics
pub struct SessionEndConsolidationHook {
    store: Arc<dyn TeleologicalArrayStore>,
    index_manager: Arc<IndexManager>,
    consolidator: Arc<MemoryConsolidator>,
    metrics_exporter: Arc<MetricsExporter>,
}

/// Session consolidation statistics.
#[derive(Clone, Debug, Default)]
pub struct ConsolidationStats {
    /// Memories flushed from pending queue
    pub flushed_count: usize,

    /// Memories merged (deduplicated)
    pub merged_count: usize,

    /// New links created between memories
    pub linked_count: usize,

    /// Memories promoted to hot tier
    pub promoted_count: usize,

    /// Memories demoted to cold tier
    pub demoted_count: usize,

    /// Session duration in seconds
    pub session_duration_secs: u64,

    /// Total memories in session
    pub total_memories: usize,
}

impl SessionEndConsolidationHook {
    /// Handle session end event.
    pub async fn on_session_end(&self, event: SessionEndEvent) -> Result<ConsolidationStats, HookError> {
        let mut stats = ConsolidationStats::default();

        // 1. Flush any pending embeddings
        let pending = self.flush_pending_embeddings(&event.session_id).await?;
        stats.flushed_count = pending;

        // 2. Get all session memories
        let session_memories = self.store.list_by_session(&event.session_id).await?;
        stats.total_memories = session_memories.len();

        // 3. Consolidate related memories (merge similar, link related)
        let consolidated = self.consolidator.consolidate(&session_memories).await?;
        stats.merged_count = consolidated.merged_count;
        stats.linked_count = consolidated.link_count;

        // 4. Update temporal indices (E2, E3, E4)
        self.update_temporal_indices(&session_memories).await?;

        // 5. Promote hot memories to faster tier
        let tier_changes = self.optimize_memory_tiers(&session_memories).await?;
        stats.promoted_count = tier_changes.promoted;
        stats.demoted_count = tier_changes.demoted;

        // 6. Calculate session duration
        stats.session_duration_secs = (Utc::now() - event.started_at).num_seconds() as u64;

        // 7. Store consolidation metadata
        self.store_session_summary(&event.session_id, &stats).await?;

        // 8. Export metrics if requested
        if event.export_metrics {
            self.metrics_exporter.export_session(&event.session_id, &stats).await?;
        }

        Ok(stats)
    }

    /// Flush pending embeddings for a session.
    async fn flush_pending_embeddings(&self, session_id: &str) -> Result<usize, HookError> {
        // Signal the embedding processor to flush
        // Returns count of flushed items
        todo!("Implement pending queue flush")
    }

    /// Consolidate related memories using clustering.
    async fn consolidate_related(&self, memories: &[TeleologicalArray]) -> Result<ConsolidationResult, HookError> {
        // Use E1 (semantic) for similarity clustering
        let clusters = self.cluster_by_embedder(memories, Embedder::Semantic, 0.85).await?;

        let mut result = ConsolidationResult::default();

        for cluster in clusters {
            if cluster.len() > 1 {
                // Merge highly similar memories
                let merged = self.merge_cluster(&cluster)?;
                self.store.store(merged).await?;
                result.merged_count += cluster.len() - 1;

                // Delete originals (soft delete)
                for memory in &cluster[1..] {
                    self.store.delete(memory.id).await?;
                }
            }
        }

        Ok(result)
    }

    /// Update temporal indices for session memories.
    async fn update_temporal_indices(&self, memories: &[TeleologicalArray]) -> Result<(), HookError> {
        // Recompute E2 (recency) based on access patterns
        // Update E3 (periodic) based on session timing
        // Refresh E4 (positional) based on conversation flow
        for memory in memories {
            self.index_manager.update_temporal_embeddings(memory.id).await?;
        }
        Ok(())
    }

    /// Optimize memory tiers based on purpose alignment.
    async fn optimize_memory_tiers(&self, memories: &[TeleologicalArray]) -> Result<TierChanges, HookError> {
        let mut changes = TierChanges::default();

        for memory in memories {
            let purpose_score = memory.theta_to_purpose;
            let access_score = self.calculate_access_score(memory);

            let target_tier = self.determine_target_tier(purpose_score, access_score);
            let current_tier = memory.metadata.as_ref()
                .map(|m| m.tier)
                .unwrap_or(MemoryTier::Warm);

            if target_tier != current_tier {
                self.store.migrate_tier(memory.id, target_tier).await?;
                match target_tier.cmp(&current_tier) {
                    std::cmp::Ordering::Less => changes.promoted += 1,
                    std::cmp::Ordering::Greater => changes.demoted += 1,
                    std::cmp::Ordering::Equal => {}
                }
            }
        }

        Ok(changes)
    }
}
```

### 2.5 PreCompact Hook for Memory Prioritization

The `PreCompact` hook fires before RocksDB compaction, allowing intelligent memory prioritization.

```rust
/// PreCompact hook for teleological memory prioritization.
///
/// Uses purpose alignment to determine which memories to:
/// - Keep in hot tier (high purpose alignment)
/// - Move to warm tier (moderate alignment)
/// - Move to cold tier (low alignment)
/// - Archive (very low alignment + old)
pub struct PreCompactPrioritizationHook {
    store: Arc<dyn TeleologicalArrayStore>,
    purpose_evaluator: Arc<PurposeEvaluator>,
    tier_config: TierConfig,
}

/// Tier configuration thresholds.
#[derive(Clone, Debug)]
pub struct TierConfig {
    /// Minimum purpose score for hot tier
    pub hot_threshold: f32,
    /// Minimum purpose score for warm tier
    pub warm_threshold: f32,
    /// Minimum purpose score for cold tier
    pub cold_threshold: f32,
    /// Recency weight (vs purpose weight)
    pub recency_weight: f32,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            hot_threshold: 0.8,
            warm_threshold: 0.5,
            cold_threshold: 0.2,
            recency_weight: 0.3, // 30% recency, 70% purpose
        }
    }
}

impl PreCompactPrioritizationHook {
    /// Handle pre-compaction event.
    pub async fn on_pre_compact(&self, event: PreCompactEvent) -> Result<(), HookError> {
        // Get memories in the compaction range
        let memories = self.store.list_range(
            event.start_key.as_ref(),
            event.end_key.as_ref(),
        ).await?;

        // Score each memory by purpose alignment
        let scored: Vec<(TeleologicalArray, f32)> = memories
            .into_iter()
            .map(|m| {
                let score = self.purpose_evaluator.alignment_score(&m);
                (m, score)
            })
            .collect();

        // Partition by tier thresholds
        for (memory, score) in scored {
            let target_tier = self.determine_tier(score, &memory);

            if target_tier != memory.current_tier() {
                self.store.migrate_tier(memory.id, target_tier).await?;
            }
        }

        Ok(())
    }

    /// Determine target tier based on purpose score and recency.
    fn determine_tier(&self, purpose_score: f32, memory: &TeleologicalArray) -> MemoryTier {
        let recency_factor = self.calculate_recency_factor(memory.created_at);
        let combined_score = purpose_score * (1.0 - self.tier_config.recency_weight)
            + recency_factor * self.tier_config.recency_weight;

        match combined_score {
            s if s >= self.tier_config.hot_threshold => MemoryTier::Hot,
            s if s >= self.tier_config.warm_threshold => MemoryTier::Warm,
            s if s >= self.tier_config.cold_threshold => MemoryTier::Cold,
            _ => MemoryTier::Archive,
        }
    }

    /// Calculate recency factor (exponential decay).
    fn calculate_recency_factor(&self, created_at: DateTime<Utc>) -> f32 {
        let age_hours = (Utc::now() - created_at).num_hours() as f32;
        let half_life_hours = 24.0 * 7.0; // 1 week half-life
        (-age_hours / half_life_hours).exp()
    }
}
```

### 2.6 Complete Hook Configuration

Full hook configuration for storage integration in `.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": {
          "tool_name": "Edit|Write|MultiEdit"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type edit --file \"$FILE_PATH\" --session \"$SESSION_ID\" --context \"$CONVERSATION_CONTEXT\"",
            "timeout": 5000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "Bash"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type command --cmd \"$COMMAND\" --exit-code $EXIT_CODE --session \"$SESSION_ID\"",
            "timeout": 3000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "Read"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph index-file --path \"$FILE_PATH\" --session \"$SESSION_ID\"",
            "timeout": 10000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "WebFetch|WebSearch"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type web --url \"$URL\" --session \"$SESSION_ID\"",
            "timeout": 5000,
            "run_in_background": true
          }
        ]
      },
      {
        "matcher": {
          "tool_name": "Grep|Glob"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --type search --pattern \"$PATTERN\" --session \"$SESSION_ID\"",
            "timeout": 3000,
            "run_in_background": true
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "matcher": {},
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph session-start --id \"$SESSION_ID\" --cwd \"$WORKING_DIRECTORY\"",
            "timeout": 5000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": {},
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph consolidate-session --session-id \"$SESSION_ID\" --export-metrics --optimize-tiers",
            "timeout": 60000
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": {},
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph prioritize-memories --strategy purpose-aligned --start-key \"$START_KEY\" --end-key \"$END_KEY\"",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

---

## 3. Storage Skills

Storage skills provide high-level operations that Claude Code can invoke for batch storage operations.

### 3.1 Skill Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         STORAGE SKILLS ARCHITECTURE                      │
└──────────────────────────────────────────────────────────────────────────┘

  ┌───────────────────┐     ┌───────────────────┐     ┌──────────────────┐
  │  batch-embed      │     │  index-manage     │     │  storage-cleanup │
  │  Skill            │     │  Skill            │     │  Skill           │
  │                   │     │                   │     │                  │
  │  • Batch embed    │     │  • Rebuild index  │     │  • Purge old     │
  │  • Priority queue │     │  • Optimize index │     │  • Tier migrate  │
  │  • Parallel exec  │     │  • Index stats    │     │  • Deduplicate   │
  └───────────────────┘     └───────────────────┘     └──────────────────┘
           │                         │                        │
           └─────────────────────────┼────────────────────────┘
                                     │
                                     ▼
                        ┌────────────────────────┐
                        │  TeleologicalArrayStore │
                        │  (13-Index Storage)     │
                        └────────────────────────┘
```

### 3.2 Batch Embedding Skill

```yaml
# .claude/skills/batch-embed.yaml
name: batch-embed
description: Batch embed multiple contents into TeleologicalArrays using all 13 models
triggers:
  - "batch embed"
  - "embed multiple"
  - "bulk embedding"
  - "embed files"

parameters:
  contents:
    type: array
    description: Array of content strings to embed
  files:
    type: array
    description: Array of file paths to embed
  priority:
    type: string
    enum: [low, normal, high, critical]
    default: normal
  wait_for_completion:
    type: boolean
    default: false
    description: Wait for embedding to complete before returning
  parallel_embedders:
    type: integer
    default: 4
    description: Number of embedders to run in parallel

execution:
  command: npx contextgraph batch-embed
  args:
    - --priority=$priority
    - --wait=$wait_for_completion
    - --parallel=$parallel_embedders
  stdin: $contents
  timeout: 300000
```

```rust
/// Batch embedding skill implementation.
///
/// Processes multiple content items through all 13 embedders
/// and stores as complete TeleologicalArrays.
pub struct BatchEmbedSkill {
    embedder_service: Arc<EmbeddingService>,
    store: Arc<dyn TeleologicalArrayStore>,
    batch_config: BatchEmbedConfig,
}

/// Batch embed request.
#[derive(Clone, Debug)]
pub struct BatchEmbedRequest {
    /// Content strings to embed
    pub contents: Vec<String>,
    /// File paths to read and embed
    pub files: Vec<PathBuf>,
    /// Processing priority
    pub priority: Option<Priority>,
    /// Wait for completion
    pub wait_for_completion: bool,
    /// Parallel embedder count
    pub parallel_embedders: usize,
    /// Request start time
    pub start_time: Instant,
}

/// Batch embed result.
#[derive(Clone, Debug)]
pub struct BatchEmbedResult {
    /// IDs of stored arrays
    pub ids: Vec<Uuid>,
    /// Number successfully embedded
    pub embedded_count: usize,
    /// Number failed
    pub failed_count: usize,
    /// Processing duration in ms
    pub duration_ms: u64,
}

impl BatchEmbedSkill {
    /// Execute batch embedding.
    ///
    /// 1. Validate inputs
    /// 2. Queue for parallel embedding
    /// 3. Await all 13 embedder outputs per content
    /// 4. Store complete arrays atomically
    pub async fn execute(&self, request: BatchEmbedRequest) -> Result<BatchEmbedResult, SkillError> {
        // Combine contents and file contents
        let mut all_contents = request.contents.clone();
        for file_path in &request.files {
            let content = tokio::fs::read_to_string(file_path).await?;
            all_contents.push(content);
        }

        let priority = request.priority.unwrap_or(Priority::Normal);

        // Validate batch size
        if all_contents.len() > self.batch_config.max_batch_size {
            return Err(SkillError::BatchTooLarge {
                max: self.batch_config.max_batch_size,
                actual: all_contents.len(),
            });
        }

        // Create embedding tasks for all content
        let semaphore = Arc::new(Semaphore::new(request.parallel_embedders));
        let embedding_futures: Vec<_> = all_contents
            .iter()
            .map(|content| {
                let sem = semaphore.clone();
                let embedder = self.embedder_service.clone();
                let content = content.clone();
                async move {
                    let _permit = sem.acquire().await?;
                    embedder.embed_all(&content, priority).await
                }
            })
            .collect();

        // Execute all embeddings in parallel
        let results = futures::future::join_all(embedding_futures).await;

        // Collect successful embeddings
        let mut arrays = Vec::new();
        let mut failed_count = 0;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(embeddings) => {
                    let array = TeleologicalArray {
                        id: Uuid::new_v4(),
                        embeddings,
                        source_content: Some(all_contents[i].clone()),
                        created_at: Utc::now(),
                        ..Default::default()
                    };
                    arrays.push(array);
                }
                Err(e) => {
                    tracing::warn!("Failed to embed content {}: {}", i, e);
                    failed_count += 1;
                }
            }
        }

        // Store batch atomically
        let ids = self.store.store_batch(arrays).await?;

        Ok(BatchEmbedResult {
            embedded_count: ids.len(),
            failed_count,
            ids,
            duration_ms: request.start_time.elapsed().as_millis() as u64,
        })
    }
}
```

### 3.3 Index Management Skill

```yaml
# .claude/skills/index-manage.yaml
name: index-manage
description: Manage HNSW indices for teleological storage
triggers:
  - "rebuild index"
  - "optimize index"
  - "index stats"
  - "index health"

subcommands:
  rebuild:
    description: Rebuild specific or all embedder indices
    parameters:
      embedder:
        type: string
        enum: [semantic, temporal_recent, temporal_periodic, temporal_positional, causal, splade_primary, code, graph, hdc, multimodal, entity, late_interaction, splade_keyword, all]
        default: all
    command: npx contextgraph index rebuild --embedder=$embedder

  stats:
    description: Get index statistics for all 13 embedders
    parameters:
      format:
        type: string
        enum: [summary, detailed, json]
        default: summary
    command: npx contextgraph index stats --format=$format

  optimize:
    description: Optimize index parameters based on workload
    parameters:
      workload:
        type: string
        enum: [read_heavy, write_heavy, balanced]
        default: balanced
    command: npx contextgraph index optimize --workload=$workload

  health:
    description: Check index health and consistency
    parameters:
      fix:
        type: boolean
        default: false
    command: npx contextgraph index health --fix=$fix
```

```rust
/// Index management skill implementation.
pub struct IndexManageSkill {
    index_manager: Arc<IndexManager>,
    stats_collector: Arc<IndexStatsCollector>,
}

impl IndexManageSkill {
    /// Rebuild index for specific embedder.
    pub async fn rebuild(&self, embedder: Option<Embedder>) -> Result<Vec<IndexStats>, SkillError> {
        match embedder {
            Some(e) => {
                let stats = self.index_manager.rebuild_index(e).await?;
                Ok(vec![stats])
            }
            None => {
                // Rebuild all 13 indices in parallel
                let stats = self.index_manager.rebuild_all_indices().await?;
                Ok(stats)
            }
        }
    }

    /// Get index statistics.
    pub async fn stats(&self, format: StatsFormat) -> Result<String, SkillError> {
        let all_stats = self.index_manager.all_index_stats().await?;

        match format {
            StatsFormat::Summary => Ok(self.format_summary(&all_stats)),
            StatsFormat::Detailed => Ok(self.format_detailed(&all_stats)),
            StatsFormat::Json => Ok(serde_json::to_string_pretty(&all_stats)?),
        }
    }

    /// Optimize index parameters based on workload analysis.
    pub async fn optimize(&self, workload: WorkloadType) -> Result<OptimizationResult, SkillError> {
        let current_stats = self.stats_collector.collect().await?;

        // Calculate optimal parameters per embedder
        let mut optimizations = Vec::new();

        for embedder in Embedder::all() {
            let current = self.index_manager.index_stats(*embedder).await?;

            let new_params = match workload {
                WorkloadType::ReadHeavy => HnswConfig {
                    ef_search: (current.hnsw_stats.as_ref()
                        .map(|h| h.ef_search)
                        .unwrap_or(100) as f64 * 1.5) as usize,
                    ..current.config()
                },
                WorkloadType::WriteHeavy => HnswConfig {
                    ef_construction: (current.hnsw_stats.as_ref()
                        .map(|h| h.ef_construction)
                        .unwrap_or(200) as f64 * 0.75) as usize,
                    ..current.config()
                },
                WorkloadType::Balanced => current.config(),
            };

            if new_params != current.config() {
                self.index_manager.update_config(*embedder, new_params.clone()).await?;
                optimizations.push((*embedder, new_params));
            }
        }

        Ok(OptimizationResult {
            workload,
            optimized_embedders: optimizations.len(),
            changes: optimizations,
        })
    }

    /// Check index health and consistency.
    pub async fn health(&self, fix: bool) -> Result<HealthReport, SkillError> {
        let mut report = HealthReport::default();

        for embedder in Embedder::all() {
            let stats = self.index_manager.index_stats(*embedder).await?;

            // Check for issues
            if stats.vector_count == 0 {
                report.issues.push(HealthIssue {
                    embedder: *embedder,
                    issue_type: IssueType::EmptyIndex,
                    severity: Severity::Warning,
                });
            }

            if stats.p99_search_us > 10_000 {
                report.issues.push(HealthIssue {
                    embedder: *embedder,
                    issue_type: IssueType::HighLatency,
                    severity: Severity::Warning,
                });
            }

            // Auto-fix if requested
            if fix && !report.issues.is_empty() {
                self.auto_fix_issues(*embedder, &report.issues).await?;
            }
        }

        report.healthy = report.issues.iter().all(|i| i.severity != Severity::Critical);
        Ok(report)
    }
}
```

### 3.4 Storage Cleanup Skill

```yaml
# .claude/skills/storage-cleanup.yaml
name: storage-cleanup
description: Clean up old memories and optimize storage
triggers:
  - "cleanup storage"
  - "purge old memories"
  - "storage maintenance"
  - "deduplicate memories"

parameters:
  older_than_days:
    type: integer
    default: 90
    description: Delete memories older than this many days
  min_purpose_score:
    type: number
    default: 0.2
    description: Minimum purpose alignment score to keep
  deduplicate:
    type: boolean
    default: true
    description: Merge duplicate memories
  dry_run:
    type: boolean
    default: true
    description: Preview changes without deleting

execution:
  command: npx contextgraph cleanup
  args:
    - --older-than=$older_than_days
    - --min-score=$min_purpose_score
    - --deduplicate=$deduplicate
    - --dry-run=$dry_run
  timeout: 600000
```

```rust
/// Storage cleanup skill implementation.
pub struct CleanupSkill {
    store: Arc<dyn TeleologicalArrayStore>,
    purpose_evaluator: Arc<PurposeEvaluator>,
    deduplicator: Arc<MemoryDeduplicator>,
}

/// Cleanup request parameters.
#[derive(Clone, Debug)]
pub struct CleanupRequest {
    pub older_than_days: i64,
    pub min_purpose_score: f32,
    pub deduplicate: bool,
    pub dry_run: bool,
}

/// Cleanup result statistics.
#[derive(Clone, Debug, Default)]
pub struct CleanupResult {
    pub would_delete: usize,
    pub would_keep: usize,
    pub would_merge: usize,
    pub deleted: usize,
    pub merged: usize,
    pub bytes_freed: u64,
    pub dry_run: bool,
}

impl CleanupSkill {
    /// Execute cleanup based on age and purpose score.
    pub async fn execute(&self, request: CleanupRequest) -> Result<CleanupResult, SkillError> {
        let cutoff_date = Utc::now() - Duration::days(request.older_than_days);

        // Find candidates for deletion (old + low purpose)
        let candidates = self.store.list_before(cutoff_date).await?;

        let mut to_delete = Vec::new();
        let mut to_keep = Vec::new();

        for memory in candidates {
            let purpose_score = self.purpose_evaluator.alignment_score(&memory);

            if purpose_score < request.min_purpose_score {
                to_delete.push(memory);
            } else {
                to_keep.push((memory.id, purpose_score));
            }
        }

        // Find duplicates if requested
        let mut to_merge = Vec::new();
        if request.deduplicate {
            let all_memories = self.store.list_all().await?;
            to_merge = self.deduplicator.find_duplicates(&all_memories).await?;
        }

        if request.dry_run {
            return Ok(CleanupResult {
                would_delete: to_delete.len(),
                would_keep: to_keep.len(),
                would_merge: to_merge.len(),
                deleted: 0,
                merged: 0,
                bytes_freed: 0,
                dry_run: true,
            });
        }

        // Actually delete
        let mut bytes_freed = 0u64;
        for memory in &to_delete {
            bytes_freed += self.estimate_size(memory);
            self.store.delete(memory.id).await?;
        }

        // Merge duplicates
        for duplicate_group in &to_merge {
            self.deduplicator.merge_group(duplicate_group).await?;
        }

        Ok(CleanupResult {
            would_delete: 0,
            would_keep: to_keep.len(),
            would_merge: 0,
            deleted: to_delete.len(),
            merged: to_merge.len(),
            bytes_freed,
            dry_run: false,
        })
    }
}
```

---

## 4. Storage Subagents

Storage subagents handle heavy-lifting operations in the background, freeing the main Claude Code agent.

### 4.1 Subagent Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│                      STORAGE SUBAGENT ARCHITECTURE                        │
└──────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────────────────────────────────────────────────────────────┐
  │                        SUBAGENT COORDINATOR                             │
  │                                                                         │
  │   Lifecycle: Spawn on SessionStart, Shutdown on SessionEnd              │
  │   Coordination: Shared message queue, mutex-free where possible         │
  └─────────────────────────────────────────────────────────────────────────┘
           │                    │                    │
           ▼                    ▼                    ▼
  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
  │   Embedding     │  │     Index       │  │      Tier       │
  │   Processor     │  │   Optimizer     │  │    Migrator     │
  │                 │  │                 │  │                 │
  │ • 13-model      │  │ • HNSW tune     │  │ • Access track  │
  │   embedding     │  │ • Latency       │  │ • Purpose eval  │
  │ • Priority      │  │   monitor       │  │ • Hot/warm/cold │
  │   queue         │  │ • Auto-rebuild  │  │   migration     │
  │ • Batch flush   │  │                 │  │                 │
  └─────────────────┘  └─────────────────┘  └─────────────────┘
           │                    │                    │
           └────────────────────┼────────────────────┘
                                │
                                ▼
                   ┌────────────────────────┐
                   │  TeleologicalArrayStore │
                   └────────────────────────┘
```

### 4.2 Embedding Processor Agent

```rust
/// Background agent for processing embedding queues.
///
/// Runs as a separate process/thread, consuming from the pending
/// memory queue and producing complete TeleologicalArrays.
pub struct EmbeddingProcessorAgent {
    /// Agent identifier
    id: String,

    /// Pending memory queue (shared with hooks)
    pending_queue: Arc<RwLock<VecDeque<PendingMemory>>>,

    /// Embedding service with all 13 models
    embedder: Arc<EmbeddingService>,

    /// Target store for completed arrays
    store: Arc<dyn TeleologicalArrayStore>,

    /// Processing configuration
    config: EmbeddingAgentConfig,

    /// Shutdown signal
    shutdown: Arc<AtomicBool>,

    /// Processing metrics
    metrics: Arc<AgentMetrics>,
}

/// Embedding agent configuration.
#[derive(Clone, Debug)]
pub struct EmbeddingAgentConfig {
    /// Maximum batch size
    pub batch_size: usize,
    /// Maximum idle time before checking queue
    pub idle_timeout_ms: u64,
    /// Maximum retries on embedding failure
    pub max_retries: usize,
    /// Parallel embedder count (for each batch item)
    pub parallel_embedders: usize,
}

impl Default for EmbeddingAgentConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            idle_timeout_ms: 100,
            max_retries: 3,
            parallel_embedders: 4,
        }
    }
}

impl EmbeddingProcessorAgent {
    /// Create a new embedding processor agent.
    pub fn new(
        pending_queue: Arc<RwLock<VecDeque<PendingMemory>>>,
        embedder: Arc<EmbeddingService>,
        store: Arc<dyn TeleologicalArrayStore>,
        config: EmbeddingAgentConfig,
    ) -> Self {
        Self {
            id: format!("embedding-processor-{}", Uuid::new_v4()),
            pending_queue,
            embedder,
            store,
            config,
            shutdown: Arc::new(AtomicBool::new(false)),
            metrics: Arc::new(AgentMetrics::default()),
        }
    }

    /// Run the agent's main loop.
    pub async fn run(&self) -> Result<(), AgentError> {
        tracing::info!(agent_id = %self.id, "Embedding processor agent starting");

        while !self.shutdown.load(Ordering::SeqCst) {
            // Process batch from queue
            let batch = self.collect_batch().await;

            if batch.is_empty() {
                // No work, sleep briefly
                tokio::time::sleep(Duration::from_millis(self.config.idle_timeout_ms)).await;
                continue;
            }

            // Process batch through all 13 embedders
            let start = Instant::now();
            match self.process_batch(batch).await {
                Ok(arrays) => {
                    let count = arrays.len();
                    // Store completed arrays
                    if let Err(e) = self.store.store_batch(arrays).await {
                        tracing::error!(error = %e, "Failed to store embedded arrays");
                        self.metrics.store_failures.fetch_add(1, Ordering::SeqCst);
                    } else {
                        self.metrics.embedded_count.fetch_add(count, Ordering::SeqCst);
                        self.metrics.total_latency_ms.fetch_add(
                            start.elapsed().as_millis() as u64,
                            Ordering::SeqCst
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Batch embedding failed");
                    self.metrics.embedding_failures.fetch_add(1, Ordering::SeqCst);
                }
            }
        }

        tracing::info!(agent_id = %self.id, "Embedding processor agent shutdown");
        Ok(())
    }

    /// Signal agent to shutdown.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Collect a batch from the pending queue.
    async fn collect_batch(&self) -> Vec<PendingMemory> {
        let mut queue = self.pending_queue.write().await;

        // Sort by priority (critical first)
        queue.make_contiguous().sort_by_key(|p| std::cmp::Reverse(p.priority));

        let count = queue.len().min(self.config.batch_size);
        queue.drain(..count).collect()
    }

    /// Process batch through all 13 embedders.
    async fn process_batch(&self, batch: Vec<PendingMemory>) -> Result<Vec<TeleologicalArray>, AgentError> {
        let semaphore = Arc::new(Semaphore::new(self.config.parallel_embedders));

        let futures: Vec<_> = batch
            .into_iter()
            .map(|pending| {
                let sem = semaphore.clone();
                let embedder = self.embedder.clone();
                async move {
                    let _permit = sem.acquire().await?;
                    let embeddings = embedder.embed_all(&pending.content, pending.priority).await?;

                    Ok::<_, AgentError>(TeleologicalArray {
                        id: pending.id,
                        embeddings,
                        source_content: Some(pending.content),
                        created_at: pending.source.triggered_at(),
                        metadata: Some(TeleologicalMetadata::from_context(pending.context)),
                        ..Default::default()
                    })
                }
            })
            .collect();

        let results: Vec<Result<TeleologicalArray, AgentError>> =
            futures::future::join_all(futures).await;

        // Filter successful results
        let arrays: Vec<TeleologicalArray> = results
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        Ok(arrays)
    }

    /// Get agent metrics.
    pub fn metrics(&self) -> AgentMetricsSnapshot {
        AgentMetricsSnapshot {
            embedded_count: self.metrics.embedded_count.load(Ordering::SeqCst),
            embedding_failures: self.metrics.embedding_failures.load(Ordering::SeqCst),
            store_failures: self.metrics.store_failures.load(Ordering::SeqCst),
            avg_latency_ms: self.metrics.average_latency(),
        }
    }
}
```

### 4.3 Index Optimization Agent

```rust
/// Background agent for continuous index optimization.
///
/// Monitors index performance and automatically adjusts
/// parameters for optimal query latency.
pub struct IndexOptimizationAgent {
    id: String,
    index_manager: Arc<IndexManager>,
    stats_collector: Arc<IndexStatsCollector>,
    config: OptimizationAgentConfig,
    shutdown: Arc<AtomicBool>,
}

/// Optimization agent configuration.
#[derive(Clone, Debug)]
pub struct OptimizationAgentConfig {
    /// Check interval in seconds
    pub check_interval_secs: u64,
    /// Maximum P99 latency in microseconds before optimization
    pub max_p99_latency_us: u64,
    /// Minimum ef_search / M ratio
    pub min_ef_ratio: f32,
    /// Latency improvement threshold to trigger change
    pub improvement_threshold: f32,
}

impl Default for OptimizationAgentConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 300, // 5 minutes
            max_p99_latency_us: 5000, // 5ms
            min_ef_ratio: 3.0,
            improvement_threshold: 0.1, // 10% improvement
        }
    }
}

impl IndexOptimizationAgent {
    /// Run the optimization loop.
    pub async fn run(&self) -> Result<(), AgentError> {
        tracing::info!(agent_id = %self.id, "Index optimization agent starting");

        while !self.shutdown.load(Ordering::SeqCst) {
            // Collect stats
            let stats = self.stats_collector.collect_all().await?;

            // Analyze each embedder's index
            for embedder in Embedder::all() {
                let idx_stats = &stats[*embedder as usize];

                // Check if optimization needed
                if self.needs_optimization(idx_stats) {
                    match self.optimize_embedder(*embedder, idx_stats).await {
                        Ok(_) => {
                            tracing::info!(embedder = ?embedder, "Index optimized");
                        }
                        Err(e) => {
                            tracing::warn!(embedder = ?embedder, error = %e, "Optimization failed");
                        }
                    }
                }
            }

            // Sleep until next check
            tokio::time::sleep(Duration::from_secs(self.config.check_interval_secs)).await;
        }

        Ok(())
    }

    /// Check if index needs optimization.
    fn needs_optimization(&self, stats: &IndexStats) -> bool {
        // Optimize if P99 latency exceeds threshold
        if stats.p99_search_us > self.config.max_p99_latency_us {
            return true;
        }

        // Optimize if recall appears degraded (based on ef_search ratio)
        if let Some(hnsw) = &stats.hnsw_stats {
            let ef_ratio = hnsw.ef_search as f32 / hnsw.max_connections as f32;
            if ef_ratio < self.config.min_ef_ratio {
                return true;
            }
        }

        false
    }

    /// Optimize a specific embedder's index.
    async fn optimize_embedder(
        &self,
        embedder: Embedder,
        stats: &IndexStats,
    ) -> Result<(), AgentError> {
        tracing::info!(embedder = ?embedder, "Optimizing index");

        let current_config = self.index_manager.config(embedder);

        // Determine optimization strategy
        let new_config = if stats.p99_search_us > self.config.max_p99_latency_us {
            // Latency too high: increase ef_search for better recall
            HnswConfig {
                ef_search: ((current_config.ef_search as f64) * 1.5) as usize,
                ..current_config
            }
        } else if let Some(hnsw) = &stats.hnsw_stats {
            let ef_ratio = hnsw.ef_search as f32 / hnsw.max_connections as f32;
            if ef_ratio < self.config.min_ef_ratio {
                // Recall potentially degraded: increase ef_search
                HnswConfig {
                    ef_search: (hnsw.max_connections as f32 * self.config.min_ef_ratio) as usize,
                    ..current_config
                }
            } else {
                // No optimization needed
                return Ok(());
            }
        } else {
            return Ok(());
        };

        // Apply new configuration
        self.index_manager.update_config(embedder, new_config).await?;

        Ok(())
    }
}
```

### 4.4 Tier Migration Agent

```rust
/// Background agent for memory tier migration.
///
/// Monitors access patterns and migrates memories between
/// hot/warm/cold tiers based on usage and purpose alignment.
pub struct TierMigrationAgent {
    id: String,
    store: Arc<dyn TeleologicalArrayStore>,
    purpose_evaluator: Arc<PurposeEvaluator>,
    access_tracker: Arc<AccessTracker>,
    config: TierMigrationConfig,
    shutdown: Arc<AtomicBool>,
}

/// Tier migration configuration.
#[derive(Clone, Debug)]
pub struct TierMigrationConfig {
    /// Check interval in seconds
    pub check_interval_secs: u64,
    /// Hot tier threshold (0-1)
    pub hot_threshold: f32,
    /// Warm tier threshold (0-1)
    pub warm_threshold: f32,
    /// Cold tier threshold (0-1)
    pub cold_threshold: f32,
    /// Access weight (vs purpose weight)
    pub access_weight: f32,
    /// Maximum migrations per cycle
    pub max_migrations_per_cycle: usize,
}

impl Default for TierMigrationConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 3600, // 1 hour
            hot_threshold: 0.8,
            warm_threshold: 0.5,
            cold_threshold: 0.2,
            access_weight: 0.4,
            max_migrations_per_cycle: 100,
        }
    }
}

impl TierMigrationAgent {
    /// Run the tier migration loop.
    pub async fn run(&self) -> Result<(), AgentError> {
        tracing::info!(agent_id = %self.id, "Tier migration agent starting");

        while !self.shutdown.load(Ordering::SeqCst) {
            // Get candidates for migration
            let candidates = self.find_migration_candidates().await?;

            let mut migrations = 0;
            for (memory_id, current_tier, target_tier) in candidates {
                if migrations >= self.config.max_migrations_per_cycle {
                    break;
                }

                if current_tier != target_tier {
                    match self.store.migrate_tier(memory_id, target_tier).await {
                        Ok(_) => {
                            migrations += 1;
                            tracing::debug!(
                                memory_id = %memory_id,
                                from = ?current_tier,
                                to = ?target_tier,
                                "Memory migrated"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                memory_id = %memory_id,
                                error = %e,
                                "Migration failed"
                            );
                        }
                    }
                }
            }

            if migrations > 0 {
                tracing::info!(count = migrations, "Tier migration cycle complete");
            }

            // Sleep until next check
            tokio::time::sleep(Duration::from_secs(self.config.check_interval_secs)).await;
        }

        Ok(())
    }

    /// Find memories that should be migrated.
    async fn find_migration_candidates(
        &self,
    ) -> Result<Vec<(Uuid, MemoryTier, MemoryTier)>, AgentError> {
        let mut candidates = Vec::new();

        // Check each tier for migration candidates
        for tier in [MemoryTier::Hot, MemoryTier::Warm, MemoryTier::Cold] {
            let memories = self.store.list_by_tier(tier).await?;

            for memory in memories {
                let access_score = self.access_tracker.score(memory.id);
                let purpose_score = self.purpose_evaluator.alignment_score(&memory);

                let target = self.calculate_target_tier(access_score, purpose_score);
                if target != tier {
                    candidates.push((memory.id, tier, target));
                }
            }
        }

        // Sort by urgency (biggest tier jumps first)
        candidates.sort_by_key(|(_, from, to)| {
            std::cmp::Reverse((*from as i8 - *to as i8).abs())
        });

        Ok(candidates)
    }

    /// Calculate target tier based on access and purpose scores.
    fn calculate_target_tier(&self, access_score: f32, purpose_score: f32) -> MemoryTier {
        let combined = access_score * self.config.access_weight
            + purpose_score * (1.0 - self.config.access_weight);

        match combined {
            s if s >= self.config.hot_threshold => MemoryTier::Hot,
            s if s >= self.config.warm_threshold => MemoryTier::Warm,
            s if s >= self.config.cold_threshold => MemoryTier::Cold,
            _ => MemoryTier::Archive,
        }
    }
}
```

### 4.5 Subagent Spawning Configuration

```yaml
# .claude/agents/storage-agents.yaml
agents:
  embedding-processor:
    type: background
    description: Processes pending memories through 13-model embedding pipeline
    spawn_on:
      - session_start
      - pending_queue_threshold: 10
    config:
      batch_size: 50
      idle_timeout_ms: 100
      max_retries: 3
      parallel_embedders: 4
    resources:
      memory_limit: 2GB
      cpu_limit: 2
    health_check:
      interval_secs: 60
      timeout_secs: 5
      unhealthy_threshold: 3

  index-optimizer:
    type: background
    description: Continuously monitors and optimizes HNSW indices
    spawn_on:
      - session_start
      - schedule: "*/15 * * * *"  # Every 15 minutes
    config:
      check_interval_secs: 300
      max_p99_latency_us: 5000
      min_ef_ratio: 3.0
      improvement_threshold: 0.1
    resources:
      memory_limit: 512MB
      cpu_limit: 1
    health_check:
      interval_secs: 120
      timeout_secs: 10

  tier-migrator:
    type: background
    description: Migrates memories between storage tiers based on access patterns
    spawn_on:
      - schedule: "0 * * * *"  # Every hour
    config:
      check_interval_secs: 3600
      hot_threshold: 0.8
      warm_threshold: 0.5
      cold_threshold: 0.2
      access_weight: 0.4
      max_migrations_per_cycle: 100
    resources:
      memory_limit: 256MB
      cpu_limit: 0.5
    health_check:
      interval_secs: 300
      timeout_secs: 30
```

---

## 5. Storage Interface

### 5.1 Core Trait

```rust
/// Primary storage interface for teleological arrays.
///
/// A TeleologicalArray is always stored as an atomic unit.
/// All 13 embedder outputs must be present and valid.
#[async_trait]
pub trait TeleologicalArrayStore: Send + Sync {
    /// Store a teleological array atomically.
    ///
    /// This operation:
    /// 1. Validates all 13 embedder outputs are present
    /// 2. Serializes the array as a single blob
    /// 3. Writes to primary storage
    /// 4. Updates all 13 per-embedder indices
    /// 5. **Triggers PostStore hook** (if configured)
    ///
    /// Returns the generated UUID for the stored array.
    /// If an ID is pre-set on the array, uses that ID.
    async fn store(&self, array: TeleologicalArray) -> Result<Uuid, StorageError>;

    /// Store multiple arrays in a batch (optimized for autonomous agents).
    ///
    /// Uses RocksDB WriteBatch for atomicity:
    /// - All arrays succeed or none do
    /// - Index updates are batched per-embedder
    /// - **Triggers PostBatchStore hook** with batch stats
    ///
    /// More efficient than calling store() in a loop.
    /// Returns IDs in the same order as input arrays.
    async fn store_batch(&self, arrays: Vec<TeleologicalArray>) -> Result<Vec<Uuid>, StorageError>;

    /// Retrieve a teleological array by ID.
    ///
    /// Returns the complete 13-embedder array or None if not found.
    /// **Updates access tracker** for tier migration decisions.
    async fn retrieve(&self, id: Uuid) -> Result<Option<TeleologicalArray>, StorageError>;

    /// Retrieve multiple arrays by ID (batch operation).
    ///
    /// Optimized for autonomous agent batch retrieval.
    async fn retrieve_batch(&self, ids: &[Uuid]) -> Result<Vec<Option<TeleologicalArray>>, StorageError>;

    /// Delete a teleological array atomically.
    ///
    /// Removes from primary storage AND all 13 indices.
    async fn delete(&self, id: Uuid) -> Result<bool, StorageError>;

    /// Count total stored arrays.
    async fn count(&self) -> Result<usize, StorageError>;

    /// Check if an ID exists.
    async fn exists(&self, id: Uuid) -> Result<bool, StorageError>;

    /// Get storage statistics.
    async fn stats(&self) -> Result<StorageStats, StorageError>;

    /// List memories by session ID.
    async fn list_by_session(&self, session_id: &str) -> Result<Vec<TeleologicalArray>, StorageError>;

    /// List memories by storage tier.
    async fn list_by_tier(&self, tier: MemoryTier) -> Result<Vec<TeleologicalArray>, StorageError>;

    /// List memories created before a timestamp.
    async fn list_before(&self, before: DateTime<Utc>) -> Result<Vec<TeleologicalArray>, StorageError>;

    /// Migrate a memory to a different tier.
    async fn migrate_tier(&self, id: Uuid, target_tier: MemoryTier) -> Result<(), StorageError>;

    /// List memories in a key range (for compaction hooks).
    async fn list_range(
        &self,
        start_key: Option<&[u8]>,
        end_key: Option<&[u8]>,
    ) -> Result<Vec<TeleologicalArray>, StorageError>;

    /// List all memories (use with caution).
    async fn list_all(&self) -> Result<Vec<TeleologicalArray>, StorageError>;
}
```

### 5.2 Indexed Search Trait

```rust
/// Extended interface with per-embedder indexed search capabilities.
///
/// CRITICAL: All searches are apples-to-apples.
/// When searching by embedder E_i, we only compare E_i vectors to E_i vectors.
#[async_trait]
pub trait IndexedTeleologicalStore: TeleologicalArrayStore {
    /// Search using a single embedder's index.
    ///
    /// This is the PRIMARY search method. It queries the HNSW index
    /// for the specified embedder and returns full TeleologicalArrays.
    ///
    /// APPLES-TO-APPLES: Only E_i is compared to E_i.
    async fn search_single_embedder(
        &self,
        query_embedding: &EmbedderOutput,
        embedder: Embedder,
        top_k: usize,
        filter: Option<SearchFilter>,
    ) -> Result<Vec<SearchResult>, StorageError>;

    /// Search using a comparison type (multi-embedder).
    ///
    /// For ComparisonType::WeightedFull or EmbedderGroup:
    /// 1. Search each active embedder's index separately
    /// 2. Collect results from each (apples-to-apples)
    /// 3. Aggregate using RRF or weighted average
    ///
    /// The query must be a complete TeleologicalArray.
    async fn search(
        &self,
        query: &TeleologicalArray,
        comparison: ComparisonType,
        top_k: usize,
        filter: Option<SearchFilter>,
    ) -> Result<Vec<SearchResult>, StorageError>;

    /// Rebuild a specific embedder's index.
    ///
    /// Reads all stored arrays and rebuilds the HNSW graph
    /// for the specified embedder. Does not affect other indices.
    async fn rebuild_index(&self, embedder: Embedder) -> Result<IndexStats, StorageError>;

    /// Rebuild all 13 indices (parallel execution).
    async fn rebuild_all_indices(&self) -> Result<Vec<IndexStats>, StorageError>;

    /// Get statistics for a specific embedder's index.
    async fn index_stats(&self, embedder: Embedder) -> Result<IndexStats, StorageError>;

    /// Get statistics for all 13 indices.
    async fn all_index_stats(&self) -> Result<[IndexStats; 13], StorageError>;
}

/// Search result with similarity and metadata.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// The matched teleological array (full 13 embedders).
    pub array: TeleologicalArray,

    /// Overall similarity score [0, 1].
    /// For single-embedder search: the embedder's score.
    /// For multi-embedder search: RRF or weighted aggregate.
    pub similarity: f32,

    /// Per-embedder scores (for analysis).
    /// Only populated for multi-embedder searches.
    pub embedder_scores: [f32; 13],

    /// Which embedders were used in the search.
    pub active_embedders: EmbedderMask,

    /// Rank in the result set (1-indexed).
    pub rank: usize,
}

/// Filter for search operations.
#[derive(Clone, Debug, Default)]
pub struct SearchFilter {
    /// Minimum similarity threshold.
    pub min_similarity: Option<f32>,

    /// Time range filter.
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,

    /// Metadata filters.
    pub metadata_filters: Vec<MetadataFilter>,

    /// Exclude specific IDs.
    pub exclude_ids: Vec<Uuid>,

    /// Maximum candidates to evaluate (for performance).
    pub max_candidates: Option<usize>,
}
```

---

## 6. Index Architecture

### 6.1 Per-Embedder Index Design

Each of the 13 embedders has its own dedicated index. This ensures:
- **Apples-to-apples comparisons**: E1 only compared to E1
- **Optimal index type per embedder**: HNSW for dense, inverted for sparse
- **Independent scaling**: Hot embedders can be cached more aggressively

```rust
/// Index configuration per embedder.
#[derive(Clone, Debug)]
pub struct EmbedderIndexConfig {
    /// Which embedder this index serves.
    pub embedder: Embedder,

    /// Index type (depends on embedding type).
    pub index_type: IndexType,

    /// HNSW parameters (for dense embeddings).
    pub hnsw_config: Option<HnswConfig>,

    /// Inverted index parameters (for sparse embeddings).
    pub inverted_config: Option<InvertedIndexConfig>,

    /// Token-level index parameters (for E12 late interaction).
    pub token_config: Option<TokenIndexConfig>,
}

#[derive(Clone, Copy, Debug)]
pub enum IndexType {
    /// HNSW for dense embeddings (E1-E5, E7-E11).
    /// Best for high recall with O(log N) query time.
    Hnsw,

    /// Inverted index for sparse embeddings (E6, E13).
    /// Efficient for lexical/keyword matching.
    Inverted,

    /// Token-level index for late interaction (E12).
    /// Supports MaxSim computation across token embeddings.
    TokenLevel,
}

/// HNSW configuration optimized per embedder dimension.
#[derive(Clone, Debug, PartialEq)]
pub struct HnswConfig {
    /// Maximum number of connections per node.
    /// Higher M = better recall, more memory.
    /// Recommended: 16 for <512D, 32 for >512D.
    pub m: usize,

    /// Size of dynamic candidate list during construction.
    /// Higher = better index quality, slower build.
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search.
    /// Higher = better recall, slower query.
    /// Can be adjusted at query time.
    pub ef_search: usize,

    /// Distance metric.
    pub metric: DistanceMetric,

    /// Memory tier (hot/warm/cold).
    pub memory_tier: MemoryTier,
}

impl HnswConfig {
    /// Configuration for semantic embedder (E1: 1024D).
    pub fn for_semantic() -> Self {
        Self {
            m: 32,
            ef_construction: 256,
            ef_search: 128,
            metric: DistanceMetric::Cosine,
            memory_tier: MemoryTier::Hot,
        }
    }

    /// Configuration for medium-dim embedders (E2-E5, E8-E11: 256-768D).
    pub fn for_medium_dim() -> Self {
        Self {
            m: 24,
            ef_construction: 200,
            ef_search: 100,
            metric: DistanceMetric::Cosine,
            memory_tier: MemoryTier::Warm,
        }
    }

    /// Configuration for code embedder (E7: 1536D).
    pub fn for_code() -> Self {
        Self {
            m: 32,
            ef_construction: 256,
            ef_search: 128,
            metric: DistanceMetric::Cosine,
            memory_tier: MemoryTier::Hot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
    /// For E5 Causal: asymmetric distance
    AsymmetricCosine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryTier {
    /// Kept in memory, fastest access.
    Hot = 0,
    /// Memory-mapped, moderate access.
    Warm = 1,
    /// Disk-resident, slowest access.
    Cold = 2,
    /// Compressed archive storage.
    Archive = 3,
}
```

### 6.2 Index Type Selection by Embedder

| Embedder | Index Type | Dimensions | HNSW M | ef_search | Distance | Tier |
|----------|------------|------------|--------|-----------|----------|------|
| E1 Semantic | HNSW | 1024D | 32 | 128 | Cosine | Hot |
| E2 TemporalRecent | HNSW | 512D | 24 | 100 | Cosine | Warm |
| E3 TemporalPeriodic | HNSW | 512D | 24 | 100 | Cosine | Warm |
| E4 TemporalPositional | HNSW | 512D | 24 | 100 | Cosine | Warm |
| E5 Causal | HNSW | 768D | 24 | 100 | Asymmetric | Warm |
| E6 SpladePrimary | Inverted | ~30K sparse | N/A | N/A | Sparse Dot | Hot |
| E7 Code | HNSW | 1536D | 32 | 128 | Cosine | Hot |
| E8 Graph | HNSW | 384D | 16 | 64 | Cosine | Cold |
| E9 HDC | HNSW | 1024D | 32 | 128 | Cosine | Cold |
| E10 Multimodal | HNSW | 768D | 24 | 100 | Cosine | Warm |
| E11 Entity | HNSW | 384D | 16 | 64 | Cosine | Warm |
| E12 LateInteraction | TokenLevel | 128D/tok | N/A | N/A | MaxSim | Cold |
| E13 SpladeKeyword | Inverted | ~30K sparse | N/A | N/A | Sparse Dot | Hot |

### 6.3 Storage Layout

```
Storage Structure:
/data/teleological/
  +-- arrays/                    # Primary atomic storage (RocksDB)
  |   +-- cf_arrays/             # Column family: serialized TeleologicalArrays
  |   +-- cf_metadata/           # Column family: array metadata
  |   +-- cf_id_map/             # Column family: UUID -> internal ID mapping
  |   +-- cf_sessions/           # Column family: session -> array ID mapping
  |   +-- cf_tiers/              # Column family: tier -> array ID mapping
  |
  +-- indices/                   # Per-embedder HNSW indices
  |   +-- e01_semantic/          # E1: HNSW index (1024D)
  |   |   +-- graph.bin          # HNSW graph structure
  |   |   +-- vectors.bin        # Quantized vectors (PQ-8)
  |   |   +-- metadata.json      # Index stats, config
  |   |
  |   +-- e02_temporal_recent/   # E2: HNSW index (512D)
  |   +-- e03_temporal_periodic/ # E3: HNSW index (512D)
  |   +-- e04_temporal_pos/      # E4: HNSW index (512D)
  |   +-- e05_causal/            # E5: HNSW index (768D, asymmetric)
  |   +-- e06_splade/            # E6: Inverted index
  |   |   +-- postings.bin       # Posting lists
  |   |   +-- vocab.bin          # Token vocabulary
  |   |
  |   +-- e07_code/              # E7: HNSW index (1536D)
  |   +-- e08_graph/             # E8: HNSW index (384D)
  |   +-- e09_hdc/               # E9: HNSW index (1024D)
  |   +-- e10_multimodal/        # E10: HNSW index (768D)
  |   +-- e11_entity/            # E11: HNSW index (384D)
  |   +-- e12_late/              # E12: Token-level index
  |   |   +-- token_vectors.bin  # Per-token embeddings
  |   |   +-- doc_index.bin      # Document -> token mapping
  |   |
  |   +-- e13_keyword/           # E13: Inverted index
  |
  +-- hooks/                     # Hook state and queues
  |   +-- pending_queue.bin      # Pending embeddings queue
  |   +-- session_state/         # Per-session hook state
  |
  +-- agents/                    # Subagent state
  |   +-- embedding_processor/   # Embedding agent state
  |   +-- index_optimizer/       # Optimizer agent state
  |   +-- tier_migrator/         # Migration agent state
  |
  +-- snapshots/                 # Point-in-time snapshots
  |   +-- 2025-01-09T12:00:00Z/
  |
  +-- wal/                       # Write-ahead log for recovery
```

### 6.4 Index Manager with Hook Integration

```rust
/// Manages all 13 per-embedder indices.
///
/// Coordinates atomic updates across all indices when storing arrays.
/// Enables parallel index searches for multi-embedder queries.
/// **Integrates with hooks** for storage events.
pub struct IndexManager {
    /// Per-embedder indices (13 total).
    indices: [Box<dyn EmbedderIndex>; 13],

    /// Shared ID mapping (UUID -> internal u64).
    id_map: Arc<RwLock<IdMap>>,

    /// Background index maintenance queue.
    maintenance_queue: IndexMaintenanceQueue,

    /// Index statistics collector.
    stats: Arc<IndexStatsCollector>,

    /// Hook dispatcher for storage events.
    hook_dispatcher: Arc<HookDispatcher>,
}

impl IndexManager {
    /// Create a new index manager with the given configuration.
    pub fn new(config: &IndexManagerConfig) -> Result<Self, StorageError> {
        let indices = create_all_indices(config)?;
        Ok(Self {
            indices,
            id_map: Arc::new(RwLock::new(IdMap::new())),
            maintenance_queue: IndexMaintenanceQueue::new(),
            stats: Arc::new(IndexStatsCollector::new()),
            hook_dispatcher: Arc::new(HookDispatcher::new()),
        })
    }

    /// Create with hook dispatcher.
    pub fn new_with_hooks(
        config: &IndexManagerConfig,
        hook_dispatcher: Arc<HookDispatcher>,
    ) -> Result<Self, StorageError> {
        let indices = create_all_indices(config)?;
        Ok(Self {
            indices,
            id_map: Arc::new(RwLock::new(IdMap::new())),
            maintenance_queue: IndexMaintenanceQueue::new(),
            stats: Arc::new(IndexStatsCollector::new()),
            hook_dispatcher,
        })
    }

    /// Add an array to all 13 indices atomically.
    ///
    /// This operation:
    /// 1. Allocates an internal ID
    /// 2. Adds to each embedder's index in parallel
    /// 3. Commits all or rolls back on failure
    /// 4. **Dispatches PostIndexUpdate hook**
    pub async fn add(&self, id: Uuid, array: &TeleologicalArray) -> Result<(), StorageError> {
        let internal_id = self.id_map.write().await.allocate(id)?;

        // Add to all 13 indices in parallel
        let futures: Vec<_> = array.embeddings.iter()
            .enumerate()
            .map(|(i, emb)| {
                let index = &self.indices[i];
                async move { index.add(internal_id, emb).await }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        // Check for failures and rollback if needed
        let errors: Vec<_> = results.iter()
            .enumerate()
            .filter_map(|(i, r)| r.as_ref().err().map(|e| (i, e.clone())))
            .collect();

        if !errors.is_empty() {
            // Rollback: remove from indices that succeeded
            self.rollback_add(internal_id, &errors).await;
            return Err(StorageError::IndexUpdateFailed {
                embedders: errors.iter().map(|(i, _)| Embedder::from_index(*i)).collect(),
            });
        }

        // Dispatch hook on successful index update
        self.hook_dispatcher.dispatch(HookEvent::PostIndexUpdate {
            array_id: id,
            embedders_updated: 13,
        }).await;

        Ok(())
    }

    /// Remove an array from all 13 indices.
    pub async fn remove(&self, id: Uuid) -> Result<(), StorageError> {
        let internal_id = self.id_map.read().await.get(id)
            .ok_or(StorageError::NotFound(id))?;

        let futures: Vec<_> = self.indices.iter()
            .map(|index| index.remove(internal_id))
            .collect();

        let _ = futures::future::join_all(futures).await;
        self.id_map.write().await.remove(id);

        Ok(())
    }

    /// Search a single embedder's index.
    pub async fn search_embedder(
        &self,
        embedder: Embedder,
        query: &EmbedderOutput,
        top_k: usize,
    ) -> Result<Vec<(u64, f32)>, StorageError> {
        let idx = embedder as usize;
        self.indices[idx].search(query, top_k).await.map_err(Into::into)
    }

    /// Search multiple embedders and aggregate with RRF.
    pub async fn search_multi_embedder(
        &self,
        query: &TeleologicalArray,
        embedders: &[Embedder],
        top_k: usize,
        k_rrf: f32,
    ) -> Result<Vec<(Uuid, f32, [f32; 13])>, StorageError> {
        // Search each embedder in parallel
        let futures: Vec<_> = embedders.iter()
            .map(|e| {
                let idx = *e as usize;
                let query_emb = &query.embeddings[idx];
                async move {
                    (*e, self.indices[idx].search(query_emb, top_k * 3).await)
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        // Aggregate using Reciprocal Rank Fusion
        let aggregated = self.rrf_aggregate(&results, k_rrf)?;

        // Resolve internal IDs to UUIDs
        let id_map = self.id_map.read().await;
        let resolved: Vec<_> = aggregated.into_iter()
            .filter_map(|(internal_id, score, per_emb)| {
                id_map.resolve(internal_id).map(|uuid| (uuid, score, per_emb))
            })
            .take(top_k)
            .collect();

        Ok(resolved)
    }

    /// Update temporal embeddings for a memory.
    pub async fn update_temporal_embeddings(&self, id: Uuid) -> Result<(), StorageError> {
        // Update E2, E3, E4 indices based on access patterns
        // This is called by SessionEnd consolidation
        todo!("Implement temporal embedding update")
    }

    /// Get configuration for an embedder's index.
    pub fn config(&self, embedder: Embedder) -> HnswConfig {
        self.indices[embedder as usize].config()
    }

    /// Update configuration for an embedder's index.
    pub async fn update_config(&self, embedder: Embedder, config: HnswConfig) -> Result<(), StorageError> {
        self.indices[embedder as usize].update_config(config).await.map_err(Into::into)
    }

    /// Rebuild index for a specific embedder.
    pub async fn rebuild_index(&self, embedder: Embedder) -> Result<IndexStats, StorageError> {
        self.indices[embedder as usize].rebuild().await.map_err(Into::into)
    }

    /// Rebuild all 13 indices in parallel.
    pub async fn rebuild_all_indices(&self) -> Result<Vec<IndexStats>, StorageError> {
        let futures: Vec<_> = self.indices.iter()
            .map(|index| index.rebuild())
            .collect();

        let results = futures::future::join_all(futures).await;
        results.into_iter().map(|r| r.map_err(Into::into)).collect()
    }

    /// Get statistics for a specific embedder's index.
    pub async fn index_stats(&self, embedder: Embedder) -> Result<IndexStats, StorageError> {
        Ok(self.indices[embedder as usize].stats())
    }

    /// Get statistics for all 13 indices.
    pub async fn all_stats(&self) -> [IndexStats; 13] {
        std::array::from_fn(|i| self.indices[i].stats())
    }

    /// Aggregate results using Reciprocal Rank Fusion.
    fn rrf_aggregate(
        &self,
        results: &[(Embedder, Result<Vec<(u64, f32)>, IndexError>)],
        k: f32,
    ) -> Result<Vec<(u64, f32, [f32; 13])>, StorageError> {
        let mut scores: HashMap<u64, (f32, [f32; 13])> = HashMap::new();

        for (embedder, result) in results {
            if let Ok(hits) = result {
                for (rank, (id, sim)) in hits.iter().enumerate() {
                    let rrf_score = 1.0 / (k + rank as f32 + 1.0);
                    let entry = scores.entry(*id).or_insert((0.0, [0.0; 13]));
                    entry.0 += rrf_score;
                    entry.1[*embedder as usize] = *sim;
                }
            }
        }

        let mut sorted: Vec<_> = scores.into_iter()
            .map(|(id, (score, per_emb))| (id, score, per_emb))
            .collect();

        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(sorted)
    }
}

/// Trait for individual embedder indices.
#[async_trait]
pub trait EmbedderIndex: Send + Sync {
    /// Add an embedding to the index.
    async fn add(&self, internal_id: u64, embedding: &EmbedderOutput) -> Result<(), IndexError>;

    /// Remove an embedding from the index.
    async fn remove(&self, internal_id: u64) -> Result<(), IndexError>;

    /// Search for similar embeddings.
    ///
    /// Returns (internal_id, similarity) pairs sorted by similarity descending.
    async fn search(
        &self,
        query: &EmbedderOutput,
        top_k: usize,
    ) -> Result<Vec<(u64, f32)>, IndexError>;

    /// Rebuild the entire index from scratch.
    async fn rebuild(&self) -> Result<IndexStats, IndexError>;

    /// Get index statistics.
    fn stats(&self) -> IndexStats;

    /// Get the embedder type this index serves.
    fn embedder(&self) -> Embedder;

    /// Get current configuration.
    fn config(&self) -> HnswConfig;

    /// Update configuration.
    async fn update_config(&self, config: HnswConfig) -> Result<(), IndexError>;
}
```

---

## 7. RocksDB Implementation

### 7.1 Column Families

```rust
/// RocksDB column families for teleological storage.
pub struct TeleologicalColumnFamilies {
    /// Primary array storage (key: UUID bytes, value: serialized TeleologicalArray).
    pub cf_arrays: ColumnFamily,

    /// Array metadata (key: UUID bytes, value: ArrayMetadata).
    pub cf_metadata: ColumnFamily,

    /// UUID to internal ID mapping (key: UUID bytes, value: u64 LE).
    pub cf_id_map: ColumnFamily,

    /// Internal ID to UUID reverse mapping (key: u64 LE, value: UUID bytes).
    pub cf_reverse_id_map: ColumnFamily,

    /// Soft-deleted arrays (key: UUID bytes, value: deletion timestamp).
    pub cf_tombstones: ColumnFamily,

    /// Session to array ID mapping (key: session_id, value: array IDs).
    pub cf_sessions: ColumnFamily,

    /// Tier to array ID mapping (key: tier, value: array IDs).
    pub cf_tiers: ColumnFamily,
}
```

### 7.2 RocksDB Store Implementation with Hook Integration

```rust
/// RocksDB-backed teleological array store.
///
/// Provides atomic storage of complete TeleologicalArrays with
/// per-embedder HNSW indexing and **hook integration**.
pub struct RocksDbTeleologicalStore {
    /// RocksDB instance.
    db: Arc<DB>,

    /// Column families.
    cf: TeleologicalColumnFamilies,

    /// Index manager for all 13 embedder indices.
    index_manager: Arc<IndexManager>,

    /// Serialization format.
    serializer: ArraySerializer,

    /// Write options.
    write_opts: WriteOptions,

    /// Configuration.
    config: RocksDbStoreConfig,

    /// Hook dispatcher for storage events.
    hook_dispatcher: Arc<HookDispatcher>,

    /// Access tracker for tier migration.
    access_tracker: Arc<AccessTracker>,
}

impl RocksDbTeleologicalStore {
    /// Open or create a RocksDB-backed store.
    pub fn open(path: impl AsRef<Path>, config: RocksDbStoreConfig) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Performance tuning for vector workload
        opts.set_compression_type(CompressionType::Lz4);
        opts.set_max_open_files(1000);
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_max_write_buffer_number(4);
        opts.set_target_file_size_base(64 * 1024 * 1024);
        opts.set_max_background_jobs(4);

        // Enable direct I/O for better performance with large values
        opts.set_use_direct_reads(true);
        opts.set_use_direct_io_for_flush_and_compaction(true);

        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new("arrays", Options::default()),
            ColumnFamilyDescriptor::new("metadata", Options::default()),
            ColumnFamilyDescriptor::new("id_map", Options::default()),
            ColumnFamilyDescriptor::new("reverse_id_map", Options::default()),
            ColumnFamilyDescriptor::new("tombstones", Options::default()),
            ColumnFamilyDescriptor::new("sessions", Options::default()),
            ColumnFamilyDescriptor::new("tiers", Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path.as_ref(), cf_descriptors)?;

        // Initialize index manager with hook dispatcher
        let hook_dispatcher = Arc::new(HookDispatcher::new());
        let index_manager = IndexManager::new_with_hooks(&config.index_config, hook_dispatcher.clone())?;

        Ok(Self {
            db: Arc::new(db),
            cf: TeleologicalColumnFamilies::from_db(&db)?,
            index_manager: Arc::new(index_manager),
            serializer: ArraySerializer::new(config.serialization_format),
            write_opts: Self::create_write_opts(&config),
            config,
            hook_dispatcher,
            access_tracker: Arc::new(AccessTracker::new()),
        })
    }
}

#[async_trait]
impl TeleologicalArrayStore for RocksDbTeleologicalStore {
    async fn store(&self, array: TeleologicalArray) -> Result<Uuid, StorageError> {
        let id = array.id;

        // Validate array completeness
        validate_array_complete(&array)?;

        // Serialize the complete array
        let bytes = self.serializer.serialize(&array)?;

        // Create metadata
        let metadata = ArrayMetadata::from(&array);
        let metadata_bytes = serde_json::to_vec(&metadata)?;

        // Use WriteBatch for atomic write
        let mut batch = WriteBatch::default();
        batch.put_cf(&self.cf.cf_arrays, id.as_bytes(), &bytes);
        batch.put_cf(&self.cf.cf_metadata, id.as_bytes(), &metadata_bytes);

        // Write to RocksDB
        self.db.write_opt(&batch, &self.write_opts)?;

        // Add to all 13 indices
        self.index_manager.add(id, &array).await?;

        // Dispatch PostStore hook
        self.hook_dispatcher.dispatch(HookEvent::PostStore {
            array_id: id,
            size_bytes: bytes.len(),
        }).await;

        Ok(id)
    }

    async fn store_batch(&self, arrays: Vec<TeleologicalArray>) -> Result<Vec<Uuid>, StorageError> {
        if arrays.is_empty() {
            return Ok(Vec::new());
        }

        // Validate all arrays first
        for array in &arrays {
            validate_array_complete(array)?;
        }

        let mut batch = WriteBatch::default();
        let mut ids = Vec::with_capacity(arrays.len());
        let mut total_bytes = 0usize;

        // Serialize and batch all writes
        for array in &arrays {
            let id = array.id;
            ids.push(id);

            let bytes = self.serializer.serialize(array)?;
            total_bytes += bytes.len();
            let metadata = ArrayMetadata::from(array);
            let metadata_bytes = serde_json::to_vec(&metadata)?;

            batch.put_cf(&self.cf.cf_arrays, id.as_bytes(), &bytes);
            batch.put_cf(&self.cf.cf_metadata, id.as_bytes(), &metadata_bytes);
        }

        // Atomic write to RocksDB
        self.db.write_opt(&batch, &self.write_opts)?;

        // Add to indices in parallel (batched per embedder)
        let index_futures: Vec<_> = arrays.iter()
            .map(|array| self.index_manager.add(array.id, array))
            .collect();

        let results = futures::future::join_all(index_futures).await;

        // Check for index failures
        let failures: Vec<_> = results.iter()
            .enumerate()
            .filter_map(|(i, r)| r.as_ref().err().map(|e| (i, e)))
            .collect();

        if !failures.is_empty() {
            // Log but don't fail - data is safely stored, indices can be rebuilt
            tracing::warn!(
                "Index update failed for {} arrays, rebuild recommended",
                failures.len()
            );
        }

        // Dispatch PostBatchStore hook
        self.hook_dispatcher.dispatch(HookEvent::PostBatchStore {
            array_count: arrays.len(),
            total_bytes,
            failed_indices: failures.len(),
        }).await;

        Ok(ids)
    }

    async fn retrieve(&self, id: Uuid) -> Result<Option<TeleologicalArray>, StorageError> {
        // Check tombstone first
        if self.db.get_cf(&self.cf.cf_tombstones, id.as_bytes())?.is_some() {
            return Ok(None);
        }

        match self.db.get_cf(&self.cf.cf_arrays, id.as_bytes())? {
            Some(bytes) => {
                let array = self.serializer.deserialize(&bytes)?;

                // Update access tracker for tier migration
                self.access_tracker.record_access(id);

                Ok(Some(array))
            }
            None => Ok(None),
        }
    }

    async fn retrieve_batch(&self, ids: &[Uuid]) -> Result<Vec<Option<TeleologicalArray>>, StorageError> {
        let keys: Vec<_> = ids.iter().map(|id| id.as_bytes().to_vec()).collect();

        // Multi-get for efficiency
        let results = self.db.multi_get_cf(
            keys.iter().map(|k| (&self.cf.cf_arrays, k.as_slice()))
        );

        let mut arrays = Vec::with_capacity(ids.len());
        for (i, result) in results.into_iter().enumerate() {
            match result? {
                Some(bytes) => {
                    // Check tombstone
                    let id = ids[i];
                    if self.db.get_cf(&self.cf.cf_tombstones, id.as_bytes())?.is_some() {
                        arrays.push(None);
                    } else {
                        let array = self.serializer.deserialize(&bytes)?;
                        self.access_tracker.record_access(id);
                        arrays.push(Some(array));
                    }
                }
                None => arrays.push(None),
            }
        }

        Ok(arrays)
    }

    async fn delete(&self, id: Uuid) -> Result<bool, StorageError> {
        // Check if exists
        if self.db.get_cf(&self.cf.cf_arrays, id.as_bytes())?.is_none() {
            return Ok(false);
        }

        let mut batch = WriteBatch::default();

        // Remove from primary storage
        batch.delete_cf(&self.cf.cf_arrays, id.as_bytes());
        batch.delete_cf(&self.cf.cf_metadata, id.as_bytes());

        // Add tombstone
        let now = Utc::now().timestamp();
        batch.put_cf(&self.cf.cf_tombstones, id.as_bytes(), &now.to_le_bytes());

        // Write atomically
        self.db.write_opt(&batch, &self.write_opts)?;

        // Remove from all indices
        self.index_manager.remove(id).await?;

        Ok(true)
    }

    async fn count(&self) -> Result<usize, StorageError> {
        let iter = self.db.iterator_cf(&self.cf.cf_arrays, IteratorMode::Start);
        Ok(iter.count())
    }

    async fn exists(&self, id: Uuid) -> Result<bool, StorageError> {
        // Check tombstone first
        if self.db.get_cf(&self.cf.cf_tombstones, id.as_bytes())?.is_some() {
            return Ok(false);
        }

        Ok(self.db.get_cf(&self.cf.cf_arrays, id.as_bytes())?.is_some())
    }

    async fn stats(&self) -> Result<StorageStats, StorageError> {
        let array_count = self.count().await?;
        let tombstone_count = self.db.iterator_cf(&self.cf.cf_tombstones, IteratorMode::Start).count();

        // Get storage size from RocksDB
        let storage_bytes = self.db.property_int_value("rocksdb.total-sst-files-size")?
            .unwrap_or(0);

        // Get per-embedder index stats
        let index_stats = self.index_manager.all_stats().await;

        Ok(StorageStats {
            total_arrays: array_count,
            tombstone_count,
            storage_bytes,
            index_stats,
            avg_array_bytes: if array_count > 0 {
                storage_bytes as usize / array_count
            } else {
                0
            },
            last_compaction: self.config.last_compaction,
            cache_hit_rate: self.calculate_cache_hit_rate(),
        })
    }

    async fn list_by_session(&self, session_id: &str) -> Result<Vec<TeleologicalArray>, StorageError> {
        // Get array IDs for session
        let ids_bytes = self.db.get_cf(&self.cf.cf_sessions, session_id.as_bytes())?;

        match ids_bytes {
            Some(bytes) => {
                let ids: Vec<Uuid> = bincode::deserialize(&bytes)?;
                let arrays = self.retrieve_batch(&ids).await?;
                Ok(arrays.into_iter().flatten().collect())
            }
            None => Ok(Vec::new()),
        }
    }

    async fn list_by_tier(&self, tier: MemoryTier) -> Result<Vec<TeleologicalArray>, StorageError> {
        let tier_key = (tier as u8).to_le_bytes();
        let ids_bytes = self.db.get_cf(&self.cf.cf_tiers, &tier_key)?;

        match ids_bytes {
            Some(bytes) => {
                let ids: Vec<Uuid> = bincode::deserialize(&bytes)?;
                let arrays = self.retrieve_batch(&ids).await?;
                Ok(arrays.into_iter().flatten().collect())
            }
            None => Ok(Vec::new()),
        }
    }

    async fn list_before(&self, before: DateTime<Utc>) -> Result<Vec<TeleologicalArray>, StorageError> {
        let mut results = Vec::new();
        let iter = self.db.iterator_cf(&self.cf.cf_metadata, IteratorMode::Start);

        for item in iter {
            let (key, value) = item?;
            let metadata: ArrayMetadata = serde_json::from_slice(&value)?;

            if metadata.created_at < before {
                let id = Uuid::from_slice(&key)?;
                if let Some(array) = self.retrieve(id).await? {
                    results.push(array);
                }
            }
        }

        Ok(results)
    }

    async fn migrate_tier(&self, id: Uuid, target_tier: MemoryTier) -> Result<(), StorageError> {
        // Get current metadata
        let metadata_bytes = self.db.get_cf(&self.cf.cf_metadata, id.as_bytes())?
            .ok_or(StorageError::NotFound(id))?;

        let mut metadata: ArrayMetadata = serde_json::from_slice(&metadata_bytes)?;
        let old_tier = metadata.tier;

        // Update metadata
        metadata.tier = target_tier;
        let new_metadata_bytes = serde_json::to_vec(&metadata)?;

        // Update tier indices
        let mut batch = WriteBatch::default();

        // Remove from old tier index
        let old_tier_key = (old_tier as u8).to_le_bytes();
        if let Some(old_ids_bytes) = self.db.get_cf(&self.cf.cf_tiers, &old_tier_key)? {
            let mut old_ids: Vec<Uuid> = bincode::deserialize(&old_ids_bytes)?;
            old_ids.retain(|&i| i != id);
            let new_old_ids_bytes = bincode::serialize(&old_ids)?;
            batch.put_cf(&self.cf.cf_tiers, &old_tier_key, &new_old_ids_bytes);
        }

        // Add to new tier index
        let new_tier_key = (target_tier as u8).to_le_bytes();
        let mut new_ids: Vec<Uuid> = match self.db.get_cf(&self.cf.cf_tiers, &new_tier_key)? {
            Some(bytes) => bincode::deserialize(&bytes)?,
            None => Vec::new(),
        };
        new_ids.push(id);
        let new_ids_bytes = bincode::serialize(&new_ids)?;
        batch.put_cf(&self.cf.cf_tiers, &new_tier_key, &new_ids_bytes);

        // Update metadata
        batch.put_cf(&self.cf.cf_metadata, id.as_bytes(), &new_metadata_bytes);

        // Write atomically
        self.db.write_opt(&batch, &self.write_opts)?;

        Ok(())
    }

    async fn list_range(
        &self,
        start_key: Option<&[u8]>,
        end_key: Option<&[u8]>,
    ) -> Result<Vec<TeleologicalArray>, StorageError> {
        let mode = match start_key {
            Some(key) => IteratorMode::From(key, Direction::Forward),
            None => IteratorMode::Start,
        };

        let iter = self.db.iterator_cf(&self.cf.cf_arrays, mode);
        let mut results = Vec::new();

        for item in iter {
            let (key, value) = item?;

            // Check end key
            if let Some(end) = end_key {
                if key.as_ref() >= end {
                    break;
                }
            }

            let array = self.serializer.deserialize(&value)?;
            results.push(array);
        }

        Ok(results)
    }

    async fn list_all(&self) -> Result<Vec<TeleologicalArray>, StorageError> {
        self.list_range(None, None).await
    }
}
```

---

## 8. Performance Targets

Based on the PRD requirements:

| Operation | Target | Notes |
|-----------|--------|-------|
| Single array store | <10ms | Includes all 13 index updates |
| Batch store (100 arrays) | <200ms | Amortized <2ms per array |
| Single embedder search | <2ms | Per-embedder HNSW query |
| Multi-embedder search (13) | <30ms | Parallel search + RRF fusion |
| Array retrieval | <1ms | RocksDB point lookup |
| Batch retrieval (100) | <20ms | Multi-get optimization |
| Storage per array (quantized) | ~17KB | 63% compression |
| Storage per array (uncompressed) | ~46KB | Full precision |
| HNSW index memory (1M vectors) | ~1.5GB per embedder | M=24, 512D average |
| Hook dispatch latency | <1ms | Background execution |
| Session consolidation | <30s | End-of-session processing |

---

## 9. References

### Internal
- [01-ARCHITECTURE.md](./01-ARCHITECTURE.md) - Core types and comparison architecture
- [03-SEARCH.md](./03-SEARCH.md) - Search layer specification
- [05-NORTH-STAR-REMOVAL.md](./05-NORTH-STAR-REMOVAL.md) - Removal of broken alignment code
- [06-AUTONOMOUS-INTEGRATION.md](./06-AUTONOMOUS-INTEGRATION.md) - Autonomous agent integration

### External (2025 Best Practices)
- [Vector Databases 2025 Benchmarked](https://medium.com/@ThinkingLoop/d3-4-vector-databases-in-2025-top-10-index-choices-benchmarked-1bbce68e1871) - Index selection
- [Vespa Multi-Vector HNSW](https://blog.vespa.ai/semantic-search-with-multi-vector-indexing/) - Multi-field indexing
- [HNSW Fundamentals](https://qdrant.tech/course/essentials/day-2/what-is-hnsw/) - Index architecture
- [CoreNN with RocksDB](https://blog.wilsonl.in/corenn/) - RocksDB for vector storage
- [P-HNSW Crash Consistency](https://www.mdpi.com/2076-3417/15/19/10554) - Persistence patterns
- [Milvus Best Practices](https://milvus.io/ai-quick-reference/what-are-the-best-practices-for-storing-embeddings-in-an-ai-database) - Embedding storage
- [Amazon Keyspaces Batches](https://aws.amazon.com/blogs/database/amazon-keyspaces-now-supports-logged-batches-for-atomic-multi-statement-operations/) - Atomic batch operations
- [Vector Search Cloud Architecture](https://arxiv.org/html/2601.01937) - Tiered storage patterns
- [Claude Code Hooks](https://docs.anthropic.com/en/docs/claude-code/hooks) - Hook integration patterns

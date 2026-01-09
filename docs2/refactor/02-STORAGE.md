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

### 1.2 Research-Backed Decisions

Based on 2025 best practices for multi-vector storage:

| Decision | Rationale | Source |
|----------|-----------|--------|
| 13 separate HNSW indices | Multi-field vector indexing with no overhead from searching multiple graphs | [Vespa Multi-Vector HNSW](https://blog.vespa.ai/semantic-search-with-multi-vector-indexing/) |
| RocksDB as primary store | Robust KV store for graph/vector metadata with efficient compaction | [CoreNN Architecture](https://blog.wilsonl.in/corenn/) |
| Mutable HNSW graphs | One graph per embedder per content node | [Weaviate Vector Indexing](https://docs.weaviate.io/weaviate/concepts/vector-index) |
| WriteBatch for atomicity | Atomic multi-statement operations | [Amazon Keyspaces Logged Batches](https://aws.amazon.com/blogs/database/amazon-keyspaces-now-supports-logged-batches-for-atomic-multi-statement-operations/) |
| Tiered storage (hot/warm/cold) | Memory-SSD-object architecture by access frequency | [Vector Search Cloud Architectures](https://arxiv.org/html/2601.01937) |

### 1.3 Storage Pipeline with Hooks

```
                    STORAGE PIPELINE WITH HOOK INTEGRATION
    ================================================================

    [Claude Code Tool Use]
           |
           v
    +------------------+
    | PostToolUse Hook |-----> Captures context, extracts content
    +------------------+
           |
           v
    +----------------------+
    | Storage Skills       |-----> Queues for batch embedding
    | (batch-embed skill)  |
    +----------------------+
           |
           v
    +------------------------+
    | Storage Subagent       |-----> Background 13-model embedding
    | (embedding-processor)  |
    +------------------------+
           |
           v
    +------------------+     +------------------+
    | RocksDB Primary  |<--->| 13 HNSW Indices  |
    | (atomic arrays)  |     | (per-embedder)   |
    +------------------+     +------------------+
           |
           v
    +------------------+
    | SessionEnd Hook  |-----> Consolidates, compacts, optimizes
    +------------------+
           |
           v
    +----------------------+
    | PreCompact Hook      |-----> Prioritizes memories by purpose
    +----------------------+
```

## 2. Hook-Triggered Storage

Claude Code hooks provide automatic storage triggers that capture memories without explicit user intervention.

### 2.1 PostToolUse Hook for Memory Storage

The `PostToolUse` hook fires after any tool completes, enabling automatic memory capture:

```typescript
// Hook configuration in .claude/settings.json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": {
          "tool_name": "Edit|Write|Bash"
        },
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph store-memory --source hook",
            "timeout": 5000,
            "run_in_background": true
          }
        ]
      }
    ]
  }
}
```

```rust
/// PostToolUse hook handler for automatic memory storage.
///
/// Triggers on: Edit, Write, Bash, Read (configurable)
/// Captures: tool context, file content, conversation state
pub struct PostToolUseStorageHook {
    /// Pending memories awaiting batch embedding
    pending_queue: Arc<RwLock<Vec<PendingMemory>>>,

    /// Storage batch configuration
    batch_config: BatchConfig,

    /// Embedding service client
    embedder: Arc<dyn EmbeddingService>,

    /// Primary store
    store: Arc<dyn TeleologicalArrayStore>,
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
        };

        // Add to queue (non-blocking)
        self.pending_queue.write().await.push(pending);

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
            "NotebookEdit" | "WebFetch" | "WebSearch"
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
            _ => Ok(String::new()),
        }
    }
}
```

### 2.2 SessionEnd Hook for Consolidation

The `SessionEnd` hook triggers memory consolidation when a Claude Code session ends:

```typescript
// Hook configuration for session consolidation
{
  "hooks": {
    "SessionEnd": [
      {
        "matcher": {},
        "hooks": [
          {
            "type": "command",
            "command": "npx contextgraph consolidate-session --session-id $SESSION_ID",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

```rust
/// SessionEnd hook handler for memory consolidation.
///
/// Performs end-of-session operations:
/// 1. Flush pending embeddings
/// 2. Consolidate related memories
/// 3. Update temporal indices
/// 4. Optimize hot memories
pub struct SessionEndConsolidationHook {
    store: Arc<dyn TeleologicalArrayStore>,
    index_manager: Arc<IndexManager>,
    consolidator: Arc<MemoryConsolidator>,
}

impl SessionEndConsolidationHook {
    /// Handle session end event.
    pub async fn on_session_end(&self, event: SessionEndEvent) -> Result<ConsolidationStats, HookError> {
        let mut stats = ConsolidationStats::default();

        // 1. Flush any pending embeddings
        let pending = self.flush_pending_embeddings().await?;
        stats.flushed_count = pending;

        // 2. Get session memories
        let session_memories = self.store.list_by_session(&event.session_id).await?;

        // 3. Consolidate related memories (merge similar, link related)
        let consolidated = self.consolidator.consolidate(&session_memories).await?;
        stats.consolidated_count = consolidated.merged_count;
        stats.linked_count = consolidated.link_count;

        // 4. Update temporal indices (E2, E3)
        self.update_temporal_indices(&session_memories).await?;

        // 5. Promote hot memories to faster tier
        let promoted = self.optimize_memory_tiers(&session_memories).await?;
        stats.promoted_count = promoted;

        // 6. Store consolidation metadata
        self.store_session_summary(&event.session_id, &stats).await?;

        Ok(stats)
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
}
```

### 2.3 PreCompact Hook for Memory Prioritization

The `PreCompact` hook fires before RocksDB compaction, allowing intelligent memory prioritization:

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
                self.migrate_tier(memory.id, target_tier).await?;
            }
        }

        Ok(())
    }

    /// Determine target tier based on purpose score and recency.
    fn determine_tier(&self, purpose_score: f32, memory: &TeleologicalArray) -> MemoryTier {
        let recency_factor = self.calculate_recency_factor(memory.created_at);
        let combined_score = purpose_score * 0.7 + recency_factor * 0.3;

        match combined_score {
            s if s >= self.tier_config.hot_threshold => MemoryTier::Hot,
            s if s >= self.tier_config.warm_threshold => MemoryTier::Warm,
            s if s >= self.tier_config.cold_threshold => MemoryTier::Cold,
            _ => MemoryTier::Archive,
        }
    }
}
```

### 2.4 Hook Configuration

Complete hook configuration for storage integration in `.claude/settings.json`:

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
            "command": "npx contextgraph store-memory --type edit --file \"$FILE_PATH\"",
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
            "command": "npx contextgraph store-memory --type command --cmd \"$COMMAND\"",
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
            "command": "npx contextgraph index-file --path \"$FILE_PATH\"",
            "timeout": 10000,
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
            "command": "npx contextgraph session-start --id \"$SESSION_ID\"",
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
            "command": "npx contextgraph consolidate-session --session-id \"$SESSION_ID\" --export-metrics",
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
            "command": "npx contextgraph prioritize-memories --strategy purpose-aligned",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

## 3. Storage Skills

Storage skills provide high-level operations that Claude Code can invoke for batch storage operations.

### 3.1 Batch Embedding Skill

```yaml
# .claude/skills/batch-embed.yaml
name: batch-embed
description: Batch embed multiple contents into TeleologicalArrays
triggers:
  - "batch embed"
  - "embed multiple"
  - "bulk embedding"

parameters:
  contents:
    type: array
    description: Array of content strings to embed
  priority:
    type: string
    enum: [low, normal, high, critical]
    default: normal
  wait_for_completion:
    type: boolean
    default: false

execution:
  command: npx contextgraph batch-embed
  args:
    - --priority=$priority
    - --wait=$wait_for_completion
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

impl BatchEmbedSkill {
    /// Execute batch embedding.
    ///
    /// 1. Validate inputs
    /// 2. Queue for parallel embedding
    /// 3. Await all 13 embedder outputs per content
    /// 4. Store complete arrays atomically
    pub async fn execute(&self, request: BatchEmbedRequest) -> Result<BatchEmbedResult, SkillError> {
        let contents = request.contents;
        let priority = request.priority.unwrap_or(Priority::Normal);

        // Validate batch size
        if contents.len() > self.batch_config.max_batch_size {
            return Err(SkillError::BatchTooLarge {
                max: self.batch_config.max_batch_size,
                actual: contents.len(),
            });
        }

        // Create embedding tasks for all content
        let embedding_futures: Vec<_> = contents
            .iter()
            .map(|content| self.embed_single(content, priority))
            .collect();

        // Execute all embeddings in parallel
        let arrays = futures::future::try_join_all(embedding_futures).await?;

        // Store batch atomically
        let ids = self.store.store_batch(arrays).await?;

        Ok(BatchEmbedResult {
            ids,
            embedded_count: contents.len(),
            duration_ms: request.start_time.elapsed().as_millis() as u64,
        })
    }

    /// Embed single content through all 13 embedders.
    async fn embed_single(&self, content: &str, priority: Priority) -> Result<TeleologicalArray, SkillError> {
        // Parallel embedding through all 13 models
        let embeddings = self.embedder_service.embed_all(content, priority).await?;

        Ok(TeleologicalArray {
            id: Uuid::new_v4(),
            embeddings,
            source_content: content.to_string(),
            created_at: Utc::now(),
            metadata: TeleologicalMetadata::default(),
        })
    }
}
```

### 3.2 Index Management Skill

```yaml
# .claude/skills/index-manage.yaml
name: index-manage
description: Manage HNSW indices for teleological storage
triggers:
  - "rebuild index"
  - "optimize index"
  - "index stats"

subcommands:
  rebuild:
    description: Rebuild specific or all embedder indices
    parameters:
      embedder:
        type: string
        enum: [semantic, temporal_recent, temporal_periodic, entity, causal, splade, contextual, emotional, syntactic, pragmatic, crossmodal, late_interaction, keyword, all]
        default: all
    command: npx contextgraph index rebuild --embedder=$embedder

  stats:
    description: Get index statistics
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

        // Analyze workload patterns
        let read_ratio = current_stats.read_ops as f64 /
            (current_stats.read_ops + current_stats.write_ops).max(1) as f64;

        // Calculate optimal parameters per embedder
        let mut optimizations = Vec::new();

        for embedder in Embedder::all() {
            let current = self.index_manager.index_stats(embedder).await?;

            let new_params = match workload {
                WorkloadType::ReadHeavy => HnswConfig {
                    ef_search: (current.hnsw_stats.as_ref().map(|h| h.ef_search).unwrap_or(100) * 2).min(500),
                    ..current.config()
                },
                WorkloadType::WriteHeavy => HnswConfig {
                    ef_construction: (current.hnsw_stats.as_ref().map(|h| h.ef_construction).unwrap_or(200) / 2).max(50),
                    ..current.config()
                },
                WorkloadType::Balanced => current.config(),
            };

            if new_params != current.config() {
                self.index_manager.update_config(embedder, new_params).await?;
                optimizations.push((embedder, new_params));
            }
        }

        Ok(OptimizationResult {
            workload,
            optimized_embedders: optimizations.len(),
            changes: optimizations,
        })
    }
}
```

### 3.3 Cleanup Skill

```yaml
# .claude/skills/storage-cleanup.yaml
name: storage-cleanup
description: Clean up old memories and optimize storage
triggers:
  - "cleanup storage"
  - "purge old memories"
  - "storage maintenance"

parameters:
  older_than_days:
    type: integer
    default: 90
    description: Delete memories older than this many days
  min_purpose_score:
    type: number
    default: 0.2
    description: Minimum purpose alignment score to keep
  dry_run:
    type: boolean
    default: true
    description: Preview changes without deleting

execution:
  command: npx contextgraph cleanup
  args:
    - --older-than=$older_than_days
    - --min-score=$min_purpose_score
    - --dry-run=$dry_run
  timeout: 600000
```

```rust
/// Storage cleanup skill implementation.
pub struct CleanupSkill {
    store: Arc<dyn TeleologicalArrayStore>,
    purpose_evaluator: Arc<PurposeEvaluator>,
}

impl CleanupSkill {
    /// Execute cleanup based on age and purpose score.
    pub async fn execute(&self, request: CleanupRequest) -> Result<CleanupResult, SkillError> {
        let cutoff_date = Utc::now() - Duration::days(request.older_than_days as i64);

        // Find candidates for deletion
        let candidates = self.store.list_before(cutoff_date).await?;

        let mut to_delete = Vec::new();
        let mut to_keep = Vec::new();

        for memory in candidates {
            let purpose_score = self.purpose_evaluator.alignment_score(&memory);

            if purpose_score < request.min_purpose_score {
                to_delete.push(memory.id);
            } else {
                to_keep.push((memory.id, purpose_score));
            }
        }

        if request.dry_run {
            return Ok(CleanupResult {
                would_delete: to_delete.len(),
                would_keep: to_keep.len(),
                deleted: 0,
                dry_run: true,
            });
        }

        // Actually delete
        for id in &to_delete {
            self.store.delete(*id).await?;
        }

        Ok(CleanupResult {
            would_delete: 0,
            would_keep: to_keep.len(),
            deleted: to_delete.len(),
            dry_run: false,
        })
    }
}
```

## 4. Storage Subagents

Storage subagents handle heavy-lifting operations in the background, freeing the main Claude Code agent.

### 4.1 Background Embedding Agent

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
}

impl EmbeddingProcessorAgent {
    /// Run the agent's main loop.
    pub async fn run(&self) -> Result<(), AgentError> {
        tracing::info!(agent_id = %self.id, "Embedding processor agent starting");

        while !self.shutdown.load(Ordering::SeqCst) {
            // Process batch from queue
            let batch = self.collect_batch().await;

            if batch.is_empty() {
                // No work, sleep briefly
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Process batch through all 13 embedders
            match self.process_batch(batch).await {
                Ok(arrays) => {
                    // Store completed arrays
                    if let Err(e) = self.store.store_batch(arrays).await {
                        tracing::error!(error = %e, "Failed to store embedded arrays");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Batch embedding failed");
                }
            }
        }

        tracing::info!(agent_id = %self.id, "Embedding processor agent shutdown");
        Ok(())
    }

    /// Collect a batch from the pending queue.
    async fn collect_batch(&self) -> Vec<PendingMemory> {
        let mut queue = self.pending_queue.write().await;
        let count = queue.len().min(self.config.batch_size);
        queue.drain(..count).collect()
    }

    /// Process batch through all 13 embedders.
    async fn process_batch(&self, batch: Vec<PendingMemory>) -> Result<Vec<TeleologicalArray>, AgentError> {
        let futures: Vec<_> = batch
            .into_iter()
            .map(|pending| async move {
                let embeddings = self.embedder.embed_all(&pending.content, pending.priority).await?;

                Ok::<_, AgentError>(TeleologicalArray {
                    id: pending.id,
                    embeddings,
                    source_content: pending.content,
                    created_at: pending.source.triggered_at(),
                    metadata: TeleologicalMetadata::from_context(pending.context),
                })
            })
            .collect();

        futures::future::try_join_all(futures).await
    }
}
```

### 4.2 Index Optimization Agent

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

impl IndexOptimizationAgent {
    /// Run the optimization loop.
    pub async fn run(&self) -> Result<(), AgentError> {
        tracing::info!(agent_id = %self.id, "Index optimization agent starting");

        while !self.shutdown.load(Ordering::SeqCst) {
            // Collect stats
            let stats = self.stats_collector.collect_all().await?;

            // Analyze each embedder's index
            for embedder in Embedder::all() {
                let idx_stats = &stats[embedder as usize];

                // Check if optimization needed
                if self.needs_optimization(idx_stats) {
                    self.optimize_embedder(embedder, idx_stats).await?;
                }
            }

            // Sleep until next check
            tokio::time::sleep(self.config.check_interval).await;
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
            let ef_ratio = hnsw.ef_search as f64 / hnsw.max_connections as f64;
            if ef_ratio < self.config.min_ef_ratio {
                return true;
            }
        }

        false
    }

    /// Optimize a specific embedder's index.
    async fn optimize_embedder(&self, embedder: Embedder, stats: &IndexStats) -> Result<(), AgentError> {
        tracing::info!(embedder = ?embedder, "Optimizing index");

        // Determine optimization strategy
        let new_config = if stats.p99_search_us > self.config.max_p99_latency_us {
            // Latency too high: increase ef_search for better recall
            HnswConfig {
                ef_search: (stats.hnsw_stats.as_ref()
                    .map(|h| h.ef_search)
                    .unwrap_or(100) as f64 * 1.5) as usize,
                ..self.index_manager.config(embedder)
            }
        } else {
            // Standard optimization
            self.calculate_optimal_config(embedder, stats)
        };

        self.index_manager.update_config(embedder, new_config).await?;

        Ok(())
    }
}
```

### 4.3 Tier Migration Agent

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

impl TierMigrationAgent {
    /// Run the tier migration loop.
    pub async fn run(&self) -> Result<(), AgentError> {
        tracing::info!(agent_id = %self.id, "Tier migration agent starting");

        while !self.shutdown.load(Ordering::SeqCst) {
            // Get candidates for migration
            let candidates = self.find_migration_candidates().await?;

            for (memory_id, current_tier, target_tier) in candidates {
                if current_tier != target_tier {
                    self.migrate(memory_id, target_tier).await?;
                }
            }

            // Sleep until next check
            tokio::time::sleep(self.config.check_interval).await;
        }

        Ok(())
    }

    /// Find memories that should be migrated.
    async fn find_migration_candidates(&self) -> Result<Vec<(Uuid, MemoryTier, MemoryTier)>, AgentError> {
        let mut candidates = Vec::new();

        // Check hot tier for demotion candidates
        let hot_memories = self.store.list_by_tier(MemoryTier::Hot).await?;
        for memory in hot_memories {
            let access_score = self.access_tracker.score(memory.id);
            let purpose_score = self.purpose_evaluator.alignment_score(&memory);

            let target = self.calculate_target_tier(access_score, purpose_score);
            if target != MemoryTier::Hot {
                candidates.push((memory.id, MemoryTier::Hot, target));
            }
        }

        // Check warm/cold tiers for promotion candidates
        for tier in [MemoryTier::Warm, MemoryTier::Cold] {
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

        Ok(candidates)
    }

    /// Calculate target tier based on access and purpose scores.
    fn calculate_target_tier(&self, access_score: f32, purpose_score: f32) -> MemoryTier {
        let combined = access_score * 0.4 + purpose_score * 0.6;

        match combined {
            s if s >= self.config.hot_threshold => MemoryTier::Hot,
            s if s >= self.config.warm_threshold => MemoryTier::Warm,
            s if s >= self.config.cold_threshold => MemoryTier::Cold,
            _ => MemoryTier::Archive,
        }
    }
}
```

### 4.4 Subagent Spawning Configuration

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
      max_concurrent: 4
      priority_order: [critical, high, normal, low]
    resources:
      memory_limit: 2GB
      cpu_limit: 2

  index-optimizer:
    type: background
    description: Continuously monitors and optimizes HNSW indices
    spawn_on:
      - session_start
      - schedule: "*/15 * * * *"  # Every 15 minutes
    config:
      check_interval_seconds: 300
      max_p99_latency_us: 5000
      min_ef_ratio: 3.0
    resources:
      memory_limit: 512MB
      cpu_limit: 1

  tier-migrator:
    type: background
    description: Migrates memories between storage tiers based on access patterns
    spawn_on:
      - schedule: "0 * * * *"  # Every hour
    config:
      check_interval_seconds: 3600
      hot_threshold: 0.8
      warm_threshold: 0.5
      cold_threshold: 0.2
    resources:
      memory_limit: 256MB
      cpu_limit: 0.5
```

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
#[derive(Clone, Debug)]
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

    /// Configuration for medium-dim embedders (E2-E5, E7-E10: 256-768D).
    pub fn for_medium_dim() -> Self {
        Self {
            m: 24,
            ef_construction: 200,
            ef_search: 100,
            metric: DistanceMetric::Cosine,
            memory_tier: MemoryTier::Warm,
        }
    }

    /// Configuration for cross-modal embedder (E11: 768D).
    pub fn for_cross_modal() -> Self {
        Self {
            m: 24,
            ef_construction: 200,
            ef_search: 100,
            metric: DistanceMetric::Cosine,
            memory_tier: MemoryTier::Warm,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
    /// For E5 Causal: asymmetric distance
    AsymmetricCosine,
}

#[derive(Clone, Copy, Debug)]
pub enum MemoryTier {
    /// Kept in memory, fastest access.
    Hot,
    /// Memory-mapped, moderate access.
    Warm,
    /// Disk-resident, slowest access.
    Cold,
    /// Compressed archive storage.
    Archive,
}
```

### 6.2 Index Type Selection by Embedder

| Embedder | Index Type | Dimensions | HNSW M | ef_search | Distance | Tier |
|----------|------------|------------|--------|-----------|----------|------|
| E1 Semantic | HNSW | 1024D | 32 | 128 | Cosine | Hot |
| E2 TemporalRecent | HNSW | 512D | 24 | 100 | Cosine | Warm |
| E3 TemporalPeriodic | HNSW | 512D | 24 | 100 | Cosine | Warm |
| E4 EntityRelationship | HNSW | 768D | 24 | 100 | Cosine | Warm |
| E5 Causal | HNSW | 512D | 24 | 100 | Asymmetric | Warm |
| E6 SpladePrimary | Inverted | ~30K sparse | N/A | N/A | Sparse Dot | Hot |
| E7 Contextual | HNSW | 1024D | 32 | 128 | Cosine | Warm |
| E8 Emotional | HNSW | 256D | 16 | 64 | Cosine | Cold |
| E9 Syntactic | HNSW | 512D | 24 | 100 | Cosine | Cold |
| E10 Pragmatic | HNSW | 512D | 24 | 100 | Cosine | Warm |
| E11 CrossModal | HNSW | 768D | 24 | 100 | Cosine | Warm |
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
  |   +-- e04_entity/            # E4: HNSW index (768D)
  |   +-- e05_causal/            # E5: HNSW index (512D, asymmetric)
  |   +-- e06_splade/            # E6: Inverted index
  |   |   +-- postings.bin       # Posting lists
  |   |   +-- vocab.bin          # Token vocabulary
  |   |
  |   +-- e07_contextual/        # E7: HNSW index (1024D)
  |   +-- e08_emotional/         # E8: HNSW index (256D)
  |   +-- e09_syntactic/         # E9: HNSW index (512D)
  |   +-- e10_pragmatic/         # E10: HNSW index (512D)
  |   +-- e11_crossmodal/        # E11: HNSW index (768D)
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
            .filter_map(|(i, r)| r.as_ref().err().map(|e| (i, e)))
            .collect();

        if !errors.is_empty() {
            // Rollback: remove from indices that succeeded
            self.rollback_add(internal_id, &errors).await;
            return Err(StorageError::IndexUpdateFailed {
                embedders: errors.iter().map(|(i, _)| Embedder::all()[*i]).collect(),
            });
        }

        // Dispatch hook on successful index update
        self.hook_dispatcher.dispatch(HookEvent::PostIndexUpdate {
            array_id: id,
            embedders_updated: 13,
        }).await;

        Ok(())
    }

    /// Search a single embedder's index.
    ///
    /// Returns internal IDs and similarity scores.
    pub async fn search_embedder(
        &self,
        embedder: Embedder,
        query: &EmbedderOutput,
        top_k: usize,
    ) -> Result<Vec<(u64, f32)>, StorageError> {
        let idx = embedder as usize;
        self.indices[idx].search(query, top_k).await
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
    async fn rebuild(&self, embeddings: &[(u64, EmbedderOutput)]) -> Result<(), IndexError>;

    /// Get index statistics.
    fn stats(&self) -> IndexStats;

    /// Get the embedder type this index serves.
    fn embedder(&self) -> Embedder;
}
```

## 7. RocksDB Implementation

### 7.1 Column Families

```rust
/// RocksDB column families for teleological storage.
pub struct TeleologicalColumnFamilies {
    /// Primary array storage (key: UUID bytes, value: serialized TeleologicalArray).
    cf_arrays: ColumnFamily,

    /// Array metadata (key: UUID bytes, value: ArrayMetadata).
    cf_metadata: ColumnFamily,

    /// UUID to internal ID mapping (key: UUID bytes, value: u64 LE).
    cf_id_map: ColumnFamily,

    /// Internal ID to UUID reverse mapping (key: u64 LE, value: UUID bytes).
    cf_reverse_id_map: ColumnFamily,

    /// Soft-deleted arrays (key: UUID bytes, value: deletion timestamp).
    cf_tombstones: ColumnFamily,

    /// Session to array ID mapping (key: session_id, value: array IDs).
    cf_sessions: ColumnFamily,

    /// Tier to array ID mapping (key: tier, value: array IDs).
    cf_tiers: ColumnFamily,
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
    /// Store a teleological array atomically.
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

    /// Store multiple arrays in a batch (optimized for autonomous agents).
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
}
```

## 8. Batch Operations for Autonomous Agents

### 8.1 Batch Store Optimization

Autonomous agents often need to store many arrays at once (e.g., after processing a document or conversation). Batch operations are optimized for this use case.

```rust
/// Batch operation configuration for autonomous agents.
#[derive(Clone, Debug)]
pub struct BatchConfig {
    /// Maximum batch size before auto-flush.
    pub max_batch_size: usize,

    /// Maximum memory usage before auto-flush (bytes).
    pub max_batch_memory: usize,

    /// Flush interval (ms).
    pub flush_interval_ms: u64,

    /// Enable parallel index updates.
    pub parallel_index_updates: bool,

    /// Number of parallel index update workers.
    pub index_workers: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            max_batch_memory: 256 * 1024 * 1024, // 256MB
            flush_interval_ms: 1000,
            parallel_index_updates: true,
            index_workers: 4,
        }
    }
}

/// Batch writer for high-throughput autonomous agent operations.
pub struct BatchWriter {
    store: Arc<dyn TeleologicalArrayStore>,
    config: BatchConfig,
    pending: RwLock<Vec<TeleologicalArray>>,
    pending_memory: AtomicUsize,
    last_flush: RwLock<Instant>,
}

impl BatchWriter {
    /// Add an array to the batch.
    ///
    /// May trigger auto-flush if batch limits are reached.
    pub async fn add(&self, array: TeleologicalArray) -> Result<(), StorageError> {
        let array_size = estimate_array_size(&array);

        // Check if we need to flush
        if self.should_flush(array_size) {
            self.flush().await?;
        }

        // Add to pending batch
        self.pending.write().await.push(array);
        self.pending_memory.fetch_add(array_size, Ordering::SeqCst);

        Ok(())
    }

    /// Flush all pending arrays to storage.
    pub async fn flush(&self) -> Result<Vec<Uuid>, StorageError> {
        let arrays = std::mem::take(&mut *self.pending.write().await);
        self.pending_memory.store(0, Ordering::SeqCst);
        *self.last_flush.write().await = Instant::now();

        if arrays.is_empty() {
            return Ok(Vec::new());
        }

        self.store.store_batch(arrays).await
    }

    fn should_flush(&self, additional_size: usize) -> bool {
        let current_size = self.pending_memory.load(Ordering::SeqCst);
        let current_count = self.pending.blocking_read().len();

        current_count >= self.config.max_batch_size
            || current_size + additional_size > self.config.max_batch_memory
    }
}
```

### 8.2 Batch Search for Retrieval

```rust
/// Batch search across multiple queries.
///
/// Optimized for autonomous agents that need to search for
/// multiple items simultaneously.
pub async fn search_batch(
    store: &dyn IndexedTeleologicalStore,
    queries: Vec<SearchQuery>,
    options: BatchSearchOptions,
) -> Result<Vec<Vec<SearchResult>>, StorageError> {
    if queries.is_empty() {
        return Ok(Vec::new());
    }

    // Group queries by embedder for efficient batching
    let mut by_embedder: HashMap<Embedder, Vec<(usize, &EmbedderOutput)>> = HashMap::new();

    for (idx, query) in queries.iter().enumerate() {
        by_embedder
            .entry(query.embedder)
            .or_default()
            .push((idx, &query.embedding));
    }

    // Search each embedder's batch in parallel
    let futures: Vec<_> = by_embedder.into_iter()
        .map(|(embedder, batch)| async move {
            let results = store.search_embedder_batch(embedder, &batch, options.top_k).await?;
            Ok::<_, StorageError>((batch.iter().map(|(i, _)| *i).collect::<Vec<_>>(), results))
        })
        .collect();

    let batch_results = futures::future::try_join_all(futures).await?;

    // Reorganize results by original query order
    let mut results = vec![Vec::new(); queries.len()];
    for (indices, embedder_results) in batch_results {
        for (i, idx) in indices.into_iter().enumerate() {
            results[idx] = embedder_results[i].clone();
        }
    }

    Ok(results)
}
```

## 9. Serialization

### 9.1 Array Serialization

```rust
/// Serialization format for teleological arrays.
#[derive(Clone, Copy, Debug)]
pub enum SerializationFormat {
    /// MessagePack - compact and fast (default).
    MessagePack,

    /// Bincode - fastest but less portable.
    Bincode,

    /// CBOR - good interoperability.
    Cbor,
}

pub struct ArraySerializer {
    format: SerializationFormat,
    compressor: Option<EmbeddingCompressor>,
}

impl ArraySerializer {
    /// Serialize a complete TeleologicalArray.
    ///
    /// The array is serialized as a single atomic unit:
    /// - Header: version, embedder count (13), metadata
    /// - Body: all 13 embedder outputs in order
    pub fn serialize(&self, array: &TeleologicalArray) -> Result<Vec<u8>, SerializationError> {
        let serializable = if let Some(ref compressor) = self.compressor {
            SerializableTeleologicalArray::compressed(array, compressor)
        } else {
            SerializableTeleologicalArray::from(array)
        };

        match self.format {
            SerializationFormat::MessagePack => {
                rmp_serde::to_vec(&serializable).map_err(Into::into)
            }
            SerializationFormat::Bincode => {
                bincode::serialize(&serializable).map_err(Into::into)
            }
            SerializationFormat::Cbor => {
                let mut buf = Vec::new();
                ciborium::into_writer(&serializable, &mut buf)?;
                Ok(buf)
            }
        }
    }

    pub fn deserialize(&self, bytes: &[u8]) -> Result<TeleologicalArray, SerializationError> {
        let serializable: SerializableTeleologicalArray = match self.format {
            SerializationFormat::MessagePack => {
                rmp_serde::from_slice(bytes)?
            }
            SerializationFormat::Bincode => {
                bincode::deserialize(bytes)?
            }
            SerializationFormat::Cbor => {
                ciborium::from_reader(bytes)?
            }
        };

        if let Some(ref compressor) = self.compressor {
            serializable.decompress(compressor)
        } else {
            Ok(serializable.into())
        }
    }
}
```

### 9.2 Embedding Compression

```rust
/// Compression for embeddings to reduce storage size.
///
/// Storage targets (per the PRD):
/// - Quantized: ~17KB per memory
/// - Uncompressed: ~46KB per memory
/// - 63% reduction via PQ-8/Float8/Binary
pub struct EmbeddingCompressor {
    /// Per-embedder quantization settings.
    quantization: [QuantizationLevel; 13],

    /// Product quantization codebooks (for PQ-8).
    pq_codebooks: HashMap<Embedder, ProductQuantizer>,
}

#[derive(Clone, Copy, Debug)]
pub enum QuantizationLevel {
    /// Full precision (f32) - 4 bytes per dimension.
    None,

    /// Half precision (f16) - 2 bytes per dimension.
    Half,

    /// 8-bit quantization - 1 byte per dimension.
    Int8,

    /// Product quantization with 8 subquantizers.
    /// ~1 byte per 8 dimensions.
    ProductQuantization8,

    /// Binary (for E9 HDC) - 1 bit per dimension.
    Binary,
}

impl EmbeddingCompressor {
    /// Create compressor with PRD-recommended settings.
    pub fn from_prd() -> Self {
        Self {
            quantization: [
                QuantizationLevel::ProductQuantization8, // E1 Semantic
                QuantizationLevel::Half,                 // E2 TemporalRecent
                QuantizationLevel::Half,                 // E3 TemporalPeriodic
                QuantizationLevel::Half,                 // E4 EntityRelationship
                QuantizationLevel::ProductQuantization8, // E5 Causal
                QuantizationLevel::None,                 // E6 SpladePrimary (sparse)
                QuantizationLevel::ProductQuantization8, // E7 Contextual
                QuantizationLevel::Half,                 // E8 Emotional
                QuantizationLevel::Half,                 // E9 Syntactic
                QuantizationLevel::Half,                 // E10 Pragmatic
                QuantizationLevel::ProductQuantization8, // E11 CrossModal
                QuantizationLevel::None,                 // E12 LateInteraction (token-level)
                QuantizationLevel::None,                 // E13 SpladeKeyword (sparse)
            ],
            pq_codebooks: HashMap::new(),
        }
    }

    /// Compress a dense embedding.
    pub fn compress_dense(&self, embedder: Embedder, embedding: &[f32]) -> CompressedEmbedding {
        match self.quantization[embedder as usize] {
            QuantizationLevel::None => {
                CompressedEmbedding::F32(embedding.to_vec())
            }
            QuantizationLevel::Half => {
                let f16s: Vec<f16> = embedding.iter().map(|&f| f16::from_f32(f)).collect();
                CompressedEmbedding::F16(f16s)
            }
            QuantizationLevel::Int8 => {
                let (scale, zero_point, data) = quantize_int8(embedding);
                CompressedEmbedding::Int8 { scale, zero_point, data }
            }
            QuantizationLevel::ProductQuantization8 => {
                if let Some(pq) = self.pq_codebooks.get(&embedder) {
                    let codes = pq.encode(embedding);
                    CompressedEmbedding::PQ8 { codes }
                } else {
                    // Fallback to f16 if no codebook
                    let f16s: Vec<f16> = embedding.iter().map(|&f| f16::from_f32(f)).collect();
                    CompressedEmbedding::F16(f16s)
                }
            }
            QuantizationLevel::Binary => {
                let bits = quantize_binary(embedding);
                CompressedEmbedding::Binary(bits)
            }
        }
    }
}
```

## 10. Statistics and Monitoring

```rust
#[derive(Clone, Debug, Serialize)]
pub struct StorageStats {
    /// Total number of arrays stored.
    pub total_arrays: usize,

    /// Number of soft-deleted arrays.
    pub tombstone_count: usize,

    /// Total storage size in bytes.
    pub storage_bytes: u64,

    /// Per-embedder index statistics.
    pub index_stats: [IndexStats; 13],

    /// Average array size in bytes.
    pub avg_array_bytes: usize,

    /// Last compaction timestamp.
    pub last_compaction: Option<DateTime<Utc>>,

    /// Cache hit rate (0.0 - 1.0).
    pub cache_hit_rate: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexStats {
    /// Which embedder this index serves.
    pub embedder: Embedder,

    /// Number of vectors indexed.
    pub vector_count: usize,

    /// Index size in bytes.
    pub index_bytes: u64,

    /// Index type (HNSW, Inverted, TokenLevel).
    pub index_type: IndexType,

    /// HNSW-specific stats (if applicable).
    pub hnsw_stats: Option<HnswStats>,

    /// Average search latency in microseconds.
    pub avg_search_us: u64,

    /// P99 search latency in microseconds.
    pub p99_search_us: u64,

    /// Last rebuild timestamp.
    pub last_rebuild: Option<DateTime<Utc>>,

    /// Memory tier.
    pub memory_tier: MemoryTier,
}

#[derive(Clone, Debug, Serialize)]
pub struct HnswStats {
    /// Number of graph layers.
    pub num_layers: usize,

    /// Entry point ID.
    pub entry_point: u64,

    /// Average connections per node.
    pub avg_connections: f32,

    /// Maximum connections (M parameter).
    pub max_connections: usize,

    /// ef_construction used.
    pub ef_construction: usize,

    /// ef_search current setting.
    pub ef_search: usize,
}
```

## 11. Migration from Current System

### 11.1 Current State Assessment

The existing `TeleologicalFingerprint` is close to the target `TeleologicalArray`. Key differences:

| Current | Target | Action |
|---------|--------|--------|
| `TeleologicalFingerprint` | `TeleologicalArray` | Rename type |
| `SemanticFingerprint` (nested) | `embeddings: [EmbedderOutput; 13]` | Flatten structure |
| `theta_to_north_star: f32` | **REMOVED** | Delete field (broken) |
| `purpose_vector: PurposeVector` | Keep as-is | No change |
| `johari_fingerprint: JohariFingerprint` | Keep as-is | No change |

### 11.2 Migration Steps

1. **Phase 1: Introduce New Types**
   - Create `TeleologicalArray` alongside existing `TeleologicalFingerprint`
   - Create adapter functions for conversion
   - Update storage interfaces to accept both

2. **Phase 2: Migrate Storage Layer**
   - Implement new `TeleologicalArrayStore` trait
   - Create 13 separate HNSW indices
   - Migrate existing data with conversion

3. **Phase 3: Migrate Search Layer**
   - Update all search functions to use `ComparisonType`
   - Remove any cross-embedder comparison code
   - Implement RRF aggregation for multi-embedder search

4. **Phase 4: Remove Legacy**
   - Remove `theta_to_north_star` field
   - Remove broken North Star alignment code
   - Remove `TeleologicalFingerprint` type

### 11.3 Data Migration Script

```rust
/// Migrate existing fingerprints to teleological arrays.
pub async fn migrate_fingerprints(
    old_store: &dyn TeleologicalMemoryStore,
    new_store: &dyn TeleologicalArrayStore,
    batch_size: usize,
) -> Result<MigrationStats, MigrationError> {
    let mut stats = MigrationStats::default();
    let mut offset = 0;

    loop {
        // Retrieve batch of old fingerprints
        let fingerprints = old_store.list(offset, batch_size).await?;
        if fingerprints.is_empty() {
            break;
        }

        // Convert to arrays
        let arrays: Vec<TeleologicalArray> = fingerprints
            .into_iter()
            .map(convert_fingerprint_to_array)
            .collect();

        // Store batch in new format
        new_store.store_batch(arrays).await?;

        stats.migrated_count += batch_size;
        offset += batch_size;

        tracing::info!("Migrated {} fingerprints", stats.migrated_count);
    }

    Ok(stats)
}

/// Convert a TeleologicalFingerprint to a TeleologicalArray.
fn convert_fingerprint_to_array(fp: TeleologicalFingerprint) -> TeleologicalArray {
    TeleologicalArray {
        id: fp.id,
        embeddings: [
            EmbedderOutput::Dense(fp.semantic.e1_semantic.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e2_temporal_recent.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e3_temporal_periodic.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e4_entity_relationship.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e5_causal.to_vec()),
            EmbedderOutput::Sparse(convert_sparse(&fp.semantic.e6_splade)),
            EmbedderOutput::Dense(fp.semantic.e7_contextual.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e8_emotional.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e9_syntactic.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e10_pragmatic.to_vec()),
            EmbedderOutput::Dense(fp.semantic.e11_cross_modal.to_vec()),
            EmbedderOutput::TokenLevel(convert_late_interaction(&fp.semantic.e12_late_interaction)),
            EmbedderOutput::Sparse(convert_sparse(&fp.semantic.e13_splade_keyword)),
        ],
        source_content: fp.original_content,
        created_at: fp.created_at,
        metadata: TeleologicalMetadata::from(fp.metadata),
    }
}
```

## 12. Performance Targets

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

## 13. References

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

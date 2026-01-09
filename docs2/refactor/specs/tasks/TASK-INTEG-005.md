# TASK-INTEG-005: Edit Hooks Implementation

```xml
<task_spec>
  <task_id>TASK-INTEG-005</task_id>
  <title>Edit Hooks Implementation (PreEdit, PostEdit with Pattern Training)</title>
  <status>pending</status>

  <objective>
    Implement PreFileWrite and PostFileWrite hook handlers that integrate with the
    teleological array system for change tracking, alignment validation, and
    pattern learning from code edits.
  </objective>

  <rationale>
    Edit hooks are critical for:
    1. PreFileWrite: Validate changes against goal alignment before writing
    2. PostFileWrite: Track changes for learning and drift detection
    3. Pattern training: Learn from edit patterns to improve future suggestions
    4. Change embedding: Store edit contexts as teleological arrays for retrieval

    These hooks enable continuous learning from developer actions within Claude Code.
  </rationale>

  <dependencies>
    <dependency type="required">TASK-INTEG-004</dependency>    <!-- Hook protocol -->
    <dependency type="required">TASK-CORE-003</dependency>     <!-- TeleologicalArray type -->
    <dependency type="required">TASK-LOGIC-006</dependency>    <!-- Trajectory tracking -->
    <dependency type="required">TASK-LOGIC-010</dependency>    <!-- Drift detection -->
  </dependencies>

  <input_context_files>
    <file purpose="hook_protocol">crates/context-graph-mcp/src/hooks/protocol.rs</file>
    <file purpose="trajectory_tracking">crates/context-graph-core/src/autonomous/trajectory.rs</file>
    <file purpose="alignment_calculator">crates/context-graph-core/src/alignment/calculator.rs</file>
    <file purpose="drift_detector">crates/context-graph-core/src/autonomous/drift.rs</file>
    <file purpose="embedder_pipeline">crates/context-graph-core/src/teleology/embedder.rs</file>
  </input_context_files>

  <output_artifacts>
    <artifact type="source">crates/context-graph-mcp/src/hooks/edit.rs</artifact>
    <artifact type="source">crates/context-graph-mcp/src/hooks/file_ops.rs</artifact>
    <artifact type="config">.claude/hooks/pre_file_write.sh</artifact>
    <artifact type="config">.claude/hooks/post_file_write.sh</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/edit_hooks_test.rs</artifact>
  </output_artifacts>

  <definition_of_done>
    <criterion id="1">PreFileWrite handler validates alignment before allowing writes</criterion>
    <criterion id="2">PostFileWrite handler stores edits as teleological arrays</criterion>
    <criterion id="3">Pattern training extracts learnable patterns from edits</criterion>
    <criterion id="4">Edit contexts include before/after content for diff analysis</criterion>
    <criterion id="5">Alignment warnings emitted when writes may cause drift</criterion>
    <criterion id="6">Hook latency under 100ms for pre-write validation</criterion>
    <criterion id="7">Pattern training runs asynchronously to avoid blocking</criterion>
    <criterion id="8">Shell scripts invoke MCP tool correctly with file context</criterion>
  </definition_of_done>

  <estimated_complexity>High</estimated_complexity>

  <pseudo_code>
    <section name="PreFileWriteHandler">
```rust
// crates/context-graph-mcp/src/hooks/edit.rs

use crate::hooks::protocol::{HookEvent, HookPayload, HookResponse, HookStatus};
use context_graph_core::teleology::{TeleologicalArray, TeleologicalComparator};
use context_graph_core::autonomous::drift::TeleologicalDriftDetector;
use context_graph_core::alignment::TeleologicalAlignmentCalculator;

/// Pre-file-write hook handler for alignment validation
pub struct PreFileWriteHandler {
    /// Teleological store for retrieving related content
    store: Arc<TeleologicalStore>,
    /// Alignment calculator for goal checking
    alignment_calculator: Arc<TeleologicalAlignmentCalculator>,
    /// Drift detector for change impact analysis
    drift_detector: Arc<TeleologicalDriftDetector>,
    /// Embedding pipeline for generating preview arrays
    embedder_pipeline: Arc<EmbedderPipeline>,
    /// Configuration
    config: PreFileWriteConfig,
}

#[derive(Debug, Clone)]
pub struct PreFileWriteConfig {
    /// Minimum alignment score to allow write (0.0 to 1.0)
    pub min_alignment_threshold: f32,
    /// Whether to block on low alignment (vs just warn)
    pub block_on_low_alignment: bool,
    /// Maximum latency before skipping validation (ms)
    pub max_validation_latency_ms: u64,
    /// Embedders to use for quick validation
    pub validation_embedders: Vec<EmbedderType>,
}

impl Default for PreFileWriteConfig {
    fn default() -> Self {
        Self {
            min_alignment_threshold: 0.5,
            block_on_low_alignment: false,
            max_validation_latency_ms: 100,
            // Use fast embedders for pre-write validation
            validation_embedders: vec![
                EmbedderType::Semantic,
                EmbedderType::Code,
                EmbedderType::Causal,
            ],
        }
    }
}

/// Payload for file write hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWritePayload {
    /// File path being written
    pub file_path: String,
    /// New content to be written
    pub new_content: String,
    /// Original content (if file exists)
    pub original_content: Option<String>,
    /// Operation type (create, update, append)
    pub operation: FileOperation,
    /// Tool that initiated the write (Write, Edit, MultiEdit)
    pub source_tool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Create,
    Update,
    Append,
    Delete,
}

impl PreFileWriteHandler {
    pub fn new(
        store: Arc<TeleologicalStore>,
        alignment_calculator: Arc<TeleologicalAlignmentCalculator>,
        drift_detector: Arc<TeleologicalDriftDetector>,
        embedder_pipeline: Arc<EmbedderPipeline>,
        config: PreFileWriteConfig,
    ) -> Self {
        Self {
            store,
            alignment_calculator,
            drift_detector,
            embedder_pipeline,
            config,
        }
    }

    /// Handle PreFileWrite event
    pub async fn handle(&self, payload: FileWritePayload) -> Result<HookResponse, HookError> {
        let start = Instant::now();

        // 1. Generate partial teleological array for new content (fast embedders only)
        let preview_array = self.generate_preview_array(&payload.new_content).await?;

        // 2. Check if we're running out of time
        if start.elapsed().as_millis() > (self.config.max_validation_latency_ms / 2) as u128 {
            // Skip validation if taking too long
            return Ok(HookResponse {
                status: HookStatus::Proceed,
                message: Some("Validation skipped due to time constraints".to_string()),
                warnings: vec![],
                injected_context: None,
                metrics: HookMetrics::timeout(),
            });
        }

        // 3. Find active goals related to this file
        let related_goals = self.find_related_goals(&payload.file_path).await?;

        // 4. Calculate alignment against active goals
        let mut alignment_scores = Vec::new();
        let mut warnings = Vec::new();

        for goal in &related_goals {
            let alignment = self.alignment_calculator.calculate_alignment(
                &preview_array,
                &goal.teleological_array,
                ComparisonStrategy::fast_validation(),
            )?;

            alignment_scores.push((goal.goal_id.clone(), alignment.overall_score));

            if alignment.overall_score < self.config.min_alignment_threshold {
                warnings.push(AlignmentWarning {
                    goal_id: goal.goal_id.clone(),
                    goal_label: goal.label.clone(),
                    alignment_score: alignment.overall_score,
                    threshold: self.config.min_alignment_threshold,
                    drifted_embedders: alignment.low_scoring_embedders(),
                    recommendation: self.generate_recommendation(&alignment, &goal),
                });
            }
        }

        // 5. Check for potential drift if original content exists
        let drift_warnings = if let Some(ref original) = payload.original_content {
            self.check_edit_drift(&original, &payload.new_content, &preview_array).await?
        } else {
            vec![]
        };

        warnings.extend(drift_warnings.into_iter().map(|d| AlignmentWarning {
            goal_id: "drift".to_string(),
            goal_label: format!("Drift detected in {}", d.embedder),
            alignment_score: 1.0 - d.drift_magnitude,
            threshold: self.config.min_alignment_threshold,
            drifted_embedders: vec![d.embedder],
            recommendation: d.recommendation,
        }));

        // 6. Determine response status
        let status = if warnings.is_empty() {
            HookStatus::Proceed
        } else if self.config.block_on_low_alignment &&
                  warnings.iter().any(|w| w.alignment_score < 0.3) {
            HookStatus::Block
        } else {
            HookStatus::ProceedWithWarning
        };

        // 7. Build response
        Ok(HookResponse {
            status,
            message: self.build_status_message(&warnings),
            warnings: warnings.iter().map(|w| w.to_string()).collect(),
            injected_context: Some(serde_json::json!({
                "alignment_scores": alignment_scores,
                "preview_embedders": self.config.validation_embedders,
                "validation_latency_ms": start.elapsed().as_millis(),
            })),
            metrics: HookMetrics {
                latency_ms: start.elapsed().as_millis() as u64,
                embedders_used: self.config.validation_embedders.len(),
                goals_checked: related_goals.len(),
            },
        })
    }

    /// Generate preview array using fast embedders only
    async fn generate_preview_array(&self, content: &str) -> Result<TeleologicalArray, HookError> {
        self.embedder_pipeline.generate_partial(
            content,
            &self.config.validation_embedders,
        ).await.map_err(HookError::EmbeddingFailed)
    }

    /// Find goals related to the file path
    async fn find_related_goals(&self, file_path: &str) -> Result<Vec<GoalNode>, HookError> {
        // Search for goals by file path metadata
        self.store.search_goals_by_file(file_path, 5).await
            .map_err(HookError::StorageError)
    }

    /// Check for drift between original and new content
    async fn check_edit_drift(
        &self,
        original: &str,
        new_content: &str,
        new_array: &TeleologicalArray,
    ) -> Result<Vec<DriftWarning>, HookError> {
        // Generate array for original content
        let original_array = self.generate_preview_array(original).await?;

        // Analyze drift
        let drift_report = self.drift_detector.analyze_drift(
            &original_array,
            new_array,
            DriftAnalysisConfig::quick(),
        )?;

        // Return significant drifts
        Ok(drift_report.per_embedder
            .into_iter()
            .filter(|(_, d)| d.drift_level >= DriftLevel::Medium)
            .map(|(embedder, d)| DriftWarning {
                embedder,
                drift_magnitude: d.similarity,
                drift_level: d.drift_level,
                recommendation: d.recommendation,
            })
            .collect())
    }

    fn generate_recommendation(&self, alignment: &AlignmentResult, goal: &GoalNode) -> String {
        let weak_embedders: Vec<_> = alignment.per_embedder
            .iter()
            .filter(|(_, score)| *score < 0.5)
            .map(|(e, _)| e.short_name())
            .collect();

        if weak_embedders.is_empty() {
            "Changes are well-aligned with the goal.".to_string()
        } else {
            format!(
                "Consider reviewing changes for {} alignment with goal '{}'. Weak areas: {}",
                goal.level.display_name(),
                goal.label,
                weak_embedders.join(", ")
            )
        }
    }

    fn build_status_message(&self, warnings: &[AlignmentWarning]) -> Option<String> {
        if warnings.is_empty() {
            None
        } else {
            Some(format!(
                "{} alignment warning(s) detected. Review before proceeding.",
                warnings.len()
            ))
        }
    }
}

#[derive(Debug)]
struct AlignmentWarning {
    goal_id: String,
    goal_label: String,
    alignment_score: f32,
    threshold: f32,
    drifted_embedders: Vec<EmbedderType>,
    recommendation: String,
}

impl std::fmt::Display for AlignmentWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Goal '{}' alignment: {:.2} (threshold: {:.2}) - {}",
            self.goal_label, self.alignment_score, self.threshold, self.recommendation
        )
    }
}
```
    </section>

    <section name="PostFileWriteHandler">
```rust
/// Post-file-write hook handler for tracking and learning
pub struct PostFileWriteHandler {
    /// Teleological store for persisting edit memories
    store: Arc<TeleologicalStore>,
    /// Full embedding pipeline
    embedder_pipeline: Arc<EmbedderPipeline>,
    /// Trajectory tracker for learning
    trajectory_tracker: Arc<TrajectoryTracker>,
    /// Pattern extractor for edit patterns
    pattern_extractor: Arc<EditPatternExtractor>,
    /// Background task queue
    task_queue: Arc<BackgroundTaskQueue>,
    /// Configuration
    config: PostFileWriteConfig,
}

#[derive(Debug, Clone)]
pub struct PostFileWriteConfig {
    /// Whether to store all edits or only significant ones
    pub store_all_edits: bool,
    /// Minimum content length to trigger storage
    pub min_content_length: usize,
    /// File extensions to process
    pub process_extensions: HashSet<String>,
    /// Whether to run pattern training synchronously
    pub sync_pattern_training: bool,
    /// Maximum edits before triggering batch pattern analysis
    pub pattern_batch_size: usize,
}

impl Default for PostFileWriteConfig {
    fn default() -> Self {
        let mut extensions = HashSet::new();
        extensions.extend([
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java",
            "md", "json", "yaml", "yml", "toml"
        ].iter().map(|s| s.to_string()));

        Self {
            store_all_edits: true,
            min_content_length: 50,
            process_extensions: extensions,
            sync_pattern_training: false,
            pattern_batch_size: 100,
        }
    }
}

/// Edit context for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditContext {
    /// Unique ID for this edit
    pub edit_id: Uuid,
    /// File path that was edited
    pub file_path: String,
    /// Content before edit (if available)
    pub before_content: Option<String>,
    /// Content after edit
    pub after_content: String,
    /// Diff representation
    pub diff: Option<String>,
    /// Edit operation type
    pub operation: FileOperation,
    /// Source tool
    pub source_tool: String,
    /// Session ID
    pub session_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Related memories discovered during edit
    pub related_memories: Vec<Uuid>,
    /// Alignment score at time of edit
    pub alignment_snapshot: Option<f32>,
}

impl PostFileWriteHandler {
    pub fn new(
        store: Arc<TeleologicalStore>,
        embedder_pipeline: Arc<EmbedderPipeline>,
        trajectory_tracker: Arc<TrajectoryTracker>,
        pattern_extractor: Arc<EditPatternExtractor>,
        task_queue: Arc<BackgroundTaskQueue>,
        config: PostFileWriteConfig,
    ) -> Self {
        Self {
            store,
            embedder_pipeline,
            trajectory_tracker,
            pattern_extractor,
            task_queue,
            config,
        }
    }

    /// Handle PostFileWrite event
    pub async fn handle(&self, payload: FileWritePayload) -> Result<HookResponse, HookError> {
        let start = Instant::now();

        // 1. Check if we should process this file
        if !self.should_process(&payload) {
            return Ok(HookResponse::skip("File type not configured for processing"));
        }

        // 2. Generate full teleological array for new content
        let teleological_array = self.embedder_pipeline
            .generate_full(&payload.new_content)
            .await
            .map_err(HookError::EmbeddingFailed)?;

        // 3. Create edit context
        let edit_context = EditContext {
            edit_id: Uuid::new_v4(),
            file_path: payload.file_path.clone(),
            before_content: payload.original_content.clone(),
            after_content: payload.new_content.clone(),
            diff: self.compute_diff(&payload.original_content, &payload.new_content),
            operation: payload.operation.clone(),
            source_tool: payload.source_tool.clone(),
            session_id: self.get_current_session_id(),
            timestamp: Utc::now(),
            related_memories: vec![],
            alignment_snapshot: None,
        };

        // 4. Store in teleological store
        let memory_id = self.store.inject(InjectionRequest {
            content: payload.new_content.clone(),
            teleological_array,
            memory_type: MemoryType::CodeEdit,
            namespace: self.derive_namespace(&payload.file_path),
            metadata: serde_json::to_value(&edit_context)?,
        }).await.map_err(HookError::StorageError)?;

        // 5. Record in trajectory tracker
        let trajectory_step = TrajectoryStep {
            action: TrajectoryAction::FileWrite {
                file_path: payload.file_path.clone(),
                memory_id,
            },
            timestamp: Utc::now(),
            context_size: payload.new_content.len(),
            outcome: None, // Will be filled by verdict system
        };

        self.trajectory_tracker.record_step(trajectory_step).await?;

        // 6. Queue pattern training (async unless configured for sync)
        let pattern_task = PatternTrainingTask {
            edit_id: edit_context.edit_id,
            memory_id,
            before_content: payload.original_content,
            after_content: payload.new_content,
            file_path: payload.file_path.clone(),
        };

        if self.config.sync_pattern_training {
            self.train_patterns(pattern_task).await?;
        } else {
            self.task_queue.enqueue(BackgroundTask::PatternTraining(pattern_task));
        }

        // 7. Check if we should trigger batch pattern analysis
        let pending_edits = self.trajectory_tracker.pending_edit_count().await;
        if pending_edits >= self.config.pattern_batch_size {
            self.task_queue.enqueue(BackgroundTask::BatchPatternAnalysis {
                session_id: self.get_current_session_id(),
            });
        }

        Ok(HookResponse {
            status: HookStatus::Success,
            message: Some(format!("Edit stored as memory {}", memory_id)),
            warnings: vec![],
            injected_context: Some(serde_json::json!({
                "memory_id": memory_id,
                "edit_id": edit_context.edit_id,
                "embedders_generated": 13,
                "pattern_training_queued": !self.config.sync_pattern_training,
            })),
            metrics: HookMetrics {
                latency_ms: start.elapsed().as_millis() as u64,
                embedders_used: 13,
                goals_checked: 0,
            },
        })
    }

    fn should_process(&self, payload: &FileWritePayload) -> bool {
        // Check content length
        if payload.new_content.len() < self.config.min_content_length {
            return false;
        }

        // Check file extension
        let extension = std::path::Path::new(&payload.file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        self.config.process_extensions.contains(extension)
    }

    fn compute_diff(&self, before: &Option<String>, after: &str) -> Option<String> {
        before.as_ref().map(|b| {
            // Generate unified diff
            let diff = similar::TextDiff::from_lines(b, after);
            diff.unified_diff()
                .header("before", "after")
                .to_string()
        })
    }

    fn derive_namespace(&self, file_path: &str) -> String {
        // Extract namespace from file path structure
        let path = std::path::Path::new(file_path);

        // Use parent directory as namespace
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string()
    }

    fn get_current_session_id(&self) -> String {
        // Get from session context
        SESSION_CONTEXT.with(|ctx| ctx.borrow().session_id.clone())
            .unwrap_or_else(|| format!("session-{}", Utc::now().format("%Y%m%d")))
    }

    /// Train patterns from edit
    async fn train_patterns(&self, task: PatternTrainingTask) -> Result<(), HookError> {
        // Extract patterns from the edit
        let patterns = self.pattern_extractor.extract_patterns(
            task.before_content.as_deref(),
            &task.after_content,
            &task.file_path,
        )?;

        // Store patterns for learning
        for pattern in patterns {
            self.store.store_pattern(pattern).await?;
        }

        Ok(())
    }
}

/// Pattern extractor for learning from edits
pub struct EditPatternExtractor {
    /// AST parser for code analysis
    ast_parser: Arc<AstParser>,
    /// Pattern matcher for common edit types
    pattern_matcher: PatternMatcher,
}

impl EditPatternExtractor {
    /// Extract learnable patterns from an edit
    pub fn extract_patterns(
        &self,
        before: Option<&str>,
        after: &str,
        file_path: &str,
    ) -> Result<Vec<EditPattern>, PatternError> {
        let mut patterns = Vec::new();

        // 1. Detect edit type
        let edit_type = self.classify_edit(before, after);

        // 2. Extract structural patterns (for code files)
        if let Some(lang) = self.detect_language(file_path) {
            let structural = self.extract_structural_patterns(before, after, lang)?;
            patterns.extend(structural);
        }

        // 3. Extract semantic patterns
        let semantic = self.extract_semantic_patterns(before, after)?;
        patterns.extend(semantic);

        // 4. Extract refactoring patterns
        if let Some(before_content) = before {
            let refactoring = self.extract_refactoring_patterns(before_content, after)?;
            patterns.extend(refactoring);
        }

        Ok(patterns)
    }

    fn classify_edit(&self, before: Option<&str>, after: &str) -> EditType {
        match before {
            None => EditType::Creation,
            Some(b) if b.is_empty() => EditType::Creation,
            Some(b) if after.is_empty() => EditType::Deletion,
            Some(b) => {
                let similarity = strsim::jaro_winkler(b, after) as f32;
                if similarity > 0.9 {
                    EditType::MinorModification
                } else if similarity > 0.5 {
                    EditType::MajorModification
                } else {
                    EditType::Rewrite
                }
            }
        }
    }

    fn detect_language(&self, file_path: &str) -> Option<Language> {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())?;

        match ext {
            "rs" => Some(Language::Rust),
            "ts" | "tsx" => Some(Language::TypeScript),
            "js" | "jsx" => Some(Language::JavaScript),
            "py" => Some(Language::Python),
            "go" => Some(Language::Go),
            _ => None,
        }
    }

    fn extract_structural_patterns(
        &self,
        before: Option<&str>,
        after: &str,
        lang: Language,
    ) -> Result<Vec<EditPattern>, PatternError> {
        // Parse ASTs
        let after_ast = self.ast_parser.parse(after, lang)?;

        let mut patterns = Vec::new();

        // Detect function/method changes
        if let Some(functions) = after_ast.find_functions() {
            for func in functions {
                patterns.push(EditPattern::FunctionDefinition {
                    name: func.name.clone(),
                    signature_hash: func.signature_hash(),
                    language: lang,
                });
            }
        }

        // Detect import changes
        if let Some(imports) = after_ast.find_imports() {
            for import in imports {
                patterns.push(EditPattern::ImportAddition {
                    module: import.module.clone(),
                    items: import.items.clone(),
                    language: lang,
                });
            }
        }

        Ok(patterns)
    }

    fn extract_semantic_patterns(
        &self,
        before: Option<&str>,
        after: &str,
    ) -> Result<Vec<EditPattern>, PatternError> {
        let mut patterns = Vec::new();

        // Detect common patterns
        if self.pattern_matcher.is_error_handling_addition(before, after) {
            patterns.push(EditPattern::ErrorHandling);
        }

        if self.pattern_matcher.is_logging_addition(before, after) {
            patterns.push(EditPattern::LoggingAddition);
        }

        if self.pattern_matcher.is_test_addition(before, after) {
            patterns.push(EditPattern::TestAddition);
        }

        if self.pattern_matcher.is_documentation_addition(before, after) {
            patterns.push(EditPattern::DocumentationAddition);
        }

        Ok(patterns)
    }

    fn extract_refactoring_patterns(
        &self,
        before: &str,
        after: &str,
    ) -> Result<Vec<EditPattern>, PatternError> {
        let mut patterns = Vec::new();

        // Detect rename refactoring
        if let Some(rename) = self.pattern_matcher.detect_rename(before, after) {
            patterns.push(EditPattern::Rename {
                old_name: rename.old_name,
                new_name: rename.new_name,
            });
        }

        // Detect extract refactoring
        if let Some(extract) = self.pattern_matcher.detect_extraction(before, after) {
            patterns.push(EditPattern::ExtractFunction {
                extracted_name: extract.new_function_name,
                source_lines: extract.line_count,
            });
        }

        Ok(patterns)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditPattern {
    FunctionDefinition {
        name: String,
        signature_hash: String,
        language: Language,
    },
    ImportAddition {
        module: String,
        items: Vec<String>,
        language: Language,
    },
    ErrorHandling,
    LoggingAddition,
    TestAddition,
    DocumentationAddition,
    Rename {
        old_name: String,
        new_name: String,
    },
    ExtractFunction {
        extracted_name: String,
        source_lines: usize,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EditType {
    Creation,
    Deletion,
    MinorModification,
    MajorModification,
    Rewrite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Java,
}
```
    </section>

    <section name="ShellScripts">
```bash
#!/bin/bash
# .claude/hooks/pre_file_write.sh
# PreFileWrite hook - validates alignment before file writes

set -e

# Read input from stdin (JSON payload)
INPUT=$(cat)

# Extract file path and content
FILE_PATH=$(echo "$INPUT" | jq -r '.file_path')
NEW_CONTENT=$(echo "$INPUT" | jq -r '.new_content')
ORIGINAL_CONTENT=$(echo "$INPUT" | jq -r '.original_content // empty')

# Call MCP tool for validation
RESPONSE=$(echo '{
  "jsonrpc": "2.0",
  "id": "pre-file-write-'$(date +%s)'",
  "method": "hooks/pre_file_write",
  "params": {
    "file_path": "'"$FILE_PATH"'",
    "new_content": '"$(echo "$NEW_CONTENT" | jq -Rs .)"',
    "original_content": '"$(echo "$ORIGINAL_CONTENT" | jq -Rs .)"'
  }
}' | nc -U /tmp/contextgraph.sock)

# Parse response
STATUS=$(echo "$RESPONSE" | jq -r '.result.status')
WARNINGS=$(echo "$RESPONSE" | jq -r '.result.warnings[]? // empty')

# Output warnings if any
if [ -n "$WARNINGS" ]; then
  echo "Alignment warnings:" >&2
  echo "$WARNINGS" >&2
fi

# Exit based on status
case "$STATUS" in
  "proceed")
    exit 0
    ;;
  "proceed_with_warning")
    exit 0
    ;;
  "block")
    echo "Write blocked due to low goal alignment" >&2
    exit 1
    ;;
  *)
    # Unknown status, allow by default
    exit 0
    ;;
esac
```

```bash
#!/bin/bash
# .claude/hooks/post_file_write.sh
# PostFileWrite hook - stores edits and triggers pattern learning

set -e

# Read input from stdin (JSON payload)
INPUT=$(cat)

# Extract file path and content
FILE_PATH=$(echo "$INPUT" | jq -r '.file_path')
NEW_CONTENT=$(echo "$INPUT" | jq -r '.new_content')
ORIGINAL_CONTENT=$(echo "$INPUT" | jq -r '.original_content // empty')
SOURCE_TOOL=$(echo "$INPUT" | jq -r '.source_tool // "Write"')

# Call MCP tool to store and learn
RESPONSE=$(echo '{
  "jsonrpc": "2.0",
  "id": "post-file-write-'$(date +%s)'",
  "method": "hooks/post_file_write",
  "params": {
    "file_path": "'"$FILE_PATH"'",
    "new_content": '"$(echo "$NEW_CONTENT" | jq -Rs .)"',
    "original_content": '"$(echo "$ORIGINAL_CONTENT" | jq -Rs .)"',
    "source_tool": "'"$SOURCE_TOOL"'",
    "train_patterns": true
  }
}' | nc -U /tmp/contextgraph.sock)

# Check for errors
ERROR=$(echo "$RESPONSE" | jq -r '.error.message // empty')
if [ -n "$ERROR" ]; then
  echo "Warning: PostFileWrite hook failed: $ERROR" >&2
  # Don't block the operation, just log
fi

# Output memory ID if successful
MEMORY_ID=$(echo "$RESPONSE" | jq -r '.result.memory_id // empty')
if [ -n "$MEMORY_ID" ]; then
  echo "Edit stored as memory: $MEMORY_ID" >&2
fi

exit 0
```
    </section>
  </pseudo_code>
</task_spec>
```

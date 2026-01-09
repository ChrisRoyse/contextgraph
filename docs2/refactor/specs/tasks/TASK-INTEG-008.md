# TASK-INTEG-008: Skills Development

```xml
<task_spec>
  <task_id>TASK-INTEG-008</task_id>
  <title>Skills Development (memory-search, goal-alignment, pattern-learning, context-injection, drift-check)</title>
  <status>pending</status>

  <objective>
    Implement Claude Code skills that wrap MCP tools to provide higher-level
    abstractions for memory search, goal alignment checking, pattern learning,
    context injection, and drift detection using the teleological array system.
  </objective>

  <rationale>
    Skills provide:
    1. Higher-level abstractions over raw MCP tools
    2. Auto-invocation based on context
    3. Pre/post processing of inputs/outputs
    4. Error handling and retry logic
    5. User-friendly interfaces for Claude Code users

    Skills make the teleological array system accessible without requiring
    direct MCP tool knowledge.
  </rationale>

  <dependencies>
    <dependency type="required">TASK-INTEG-001</dependency>    <!-- Memory MCP handlers -->
    <dependency type="required">TASK-INTEG-002</dependency>    <!-- Purpose/Goal MCP handlers -->
    <dependency type="required">TASK-INTEG-003</dependency>    <!-- Consciousness MCP handlers -->
    <dependency type="required">TASK-LOGIC-001</dependency>    <!-- Search engine -->
    <dependency type="required">TASK-LOGIC-008</dependency>    <!-- Alignment calculator -->
  </dependencies>

  <input_context_files>
    <file purpose="mcp_tools_spec">docs2/refactor/08-MCP-TOOLS.md</file>
    <file purpose="memory_handler">crates/context-graph-mcp/src/handlers/memory.rs</file>
    <file purpose="purpose_handler">crates/context-graph-mcp/src/handlers/purpose.rs</file>
    <file purpose="search_engine">crates/context-graph-storage/src/teleological/search/engine.rs</file>
  </input_context_files>

  <output_artifacts>
    <artifact type="source">crates/context-graph-mcp/src/skills/mod.rs</artifact>
    <artifact type="source">crates/context-graph-mcp/src/skills/loader.rs</artifact>
    <artifact type="config">.claude/skills/memory-search/skill.yaml</artifact>
    <artifact type="config">.claude/skills/memory-search/handler.ts</artifact>
    <artifact type="config">.claude/skills/goal-alignment/skill.yaml</artifact>
    <artifact type="config">.claude/skills/goal-alignment/handler.ts</artifact>
    <artifact type="config">.claude/skills/pattern-learning/skill.yaml</artifact>
    <artifact type="config">.claude/skills/pattern-learning/handler.ts</artifact>
    <artifact type="config">.claude/skills/context-injection/skill.yaml</artifact>
    <artifact type="config">.claude/skills/context-injection/handler.ts</artifact>
    <artifact type="config">.claude/skills/drift-check/skill.yaml</artifact>
    <artifact type="config">.claude/skills/drift-check/handler.ts</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/skills_test.rs</artifact>
  </output_artifacts>

  <definition_of_done>
    <criterion id="1">All 5 core skills implemented with YAML definitions</criterion>
    <criterion id="2">Skills auto-invoke appropriate MCP tools</criterion>
    <criterion id="3">Skill loader discovers and loads skills from .claude/skills/</criterion>
    <criterion id="4">Error handling with retry logic for each skill</criterion>
    <criterion id="5">TypeScript handlers provide clean interfaces</criterion>
    <criterion id="6">Skills transform results into user-friendly formats</criterion>
    <criterion id="7">Skills integrate with hook system for automated invocation</criterion>
    <criterion id="8">Test coverage for skill loading and invocation</criterion>
  </definition_of_done>

  <estimated_complexity>Medium</estimated_complexity>

  <pseudo_code>
    <section name="SkillLoader">
```rust
// crates/context-graph-mcp/src/skills/mod.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// Skill definition from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    /// Skill name
    pub name: String,
    /// Version
    pub version: String,
    /// Description
    pub description: String,
    /// MCP configuration
    pub mcp: McpConfig,
    /// Parameters schema
    pub parameters: HashMap<String, ParameterDef>,
    /// Success handlers
    #[serde(default)]
    pub on_success: Vec<SuccessAction>,
    /// Error handlers
    #[serde(default)]
    pub on_error: Vec<ErrorAction>,
    /// Result transformations
    #[serde(default)]
    pub transforms: Option<TransformConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// MCP server name
    pub server: String,
    /// Tool to invoke
    pub tool: String,
    /// Whether to auto-invoke
    pub auto_invoke: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDef {
    /// Parameter type
    #[serde(rename = "type")]
    pub param_type: String,
    /// Whether required
    #[serde(default)]
    pub required: bool,
    /// Default value
    pub default: Option<serde_json::Value>,
    /// Description
    pub description: Option<String>,
    /// Allowed values (for enums)
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SuccessAction {
    #[serde(rename = "log")]
    Log { message: String },
    #[serde(rename = "notify_hook")]
    NotifyHook { hook: String },
    #[serde(rename = "set_context")]
    SetContext { key: String },
    #[serde(rename = "store_goals")]
    StoreGoals { path: String },
    #[serde(rename = "emit_event")]
    EmitEvent { event: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ErrorAction {
    #[serde(rename = "log_error")]
    LogError { message: String },
    #[serde(rename = "retry")]
    Retry { max_attempts: u32, backoff: String },
    #[serde(rename = "fallback")]
    Fallback { skill: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformConfig {
    /// Result transformation expression
    pub result: Option<String>,
}

/// Skill registry and loader
pub struct SkillLoader {
    /// Base directory for skills
    skills_dir: PathBuf,
    /// Loaded skills
    skills: Arc<RwLock<HashMap<String, LoadedSkill>>>,
    /// MCP client for invoking tools
    mcp_client: Arc<McpClient>,
}

#[derive(Debug, Clone)]
pub struct LoadedSkill {
    /// Skill definition
    pub definition: SkillDefinition,
    /// Handler script path
    pub handler_path: Option<PathBuf>,
    /// Load timestamp
    pub loaded_at: DateTime<Utc>,
}

impl SkillLoader {
    pub fn new(skills_dir: PathBuf, mcp_client: Arc<McpClient>) -> Self {
        Self {
            skills_dir,
            skills: Arc::new(RwLock::new(HashMap::new())),
            mcp_client,
        }
    }

    /// Load all skills from the skills directory
    pub async fn load_all(&self) -> Result<usize, SkillError> {
        let mut count = 0;
        let mut skills = self.skills.write().await;

        // Scan skills directory
        let entries = std::fs::read_dir(&self.skills_dir)
            .map_err(|e| SkillError::IoError(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_yaml = path.join("skill.yaml");
                if skill_yaml.exists() {
                    match self.load_skill(&skill_yaml).await {
                        Ok(skill) => {
                            let name = skill.definition.name.clone();
                            skills.insert(name, skill);
                            count += 1;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load skill from {:?}: {}", skill_yaml, e);
                        }
                    }
                }
            }
        }

        tracing::info!("Loaded {} skills from {:?}", count, self.skills_dir);
        Ok(count)
    }

    /// Load a single skill from YAML file
    async fn load_skill(&self, yaml_path: &Path) -> Result<LoadedSkill, SkillError> {
        let content = std::fs::read_to_string(yaml_path)
            .map_err(|e| SkillError::IoError(e.to_string()))?;

        let definition: SkillDefinition = serde_yaml::from_str(&content)
            .map_err(|e| SkillError::ParseError(e.to_string()))?;

        // Look for handler script
        let skill_dir = yaml_path.parent().unwrap();
        let handler_path = skill_dir.join("handler.ts");
        let handler_path = if handler_path.exists() {
            Some(handler_path)
        } else {
            None
        };

        Ok(LoadedSkill {
            definition,
            handler_path,
            loaded_at: Utc::now(),
        })
    }

    /// Get a loaded skill by name
    pub async fn get(&self, name: &str) -> Option<LoadedSkill> {
        self.skills.read().await.get(name).cloned()
    }

    /// List all loaded skills
    pub async fn list(&self) -> Vec<String> {
        self.skills.read().await.keys().cloned().collect()
    }

    /// Invoke a skill
    pub async fn invoke(
        &self,
        name: &str,
        params: serde_json::Value,
    ) -> Result<SkillResult, SkillError> {
        let skill = self.get(name).await
            .ok_or_else(|| SkillError::NotFound(name.to_string()))?;

        let invoker = SkillInvoker::new(
            skill,
            self.mcp_client.clone(),
        );

        invoker.invoke(params).await
    }
}

/// Skill invocation handler
pub struct SkillInvoker {
    skill: LoadedSkill,
    mcp_client: Arc<McpClient>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    /// Whether invocation succeeded
    pub success: bool,
    /// Result data
    pub data: serde_json::Value,
    /// Messages/logs
    pub messages: Vec<String>,
    /// Execution time (ms)
    pub execution_time_ms: u64,
}

impl SkillInvoker {
    pub fn new(skill: LoadedSkill, mcp_client: Arc<McpClient>) -> Self {
        Self { skill, mcp_client }
    }

    /// Invoke the skill with given parameters
    pub async fn invoke(&self, params: serde_json::Value) -> Result<SkillResult, SkillError> {
        let start = Instant::now();
        let mut messages = Vec::new();

        // 1. Validate parameters
        let validated_params = self.validate_and_apply_defaults(params)?;

        // 2. Invoke MCP tool
        let mcp_result = self.invoke_mcp(&validated_params).await;

        // 3. Handle result
        match mcp_result {
            Ok(result) => {
                // Run success actions
                for action in &self.skill.definition.on_success {
                    let msg = self.run_success_action(action, &result).await?;
                    if let Some(m) = msg {
                        messages.push(m);
                    }
                }

                // Transform result if configured
                let transformed = self.transform_result(result)?;

                Ok(SkillResult {
                    success: true,
                    data: transformed,
                    messages,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            }
            Err(e) => {
                // Run error actions
                for action in &self.skill.definition.on_error {
                    match action {
                        ErrorAction::LogError { message } => {
                            let msg = self.interpolate(message, &serde_json::json!({"error": e.to_string()}))?;
                            tracing::error!("{}", msg);
                            messages.push(msg);
                        }
                        ErrorAction::Retry { max_attempts, backoff } => {
                            // Implement retry with backoff
                            for attempt in 1..=*max_attempts {
                                let delay = self.calculate_backoff(backoff, attempt);
                                tokio::time::sleep(delay).await;

                                if let Ok(result) = self.invoke_mcp(&validated_params).await {
                                    let transformed = self.transform_result(result)?;
                                    return Ok(SkillResult {
                                        success: true,
                                        data: transformed,
                                        messages,
                                        execution_time_ms: start.elapsed().as_millis() as u64,
                                    });
                                }
                            }
                        }
                        ErrorAction::Fallback { skill } => {
                            messages.push(format!("Falling back to skill: {}", skill));
                            // Would need to invoke fallback skill here
                        }
                    }
                }

                Err(e)
            }
        }
    }

    fn validate_and_apply_defaults(
        &self,
        mut params: serde_json::Value,
    ) -> Result<serde_json::Value, SkillError> {
        let params_obj = params.as_object_mut()
            .ok_or_else(|| SkillError::InvalidParams("Expected object".to_string()))?;

        for (name, def) in &self.skill.definition.parameters {
            if !params_obj.contains_key(name) {
                if def.required {
                    return Err(SkillError::MissingParam(name.clone()));
                }
                if let Some(default) = &def.default {
                    params_obj.insert(name.clone(), default.clone());
                }
            }
        }

        Ok(params)
    }

    async fn invoke_mcp(
        &self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, SkillError> {
        let config = &self.skill.definition.mcp;

        self.mcp_client.invoke(
            &config.server,
            &config.tool,
            params.clone(),
        )
        .await
        .map_err(|e| SkillError::McpError(e.to_string()))
    }

    async fn run_success_action(
        &self,
        action: &SuccessAction,
        result: &serde_json::Value,
    ) -> Result<Option<String>, SkillError> {
        match action {
            SuccessAction::Log { message } => {
                let msg = self.interpolate(message, result)?;
                tracing::info!("{}", msg);
                Ok(Some(msg))
            }
            SuccessAction::NotifyHook { hook } => {
                // Emit hook notification
                tracing::debug!("Notifying hook: {}", hook);
                Ok(None)
            }
            SuccessAction::SetContext { key } => {
                // Set in session context
                tracing::debug!("Setting context key: {}", key);
                Ok(None)
            }
            SuccessAction::StoreGoals { path } => {
                // Store goals from result
                tracing::debug!("Storing goals from: {}", path);
                Ok(None)
            }
            SuccessAction::EmitEvent { event } => {
                // Emit event
                tracing::debug!("Emitting event: {}", event);
                Ok(None)
            }
        }
    }

    fn transform_result(&self, result: serde_json::Value) -> Result<serde_json::Value, SkillError> {
        if let Some(config) = &self.skill.definition.transforms {
            if let Some(expr) = &config.result {
                // Apply transformation expression
                // For now, just return the result
                // A full implementation would use a JS/expression engine
                return Ok(result);
            }
        }
        Ok(result)
    }

    fn interpolate(&self, template: &str, context: &serde_json::Value) -> Result<String, SkillError> {
        let mut result = template.to_string();

        // Simple {{key}} interpolation
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        for cap in re.captures_iter(template) {
            let key = &cap[1];
            let value = self.extract_value(context, key)
                .unwrap_or_else(|| serde_json::Value::String(format!("{{{{{}}}}}",key)));
            let replacement = match value {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            result = result.replace(&cap[0], &replacement);
        }

        Ok(result)
    }

    fn extract_value(&self, obj: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = obj;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }

    fn calculate_backoff(&self, backoff_type: &str, attempt: u32) -> Duration {
        match backoff_type {
            "exponential" => Duration::from_millis(100 * 2u64.pow(attempt - 1)),
            "linear" => Duration::from_millis(100 * attempt as u64),
            _ => Duration::from_millis(100),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    #[error("Missing required parameter: {0}")]
    MissingParam(String),
    #[error("MCP error: {0}")]
    McpError(String),
}
```
    </section>

    <section name="MemorySearchSkill">
```yaml
# .claude/skills/memory-search/skill.yaml
---
skill: memory-search
version: 1.0.0
description: |
  Search teleological memory using entry-point discovery. Automatically
  selects optimal embedding spaces for the query.

mcp:
  server: contextgraph
  tool: memory/search
  auto_invoke: true

parameters:
  query:
    type: string
    required: true
    description: Search query text
  strategy:
    type: object
    required: false
    default:
      type: auto_discover
      max_entry_points: 5
      min_confidence: 0.6
    description: Search strategy configuration
  limit:
    type: integer
    required: false
    default: 10
    description: Maximum results to return
  threshold:
    type: number
    required: false
    default: 0.5
    description: Minimum similarity threshold
  namespace:
    type: string
    required: false
    description: Namespace to search within

on_success:
  - type: log
    message: "Found {{result.memories.length}} memories for query"
  - type: set_context
    key: last_search_results

on_error:
  - type: log_error
    message: "Memory search failed: {{error.message}}"
  - type: retry
    max_attempts: 2
    backoff: exponential

transforms:
  result: |
    result.memories.map(m => ({
      id: m.memory_id,
      content: m.content.substring(0, 200),
      score: m.overall_similarity.toFixed(2),
      entry_points: Object.keys(m.entry_point_hits || {}),
      type: m.memory_type
    }))
---
```

```typescript
// .claude/skills/memory-search/handler.ts
import { McpClient } from '@contextgraph/mcp-client';

interface SearchParams {
  query: string;
  strategy?: {
    type: 'auto_discover' | 'single_embedder' | 'embedder_group' | 'weighted_full';
    max_entry_points?: number;
    min_confidence?: number;
    embedder?: string;
    group?: string;
    weights?: Record<string, number>;
  };
  limit?: number;
  threshold?: number;
  namespace?: string;
}

interface SearchResult {
  memories: Array<{
    memory_id: string;
    content: string;
    overall_similarity: number;
    entry_point_hits: Record<string, number>;
    memory_type: string;
  }>;
  search_metrics: {
    total_candidates: number;
    embedders_used: string[];
    execution_time_ms: number;
  };
}

export async function handler(params: SearchParams, client: McpClient): Promise<any> {
  // Apply defaults
  const searchParams = {
    query: params.query,
    strategy: params.strategy || {
      type: 'auto_discover',
      max_entry_points: 5,
      min_confidence: 0.6
    },
    limit: params.limit || 10,
    threshold: params.threshold || 0.5,
    namespace: params.namespace
  };

  // Invoke MCP tool
  const result = await client.invoke<SearchResult>('contextgraph', 'memory/search', searchParams);

  // Transform results for user-friendly output
  return {
    found: result.memories.length,
    results: result.memories.map(m => ({
      id: m.memory_id,
      preview: m.content.substring(0, 200) + (m.content.length > 200 ? '...' : ''),
      similarity: (m.overall_similarity * 100).toFixed(1) + '%',
      entryPoints: Object.keys(m.entry_point_hits),
      type: m.memory_type
    })),
    searchInfo: {
      candidates: result.search_metrics.total_candidates,
      embedders: result.search_metrics.embedders_used,
      timeMs: result.search_metrics.execution_time_ms
    }
  };
}
```
    </section>

    <section name="GoalAlignmentSkill">
```yaml
# .claude/skills/goal-alignment/skill.yaml
---
skill: goal-alignment
version: 1.0.0
description: |
  Check alignment between content and discovered goals. Uses teleological
  array comparison for accurate apples-to-apples measurement.

mcp:
  server: contextgraph
  tool: purpose/goal_alignment
  auto_invoke: true

parameters:
  content:
    type: string
    required: true
    description: Content to check alignment for
  goal_id:
    type: string
    required: false
    description: Specific goal ID to check against (optional)
  comparison_type:
    type: object
    required: false
    default:
      type: auto_discover
      max_entry_points: 5
    description: Comparison strategy

on_success:
  - type: log
    message: "Goal alignment: {{result.overall_alignment}}% with {{result.matched_goal.label}}"
  - type: notify_hook
    hook: post_alignment_check

on_error:
  - type: log_error
    message: "Goal alignment check failed: {{error.message}}"
  - type: retry
    max_attempts: 2
    backoff: exponential

transforms:
  result: |
    {
      aligned: result.overall_alignment > 0.6,
      score: (result.overall_alignment * 100).toFixed(1) + '%',
      goal: result.matched_goal?.label,
      strong_embedders: result.per_embedder_alignment
        .filter(e => e.score > 0.7)
        .map(e => e.embedder),
      weak_embedders: result.per_embedder_alignment
        .filter(e => e.score < 0.5)
        .map(e => e.embedder)
    }
---
```

```typescript
// .claude/skills/goal-alignment/handler.ts
import { McpClient } from '@contextgraph/mcp-client';

interface AlignmentParams {
  content: string;
  goal_id?: string;
  comparison_type?: {
    type: 'auto_discover' | 'weighted_full' | 'matrix_strategy';
    max_entry_points?: number;
    weights?: Record<string, number>;
    matrix?: string;
  };
}

interface AlignmentResult {
  overall_alignment: number;
  matched_goal: {
    goal_id: string;
    label: string;
    level: string;
  };
  per_embedder_alignment: Array<{
    embedder: string;
    score: number;
    contribution: number;
  }>;
  recommendations: Array<{
    embedder: string;
    issue: string;
    suggestion: string;
  }>;
}

export async function handler(params: AlignmentParams, client: McpClient): Promise<any> {
  const result = await client.invoke<AlignmentResult>(
    'contextgraph',
    'purpose/goal_alignment',
    params
  );

  // Build user-friendly response
  const alignmentPercent = (result.overall_alignment * 100).toFixed(1);
  const isAligned = result.overall_alignment >= 0.6;

  return {
    status: isAligned ? 'aligned' : 'misaligned',
    score: `${alignmentPercent}%`,
    goal: {
      id: result.matched_goal.goal_id,
      name: result.matched_goal.label,
      level: result.matched_goal.level
    },
    analysis: {
      strongAreas: result.per_embedder_alignment
        .filter(e => e.score >= 0.7)
        .map(e => ({
          area: formatEmbedder(e.embedder),
          score: `${(e.score * 100).toFixed(0)}%`
        })),
      weakAreas: result.per_embedder_alignment
        .filter(e => e.score < 0.5)
        .map(e => ({
          area: formatEmbedder(e.embedder),
          score: `${(e.score * 100).toFixed(0)}%`
        }))
    },
    recommendations: result.recommendations.map(r => ({
      area: formatEmbedder(r.embedder),
      issue: r.issue,
      action: r.suggestion
    }))
  };
}

function formatEmbedder(embedder: string): string {
  const names: Record<string, string> = {
    'e1_semantic': 'Semantic meaning',
    'e2_temporal_recent': 'Recent context',
    'e3_temporal_periodic': 'Periodic patterns',
    'e4_entity': 'Entity relationships',
    'e5_causal': 'Cause-effect',
    'e6_splade_expansion': 'Term expansion',
    'e7_code': 'Code understanding',
    'e8_graph': 'Graph structure',
    'e9_hdc': 'Holographic',
    'e10_multimodal': 'Multimodal',
    'e11_entity_transe': 'Knowledge base',
    'e12_late_interaction': 'Precision',
    'e13_splade_keyword': 'Keywords'
  };
  return names[embedder] || embedder;
}
```
    </section>

    <section name="PatternLearningSkill">
```yaml
# .claude/skills/pattern-learning/skill.yaml
---
skill: pattern-learning
version: 1.0.0
description: |
  Learn and store patterns from observations. Supports code patterns,
  workflow patterns, and error patterns for future recall.

mcp:
  server: contextgraph
  tool: memory/inject
  auto_invoke: false

parameters:
  pattern_type:
    type: string
    required: true
    values: [code, workflow, error, refactoring, test]
    description: Type of pattern to learn
  content:
    type: string
    required: true
    description: Pattern content to learn
  context:
    type: object
    required: false
    description: Additional context for the pattern
  tags:
    type: array
    required: false
    description: Tags for categorization

on_success:
  - type: log
    message: "Learned {{params.pattern_type}} pattern: {{result.memory_id}}"
  - type: emit_event
    event: pattern_learned

on_error:
  - type: log_error
    message: "Pattern learning failed: {{error.message}}"
  - type: retry
    max_attempts: 3
    backoff: exponential
---
```

```typescript
// .claude/skills/pattern-learning/handler.ts
import { McpClient } from '@contextgraph/mcp-client';

interface PatternParams {
  pattern_type: 'code' | 'workflow' | 'error' | 'refactoring' | 'test';
  content: string;
  context?: {
    file_path?: string;
    language?: string;
    framework?: string;
    before_content?: string;
    after_content?: string;
  };
  tags?: string[];
}

export async function handler(params: PatternParams, client: McpClient): Promise<any> {
  // Prepare content with pattern metadata
  const enrichedContent = `
[Pattern Type: ${params.pattern_type}]
${params.context?.language ? `[Language: ${params.context.language}]` : ''}
${params.context?.framework ? `[Framework: ${params.context.framework}]` : ''}

${params.content}

${params.context?.before_content ? `\n[Before]:\n${params.context.before_content}` : ''}
${params.context?.after_content ? `\n[After]:\n${params.context.after_content}` : ''}
`.trim();

  // Inject into memory with pattern metadata
  const result = await client.invoke('contextgraph', 'memory/inject', {
    content: enrichedContent,
    memory_type: `pattern_${params.pattern_type}`,
    namespace: 'patterns',
    metadata: {
      pattern_type: params.pattern_type,
      tags: params.tags || [],
      context: params.context,
      learned_at: new Date().toISOString()
    }
  });

  return {
    success: true,
    patternId: result.memory_id,
    type: params.pattern_type,
    tags: params.tags || [],
    message: `Pattern learned and stored for future reference`
  };
}
```
    </section>

    <section name="DriftCheckSkill">
```yaml
# .claude/skills/drift-check/skill.yaml
---
skill: drift-check
version: 1.0.0
description: |
  Check for purpose drift between current work and established goals.
  Detects when work is diverging from intended direction.

mcp:
  server: contextgraph
  tool: purpose/drift_check
  auto_invoke: true

parameters:
  memory_ids:
    type: array
    required: true
    description: Memory IDs representing recent work
  goal_id:
    type: string
    required: true
    description: Goal to check drift against
  comparison_type:
    type: object
    required: false
    default:
      type: matrix_strategy
      matrix: semantic_focused

on_success:
  - type: log
    message: "Drift check: {{result.overall_drift.drift_level}} drift detected"

on_error:
  - type: log_error
    message: "Drift check failed: {{error.message}}"
  - type: retry
    max_attempts: 2
    backoff: linear
---
```

```typescript
// .claude/skills/drift-check/handler.ts
import { McpClient } from '@contextgraph/mcp-client';

interface DriftParams {
  memory_ids: string[];
  goal_id: string;
  comparison_type?: {
    type: 'matrix_strategy' | 'weighted_full';
    matrix?: string;
    weights?: Record<string, number>;
  };
}

interface DriftResult {
  overall_drift: {
    has_drifted: boolean;
    drift_score: number;
    drift_level: 'none' | 'low' | 'medium' | 'high' | 'critical';
  };
  per_embedder_drift: Record<string, {
    similarity: number;
    drift_level: string;
  }>;
  most_drifted_embedders: Array<{
    embedder: string;
    drift_level: string;
  }>;
  recommendations: Array<{
    embedder: string;
    issue: string;
    suggestion: string;
  }>;
  trend: {
    direction: 'improving' | 'stable' | 'worsening';
    velocity: number;
    projected_critical_in?: string;
  };
}

export async function handler(params: DriftParams, client: McpClient): Promise<any> {
  const result = await client.invoke<DriftResult>(
    'contextgraph',
    'purpose/drift_check',
    params
  );

  // Build user-friendly response
  const driftEmoji = getDriftEmoji(result.overall_drift.drift_level);
  const trendEmoji = getTrendEmoji(result.trend.direction);

  return {
    status: `${driftEmoji} ${result.overall_drift.drift_level.toUpperCase()} drift`,
    hasDrifted: result.overall_drift.has_drifted,
    driftScore: `${(result.overall_drift.drift_score * 100).toFixed(1)}%`,
    trend: {
      direction: `${trendEmoji} ${result.trend.direction}`,
      velocity: result.trend.velocity.toFixed(3),
      warning: result.trend.projected_critical_in
    },
    driftedAreas: result.most_drifted_embedders.map(e => ({
      area: formatEmbedder(e.embedder),
      level: e.drift_level
    })),
    actions: result.recommendations.map(r => ({
      area: formatEmbedder(r.embedder),
      problem: r.issue,
      solution: r.suggestion
    }))
  };
}

function getDriftEmoji(level: string): string {
  const emojis: Record<string, string> = {
    'none': '',
    'low': '',
    'medium': '',
    'high': '',
    'critical': ''
  };
  return emojis[level] || '';
}

function getTrendEmoji(direction: string): string {
  const emojis: Record<string, string> = {
    'improving': '',
    'stable': '',
    'worsening': ''
  };
  return emojis[direction] || '';
}

function formatEmbedder(embedder: string): string {
  // Same as in goal-alignment
  const names: Record<string, string> = {
    'e1_semantic': 'Semantic meaning',
    'e2_temporal_recent': 'Recent context',
    'e5_causal': 'Cause-effect',
    'e7_code': 'Code understanding'
  };
  return names[embedder] || embedder;
}
```
    </section>

    <section name="ContextInjectionSkill">
```yaml
# .claude/skills/context-injection/skill.yaml
---
skill: context-injection
version: 1.0.0
description: |
  Inject content into teleological memory with autonomous embedding
  across all 13 embedding dimensions.

mcp:
  server: contextgraph
  tool: memory/inject
  auto_invoke: true

parameters:
  content:
    type: string
    required: true
    description: Content to inject into memory
  memory_type:
    type: string
    required: false
    default: general
    values: [code_context, documentation, code_snippet, conversation, general, pattern]
    description: Type of memory being stored
  namespace:
    type: string
    required: false
    default: default
    description: Namespace for organization
  metadata:
    type: object
    required: false
    default: {}
    description: Additional metadata to store

on_success:
  - type: log
    message: "Injected memory {{result.memory_id}} with {{result.embedders_generated}} embedders"
  - type: notify_hook
    hook: post_memory_inject

on_error:
  - type: log_error
    message: "Memory injection failed: {{error.message}}"
  - type: retry
    max_attempts: 3
    backoff: exponential
---
```

```typescript
// .claude/skills/context-injection/handler.ts
import { McpClient } from '@contextgraph/mcp-client';

interface InjectionParams {
  content: string;
  memory_type?: string;
  namespace?: string;
  metadata?: Record<string, any>;
}

interface InjectionResult {
  memory_id: string;
  embedders_generated: number;
  embedding_time_ms: number;
  namespace: string;
}

export async function handler(params: InjectionParams, client: McpClient): Promise<any> {
  // Detect content type if not specified
  const memoryType = params.memory_type || detectContentType(params.content);

  const result = await client.invoke<InjectionResult>(
    'contextgraph',
    'memory/inject',
    {
      content: params.content,
      memory_type: memoryType,
      namespace: params.namespace || 'default',
      metadata: {
        ...params.metadata,
        injected_at: new Date().toISOString(),
        source: 'context-injection-skill'
      }
    }
  );

  return {
    success: true,
    memoryId: result.memory_id,
    type: memoryType,
    namespace: result.namespace,
    embeddings: result.embedders_generated,
    timeMs: result.embedding_time_ms,
    message: `Content stored with ${result.embedders_generated} embedding dimensions`
  };
}

function detectContentType(content: string): string {
  // Simple heuristics for content type detection
  if (content.includes('```') || content.includes('function ') || content.includes('class ')) {
    return 'code_snippet';
  }
  if (content.includes('#') && content.includes('\n')) {
    return 'documentation';
  }
  if (content.startsWith('{') || content.startsWith('[')) {
    return 'data';
  }
  return 'general';
}
```
    </section>
  </pseudo_code>
</task_spec>
```

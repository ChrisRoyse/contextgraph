# TASK-INTEG-004: Hook Protocol and Core Handlers

```xml
<task_spec id="TASK-INTEG-004" version="1.0">
<metadata>
  <title>Implement Hook Protocol and Core Handlers</title>
  <status>todo</status>
  <layer>integration</layer>
  <sequence>24</sequence>
  <implements>
    <requirement_ref>REQ-HOOKS-PROTOCOL-01</requirement_ref>
    <requirement_ref>REQ-HOOKS-PRETASK-01</requirement_ref>
    <requirement_ref>REQ-HOOKS-POSTTASK-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-INTEG-001</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
Hook handlers integrate with Claude Code's hooks system to enable automatic memory
injection, context loading, and learning from operations. This implements the core
PreTask, PostTask, SessionStart, and SessionEnd hooks.
</context>

<objective>
Implement the hook protocol and core handlers for PreTask, PostTask, SessionStart,
and SessionEnd hooks that trigger teleological array operations.
</objective>

<rationale>
Hooks enable automatic learning:
1. PreTask loads relevant context before operations
2. PostTask stores outcomes and trains patterns
3. SessionStart initializes teleological context
4. SessionEnd consolidates session learnings
</rationale>

<input_context_files>
  <file purpose="hook_spec">docs2/refactor/08-MCP-TOOLS.md#hook-integration</file>
  <file purpose="task_breakdown">docs2/refactor/07-TASK-BREAKDOWN.md#phase-4</file>
</input_context_files>

<prerequisites>
  <check>TASK-INTEG-001 complete (MemoryHandler exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Define HookEvent, HookPayload, HookResponse types</item>
    <item>Implement hooks/pre-task handler</item>
    <item>Implement hooks/post-task handler</item>
    <item>Implement hooks/session-start handler</item>
    <item>Implement hooks/session-end handler</item>
    <item>Trajectory tracking for learning</item>
  </in_scope>
  <out_of_scope>
    <item>Edit hooks (TASK-INTEG-005)</item>
    <item>File operation hooks (TASK-INTEG-006)</item>
    <item>Background workers (TASK-INTEG-007)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/hooks/protocol.rs">
      use serde::{Deserialize, Serialize};

      /// Hook event types matching Claude Code specification.
      #[derive(Debug, Clone, Serialize, Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub enum HookEvent {
          PreTask,
          PostTask,
          PreEdit,
          PostEdit,
          PreFileRead,
          PostFileRead,
          PreFileWrite,
          PostFileWrite,
          PreBashExec,
          PostBashExec,
          SessionStart,
          SessionEnd,
      }

      /// Payload sent with hook invocations.
      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct HookPayload {
          pub event: HookEvent,
          pub session_id: String,
          pub timestamp: DateTime<Utc>,
          pub context: HookContext,
      }

      /// Context data specific to each hook type.
      #[derive(Debug, Clone, Serialize, Deserialize)]
      #[serde(untagged)]
      pub enum HookContext {
          Task(TaskContext),
          Edit(EditContext),
          File(FileContext),
          Bash(BashContext),
          Session(SessionContext),
      }

      /// Response returned from hook handlers.
      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct HookResponse {
          pub success: bool,
          pub context_recommendations: Option<Vec<ContextRecommendation>>,
          pub trajectory_id: Option<String>,
          pub metrics: Option<HookMetrics>,
          pub error: Option<String>,
      }
    </signature>

    <signature file="crates/context-graph-mcp/src/hooks/pre_task.rs">
      /// Pre-task hook handler.
      pub struct PreTaskHandler {
          memory_handler: Arc<MemoryHandler>,
          trajectory_tracker: Arc<TrajectoryTracker>,
      }

      impl PreTaskHandler {
          pub async fn handle(
              &self,
              payload: HookPayload,
          ) -> Result<HookResponse, HandlerError>;
      }
    </signature>

    <signature file="crates/context-graph-mcp/src/hooks/post_task.rs">
      /// Post-task hook handler.
      pub struct PostTaskHandler {
          memory_handler: Arc<MemoryHandler>,
          trajectory_tracker: Arc<TrajectoryTracker>,
          pattern_store: Arc<PatternStore>,
      }

      impl PostTaskHandler {
          pub async fn handle(
              &self,
              payload: HookPayload,
          ) -> Result<HookResponse, HandlerError>;
      }
    </signature>

    <signature file="crates/context-graph-mcp/src/hooks/session.rs">
      /// Session lifecycle handler.
      pub struct SessionHandler {
          memory_handler: Arc<MemoryHandler>,
          session_store: Arc<SessionStore>,
          consolidator: Arc<MemoryConsolidator>,
      }

      impl SessionHandler {
          pub async fn handle_start(
              &self,
              payload: HookPayload,
          ) -> Result<HookResponse, HandlerError>;

          pub async fn handle_end(
              &self,
              payload: HookPayload,
          ) -> Result<HookResponse, HandlerError>;
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Hook responses complete within 50ms target</constraint>
    <constraint>Trajectory tracking persists across sessions</constraint>
    <constraint>Session state serializable for restoration</constraint>
    <constraint>Error handling graceful (don't break Claude Code)</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-mcp hooks::protocol</command>
    <command>cargo test -p context-graph-mcp hooks::pre_task</command>
    <command>cargo test -p context-graph-mcp hooks::post_task</command>
    <command>cargo test -p context-graph-mcp hooks::session</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-mcp/src/hooks/protocol.rs

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    PreTask,
    PostTask,
    PreEdit,
    PostEdit,
    PreFileRead,
    PostFileRead,
    PreFileWrite,
    PostFileWrite,
    PreBashExec,
    PostBashExec,
    SessionStart,
    SessionEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    pub event: HookEvent,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub context: HookContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookContext {
    Task(TaskContext),
    Edit(EditContext),
    File(FileContext),
    Bash(BashContext),
    Session(SessionContext),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub task_id: String,
    pub description: String,
    pub tool_name: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub result: Option<TaskResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub namespace: Option<String>,
    pub previous_session_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub success: bool,
    pub context_recommendations: Option<Vec<ContextRecommendation>>,
    pub trajectory_id: Option<String>,
    pub metrics: Option<HookMetrics>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRecommendation {
    pub memory_id: String,
    pub content_preview: String,
    pub relevance_score: f32,
    pub entry_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMetrics {
    pub processing_time_ms: u64,
    pub memories_accessed: usize,
    pub patterns_matched: usize,
}

// crates/context-graph-mcp/src/hooks/pre_task.rs

use std::sync::Arc;

use crate::protocol::HandlerError;
use crate::handlers::memory::MemoryHandler;
use super::protocol::{HookPayload, HookResponse, HookContext, TaskContext};

pub struct TrajectoryTracker {
    trajectories: RwLock<HashMap<String, Trajectory>>,
}

impl TrajectoryTracker {
    pub fn new() -> Self {
        Self {
            trajectories: RwLock::new(HashMap::new()),
        }
    }

    pub async fn start_trajectory(&self, session_id: &str, task_id: &str) -> String {
        let trajectory_id = format!("{}_{}", session_id, task_id);
        let trajectory = Trajectory {
            id: trajectory_id.clone(),
            session_id: session_id.to_string(),
            task_id: task_id.to_string(),
            steps: Vec::new(),
            started_at: Utc::now(),
            completed_at: None,
            verdict: None,
        };
        self.trajectories.write().await.insert(trajectory_id.clone(), trajectory);
        trajectory_id
    }

    pub async fn add_step(&self, trajectory_id: &str, step: TrajectoryStep) {
        if let Some(trajectory) = self.trajectories.write().await.get_mut(trajectory_id) {
            trajectory.steps.push(step);
        }
    }

    pub async fn complete_trajectory(&self, trajectory_id: &str, verdict: Verdict) {
        if let Some(trajectory) = self.trajectories.write().await.get_mut(trajectory_id) {
            trajectory.completed_at = Some(Utc::now());
            trajectory.verdict = Some(verdict);
        }
    }
}

pub struct PreTaskHandler {
    memory_handler: Arc<MemoryHandler>,
    trajectory_tracker: Arc<TrajectoryTracker>,
}

impl PreTaskHandler {
    pub fn new(
        memory_handler: Arc<MemoryHandler>,
        trajectory_tracker: Arc<TrajectoryTracker>,
    ) -> Self {
        Self {
            memory_handler,
            trajectory_tracker,
        }
    }

    pub async fn handle(
        &self,
        payload: HookPayload,
    ) -> Result<HookResponse, HandlerError> {
        let start = std::time::Instant::now();

        // Extract task context
        let task_ctx = match payload.context {
            HookContext::Task(ctx) => ctx,
            _ => return Err(HandlerError::new(
                ErrorCode::InvalidParams,
                "Expected task context",
            )),
        };

        // Start trajectory tracking
        let trajectory_id = self.trajectory_tracker
            .start_trajectory(&payload.session_id, &task_ctx.task_id)
            .await;

        // Search for relevant context
        let search_params = SearchParams {
            query: task_ctx.description.clone(),
            strategy: SearchStrategy::AutoDiscover {
                max_entry_points: Some(3),
                min_confidence: Some(0.6),
            },
            limit: Some(5),
            threshold: Some(0.5),
            options: None,
        };

        let search_result = self.memory_handler
            .handle_search(search_params)
            .await
            .unwrap_or_default();

        // Build recommendations
        let recommendations: Vec<_> = search_result.memories.iter()
            .map(|m| ContextRecommendation {
                memory_id: m.memory_id.to_string(),
                content_preview: m.content.chars().take(200).collect(),
                relevance_score: m.overall_similarity,
                entry_points: m.entry_point_hits.as_ref()
                    .map(|h| h.keys().cloned().collect())
                    .unwrap_or_default(),
            })
            .collect();

        let metrics = HookMetrics {
            processing_time_ms: start.elapsed().as_millis() as u64,
            memories_accessed: search_result.memories.len(),
            patterns_matched: 0,
        };

        Ok(HookResponse {
            success: true,
            context_recommendations: Some(recommendations),
            trajectory_id: Some(trajectory_id),
            metrics: Some(metrics),
            error: None,
        })
    }
}

// crates/context-graph-mcp/src/hooks/post_task.rs

pub struct PostTaskHandler {
    memory_handler: Arc<MemoryHandler>,
    trajectory_tracker: Arc<TrajectoryTracker>,
    pattern_store: Arc<PatternStore>,
}

impl PostTaskHandler {
    pub fn new(
        memory_handler: Arc<MemoryHandler>,
        trajectory_tracker: Arc<TrajectoryTracker>,
        pattern_store: Arc<PatternStore>,
    ) -> Self {
        Self {
            memory_handler,
            trajectory_tracker,
            pattern_store,
        }
    }

    pub async fn handle(
        &self,
        payload: HookPayload,
    ) -> Result<HookResponse, HandlerError> {
        let start = std::time::Instant::now();

        let task_ctx = match payload.context {
            HookContext::Task(ctx) => ctx,
            _ => return Err(HandlerError::new(
                ErrorCode::InvalidParams,
                "Expected task context",
            )),
        };

        // Determine verdict from result
        let verdict = match &task_ctx.result {
            Some(result) if result.success => Verdict::Success,
            Some(_) => Verdict::Failure,
            None => Verdict::Partial,
        };

        // Complete trajectory
        let trajectory_id = format!("{}_{}", payload.session_id, task_ctx.task_id);
        self.trajectory_tracker
            .complete_trajectory(&trajectory_id, verdict.clone())
            .await;

        // Store task outcome if successful
        if verdict == Verdict::Success {
            if let Some(result) = &task_ctx.result {
                if let Some(output) = &result.output {
                    let inject_params = InjectParams {
                        content: format!(
                            "Task: {}\nOutput: {}",
                            task_ctx.description,
                            output.chars().take(1000).collect::<String>()
                        ),
                        memory_type: Some(MemoryType::TaskOutcome),
                        namespace: Some(payload.session_id.clone()),
                        metadata: Some(serde_json::json!({
                            "task_id": task_ctx.task_id,
                            "tool_name": task_ctx.tool_name,
                            "verdict": format!("{:?}", verdict),
                        })),
                        options: None,
                    };

                    let _ = self.memory_handler.handle_inject(inject_params).await;
                }
            }
        }

        // Extract and store patterns
        let patterns_matched = self.extract_patterns(&task_ctx, &verdict).await;

        let metrics = HookMetrics {
            processing_time_ms: start.elapsed().as_millis() as u64,
            memories_accessed: 1,
            patterns_matched,
        };

        Ok(HookResponse {
            success: true,
            context_recommendations: None,
            trajectory_id: Some(trajectory_id),
            metrics: Some(metrics),
            error: None,
        })
    }

    async fn extract_patterns(&self, task_ctx: &TaskContext, verdict: &Verdict) -> usize {
        // Extract patterns from successful tasks
        if *verdict == Verdict::Success {
            // Store pattern for future retrieval
            if let Some(tool_name) = &task_ctx.tool_name {
                let pattern = Pattern {
                    tool: tool_name.clone(),
                    description: task_ctx.description.clone(),
                    success: true,
                    created_at: Utc::now(),
                };
                let _ = self.pattern_store.store(pattern).await;
                return 1;
            }
        }
        0
    }
}

// crates/context-graph-mcp/src/hooks/session.rs

pub struct SessionHandler {
    memory_handler: Arc<MemoryHandler>,
    session_store: Arc<SessionStore>,
    consolidator: Arc<MemoryConsolidator>,
}

impl SessionHandler {
    pub fn new(
        memory_handler: Arc<MemoryHandler>,
        session_store: Arc<SessionStore>,
        consolidator: Arc<MemoryConsolidator>,
    ) -> Self {
        Self {
            memory_handler,
            session_store,
            consolidator,
        }
    }

    pub async fn handle_start(
        &self,
        payload: HookPayload,
    ) -> Result<HookResponse, HandlerError> {
        let start = std::time::Instant::now();

        let session_ctx = match payload.context {
            HookContext::Session(ctx) => ctx,
            _ => return Err(HandlerError::new(
                ErrorCode::InvalidParams,
                "Expected session context",
            )),
        };

        // Initialize session state
        let session = Session {
            id: session_ctx.session_id.clone(),
            namespace: session_ctx.namespace.clone(),
            started_at: Utc::now(),
            previous_session_id: session_ctx.previous_session_id.clone(),
            metadata: session_ctx.metadata.clone(),
        };
        self.session_store.store(session).await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Load relevant prior context
        let recommendations = if let Some(prev_id) = &session_ctx.previous_session_id {
            // Search for context from previous session
            let search_params = SearchParams {
                query: "recent session context".to_string(),
                strategy: SearchStrategy::EmbedderGroup {
                    group: EmbedderGroup::Temporal,
                    embedders: None,
                },
                limit: Some(10),
                threshold: None,
                options: None,
            };

            self.memory_handler
                .handle_search(search_params)
                .await
                .ok()
                .map(|r| r.memories.into_iter()
                    .map(|m| ContextRecommendation {
                        memory_id: m.memory_id.to_string(),
                        content_preview: m.content.chars().take(200).collect(),
                        relevance_score: m.overall_similarity,
                        entry_points: Vec::new(),
                    })
                    .collect())
        } else {
            None
        };

        let metrics = HookMetrics {
            processing_time_ms: start.elapsed().as_millis() as u64,
            memories_accessed: recommendations.as_ref().map(|r| r.len()).unwrap_or(0),
            patterns_matched: 0,
        };

        Ok(HookResponse {
            success: true,
            context_recommendations: recommendations,
            trajectory_id: None,
            metrics: Some(metrics),
            error: None,
        })
    }

    pub async fn handle_end(
        &self,
        payload: HookPayload,
    ) -> Result<HookResponse, HandlerError> {
        let start = std::time::Instant::now();

        let session_ctx = match payload.context {
            HookContext::Session(ctx) => ctx,
            _ => return Err(HandlerError::new(
                ErrorCode::InvalidParams,
                "Expected session context",
            )),
        };

        // Consolidate session memories
        let consolidation_result = self.consolidator
            .consolidate_session(&session_ctx.session_id)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Update session state
        self.session_store
            .mark_completed(&session_ctx.session_id)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        let metrics = HookMetrics {
            processing_time_ms: start.elapsed().as_millis() as u64,
            memories_accessed: consolidation_result.original_count,
            patterns_matched: consolidation_result.patterns_extracted,
        };

        Ok(HookResponse {
            success: true,
            context_recommendations: None,
            trajectory_id: None,
            metrics: Some(metrics),
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pre_task_hook() {
        // Test pre-task context loading
    }

    #[tokio::test]
    async fn test_post_task_hook() {
        // Test post-task outcome storage
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        // Test session start/end
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/hooks/protocol.rs">
    Hook protocol types
  </file>
  <file path="crates/context-graph-mcp/src/hooks/pre_task.rs">
    Pre-task hook handler
  </file>
  <file path="crates/context-graph-mcp/src/hooks/post_task.rs">
    Post-task hook handler
  </file>
  <file path="crates/context-graph-mcp/src/hooks/session.rs">
    Session lifecycle hooks
  </file>
  <file path="crates/context-graph-mcp/src/hooks/mod.rs">
    Hooks module definition
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/core.rs">
    Add dispatch routes for hooks/* endpoints
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Pre-task returns context recommendations</criterion>
  <criterion>Post-task stores outcomes and patterns</criterion>
  <criterion>Session-start initializes state</criterion>
  <criterion>Session-end consolidates learnings</criterion>
  <criterion>Response time under 50ms target</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mcp hooks -- --nocapture</command>
</test_commands>
</task_spec>
```

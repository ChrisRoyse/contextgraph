# TASK-INTEG-010: Integration Testing (Hooks, Skills, Subagents)

```xml
<task_spec>
  <task_id>TASK-INTEG-010</task_id>
  <title>Integration Testing for Hooks, Skills, and Subagents</title>
  <status>pending</status>

  <objective>
    Implement comprehensive integration tests for the Claude Code integration
    components (hooks, skills, subagents) working together with the teleological
    array system.
  </objective>

  <rationale>
    Integration testing ensures:
    1. All components work together correctly
    2. Hook-to-skill-to-store data flows are correct
    3. Subagent orchestration functions properly
    4. Performance meets targets under realistic conditions
    5. Error handling and recovery work across components

    This testing phase validates the complete Claude Code integration.
  </rationale>

  <dependencies>
    <dependency type="required">TASK-INTEG-004</dependency>    <!-- Hook protocol -->
    <dependency type="required">TASK-INTEG-005</dependency>    <!-- Edit hooks -->
    <dependency type="required">TASK-INTEG-006</dependency>    <!-- File read hooks -->
    <dependency type="required">TASK-INTEG-007</dependency>    <!-- Bash hooks -->
    <dependency type="required">TASK-INTEG-008</dependency>    <!-- Skills -->
    <dependency type="required">TASK-INTEG-009</dependency>    <!-- Subagents -->
  </dependencies>

  <input_context_files>
    <file purpose="hooks">crates/context-graph-mcp/src/hooks/mod.rs</file>
    <file purpose="skills">crates/context-graph-mcp/src/skills/mod.rs</file>
    <file purpose="agents">crates/context-graph-mcp/src/agents/mod.rs</file>
    <file purpose="store">crates/context-graph-storage/src/teleological/store.rs</file>
  </input_context_files>

  <output_artifacts>
    <artifact type="test">crates/context-graph-mcp/tests/hooks_integration.rs</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/skills_integration.rs</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/agents_integration.rs</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/full_integration.rs</artifact>
    <artifact type="test">tests/claude_code/compatibility_test.rs</artifact>
    <artifact type="bench">crates/context-graph-mcp/benches/integration_bench.rs</artifact>
  </output_artifacts>

  <definition_of_done>
    <criterion id="1">Hook integration tests cover all 10 hook types E2E</criterion>
    <criterion id="2">Skill integration tests verify all 5 skills function correctly</criterion>
    <criterion id="3">Subagent integration tests verify lifecycle and communication</criterion>
    <criterion id="4">Full system tests exercise complete workflows</criterion>
    <criterion id="5">Claude Code compatibility verified with settings.json</criterion>
    <criterion id="6">Performance benchmarks pass all targets</criterion>
    <criterion id="7">Error recovery scenarios tested</criterion>
    <criterion id="8">Test coverage exceeds 80% for integration code</criterion>
  </definition_of_done>

  <estimated_complexity>Medium</estimated_complexity>

  <pseudo_code>
    <section name="HooksIntegrationTests">
```rust
// crates/context-graph-mcp/tests/hooks_integration.rs

use context_graph_mcp::hooks::*;
use context_graph_mcp::test_utils::*;
use context_graph_storage::teleological::TeleologicalStore;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Test harness for hook integration tests
struct HookTestHarness {
    store: Arc<TeleologicalStore>,
    hook_registry: Arc<HookRegistry>,
    pre_task_handler: PreTaskHandler,
    post_task_handler: PostTaskHandler,
    session_handler: SessionHandler,
    pre_file_write_handler: PreFileWriteHandler,
    post_file_write_handler: PostFileWriteHandler,
    pre_file_read_handler: PreFileReadHandler,
    post_file_read_handler: PostFileReadHandler,
    pre_bash_exec_handler: PreBashExecHandler,
    post_bash_exec_handler: PostBashExecHandler,
}

impl HookTestHarness {
    async fn new() -> Self {
        let store = Arc::new(TeleologicalStore::in_memory().await.unwrap());
        let hook_registry = Arc::new(HookRegistry::new());

        // Create all handlers
        Self {
            store: store.clone(),
            hook_registry: hook_registry.clone(),
            pre_task_handler: PreTaskHandler::new(store.clone(), Default::default()),
            post_task_handler: PostTaskHandler::new(store.clone(), Default::default()),
            session_handler: SessionHandler::new(store.clone(), Default::default()),
            pre_file_write_handler: PreFileWriteHandler::new(
                store.clone(),
                Arc::new(TeleologicalAlignmentCalculator::new()),
                Arc::new(TeleologicalDriftDetector::new()),
                Arc::new(EmbedderPipeline::new().await.unwrap()),
                Default::default(),
            ),
            post_file_write_handler: PostFileWriteHandler::new(
                store.clone(),
                Arc::new(EmbedderPipeline::new().await.unwrap()),
                Arc::new(TrajectoryTracker::new()),
                Arc::new(EditPatternExtractor::new()),
                Arc::new(BackgroundTaskQueue::new()),
                Default::default(),
            ),
            pre_file_read_handler: PreFileReadHandler::new(
                store.clone(),
                Arc::new(SearchEngine::new(store.clone())),
                Arc::new(FileAccessTracker::new(store.clone())),
                Default::default(),
            ),
            post_file_read_handler: PostFileReadHandler::new(
                store.clone(),
                Arc::new(EmbedderPipeline::new().await.unwrap()),
                Default::default(),
            ),
            pre_bash_exec_handler: PreBashExecHandler::new(
                Arc::new(CommandSafetyEngine::new()),
                store.clone(),
                Default::default(),
            ),
            post_bash_exec_handler: PostBashExecHandler::new(
                store.clone(),
                Arc::new(EmbedderPipeline::new().await.unwrap()),
                Arc::new(TrajectoryTracker::new()),
                Arc::new(CommandPatternLearner::new(store.clone())),
                Default::default(),
            ),
        }
    }
}

mod session_hooks {
    use super::*;

    #[tokio::test]
    async fn test_session_start_initializes_context() {
        let harness = HookTestHarness::new().await;

        let payload = SessionPayload {
            session_id: "test-session-001".to_string(),
            user_context: Some(serde_json::json!({"project": "test"})),
        };

        let response = harness.session_handler.handle_start(payload).await.unwrap();

        assert_eq!(response.status, HookStatus::Success);
        assert!(response.injected_context.is_some());

        // Verify session was stored
        let stored = harness.store
            .get_session("test-session-001")
            .await
            .unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_session_end_triggers_consolidation() {
        let harness = HookTestHarness::new().await;

        // Start session first
        let start_payload = SessionPayload {
            session_id: "test-session-002".to_string(),
            user_context: None,
        };
        harness.session_handler.handle_start(start_payload).await.unwrap();

        // Inject some memories
        for i in 0..10 {
            harness.store.inject(InjectionRequest {
                content: format!("Test content {}", i),
                memory_type: MemoryType::General,
                namespace: "test".to_string(),
                ..Default::default()
            }).await.unwrap();
        }

        // End session
        let end_response = harness.session_handler
            .handle_end("test-session-002".to_string())
            .await
            .unwrap();

        assert_eq!(end_response.status, HookStatus::Success);

        // Verify consolidation occurred
        let context = end_response.injected_context.unwrap();
        assert!(context.get("consolidated_count").is_some());
    }

    #[tokio::test]
    async fn test_session_restore_loads_context() {
        let harness = HookTestHarness::new().await;

        // Create and end a session
        let session_id = "test-session-003".to_string();
        harness.session_handler.handle_start(SessionPayload {
            session_id: session_id.clone(),
            user_context: Some(serde_json::json!({"key": "value"})),
        }).await.unwrap();
        harness.session_handler.handle_end(session_id.clone()).await.unwrap();

        // Restore session
        let restore_response = harness.session_handler
            .handle_restore(session_id.clone())
            .await
            .unwrap();

        assert_eq!(restore_response.status, HookStatus::Success);
        let context = restore_response.injected_context.unwrap();
        assert_eq!(context.get("key").unwrap(), "value");
    }
}

mod file_write_hooks {
    use super::*;

    #[tokio::test]
    async fn test_pre_file_write_validates_alignment() {
        let harness = HookTestHarness::new().await;

        // Create a goal first
        let goal_array = create_test_teleological_array("authentication system").await;
        harness.store.store_goal(GoalNode {
            goal_id: "goal-001".to_string(),
            label: "Implement authentication".to_string(),
            level: GoalLevel::Strategic,
            teleological_array: goal_array,
            ..Default::default()
        }).await.unwrap();

        // Test aligned write
        let aligned_payload = FileWritePayload {
            file_path: "src/auth/handler.rs".to_string(),
            new_content: "fn verify_token(token: &str) -> Result<Claims, AuthError>".to_string(),
            original_content: None,
            operation: FileOperation::Create,
            source_tool: "Write".to_string(),
        };

        let response = harness.pre_file_write_handler.handle(aligned_payload).await.unwrap();
        assert!(matches!(response.status, HookStatus::Proceed | HookStatus::ProceedWithWarning));

        // Test misaligned write
        let misaligned_payload = FileWritePayload {
            file_path: "src/random/unrelated.rs".to_string(),
            new_content: "fn calculate_tax(amount: f64) -> f64".to_string(),
            original_content: None,
            operation: FileOperation::Create,
            source_tool: "Write".to_string(),
        };

        let response = harness.pre_file_write_handler.handle(misaligned_payload).await.unwrap();
        // Should have warnings about low alignment
        assert!(!response.warnings.is_empty() || response.status == HookStatus::ProceedWithWarning);
    }

    #[tokio::test]
    async fn test_post_file_write_stores_memory() {
        let harness = HookTestHarness::new().await;

        let payload = FileWritePayload {
            file_path: "src/test.rs".to_string(),
            new_content: r#"
                pub fn test_function() {
                    println!("Hello, world!");
                }
            "#.to_string(),
            original_content: None,
            operation: FileOperation::Create,
            source_tool: "Write".to_string(),
        };

        let response = harness.post_file_write_handler.handle(payload).await.unwrap();

        assert_eq!(response.status, HookStatus::Success);
        let context = response.injected_context.unwrap();
        let memory_id = context.get("memory_id").unwrap().as_str().unwrap();

        // Verify memory was stored
        let stored = harness.store.get_memory(memory_id.parse().unwrap()).await.unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_post_file_write_triggers_pattern_learning() {
        let harness = HookTestHarness::new().await;

        // Write with before content for pattern extraction
        let payload = FileWritePayload {
            file_path: "src/refactor.rs".to_string(),
            new_content: r#"
                pub fn get_user_by_id(id: UserId) -> Result<User, Error> {
                    self.repository.find_by_id(id)
                }
            "#.to_string(),
            original_content: Some(r#"
                pub fn get_user(id: i64) -> User {
                    self.users.get(&id).unwrap()
                }
            "#.to_string()),
            operation: FileOperation::Update,
            source_tool: "Edit".to_string(),
        };

        let response = harness.post_file_write_handler.handle(payload).await.unwrap();

        assert_eq!(response.status, HookStatus::Success);
        let context = response.injected_context.unwrap();
        assert!(context.get("pattern_training_queued").is_some());
    }

    #[tokio::test]
    async fn test_pre_file_write_latency_under_threshold() {
        let harness = HookTestHarness::new().await;

        let payload = FileWritePayload {
            file_path: "src/test.rs".to_string(),
            new_content: "fn test() {}".to_string(),
            original_content: None,
            operation: FileOperation::Create,
            source_tool: "Write".to_string(),
        };

        let start = std::time::Instant::now();
        let response = harness.pre_file_write_handler.handle(payload).await.unwrap();
        let elapsed = start.elapsed();

        // Pre-write should be under 100ms
        assert!(elapsed.as_millis() < 100, "Pre-write took {}ms", elapsed.as_millis());
        assert!(response.metrics.latency_ms < 100);
    }
}

mod file_read_hooks {
    use super::*;

    #[tokio::test]
    async fn test_pre_file_read_injects_context() {
        let harness = HookTestHarness::new().await;

        // Store some related content first
        harness.store.inject(InjectionRequest {
            content: "Authentication handler for JWT tokens".to_string(),
            memory_type: MemoryType::CodeContext,
            namespace: "auth".to_string(),
            metadata: serde_json::json!({"file_path": "src/auth/jwt.rs"}),
            ..Default::default()
        }).await.unwrap();

        let payload = FileReadPayload {
            file_path: "src/auth/handler.rs".to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };

        let response = harness.pre_file_read_handler.handle(payload).await.unwrap();

        assert_eq!(response.status, HookStatus::Proceed);
        if let Some(context) = response.injected_context {
            // Should find related memories
            let related = context.get("related_memories").and_then(|r| r.as_array());
            // May or may not find related content depending on embedding similarity
        }
    }

    #[tokio::test]
    async fn test_post_file_read_embeds_content() {
        let harness = HookTestHarness::new().await;

        let payload = PostFileReadPayload {
            file_path: "src/test.rs".to_string(),
            content: r#"
                /// Test module for authentication
                pub mod auth {
                    pub fn verify(token: &str) -> bool {
                        !token.is_empty()
                    }
                }
            "#.to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };

        let response = harness.post_file_read_handler.handle(payload).await.unwrap();

        assert_eq!(response.status, HookStatus::Success);
        let context = response.injected_context.unwrap();
        assert!(context.get("memory_id").is_some());
        assert_eq!(context.get("embedders_generated").unwrap(), 13);
    }

    #[tokio::test]
    async fn test_post_file_read_deduplicates() {
        let harness = HookTestHarness::new().await;

        let content = "fn duplicate_content() {}".to_string();

        // First read
        let payload1 = PostFileReadPayload {
            file_path: "src/test.rs".to_string(),
            content: content.clone(),
            line_range: None,
            source_tool: "Read".to_string(),
        };
        let response1 = harness.post_file_read_handler.handle(payload1).await.unwrap();
        let memory_id1 = response1.injected_context.unwrap()
            .get("memory_id").unwrap().as_str().unwrap().to_string();

        // Second read of same content
        let payload2 = PostFileReadPayload {
            file_path: "src/test.rs".to_string(),
            content,
            line_range: None,
            source_tool: "Read".to_string(),
        };
        let response2 = harness.post_file_read_handler.handle(payload2).await.unwrap();

        // Should be deduplicated
        let context = response2.injected_context.unwrap();
        assert_eq!(context.get("deduplicated").unwrap(), true);
    }

    #[tokio::test]
    async fn test_pre_file_read_latency_under_threshold() {
        let harness = HookTestHarness::new().await;

        let payload = FileReadPayload {
            file_path: "src/test.rs".to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };

        let start = std::time::Instant::now();
        let response = harness.pre_file_read_handler.handle(payload).await.unwrap();
        let elapsed = start.elapsed();

        // Pre-read should be under 50ms
        assert!(elapsed.as_millis() < 50, "Pre-read took {}ms", elapsed.as_millis());
    }
}

mod bash_hooks {
    use super::*;

    #[tokio::test]
    async fn test_pre_bash_blocks_dangerous_commands() {
        let harness = HookTestHarness::new().await;

        // Dangerous command
        let payload = BashExecPayload {
            command: "rm -rf /".to_string(),
            working_dir: None,
            env_vars: None,
            timeout_ms: None,
        };

        let response = harness.pre_bash_exec_handler.handle(payload).await.unwrap();
        assert_eq!(response.status, HookStatus::Block);
        assert!(!response.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_pre_bash_allows_safe_commands() {
        let harness = HookTestHarness::new().await;

        let payload = BashExecPayload {
            command: "cargo test".to_string(),
            working_dir: None,
            env_vars: None,
            timeout_ms: None,
        };

        let response = harness.pre_bash_exec_handler.handle(payload).await.unwrap();
        assert!(matches!(response.status, HookStatus::Proceed | HookStatus::ProceedWithWarning));
    }

    #[tokio::test]
    async fn test_pre_bash_warns_on_medium_risk() {
        let harness = HookTestHarness::new().await;

        let payload = BashExecPayload {
            command: "git push --force".to_string(),
            working_dir: None,
            env_vars: None,
            timeout_ms: None,
        };

        let response = harness.pre_bash_exec_handler.handle(payload).await.unwrap();
        assert_eq!(response.status, HookStatus::ProceedWithWarning);
        assert!(response.warnings.iter().any(|w| w.contains("force")));
    }

    #[tokio::test]
    async fn test_post_bash_stores_output() {
        let harness = HookTestHarness::new().await;

        let payload = PostBashExecPayload {
            command: "cargo test".to_string(),
            exit_code: 0,
            stdout: "running 5 tests\ntest result: ok. 5 passed".to_string(),
            stderr: "".to_string(),
            duration_ms: 1500,
            working_dir: Some("/project".to_string()),
        };

        let response = harness.post_bash_exec_handler.handle(payload).await.unwrap();

        assert_eq!(response.status, HookStatus::Success);
        let context = response.injected_context.unwrap();
        assert!(context.get("memory_id").is_some());
        assert_eq!(context.get("success").unwrap(), true);
    }

    #[tokio::test]
    async fn test_post_bash_extracts_error_patterns() {
        let harness = HookTestHarness::new().await;

        let payload = PostBashExecPayload {
            command: "cargo build".to_string(),
            exit_code: 1,
            stdout: "".to_string(),
            stderr: "error[E0425]: cannot find value `foo` in this scope".to_string(),
            duration_ms: 500,
            working_dir: None,
        };

        let response = harness.post_bash_exec_handler.handle(payload).await.unwrap();

        let context = response.injected_context.unwrap();
        assert!(context.get("error_patterns").is_some());
        // Should include troubleshooting suggestions
    }

    #[tokio::test]
    async fn test_pre_bash_latency_under_threshold() {
        let harness = HookTestHarness::new().await;

        let payload = BashExecPayload {
            command: "echo hello".to_string(),
            working_dir: None,
            env_vars: None,
            timeout_ms: None,
        };

        let start = std::time::Instant::now();
        let response = harness.pre_bash_exec_handler.handle(payload).await.unwrap();
        let elapsed = start.elapsed();

        // Pre-bash should be under 20ms
        assert!(elapsed.as_millis() < 20, "Pre-bash took {}ms", elapsed.as_millis());
    }
}

mod hook_chaining {
    use super::*;

    #[tokio::test]
    async fn test_session_start_to_file_read_chain() {
        let harness = HookTestHarness::new().await;

        // 1. Start session
        let session_response = harness.session_handler.handle_start(SessionPayload {
            session_id: "chain-test-001".to_string(),
            user_context: None,
        }).await.unwrap();
        assert_eq!(session_response.status, HookStatus::Success);

        // 2. Read a file
        let read_payload = FileReadPayload {
            file_path: "src/main.rs".to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };
        let read_response = harness.pre_file_read_handler.handle(read_payload).await.unwrap();
        assert_eq!(read_response.status, HookStatus::Proceed);

        // 3. Post read to store
        let post_read = PostFileReadPayload {
            file_path: "src/main.rs".to_string(),
            content: "fn main() { println!(\"Hello\"); }".to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };
        let post_read_response = harness.post_file_read_handler.handle(post_read).await.unwrap();
        assert_eq!(post_read_response.status, HookStatus::Success);

        // Verify memory was stored in session context
        let memory_id = post_read_response.injected_context.unwrap()
            .get("memory_id").unwrap().as_str().unwrap().to_string();

        let session_memories = harness.store
            .get_session_memories("chain-test-001")
            .await
            .unwrap();
        // Memory should be associated with session
    }

    #[tokio::test]
    async fn test_file_write_to_goal_alignment_chain() {
        let harness = HookTestHarness::new().await;

        // 1. Create a goal
        let goal_array = create_test_teleological_array("user authentication").await;
        harness.store.store_goal(GoalNode {
            goal_id: "chain-goal-001".to_string(),
            label: "Implement user auth".to_string(),
            level: GoalLevel::Strategic,
            teleological_array: goal_array,
            ..Default::default()
        }).await.unwrap();

        // 2. Pre-write check
        let pre_write = FileWritePayload {
            file_path: "src/auth/login.rs".to_string(),
            new_content: "pub fn login(credentials: Credentials) -> Result<Token, AuthError>".to_string(),
            original_content: None,
            operation: FileOperation::Create,
            source_tool: "Write".to_string(),
        };
        let pre_response = harness.pre_file_write_handler.handle(pre_write.clone()).await.unwrap();

        // Should check alignment
        let context = pre_response.injected_context.unwrap();
        assert!(context.get("alignment_scores").is_some());

        // 3. Post-write storage
        let post_response = harness.post_file_write_handler.handle(pre_write).await.unwrap();
        assert_eq!(post_response.status, HookStatus::Success);
    }
}

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_hook_timeout_handling() {
        let harness = HookTestHarness::new().await;

        // Configure very short timeout
        let mut config = PreFileReadConfig::default();
        config.max_preload_latency_ms = 1; // 1ms timeout

        let handler = PreFileReadHandler::new(
            harness.store.clone(),
            Arc::new(SearchEngine::new(harness.store.clone())),
            Arc::new(FileAccessTracker::new(harness.store.clone())),
            config,
        );

        let payload = FileReadPayload {
            file_path: "src/test.rs".to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };

        // Should complete without error even with timeout
        let response = handler.handle(payload).await.unwrap();
        assert_eq!(response.status, HookStatus::Proceed);
    }

    #[tokio::test]
    async fn test_hook_graceful_degradation() {
        // Test that hooks degrade gracefully when store is unavailable
        let store = Arc::new(TeleologicalStore::in_memory().await.unwrap());

        // Simulate store being temporarily unavailable by dropping it
        // In practice, handlers should catch errors and return degraded responses

        let payload = FileReadPayload {
            file_path: "src/test.rs".to_string(),
            line_range: None,
            source_tool: "Read".to_string(),
        };

        // Handler should not panic, should return safe default
    }
}
```
    </section>

    <section name="SkillsIntegrationTests">
```rust
// crates/context-graph-mcp/tests/skills_integration.rs

use context_graph_mcp::skills::*;
use context_graph_mcp::test_utils::*;

struct SkillTestHarness {
    skill_loader: Arc<SkillLoader>,
    mcp_client: Arc<MockMcpClient>,
    store: Arc<TeleologicalStore>,
}

impl SkillTestHarness {
    async fn new() -> Self {
        let store = Arc::new(TeleologicalStore::in_memory().await.unwrap());
        let mcp_client = Arc::new(MockMcpClient::new(store.clone()));
        let skills_dir = PathBuf::from(".claude/skills");
        let skill_loader = Arc::new(SkillLoader::new(skills_dir, mcp_client.clone()));

        Self {
            skill_loader,
            mcp_client,
            store,
        }
    }
}

#[tokio::test]
async fn test_skill_loader_discovers_skills() {
    let harness = SkillTestHarness::new().await;

    let count = harness.skill_loader.load_all().await.unwrap();

    // Should find at least the 5 core skills
    assert!(count >= 5, "Expected at least 5 skills, found {}", count);

    let skills = harness.skill_loader.list().await;
    assert!(skills.contains(&"memory-search".to_string()));
    assert!(skills.contains(&"goal-alignment".to_string()));
    assert!(skills.contains(&"pattern-learning".to_string()));
    assert!(skills.contains(&"context-injection".to_string()));
    assert!(skills.contains(&"drift-check".to_string()));
}

#[tokio::test]
async fn test_memory_search_skill() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    // Inject some test content
    harness.store.inject(InjectionRequest {
        content: "JWT token validation and refresh logic".to_string(),
        memory_type: MemoryType::CodeContext,
        namespace: "auth".to_string(),
        ..Default::default()
    }).await.unwrap();

    // Invoke skill
    let result = harness.skill_loader.invoke(
        "memory-search",
        serde_json::json!({
            "query": "authentication tokens",
            "limit": 5
        }),
    ).await.unwrap();

    assert!(result.success);
    let data = result.data;
    assert!(data.get("found").is_some());
}

#[tokio::test]
async fn test_goal_alignment_skill() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    // Create a goal
    let goal_array = create_test_teleological_array("implement caching").await;
    harness.store.store_goal(GoalNode {
        goal_id: "cache-goal-001".to_string(),
        label: "Implement caching layer".to_string(),
        level: GoalLevel::Strategic,
        teleological_array: goal_array,
        ..Default::default()
    }).await.unwrap();

    // Check alignment
    let result = harness.skill_loader.invoke(
        "goal-alignment",
        serde_json::json!({
            "content": "Adding Redis cache for session storage",
            "goal_id": "cache-goal-001"
        }),
    ).await.unwrap();

    assert!(result.success);
    let data = result.data;
    assert!(data.get("score").is_some());
    assert!(data.get("status").is_some());
}

#[tokio::test]
async fn test_pattern_learning_skill() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    let result = harness.skill_loader.invoke(
        "pattern-learning",
        serde_json::json!({
            "pattern_type": "code",
            "content": "pub fn handle_error<E: Error>(e: E) -> Response { ... }",
            "context": {
                "language": "rust",
                "framework": "actix-web"
            },
            "tags": ["error-handling", "web"]
        }),
    ).await.unwrap();

    assert!(result.success);
    let data = result.data;
    assert!(data.get("patternId").is_some());
}

#[tokio::test]
async fn test_context_injection_skill() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    let result = harness.skill_loader.invoke(
        "context-injection",
        serde_json::json!({
            "content": "Important context about the authentication flow",
            "memory_type": "documentation",
            "namespace": "auth"
        }),
    ).await.unwrap();

    assert!(result.success);
    let data = result.data;
    assert!(data.get("memoryId").is_some());
    assert_eq!(data.get("embeddings").unwrap(), 13);
}

#[tokio::test]
async fn test_drift_check_skill() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    // Create goal and memories
    let goal_array = create_test_teleological_array("payment processing").await;
    harness.store.store_goal(GoalNode {
        goal_id: "payment-goal-001".to_string(),
        label: "Implement payment processing".to_string(),
        level: GoalLevel::Strategic,
        teleological_array: goal_array,
        ..Default::default()
    }).await.unwrap();

    let memory_id = harness.store.inject(InjectionRequest {
        content: "Shopping cart implementation".to_string(),
        memory_type: MemoryType::CodeContext,
        namespace: "commerce".to_string(),
        ..Default::default()
    }).await.unwrap();

    let result = harness.skill_loader.invoke(
        "drift-check",
        serde_json::json!({
            "memory_ids": [memory_id.to_string()],
            "goal_id": "payment-goal-001"
        }),
    ).await.unwrap();

    assert!(result.success);
    let data = result.data;
    assert!(data.get("status").is_some());
    assert!(data.get("hasDrifted").is_some());
}

#[tokio::test]
async fn test_skill_error_handling_and_retry() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    // Configure mock to fail first two times
    harness.mcp_client.set_failure_count(2);

    let result = harness.skill_loader.invoke(
        "memory-search",
        serde_json::json!({
            "query": "test query"
        }),
    ).await;

    // Should succeed after retries
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_skill_parameter_validation() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    // Missing required parameter
    let result = harness.skill_loader.invoke(
        "memory-search",
        serde_json::json!({}), // Missing 'query'
    ).await;

    assert!(result.is_err());
    match result {
        Err(SkillError::MissingParam(param)) => {
            assert_eq!(param, "query");
        }
        _ => panic!("Expected MissingParam error"),
    }
}

#[tokio::test]
async fn test_skill_default_parameters() {
    let harness = SkillTestHarness::new().await;
    harness.skill_loader.load_all().await.unwrap();

    // Only provide required parameter
    let result = harness.skill_loader.invoke(
        "memory-search",
        serde_json::json!({
            "query": "test"
        }),
    ).await.unwrap();

    // Should use defaults for limit, threshold, etc.
    assert!(result.success);
}
```
    </section>

    <section name="AgentsIntegrationTests">
```rust
// crates/context-graph-mcp/tests/agents_integration.rs

use context_graph_mcp::agents::*;
use context_graph_mcp::test_utils::*;

struct AgentTestHarness {
    orchestrator: Arc<SubagentOrchestrator>,
    store: Arc<TeleologicalStore>,
}

impl AgentTestHarness {
    async fn new() -> Self {
        let store = Arc::new(TeleologicalStore::in_memory().await.unwrap());
        let skill_loader = Arc::new(SkillLoader::new(
            PathBuf::from(".claude/skills"),
            Arc::new(MockMcpClient::new(store.clone())),
        ));

        let orchestrator = Arc::new(SubagentOrchestrator::new(
            store.clone(),
            skill_loader,
            OrchestratorConfig {
                agents_dir: PathBuf::from(".claude/agents"),
                ..Default::default()
            },
        ));

        Self { orchestrator, store }
    }
}

#[tokio::test]
async fn test_agent_lifecycle_start_stop() {
    let harness = AgentTestHarness::new().await;

    // Start agent
    let agent_id = harness.orchestrator.start_agent("goal-tracker").await.unwrap();
    assert!(!agent_id.is_empty());

    // Check status
    let status = harness.orchestrator.get_status("goal-tracker").await.unwrap();
    assert!(matches!(status.status, AgentStatus::Running | AgentStatus::Starting));

    // Stop agent
    harness.orchestrator.stop_agent("goal-tracker").await.unwrap();

    // Should no longer be running
    let result = harness.orchestrator.get_status("goal-tracker").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_agent_list_available() {
    let harness = AgentTestHarness::new().await;

    let definitions = harness.orchestrator.load_definitions().await.unwrap();

    // Should find all 4 agents
    assert!(definitions.len() >= 4);

    let names: Vec<_> = definitions.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"goal-tracker"));
    assert!(names.contains(&"context-curator"));
    assert!(names.contains(&"pattern-miner"));
    assert!(names.contains(&"learning-coach"));
}

#[tokio::test]
async fn test_inter_agent_communication() {
    let harness = AgentTestHarness::new().await;

    // Start two agents
    harness.orchestrator.start_agent("goal-tracker").await.unwrap();
    harness.orchestrator.start_agent("context-curator").await.unwrap();

    // Wait for startup
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send message from goal-tracker to context-curator
    let message = AgentMessage {
        message_id: Uuid::new_v4(),
        from: "goal-tracker".to_string(),
        to: "context-curator".to_string(),
        message_type: MessageType::Query,
        payload: serde_json::json!({"query": "recent file changes"}),
        timestamp: Utc::now(),
    };

    harness.orchestrator.send_message("context-curator", message).await.unwrap();

    // Clean up
    harness.orchestrator.stop_agent("goal-tracker").await.unwrap();
    harness.orchestrator.stop_agent("context-curator").await.unwrap();
}

#[tokio::test]
async fn test_agent_health_monitoring() {
    let harness = AgentTestHarness::new().await;

    // Start an agent
    harness.orchestrator.start_agent("pattern-miner").await.unwrap();

    // Check health
    let health = harness.orchestrator.health_check().await;
    assert!(!health.is_empty());

    let agent_health = health.iter().find(|h| h.name == "pattern-miner").unwrap();
    assert!(agent_health.healthy);
    assert!(agent_health.issues.is_empty());

    harness.orchestrator.stop_agent("pattern-miner").await.unwrap();
}

#[tokio::test]
async fn test_max_agents_limit() {
    let harness = AgentTestHarness::new().await;

    // Try to start more agents than allowed
    for agent in &["goal-tracker", "context-curator", "pattern-miner", "learning-coach"] {
        let _ = harness.orchestrator.start_agent(agent).await;
    }

    // Depending on max_agents setting, some may fail
    let running = harness.orchestrator.list_agents().await;
    assert!(running.len() <= 10); // Default max

    // Clean up
    for info in running {
        let _ = harness.orchestrator.stop_agent(&info.name).await;
    }
}

#[tokio::test]
async fn test_agent_store_integration() {
    let harness = AgentTestHarness::new().await;

    // Start goal-tracker
    harness.orchestrator.start_agent("goal-tracker").await.unwrap();

    // Inject a memory
    harness.store.inject(InjectionRequest {
        content: "New authentication feature completed".to_string(),
        memory_type: MemoryType::CodeContext,
        namespace: "auth".to_string(),
        ..Default::default()
    }).await.unwrap();

    // Goal tracker should be able to search and analyze
    // (In practice, this would happen via message passing)

    harness.orchestrator.stop_agent("goal-tracker").await.unwrap();
}

#[tokio::test]
async fn test_agent_skill_invocation() {
    let harness = AgentTestHarness::new().await;

    // Start context-curator which uses memory-search skill
    harness.orchestrator.start_agent("context-curator").await.unwrap();

    // Send task message
    let message = AgentMessage {
        message_id: Uuid::new_v4(),
        from: "test".to_string(),
        to: "context-curator".to_string(),
        message_type: MessageType::Task,
        payload: serde_json::json!({
            "skill": "memory-search",
            "query": "authentication"
        }),
        timestamp: Utc::now(),
    };

    harness.orchestrator.send_message("context-curator", message).await.unwrap();

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    harness.orchestrator.stop_agent("context-curator").await.unwrap();
}
```
    </section>

    <section name="FullIntegrationTests">
```rust
// crates/context-graph-mcp/tests/full_integration.rs

use context_graph_mcp::*;

/// Full system integration test
#[tokio::test]
async fn test_complete_session_workflow() {
    let system = IntegrationTestSystem::new().await;

    // 1. Start session
    let session_id = system.start_session().await.unwrap();

    // 2. Read some files (triggers hooks)
    let file_content = system.read_file("src/auth/handler.rs").await.unwrap();
    assert!(system.memory_count().await > 0);

    // 3. Write a file (triggers alignment check and storage)
    system.write_file(
        "src/auth/validator.rs",
        "pub fn validate(token: &str) -> bool { !token.is_empty() }",
    ).await.unwrap();

    // 4. Run a command (triggers safety check and learning)
    let output = system.run_command("cargo test --lib").await.unwrap();

    // 5. Check goal alignment
    let alignment = system.check_alignment("authentication").await.unwrap();
    assert!(alignment.score > 0.0);

    // 6. End session (triggers consolidation)
    system.end_session(session_id).await.unwrap();

    // Verify consolidation occurred
    let consolidated_count = system.consolidated_memory_count().await;
    assert!(consolidated_count > 0);
}

#[tokio::test]
async fn test_hook_skill_agent_interaction() {
    let system = IntegrationTestSystem::new().await;

    // 1. Start goal-tracker agent
    system.start_agent("goal-tracker").await.unwrap();

    // 2. Create a goal via skill
    system.invoke_skill("context-injection", serde_json::json!({
        "content": "Implement user authentication with OAuth2",
        "memory_type": "documentation"
    })).await.unwrap();

    // 3. Discover goals (should find the one we just created)
    system.trigger_goal_discovery().await.unwrap();

    // 4. Write code (triggers alignment via hook)
    system.write_file(
        "src/oauth/client.rs",
        "pub struct OAuthClient { /* ... */ }",
    ).await.unwrap();

    // 5. Agent should detect and track alignment
    let status = system.get_agent_status("goal-tracker").await.unwrap();
    assert_eq!(status.status, AgentStatus::Running);

    system.stop_agent("goal-tracker").await.unwrap();
}

#[tokio::test]
async fn test_error_recovery_scenario() {
    let system = IntegrationTestSystem::new().await;

    // 1. Start with a session
    let session_id = system.start_session().await.unwrap();

    // 2. Simulate store becoming temporarily unavailable
    system.simulate_store_failure().await;

    // 3. Try operations - should degrade gracefully
    let read_result = system.read_file("src/test.rs").await;
    // Should not panic, should return degraded response

    // 4. Restore store
    system.restore_store().await;

    // 5. Operations should resume
    system.write_file("src/test.rs", "fn test() {}").await.unwrap();

    system.end_session(session_id).await.unwrap();
}

#[tokio::test]
async fn test_learning_feedback_loop() {
    let system = IntegrationTestSystem::new().await;

    // 1. Create initial pattern
    for i in 0..5 {
        system.write_file(
            &format!("src/handlers/handler{}.rs", i),
            &format!("pub fn handle_{i}(req: Request) -> Response {{ /* ... */ }}"),
        ).await.unwrap();
    }

    // 2. Run pattern mining
    system.start_agent("pattern-miner").await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 3. Check if patterns were discovered
    let patterns = system.search_patterns("handler").await.unwrap();
    // Should find the handler pattern

    system.stop_agent("pattern-miner").await.unwrap();
}
```
    </section>

    <section name="PerformanceBenchmarks">
```rust
// crates/context-graph-mcp/benches/integration_bench.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use context_graph_mcp::*;

fn hook_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("Hooks");

    // Pre-file-read benchmark
    group.bench_function("pre_file_read", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_hook_harness().await;
                harness.pre_file_read_handler.handle(FileReadPayload {
                    file_path: "src/test.rs".to_string(),
                    line_range: None,
                    source_tool: "Read".to_string(),
                }).await.unwrap()
            })
        })
    });

    // Pre-file-write benchmark
    group.bench_function("pre_file_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_hook_harness().await;
                harness.pre_file_write_handler.handle(FileWritePayload {
                    file_path: "src/test.rs".to_string(),
                    new_content: "fn test() {}".to_string(),
                    original_content: None,
                    operation: FileOperation::Create,
                    source_tool: "Write".to_string(),
                }).await.unwrap()
            })
        })
    });

    // Pre-bash-exec benchmark
    group.bench_function("pre_bash_exec", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_hook_harness().await;
                harness.pre_bash_exec_handler.handle(BashExecPayload {
                    command: "cargo test".to_string(),
                    working_dir: None,
                    env_vars: None,
                    timeout_ms: None,
                }).await.unwrap()
            })
        })
    });

    group.finish();
}

fn skill_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("Skills");

    // Memory search skill
    group.bench_function("memory_search_skill", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_skill_harness().await;
                harness.skill_loader.invoke(
                    "memory-search",
                    serde_json::json!({"query": "authentication"}),
                ).await.unwrap()
            })
        })
    });

    // Goal alignment skill
    group.bench_function("goal_alignment_skill", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_skill_harness().await;
                harness.skill_loader.invoke(
                    "goal-alignment",
                    serde_json::json!({
                        "content": "test content",
                        "goal_id": "test-goal"
                    }),
                ).await.unwrap()
            })
        })
    });

    group.finish();
}

fn agent_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("Agents");

    // Agent start time
    group.bench_function("agent_start", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_agent_harness().await;
                let id = harness.orchestrator.start_agent("goal-tracker").await.unwrap();
                harness.orchestrator.stop_agent("goal-tracker").await.unwrap();
                id
            })
        })
    });

    // Message send time
    group.bench_function("agent_message", |b| {
        b.iter(|| {
            rt.block_on(async {
                let harness = create_agent_harness().await;
                harness.orchestrator.start_agent("context-curator").await.unwrap();

                let message = AgentMessage {
                    message_id: Uuid::new_v4(),
                    from: "test".to_string(),
                    to: "context-curator".to_string(),
                    message_type: MessageType::Query,
                    payload: serde_json::json!({}),
                    timestamp: Utc::now(),
                };

                harness.orchestrator.send_message("context-curator", message).await.unwrap();
                harness.orchestrator.stop_agent("context-curator").await.unwrap();
            })
        })
    });

    group.finish();
}

fn assert_performance_targets(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Performance targets from spec
    let targets = [
        ("hook_execution", 50_u64),       // <50ms
        ("skill_invocation", 100_u64),    // <100ms
        ("agent_response", 500_u64),      // <500ms
        ("store_operation", 10_u64),      // <10ms
    ];

    let mut group = c.benchmark_group("Performance Targets");

    for (name, target_ms) in targets {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let start = std::time::Instant::now();

                    match name {
                        "hook_execution" => {
                            let harness = create_hook_harness().await;
                            harness.pre_file_read_handler.handle(FileReadPayload {
                                file_path: "test.rs".to_string(),
                                line_range: None,
                                source_tool: "Read".to_string(),
                            }).await.unwrap();
                        }
                        "skill_invocation" => {
                            let harness = create_skill_harness().await;
                            harness.skill_loader.invoke(
                                "context-injection",
                                serde_json::json!({"content": "test"}),
                            ).await.unwrap();
                        }
                        "agent_response" => {
                            // Agent response time
                        }
                        "store_operation" => {
                            let store = TeleologicalStore::in_memory().await.unwrap();
                            store.inject(InjectionRequest {
                                content: "test".to_string(),
                                ..Default::default()
                            }).await.unwrap();
                        }
                        _ => {}
                    }

                    let elapsed = start.elapsed().as_millis() as u64;
                    assert!(
                        elapsed < target_ms,
                        "{} took {}ms, target is {}ms",
                        name, elapsed, target_ms
                    );
                })
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    hook_benchmarks,
    skill_benchmarks,
    agent_benchmarks,
    assert_performance_targets,
);
criterion_main!(benches);
```
    </section>
  </pseudo_code>
</task_spec>
```

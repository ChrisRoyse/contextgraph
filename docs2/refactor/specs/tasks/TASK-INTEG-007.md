# TASK-INTEG-007: Bash Execution Hooks Implementation

```xml
<task_spec>
  <task_id>TASK-INTEG-007</task_id>
  <title>Bash Execution Hooks (PreBashExec, PostBashExec)</title>
  <status>pending</status>

  <objective>
    Implement PreBashExec and PostBashExec hook handlers that provide safety
    checks before command execution and learning from command outputs for
    the teleological array system.
  </objective>

  <rationale>
    Bash execution hooks enable:
    1. PreBashExec: Safety validation and context injection before commands
    2. PostBashExec: Learning from command outputs and error patterns
    3. Command pattern recognition: Build understanding of useful command sequences
    4. Risk mitigation: Warn or block potentially dangerous commands

    These hooks protect the system while enabling learning from shell interactions.
  </rationale>

  <dependencies>
    <dependency type="required">TASK-INTEG-004</dependency>    <!-- Hook protocol -->
    <dependency type="required">TASK-CORE-003</dependency>     <!-- TeleologicalArray type -->
    <dependency type="required">TASK-LOGIC-006</dependency>    <!-- Trajectory tracking -->
  </dependencies>

  <input_context_files>
    <file purpose="hook_protocol">crates/context-graph-mcp/src/hooks/protocol.rs</file>
    <file purpose="trajectory_tracking">crates/context-graph-core/src/autonomous/trajectory.rs</file>
    <file purpose="embedder_pipeline">crates/context-graph-core/src/teleology/embedder.rs</file>
    <file purpose="store">crates/context-graph-storage/src/teleological/store.rs</file>
  </input_context_files>

  <output_artifacts>
    <artifact type="source">crates/context-graph-mcp/src/hooks/bash_exec.rs</artifact>
    <artifact type="config">.claude/hooks/pre_bash_exec.sh</artifact>
    <artifact type="config">.claude/hooks/post_bash_exec.sh</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/bash_hooks_test.rs</artifact>
  </output_artifacts>

  <definition_of_done>
    <criterion id="1">PreBashExec handler validates commands against safety rules</criterion>
    <criterion id="2">PostBashExec handler stores command outputs for learning</criterion>
    <criterion id="3">Dangerous command patterns detected and warned/blocked</criterion>
    <criterion id="4">Command sequences tracked for pattern learning</criterion>
    <criterion id="5">Error outputs analyzed for troubleshooting context</criterion>
    <criterion id="6">Hook latency under 20ms for pre-execution checks</criterion>
    <criterion id="7">Configurable safety levels (permissive, standard, strict)</criterion>
    <criterion id="8">Shell scripts integrate with Claude Code bash workflow</criterion>
  </definition_of_done>

  <estimated_complexity>Medium</estimated_complexity>

  <pseudo_code>
    <section name="PreBashExecHandler">
```rust
// crates/context-graph-mcp/src/hooks/bash_exec.rs

use crate::hooks::protocol::{HookEvent, HookPayload, HookResponse, HookStatus};
use std::sync::Arc;
use regex::Regex;

/// Pre-bash-execution hook handler for safety checks
pub struct PreBashExecHandler {
    /// Safety rule engine
    safety_engine: Arc<CommandSafetyEngine>,
    /// Teleological store for context
    store: Arc<TeleologicalStore>,
    /// Configuration
    config: PreBashExecConfig,
}

#[derive(Debug, Clone)]
pub struct PreBashExecConfig {
    /// Safety level (permissive, standard, strict)
    pub safety_level: SafetyLevel,
    /// Maximum command length to analyze
    pub max_command_length: usize,
    /// Whether to inject context suggestions
    pub inject_context: bool,
    /// Maximum latency for safety checks (ms)
    pub max_latency_ms: u64,
    /// Custom blocked patterns
    pub blocked_patterns: Vec<String>,
    /// Custom allowed patterns (override blocks)
    pub allowed_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    /// Minimal checks, mostly informational
    Permissive,
    /// Standard checks, warn on risky commands
    Standard,
    /// Strict checks, block risky commands
    Strict,
}

impl Default for PreBashExecConfig {
    fn default() -> Self {
        Self {
            safety_level: SafetyLevel::Standard,
            max_command_length: 10_000,
            inject_context: true,
            max_latency_ms: 20,
            blocked_patterns: vec![],
            allowed_patterns: vec![],
        }
    }
}

/// Payload for bash execution hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashExecPayload {
    /// Command to execute
    pub command: String,
    /// Working directory
    pub working_dir: Option<String>,
    /// Environment variables set
    pub env_vars: Option<HashMap<String, String>>,
    /// Timeout setting (ms)
    pub timeout_ms: Option<u64>,
}

/// Command analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAnalysis {
    /// Overall risk level
    pub risk_level: RiskLevel,
    /// Detected command type
    pub command_type: CommandType,
    /// Safety warnings
    pub warnings: Vec<SafetyWarning>,
    /// Suggested alternatives (if risky)
    pub alternatives: Vec<String>,
    /// Related context from previous commands
    pub related_context: Vec<CommandContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    FileSystem,
    Git,
    Package,
    Build,
    Test,
    Network,
    System,
    Docker,
    Database,
    Custom,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyWarning {
    pub level: RiskLevel,
    pub message: String,
    pub pattern_matched: Option<String>,
    pub mitigation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContext {
    pub previous_command: String,
    pub output_summary: String,
    pub timestamp: DateTime<Utc>,
    pub relevance_score: f32,
}

impl PreBashExecHandler {
    pub fn new(
        safety_engine: Arc<CommandSafetyEngine>,
        store: Arc<TeleologicalStore>,
        config: PreBashExecConfig,
    ) -> Self {
        Self {
            safety_engine,
            store,
            config,
        }
    }

    /// Handle PreBashExec event
    pub async fn handle(&self, payload: BashExecPayload) -> Result<HookResponse, HookError> {
        let start = Instant::now();

        // 1. Quick command length check
        if payload.command.len() > self.config.max_command_length {
            return Ok(HookResponse {
                status: HookStatus::ProceedWithWarning,
                message: Some("Command exceeds maximum analyzable length".to_string()),
                warnings: vec!["Command too long for full safety analysis".to_string()],
                injected_context: None,
                metrics: HookMetrics::quick(start.elapsed().as_millis() as u64),
            });
        }

        // 2. Analyze command safety
        let analysis = self.safety_engine.analyze(&payload.command, &self.config)?;

        // 3. Determine action based on safety level and risk
        let (status, should_block) = self.determine_action(&analysis);

        // 4. Fetch related context if enabled and time permits
        let related_context = if self.config.inject_context
            && start.elapsed().as_millis() < (self.config.max_latency_ms / 2) as u128 {
            self.fetch_related_context(&payload.command).await.unwrap_or_default()
        } else {
            vec![]
        };

        // 5. Build response
        let warnings: Vec<String> = analysis.warnings
            .iter()
            .map(|w| format!("[{}] {}", w.level.display(), w.message))
            .collect();

        let mut response = HookResponse {
            status,
            message: self.build_status_message(&analysis, should_block),
            warnings,
            injected_context: Some(serde_json::json!({
                "command_type": analysis.command_type,
                "risk_level": analysis.risk_level,
                "alternatives": analysis.alternatives,
                "related_context": related_context,
            })),
            metrics: HookMetrics {
                latency_ms: start.elapsed().as_millis() as u64,
                embedders_used: 0,
                goals_checked: 0,
            },
        };

        // 6. Add alternatives if command is risky
        if !analysis.alternatives.is_empty() && analysis.risk_level >= RiskLevel::Medium {
            response.message = Some(format!(
                "{}. Consider: {}",
                response.message.unwrap_or_default(),
                analysis.alternatives.join(" or ")
            ));
        }

        Ok(response)
    }

    fn determine_action(&self, analysis: &CommandAnalysis) -> (HookStatus, bool) {
        match (self.config.safety_level, analysis.risk_level) {
            // Permissive: Only block critical
            (SafetyLevel::Permissive, RiskLevel::Critical) => (HookStatus::Block, true),
            (SafetyLevel::Permissive, _) => (HookStatus::Proceed, false),

            // Standard: Block critical, warn high
            (SafetyLevel::Standard, RiskLevel::Critical) => (HookStatus::Block, true),
            (SafetyLevel::Standard, RiskLevel::High) => (HookStatus::ProceedWithWarning, false),
            (SafetyLevel::Standard, RiskLevel::Medium) => (HookStatus::ProceedWithWarning, false),
            (SafetyLevel::Standard, _) => (HookStatus::Proceed, false),

            // Strict: Block high+, warn medium
            (SafetyLevel::Strict, RiskLevel::Critical) => (HookStatus::Block, true),
            (SafetyLevel::Strict, RiskLevel::High) => (HookStatus::Block, true),
            (SafetyLevel::Strict, RiskLevel::Medium) => (HookStatus::ProceedWithWarning, false),
            (SafetyLevel::Strict, RiskLevel::Low) => (HookStatus::ProceedWithWarning, false),
            (SafetyLevel::Strict, RiskLevel::Safe) => (HookStatus::Proceed, false),
        }
    }

    fn build_status_message(&self, analysis: &CommandAnalysis, blocked: bool) -> Option<String> {
        if blocked {
            Some(format!(
                "Command blocked due to {} risk: {}",
                analysis.risk_level.display(),
                analysis.warnings.first()
                    .map(|w| w.message.as_str())
                    .unwrap_or("Safety policy violation")
            ))
        } else if analysis.risk_level >= RiskLevel::Medium {
            Some(format!(
                "{} risk detected for {:?} command",
                analysis.risk_level.display(),
                analysis.command_type
            ))
        } else {
            None
        }
    }

    async fn fetch_related_context(&self, command: &str) -> Result<Vec<CommandContext>, HookError> {
        // Search for similar previous commands
        let results = self.store.search_commands(command, 3).await?;

        Ok(results
            .into_iter()
            .map(|r| CommandContext {
                previous_command: r.command,
                output_summary: truncate_content(&r.output, 100),
                timestamp: r.timestamp,
                relevance_score: r.similarity,
            })
            .collect())
    }
}

/// Command safety analysis engine
pub struct CommandSafetyEngine {
    /// Built-in dangerous patterns
    dangerous_patterns: Vec<DangerousPattern>,
    /// Command type classifiers
    classifiers: Vec<CommandClassifier>,
}

#[derive(Debug, Clone)]
pub struct DangerousPattern {
    pub pattern: Regex,
    pub risk_level: RiskLevel,
    pub description: String,
    pub mitigation: Option<String>,
    pub alternatives: Vec<String>,
}

impl CommandSafetyEngine {
    pub fn new() -> Self {
        let dangerous_patterns = vec![
            // Critical - data destruction
            DangerousPattern {
                pattern: Regex::new(r"rm\s+(-rf?|--recursive)\s+(/|~|\$HOME|\*|\.\./)").unwrap(),
                risk_level: RiskLevel::Critical,
                description: "Recursive deletion of critical directories".to_string(),
                mitigation: Some("Use trash-cli or move to backup first".to_string()),
                alternatives: vec!["trash-put".to_string(), "mv to backup dir".to_string()],
            },
            DangerousPattern {
                pattern: Regex::new(r">\s*/dev/sd[a-z]").unwrap(),
                risk_level: RiskLevel::Critical,
                description: "Direct write to block device".to_string(),
                mitigation: None,
                alternatives: vec![],
            },
            DangerousPattern {
                pattern: Regex::new(r"mkfs\.|dd\s+if=.+of=/dev").unwrap(),
                risk_level: RiskLevel::Critical,
                description: "Filesystem formatting or raw disk write".to_string(),
                mitigation: Some("Verify device path carefully".to_string()),
                alternatives: vec![],
            },

            // High - system modification
            DangerousPattern {
                pattern: Regex::new(r"chmod\s+(-R\s+)?[0-7]*777").unwrap(),
                risk_level: RiskLevel::High,
                description: "Setting world-writable permissions".to_string(),
                mitigation: Some("Use more restrictive permissions like 755 or 644".to_string()),
                alternatives: vec!["chmod 755".to_string(), "chmod 644".to_string()],
            },
            DangerousPattern {
                pattern: Regex::new(r"curl\s+.*\|\s*(sudo\s+)?(bash|sh)").unwrap(),
                risk_level: RiskLevel::High,
                description: "Piping remote script to shell".to_string(),
                mitigation: Some("Download and review script before execution".to_string()),
                alternatives: vec!["curl -O then review".to_string()],
            },
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+.*rm|rm\s+.*sudo").unwrap(),
                risk_level: RiskLevel::High,
                description: "Sudo with file deletion".to_string(),
                mitigation: Some("Verify paths before sudo rm".to_string()),
                alternatives: vec![],
            },

            // Medium - potentially risky
            DangerousPattern {
                pattern: Regex::new(r"git\s+(push|reset|rebase)\s+(-f|--force)").unwrap(),
                risk_level: RiskLevel::Medium,
                description: "Force git operation".to_string(),
                mitigation: Some("Use --force-with-lease for safer push".to_string()),
                alternatives: vec!["git push --force-with-lease".to_string()],
            },
            DangerousPattern {
                pattern: Regex::new(r"npm\s+publish|cargo\s+publish").unwrap(),
                risk_level: RiskLevel::Medium,
                description: "Publishing package".to_string(),
                mitigation: Some("Verify version and content before publish".to_string()),
                alternatives: vec!["--dry-run first".to_string()],
            },
            DangerousPattern {
                pattern: Regex::new(r"docker\s+system\s+prune").unwrap(),
                risk_level: RiskLevel::Medium,
                description: "Docker system cleanup".to_string(),
                mitigation: Some("May remove needed images/containers".to_string()),
                alternatives: vec!["docker image prune".to_string()],
            },

            // Low - informational
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+").unwrap(),
                risk_level: RiskLevel::Low,
                description: "Elevated privileges".to_string(),
                mitigation: None,
                alternatives: vec![],
            },
        ];

        Self {
            dangerous_patterns,
            classifiers: Self::build_classifiers(),
        }
    }

    fn build_classifiers() -> Vec<CommandClassifier> {
        vec![
            CommandClassifier {
                pattern: Regex::new(r"^(ls|cat|head|tail|less|more|find|grep|wc)").unwrap(),
                command_type: CommandType::FileSystem,
            },
            CommandClassifier {
                pattern: Regex::new(r"^git\s").unwrap(),
                command_type: CommandType::Git,
            },
            CommandClassifier {
                pattern: Regex::new(r"^(npm|yarn|pnpm|cargo|pip|poetry)").unwrap(),
                command_type: CommandType::Package,
            },
            CommandClassifier {
                pattern: Regex::new(r"^(make|cmake|cargo\s+build|npm\s+run\s+build)").unwrap(),
                command_type: CommandType::Build,
            },
            CommandClassifier {
                pattern: Regex::new(r"(test|spec|pytest|jest|cargo\s+test)").unwrap(),
                command_type: CommandType::Test,
            },
            CommandClassifier {
                pattern: Regex::new(r"^(curl|wget|ssh|scp|rsync)").unwrap(),
                command_type: CommandType::Network,
            },
            CommandClassifier {
                pattern: Regex::new(r"^(systemctl|service|launchctl)").unwrap(),
                command_type: CommandType::System,
            },
            CommandClassifier {
                pattern: Regex::new(r"^docker").unwrap(),
                command_type: CommandType::Docker,
            },
            CommandClassifier {
                pattern: Regex::new(r"(psql|mysql|mongo|redis-cli|sqlite)").unwrap(),
                command_type: CommandType::Database,
            },
        ]
    }

    /// Analyze command for safety
    pub fn analyze(&self, command: &str, config: &PreBashExecConfig) -> Result<CommandAnalysis, HookError> {
        let mut warnings = Vec::new();
        let mut max_risk = RiskLevel::Safe;
        let mut alternatives = Vec::new();

        // Check against dangerous patterns
        for pattern in &self.dangerous_patterns {
            if pattern.pattern.is_match(command) {
                // Check if allowed by config override
                if config.allowed_patterns.iter().any(|p| command.contains(p)) {
                    continue;
                }

                warnings.push(SafetyWarning {
                    level: pattern.risk_level,
                    message: pattern.description.clone(),
                    pattern_matched: Some(pattern.pattern.as_str().to_string()),
                    mitigation: pattern.mitigation.clone(),
                });

                alternatives.extend(pattern.alternatives.clone());

                if pattern.risk_level > max_risk {
                    max_risk = pattern.risk_level;
                }
            }
        }

        // Check custom blocked patterns
        for blocked in &config.blocked_patterns {
            if command.contains(blocked) {
                warnings.push(SafetyWarning {
                    level: RiskLevel::High,
                    message: format!("Matches custom blocked pattern: {}", blocked),
                    pattern_matched: Some(blocked.clone()),
                    mitigation: None,
                });
                if max_risk < RiskLevel::High {
                    max_risk = RiskLevel::High;
                }
            }
        }

        // Classify command type
        let command_type = self.classify_command(command);

        Ok(CommandAnalysis {
            risk_level: max_risk,
            command_type,
            warnings,
            alternatives,
            related_context: vec![],
        })
    }

    fn classify_command(&self, command: &str) -> CommandType {
        for classifier in &self.classifiers {
            if classifier.pattern.is_match(command) {
                return classifier.command_type.clone();
            }
        }
        CommandType::Unknown
    }
}

#[derive(Debug, Clone)]
struct CommandClassifier {
    pattern: Regex,
    command_type: CommandType,
}

impl RiskLevel {
    pub fn display(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "Safe",
            RiskLevel::Low => "Low",
            RiskLevel::Medium => "Medium",
            RiskLevel::High => "High",
            RiskLevel::Critical => "CRITICAL",
        }
    }
}
```
    </section>

    <section name="PostBashExecHandler">
```rust
/// Post-bash-execution hook handler for learning
pub struct PostBashExecHandler {
    /// Teleological store
    store: Arc<TeleologicalStore>,
    /// Embedding pipeline
    embedder_pipeline: Arc<EmbedderPipeline>,
    /// Trajectory tracker
    trajectory_tracker: Arc<TrajectoryTracker>,
    /// Command pattern learner
    pattern_learner: Arc<CommandPatternLearner>,
    /// Configuration
    config: PostBashExecConfig,
}

#[derive(Debug, Clone)]
pub struct PostBashExecConfig {
    /// Store successful command outputs
    pub store_success_outputs: bool,
    /// Store error outputs (for troubleshooting learning)
    pub store_error_outputs: bool,
    /// Maximum output length to store
    pub max_output_length: usize,
    /// Minimum output length to store
    pub min_output_length: usize,
    /// Learn command sequences
    pub learn_sequences: bool,
    /// Error pattern extraction
    pub extract_error_patterns: bool,
}

impl Default for PostBashExecConfig {
    fn default() -> Self {
        Self {
            store_success_outputs: true,
            store_error_outputs: true,
            max_output_length: 50_000,
            min_output_length: 10,
            learn_sequences: true,
            extract_error_patterns: true,
        }
    }
}

/// Payload for post-bash-execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostBashExecPayload {
    /// Command that was executed
    pub command: String,
    /// Exit code
    pub exit_code: i32,
    /// Stdout content
    pub stdout: String,
    /// Stderr content
    pub stderr: String,
    /// Execution duration (ms)
    pub duration_ms: u64,
    /// Working directory
    pub working_dir: Option<String>,
}

/// Command execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    /// Unique ID
    pub record_id: Uuid,
    /// Command executed
    pub command: String,
    /// Exit code
    pub exit_code: i32,
    /// Output (truncated)
    pub output: String,
    /// Whether command succeeded
    pub success: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Session ID
    pub session_id: String,
    /// Working directory
    pub working_dir: Option<String>,
    /// Duration
    pub duration_ms: u64,
    /// Extracted patterns
    pub patterns: Vec<String>,
}

impl PostBashExecHandler {
    pub fn new(
        store: Arc<TeleologicalStore>,
        embedder_pipeline: Arc<EmbedderPipeline>,
        trajectory_tracker: Arc<TrajectoryTracker>,
        pattern_learner: Arc<CommandPatternLearner>,
        config: PostBashExecConfig,
    ) -> Self {
        Self {
            store,
            embedder_pipeline,
            trajectory_tracker,
            pattern_learner,
            config,
        }
    }

    /// Handle PostBashExec event
    pub async fn handle(&self, payload: PostBashExecPayload) -> Result<HookResponse, HookError> {
        let start = Instant::now();
        let success = payload.exit_code == 0;

        // 1. Determine if we should store this output
        let should_store = self.should_store(&payload, success);

        if !should_store {
            return Ok(HookResponse {
                status: HookStatus::Success,
                message: Some("Output not stored (filtered)".to_string()),
                warnings: vec![],
                injected_context: None,
                metrics: HookMetrics::quick(start.elapsed().as_millis() as u64),
            });
        }

        // 2. Prepare output content
        let output = if success {
            &payload.stdout
        } else {
            // Combine stderr and stdout for errors
            &format!("{}\n{}", payload.stderr, payload.stdout)
        };

        let truncated_output = truncate_content(output, self.config.max_output_length);

        // 3. Generate teleological array for the command+output
        let content_for_embedding = format!(
            "Command: {}\nOutput: {}",
            payload.command,
            truncated_output
        );

        let teleological_array = self.embedder_pipeline
            .generate_full(&content_for_embedding)
            .await
            .map_err(HookError::EmbeddingFailed)?;

        // 4. Create command record
        let patterns = if self.config.extract_error_patterns && !success {
            self.pattern_learner.extract_error_patterns(&payload.stderr).await?
        } else {
            vec![]
        };

        let record = CommandRecord {
            record_id: Uuid::new_v4(),
            command: payload.command.clone(),
            exit_code: payload.exit_code,
            output: truncated_output.clone(),
            success,
            timestamp: Utc::now(),
            session_id: get_current_session_id(),
            working_dir: payload.working_dir.clone(),
            duration_ms: payload.duration_ms,
            patterns: patterns.clone(),
        };

        // 5. Store in teleological store
        let memory_id = self.store.inject(InjectionRequest {
            content: content_for_embedding,
            teleological_array,
            memory_type: if success { MemoryType::CommandOutput } else { MemoryType::ErrorLog },
            namespace: "commands".to_string(),
            metadata: serde_json::to_value(&record)?,
        }).await.map_err(HookError::StorageError)?;

        // 6. Record in trajectory
        let trajectory_step = TrajectoryStep {
            action: TrajectoryAction::BashExec {
                command: payload.command.clone(),
                exit_code: payload.exit_code,
                memory_id,
            },
            timestamp: Utc::now(),
            context_size: output.len(),
            outcome: Some(if success {
                TrajectoryOutcome::Success
            } else {
                TrajectoryOutcome::Failure {
                    error: payload.stderr.lines().next().unwrap_or("Unknown error").to_string(),
                }
            }),
        };

        self.trajectory_tracker.record_step(trajectory_step).await?;

        // 7. Learn command sequences if enabled
        if self.config.learn_sequences {
            self.pattern_learner.record_command(
                &payload.command,
                success,
                payload.duration_ms,
            ).await?;
        }

        // 8. Build response
        let mut response_context = serde_json::json!({
            "memory_id": memory_id,
            "record_id": record.record_id,
            "success": success,
            "duration_ms": payload.duration_ms,
        });

        // Add error analysis if failed
        if !success && !patterns.is_empty() {
            response_context["error_patterns"] = serde_json::json!(patterns);
            response_context["troubleshooting"] = serde_json::json!(
                self.pattern_learner.suggest_fixes(&patterns).await?
            );
        }

        Ok(HookResponse {
            status: HookStatus::Success,
            message: Some(format!(
                "Command {} stored as memory {}",
                if success { "output" } else { "error" },
                memory_id
            )),
            warnings: if !success {
                vec![format!("Command failed with exit code {}", payload.exit_code)]
            } else {
                vec![]
            },
            injected_context: Some(response_context),
            metrics: HookMetrics {
                latency_ms: start.elapsed().as_millis() as u64,
                embedders_used: 13,
                goals_checked: 0,
            },
        })
    }

    fn should_store(&self, payload: &PostBashExecPayload, success: bool) -> bool {
        // Check config settings
        if success && !self.config.store_success_outputs {
            return false;
        }
        if !success && !self.config.store_error_outputs {
            return false;
        }

        // Check output length
        let output_len = if success {
            payload.stdout.len()
        } else {
            payload.stderr.len() + payload.stdout.len()
        };

        if output_len < self.config.min_output_length {
            return false;
        }

        // Filter out noisy commands
        let noisy_commands = ["ls", "pwd", "echo", "cd", "clear"];
        let cmd_start = payload.command.split_whitespace().next().unwrap_or("");
        if noisy_commands.contains(&cmd_start) && success {
            return false;
        }

        true
    }
}

/// Command pattern learner
pub struct CommandPatternLearner {
    /// Store for pattern persistence
    store: Arc<TeleologicalStore>,
    /// Recent command buffer
    command_buffer: Arc<RwLock<VecDeque<CommandBufferEntry>>>,
    /// Error pattern database
    error_patterns: Arc<ErrorPatternDatabase>,
}

#[derive(Debug, Clone)]
struct CommandBufferEntry {
    command: String,
    success: bool,
    duration_ms: u64,
    timestamp: DateTime<Utc>,
}

impl CommandPatternLearner {
    /// Extract error patterns from stderr
    pub async fn extract_error_patterns(&self, stderr: &str) -> Result<Vec<String>, HookError> {
        let mut patterns = Vec::new();

        // Common error patterns
        let error_regexes = [
            (r"error\[E\d+\]", "Rust compiler error"),
            (r"npm ERR!", "npm error"),
            (r"TypeError:|ReferenceError:|SyntaxError:", "JavaScript error"),
            (r"ModuleNotFoundError:|ImportError:", "Python import error"),
            (r"Permission denied", "Permission error"),
            (r"No such file or directory", "File not found"),
            (r"command not found", "Missing command"),
            (r"Connection refused", "Network error"),
        ];

        for (regex, description) in &error_regexes {
            if Regex::new(regex).unwrap().is_match(stderr) {
                patterns.push(description.to_string());
            }
        }

        Ok(patterns)
    }

    /// Record command for sequence learning
    pub async fn record_command(
        &self,
        command: &str,
        success: bool,
        duration_ms: u64,
    ) -> Result<(), HookError> {
        let entry = CommandBufferEntry {
            command: command.to_string(),
            success,
            duration_ms,
            timestamp: Utc::now(),
        };

        let mut buffer = self.command_buffer.write().await;
        buffer.push_back(entry);

        // Keep last 100 commands
        while buffer.len() > 100 {
            buffer.pop_front();
        }

        // Check for repeated sequences
        if buffer.len() >= 3 {
            self.detect_sequences(&buffer).await?;
        }

        Ok(())
    }

    /// Suggest fixes based on error patterns
    pub async fn suggest_fixes(&self, patterns: &[String]) -> Result<Vec<String>, HookError> {
        let mut suggestions = Vec::new();

        for pattern in patterns {
            if let Some(fixes) = self.error_patterns.get_fixes(pattern).await {
                suggestions.extend(fixes);
            }
        }

        Ok(suggestions)
    }

    async fn detect_sequences(&self, buffer: &VecDeque<CommandBufferEntry>) -> Result<(), HookError> {
        // Look for repeating patterns
        let commands: Vec<_> = buffer.iter()
            .map(|e| e.command.split_whitespace().next().unwrap_or(""))
            .collect();

        // Find 2-3 command patterns that repeat
        // Store as useful sequences
        // This enables suggesting "you usually run X after Y"

        Ok(())
    }
}
```
    </section>

    <section name="ShellScripts">
```bash
#!/bin/bash
# .claude/hooks/pre_bash_exec.sh
# PreBashExec hook - safety checks before command execution

set -e

# Read input from stdin (JSON payload)
INPUT=$(cat)

# Extract command
COMMAND=$(echo "$INPUT" | jq -r '.command')
WORKING_DIR=$(echo "$INPUT" | jq -r '.working_dir // empty')

# Call MCP tool for safety check
RESPONSE=$(echo '{
  "jsonrpc": "2.0",
  "id": "pre-bash-exec-'$(date +%s)'",
  "method": "hooks/pre_bash_exec",
  "params": {
    "command": '"$(echo "$COMMAND" | jq -Rs .)"',
    "working_dir": "'"$WORKING_DIR"'"
  }
}' | nc -U /tmp/contextgraph.sock 2>/dev/null || echo '{"result":{"status":"proceed"}}')

# Parse response
STATUS=$(echo "$RESPONSE" | jq -r '.result.status // "proceed"')
MESSAGE=$(echo "$RESPONSE" | jq -r '.result.message // empty')
WARNINGS=$(echo "$RESPONSE" | jq -r '.result.warnings[]? // empty')

# Output warnings
if [ -n "$WARNINGS" ]; then
  echo "Safety warnings:" >&2
  echo "$WARNINGS" | while read -r warning; do
    echo "  - $warning" >&2
  done
fi

# Output message
if [ -n "$MESSAGE" ]; then
  echo "$MESSAGE" >&2
fi

# Output suggestions if any
ALTERNATIVES=$(echo "$RESPONSE" | jq -r '.result.injected_context.alternatives[]? // empty')
if [ -n "$ALTERNATIVES" ]; then
  echo "Suggested alternatives:" >&2
  echo "$ALTERNATIVES" | while read -r alt; do
    echo "  - $alt" >&2
  done
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
    echo "Command blocked for safety reasons" >&2
    exit 1
    ;;
  *)
    exit 0
    ;;
esac
```

```bash
#!/bin/bash
# .claude/hooks/post_bash_exec.sh
# PostBashExec hook - stores command outputs for learning

set -e

# Read input from stdin (JSON payload)
INPUT=$(cat)

# Extract command details
COMMAND=$(echo "$INPUT" | jq -r '.command')
EXIT_CODE=$(echo "$INPUT" | jq -r '.exit_code // 0')
STDOUT=$(echo "$INPUT" | jq -r '.stdout // ""')
STDERR=$(echo "$INPUT" | jq -r '.stderr // ""')
DURATION_MS=$(echo "$INPUT" | jq -r '.duration_ms // 0')
WORKING_DIR=$(echo "$INPUT" | jq -r '.working_dir // empty')

# Skip noisy commands
CMD_START=$(echo "$COMMAND" | awk '{print $1}')
case "$CMD_START" in
  ls|pwd|echo|cd|clear)
    if [ "$EXIT_CODE" -eq 0 ]; then
      exit 0
    fi
    ;;
esac

# Check output length
TOTAL_LEN=$((${#STDOUT} + ${#STDERR}))
if [ "$TOTAL_LEN" -lt 10 ]; then
  exit 0
fi

# Call MCP tool to store and learn
RESPONSE=$(echo '{
  "jsonrpc": "2.0",
  "id": "post-bash-exec-'$(date +%s)'",
  "method": "hooks/post_bash_exec",
  "params": {
    "command": '"$(echo "$COMMAND" | jq -Rs .)"',
    "exit_code": '"$EXIT_CODE"',
    "stdout": '"$(echo "$STDOUT" | jq -Rs .)"',
    "stderr": '"$(echo "$STDERR" | jq -Rs .)"',
    "duration_ms": '"$DURATION_MS"',
    "working_dir": "'"$WORKING_DIR"'"
  }
}' | nc -U /tmp/contextgraph.sock 2>/dev/null || echo '{"error":{"message":"Connection failed"}}')

# Check for errors
ERROR=$(echo "$RESPONSE" | jq -r '.error.message // empty')
if [ -n "$ERROR" ]; then
  echo "Warning: PostBashExec hook failed: $ERROR" >&2
fi

# If command failed, output troubleshooting suggestions
if [ "$EXIT_CODE" -ne 0 ]; then
  TROUBLESHOOTING=$(echo "$RESPONSE" | jq -r '.result.injected_context.troubleshooting[]? // empty')
  if [ -n "$TROUBLESHOOTING" ]; then
    echo "Troubleshooting suggestions:" >&2
    echo "$TROUBLESHOOTING" | while read -r suggestion; do
      echo "  - $suggestion" >&2
    done
  fi
fi

exit 0
```
    </section>
  </pseudo_code>
</task_spec>
```

# TASK-INTEG-009: Subagent Creation

```xml
<task_spec>
  <task_id>TASK-INTEG-009</task_id>
  <title>Subagent Creation (goal-tracker, context-curator, pattern-miner, learning-coach)</title>
  <status>pending</status>

  <objective>
    Implement four specialized subagents that leverage the teleological array
    system to provide intelligent assistance: goal tracking, context curation,
    pattern mining, and learning coaching.
  </objective>

  <rationale>
    Subagents provide:
    1. Goal Tracker: Monitors progress toward discovered goals
    2. Context Curator: Manages and surfaces relevant context
    3. Pattern Miner: Discovers patterns in code and workflows
    4. Learning Coach: Guides improvement based on trajectory analysis

    These agents work autonomously to enhance the development experience
    through the teleological array system.
  </rationale>

  <dependencies>
    <dependency type="required">TASK-INTEG-008</dependency>    <!-- Skills -->
    <dependency type="required">TASK-INTEG-001</dependency>    <!-- Memory MCP handlers -->
    <dependency type="required">TASK-INTEG-002</dependency>    <!-- Purpose/Goal MCP handlers -->
    <dependency type="required">TASK-LOGIC-009</dependency>    <!-- Goal discovery -->
    <dependency type="required">TASK-LOGIC-010</dependency>    <!-- Drift detection -->
  </dependencies>

  <input_context_files>
    <file purpose="skills">crates/context-graph-mcp/src/skills/mod.rs</file>
    <file purpose="mcp_tools_spec">docs2/refactor/08-MCP-TOOLS.md</file>
    <file purpose="goal_discovery">crates/context-graph-core/src/autonomous/discovery.rs</file>
    <file purpose="drift_detector">crates/context-graph-core/src/autonomous/drift.rs</file>
  </input_context_files>

  <output_artifacts>
    <artifact type="source">crates/context-graph-mcp/src/agents/mod.rs</artifact>
    <artifact type="source">crates/context-graph-mcp/src/agents/orchestrator.rs</artifact>
    <artifact type="config">.claude/agents/goal-tracker/agent.yaml</artifact>
    <artifact type="config">.claude/agents/goal-tracker/prompts/system.md</artifact>
    <artifact type="config">.claude/agents/context-curator/agent.yaml</artifact>
    <artifact type="config">.claude/agents/context-curator/prompts/system.md</artifact>
    <artifact type="config">.claude/agents/pattern-miner/agent.yaml</artifact>
    <artifact type="config">.claude/agents/pattern-miner/prompts/system.md</artifact>
    <artifact type="config">.claude/agents/learning-coach/agent.yaml</artifact>
    <artifact type="config">.claude/agents/learning-coach/prompts/system.md</artifact>
    <artifact type="source">crates/context-graph-mcp/src/handlers/agents.rs</artifact>
    <artifact type="test">crates/context-graph-mcp/tests/agents_test.rs</artifact>
  </output_artifacts>

  <definition_of_done>
    <criterion id="1">All 4 subagents implemented with YAML definitions</criterion>
    <criterion id="2">Subagent orchestrator manages lifecycle (start/stop/restart)</criterion>
    <criterion id="3">Inter-agent communication via teleological store</criterion>
    <criterion id="4">MCP handlers for agent control (list, start, stop, status, communicate)</criterion>
    <criterion id="5">Each agent has system prompt and instruction files</criterion>
    <criterion id="6">Agents can invoke skills and access MCP tools</criterion>
    <criterion id="7">Agent health monitoring implemented</criterion>
    <criterion id="8">Test coverage for agent orchestration</criterion>
  </definition_of_done>

  <estimated_complexity>High</estimated_complexity>

  <pseudo_code>
    <section name="SubagentOrchestrator">
```rust
// crates/context-graph-mcp/src/agents/orchestrator.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

/// Subagent orchestrator manages agent lifecycle and coordination
pub struct SubagentOrchestrator {
    /// Agent registry
    agents: Arc<RwLock<HashMap<String, RunningAgent>>>,
    /// Teleological store for inter-agent communication
    store: Arc<TeleologicalStore>,
    /// Skill loader for agent capabilities
    skill_loader: Arc<SkillLoader>,
    /// Message router
    message_router: Arc<MessageRouter>,
    /// Configuration
    config: OrchestratorConfig,
}

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum concurrent agents
    pub max_agents: usize,
    /// Agent health check interval (seconds)
    pub health_check_interval_secs: u64,
    /// Agent timeout (seconds)
    pub agent_timeout_secs: u64,
    /// Agents directory
    pub agents_dir: PathBuf,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_agents: 10,
            health_check_interval_secs: 30,
            agent_timeout_secs: 300,
            agents_dir: PathBuf::from(".claude/agents"),
        }
    }
}

/// Running agent instance
pub struct RunningAgent {
    /// Agent definition
    pub definition: AgentDefinition,
    /// Agent ID
    pub agent_id: String,
    /// Status
    pub status: AgentStatus,
    /// Message channel
    pub message_tx: mpsc::Sender<AgentMessage>,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// Last activity
    pub last_activity: DateTime<Utc>,
    /// Task handle
    task_handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent name
    pub name: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Version
    pub version: String,
    /// Description
    pub description: String,
    /// Capabilities (skill names)
    pub capabilities: Vec<String>,
    /// Allowed MCP tools
    pub allowed_tools: Vec<String>,
    /// System prompt path
    pub system_prompt: PathBuf,
    /// Instructions path
    pub instructions: Option<PathBuf>,
    /// Trigger configuration
    pub triggers: Vec<AgentTrigger>,
    /// Resource limits
    pub resources: AgentResources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    GoalTracker,
    ContextCurator,
    PatternMiner,
    LearningCoach,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Starting,
    Running,
    Idle,
    Busy,
    Stopping,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTrigger {
    /// Trigger type
    pub trigger_type: TriggerType,
    /// Trigger condition
    pub condition: Option<String>,
    /// Cooldown between triggers (seconds)
    pub cooldown_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerType {
    /// Triggered on session start
    SessionStart,
    /// Triggered on session end
    SessionEnd,
    /// Triggered periodically
    Periodic { interval_secs: u64 },
    /// Triggered by event
    Event { event_name: String },
    /// Triggered by message
    Message,
    /// Manual only
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResources {
    /// Maximum memory (MB)
    pub max_memory_mb: usize,
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Rate limit (requests per minute)
    pub rate_limit_rpm: usize,
}

impl Default for AgentResources {
    fn default() -> Self {
        Self {
            max_memory_mb: 256,
            max_concurrent_tasks: 5,
            rate_limit_rpm: 60,
        }
    }
}

/// Message for inter-agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Message ID
    pub message_id: Uuid,
    /// Sender agent
    pub from: String,
    /// Target agent (or "broadcast")
    pub to: String,
    /// Message type
    pub message_type: MessageType,
    /// Payload
    pub payload: serde_json::Value,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Query,
    Response,
    Notification,
    Task,
    Result,
}

impl SubagentOrchestrator {
    pub fn new(
        store: Arc<TeleologicalStore>,
        skill_loader: Arc<SkillLoader>,
        config: OrchestratorConfig,
    ) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            store,
            skill_loader,
            message_router: Arc::new(MessageRouter::new()),
            config,
        }
    }

    /// Load all agent definitions from the agents directory
    pub async fn load_definitions(&self) -> Result<Vec<AgentDefinition>, AgentError> {
        let mut definitions = Vec::new();

        let entries = std::fs::read_dir(&self.config.agents_dir)
            .map_err(|e| AgentError::IoError(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let agent_yaml = path.join("agent.yaml");
                if agent_yaml.exists() {
                    match self.load_definition(&agent_yaml) {
                        Ok(def) => definitions.push(def),
                        Err(e) => tracing::warn!("Failed to load agent from {:?}: {}", agent_yaml, e),
                    }
                }
            }
        }

        Ok(definitions)
    }

    fn load_definition(&self, yaml_path: &Path) -> Result<AgentDefinition, AgentError> {
        let content = std::fs::read_to_string(yaml_path)
            .map_err(|e| AgentError::IoError(e.to_string()))?;

        serde_yaml::from_str(&content)
            .map_err(|e| AgentError::ParseError(e.to_string()))
    }

    /// Start an agent
    pub async fn start_agent(&self, name: &str) -> Result<String, AgentError> {
        // Check if already running
        {
            let agents = self.agents.read().await;
            if agents.contains_key(name) {
                return Err(AgentError::AlreadyRunning(name.to_string()));
            }
        }

        // Check max agents
        {
            let agents = self.agents.read().await;
            if agents.len() >= self.config.max_agents {
                return Err(AgentError::MaxAgentsReached(self.config.max_agents));
            }
        }

        // Load definition
        let agent_yaml = self.config.agents_dir.join(name).join("agent.yaml");
        let definition = self.load_definition(&agent_yaml)?;

        // Create agent instance
        let agent_id = format!("{}-{}", name, Uuid::new_v4().to_string()[..8].to_string());
        let (tx, rx) = mpsc::channel(100);

        // Spawn agent task
        let agent_runner = AgentRunner::new(
            definition.clone(),
            agent_id.clone(),
            rx,
            self.store.clone(),
            self.skill_loader.clone(),
            self.message_router.clone(),
        );

        let task_handle = tokio::spawn(async move {
            agent_runner.run().await;
        });

        // Register agent
        let running = RunningAgent {
            definition,
            agent_id: agent_id.clone(),
            status: AgentStatus::Starting,
            message_tx: tx,
            started_at: Utc::now(),
            last_activity: Utc::now(),
            task_handle,
        };

        {
            let mut agents = self.agents.write().await;
            agents.insert(name.to_string(), running);
        }

        // Register with message router
        self.message_router.register(&agent_id).await;

        tracing::info!("Started agent {} with ID {}", name, agent_id);
        Ok(agent_id)
    }

    /// Stop an agent
    pub async fn stop_agent(&self, name: &str) -> Result<(), AgentError> {
        let mut agents = self.agents.write().await;

        let running = agents.remove(name)
            .ok_or_else(|| AgentError::NotFound(name.to_string()))?;

        // Send stop signal
        let _ = running.message_tx.send(AgentMessage {
            message_id: Uuid::new_v4(),
            from: "orchestrator".to_string(),
            to: running.agent_id.clone(),
            message_type: MessageType::Notification,
            payload: serde_json::json!({"action": "stop"}),
            timestamp: Utc::now(),
        }).await;

        // Abort task after timeout
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                running.task_handle.abort();
            }
            _ = running.task_handle => {}
        }

        // Unregister from router
        self.message_router.unregister(&running.agent_id).await;

        tracing::info!("Stopped agent {}", name);
        Ok(())
    }

    /// Get agent status
    pub async fn get_status(&self, name: &str) -> Result<AgentStatusInfo, AgentError> {
        let agents = self.agents.read().await;

        let running = agents.get(name)
            .ok_or_else(|| AgentError::NotFound(name.to_string()))?;

        Ok(AgentStatusInfo {
            agent_id: running.agent_id.clone(),
            name: running.definition.name.clone(),
            agent_type: running.definition.agent_type,
            status: running.status,
            started_at: running.started_at,
            last_activity: running.last_activity,
            uptime_secs: (Utc::now() - running.started_at).num_seconds() as u64,
        })
    }

    /// List all agents
    pub async fn list_agents(&self) -> Vec<AgentStatusInfo> {
        let agents = self.agents.read().await;

        agents.values()
            .map(|r| AgentStatusInfo {
                agent_id: r.agent_id.clone(),
                name: r.definition.name.clone(),
                agent_type: r.definition.agent_type,
                status: r.status,
                started_at: r.started_at,
                last_activity: r.last_activity,
                uptime_secs: (Utc::now() - r.started_at).num_seconds() as u64,
            })
            .collect()
    }

    /// Send message to agent
    pub async fn send_message(&self, target: &str, message: AgentMessage) -> Result<(), AgentError> {
        let agents = self.agents.read().await;

        if target == "broadcast" {
            // Broadcast to all agents
            for (_, running) in agents.iter() {
                let _ = running.message_tx.send(message.clone()).await;
            }
        } else {
            let running = agents.get(target)
                .ok_or_else(|| AgentError::NotFound(target.to_string()))?;

            running.message_tx.send(message).await
                .map_err(|_| AgentError::MessageFailed)?;
        }

        Ok(())
    }

    /// Health check for all agents
    pub async fn health_check(&self) -> Vec<AgentHealth> {
        let agents = self.agents.read().await;
        let now = Utc::now();

        agents.values()
            .map(|r| {
                let inactive_secs = (now - r.last_activity).num_seconds() as u64;
                let healthy = r.status != AgentStatus::Error
                    && inactive_secs < self.config.agent_timeout_secs;

                AgentHealth {
                    agent_id: r.agent_id.clone(),
                    name: r.definition.name.clone(),
                    healthy,
                    status: r.status,
                    inactive_secs,
                    issues: if !healthy {
                        vec![format!("Inactive for {} seconds", inactive_secs)]
                    } else {
                        vec![]
                    },
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusInfo {
    pub agent_id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    pub agent_id: String,
    pub name: String,
    pub healthy: bool,
    pub status: AgentStatus,
    pub inactive_secs: u64,
    pub issues: Vec<String>,
}

/// Agent runner - executes the agent logic
pub struct AgentRunner {
    definition: AgentDefinition,
    agent_id: String,
    message_rx: mpsc::Receiver<AgentMessage>,
    store: Arc<TeleologicalStore>,
    skill_loader: Arc<SkillLoader>,
    message_router: Arc<MessageRouter>,
}

impl AgentRunner {
    pub fn new(
        definition: AgentDefinition,
        agent_id: String,
        message_rx: mpsc::Receiver<AgentMessage>,
        store: Arc<TeleologicalStore>,
        skill_loader: Arc<SkillLoader>,
        message_router: Arc<MessageRouter>,
    ) -> Self {
        Self {
            definition,
            agent_id,
            message_rx,
            store,
            skill_loader,
            message_router,
        }
    }

    pub async fn run(&mut self) {
        tracing::info!("Agent {} starting", self.agent_id);

        // Load system prompt
        let system_prompt = self.load_system_prompt().await;

        // Main agent loop
        loop {
            tokio::select! {
                Some(message) = self.message_rx.recv() => {
                    if message.payload.get("action") == Some(&serde_json::json!("stop")) {
                        break;
                    }
                    self.handle_message(message).await;
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Check for triggered actions
                    self.check_triggers().await;
                }
            }
        }

        tracing::info!("Agent {} stopped", self.agent_id);
    }

    async fn load_system_prompt(&self) -> String {
        std::fs::read_to_string(&self.definition.system_prompt)
            .unwrap_or_else(|_| "You are an AI assistant.".to_string())
    }

    async fn handle_message(&self, message: AgentMessage) {
        tracing::debug!("Agent {} received message: {:?}", self.agent_id, message.message_type);

        match message.message_type {
            MessageType::Query => {
                // Handle query and send response
                let response = self.process_query(&message.payload).await;
                let _ = self.message_router.send(AgentMessage {
                    message_id: Uuid::new_v4(),
                    from: self.agent_id.clone(),
                    to: message.from,
                    message_type: MessageType::Response,
                    payload: response,
                    timestamp: Utc::now(),
                }).await;
            }
            MessageType::Task => {
                let result = self.execute_task(&message.payload).await;
                let _ = self.message_router.send(AgentMessage {
                    message_id: Uuid::new_v4(),
                    from: self.agent_id.clone(),
                    to: message.from,
                    message_type: MessageType::Result,
                    payload: result,
                    timestamp: Utc::now(),
                }).await;
            }
            MessageType::Notification => {
                // Handle notification
            }
            _ => {}
        }
    }

    async fn process_query(&self, payload: &serde_json::Value) -> serde_json::Value {
        // Process based on agent type
        match self.definition.agent_type {
            AgentType::GoalTracker => self.goal_tracker_query(payload).await,
            AgentType::ContextCurator => self.context_curator_query(payload).await,
            AgentType::PatternMiner => self.pattern_miner_query(payload).await,
            AgentType::LearningCoach => self.learning_coach_query(payload).await,
            _ => serde_json::json!({"error": "Unknown agent type"}),
        }
    }

    async fn execute_task(&self, payload: &serde_json::Value) -> serde_json::Value {
        // Execute task using skills
        if let Some(skill_name) = payload.get("skill").and_then(|s| s.as_str()) {
            match self.skill_loader.invoke(skill_name, payload.clone()).await {
                Ok(result) => serde_json::to_value(result).unwrap_or_default(),
                Err(e) => serde_json::json!({"error": e.to_string()}),
            }
        } else {
            serde_json::json!({"error": "No skill specified"})
        }
    }

    async fn check_triggers(&self) {
        // Check if any triggers should fire
        for trigger in &self.definition.triggers {
            match &trigger.trigger_type {
                TriggerType::Periodic { interval_secs } => {
                    // Check if interval has elapsed
                    // Implementation would track last trigger time
                }
                _ => {}
            }
        }
    }

    // Agent-specific query handlers
    async fn goal_tracker_query(&self, payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({"agent": "goal-tracker", "status": "processing"})
    }

    async fn context_curator_query(&self, payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({"agent": "context-curator", "status": "processing"})
    }

    async fn pattern_miner_query(&self, payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({"agent": "pattern-miner", "status": "processing"})
    }

    async fn learning_coach_query(&self, payload: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({"agent": "learning-coach", "status": "processing"})
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    NotFound(String),
    #[error("Agent already running: {0}")]
    AlreadyRunning(String),
    #[error("Maximum agents reached: {0}")]
    MaxAgentsReached(usize),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Message delivery failed")]
    MessageFailed,
}
```
    </section>

    <section name="GoalTrackerAgent">
```yaml
# .claude/agents/goal-tracker/agent.yaml
---
name: goal-tracker
agent_type: GoalTracker
version: 1.0.0
description: |
  Monitors progress toward discovered goals, tracks alignment over time,
  and alerts when significant drift is detected.

capabilities:
  - goal-alignment
  - drift-check
  - memory-search

allowed_tools:
  - purpose/goal_alignment
  - purpose/drift_check
  - purpose/discover_goals
  - memory/search
  - consciousness/sync_level

system_prompt: prompts/system.md
instructions: prompts/instructions.md

triggers:
  - trigger_type: SessionStart
  - trigger_type:
      Periodic:
        interval_secs: 300
  - trigger_type:
      Event:
        event_name: goal_discovered
  - trigger_type: Message

resources:
  max_memory_mb: 128
  max_concurrent_tasks: 3
  rate_limit_rpm: 30
---
```

```markdown
<!-- .claude/agents/goal-tracker/prompts/system.md -->
# Goal Tracker Agent

You are the Goal Tracker Agent, responsible for monitoring progress toward discovered goals and detecting drift.

## Your Responsibilities

1. **Track Goal Progress**: Monitor alignment scores over time for all active goals
2. **Detect Drift**: Alert when work is diverging from established goals
3. **Report Status**: Provide clear progress reports on goal achievement
4. **Recommend Actions**: Suggest course corrections when drift is detected

## How You Work

You receive regular updates about:
- New memories being stored (code, documentation, conversations)
- Session start/end events
- Direct queries from users or other agents

For each update, you:
1. Calculate alignment with relevant goals
2. Compare to historical alignment (trend analysis)
3. Determine if drift exceeds thresholds
4. Generate reports or alerts as needed

## Communication

When reporting, be:
- Concise but informative
- Data-driven (include alignment percentages)
- Actionable (provide specific recommendations)
- Prioritized (critical drift first)

## Available Skills

- `goal-alignment`: Check content alignment with goals
- `drift-check`: Analyze drift trends
- `memory-search`: Find related memories

## Output Format

When reporting goal status:

```
Goal: [Goal Label]
Level: [Strategic/Tactical/Immediate]
Current Alignment: X%
Trend: [Improving/Stable/Declining]
Recommendation: [Action to take]
```
```
    </section>

    <section name="ContextCuratorAgent">
```yaml
# .claude/agents/context-curator/agent.yaml
---
name: context-curator
agent_type: ContextCurator
version: 1.0.0
description: |
  Manages and surfaces relevant context based on current work,
  ensuring developers have the right information at the right time.

capabilities:
  - memory-search
  - context-injection

allowed_tools:
  - memory/search
  - memory/search_multi_perspective
  - memory/inject
  - analysis/embedder_distribution

system_prompt: prompts/system.md
instructions: prompts/instructions.md

triggers:
  - trigger_type: SessionStart
  - trigger_type:
      Event:
        event_name: file_read
  - trigger_type: Message

resources:
  max_memory_mb: 256
  max_concurrent_tasks: 5
  rate_limit_rpm: 60
---
```

```markdown
<!-- .claude/agents/context-curator/prompts/system.md -->
# Context Curator Agent

You are the Context Curator Agent, responsible for surfacing relevant context to enhance productivity.

## Your Responsibilities

1. **Anticipate Needs**: Predict what context will be useful based on current activity
2. **Surface Relevant Info**: Proactively provide related memories, patterns, and history
3. **Organize Context**: Structure information for easy consumption
4. **Reduce Friction**: Help developers find what they need without searching

## How You Work

When activities occur (file reads, searches, session starts), you:
1. Analyze the current context
2. Search for related memories across multiple dimensions
3. Rank and filter results by relevance
4. Present context in a digestible format

## Context Types You Manage

- **Code Context**: Related code files, functions, patterns
- **Documentation**: Relevant docs, comments, READMEs
- **History**: Recent changes to related files
- **Patterns**: Similar code patterns used elsewhere
- **Goals**: Related goals and alignment information

## Communication

Present context as:
- Brief summaries (not full content)
- Relevance scores
- Links/references for deeper exploration
- Organized by type

## Available Skills

- `memory-search`: Find related memories
- `context-injection`: Store new context

## Output Format

When surfacing context:

```
Related to: [Current File/Activity]

Code Context:
- [file.rs]: Brief description (85% relevant)
- [other.rs]: Brief description (72% relevant)

Recent History:
- [change]: Description (2 hours ago)

Patterns:
- [pattern]: Where else this appears
```
```
    </section>

    <section name="PatternMinerAgent">
```yaml
# .claude/agents/pattern-miner/agent.yaml
---
name: pattern-miner
agent_type: PatternMiner
version: 1.0.0
description: |
  Discovers patterns in code and workflows, learning from developer
  behavior to identify reusable solutions and common approaches.

capabilities:
  - pattern-learning
  - memory-search

allowed_tools:
  - memory/search
  - memory/inject
  - goal/cluster_analysis
  - analysis/embedder_distribution

system_prompt: prompts/system.md
instructions: prompts/instructions.md

triggers:
  - trigger_type:
      Periodic:
        interval_secs: 600
  - trigger_type: SessionEnd
  - trigger_type:
      Event:
        event_name: pattern_learned
  - trigger_type: Message

resources:
  max_memory_mb: 512
  max_concurrent_tasks: 2
  rate_limit_rpm: 20
---
```

```markdown
<!-- .claude/agents/pattern-miner/prompts/system.md -->
# Pattern Miner Agent

You are the Pattern Miner Agent, responsible for discovering and cataloging patterns in code and workflows.

## Your Responsibilities

1. **Discover Patterns**: Find recurring structures in code, edits, and workflows
2. **Catalog Patterns**: Store and organize discovered patterns for future use
3. **Suggest Patterns**: Recommend applicable patterns when relevant
4. **Track Evolution**: Notice when patterns change or new ones emerge

## Pattern Types You Track

- **Code Patterns**: Common code structures, idioms, architectures
- **Refactoring Patterns**: How code changes are typically made
- **Error Patterns**: Common mistakes and their fixes
- **Workflow Patterns**: Sequences of actions that work well together
- **Test Patterns**: Testing approaches and structures

## How You Work

Periodically and on session end:
1. Analyze recent memories for pattern candidates
2. Cluster similar items to find emergent patterns
3. Validate patterns against existing catalog
4. Store new patterns with metadata

## Pattern Quality Criteria

Good patterns are:
- Recurring (appear multiple times)
- Consistent (similar structure each time)
- Useful (provide value when applied)
- Contextual (clear when to use)

## Available Skills

- `pattern-learning`: Store and categorize patterns
- `memory-search`: Find pattern candidates

## Output Format

When reporting patterns:

```
Pattern Discovered: [Pattern Name]
Type: [Code/Refactoring/Error/Workflow/Test]
Frequency: [X occurrences]
Context: [When this pattern applies]
Example: [Brief code example]
```
```
    </section>

    <section name="LearningCoachAgent">
```yaml
# .claude/agents/learning-coach/agent.yaml
---
name: learning-coach
agent_type: LearningCoach
version: 1.0.0
description: |
  Guides improvement based on trajectory analysis, identifying
  skill gaps and suggesting learning opportunities.

capabilities:
  - memory-search
  - goal-alignment
  - drift-check

allowed_tools:
  - memory/search
  - purpose/goal_alignment
  - analysis/entry_point_stats
  - consciousness/get_state

system_prompt: prompts/system.md
instructions: prompts/instructions.md

triggers:
  - trigger_type: SessionEnd
  - trigger_type:
      Periodic:
        interval_secs: 1800
  - trigger_type: Message

resources:
  max_memory_mb: 128
  max_concurrent_tasks: 2
  rate_limit_rpm: 15
---
```

```markdown
<!-- .claude/agents/learning-coach/prompts/system.md -->
# Learning Coach Agent

You are the Learning Coach Agent, responsible for analyzing work patterns and suggesting improvements.

## Your Responsibilities

1. **Analyze Trajectories**: Study patterns in work sessions
2. **Identify Gaps**: Find areas where improvement is possible
3. **Suggest Learning**: Recommend resources and practice
4. **Track Progress**: Monitor improvement over time
5. **Provide Guidance**: Offer actionable advice

## What You Analyze

- **Error Patterns**: Repeated mistakes or struggles
- **Time Patterns**: Where time is spent inefficiently
- **Success Patterns**: What works well (reinforce)
- **Skill Gaps**: Missing knowledge or techniques
- **Goal Progress**: How effectively goals are being achieved

## How You Work

At session end and periodically:
1. Review trajectory data (actions, outcomes, timing)
2. Identify patterns (good and bad)
3. Compare to successful patterns
4. Generate personalized recommendations

## Coaching Philosophy

- Be encouraging, not critical
- Focus on growth, not mistakes
- Provide actionable next steps
- Celebrate progress
- Be specific with feedback

## Available Skills

- `memory-search`: Review session history
- `goal-alignment`: Check alignment with goals
- `drift-check`: Analyze direction of work

## Output Format

When providing coaching:

```
Session Summary
---------------
Duration: X hours
Key Accomplishments: [list]
Areas for Growth: [list]

Recommendations
---------------
1. [Specific actionable suggestion]
2. [Learning resource to explore]
3. [Practice exercise]

Progress Update
---------------
Compared to last session: [Improved/Stable/Declined] in [area]
Keep up: [What's working well]
```
```
    </section>

    <section name="MCPHandlers">
```rust
// crates/context-graph-mcp/src/handlers/agents.rs

use crate::agents::{SubagentOrchestrator, AgentMessage, MessageType};
use std::sync::Arc;

/// MCP handlers for agent operations
pub struct AgentsHandler {
    orchestrator: Arc<SubagentOrchestrator>,
}

impl AgentsHandler {
    pub fn new(orchestrator: Arc<SubagentOrchestrator>) -> Self {
        Self { orchestrator }
    }

    /// List available and running agents
    pub async fn handle_list(&self, params: ListAgentsParams) -> Result<ListAgentsResponse, McpError> {
        let running = self.orchestrator.list_agents().await;
        let available = self.orchestrator.load_definitions().await
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.name)
            .collect();

        Ok(ListAgentsResponse {
            running,
            available,
        })
    }

    /// Start an agent
    pub async fn handle_start(&self, params: StartAgentParams) -> Result<StartAgentResponse, McpError> {
        let agent_id = self.orchestrator.start_agent(&params.name).await
            .map_err(|e| McpError::internal(e.to_string()))?;

        Ok(StartAgentResponse {
            agent_id,
            status: "started".to_string(),
        })
    }

    /// Stop an agent
    pub async fn handle_stop(&self, params: StopAgentParams) -> Result<StopAgentResponse, McpError> {
        self.orchestrator.stop_agent(&params.name).await
            .map_err(|e| McpError::internal(e.to_string()))?;

        Ok(StopAgentResponse {
            success: true,
        })
    }

    /// Get agent status
    pub async fn handle_status(&self, params: AgentStatusParams) -> Result<AgentStatusResponse, McpError> {
        let status = self.orchestrator.get_status(&params.name).await
            .map_err(|e| McpError::not_found(e.to_string()))?;

        Ok(AgentStatusResponse { status })
    }

    /// Send message to agent
    pub async fn handle_communicate(&self, params: CommunicateParams) -> Result<CommunicateResponse, McpError> {
        let message = AgentMessage {
            message_id: Uuid::new_v4(),
            from: "mcp".to_string(),
            to: params.target.clone(),
            message_type: MessageType::Query,
            payload: params.message,
            timestamp: Utc::now(),
        };

        self.orchestrator.send_message(&params.target, message).await
            .map_err(|e| McpError::internal(e.to_string()))?;

        Ok(CommunicateResponse {
            delivered: true,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ListAgentsParams {}

#[derive(Debug, Serialize)]
pub struct ListAgentsResponse {
    pub running: Vec<AgentStatusInfo>,
    pub available: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct StartAgentParams {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct StartAgentResponse {
    pub agent_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct StopAgentParams {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct StopAgentResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct AgentStatusParams {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AgentStatusResponse {
    pub status: AgentStatusInfo,
}

#[derive(Debug, Deserialize)]
pub struct CommunicateParams {
    pub target: String,
    pub message: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct CommunicateResponse {
    pub delivered: bool,
}
```
    </section>
  </pseudo_code>
</task_spec>
```

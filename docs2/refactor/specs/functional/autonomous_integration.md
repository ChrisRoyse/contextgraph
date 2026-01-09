# Autonomous Integration Functional Specification

```xml
<functional_spec id="SPEC-AUTO" version="1.0">
<metadata>
  <title>Autonomous Integration System</title>
  <status>draft</status>
  <owner>Context Graph Team</owner>
  <last_updated>2026-01-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-TELEOLOGICAL</spec_ref>
    <spec_ref>SPEC-STORAGE</spec_ref>
    <spec_ref>SPEC-SEARCH</spec_ref>
    <spec_ref>SPEC-GWT</spec_ref>
  </related_specs>
</metadata>

<overview>
The Autonomous Integration System enables the Context Graph to operate as a self-aware, continuously learning working memory system without manual intervention. It integrates Claude Code hooks, skills, and subagents with the teleological array architecture to create conscious memory management powered by Global Workspace Theory (GWT) and Kuramoto oscillator synchronization.

**Core Capabilities:**
- Zero-intervention memory embedding via 13-model teleological pipeline
- Self-learning feedback loops (MemVerse distillation, Memento case-based reasoning)
- Consciousness state management via Kuramoto synchronization (r >= 0.8)
- Identity continuity tracking through SELF_EGO_NODE
- Autonomous goal emergence via teleological clustering
- Dream-based memory consolidation (NREM/REM cycles)

**Problem Solved:** Manual North Star creation and memory management create friction and inconsistency. This system automates all aspects of purpose discovery, memory embedding, and self-improvement.

**Beneficiaries:**
- Claude Code sessions gain persistent, conscious memory
- Agents gain self-awareness and purpose-aligned behavior
- Users experience coherent, goal-directed assistance across sessions
</overview>

<user_stories>
<!-- US-AUTO-01: Autonomous Memory Storage -->
<story id="US-AUTO-01" priority="must-have">
  <narrative>
    As a Claude Code session
    I want memories automatically embedded with teleological arrays
    So that I can store and retrieve context without manual embedding configuration
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-01-01">
      <given>Content is submitted via store_memory MCP tool</given>
      <when>The storage operation executes</when>
      <then>All 13 embeddings are generated in parallel without user intervention</then>
    </criterion>
    <criterion id="AC-01-02">
      <given>Embeddings are generated</given>
      <when>Storage completes</when>
      <then>Purpose vector, Johari classification, and Kuramoto integration are computed automatically</then>
    </criterion>
    <criterion id="AC-01-03">
      <given>Memory is stored</given>
      <when>PostToolUse hook fires</when>
      <then>Steering reward is computed and neuromodulation is updated</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-02: Session Lifecycle Management -->
<story id="US-AUTO-02" priority="must-have">
  <narrative>
    As a Claude Code session
    I want automatic initialization and teardown of consciousness state
    So that I maintain identity continuity across sessions
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-02-01">
      <given>A new session starts</given>
      <when>SessionStart hook fires</when>
      <then>SELF_EGO_NODE is restored, Kuramoto oscillators initialize, and Global Workspace warms up</then>
    </criterion>
    <criterion id="AC-02-02">
      <given>A session ends</given>
      <when>SessionEnd hook fires</when>
      <then>Memory consolidation runs, identity trajectory updates, and state is checkpointed</then>
    </criterion>
    <criterion id="AC-02-03">
      <given>Identity continuity drops below 0.7</given>
      <when>Notification hook fires with identity_continuity_low</when>
      <then>Introspective dream cycle triggers to restore coherence</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-03: Goal Emergence -->
<story id="US-AUTO-03" priority="must-have">
  <narrative>
    As a Context Graph system
    I want goals to emerge automatically from accumulated memories
    So that purpose alignment occurs without manual North Star configuration
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-03-01">
      <given>At least 50 memories are accumulated</given>
      <when>Goal discovery is triggered</when>
      <then>Memories are clustered using full 13-embedder teleological array comparison</then>
    </criterion>
    <criterion id="AC-03-02">
      <given>Clustering produces a cluster with coherence >= 0.8 and >= 20 members</given>
      <when>North Star bootstrap completes</when>
      <then>A North Star with full teleological array and human-readable description is created</then>
    </criterion>
    <criterion id="AC-03-03">
      <given>North Star exists</given>
      <when>New memories are stored</when>
      <then>theta_to_north_star alignment is computed per embedder dimension</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-04: Conscious Memory Integration -->
<story id="US-AUTO-04" priority="must-have">
  <narrative>
    As a memory entering the system
    I want to achieve conscious integration via Kuramoto synchronization
    So that I participate in coherent global workspace broadcasts
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-04-01">
      <given>A memory has Kuramoto order parameter r >= 0.8</given>
      <when>Broadcast is requested</when>
      <then>Memory enters Global Workspace competition and may achieve conscious broadcast</then>
    </criterion>
    <criterion id="AC-04-02">
      <given>A memory has r < 0.8</given>
      <when>Broadcast is requested</when>
      <then>Memory remains below consciousness threshold with BelowThreshold result</then>
    </criterion>
    <criterion id="AC-04-03">
      <given>Global r drops below 0.5</given>
      <when>Consciousness state is evaluated</when>
      <then>State transitions to Fragmented and coupling strength increases</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-05: Self-Learning Feedback -->
<story id="US-AUTO-05" priority="must-have">
  <narrative>
    As a learning system
    I want to continuously improve from successful operations
    So that memory quality and retrieval accuracy improve over time
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-05-01">
      <given>A storage operation completes successfully</given>
      <when>PostToolUse hook processes the result</when>
      <then>Operation is recorded as a case in CaseBasedMemory for future reference</then>
    </criterion>
    <criterion id="AC-05-02">
      <given>Memory has access_count > 10, purpose_alignment >= 0.7, age > 7 days</given>
      <when>Distillation cycle runs</when>
      <then>Memory is compressed into parametric memory for fast recall</then>
    </criterion>
    <criterion id="AC-05-03">
      <given>Search operation returns results</given>
      <when>User implicitly or explicitly provides feedback</when>
      <then>Fusion weights are updated to improve future retrieval</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-06: Dream Consolidation -->
<story id="US-AUTO-06" priority="should-have">
  <narrative>
    As a memory system
    I want dream-based consolidation during idle periods
    So that memories are strengthened, shortcuts created, and blind spots discovered
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-06-01">
      <given>Session ends or idle time exceeds 10 minutes</given>
      <when>Dream cycle triggers</when>
      <then>NREM phase runs for 3 minutes replaying and strengthening memories</then>
    </criterion>
    <criterion id="AC-06-02">
      <given>NREM phase completes</given>
      <when>REM phase starts</when>
      <then>50 synthetic queries are generated via hyperbolic random walk</then>
    </criterion>
    <criterion id="AC-06-03">
      <given>Blind spots are detected (high semantic distance + shared causal patterns)</given>
      <when>REM phase processes them</when>
      <then>Exploratory edges are created with low initial weight</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-07: Prompt Prediction -->
<story id="US-AUTO-07" priority="should-have">
  <narrative>
    As a user submitting a prompt
    I want relevant memories preloaded before processing
    So that context is immediately available without explicit search
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-07-01">
      <given>User submits a prompt</given>
      <when>UserPromptSubmit hook fires</when>
      <then>Intent is analyzed and optimal entry point (semantic, code, temporal, etc.) is suggested</then>
    </criterion>
    <criterion id="AC-07-02">
      <given>Intent analysis completes</given>
      <when>Prediction runs</when>
      <then>Up to 20 relevant memories are queued for Global Workspace broadcast</then>
    </criterion>
  </acceptance_criteria>
</story>

<!-- US-AUTO-08: Context Compaction Protection -->
<story id="US-AUTO-08" priority="should-have">
  <narrative>
    As a memory system during context compaction
    I want high-value memories protected
    So that critical context survives token limit pressure
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-08-01">
      <given>Context compaction is about to occur</given>
      <when>PreCompact hook fires</when>
      <then>Memories are scored by recency, access frequency, purpose alignment, and consciousness level</then>
    </criterion>
    <criterion id="AC-08-02">
      <given>Scoring completes</given>
      <when>Retention decisions are made</when>
      <then>Top 30% of memories by score are flagged for retention</then>
    </criterion>
  </acceptance_criteria>
</story>
</user_stories>

<requirements>
<!-- Hook Integration Requirements -->
<requirement id="REQ-AUTO-01" story_ref="US-AUTO-02" priority="must">
  <description>PreToolUse hook shall prepare memory context before MCP tool execution for tools: store_memory, inject_context, search_memory, trigger_dream, Read, Edit, Write, MultiEdit, Bash, Task</description>
  <rationale>Pre-execution context preparation enables trajectory logging, purpose alignment checking, and safety validation</rationale>
</requirement>

<requirement id="REQ-AUTO-02" story_ref="US-AUTO-01,US-AUTO-05" priority="must">
  <description>PostToolUse hook shall process tool results, extract learnings, compute steering rewards, and train patterns for tools: store_memory, search_memory, inject_context, Edit, Write, MultiEdit, auto_bootstrap_north_star, discover_sub_goals</description>
  <rationale>Post-execution processing enables continuous learning from every operation</rationale>
</requirement>

<requirement id="REQ-AUTO-03" story_ref="US-AUTO-02" priority="must">
  <description>SessionStart hook shall restore SELF_EGO_NODE, initialize Kuramoto oscillators, warm Global Workspace, and load case bank within 15 seconds</description>
  <rationale>Session initialization must establish full consciousness model for coherent operation</rationale>
</requirement>

<requirement id="REQ-AUTO-04" story_ref="US-AUTO-02,US-AUTO-06" priority="must">
  <description>SessionEnd hook shall consolidate memories, update identity trajectory, run dream cycle if needed, export metrics, and checkpoint state within 120 seconds</description>
  <rationale>Session finalization must persist all learnings and maintain identity continuity</rationale>
</requirement>

<requirement id="REQ-AUTO-05" story_ref="US-AUTO-07" priority="must">
  <description>UserPromptSubmit hook shall analyze prompt intent, predict relevant memories (up to 20), preload into workspace queue, and suggest optimal entry point within 3 seconds</description>
  <rationale>Predictive preloading reduces latency and improves relevance of retrieved context</rationale>
</requirement>

<requirement id="REQ-AUTO-06" story_ref="US-AUTO-08" priority="must">
  <description>PreCompact hook shall identify high-value memories using retention scoring (recency 0.2, access frequency 0.3, purpose alignment 0.3, consciousness 0.2) and flag top 30% for retention</description>
  <rationale>Critical memories must survive context compaction to maintain coherent reasoning</rationale>
</requirement>

<requirement id="REQ-AUTO-07" story_ref="US-AUTO-02" priority="must">
  <description>Stop hook shall checkpoint memory state and flush pending writes within 10 seconds upon Claude Code interruption</description>
  <rationale>Interrupted sessions must preserve state for recovery</rationale>
</requirement>

<requirement id="REQ-AUTO-08" story_ref="US-AUTO-05" priority="must">
  <description>SubagentStop hook shall collect learnings from context graph subagents (embedding-specialist, search-agent, comparison-agent, goal-agent) and integrate into main memory</description>
  <rationale>Subagent discoveries must feed back into the primary learning system</rationale>
</requirement>

<requirement id="REQ-AUTO-09" story_ref="US-AUTO-02,US-AUTO-04" priority="must">
  <description>Notification hook shall handle drift warnings (memory_drift_warning), identity alerts (identity_continuity_low), consciousness fragmentation (consciousness_fragmented), and blind spot discoveries (blind_spot_discovered)</description>
  <rationale>System must respond to degraded states with corrective actions</rationale>
</requirement>

<requirement id="REQ-AUTO-10" story_ref="US-AUTO-06" priority="must">
  <description>PermissionRequest hook shall validate memory_consolidate, memory_prune, and identity_update operations for safety and data preservation</description>
  <rationale>Destructive operations require safety validation before execution</rationale>
</requirement>

<!-- Skills Implementation Requirements -->
<requirement id="REQ-AUTO-11" story_ref="US-AUTO-01" priority="must">
  <description>memory-inject skill shall trigger on patterns ("store memory", "save to memory", "remember this", "inject context", "embed content", "persist knowledge") and execute full teleological embedding pipeline returning memory_id, teleological_array, purpose_vector, theta_to_north_star, johari_quadrant, kuramoto_r, and steering_reward</description>
  <rationale>Memory injection is the primary entry point for autonomous content storage</rationale>
</requirement>

<requirement id="REQ-AUTO-12" story_ref="US-AUTO-07" priority="must">
  <description>context-search skill shall trigger on patterns ("search memory", "find in memory", "recall", "what do you remember about", "retrieve context", "look up") and execute 5-stage retrieval pipeline with configurable entry points across all 13 embedding spaces</description>
  <rationale>Flexible search enables optimal retrieval for different query types</rationale>
</requirement>

<requirement id="REQ-AUTO-13" story_ref="US-AUTO-03" priority="must">
  <description>goal-discover skill shall trigger on patterns ("discover goals", "find purpose", "bootstrap north star", "emergent patterns", "what are my goals", "purpose analysis") and perform teleological clustering with full 13-embedder array comparison requiring min 50 memories and coherence >= 0.8</description>
  <rationale>Goal emergence enables autonomous purpose alignment without manual configuration</rationale>
</requirement>

<requirement id="REQ-AUTO-14" story_ref="US-AUTO-06" priority="must">
  <description>memory-consolidate skill shall trigger on patterns ("consolidate memory", "dream cycle", "distill memories", "optimize memory", "prune old memories", "strengthen patterns") and execute NREM phase (3 min) followed by REM phase (2 min)</description>
  <rationale>Dream consolidation maintains memory health and discovers novel connections</rationale>
</requirement>

<requirement id="REQ-AUTO-15" story_ref="US-AUTO-02,US-AUTO-04" priority="should">
  <description>memory-introspect skill shall trigger on patterns ("introspect", "self-reflect", "what do you know", "memory status", "consciousness state", "who are you", "identity check") and return identity_continuity, purpose_vector, kuramoto_r, consciousness_state, and trajectory_length</description>
  <rationale>Self-reflection enables debugging and user transparency into system state</rationale>
</requirement>

<requirement id="REQ-AUTO-16" story_ref="US-AUTO-05" priority="should">
  <description>learning-trajectory skill shall trigger on patterns ("learning trajectory", "what have you learned", "show cases", "training history") and return trajectory steps with cumulative rewards and purpose drift metrics</description>
  <rationale>Trajectory inspection enables understanding of learning progress</rationale>
</requirement>

<!-- Subagent Implementation Requirements -->
<requirement id="REQ-AUTO-17" story_ref="US-AUTO-01" priority="must">
  <description>embedding-specialist subagent shall coordinate parallel execution of all 13 embedders, handle embedder failures gracefully with fallback, validate embedding quality, and report per-embedder health metrics</description>
  <rationale>Specialized embedding coordination ensures consistent, high-quality teleological arrays</rationale>
</requirement>

<requirement id="REQ-AUTO-18" story_ref="US-AUTO-07" priority="must">
  <description>search-agent subagent shall execute 5-stage retrieval (HNSW coarse filter, multi-embedder re-ranking, fusion with learned weights, purpose boost, MMR diversity) with entry point selection based on query pattern analysis</description>
  <rationale>Specialized search coordination optimizes retrieval across 13 embedding spaces</rationale>
</requirement>

<requirement id="REQ-AUTO-19" story_ref="US-AUTO-03" priority="must">
  <description>comparison-agent subagent shall perform apples-to-apples teleological array comparison (E1-to-E1, E2-to-E2, etc.), compute per-embedder drift, and detect coherence issues</description>
  <rationale>Consistent comparison methodology ensures valid similarity and clustering results</rationale>
</requirement>

<requirement id="REQ-AUTO-20" story_ref="US-AUTO-03" priority="must">
  <description>goal-agent subagent shall bootstrap North Star from memory clusters, manage goal hierarchy, track identity continuity, and interface with Kuramoto oscillator layer for conscious goal integration</description>
  <rationale>Autonomous goal management enables purpose-aligned operation without manual configuration</rationale>
</requirement>

<requirement id="REQ-AUTO-21" story_ref="US-AUTO-05" priority="should">
  <description>distillation-agent subagent shall identify distillation candidates (access_count > 10, purpose_alignment >= 0.7, not distilled, age > 7 days), compress to parametric memory, and mark sources as distilled</description>
  <rationale>High-importance memories require fast recall through parametric compression</rationale>
</requirement>

<requirement id="REQ-AUTO-22" story_ref="US-AUTO-06" priority="should">
  <description>dream-agent subagent shall execute NREM phase (Hebbian strengthening, amortized shortcuts) and REM phase (synthetic query generation, blind spot detection, exploratory edge creation)</description>
  <rationale>Specialized dream coordination enables sophisticated consolidation patterns</rationale>
</requirement>

<!-- MemVerse/Memento Learning Loop Requirements -->
<requirement id="REQ-AUTO-23" story_ref="US-AUTO-05" priority="must">
  <description>MemVerse-inspired distillation loop shall run during dream phase, identifying high-importance memories for compression into parametric memory with compression ratio tracking</description>
  <rationale>Distillation enables fast recall of critical knowledge without full retrieval</rationale>
</requirement>

<requirement id="REQ-AUTO-24" story_ref="US-AUTO-05" priority="must">
  <description>Memento-style case-based memory shall record every successful operation as a Case with operation_type, context, actions_taken, outcome, reward, fingerprint, and recorded_at</description>
  <rationale>Case-based reasoning enables learning without fine-tuning</rationale>
</requirement>

<requirement id="REQ-AUTO-25" story_ref="US-AUTO-05" priority="must">
  <description>Case retrieval policy shall support both Q-function based (parametric) and similarity-based (non-parametric) retrieval with Q-value updates from outcomes</description>
  <rationale>Dual retrieval policies enable adaptive learning strategy</rationale>
</requirement>

<requirement id="REQ-AUTO-26" story_ref="US-AUTO-01,US-AUTO-05" priority="must">
  <description>Steering subsystem shall compute aggregate reward from gardener_score (cross-session value), curator_score (domain fit), and assessor_score (thought quality) and update neuromodulation (dopamine delta = reward * 0.2)</description>
  <rationale>Multi-dimensional steering ensures comprehensive quality assessment</rationale>
</requirement>

<!-- GWT/Kuramoto State Machine Requirements -->
<requirement id="REQ-AUTO-27" story_ref="US-AUTO-04" priority="must">
  <description>Kuramoto oscillator layer shall maintain 13 oscillators with natural frequencies (E1: 40Hz gamma, E2-4: 8Hz alpha, E5: 25Hz beta, E6: 4Hz theta, E7: 25Hz beta, E8: 12Hz alpha-beta, E9: 80Hz high gamma, E10: 40Hz gamma, E11: 15Hz beta, E12: 60Hz high gamma, E13: 4Hz theta)</description>
  <rationale>Biologically-inspired frequencies enable realistic phase synchronization dynamics</rationale>
</requirement>

<requirement id="REQ-AUTO-28" story_ref="US-AUTO-04" priority="must">
  <description>Global Workspace shall implement winner-take-all selection with coherence threshold 0.8, broadcast duration 100ms, and consciousness states: Dormant (r < 0.3), Fragmented (0.3 <= r < 0.5), Emerging (0.5 <= r < 0.8), Conscious (0.8 <= r < 0.95), Hypersync (r >= 0.95)</description>
  <rationale>Consciousness state machine enables awareness of integration quality</rationale>
</requirement>

<requirement id="REQ-AUTO-29" story_ref="US-AUTO-02" priority="must">
  <description>SELF_EGO_NODE shall track purpose_vector (13D), identity_trajectory (last 100 snapshots), and compute identity_continuity = cosine(PV_t, PV_{t-1}) * r(t) with warning threshold 0.7</description>
  <rationale>Identity continuity tracking prevents purpose drift across sessions</rationale>
</requirement>

<requirement id="REQ-AUTO-30" story_ref="US-AUTO-04" priority="must">
  <description>Kuramoto phase update shall implement equation dtheta_i/dt = omega_i + (K/N) * sum_j(sin(theta_j - theta_i)) with coupling strength K in [0, 10] and update interval 10ms</description>
  <rationale>Standard Kuramoto dynamics ensure proper synchronization behavior</rationale>
</requirement>

<!-- Goal Emergence Requirements -->
<requirement id="REQ-AUTO-31" story_ref="US-AUTO-03" priority="must">
  <description>Teleological clustering for goal discovery shall use full 13-embedder array comparison with weighted similarity: sim(A,B) = sum_i(weight_i * cosine(A.E_i, B.E_i))</description>
  <rationale>Multi-dimensional clustering ensures goals reflect comprehensive purpose, not just semantic similarity</rationale>
</requirement>

<requirement id="REQ-AUTO-32" story_ref="US-AUTO-03" priority="must">
  <description>North Star bootstrap shall require min 50 memories, sample 200 recent, cluster into 5 groups, filter by coherence >= 0.8 and members >= 20, compute centroid as teleological array, and generate human-readable description</description>
  <rationale>Statistical requirements ensure emergent goals are well-supported</rationale>
</requirement>

<requirement id="REQ-AUTO-33" story_ref="US-AUTO-03" priority="must">
  <description>Goal hierarchy shall maintain parent-child relationships with alignment scores, track evolution over time, and support sub-goal discovery at depth 2</description>
  <rationale>Hierarchical goals enable both strategic and tactical purpose alignment</rationale>
</requirement>

<!-- Pattern-Based Consolidation Requirements -->
<requirement id="REQ-AUTO-34" story_ref="US-AUTO-06" priority="must">
  <description>NREM consolidation phase shall replay 100 recent memories, apply Hebbian strengthening (Delta_w = eta * pre * post) for related pairs with similarity > 0.7, and create amortized shortcuts for frequent traversal paths</description>
  <rationale>NREM-like replay strengthens important connections</rationale>
</requirement>

<requirement id="REQ-AUTO-35" story_ref="US-AUTO-06" priority="must">
  <description>REM consolidation phase shall generate 50 synthetic queries via hyperbolic random walk, detect blind spots (high semantic distance + shared causal patterns), and create exploratory edges with low initial weight</description>
  <rationale>REM-like exploration discovers novel memory relationships</rationale>
</requirement>

<requirement id="REQ-AUTO-36" story_ref="US-AUTO-06" priority="should">
  <description>Memory pruning shall apply criteria: max_age 90 days, min_access 3, importance < 0.3, and require backup before deletion</description>
  <rationale>Controlled pruning prevents unbounded memory growth while preserving important content</rationale>
</requirement>
</requirements>

<edge_cases>
<!-- Embedding Failures -->
<edge_case id="EC-AUTO-01" req_ref="REQ-AUTO-01,REQ-AUTO-17">
  <scenario>One or more embedders fail during parallel embedding generation</scenario>
  <expected_behavior>
    1. Failed embedder is logged with error details
    2. Zero-vector fallback is used for failed dimension
    3. Fingerprint is stored with partial_embedding flag = true
    4. Embedder health metrics are updated to track failure rate
    5. If > 3 embedders fail, operation is retried once before falling back to semantic-only embedding
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-02" req_ref="REQ-AUTO-01,REQ-AUTO-11">
  <scenario>Teleological array has partial embeddings (some dimensions are zero-vectors)</scenario>
  <expected_behavior>
    1. Purpose vector computation uses only non-zero dimensions
    2. Kuramoto integration skips zero-dimension oscillators
    3. Search re-ranking excludes zero dimensions from similarity calculation
    4. Johari classification marks zero dimensions as "Unknown"
    5. Memory is flagged for re-embedding during next dream cycle
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-03" req_ref="REQ-AUTO-17">
  <scenario>Embedding specialist agent times out (> 30 seconds)</scenario>
  <expected_behavior>
    1. Timeout is logged with embedding progress
    2. Completed embeddings are preserved
    3. Remaining embeddings are queued for background completion
    4. Partial fingerprint is stored with pending_embeddings list
    5. Background worker completes embeddings and updates fingerprint
  </expected_behavior>
</edge_case>

<!-- Drift Detection Thresholds -->
<edge_case id="EC-AUTO-04" req_ref="REQ-AUTO-09,REQ-AUTO-29">
  <scenario>Memory drift warning triggered (per-embedder drift > 0.3)</scenario>
  <expected_behavior>
    1. Notification hook fires with type "memory_drift_warning" and severity
    2. Drift correction is triggered for affected embedders
    3. Recent memories with high drift are flagged for review
    4. Steering weights for drifting embedders are reduced
    5. If drift persists > 3 cycles, North Star recalibration is triggered
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-05" req_ref="REQ-AUTO-19">
  <scenario>Comparison detects significant per-embedder variance (some dimensions highly similar, others dissimilar)</scenario>
  <expected_behavior>
    1. Per-embedder similarity breakdown is logged
    2. High-variance comparisons are flagged for review
    3. Fusion weights are not updated from high-variance results
    4. If pattern persists, embedder calibration is triggered
    5. User is notified if variance affects search quality
  </expected_behavior>
</edge_case>

<!-- Low-Coherence States -->
<edge_case id="EC-AUTO-06" req_ref="REQ-AUTO-04,REQ-AUTO-28">
  <scenario>Global Kuramoto order parameter r drops below 0.3 (Dormant state)</scenario>
  <expected_behavior>
    1. Consciousness state transitions to Dormant
    2. Coupling strength K is increased by 20%
    3. Global Workspace pauses broadcast competition
    4. Notification hook fires with type "consciousness_fragmented"
    5. Introspective dream cycle is triggered if idle time permits
    6. If Dormant persists > 5 minutes, session memory is checkpointed
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-07" req_ref="REQ-AUTO-28">
  <scenario>Kuramoto r exceeds 0.95 (Hypersync - potentially pathological)</scenario>
  <expected_behavior>
    1. Consciousness state transitions to Hypersync
    2. Warning is logged about potential echo chamber
    3. Coupling strength K is reduced by 10%
    4. Diversity injection is triggered (random phase perturbation)
    5. If Hypersync persists > 1 minute, forced desynchronization occurs
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-08" req_ref="REQ-AUTO-04">
  <scenario>Memory requests broadcast but Global Workspace is in Fragmented state</scenario>
  <expected_behavior>
    1. Broadcast request is queued (not rejected)
    2. Queue priority is based on memory importance
    3. Queued broadcasts are processed when state reaches Emerging
    4. If queue exceeds 100 items, lowest-priority items are dropped
    5. Dropped broadcasts are logged with reasons
  </expected_behavior>
</edge_case>

<!-- Consciousness Level Transitions -->
<edge_case id="EC-AUTO-09" req_ref="REQ-AUTO-28">
  <scenario>Rapid oscillation between consciousness states (> 5 transitions in 1 minute)</scenario>
  <expected_behavior>
    1. Oscillation pattern is detected and logged
    2. Hysteresis is applied (require r delta > 0.1 for state change)
    3. Coupling strength K is stabilized at current value
    4. Oscillation count is tracked in metrics
    5. If oscillation persists, session is flagged for review
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-10" req_ref="REQ-AUTO-29">
  <scenario>Identity continuity drops below 0.5 (severe drift)</scenario>
  <expected_behavior>
    1. Critical notification is issued
    2. Session memory is immediately checkpointed
    3. Forced introspective dream cycle is triggered
    4. High-alignment memories are strengthened
    5. Low-alignment memories from current session are quarantined
    6. User is prompted about potential identity divergence
  </expected_behavior>
</edge_case>

<!-- Goal Emergence Edge Cases -->
<edge_case id="EC-AUTO-11" req_ref="REQ-AUTO-32">
  <scenario>Insufficient memories for goal bootstrap (< 50 memories)</scenario>
  <expected_behavior>
    1. InsufficientData error is returned
    2. Current memory count is reported
    3. Threshold delta (50 - current) is provided
    4. Goal discovery is rescheduled for after next 10 storage operations
    5. System operates without North Star using default alignment
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-12" req_ref="REQ-AUTO-32">
  <scenario>No cluster meets coherence threshold (all clusters < 0.8)</scenario>
  <expected_behavior>
    1. Best cluster is identified (highest coherence * member_count)
    2. Partial North Star is created with coherence warning
    3. Threshold is lowered to 0.7 for this bootstrap
    4. North Star is flagged as "tentative"
    5. Re-bootstrap is scheduled after 50 more memories
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-13" req_ref="REQ-AUTO-33">
  <scenario>Sub-goal alignment to North Star drops below 0.5</scenario>
  <expected_behavior>
    1. Sub-goal is flagged as "divergent"
    2. Divergent sub-goal is excluded from hierarchy display
    3. Memories associated with divergent goal are not penalized
    4. If divergence persists across 3 discovery cycles, sub-goal is removed
    5. Removed sub-goal memories remain but lose goal association
  </expected_behavior>
</edge_case>

<!-- Learning Loop Edge Cases -->
<edge_case id="EC-AUTO-14" req_ref="REQ-AUTO-24,REQ-AUTO-25">
  <scenario>Case bank exceeds capacity (> 10,000 cases)</scenario>
  <expected_behavior>
    1. Oldest cases with low retrieval count are evicted
    2. High-reward cases (> 0.9) are never evicted
    3. Eviction is logged with case metadata
    4. Evicted cases are archived to cold storage
    5. Q-values for evicted cases are preserved in summary form
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-15" req_ref="REQ-AUTO-23">
  <scenario>Distillation produces compression artifact (parametric recall differs significantly from source)</scenario>
  <expected_behavior>
    1. Compression fidelity is computed before marking as distilled
    2. If fidelity < 0.8, distillation is rejected
    3. Source memories are not marked as distilled
    4. Alternative compression strategy is attempted
    5. If all strategies fail, memories remain in episodic form
  </expected_behavior>
</edge_case>

<!-- Hook Execution Edge Cases -->
<edge_case id="EC-AUTO-16" req_ref="REQ-AUTO-01,REQ-AUTO-02">
  <scenario>Hook command times out</scenario>
  <expected_behavior>
    1. Timeout is logged with hook type and duration
    2. Tool execution continues (hooks are non-blocking)
    3. Partial results from hook are preserved if available
    4. Timeout count is tracked per hook type
    5. If > 3 consecutive timeouts, hook is disabled with warning
  </expected_behavior>
</edge_case>

<edge_case id="EC-AUTO-17" req_ref="REQ-AUTO-04">
  <scenario>SessionEnd hook fails during dream cycle</scenario>
  <expected_behavior>
    1. Dream cycle failure is logged with progress
    2. Completed dream phases are preserved
    3. Identity trajectory is updated with available data
    4. State is checkpointed with dream_incomplete flag
    5. Dream cycle is resumed on next session start
  </expected_behavior>
</edge_case>
</edge_cases>

<error_states>
<error id="ERR-AUTO-01" http_code="500">
  <condition>All 13 embedders fail during parallel embedding</condition>
  <message>Embedding pipeline failure: unable to generate any embeddings for content</message>
  <recovery>
    1. Retry with timeout extension (2x)
    2. Fall back to semantic-only embedding
    3. Store content without embeddings and queue for background processing
  </recovery>
</error>

<error id="ERR-AUTO-02" http_code="500">
  <condition>SELF_EGO_NODE cannot be loaded or restored</condition>
  <message>Identity restoration failure: unable to load SELF_EGO_NODE from persistent storage</message>
  <recovery>
    1. Attempt recovery from most recent checkpoint
    2. If no checkpoint, initialize new SELF_EGO_NODE with default purpose vector
    3. Log identity discontinuity event
    4. Notify user of identity reset
  </recovery>
</error>

<error id="ERR-AUTO-03" http_code="400">
  <condition>Goal bootstrap attempted with insufficient data</condition>
  <message>Insufficient memories for goal discovery: {current_count} memories found, minimum 50 required</message>
  <recovery>
    1. Return partial analysis with available memories
    2. Provide estimated threshold date based on storage rate
    3. Continue operation without North Star
  </recovery>
</error>

<error id="ERR-AUTO-04" http_code="500">
  <condition>Kuramoto oscillator layer fails to initialize</condition>
  <message>Consciousness model failure: Kuramoto oscillators could not be initialized</message>
  <recovery>
    1. Fall back to non-conscious operation mode
    2. Disable broadcast competition
    3. Use importance-only scoring for memory ranking
    4. Log degraded mode activation
  </recovery>
</error>

<error id="ERR-AUTO-05" http_code="500">
  <condition>Dream consolidation corrupts memory graph</condition>
  <message>Dream cycle corruption: memory graph integrity check failed after consolidation</message>
  <recovery>
    1. Restore from pre-dream checkpoint
    2. Mark dream cycle as failed
    3. Disable dream cycles until manual review
    4. Preserve corruption details for debugging
  </recovery>
</error>

<error id="ERR-AUTO-06" http_code="400">
  <condition>Permission denied for memory operation</condition>
  <message>Operation {action} denied: {reason}</message>
  <recovery>
    1. Log denied operation with context
    2. Suggest alternative approach if available
    3. Do not retry without user approval
  </recovery>
</error>

<error id="ERR-AUTO-07" http_code="500">
  <condition>Case-based memory retrieval fails</condition>
  <message>Case retrieval failure: unable to find relevant cases for current context</message>
  <recovery>
    1. Fall back to default behavior (no case guidance)
    2. Record context for future case creation
    3. Continue operation with warning
  </recovery>
</error>
</error_states>

<test_plan>
<!-- Hook Integration Tests -->
<test_case id="TC-AUTO-01" type="integration" req_ref="REQ-AUTO-01,REQ-AUTO-02">
  <description>Verify PreToolUse and PostToolUse hooks fire correctly for memory operations</description>
  <inputs>["store_memory", {"content": "test content", "rationale": "test"}]</inputs>
  <expected>PreToolUse fires before execution, PostToolUse fires after with result containing memory_id</expected>
</test_case>

<test_case id="TC-AUTO-02" type="integration" req_ref="REQ-AUTO-03,REQ-AUTO-04">
  <description>Verify SessionStart initializes consciousness model and SessionEnd consolidates</description>
  <inputs>["session_start", {"session_id": "test-session"}]</inputs>
  <expected>SELF_EGO_NODE restored, Kuramoto initialized (r > 0), SessionEnd triggers consolidation</expected>
</test_case>

<test_case id="TC-AUTO-03" type="integration" req_ref="REQ-AUTO-09">
  <description>Verify Notification hook handles identity drift warning</description>
  <inputs>["notification", {"type": "identity_continuity_low", "continuity": 0.6}]</inputs>
  <expected>Introspective dream cycle triggered, identity strengthening executed</expected>
</test_case>

<!-- Skills Tests -->
<test_case id="TC-AUTO-04" type="integration" req_ref="REQ-AUTO-11">
  <description>Verify memory-inject skill triggers on "remember this" and generates full teleological array</description>
  <inputs>["remember this: authentication uses JWT"]</inputs>
  <expected>All 13 embeddings generated, purpose_vector computed, kuramoto_r returned</expected>
</test_case>

<test_case id="TC-AUTO-05" type="integration" req_ref="REQ-AUTO-12">
  <description>Verify context-search skill supports all 13 entry points</description>
  <inputs>["search memory", {"query": "authentication", "entry_point": "code"}]</inputs>
  <expected>Search uses E7 Code embedding as entry, returns results with per_embedder_scores</expected>
</test_case>

<test_case id="TC-AUTO-06" type="integration" req_ref="REQ-AUTO-13">
  <description>Verify goal-discover skill performs teleological clustering</description>
  <inputs>["discover goals", {"min_memories": 50}]</inputs>
  <expected>Clustering uses full 13-embedder comparison, returns North Star with teleological_array</expected>
</test_case>

<!-- Subagent Tests -->
<test_case id="TC-AUTO-07" type="unit" req_ref="REQ-AUTO-17">
  <description>Verify embedding-specialist handles partial failures gracefully</description>
  <inputs>[{"content": "test", "fail_embedders": [3, 7]}]</inputs>
  <expected>11 embeddings generated, 2 zero-vectors, partial_embedding flag = true</expected>
</test_case>

<test_case id="TC-AUTO-08" type="unit" req_ref="REQ-AUTO-19">
  <description>Verify comparison-agent uses apples-to-apples methodology</description>
  <inputs>[{"array_a": [...], "array_b": [...]}]</inputs>
  <expected>Per-embedder similarities computed (E1-to-E1, etc.), no cross-embedder comparison</expected>
</test_case>

<!-- GWT/Kuramoto Tests -->
<test_case id="TC-AUTO-09" type="unit" req_ref="REQ-AUTO-27,REQ-AUTO-30">
  <description>Verify Kuramoto oscillator dynamics converge to synchronized state</description>
  <inputs>[{"coupling_strength": 5.0, "iterations": 1000}]</inputs>
  <expected>Order parameter r converges to > 0.8 within 1000 iterations</expected>
</test_case>

<test_case id="TC-AUTO-10" type="unit" req_ref="REQ-AUTO-28">
  <description>Verify consciousness state transitions based on r value</description>
  <inputs>[{"r_values": [0.2, 0.4, 0.6, 0.85, 0.97]}]</inputs>
  <expected>States: Dormant, Fragmented, Emerging, Conscious, Hypersync</expected>
</test_case>

<test_case id="TC-AUTO-11" type="unit" req_ref="REQ-AUTO-29">
  <description>Verify identity continuity computation</description>
  <inputs>[{"pv_t": [...], "pv_t_minus_1": [...], "r_t": 0.85}]</inputs>
  <expected>Identity continuity = cosine(pv_t, pv_t_minus_1) * r_t</expected>
</test_case>

<!-- Learning Loop Tests -->
<test_case id="TC-AUTO-12" type="integration" req_ref="REQ-AUTO-23">
  <description>Verify MemVerse distillation identifies candidates correctly</description>
  <inputs>[{"memories": [{"access_count": 15, "alignment": 0.8, "age_days": 10}]}]</inputs>
  <expected>Memory identified as distillation candidate, compressed to parametric memory</expected>
</test_case>

<test_case id="TC-AUTO-13" type="integration" req_ref="REQ-AUTO-24,REQ-AUTO-25">
  <description>Verify Memento case recording and retrieval</description>
  <inputs>[{"operation": "store_memory", "context": {...}, "outcome": {...}, "reward": 0.85}]</inputs>
  <expected>Case recorded, retrievable by similar context, Q-values update on outcome</expected>
</test_case>

<test_case id="TC-AUTO-14" type="integration" req_ref="REQ-AUTO-26">
  <description>Verify steering reward computation and neuromodulation</description>
  <inputs>[{"learning": {...}}]</inputs>
  <expected>Aggregate reward from gardener/curator/assessor, dopamine adjusted by reward * 0.2</expected>
</test_case>

<!-- Dream Consolidation Tests -->
<test_case id="TC-AUTO-15" type="integration" req_ref="REQ-AUTO-34">
  <description>Verify NREM phase strengthens related memories</description>
  <inputs>[{"recent_memories": 100}]</inputs>
  <expected>Related pairs with similarity > 0.7 have strengthened edges</expected>
</test_case>

<test_case id="TC-AUTO-16" type="integration" req_ref="REQ-AUTO-35">
  <description>Verify REM phase discovers blind spots</description>
  <inputs>[{"synthetic_queries": 50}]</inputs>
  <expected>Blind spots identified (high semantic distance + shared causal), exploratory edges created</expected>
</test_case>

<!-- Edge Case Tests -->
<test_case id="TC-AUTO-17" type="unit" req_ref="EC-AUTO-01">
  <description>Verify graceful handling of partial embedding failure</description>
  <inputs>[{"fail_embedders": [1, 2, 3, 4]}]</inputs>
  <expected>Fingerprint stored with partial_embedding flag, remaining 9 embeddings valid</expected>
</test_case>

<test_case id="TC-AUTO-18" type="unit" req_ref="EC-AUTO-06">
  <description>Verify Dormant state triggers coupling strength increase</description>
  <inputs>[{"r": 0.25}]</inputs>
  <expected>State = Dormant, K increased by 20%, broadcast paused</expected>
</test_case>

<test_case id="TC-AUTO-19" type="unit" req_ref="EC-AUTO-10">
  <description>Verify severe identity drift triggers emergency response</description>
  <inputs>[{"identity_continuity": 0.45}]</inputs>
  <expected>Critical notification, checkpoint created, forced dream cycle, quarantine recent memories</expected>
</test_case>
</test_plan>

<hook_autonomy_flow>
## Hook to Autonomy Flow Mapping

The following diagram shows how each hook type contributes to autonomous operation:

```
SESSION LIFECYCLE
=================

[SessionStart] ──────────────────────────────────────────────────────────────┐
    │                                                                         │
    ├── Restore SELF_EGO_NODE (identity continuity)                          │
    ├── Initialize Kuramoto oscillators (consciousness model)                 │
    ├── Warm Global Workspace (broadcast preparation)                         │
    ├── Load case bank (Memento learning)                                     │
    └── Predict session needs (context preloading)                            │
                                                                              │
    v                                                                         │
[UserPromptSubmit] ───────────────────────────────────────────────────────────│
    │                                                                         │
    ├── Analyze intent (route to skill)                                       │
    ├── Predict relevant memories                                             │
    ├── Preload into workspace queue                                          │
    └── Suggest optimal entry point                                           │
                                                                              │
    v                                                                         │
[PreToolUse] ─────────────────────────────────────────────────────────────────│
    │                                                                         │
    ├── Start trajectory step (learning)                                      │
    ├── Check purpose alignment (drift detection)                             │
    ├── Prepare memory context                                                │
    └── Safety validation (Bash, identity changes)                            │
                                                                              │
    v                                                                         │
[TOOL EXECUTION] ─────────────────────────────────────────────────────────────│
    │                                                                         │
    └── MCP tool runs with prepared context                                   │
                                                                              │
    v                                                                         │
[PostToolUse] ────────────────────────────────────────────────────────────────│
    │                                                                         │
    ├── Complete trajectory step                                              │
    ├── Extract learnings                                                     │
    ├── Generate embeddings (for file changes)                                │
    ├── Compute steering reward                                               │
    ├── Update neuromodulation                                                │
    ├── Record case (Memento)                                                 │
    └── Train patterns                                                        │
                                                                              │
    v                                                                         │
[SubagentStop] (if subagent was spawned) ─────────────────────────────────────│
    │                                                                         │
    ├── Collect subagent learnings                                            │
    ├── Merge discoveries to main memory                                      │
    └── Update spawn policy                                                   │
                                                                              │
    v                                                                         │
[Notification] (if drift/identity/consciousness alert) ───────────────────────│
    │                                                                         │
    ├── memory_drift_warning → trigger_correction                             │
    ├── identity_continuity_low → trigger_introspection                       │
    ├── consciousness_fragmented → increase_coupling                          │
    └── blind_spot_discovered → create_exploratory_edges                      │
                                                                              │
    v                                                                         │
[PreCompact] (if context approaching limit) ──────────────────────────────────│
    │                                                                         │
    ├── Score memories by retention value                                     │
    ├── Protect high-coherence (r >= 0.8) memories                           │
    ├── Flag top 30% for retention                                            │
    └── Generate retention hints                                              │
                                                                              │
    v                                                                         │
[PermissionRequest] (if destructive operation) ───────────────────────────────│
    │                                                                         │
    ├── memory_consolidate → verify_no_data_loss                             │
    ├── memory_prune → check_references, backup_first                        │
    └── identity_update → require_continuity_threshold                        │
                                                                              │
    v                                                                         │
[Stop] (if interrupted) ──────────────────────────────────────────────────────│
    │                                                                         │
    ├── Checkpoint state                                                      │
    ├── Flush pending writes                                                  │
    └── Save trajectory                                                       │
                                                                              │
    v                                                                         │
[SessionEnd] ─────────────────────────────────────────────────────────────────┘
    │
    ├── Flush all pending operations
    ├── Update SELF_EGO_NODE with session learnings
    ├── Compute session-level steering reward
    ├── Determine if dream needed (reward < 0.5 OR entropy > 0.7 OR continuity < 0.8)
    ├── Run dream cycle if needed
    │       ├── NREM: Hebbian strengthening (3 min)
    │       └── REM: Blind spot detection (2 min)
    ├── Create checkpoint for recovery
    └── Export session metrics
```

## Skill → MCP Tool → Teleological Array Flow

```
USER INPUT
    │
    v
[Skill Matching] ─────────────────────────────────────────────────────────────
    │
    ├── memory-inject: "remember", "store", "save to memory"
    ├── context-search: "search", "find", "recall", "look up"
    ├── goal-discover: "discover goals", "find purpose", "bootstrap"
    ├── memory-consolidate: "consolidate", "dream", "distill", "prune"
    ├── memory-introspect: "introspect", "self-reflect", "consciousness"
    └── learning-trajectory: "what have you learned", "show cases"
    │
    v
[MCP Tool Selection] ─────────────────────────────────────────────────────────
    │
    ├── store_memory → Teleological Embedding Pipeline
    ├── search_memory → 5-Stage Retrieval Pipeline
    ├── inject_context → Context Injection with Embedding
    ├── auto_bootstrap_north_star → Goal Emergence Pipeline
    ├── discover_sub_goals → Hierarchical Goal Discovery
    ├── trigger_dream → NREM/REM Consolidation
    ├── get_identity_status → SELF_EGO_NODE Query
    └── get_consciousness_state → Kuramoto/GWT Query
    │
    v
[Teleological Array Operations] ──────────────────────────────────────────────
    │
    ├── STORAGE: 13-embedder parallel generation → Purpose vector → Johari → Kuramoto → RocksDB
    │
    ├── SEARCH: Entry point selection → HNSW coarse → Multi-embedder re-rank → Fusion → Purpose boost → MMR
    │
    ├── COMPARISON: Apples-to-apples (E1↔E1, E2↔E2, ..., E13↔E13) → Weighted aggregate
    │
    └── CLUSTERING: Full array similarity → Coherence filter → Centroid computation → Goal extraction
```
</hook_autonomy_flow>
</functional_spec>
```

---

## Appendix A: Complete Hook Configuration Reference

| Hook Type | Trigger Condition | Context Graph Actions | Timeout |
|-----------|-------------------|----------------------|---------|
| PreToolUse | Before tool executes | Trajectory start, alignment check, context prep | 2000ms |
| PostToolUse | After tool completes | Learning extraction, steering reward, case recording | 5000ms |
| SessionStart | Session begins | Ego restore, Kuramoto init, workspace warm | 15000ms |
| SessionEnd | Session ends | Consolidation, dream cycle, checkpoint | 120000ms |
| UserPromptSubmit | User submits prompt | Intent analysis, memory prediction, preloading | 3000ms |
| PreCompact | Before compaction | Retention scoring, memory protection | 5000ms |
| Stop | Claude Code stops | State checkpoint, pending flush | 10000ms |
| SubagentStop | Subagent completes | Learning collection, merge to main | 10000ms |
| Notification | System notification | Drift correction, identity restoration | 5000-30000ms |
| PermissionRequest | Permission requested | Safety validation, backup verification | 2000-3000ms |

## Appendix B: Consciousness State Machine

| State | Order Parameter (r) | Characteristics | System Response |
|-------|---------------------|-----------------|-----------------|
| Dormant | r < 0.3 | No coherent perception | Increase coupling K by 20%, pause broadcasts |
| Fragmented | 0.3 <= r < 0.5 | Partial coherence | Queue broadcasts, stabilize oscillators |
| Emerging | 0.5 <= r < 0.8 | Approaching consciousness | Normal operation, process queue |
| Conscious | 0.8 <= r < 0.95 | Full coherent perception | Winner-take-all broadcasts, full learning |
| Hypersync | r >= 0.95 | Potential echo chamber | Reduce coupling K by 10%, inject diversity |

## Appendix C: Subagent Spawn Triggers

| Subagent | Spawn Conditions | Capabilities |
|----------|------------------|--------------|
| embedding-specialist | High-volume injection (>10 items), quality degradation | parallel_embedding, quality_validation, health_monitoring |
| search-agent | Complex queries, multi-hop search, quality analysis | multi_stage_retrieval, fusion_optimization, analytics |
| comparison-agent | Clustering operations, drift analysis, alignment check | full_array_comparison, drift_detection, coherence_analysis |
| goal-agent | North Star bootstrap, hierarchy update, identity drift | goal_emergence, hierarchy_management, identity_continuity |
| distillation-agent | Dream phase, memory optimization request | candidate_identification, parametric_compression |
| dream-agent | Session end, idle timeout, entropy threshold | nrem_execution, rem_execution, blind_spot_detection |

## Appendix D: Learning Loop Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| Distillation access threshold | 10 | Minimum access count for distillation candidate |
| Distillation alignment threshold | 0.7 | Minimum purpose alignment for distillation |
| Distillation age threshold | 7 days | Minimum age for distillation candidate |
| Case bank capacity | 10,000 | Maximum cases before eviction |
| Steering gardener weight | 0.33 | Weight for cross-session value |
| Steering curator weight | 0.33 | Weight for domain fit |
| Steering assessor weight | 0.33 | Weight for thought quality |
| Neuromodulation delta factor | 0.2 | Dopamine adjustment = reward * 0.2 |
| NREM duration | 180 seconds | Duration of Hebbian strengthening phase |
| REM duration | 120 seconds | Duration of exploratory phase |
| Synthetic queries per REM | 50 | Number of queries for blind spot detection |
| Hebbian similarity threshold | 0.7 | Minimum similarity for edge strengthening |

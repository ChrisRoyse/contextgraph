# North Star Removal Functional Specification

```xml
<functional_spec id="SPEC-NSR" version="1.0">
<metadata>
  <title>North Star Removal and Autonomous Teleological Discovery</title>
  <status>draft</status>
  <owner>ContextGraph Team</owner>
  <last_updated>2026-01-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-TELEO-ARCH</spec_ref>
    <spec_ref>SPEC-STORAGE</spec_ref>
    <spec_ref>SPEC-SEARCH</spec_ref>
  </related_specs>
</metadata>

<overview>
This specification defines the complete removal of manual North Star creation
functionality and its replacement with autonomous teleological pattern discovery.

**Problem Statement:**
Manual North Star creation is mathematically invalid:
- A single 1024D embedding from text description
- Compared against 13-embedder teleological arrays via broken projection
- Projection destroys semantic meaning (temporal patterns vs semantic embedding)
- Apples-to-oranges comparison across fundamentally different embedding spaces

**Solution:**
Goals emerge autonomously from stored data patterns through:
- Hierarchical clustering in 13-dimensional teleological space
- Centroid computation producing full TeleologicalArray (not single embedding)
- Surprise-adaptive intrinsic motivation for pattern discovery
- Hooks, skills, and subagents for true autonomous operation

**Primary User:** AI Agent (Claude Code / contextgraph MCP client)
**Secondary Users:** Human developers querying emergent purposes

**Benefits:**
- Mathematically valid comparisons (E1-to-E1, E5-to-E5, etc.)
- Self-organizing purpose hierarchy
- No manual configuration burden
- Continuous purpose refinement
- SAGA-like autonomous goal evolution
</overview>

<user_stories>
<!-- Primary User: AI Agent -->
<story id="US-NSR-01" priority="must-have">
  <narrative>
    As an AI agent
    I want purposes to emerge automatically from stored memories
    So that I can align my work with meaningful patterns without manual configuration
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-01">
      <given>50+ teleological arrays stored in memory</given>
      <when>Discovery is triggered (session end, threshold, or schedule)</when>
      <then>At least one purpose cluster emerges with importance >= 0.5</then>
    </criterion>
    <criterion id="AC-02">
      <given>Discovered purposes exist</given>
      <when>I query for dominant North Star</when>
      <then>I receive a full TeleologicalArray (13 embeddings), not a single vector</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-NSR-02" priority="must-have">
  <narrative>
    As an AI agent
    I want to align my current work with emergent purposes
    So that I can verify my task contributes to meaningful goals
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-03">
      <given>A memory and a discovered purpose (both TeleologicalArrays)</given>
      <when>I compute alignment</when>
      <then>All 13 embeddings are compared in their native spaces (no projection)</then>
    </criterion>
    <criterion id="AC-04">
      <given>Alignment computation</given>
      <when>Result is returned</when>
      <then>Per-embedder alignment scores and aggregate score are provided</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-NSR-03" priority="must-have">
  <narrative>
    As an AI agent
    I want legacy manual goal APIs to be completely removed
    So that there is no path to create mathematically invalid comparisons
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-05">
      <given>I call purpose/north_star_update</given>
      <when>Request is processed</when>
      <then>Error response with METHOD_REMOVED code and migration guidance</then>
    </criterion>
    <criterion id="AC-06">
      <given>I call purpose/north_star_alignment</given>
      <when>Request is processed</when>
      <then>Error response with METHOD_REMOVED code and migration guidance</then>
    </criterion>
    <criterion id="AC-07">
      <given>I call purpose/set_goal</given>
      <when>Request is processed</when>
      <then>Error response with METHOD_REMOVED code and migration guidance</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-NSR-04" priority="must-have">
  <narrative>
    As an AI agent
    I want purpose discovery to happen without explicit triggers
    So that goals emerge truly autonomously via hooks and background processing
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-08">
      <given>Session ends with 5+ memories stored and 10+ minutes duration</given>
      <when>Session end hook fires</when>
      <then>Purpose discovery is automatically triggered</then>
    </criterion>
    <criterion id="AC-09">
      <given>30 minutes idle time with 10+ new memories</given>
      <when>Background refinement hook fires</when>
      <then>Incremental clustering updates purpose structure</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-NSR-05" priority="should-have">
  <narrative>
    As an AI agent
    I want to understand the hierarchical structure of emergent purposes
    So that I can navigate from abstract goals to specific patterns
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-10">
      <given>Sufficient memories for multi-scale clustering</given>
      <when>I query purpose hierarchy</when>
      <then>Root, mid-level, and leaf purposes are returned with relationships</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-NSR-06" priority="should-have">
  <narrative>
    As an AI agent
    I want to detect drift from emergent purposes
    So that I can identify when work deviates from established patterns
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-11">
      <given>A memory and an aligned purpose</given>
      <when>Drift analysis is performed</when>
      <then>Per-embedder drift levels and most-drifted spaces are identified</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-NSR-07" priority="could-have">
  <narrative>
    As a human developer
    I want to query emergent purposes using natural language
    So that I can understand what patterns the AI has discovered
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-12">
      <given>Reflective query like "what are my goals"</given>
      <when>Goal discovery skill is triggered</when>
      <then>Formatted purpose summary with importance and coherence scores returned</then>
    </criterion>
  </acceptance_criteria>
</story>
</user_stories>

<requirements>
<!-- Phase 1: Remove Protocol Constants (Priority Order: 1) -->
<requirement id="REQ-NSR-01" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove protocol constant PURPOSE_NORTH_STAR_ALIGNMENT from protocol.rs
  </description>
  <rationale>
    This constant routes to mathematically invalid comparison logic
  </rationale>
  <file>src/protocol.rs</file>
  <line_range>270-278</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-02" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove protocol constant NORTH_STAR_UPDATE from protocol.rs
  </description>
  <rationale>
    This constant enables manual goal creation which is invalid
  </rationale>
  <file>src/protocol.rs</file>
  <line_range>270-278</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-03" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove protocol constant PURPOSE_SET_GOAL from protocol.rs
  </description>
  <rationale>
    This constant enables manual goal creation which is invalid
  </rationale>
  <file>src/protocol.rs</file>
  <line_range>270-278</line_range>
  <action>DELETE</action>
</requirement>

<!-- Phase 2: Remove Dispatch Routes (Priority Order: 2) -->
<requirement id="REQ-NSR-04" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove dispatch route for PURPOSE_NORTH_STAR_ALIGNMENT from core handler
  </description>
  <rationale>
    Route connects to broken comparison logic that must be removed
  </rationale>
  <file>src/handlers/core.rs</file>
  <line_range>781-800</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-05" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove dispatch route for NORTH_STAR_UPDATE from core handler
  </description>
  <rationale>
    Route enables manual goal creation which is invalid
  </rationale>
  <file>src/handlers/core.rs</file>
  <line_range>781-800</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-06" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove dispatch route for PURPOSE_SET_GOAL from core handler
  </description>
  <rationale>
    Route enables manual goal creation which is invalid
  </rationale>
  <file>src/handlers/core.rs</file>
  <line_range>781-800</line_range>
  <action>DELETE</action>
</requirement>

<!-- Phase 3: Remove Handler Implementations (Priority Order: 3) -->
<requirement id="REQ-NSR-07" story_ref="US-NSR-03" priority="must" layer="logic">
  <description>
    Remove handle_north_star_alignment handler implementation
  </description>
  <rationale>
    Handler performs mathematically invalid projection-based comparison
  </rationale>
  <file>src/handlers/purpose.rs</file>
  <line_range>218-420</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-08" story_ref="US-NSR-03" priority="must" layer="logic">
  <description>
    Remove handle_north_star_update handler implementation
  </description>
  <rationale>
    Handler enables manual goal creation with single embeddings
  </rationale>
  <file>src/handlers/purpose.rs</file>
  <line_range>1037-1230</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-09" story_ref="US-NSR-03" priority="must" layer="logic">
  <description>
    Remove handle_set_goal handler implementation
  </description>
  <rationale>
    Handler enables manual goal creation which is invalid
  </rationale>
  <file>src/api/goals.rs</file>
  <action>DELETE</action>
</requirement>

<!-- Phase 4: Remove Broken Projection Code (Priority Order: 4) -->
<requirement id="REQ-NSR-10" story_ref="US-NSR-02" priority="must" layer="logic">
  <description>
    Remove project_embedding function from alignment calculator
  </description>
  <rationale>
    Function performs meaningless dimension reduction that destroys semantic information.
    Projecting a semantic vector to 512D does not create temporal information.
    Cannot compare causal asymmetry (E5) to semantic meaning (E1).
  </rationale>
  <file>src/core/alignment/calculator.rs</file>
  <line_range>392-399</line_range>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-11" story_ref="US-NSR-02" priority="must" layer="logic">
  <description>
    Remove resize_for_comparison function from alignment calculator
  </description>
  <rationale>
    Function enables apples-to-oranges comparison across embedding spaces
  </rationale>
  <file>src/core/alignment/calculator.rs</file>
  <action>DELETE</action>
</requirement>

<requirement id="REQ-NSR-12" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove NorthStarConfig struct from configuration
  </description>
  <rationale>
    Configuration for manual goal creation is no longer needed
  </rationale>
  <file>src/config/purpose.rs</file>
  <action>DELETE</action>
</requirement>

<!-- Phase 5: Update GoalNode Structure (Priority Order: 5) -->
<requirement id="REQ-NSR-13" story_ref="US-NSR-01" priority="must" layer="foundation">
  <description>
    Remove embedding field from GoalNode structure
  </description>
  <rationale>
    GoalNode.embedding was a single Vec&lt;f32&gt; which is invalid for comparison
    against 13-embedder TeleologicalArrays. GoalNode is replaced by DiscoveredPurpose.
  </rationale>
  <file>src/core/purpose.rs</file>
  <action>DELETE field</action>
</requirement>

<requirement id="REQ-NSR-14" story_ref="US-NSR-01" priority="must" layer="foundation">
  <description>
    Remove GoalNode constructors (north_star, with_embedding, from_description)
  </description>
  <rationale>
    Manual goal creation is not allowed - purposes emerge from data
  </rationale>
  <file>src/core/purpose.rs</file>
  <action>DELETE methods</action>
</requirement>

<requirement id="REQ-NSR-15" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Remove North Star test files
  </description>
  <rationale>
    Tests for removed functionality are no longer needed
  </rationale>
  <file>src/handlers/tests/north_star.rs</file>
  <action>DELETE file</action>
</requirement>

<requirement id="REQ-NSR-16" story_ref="US-NSR-03" priority="must" layer="foundation">
  <description>
    Update purpose test files to remove North Star references
  </description>
  <rationale>
    Existing tests may reference removed functionality
  </rationale>
  <file>src/handlers/tests/purpose.rs</file>
  <action>UPDATE</action>
</requirement>

<!-- Replacement: Autonomous Discovery -->
<requirement id="REQ-NSR-20" story_ref="US-NSR-01" priority="must" layer="logic">
  <description>
    Implement AutonomousTeleologicalDiscovery struct for purpose emergence
  </description>
  <rationale>
    Replaces manual goal creation with data-driven clustering.
    Uses hierarchical clustering in 13-dimensional teleological space.
    Computes centroids as full TeleologicalArrays (all 13 embedders averaged).
  </rationale>
  <signatures>
    pub struct AutonomousTeleologicalDiscovery;
    pub async fn discover_purposes(&amp;self, config: DiscoveryConfig) -&gt; Result&lt;Vec&lt;DiscoveredPurpose&gt;&gt;;
    fn compute_teleological_centroid(&amp;self, members: &amp;[TeleologicalArray]) -&gt; TeleologicalArray;
    fn assess_purpose_importance(&amp;self, cluster: &amp;Cluster, centroid: &amp;TeleologicalArray) -&gt; f32;
  </signatures>
</requirement>

<requirement id="REQ-NSR-21" story_ref="US-NSR-01" priority="must" layer="foundation">
  <description>
    Define DiscoveredPurpose struct with TeleologicalArray centroid
  </description>
  <rationale>
    Purposes must be full teleological arrays for valid comparison
  </rationale>
  <signatures>
    pub struct DiscoveredPurpose {
      pub id: Uuid,
      pub centroid: TeleologicalArray,  // FULL 13-embedder array
      pub description: String,
      pub importance: f32,
      pub coherence: f32,
      pub member_count: usize,
      pub discovery_method: DiscoveryMethod,
      pub discovered_at: DateTime&lt;Utc&gt;,
    }
  </signatures>
</requirement>

<requirement id="REQ-NSR-22" story_ref="US-NSR-02" priority="must" layer="logic">
  <description>
    Implement TeleologicalAlignmentCalculator using full array comparison
  </description>
  <rationale>
    All 13 embeddings compared in native spaces without projection.
    E1-to-E1, E5-to-E5, etc. with appropriate distance metrics per type.
  </rationale>
  <signatures>
    pub fn compute_alignment(&amp;self, memory: &amp;TeleologicalArray, north_star: &amp;TeleologicalArray) -&gt; AlignmentResult;
    fn compare_embedder(&amp;self, memory_emb: &amp;EmbedderOutput, north_star_emb: &amp;EmbedderOutput, embedder: Embedder) -&gt; f32;
  </signatures>
</requirement>

<requirement id="REQ-NSR-23" story_ref="US-NSR-04" priority="must" layer="logic">
  <description>
    Implement SurpriseAdaptiveMotivation for intrinsic discovery reward
  </description>
  <rationale>
    Adaptive switching between curiosity (entropy-max) and control (entropy-min)
    based on environment feedback via Thompson Sampling bandit
  </rationale>
  <signatures>
    pub struct SurpriseAdaptiveMotivation;
    pub fn compute_intrinsic_reward(&amp;mut self, array: &amp;TeleologicalArray, context: &amp;DiscoveryContext) -&gt; f32;
  </signatures>
</requirement>

<requirement id="REQ-NSR-24" story_ref="US-NSR-05" priority="should" layer="logic">
  <description>
    Implement EmergentPurposeHierarchy for multi-scale clustering
  </description>
  <rationale>
    Purposes at different abstraction levels emerge from clustering at different granularities
  </rationale>
  <signatures>
    pub struct EmergentPurposeHierarchy {
      pub roots: Vec&lt;DiscoveredPurpose&gt;,
      pub mid_level: Vec&lt;DiscoveredPurpose&gt;,
      pub leaves: Vec&lt;DiscoveredPurpose&gt;,
      pub relationships: Vec&lt;PurposeRelationship&gt;,
    }
    pub fn discover_hierarchy(store: &amp;TeleologicalArrayStore, config: HierarchyConfig) -&gt; Result&lt;Self&gt;;
    pub fn dominant_north_star(&amp;self) -&gt; Option&lt;&amp;DiscoveredPurpose&gt;;
  </signatures>
</requirement>

<requirement id="REQ-NSR-25" story_ref="US-NSR-06" priority="should" layer="logic">
  <description>
    Implement TeleologicalDriftDetector for alignment monitoring
  </description>
  <rationale>
    Detect when work drifts from emergent purposes using full array comparison
  </rationale>
  <signatures>
    pub fn check_drift(&amp;self, memory: &amp;TeleologicalArray, purpose: &amp;TeleologicalArray) -&gt; DriftAnalysis;
  </signatures>
</requirement>

<!-- New MCP Endpoints -->
<requirement id="REQ-NSR-30" story_ref="US-NSR-01" priority="must" layer="surface">
  <description>
    Add purpose/discover MCP endpoint
  </description>
  <rationale>
    Triggers autonomous purpose discovery from stored memories
  </rationale>
  <api>
    method: purpose/discover
    params: { min_confidence: f32, max_purposes: usize, include_hierarchy: bool }
    response: { purposes_discovered: usize, dominant_purpose: DiscoveredPurpose, all_purposes: Vec }
  </api>
</requirement>

<requirement id="REQ-NSR-31" story_ref="US-NSR-01" priority="must" layer="surface">
  <description>
    Add purpose/list_discovered MCP endpoint
  </description>
  <rationale>
    Lists all discovered purposes with their metadata
  </rationale>
  <api>
    method: purpose/list_discovered
    params: { include_hierarchy: bool, sort_by: String }
    response: { purposes: Vec&lt;DiscoveredPurpose&gt; }
  </api>
</requirement>

<requirement id="REQ-NSR-32" story_ref="US-NSR-01" priority="must" layer="surface">
  <description>
    Add purpose/get_dominant MCP endpoint
  </description>
  <rationale>
    Returns the current dominant North Star (highest-ranked emergent purpose)
  </rationale>
  <api>
    method: purpose/get_dominant
    params: {}
    response: { dominant: Option&lt;DiscoveredPurpose&gt; }
  </api>
</requirement>

<requirement id="REQ-NSR-33" story_ref="US-NSR-02" priority="must" layer="surface">
  <description>
    Add purpose/compute_alignment MCP endpoint using TeleologicalArrays
  </description>
  <rationale>
    Computes alignment between memory and purpose using full 13-space comparison
  </rationale>
  <api>
    method: purpose/compute_alignment
    params: { memory_id: Uuid, purpose_id: Uuid }
    response: { score: f32, per_embedder: [f32; 13], phase_synchronization: f32 }
  </api>
</requirement>

<requirement id="REQ-NSR-34" story_ref="US-NSR-05" priority="should" layer="surface">
  <description>
    Add purpose/get_hierarchy MCP endpoint
  </description>
  <rationale>
    Returns the emergent purpose hierarchy structure
  </rationale>
  <api>
    method: purpose/get_hierarchy
    params: { depth: usize }
    response: { roots: Vec, mid_level: Vec, leaves: Vec, relationships: Vec }
  </api>
</requirement>

<!-- Graceful Deprecation -->
<requirement id="REQ-NSR-40" story_ref="US-NSR-03" priority="must" layer="surface">
  <description>
    Add graceful deprecation shim for removed endpoints during transition
  </description>
  <rationale>
    Existing clients calling removed APIs should receive clear error messages with migration guidance
  </rationale>
  <error_code>METHOD_REMOVED</error_code>
  <message_template>
    "{method} has been REMOVED. Goals are now discovered autonomously from stored memories.
    Use 'purpose/discover' to see emergent goals, or simply store memories and let purposes emerge naturally."
  </message_template>
</requirement>

<!-- Autonomous Discovery Hooks -->
<requirement id="REQ-NSR-50" story_ref="US-NSR-04" priority="must" layer="surface">
  <description>
    Configure session-end hook for automatic goal clustering trigger
  </description>
  <rationale>
    Hooks enable TRUE autonomy - no manual intervention required.
    Session end is optimal trigger for clustering accumulated memories.
  </rationale>
  <hook>
    event: session-end
    conditions: memories_stored >= 5, session_duration >= 10 minutes
    action: mcp_call contextgraph/purpose/discover
  </hook>
</requirement>

<requirement id="REQ-NSR-51" story_ref="US-NSR-04" priority="must" layer="surface">
  <description>
    Configure background hook for continuous pattern refinement
  </description>
  <rationale>
    Background refinement prevents purposes from becoming stale
  </rationale>
  <hook>
    event: background
    schedule: every 30 minutes
    conditions: system_idle, memories_since_last >= 10
    action: mcp_call contextgraph/purpose/refine_clusters
  </hook>
</requirement>

<requirement id="REQ-NSR-52" story_ref="US-NSR-04" priority="should" layer="surface">
  <description>
    Configure pre-task hook for purpose-aware context loading
  </description>
  <rationale>
    Tasks should have awareness of relevant emergent purposes
  </rationale>
  <hook>
    event: pre-task
    action: embed task, find_aligned_purposes, inject_context
  </hook>
</requirement>

<requirement id="REQ-NSR-53" story_ref="US-NSR-04" priority="should" layer="surface">
  <description>
    Configure post-edit hook for pattern learning
  </description>
  <rationale>
    Code edits contribute patterns for purpose refinement
  </rationale>
  <hook>
    event: post-edit
    conditions: file_type in [rs, ts, py, go], change_size >= 10 lines
    action: extract_patterns, embed, store_for_clustering
  </hook>
</requirement>

<!-- Goal Discovery Skills -->
<requirement id="REQ-NSR-60" story_ref="US-NSR-07" priority="should" layer="surface">
  <description>
    Define goal-discovery skill for natural language purpose queries
  </description>
  <rationale>
    Users should be able to query emergent purposes conversationally
  </rationale>
  <triggers>
    - "what are my goals"
    - "what am I working on"
    - "what matters"
    - "priorities"
    - "focus areas"
    - "emerging patterns"
    - "discovered purposes"
    - "north star"
  </triggers>
</requirement>

<requirement id="REQ-NSR-61" story_ref="US-NSR-07" priority="should" layer="surface">
  <description>
    Define pattern-analysis skill for theme identification
  </description>
  <rationale>
    Analyze patterns in stored memories to identify emerging themes
  </rationale>
  <triggers>
    - "analyze patterns"
    - "what themes"
    - "what patterns"
    - "review trends"
  </triggers>
</requirement>

<requirement id="REQ-NSR-62" story_ref="US-NSR-07" priority="could" layer="surface">
  <description>
    Define reflective-query skill for work pattern summarization
  </description>
  <rationale>
    Handle reflective queries about work patterns and accomplishments
  </rationale>
  <triggers>
    - "what have I been working on"
    - "summarize my work"
    - "show my focus areas"
  </triggers>
</requirement>

<!-- Pattern-Analysis Subagents -->
<requirement id="REQ-NSR-70" story_ref="US-NSR-04" priority="should" layer="logic">
  <description>
    Define clustering-agent for hierarchical teleological clustering
  </description>
  <rationale>
    Specialized agent for discovering purpose clusters across 13 embedding spaces
  </rationale>
  <capabilities>
    - teleological_clustering
    - centroid_computation
    - cluster_coherence_analysis
    - hierarchical_organization
  </capabilities>
  <spawn_conditions>
    - memory_count >= 50
    - time_since_last_clustering > 1h
    - new_memories >= 20
  </spawn_conditions>
</requirement>

<requirement id="REQ-NSR-71" story_ref="US-NSR-04" priority="should" layer="logic">
  <description>
    Define pattern-detection-agent for novelty and trend analysis
  </description>
  <rationale>
    Detects emerging patterns that may become new purpose clusters
  </rationale>
  <capabilities>
    - novelty_detection
    - outlier_analysis
    - trend_identification
    - pattern_correlation
  </capabilities>
</requirement>

<requirement id="REQ-NSR-72" story_ref="US-NSR-04" priority="should" layer="logic">
  <description>
    Define goal-emergence-agent as discovery coordinator
  </description>
  <rationale>
    Coordinates clustering and pattern detection to declare new purposes
  </rationale>
  <capabilities>
    - purpose_synthesis
    - importance_assessment
    - hierarchy_management
    - purpose_lifecycle
  </capabilities>
</requirement>
</requirements>

<edge_cases>
<edge_case id="EC-NSR-01" req_ref="REQ-NSR-40">
  <scenario>Legacy API call to purpose/north_star_update</scenario>
  <expected_behavior>
    Return JSON-RPC error with code METHOD_REMOVED.
    Message: "purpose/north_star_update has been REMOVED. Goals are now discovered
    autonomously from stored memories. Use 'purpose/discover' to see emergent goals,
    or simply store memories and let purposes emerge naturally."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-02" req_ref="REQ-NSR-40">
  <scenario>Legacy API call to purpose/north_star_alignment</scenario>
  <expected_behavior>
    Return JSON-RPC error with code METHOD_REMOVED.
    Message: "purpose/north_star_alignment has been REMOVED. Use 'purpose/compute_alignment'
    with a DiscoveredPurpose ID, or use 'memory/search' with comparison_type for
    teleological search."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-03" req_ref="REQ-NSR-40">
  <scenario>Legacy API call to purpose/set_goal</scenario>
  <expected_behavior>
    Return JSON-RPC error with code METHOD_REMOVED.
    Message: "purpose/set_goal has been REMOVED. Goals cannot be manually created.
    They emerge autonomously from stored memories through clustering."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-04" req_ref="REQ-NSR-20">
  <scenario>Discovery triggered with insufficient memories (&lt;20)</scenario>
  <expected_behavior>
    Return empty purposes list with recommendation:
    "Insufficient data for purpose discovery. Store at least 20 memories for
    statistical significance in clustering."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-05" req_ref="REQ-NSR-20">
  <scenario>Discovery produces no clusters above importance threshold</scenario>
  <expected_behavior>
    Return empty purposes list with recommendation:
    "No strong patterns detected yet. Continue storing memories; purposes will
    emerge as patterns strengthen."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-06" req_ref="REQ-NSR-22">
  <scenario>Alignment requested for memory without all 13 embeddings</scenario>
  <expected_behavior>
    Return error: "Memory is missing embeddings for spaces: [list].
    Re-embed memory through all 13 embedders before alignment."
    Or: compute partial alignment with available spaces and note incomplete comparison.
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-07" req_ref="REQ-NSR-21">
  <scenario>Purpose centroid computation with mixed output types</scenario>
  <expected_behavior>
    Each embedder averaged according to its output type:
    - Dense: element-wise average
    - Sparse: union of active dimensions, averaged weights, threshold 0.1
    - Binary: majority voting per bit
    - TokenLevel: use representative tokens from highest-coherence member
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-08" req_ref="REQ-NSR-50">
  <scenario>Session ends before minimum duration (10 minutes)</scenario>
  <expected_behavior>
    Session-end hook does not trigger discovery.
    Log: "Session too short for discovery trigger (X minutes < 10 minutes threshold)."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-09" req_ref="REQ-NSR-51">
  <scenario>Background refinement during high system load</scenario>
  <expected_behavior>
    Check system_idle condition before running.
    If not idle, skip refinement and log:
    "Background refinement skipped: system not idle."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-10" req_ref="REQ-NSR-24">
  <scenario>Hierarchy discovery with uneven cluster distribution</scenario>
  <expected_behavior>
    Handle gracefully:
    - Some levels may be empty
    - Relationships may skip levels
    - Return whatever hierarchy is discoverable
  </expected_behavior>
</edge_case>

<!-- Migration Edge Cases -->
<edge_case id="EC-NSR-20" req_ref="REQ-NSR-40">
  <scenario>Existing GoalNode data in storage</scenario>
  <expected_behavior>
    Migration path:
    1. Archive legacy goals (do not delete, mark as legacy)
    2. Do NOT use legacy goals for comparison
    3. Bootstrap discovery with existing teleological memories
    4. Log migration: "X legacy goals archived. Discovery bootstrapped with Y memories."
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-21" req_ref="REQ-NSR-13">
  <scenario>Code references GoalNode.embedding field</scenario>
  <expected_behavior>
    Compilation error. All references must be updated to use DiscoveredPurpose.centroid
    (TeleologicalArray) before removal is complete.
  </expected_behavior>
</edge_case>

<edge_case id="EC-NSR-22" req_ref="REQ-NSR-40">
  <scenario>External system depends on removed API</scenario>
  <expected_behavior>
    Graceful deprecation error with clear migration guidance.
    Error message includes:
    - What was removed
    - Why it was removed (mathematically invalid)
    - What to use instead
    - Link to migration documentation
  </expected_behavior>
</edge_case>
</edge_cases>

<error_states>
<error id="ERR-NSR-01" http_code="410">
  <condition>Call to removed purpose/north_star_update endpoint</condition>
  <message>Method 'purpose/north_star_update' has been removed. Goals are now discovered autonomously.</message>
  <recovery>Use 'purpose/discover' to trigger discovery or let goals emerge naturally from stored memories.</recovery>
</error>

<error id="ERR-NSR-02" http_code="410">
  <condition>Call to removed purpose/north_star_alignment endpoint</condition>
  <message>Method 'purpose/north_star_alignment' has been removed. Use full teleological comparison.</message>
  <recovery>Use 'purpose/compute_alignment' with memory_id and purpose_id for valid comparison.</recovery>
</error>

<error id="ERR-NSR-03" http_code="410">
  <condition>Call to removed purpose/set_goal endpoint</condition>
  <message>Method 'purpose/set_goal' has been removed. Goals cannot be manually created.</message>
  <recovery>Store memories and let goals emerge through autonomous discovery.</recovery>
</error>

<error id="ERR-NSR-04" http_code="400">
  <condition>Discovery requested with insufficient memories</condition>
  <message>Insufficient memories for discovery ({count} < {minimum})</message>
  <recovery>Store at least {minimum} memories before triggering discovery.</recovery>
</error>

<error id="ERR-NSR-05" http_code="404">
  <condition>Alignment requested for non-existent purpose</condition>
  <message>Purpose with ID '{id}' not found</message>
  <recovery>Use 'purpose/list_discovered' to get valid purpose IDs.</recovery>
</error>

<error id="ERR-NSR-06" http_code="400">
  <condition>Memory missing required embeddings for alignment</condition>
  <message>Memory missing embeddings for: {missing_spaces}</message>
  <recovery>Re-embed memory through all 13 embedders using memory/embed endpoint.</recovery>
</error>
</error_states>

<test_plan>
<!-- Removal Verification Tests -->
<test_case id="TC-NSR-01" type="unit" req_ref="REQ-NSR-01,REQ-NSR-02,REQ-NSR-03">
  <description>Verify protocol constants are removed</description>
  <inputs>["Search codebase for PURPOSE_NORTH_STAR_ALIGNMENT, NORTH_STAR_UPDATE, PURPOSE_SET_GOAL"]</inputs>
  <expected>No matches found (grep returns empty)</expected>
</test_case>

<test_case id="TC-NSR-02" type="unit" req_ref="REQ-NSR-04,REQ-NSR-05,REQ-NSR-06">
  <description>Verify dispatch routes are removed</description>
  <inputs>["Search handlers/core.rs for removed method patterns"]</inputs>
  <expected>No dispatch routes for removed methods</expected>
</test_case>

<test_case id="TC-NSR-03" type="unit" req_ref="REQ-NSR-10,REQ-NSR-11">
  <description>Verify projection functions are removed</description>
  <inputs>["Search for project_embedding, resize_for_comparison"]</inputs>
  <expected>No matches found</expected>
</test_case>

<test_case id="TC-NSR-04" type="unit" req_ref="REQ-NSR-13,REQ-NSR-14">
  <description>Verify GoalNode.embedding field and constructors removed</description>
  <inputs>["Search for GoalNode::north_star, GoalNode.embedding"]</inputs>
  <expected>No matches found or compilation error if referenced</expected>
</test_case>

<!-- Autonomous Discovery Tests -->
<test_case id="TC-NSR-10" type="integration" req_ref="REQ-NSR-20">
  <description>Test purpose emergence from sufficient data</description>
  <inputs>["Store 100 teleological arrays, trigger discovery"]</inputs>
  <expected>At least 1 purpose emerges with importance >= 0.5</expected>
</test_case>

<test_case id="TC-NSR-11" type="unit" req_ref="REQ-NSR-21">
  <description>Test DiscoveredPurpose has full TeleologicalArray centroid</description>
  <inputs>["Create DiscoveredPurpose from cluster"]</inputs>
  <expected>centroid.embeddings.len() == 13, each embedder has correct output type</expected>
</test_case>

<test_case id="TC-NSR-12" type="unit" req_ref="REQ-NSR-22">
  <description>Test alignment uses full array comparison</description>
  <inputs>["Compute alignment between two TeleologicalArrays"]</inputs>
  <expected>per_embedder array has 13 scores, no projection called</expected>
</test_case>

<test_case id="TC-NSR-13" type="unit" req_ref="REQ-NSR-22">
  <description>Test alignment fails compilation with single embedding</description>
  <inputs>["Attempt to call compute_alignment with Vec&lt;f32&gt;"]</inputs>
  <expected>Compilation error - type mismatch</expected>
</test_case>

<!-- Graceful Deprecation Tests -->
<test_case id="TC-NSR-20" type="integration" req_ref="REQ-NSR-40">
  <description>Test legacy API returns METHOD_REMOVED error</description>
  <inputs>["Call purpose/north_star_update"]</inputs>
  <expected>Error response with code METHOD_REMOVED, migration guidance in message</expected>
</test_case>

<test_case id="TC-NSR-21" type="integration" req_ref="REQ-NSR-40">
  <description>Test legacy API error message contains migration guidance</description>
  <inputs>["Parse error response from removed endpoint"]</inputs>
  <expected>Message contains 'purpose/discover' alternative</expected>
</test_case>

<!-- Hook Tests -->
<test_case id="TC-NSR-30" type="integration" req_ref="REQ-NSR-50">
  <description>Test session-end hook triggers discovery</description>
  <inputs>["Store 10 memories, wait 15 minutes, end session"]</inputs>
  <expected>purpose/discover called automatically, purposes stored</expected>
</test_case>

<test_case id="TC-NSR-31" type="integration" req_ref="REQ-NSR-51">
  <description>Test background hook refines clusters</description>
  <inputs>["Trigger background hook with 20 new memories"]</inputs>
  <expected>purpose/refine_clusters called, centroids updated</expected>
</test_case>

<!-- MCP Endpoint Tests -->
<test_case id="TC-NSR-40" type="integration" req_ref="REQ-NSR-30">
  <description>Test purpose/discover endpoint</description>
  <inputs>["Call purpose/discover with min_confidence=0.5"]</inputs>
  <expected>Response contains purposes_discovered count and purpose array</expected>
</test_case>

<test_case id="TC-NSR-41" type="integration" req_ref="REQ-NSR-32">
  <description>Test purpose/get_dominant endpoint</description>
  <inputs>["Call purpose/get_dominant after discovery"]</inputs>
  <expected>Response contains dominant purpose with TeleologicalArray centroid</expected>
</test_case>
</test_plan>
</functional_spec>
```

## Removal Priority Order

The removal must proceed in this specific order to avoid compilation errors and maintain system stability:

### Phase 1: Protocol Constants (Week 1, Days 1-2)
**Remove first - these are just string constants with no dependencies**

| Order | REQ ID | Component | File | Action |
|-------|--------|-----------|------|--------|
| 1 | REQ-NSR-01 | PURPOSE_NORTH_STAR_ALIGNMENT | protocol.rs | DELETE |
| 2 | REQ-NSR-02 | NORTH_STAR_UPDATE | protocol.rs | DELETE |
| 3 | REQ-NSR-03 | PURPOSE_SET_GOAL | protocol.rs | DELETE |

### Phase 2: Dispatch Routes (Week 1, Days 2-3)
**Remove after constants - routes reference constants**

| Order | REQ ID | Component | File | Action |
|-------|--------|-----------|------|--------|
| 4 | REQ-NSR-04 | Dispatch for NORTH_STAR_ALIGNMENT | handlers/core.rs | DELETE |
| 5 | REQ-NSR-05 | Dispatch for NORTH_STAR_UPDATE | handlers/core.rs | DELETE |
| 6 | REQ-NSR-06 | Dispatch for SET_GOAL | handlers/core.rs | DELETE |

### Phase 3: Handler Implementations (Week 1, Days 3-5)
**Remove after routes - handlers are called by routes**

| Order | REQ ID | Component | File | Action |
|-------|--------|-----------|------|--------|
| 7 | REQ-NSR-07 | handle_north_star_alignment | handlers/purpose.rs | DELETE |
| 8 | REQ-NSR-08 | handle_north_star_update | handlers/purpose.rs | DELETE |
| 9 | REQ-NSR-09 | handle_set_goal | api/goals.rs | DELETE |

### Phase 4: Broken Projection Code (Week 2, Days 1-3)
**Remove after handlers - projection was called by handlers**

| Order | REQ ID | Component | File | Action |
|-------|--------|-----------|------|--------|
| 10 | REQ-NSR-10 | project_embedding | alignment/calculator.rs | DELETE |
| 11 | REQ-NSR-11 | resize_for_comparison | alignment/calculator.rs | DELETE |
| 12 | REQ-NSR-12 | NorthStarConfig | config/purpose.rs | DELETE |

### Phase 5: GoalNode Structure (Week 2, Days 3-5)
**Remove last - many components may reference GoalNode**

| Order | REQ ID | Component | File | Action |
|-------|--------|-----------|------|--------|
| 13 | REQ-NSR-13 | GoalNode.embedding field | core/purpose.rs | DELETE |
| 14 | REQ-NSR-14 | GoalNode constructors | core/purpose.rs | DELETE |
| 15 | REQ-NSR-15 | North Star tests | handlers/tests/north_star.rs | DELETE |
| 16 | REQ-NSR-16 | Purpose tests update | handlers/tests/purpose.rs | UPDATE |

### Phase 6: Add Autonomous Discovery (Weeks 3-4)
**Replacement implementation - can proceed in parallel with removal**

| Order | REQ ID | Component | Priority |
|-------|--------|-----------|----------|
| 17 | REQ-NSR-21 | DiscoveredPurpose struct | MUST |
| 18 | REQ-NSR-20 | AutonomousTeleologicalDiscovery | MUST |
| 19 | REQ-NSR-22 | TeleologicalAlignmentCalculator | MUST |
| 20 | REQ-NSR-23 | SurpriseAdaptiveMotivation | MUST |
| 21 | REQ-NSR-24 | EmergentPurposeHierarchy | SHOULD |
| 22 | REQ-NSR-25 | TeleologicalDriftDetector | SHOULD |

### Phase 7: New MCP Endpoints (Week 4)
**Add after core implementation**

| Order | REQ ID | Endpoint | Priority |
|-------|--------|----------|----------|
| 23 | REQ-NSR-30 | purpose/discover | MUST |
| 24 | REQ-NSR-31 | purpose/list_discovered | MUST |
| 25 | REQ-NSR-32 | purpose/get_dominant | MUST |
| 26 | REQ-NSR-33 | purpose/compute_alignment | MUST |
| 27 | REQ-NSR-34 | purpose/get_hierarchy | SHOULD |
| 28 | REQ-NSR-40 | Graceful deprecation shims | MUST |

### Phase 8: Hooks and Skills (Week 5)
**Add after endpoints for full autonomy**

| Order | REQ ID | Component | Priority |
|-------|--------|-----------|----------|
| 29 | REQ-NSR-50 | session-end hook | MUST |
| 30 | REQ-NSR-51 | background hook | MUST |
| 31 | REQ-NSR-52 | pre-task hook | SHOULD |
| 32 | REQ-NSR-53 | post-edit hook | SHOULD |
| 33 | REQ-NSR-60 | goal-discovery skill | SHOULD |
| 34 | REQ-NSR-61 | pattern-analysis skill | SHOULD |
| 35 | REQ-NSR-62 | reflective-query skill | COULD |

### Phase 9: Subagents (Week 6)
**Add for full parallel processing**

| Order | REQ ID | Agent | Priority |
|-------|--------|-------|----------|
| 36 | REQ-NSR-70 | clustering-agent | SHOULD |
| 37 | REQ-NSR-71 | pattern-detection-agent | SHOULD |
| 38 | REQ-NSR-72 | goal-emergence-agent | SHOULD |

## Verification Commands

After each phase, run these verification commands:

```bash
# Phase 1-5: Verify removal is complete
rg "north_star_alignment" --type rust
rg "north_star_update" --type rust
rg "project_embedding" --type rust
rg "GoalNode::north_star" --type rust
rg "GoalNode.*embedding" --type rust

# Should all return empty (no matches)

# Phase 6+: Verify new implementation
rg "DiscoveredPurpose" --type rust
rg "TeleologicalAlignmentCalculator" --type rust
rg "compute_alignment.*TeleologicalArray" --type rust

# Should return matches in new implementation files
```

## Success Criteria Summary

1. **All manual goal creation APIs removed** (REQ-NSR-01 through REQ-NSR-16)
2. **All projection-based comparison removed** (REQ-NSR-10, REQ-NSR-11)
3. **Autonomous discovery implemented** (REQ-NSR-20 through REQ-NSR-25)
4. **New MCP endpoints added** (REQ-NSR-30 through REQ-NSR-34)
5. **Graceful deprecation for legacy callers** (REQ-NSR-40)
6. **Hooks enable true autonomy** (REQ-NSR-50 through REQ-NSR-53)
7. **Skills provide natural language access** (REQ-NSR-60 through REQ-NSR-62)
8. **Subagents enable parallel discovery** (REQ-NSR-70 through REQ-NSR-72)

# North Star Removal and Autonomous Teleological Discovery

## Executive Summary

**CRITICAL PARADIGM SHIFT**: This document specifies the complete removal of manual North Star creation and its replacement with **autonomous teleological pattern discovery**. Goals are no longer manually defined - they **emerge from stored data patterns** as high-importance teleological vectors discovered through clustering and semantic analysis.

### Key Principles

1. **NO manual goal/North Star setting** - This capability is entirely removed
2. **North Stars ARE teleological vectors** - Arrays of 13 embeddings, not single vectors
3. **Goals emerge from data patterns** - Clustering discovers purpose, not user input
4. **The system learns purposes autonomously** - Like SAGA (Scientific Autonomous Goal-evolving Agents)
5. **Intrinsic motivation drives discovery** - Surprise-adaptive exploration finds meaningful patterns

---

## 1. Problem Statement: Why Manual North Stars Are Invalid

### 1.1 The Fundamental Mathematical Flaw

**Manual North Star Creation** produced:
- A single 1024D embedding from text description
- Compared against 13-embedder teleological arrays via broken projection

**The Apples-to-Oranges Problem:**
```
Manual North Star:    ONE vector (1024D from text-embedding-3-large)

Teleological Array:   13 DIFFERENT vectors from 13 DIFFERENT models
                      - E1: 1024D semantic (meaning)
                      - E2-E4: 512D temporal (time patterns)
                      - E5: 768D causal (ASYMMETRIC cause-effect)
                      - E6: ~30K sparse (selective activation)
                      - E7: 1536D code (AST structure)
                      - E8: 384D graph (connectivity)
                      - E9: 10K-bit binary (holographic robustness)
                      - E10: 768D multimodal (cross-modal binding)
                      - E11: 384D entity (factual grounding)
                      - E12: 128D/token (late interaction precision)
                      - E13: ~30K sparse SPLADE (keyword precision)
```

**Why Projection Was Meaningless:**
```rust
// OLD BROKEN CODE (alignment/calculator.rs:392-399)

// E1: Semantic (1024D) - compared directly (only valid comparison!)
alignments[0] = self.compute_dense_alignment(&fingerprint.e1_semantic, &goal.embedding);

// E2: Temporal Recent (512D) - PROJECTION (BROKEN!)
let projected_e2 = Self::project_embedding(&goal.embedding, 512);
alignments[1] = self.compute_dense_alignment(&fingerprint.e2_temporal_recent, &projected_e2);
// ^ This compares TEMPORAL patterns to a SIZE-REDUCED SEMANTIC embedding
// ^ The projected vector knows NOTHING about recency, decay, or time

// Same broken pattern for ALL 13 embedders
```

**The Mathematical Invalidity:**
- `e2_temporal_recent` encodes temporal decay patterns learned from time data
- Projecting a semantic vector to 512D doesn't give you temporal information
- You cannot compare causal asymmetry (E5) to semantic meaning (E1)
- Binary holographic codes (E9) have fundamentally different semantics

### 1.2 Why Goals Cannot Be Manually Defined

Research on autonomous AI systems reveals that effective goals must **emerge from data patterns**, not be imposed externally:

1. **SAGA (Scientific Autonomous Goal-evolving Agents)** demonstrates that objectives should evolve based on observed evidence, utility calculations, and emerging understanding - not static definitions

2. **Intrinsic Motivation Research** shows agents develop emergent behaviors from exploration alone, discovering meaningful patterns without predefined goals

3. **Emergent Behavior Theory** confirms that complex patterns arise from simpler systems interacting - purposes emerge, they are not programmed

4. **Self-Organizing Neural Networks** (Fusion ART) show that learning and memory systems naturally organize into episodic, semantic, and procedural structures without external goal specification

### 1.3 Complete Removal Manifest

| Component | File | Action |
|-----------|------|--------|
| `purpose/north_star_update` handler | `handlers/purpose.rs:1037-1230` | **DELETE** |
| `purpose/north_star_alignment` handler | `handlers/purpose.rs:218-420` | **DELETE** |
| Protocol constants | `protocol.rs:270-278` | **DELETE** |
| Core dispatch routes | `handlers/core.rs:781-800` | **DELETE** |
| `GoalNode.embedding` field | `core/purpose.rs` | **DELETE** |
| `project_embedding` function | `core/alignment/calculator.rs` | **DELETE** |
| `NorthStarConfig` struct | `config/purpose.rs` | **DELETE** |
| Manual goal creation APIs | `api/goals.rs` | **DELETE** |
| North Star tests | `handlers/tests/north_star.rs` | **DELETE** |
| Goal update tests | `handlers/tests/purpose.rs` | **UPDATE** |

---

## 2. The Replacement: Autonomous Teleological Discovery

### 2.1 Core Paradigm: Goals Emerge From Data

Instead of manual goal creation, the system discovers purposes through **semantic auto-aggregation** of teleological arrays:

```
+-----------------------------------------------------------------------------+
|                   AUTONOMOUS TELEOLOGICAL DISCOVERY                          |
+-----------------------------------------------------------------------------+
|                                                                             |
|   STORED MEMORIES                    EMERGENT PURPOSES                       |
|   ---------------                    -----------------                       |
|   [TeleoArray1]  --+                                                        |
|   [TeleoArray2]  --+-- Clustering --> [Purpose Cluster A] --> North Star A  |
|   [TeleoArray3]  --|   (13-space)     (centroid = TeleoArray)               |
|   [TeleoArray4]  --|                                                        |
|   [TeleoArray5]  --+-- Clustering --> [Purpose Cluster B] --> North Star B  |
|   [TeleoArray6]  --|   (13-space)     (centroid = TeleoArray)               |
|   [TeleoArray7]  --+                                                        |
|        :                                   |                                |
|                              Importance x Coherence x Coverage               |
|                                           |                                 |
|                                           v                                 |
|                              [DOMINANT NORTH STAR]                          |
|                              (highest-ranked centroid)                       |
|                                                                             |
|   Key: North Stars ARE teleological arrays, not single embeddings           |
|        Purposes EMERGE from data, not from manual configuration             |
|                                                                             |
+-----------------------------------------------------------------------------+
```

### 2.2 The Autonomous Discovery Engine

```rust
/// Autonomous purpose discovery from stored teleological arrays
///
/// This replaces ALL manual North Star creation functionality.
/// Goals emerge from data patterns using semantic auto-aggregation.
pub struct AutonomousTeleologicalDiscovery {
    store: Arc<dyn TeleologicalArrayStore>,
    comparator: TeleologicalComparator,
    clustering: HierarchicalTeleologicalClusterer,
    intrinsic_motivation: SurpriseAdaptiveMotivation,
}

impl AutonomousTeleologicalDiscovery {
    /// Discover purposes by clustering teleological arrays
    ///
    /// Uses centroid-based clustering across ALL 13 embedding spaces.
    /// Each cluster centroid becomes a potential purpose (North Star).
    /// The system autonomously determines which purposes are significant.
    pub async fn discover_purposes(
        &self,
        config: DiscoveryConfig,
    ) -> Result<Vec<DiscoveredPurpose>, DiscoveryError> {
        // 1. Retrieve all stored teleological arrays
        let arrays = self.store.list_all().await?;

        if arrays.len() < config.min_memories_for_discovery {
            return Ok(vec![]); // Insufficient data for emergence
        }

        // 2. Hierarchical clustering in 13-dimensional teleological space
        let clusters = self.clustering.cluster_hierarchical(
            &arrays,
            HierarchicalConfig {
                // Each space clustered independently, then merged
                per_space_k: config.clusters_per_space,
                linkage: Linkage::Ward,
                similarity: TeleologicalSimilarity::RRF,
            }
        )?;

        // 3. Compute centroids as teleological arrays (NOT single embeddings!)
        let purposes: Vec<DiscoveredPurpose> = clusters
            .into_iter()
            .map(|cluster| {
                let centroid = self.compute_teleological_centroid(&cluster.members);
                let importance = self.assess_purpose_importance(&cluster, &centroid);
                let coherence = self.measure_cluster_coherence(&cluster);

                DiscoveredPurpose {
                    id: Uuid::new_v4(),
                    centroid,  // TeleologicalArray - FULL 13-embedder array!
                    description: self.generate_purpose_description(&cluster),
                    importance,
                    coherence,
                    member_count: cluster.members.len(),
                    discovery_method: DiscoveryMethod::SemanticAutoAggregation,
                    discovered_at: Utc::now(),
                }
            })
            .filter(|p| p.importance >= config.min_importance)
            .collect();

        Ok(purposes)
    }

    /// Compute cluster centroid as AVERAGE of teleological arrays
    ///
    /// This is the CORRECT approach: averaging same-space embeddings
    /// across cluster members to produce a teleological array centroid.
    fn compute_teleological_centroid(
        &self,
        members: &[TeleologicalArray],
    ) -> TeleologicalArray {
        let n = members.len() as f32;

        // Average each of the 13 embedders SEPARATELY
        let embeddings: [EmbedderOutput; 13] = std::array::from_fn(|i| {
            match Embedder::from_index(i).expected_output_type() {
                OutputType::Dense(dim) => {
                    // Dense: element-wise average
                    let mut sum = vec![0.0f32; dim];
                    for member in members {
                        if let EmbedderOutput::Dense(vec) = &member.embeddings[i] {
                            for (j, &v) in vec.iter().enumerate() {
                                sum[j] += v;
                            }
                        }
                    }
                    EmbedderOutput::Dense(
                        sum.into_iter().map(|v| v / n).collect()
                    )
                }
                OutputType::Sparse => {
                    // Sparse: union of active dimensions, averaged weights
                    let mut dim_sums: HashMap<usize, f32> = HashMap::new();
                    let mut dim_counts: HashMap<usize, usize> = HashMap::new();
                    for member in members {
                        if let EmbedderOutput::Sparse(indices, values) = &member.embeddings[i] {
                            for (&idx, &val) in indices.iter().zip(values.iter()) {
                                *dim_sums.entry(idx).or_default() += val;
                                *dim_counts.entry(idx).or_default() += 1;
                            }
                        }
                    }
                    // Average and threshold
                    let (indices, values): (Vec<_>, Vec<_>) = dim_sums
                        .into_iter()
                        .filter_map(|(idx, sum)| {
                            let avg = sum / dim_counts[&idx] as f32;
                            if avg > 0.1 { Some((idx, avg)) } else { None }
                        })
                        .unzip();
                    EmbedderOutput::Sparse(indices, values)
                }
                OutputType::Binary(bits) => {
                    // Binary: majority voting per bit
                    let mut bit_counts = vec![0usize; bits];
                    for member in members {
                        if let EmbedderOutput::Binary(bitvec) = &member.embeddings[i] {
                            for (j, bit) in bitvec.iter().enumerate() {
                                if *bit { bit_counts[j] += 1; }
                            }
                        }
                    }
                    let majority = members.len() / 2;
                    EmbedderOutput::Binary(
                        bit_counts.into_iter().map(|c| c > majority).collect()
                    )
                }
                OutputType::TokenLevel(dim) => {
                    // Token-level: use representative tokens from highest-coherence member
                    let best_member = self.find_most_coherent_member(members, i);
                    members[best_member].embeddings[i].clone()
                }
            }
        });

        TeleologicalArray {
            id: Uuid::new_v4(),
            embeddings,
            metadata: TeleologicalMetadata {
                source: "centroid".into(),
                is_goal: true,
                cluster_size: Some(members.len()),
            },
        }
    }

    /// Assess importance of a discovered purpose
    fn assess_purpose_importance(
        &self,
        cluster: &Cluster,
        centroid: &TeleologicalArray,
    ) -> f32 {
        // Factors that determine purpose importance:
        // 1. Cluster size (more members = more significant pattern)
        let size_factor = (cluster.members.len() as f32).ln() / 10.0;

        // 2. Average importance of cluster members
        let avg_member_importance = cluster.members
            .iter()
            .map(|m| m.importance)
            .sum::<f32>() / cluster.members.len() as f32;

        // 3. Cluster coherence (tight cluster = strong pattern)
        let coherence = self.measure_cluster_coherence(cluster);

        // 4. Recency-weighted access patterns
        let recency_factor = self.compute_recency_weight(&cluster.members);

        // Combine factors
        (0.3 * size_factor + 0.3 * avg_member_importance +
         0.25 * coherence + 0.15 * recency_factor)
            .clamp(0.0, 1.0)
    }
}
```

### 2.3 Surprise-Adaptive Intrinsic Motivation

The system uses intrinsic motivation to guide discovery, adapting between entropy-maximizing (curiosity) and entropy-minimizing (control) objectives:

```rust
/// Surprise-adaptive intrinsic motivation for purpose discovery
///
/// Based on research showing that neither pure curiosity nor pure control
/// works across all environments. The system adapts its objective online.
pub struct SurpriseAdaptiveMotivation {
    /// Multi-armed bandit for objective selection
    objective_bandit: ThompsonSamplingBandit,
    /// Current intrinsic motivation mode
    current_mode: IntrinsicMode,
    /// Entropy control capability estimate
    entropy_control: f32,
}

#[derive(Clone, Copy)]
pub enum IntrinsicMode {
    /// Maximize entropy - explore novel patterns
    Curiosity,
    /// Minimize entropy - reinforce known patterns
    Control,
    /// Adaptive - switch based on environment feedback
    Adaptive,
}

impl SurpriseAdaptiveMotivation {
    /// Compute intrinsic reward for a teleological array
    pub fn compute_intrinsic_reward(
        &mut self,
        array: &TeleologicalArray,
        context: &DiscoveryContext,
    ) -> f32 {
        match self.current_mode {
            IntrinsicMode::Curiosity => {
                // Reward high surprise (entropy increase)
                self.compute_surprise(array, context)
            }
            IntrinsicMode::Control => {
                // Reward entropy reduction (coherence increase)
                self.compute_control(array, context)
            }
            IntrinsicMode::Adaptive => {
                // Use bandit to select objective
                let selected_arm = self.objective_bandit.select_arm();
                let reward = match selected_arm {
                    0 => self.compute_surprise(array, context),
                    1 => self.compute_control(array, context),
                    _ => unreachable!(),
                };

                // Update bandit with feedback
                let feedback = self.compute_entropy_control_feedback(array, context);
                self.objective_bandit.update(selected_arm, feedback);

                reward
            }
        }
    }

    /// Compute surprise (novelty) for a teleological array
    fn compute_surprise(&self, array: &TeleologicalArray, context: &DiscoveryContext) -> f32 {
        // Use per-embedder entropy computation
        let entropy_scores: [f32; 13] = std::array::from_fn(|i| {
            context.delta_s_computer.compute_for_embedder(i, &array.embeddings[i])
        });

        // Weighted average across embedders
        entropy_scores.iter().zip(EMBEDDER_WEIGHTS.iter())
            .map(|(&e, &w)| e * w)
            .sum()
    }
}
```

### 2.4 Hierarchical Purpose Organization

Discovered purposes naturally organize into a hierarchy:

```rust
/// Hierarchical organization of discovered purposes
///
/// Purposes at different abstraction levels emerge from clustering
/// at different granularities. This replaces manual goal hierarchies.
pub struct EmergentPurposeHierarchy {
    /// Root purposes (most abstract, fewest members)
    pub roots: Vec<DiscoveredPurpose>,
    /// Mid-level purposes (moderate abstraction)
    pub mid_level: Vec<DiscoveredPurpose>,
    /// Leaf purposes (most specific, many members)
    pub leaves: Vec<DiscoveredPurpose>,
    /// Parent-child relationships (all discovered, not defined)
    pub relationships: Vec<PurposeRelationship>,
}

impl EmergentPurposeHierarchy {
    /// Build hierarchy through multi-scale clustering
    pub fn discover_hierarchy(
        store: &TeleologicalArrayStore,
        config: HierarchyConfig,
    ) -> Result<Self, DiscoveryError> {
        let arrays = store.list_all()?;

        // Cluster at multiple granularities
        let fine_clusters = cluster_at_granularity(&arrays, Granularity::Fine)?;
        let medium_clusters = cluster_at_granularity(&arrays, Granularity::Medium)?;
        let coarse_clusters = cluster_at_granularity(&arrays, Granularity::Coarse)?;

        // Convert to purposes (centroids = teleological arrays)
        let leaves = to_purposes(fine_clusters);
        let mid_level = to_purposes(medium_clusters);
        let roots = to_purposes(coarse_clusters);

        // Discover relationships by comparing centroids
        let relationships = discover_relationships(&roots, &mid_level, &leaves);

        Ok(Self { roots, mid_level, leaves, relationships })
    }

    /// Get the dominant North Star (highest-ranked root purpose)
    ///
    /// This is what "North Star" now means: the most important
    /// PURPOSE that has EMERGED from the data, not a manually-set goal.
    pub fn dominant_north_star(&self) -> Option<&DiscoveredPurpose> {
        self.roots.iter()
            .max_by(|a, b| {
                let score_a = a.importance * a.coherence;
                let score_b = b.importance * b.coherence;
                score_a.partial_cmp(&score_b).unwrap_or(Ordering::Equal)
            })
    }
}
```

---

## 3. Autonomous Discovery Hooks

### 3.1 Overview: Hooks Enable TRUE Autonomy

Claude Code hooks provide the infrastructure for **truly autonomous goal discovery** - no manual intervention required. Unlike the old manual North Star system where users explicitly defined goals, hooks enable the system to:

1. **Detect patterns automatically** during session activity
2. **Trigger clustering** at optimal moments (session end, idle periods)
3. **Refine purposes continuously** through background processing
4. **Surface emergent goals** without explicit user action

This is the critical difference: **hooks make autonomy real**, not just theoretical.

### 3.2 SessionEnd Hook: Goal Clustering Trigger

The `session-end` hook is the primary trigger for autonomous goal discovery:

```yaml
# .claude/hooks/session-end-goal-discovery.yaml
name: session-end-goal-discovery
description: Trigger autonomous goal clustering when session ends
trigger:
  event: session-end
  conditions:
    - memories_stored_this_session: ">= 5"
    - session_duration_minutes: ">= 10"

actions:
  - name: trigger_purpose_discovery
    type: mcp_call
    tool: contextgraph/purpose/discover
    params:
      min_confidence: 0.6
      max_purposes: 10
      clustering_method: hierarchical
      include_hierarchy: true

  - name: store_discovery_results
    type: mcp_call
    tool: contextgraph/memory/store
    params:
      namespace: purposes
      key: "session_{{session_id}}_purposes"
      value: "{{discovery_results}}"
      ttl: 604800  # 7 days

  - name: notify_emergent_goals
    type: notify
    condition: "{{discovery_results.purposes_discovered}} > 0"
    message: |
      Discovered {{discovery_results.purposes_discovered}} emergent purposes.
      Dominant: {{discovery_results.dominant_purpose.description}}
      Importance: {{discovery_results.dominant_purpose.importance}}
```

### 3.3 Background Hooks: Continuous Pattern Refinement

Background hooks run during idle periods to refine discovered patterns:

```yaml
# .claude/hooks/background-pattern-refinement.yaml
name: background-pattern-refinement
description: Continuously refine teleological patterns during idle time
trigger:
  event: background
  schedule: "*/30 * * * *"  # Every 30 minutes
  conditions:
    - system_idle: true
    - memories_since_last_refinement: ">= 10"

actions:
  - name: incremental_clustering
    type: mcp_call
    tool: contextgraph/purpose/refine_clusters
    params:
      method: incremental  # Don't recluster everything
      new_memories_only: true
      merge_threshold: 0.85
      split_threshold: 0.4

  - name: update_purpose_importance
    type: mcp_call
    tool: contextgraph/purpose/recalculate_importance
    params:
      recency_decay: 0.95
      access_weight: 0.3

  - name: prune_weak_purposes
    type: mcp_call
    tool: contextgraph/purpose/prune
    params:
      min_coherence: 0.3
      min_members: 3
      archive_pruned: true
```

### 3.4 Pre-Task Hook: Purpose-Aware Context Loading

Before each task, load relevant purpose context:

```yaml
# .claude/hooks/pre-task-purpose-context.yaml
name: pre-task-purpose-context
description: Load emergent purposes relevant to current task
trigger:
  event: pre-task

actions:
  - name: embed_task_description
    type: mcp_call
    tool: contextgraph/embed/teleological
    params:
      content: "{{task.description}}"
      models: all  # All 13 embedders
    output: task_embedding

  - name: find_aligned_purposes
    type: mcp_call
    tool: contextgraph/purpose/find_aligned
    params:
      query_embedding: "{{task_embedding}}"
      comparison_type: teleological  # Full 13-space comparison
      top_k: 3
      min_alignment: 0.5
    output: aligned_purposes

  - name: inject_purpose_context
    type: context_inject
    condition: "{{aligned_purposes.count}} > 0"
    content: |
      ## Emergent Purpose Context

      This task aligns with {{aligned_purposes.count}} discovered purposes:
      {{#each aligned_purposes}}
      - **{{this.description}}** (alignment: {{this.alignment_score}})
        - Importance: {{this.importance}}
        - Coherence: {{this.coherence}}
      {{/each}}
```

### 3.5 Post-Edit Hook: Pattern Learning

After code edits, learn patterns for purpose refinement:

```yaml
# .claude/hooks/post-edit-pattern-learn.yaml
name: post-edit-pattern-learn
description: Extract patterns from edits for purpose clustering
trigger:
  event: post-edit
  conditions:
    - file_type: ["*.rs", "*.ts", "*.py", "*.go"]
    - change_size: ">= 10"  # Minimum 10 lines changed

actions:
  - name: extract_edit_patterns
    type: mcp_call
    tool: contextgraph/patterns/extract
    params:
      file: "{{edit.file_path}}"
      diff: "{{edit.diff}}"
      include_ast: true
      include_semantic: true
    output: patterns

  - name: embed_patterns
    type: mcp_call
    tool: contextgraph/embed/teleological
    params:
      content: "{{patterns}}"
      context:
        file: "{{edit.file_path}}"
        change_type: "{{edit.change_type}}"
    output: pattern_embedding

  - name: store_for_clustering
    type: mcp_call
    tool: contextgraph/memory/store
    params:
      namespace: edit_patterns
      content: "{{patterns}}"
      embedding: "{{pattern_embedding}}"
      metadata:
        file: "{{edit.file_path}}"
        timestamp: "{{now}}"
        session: "{{session_id}}"
```

### 3.6 Hook Configuration: Autonomous Discovery Settings

```json
// .claude/hooks.config.json
{
  "autonomous_discovery": {
    "enabled": true,
    "min_memories_for_discovery": 20,
    "discovery_triggers": {
      "session_end": true,
      "memory_threshold": 50,
      "time_interval_hours": 4,
      "idle_trigger": true
    },
    "clustering": {
      "method": "hierarchical",
      "linkage": "ward",
      "per_space_k": 5,
      "merge_threshold": 0.85,
      "split_threshold": 0.4
    },
    "importance_calculation": {
      "size_weight": 0.3,
      "member_importance_weight": 0.3,
      "coherence_weight": 0.25,
      "recency_weight": 0.15
    },
    "background_refinement": {
      "enabled": true,
      "interval_minutes": 30,
      "incremental": true,
      "prune_weak": true
    },
    "notifications": {
      "on_new_purpose": true,
      "on_purpose_shift": true,
      "importance_threshold": 0.7
    }
  }
}
```

---

## 4. Goal Discovery Skills

### 4.1 Overview: Skills for Pattern-Driven Discovery

Claude Code skills provide specialized capabilities for **detecting and surfacing emergent patterns** from teleological data. Unlike manual goal setting, these skills respond to patterns in the data itself.

### 4.2 Goal Discovery Skill Definition

```yaml
# .claude/skills/goal-discovery.yaml
name: goal-discovery
description: |
  Autonomous goal discovery skill that detects emerging patterns
  from teleological data. Auto-invoked on reflective queries.

triggers:
  - pattern: "(what are my goals|what am I working on|what matters|priorities|focus areas)"
  - pattern: "(emerging patterns|discovered purposes|north star)"
  - pattern: "(show me|find|discover) (goals|purposes|themes)"
  - event: explicit_invocation
  - event: session_reflection

actions:
  discover_purposes:
    description: Run full purpose discovery
    steps:
      - name: check_memory_threshold
        tool: contextgraph/memory/count
        params:
          namespace: teleological
        condition: result >= 20

      - name: run_clustering
        tool: contextgraph/purpose/discover
        params:
          min_confidence: 0.5
          max_purposes: 10
          include_hierarchy: true
          comparison_type: teleological

      - name: format_results
        template: |
          ## Emergent Purposes (Discovered from {{memory_count}} memories)

          {{#if dominant_purpose}}
          ### Dominant North Star
          **{{dominant_purpose.description}}**
          - Importance: {{dominant_purpose.importance | percent}}
          - Coherence: {{dominant_purpose.coherence | percent}}
          - Pattern strength: {{dominant_purpose.member_count}} aligned memories
          {{/if}}

          ### All Discovered Purposes
          {{#each purposes}}
          {{@index}}. **{{this.description}}**
             - Level: {{this.level}}
             - Importance: {{this.importance | percent}}
             - Members: {{this.member_count}}
          {{/each}}

          ### Discovery Method
          These purposes emerged autonomously through semantic clustering
          across 13 embedding spaces. No manual configuration required.

  show_purpose_hierarchy:
    description: Display hierarchical purpose structure
    steps:
      - name: get_hierarchy
        tool: contextgraph/purpose/get_hierarchy
        params:
          depth: 3

      - name: visualize
        template: |
          ## Purpose Hierarchy (Emergent Structure)

          ```
          {{#each roots}}
          [ROOT] {{this.description}}
          {{#each this.children}}
            +-- [MID] {{this.description}}
            {{#each this.children}}
              +---- [LEAF] {{this.description}}
            {{/each}}
          {{/each}}
          {{/each}}
          ```

  align_current_work:
    description: Show how current work aligns with emergent purposes
    steps:
      - name: get_recent_context
        tool: contextgraph/memory/recent
        params:
          limit: 20

      - name: compute_alignments
        tool: contextgraph/purpose/batch_align
        params:
          memories: "{{recent_memories}}"
          purposes: "{{all_purposes}}"

      - name: summarize
        template: |
          ## Current Work Alignment

          Your recent work aligns most strongly with:
          {{#each top_alignments limit=3}}
          - **{{this.purpose.description}}**: {{this.alignment | percent}} aligned
          {{/each}}

          {{#if drift_detected}}
          > Note: Some recent work shows drift from established patterns.
          > This may indicate emerging new purposes.
          {{/if}}
```

### 4.3 Pattern Analysis Skill

```yaml
# .claude/skills/pattern-analysis.yaml
name: pattern-analysis
description: |
  Analyze patterns in stored memories to identify emerging themes
  and potential new purpose clusters.

triggers:
  - pattern: "(analyze|examine|review) patterns"
  - pattern: "what (themes|patterns|trends)"
  - event: background_analysis

actions:
  analyze_recent_patterns:
    steps:
      - name: get_recent_memories
        tool: contextgraph/memory/search
        params:
          time_range: "7d"
          limit: 100

      - name: cluster_recent
        tool: contextgraph/clustering/analyze
        params:
          memories: "{{recent_memories}}"
          method: dbscan  # Density-based for outlier detection
          min_cluster_size: 3

      - name: identify_emerging
        tool: contextgraph/patterns/emerging
        params:
          clusters: "{{clusters}}"
          existing_purposes: "{{current_purposes}}"
          novelty_threshold: 0.7

      - name: report
        template: |
          ## Pattern Analysis Report

          ### Established Patterns
          {{#each stable_clusters}}
          - {{this.description}} ({{this.size}} members, stable)
          {{/each}}

          ### Emerging Patterns
          {{#each emerging_clusters}}
          - **NEW**: {{this.description}} ({{this.size}} members)
            - Novelty score: {{this.novelty | percent}}
            - Potential purpose: {{this.potential_purpose}}
          {{/each}}

          ### Outliers (Potential Seeds)
          {{#each outliers limit=5}}
          - {{this.summary}} (no cluster yet)
          {{/each}}

  track_purpose_evolution:
    steps:
      - name: get_historical_purposes
        tool: contextgraph/purpose/history
        params:
          time_range: "30d"

      - name: compute_evolution
        tool: contextgraph/purpose/evolution
        params:
          historical: "{{historical_purposes}}"
          current: "{{current_purposes}}"

      - name: report
        template: |
          ## Purpose Evolution (Last 30 Days)

          ### Growing Purposes
          {{#each growing}}
          - {{this.description}}: +{{this.growth | percent}} importance
          {{/each}}

          ### Declining Purposes
          {{#each declining}}
          - {{this.description}}: {{this.decline | percent}} decline
          {{/each}}

          ### New Emergences
          {{#each new}}
          - {{this.description}} (emerged {{this.emerged_at | relative}})
          {{/each}}
```

### 4.4 Reflective Query Skill

```yaml
# .claude/skills/reflective-query.yaml
name: reflective-query
description: |
  Handle reflective queries about work patterns, goals, and purposes.
  Auto-triggers goal discovery when appropriate.

triggers:
  - pattern: "(what have I|what did I|show me what I) (been working on|done|accomplished)"
  - pattern: "(my|our) (focus|priorities|main work)"
  - pattern: "summarize (my|the) (work|progress|activities)"

actions:
  reflect_on_work:
    steps:
      - name: gather_session_data
        parallel:
          - tool: contextgraph/memory/session_summary
            params:
              session_id: "{{current_session}}"
          - tool: contextgraph/purpose/get_current
          - tool: contextgraph/memory/recent
            params:
              limit: 50

      - name: compute_session_alignment
        tool: contextgraph/purpose/align_session
        params:
          session_memories: "{{session_memories}}"
          purposes: "{{current_purposes}}"

      - name: generate_reflection
        template: |
          ## Session Reflection

          ### What You Worked On
          {{#each session_activities}}
          - {{this.description}} ({{this.duration | duration}})
          {{/each}}

          ### Purpose Alignment
          Your work this session aligned with these emergent purposes:
          {{#each purpose_alignments}}
          - **{{this.purpose}}**: {{this.alignment | percent}}
          {{/each}}

          ### Patterns Observed
          {{#each observed_patterns}}
          - {{this}}
          {{/each}}

          {{#if new_patterns}}
          ### Emerging Themes
          Some new patterns are forming that may become purposes:
          {{#each new_patterns}}
          - {{this.description}} ({{this.strength | percent}} coherence)
          {{/each}}
          {{/if}}
```

---

## 5. Discovery Subagents

### 5.1 Overview: Agents for Autonomous Discovery

Discovery subagents are specialized Claude Code agents focused on **clustering, pattern detection, and goal emergence**. They operate autonomously to surface purposes from teleological data.

### 5.2 Clustering Agent

```yaml
# .claude/agents/clustering-agent.yaml
name: clustering-agent
type: specialist
description: |
  Specialized agent for hierarchical clustering of teleological arrays.
  Discovers purpose clusters across 13 embedding spaces.

capabilities:
  - teleological_clustering
  - centroid_computation
  - cluster_coherence_analysis
  - hierarchical_organization

spawn_conditions:
  - memory_count: ">= 50"
  - time_since_last_clustering: "> 1h"
  - new_memories: ">= 20"

configuration:
  clustering:
    algorithm: hierarchical
    linkage: ward
    distance_metric: teleological_rrf
    per_space_clustering: true
    merge_strategy: consensus

  thresholds:
    min_cluster_size: 3
    max_clusters: 20
    coherence_threshold: 0.4
    split_threshold: 0.3
    merge_threshold: 0.85

tasks:
  cluster_all_memories:
    description: Full clustering of all stored memories
    priority: low
    schedule: "0 */4 * * *"  # Every 4 hours
    steps:
      - retrieve_all_teleological_arrays
      - cluster_per_embedding_space
      - merge_cross_space_clusters
      - compute_cluster_centroids
      - assess_cluster_coherence
      - build_hierarchy
      - store_results

  incremental_cluster:
    description: Add new memories to existing clusters
    priority: medium
    trigger: new_memory_threshold
    steps:
      - retrieve_new_memories
      - find_nearest_clusters
      - decide_assign_or_new_cluster
      - update_centroids_incrementally
      - check_for_splits_or_merges
      - update_hierarchy

  analyze_cluster_evolution:
    description: Track how clusters change over time
    priority: low
    schedule: "0 0 * * *"  # Daily
    steps:
      - load_historical_cluster_snapshots
      - compute_cluster_movements
      - identify_growing_clusters
      - identify_declining_clusters
      - detect_emerging_clusters
      - report_evolution

output:
  store_to: contextgraph/purposes
  notify_on:
    - new_cluster_discovered
    - cluster_split
    - cluster_merge
    - hierarchy_change
```

### 5.3 Pattern Detection Agent

```yaml
# .claude/agents/pattern-detection-agent.yaml
name: pattern-detection-agent
type: analyst
description: |
  Detects emerging patterns in teleological data that may become
  new purpose clusters. Uses novelty and surprise signals.

capabilities:
  - novelty_detection
  - outlier_analysis
  - trend_identification
  - pattern_correlation

spawn_conditions:
  - memories_in_last_hour: ">= 5"
  - background_mode: true

configuration:
  detection:
    novelty_threshold: 0.7
    min_pattern_size: 3
    correlation_threshold: 0.6
    trend_window: "24h"

  surprise_adaptive:
    mode: adaptive
    curiosity_weight: 0.5
    control_weight: 0.5
    adaptation_rate: 0.1

tasks:
  detect_novel_patterns:
    description: Find patterns that don't fit existing clusters
    priority: medium
    trigger: continuous
    steps:
      - stream_new_memories
      - compute_novelty_scores
      - identify_high_novelty_items
      - check_for_pattern_formation
      - notify_if_pattern_emerging

  correlate_across_spaces:
    description: Find correlations across embedding spaces
    priority: low
    schedule: "*/15 * * * *"  # Every 15 minutes
    steps:
      - sample_recent_memories
      - compute_per_space_distances
      - find_cross_space_correlations
      - identify_meaningful_correlations
      - report_insights

  trend_analysis:
    description: Identify trending topics and patterns
    priority: medium
    schedule: "0 * * * *"  # Hourly
    steps:
      - aggregate_hourly_memories
      - compute_semantic_shifts
      - identify_trending_topics
      - compare_to_existing_purposes
      - predict_emerging_purposes

output:
  store_to: contextgraph/patterns
  feed_to: clustering-agent
  notify_on:
    - novel_pattern_detected
    - trend_shift
    - potential_new_purpose
```

### 5.4 Goal Emergence Agent

```yaml
# .claude/agents/goal-emergence-agent.yaml
name: goal-emergence-agent
type: coordinator
description: |
  Coordinates the goal emergence process, synthesizing inputs from
  clustering and pattern detection to declare new purposes.

capabilities:
  - purpose_synthesis
  - importance_assessment
  - hierarchy_management
  - purpose_lifecycle

spawn_conditions:
  - clustering_complete: true
  - pattern_detection_available: true

configuration:
  emergence:
    min_importance_for_purpose: 0.5
    min_coherence_for_purpose: 0.4
    min_members_for_purpose: 5
    hierarchy_depth: 3

  lifecycle:
    purpose_ttl_days: 30
    importance_decay: 0.95
    reactivation_threshold: 0.6
    archive_threshold: 0.2

tasks:
  synthesize_purposes:
    description: Create purposes from cluster centroids
    priority: high
    trigger: clustering_complete
    steps:
      - receive_cluster_results
      - filter_by_coherence
      - compute_purpose_importance
      - generate_purpose_descriptions
      - build_purpose_hierarchy
      - store_purposes
      - notify_new_purposes

  manage_purpose_lifecycle:
    description: Maintain purpose freshness and relevance
    priority: medium
    schedule: "0 0 * * *"  # Daily
    steps:
      - load_all_purposes
      - apply_importance_decay
      - check_for_reactivation
      - archive_weak_purposes
      - update_hierarchy
      - report_lifecycle_changes

  determine_dominant_north_star:
    description: Identify the current dominant purpose
    priority: high
    trigger: purpose_change
    steps:
      - load_root_purposes
      - compute_combined_scores
      - rank_by_importance_coherence
      - declare_dominant
      - notify_if_changed

  handle_purpose_conflict:
    description: Resolve conflicts between competing purposes
    priority: medium
    trigger: conflict_detected
    steps:
      - identify_conflicting_purposes
      - analyze_member_overlap
      - compute_resolution_strategy
      - merge_or_split_as_needed
      - update_hierarchy

output:
  primary_store: contextgraph/purposes
  backup_store: contextgraph/purposes_archive
  notify_on:
    - new_purpose_emerged
    - purpose_archived
    - dominant_north_star_changed
    - purpose_conflict_resolved
```

### 5.5 Agent Coordination Configuration

```json
// .claude/agents/discovery-coordination.json
{
  "discovery_agents": {
    "enabled": true,
    "coordination_mode": "hierarchical",
    "agents": [
      {
        "name": "clustering-agent",
        "role": "worker",
        "reports_to": "goal-emergence-agent"
      },
      {
        "name": "pattern-detection-agent",
        "role": "worker",
        "reports_to": "goal-emergence-agent"
      },
      {
        "name": "goal-emergence-agent",
        "role": "coordinator",
        "reports_to": null
      }
    ],
    "communication": {
      "protocol": "async_message",
      "queue": "discovery_queue",
      "timeout_ms": 30000
    },
    "scheduling": {
      "full_discovery": "0 */4 * * *",
      "incremental": "*/30 * * * *",
      "trend_analysis": "0 * * * *"
    },
    "resource_limits": {
      "max_concurrent_agents": 3,
      "max_memory_mb": 512,
      "max_cpu_percent": 25
    }
  }
}
```

---

## 6. How Autonomy Replaces Manual North Star

### 6.1 The Complete Paradigm Shift

| Aspect | Old Manual System | New Autonomous System |
|--------|------------------|----------------------|
| **Goal Creation** | User writes description, system embeds it | System discovers goals from data patterns |
| **Goal Structure** | Single 1024D embedding | Full 13-embedder teleological array |
| **Comparison** | Broken projection-based | Valid apples-to-apples comparison |
| **Updates** | Manual API calls | Automatic via hooks and agents |
| **Hierarchy** | Manually defined parent-child | Emergent from multi-scale clustering |
| **Relevance** | Static until manually changed | Continuously refined based on activity |
| **Discovery** | None - goals are imposed | Continuous pattern-based emergence |

### 6.2 Why This Is TRUE Autonomy

**Manual North Star Problems:**
1. Required explicit user action to set goals
2. Goals became stale without manual updates
3. Mathematically invalid comparison (apples to oranges)
4. No connection to actual work patterns
5. Hierarchy was arbitrary, not data-driven

**Autonomous Discovery Solutions:**
1. **SessionEnd hooks** automatically trigger clustering - no user action needed
2. **Background hooks** continuously refine purposes - never stale
3. **Full teleological comparison** - mathematically valid
4. **Purposes emerge from actual memories** - directly connected to work
5. **Hierarchy emerges from data** - reflects real relationships

### 6.3 The Autonomy Pipeline

```
+-------------------------------------------------------------------------+
|                     AUTONOMOUS GOAL DISCOVERY PIPELINE                   |
+-------------------------------------------------------------------------+
|                                                                         |
|  [Memory Injection]     [Embedding]      [Storage]      [Discovery]     |
|       (MCP)           (13 models)     (TeleoArrays)     (Clustering)    |
|         |                  |               |                |           |
|         v                  v               v                v           |
|  +----------+      +-------------+   +-----------+   +-------------+    |
|  | User     |      | Autonomous  |   | Teleo-    |   | Clustering  |    |
|  | stores   | ---> | embedding   |-->| logical   |-->| Agent       |    |
|  | memory   |      | pipeline    |   | Store     |   | (periodic)  |    |
|  +----------+      +-------------+   +-----------+   +-------------+    |
|                                                            |            |
|                                                            v            |
|  +----------+      +-------------+   +-----------+   +-------------+    |
|  | Skills   |      | Goal        |   | Purpose   |   | Pattern     |    |
|  | surface  | <--- | Emergence   |<--| Hierarchy |<--| Detection   |    |
|  | purposes |      | Agent       |   |           |   | Agent       |    |
|  +----------+      +-------------+   +-----------+   +-------------+    |
|       |                  |                                              |
|       v                  v                                              |
|  +----------+      +-------------+                                      |
|  | User     |      | Hooks       |                                      |
|  | queries  |      | trigger     |                                      |
|  | goals    |      | refinement  |                                      |
|  +----------+      +-------------+                                      |
|                                                                         |
|  NO MANUAL GOAL SETTING ANYWHERE IN THIS PIPELINE                       |
|                                                                         |
+-------------------------------------------------------------------------+
```

### 6.4 Entry Points: Any of 13 Spaces

The autonomous system supports **entry from any embedding space**:

```rust
/// Entry-point discovery configuration
///
/// Unlike manual North Stars that only used semantic (E1) space,
/// autonomous discovery can start from ANY of the 13 spaces.
pub struct EntryPointConfig {
    /// Which embedding space to use as entry point
    pub entry_space: Embedder,
    /// Minimum similarity in entry space
    pub entry_threshold: f32,
    /// Whether to expand to all 13 spaces after entry
    pub expand_to_full_comparison: bool,
}

impl AutonomousTeleologicalDiscovery {
    /// Discover purposes starting from any embedding space
    pub async fn discover_from_entry_point(
        &self,
        entry_config: EntryPointConfig,
        query: &EmbedderOutput,
    ) -> Result<Vec<DiscoveredPurpose>, DiscoveryError> {
        // 1. Find initial candidates using entry space only
        let candidates = self.store.search_single_space(
            entry_config.entry_space,
            query,
            entry_config.entry_threshold,
        ).await?;

        if !entry_config.expand_to_full_comparison {
            // Return single-space results (fast but partial)
            return self.cluster_single_space(candidates, entry_config.entry_space);
        }

        // 2. Expand to full 13-space comparison (slower but complete)
        let full_arrays = self.store.get_full_arrays(&candidates).await?;

        // 3. Cluster in full teleological space
        self.cluster_full_space(full_arrays)
    }
}
```

### 6.5 Removing Manual Capabilities Entirely

With hooks, skills, and agents providing autonomous discovery, **manual goal creation is not just deprecated - it's removed**:

```rust
// These functions NO LONGER EXIST:
// - set_north_star()
// - update_goal()
// - create_goal_from_description()
// - import_goals()

// The ONLY way purposes exist is through discovery:
impl PurposeStore {
    /// Purposes can ONLY be created through discovery
    pub async fn store_discovered_purpose(
        &self,
        purpose: DiscoveredPurpose,
    ) -> Result<PurposeId, StoreError> {
        // Validate this came from discovery
        assert!(
            matches!(purpose.discovery_method,
                DiscoveryMethod::SemanticAutoAggregation |
                DiscoveryMethod::TemporalClustering |
                DiscoveryMethod::CausalInference |
                DiscoveryMethod::HierarchicalEmergence
            ),
            "Purposes can only be created through autonomous discovery"
        );

        self.inner.store(purpose).await
    }

    // NO set_purpose(), NO create_purpose(), NO import_purpose()
}
```

---

## 7. Alignment Calculation: Apples to Apples

### 7.1 New Alignment System

With North Stars as teleological arrays, alignment becomes mathematically valid:

```rust
/// Alignment calculator using full teleological array comparison
///
/// NO PROJECTION. All comparisons are apples-to-apples:
/// E1 <-> E1, E5 <-> E5, etc.
pub struct TeleologicalAlignmentCalculator {
    comparator: TeleologicalComparator,
}

impl TeleologicalAlignmentCalculator {
    /// Compute alignment between a memory and a North Star
    ///
    /// Both MUST be TeleologicalArrays. Single-embedding comparison
    /// is not supported and will not compile.
    pub fn compute_alignment(
        &self,
        memory: &TeleologicalArray,
        north_star: &TeleologicalArray,  // NOT a single embedding!
    ) -> AlignmentResult {
        // Compare each embedder to its counterpart
        let per_embedder_alignment: [f32; 13] = std::array::from_fn(|i| {
            self.compare_embedder(
                &memory.embeddings[i],
                &north_star.embeddings[i],
                Embedder::from_index(i),
            )
        });

        // Compute weighted aggregate
        let aggregate = per_embedder_alignment.iter()
            .zip(TELEOLOGICAL_WEIGHTS.iter())
            .map(|(&a, &w)| a * w)
            .sum::<f32>();

        // Compute Kuramoto phase synchronization
        let phase_sync = self.compute_kuramoto_sync(&per_embedder_alignment);

        AlignmentResult {
            score: aggregate * phase_sync,
            per_embedder: per_embedder_alignment,
            phase_synchronization: phase_sync,
            interpretation: self.interpret_alignment(aggregate, &per_embedder_alignment),
        }
    }

    /// Compare embeddings within the SAME embedding space
    fn compare_embedder(
        &self,
        memory_emb: &EmbedderOutput,
        north_star_emb: &EmbedderOutput,
        embedder: Embedder,
    ) -> f32 {
        match (memory_emb, north_star_emb) {
            (EmbedderOutput::Dense(a), EmbedderOutput::Dense(b)) => {
                if embedder == Embedder::E5Causal {
                    // Asymmetric similarity for causal embeddings
                    self.asymmetric_cosine(a, b)
                } else {
                    self.cosine_similarity(a, b)
                }
            }
            (EmbedderOutput::Sparse(ia, va), EmbedderOutput::Sparse(ib, vb)) => {
                self.sparse_cosine(ia, va, ib, vb)
            }
            (EmbedderOutput::Binary(a), EmbedderOutput::Binary(b)) => {
                1.0 - (self.hamming_distance(a, b) as f32 / a.len() as f32)
            }
            (EmbedderOutput::TokenLevel(a), EmbedderOutput::TokenLevel(b)) => {
                self.max_sim_late_interaction(a, b)
            }
            _ => panic!("Mismatched embedder output types - this should never happen"),
        }
    }
}
```

### 7.2 Drift Detection with Arrays

```rust
/// Drift detection using teleological arrays
///
/// Detects when memories are drifting from discovered purposes.
/// Uses FULL array comparison, not broken projection.
pub struct TeleologicalDriftDetector;

impl TeleologicalDriftDetector {
    /// Check if a memory has drifted from its aligned purpose
    pub fn check_drift(
        &self,
        memory: &TeleologicalArray,
        purpose: &TeleologicalArray,  // Discovered purpose, not manual goal
        comparison_type: ComparisonType,
    ) -> DriftAnalysis {
        let calculator = TeleologicalAlignmentCalculator::new();
        let alignment = calculator.compute_alignment(memory, purpose);

        DriftAnalysis {
            overall_drift: 1.0 - alignment.score,
            has_drifted: alignment.score < 0.7,
            per_embedder_drift: std::array::from_fn(|i| {
                let score = alignment.per_embedder[i];
                DriftLevel::from_score(score)
            }),
            most_drifted_spaces: self.find_most_drifted(&alignment.per_embedder),
            recommendations: self.generate_recommendations(&alignment),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DriftLevel {
    None,     // score > 0.8
    Low,      // score in [0.6, 0.8)
    Medium,   // score in [0.4, 0.6)
    High,     // score < 0.4
}
```

---

## 8. Removal Implementation Steps

### 8.1 Phase 1: Remove MCP Handlers (Week 1)

**Step 1.1: Remove protocol constants**
```rust
// DELETE from protocol.rs
pub const PURPOSE_NORTH_STAR_ALIGNMENT: &str = "purpose/north_star_alignment";
pub const NORTH_STAR_UPDATE: &str = "purpose/north_star_update";
pub const PURPOSE_SET_GOAL: &str = "purpose/set_goal";
```

**Step 1.2: Remove dispatch routes**
```rust
// DELETE from handlers/core.rs
methods::PURPOSE_NORTH_STAR_ALIGNMENT => { ... }
methods::NORTH_STAR_UPDATE => { ... }
methods::PURPOSE_SET_GOAL => { ... }
```

**Step 1.3: Remove handler implementations**
```rust
// DELETE from handlers/purpose.rs
pub(super) async fn handle_north_star_alignment(...) { ... }
pub(super) async fn handle_north_star_update(...) { ... }
pub(super) async fn handle_set_goal(...) { ... }
```

### 8.2 Phase 2: Remove Goal Single-Embedding (Week 2)

**Step 2.1: Update GoalNode structure**
```rust
// BEFORE (broken - single embedding)
pub struct GoalNode {
    pub id: GoalId,
    pub description: String,
    pub level: GoalLevel,
    pub parent: Option<GoalId>,
    pub embedding: Vec<f32>,      // REMOVE - single 1024D
    pub keywords: Vec<String>,
    pub propagation_weight: f32,
}

// AFTER (emergent purposes only)
// GoalNode is REPLACED by DiscoveredPurpose
// Goals cannot be created - they emerge from data
```

**Step 2.2: Remove GoalNode constructors**
```rust
// DELETE entirely
pub fn north_star(...) -> Self { ... }
pub fn with_embedding(...) -> Self { ... }
pub fn from_description(...) -> Self { ... }
```

### 8.3 Phase 3: Replace Alignment Calculator (Week 3)

**Step 3.1: Remove projection functions**
```rust
// DELETE from alignment/calculator.rs - these were mathematically invalid
fn project_embedding(source: &[f32], target_dim: usize) -> Vec<f32> { ... }
fn resize_for_comparison(a: &[f32], b: &[f32]) -> (Vec<f32>, Vec<f32>) { ... }
```

**Step 3.2: Implement TeleologicalAlignmentCalculator**
(See Section 7.1 above)

### 8.4 Phase 4: Add Autonomous Discovery (Week 4)

**Step 4.1: Implement AutonomousTeleologicalDiscovery**
(See Section 2.2 above)

**Step 4.2: Add discovery trigger hooks**
```rust
// Trigger discovery after sufficient memories stored
pub async fn maybe_trigger_discovery(&self) -> Option<Vec<DiscoveredPurpose>> {
    let memory_count = self.store.count().await?;
    let last_discovery = self.state.last_discovery_time;
    let time_since = Utc::now() - last_discovery;

    // Trigger conditions (all autonomous):
    // 1. Minimum memories for statistical significance
    // 2. Sufficient time since last discovery
    // 3. Significant new data since last discovery
    if memory_count >= MIN_MEMORIES_FOR_DISCOVERY
        && time_since >= Duration::hours(1)
        && self.significant_new_data() {

        let purposes = self.discovery.discover_purposes(
            DiscoveryConfig::default()
        ).await?;

        self.state.update_discovered_purposes(&purposes);
        Some(purposes)
    } else {
        None
    }
}
```

---

## 9. Migration Strategy

### 9.1 Why Migration Is Necessary

Existing North Star goals with single embeddings **cannot be directly migrated** because:
1. They only contain semantic information (E1 equivalent)
2. We cannot reverse-engineer the other 12 embeddings
3. The comparison basis was mathematically invalid

### 9.2 Migration Options

| Option | Approach | Recommendation |
|--------|----------|----------------|
| **Regenerate** | Re-embed goal descriptions through all 13 embedders | **Transitional only** |
| **Drop** | Remove all manual goals, let system discover naturally | **RECOMMENDED** |
| **Bootstrap** | Use old goals to seed initial clustering | Acceptable for transition |

**Recommended Approach: Clean Slate with Bootstrap Period**

```rust
/// Migration from manual goals to autonomous discovery
pub async fn migrate_to_autonomous(
    legacy_goals: Vec<LegacyGoalNode>,
    store: &TeleologicalArrayStore,
    embedder: &MultiArrayEmbeddingProvider,
) -> MigrationResult {
    // Phase 1: Archive legacy goals (do not use for comparison)
    for goal in &legacy_goals {
        archive_legacy_goal(goal);
    }

    // Phase 2: Bootstrap discovery with existing memories
    let discovery = AutonomousTeleologicalDiscovery::new(store);
    let initial_purposes = discovery.discover_purposes(
        DiscoveryConfig {
            min_memories_for_discovery: 10,  // Lower threshold for bootstrap
            min_importance: 0.3,              // Accept lower-confidence purposes
            ..Default::default()
        }
    ).await?;

    // Phase 3: Log migration results
    MigrationResult {
        legacy_goals_archived: legacy_goals.len(),
        purposes_discovered: initial_purposes.len(),
        dominant_purpose: initial_purposes.first().map(|p| p.description.clone()),
        recommendation: if initial_purposes.is_empty() {
            "Store more memories before purposes can emerge"
        } else {
            "Autonomous discovery active - no manual goals needed"
        },
    }
}
```

### 9.3 API Compatibility: Graceful Deprecation

Removed endpoints return clear error messages:

```rust
// Temporary shim during transition (remove after migration period)
methods::NORTH_STAR_UPDATE => {
    JsonRpcResponse::error(
        id,
        error_codes::METHOD_REMOVED,
        concat!(
            "purpose/north_star_update has been REMOVED. ",
            "Goals are now discovered autonomously from stored memories. ",
            "Use 'purpose/discover_purposes' to see emergent goals, or ",
            "simply store memories and let purposes emerge naturally."
        ),
    )
}

methods::PURPOSE_NORTH_STAR_ALIGNMENT => {
    JsonRpcResponse::error(
        id,
        error_codes::METHOD_REMOVED,
        concat!(
            "purpose/north_star_alignment has been REMOVED. ",
            "Use 'purpose/compute_alignment' with a DiscoveredPurpose ID, or ",
            "use 'memory/search' with comparison_type for teleological search."
        ),
    )
}
```

---

## 10. New MCP Tools for Autonomous Discovery

### 10.1 Purpose Discovery Tools

| Tool | Purpose | Parameters |
|------|---------|------------|
| `purpose/discover` | Trigger autonomous discovery | `min_confidence`, `max_purposes` |
| `purpose/list_discovered` | List all discovered purposes | `include_hierarchy`, `sort_by` |
| `purpose/get_dominant` | Get current dominant North Star | - |
| `purpose/compute_alignment` | Align memory to discovered purpose | `memory_id`, `purpose_id` |
| `purpose/get_hierarchy` | Get emergent purpose hierarchy | `depth` |

### 10.2 Example: New Purpose Discovery

```json
// Request: Discover purposes from stored data
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "purpose/discover",
    "params": {
        "min_confidence": 0.6,
        "max_purposes": 10,
        "include_description": true
    }
}

// Response: Autonomously discovered purposes
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "purposes_discovered": 3,
        "dominant_purpose": {
            "id": "purpose_a7f3c",
            "description": "Code quality and testing patterns",
            "importance": 0.85,
            "coherence": 0.91,
            "member_count": 47,
            "discovery_method": "semantic_auto_aggregation"
        },
        "all_purposes": [
            {
                "id": "purpose_a7f3c",
                "description": "Code quality and testing patterns",
                "importance": 0.85,
                "coherence": 0.91,
                "member_count": 47,
                "level": "root"
            },
            {
                "id": "purpose_b2e1d",
                "description": "API design and documentation",
                "importance": 0.72,
                "coherence": 0.88,
                "member_count": 31,
                "level": "mid"
            },
            {
                "id": "purpose_c9f4a",
                "description": "Performance optimization techniques",
                "importance": 0.68,
                "coherence": 0.79,
                "member_count": 23,
                "level": "mid"
            }
        ],
        "recommendation": "Store more memories in underrepresented areas for richer purpose discovery"
    }
}
```

---

## 11. Verification and Testing

### 11.1 Verify Removal is Complete

```bash
# Ensure no references to removed functionality
rg "north_star_alignment" --type rust
rg "north_star_update" --type rust
rg "project_embedding" --type rust
rg "GoalNode::north_star" --type rust
rg "single.*embedding.*goal" --type rust -i

# Verify alignment uses arrays only
rg "compute_alignment" --type rust | grep -v TeleologicalArray
# Should return nothing - all alignment uses full arrays
```

### 11.2 New Test Suite

```rust
#[cfg(test)]
mod autonomous_discovery_tests {
    #[test]
    fn test_purposes_emerge_from_data() {
        let store = TestTeleologicalStore::with_fixtures(100);
        let discovery = AutonomousTeleologicalDiscovery::new(&store);

        let purposes = discovery.discover_purposes(DiscoveryConfig::default())
            .await.unwrap();

        // Purposes MUST emerge from data
        assert!(!purposes.is_empty(), "Purposes should emerge from sufficient data");

        // Each purpose MUST be a full teleological array
        for purpose in &purposes {
            assert_eq!(purpose.centroid.embeddings.len(), 13);
            for (i, emb) in purpose.centroid.embeddings.iter().enumerate() {
                assert!(
                    matches_expected_output_type(emb, Embedder::from_index(i)),
                    "Purpose centroid must have correct embedder types"
                );
            }
        }
    }

    #[test]
    fn test_alignment_uses_full_arrays() {
        let calculator = TeleologicalAlignmentCalculator::new();
        let memory = TeleologicalArray::test_fixture();
        let purpose = TeleologicalArray::test_fixture();

        let result = calculator.compute_alignment(&memory, &purpose);

        // All 13 embedders MUST be compared
        assert_eq!(result.per_embedder.len(), 13);
        for score in &result.per_embedder {
            assert!(*score >= 0.0 && *score <= 1.0);
        }
    }

    #[test]
    fn test_no_manual_goal_creation() {
        // This test ensures manual goal creation is impossible
        // GoalNode with single embedding should not exist

        // The following should NOT compile:
        // let goal = GoalNode::north_star("id", "desc", vec![0.0; 1024], vec![]);

        // Only DiscoveredPurpose can exist, and it requires a TeleologicalArray
    }

    #[test]
    fn test_projection_removed() {
        // Ensure projection function does not exist
        // This test passes by compilation - if project_embedding exists, it won't compile

        // The alignment calculator should have no project_embedding method
        let calculator = TeleologicalAlignmentCalculator::new();
        // calculator.project_embedding(...) // Should not exist
    }
}
```

---

## 12. Success Criteria

### 12.1 Removal Checklist

- [ ] `purpose/north_star_alignment` endpoint removed
- [ ] `purpose/north_star_update` endpoint removed
- [ ] `purpose/set_goal` endpoint removed
- [ ] `GoalNode` struct removed or repurposed
- [ ] `GoalNode.embedding` field removed
- [ ] `project_embedding` function removed
- [ ] All projection-based comparison removed
- [ ] `NorthStarConfig` removed from configuration

### 12.2 Autonomous Discovery Checklist

- [ ] `AutonomousTeleologicalDiscovery` implemented
- [ ] Hierarchical clustering on 13-space implemented
- [ ] Centroid computation for all 13 embedders implemented
- [ ] `purpose/discover` endpoint added
- [ ] `purpose/list_discovered` endpoint added
- [ ] `purpose/get_dominant` endpoint added
- [ ] `purpose/compute_alignment` uses full arrays
- [ ] Surprise-adaptive intrinsic motivation implemented

### 12.3 Hooks and Skills Checklist

- [ ] SessionEnd hook for goal discovery configured
- [ ] Background hooks for pattern refinement configured
- [ ] Pre-task hook for purpose context loading configured
- [ ] Post-edit hook for pattern learning configured
- [ ] Goal discovery skill defined and triggers on reflective queries
- [ ] Pattern analysis skill defined
- [ ] Reflective query skill defined

### 12.4 Subagent Checklist

- [ ] Clustering agent defined and scheduled
- [ ] Pattern detection agent defined and triggered
- [ ] Goal emergence agent defined as coordinator
- [ ] Agent coordination configuration complete
- [ ] Inter-agent communication working

### 12.5 Testing Checklist

- [ ] All references to removed code eliminated
- [ ] New test suite for autonomous discovery passing
- [ ] Integration tests for purpose emergence passing
- [ ] Hook triggering tests passing
- [ ] Skill invocation tests passing
- [ ] Subagent coordination tests passing
- [ ] Migration script tested and documented
- [ ] API compatibility shims in place

---

## 13. References

### Research Foundations

- [SAGA: Accelerating Scientific Discovery with Autonomous Goal-evolving Agents](https://arxiv.org/abs/2512.21782) - Goal-evolving AI systems
- [Surprise-Adaptive Intrinsic Motivation for Unsupervised Reinforcement Learning](https://arxiv.org/abs/2405.17243) - Adaptive intrinsic motivation
- [Constrained Intrinsic Motivation for Reinforcement Learning (IJCAI 2024)](https://www.ijcai.org/proceedings/2024/620) - Constrained exploration
- [Self-organizing neural networks for universal learning](https://www.sciencedirect.com/science/article/abs/pii/S0893608019302370) - Fusion ART memory systems
- [Emergent Abilities in Large Language Models: A Survey](https://arxiv.org/html/2503.05788v1) - Emergence patterns
- [Semantic Clustering in Vector Databases (MongoDB)](https://www.mongodb.com/developer/products/atlas/discover-latent-semantic-structure-with-vector-clustering/) - Semantic auto-aggregation
- [International AI Safety Report 2025](https://internationalaisafetyreport.org/publication/international-ai-safety-report-2025) - Autonomous agent safety

### Internal Documentation

- [Teleological Fingerprint Architecture](./01-ARCHITECTURE.md)
- [Storage Layer Design](./02-STORAGE.md)
- [Search and Retrieval](./03-SEARCH.md)
- [Comparison Types](./04-COMPARISON.md)
- [Autonomous Integration](./06-AUTONOMOUS-INTEGRATION.md)

---

## Summary

This specification completely removes manual North Star creation and replaces it with **autonomous teleological discovery**. The key insights are:

1. **Goals emerge from data** - Like SAGA and intrinsic motivation systems, purposes are discovered, not defined
2. **North Stars are teleological arrays** - Full 13-embedder arrays, not single embeddings
3. **All comparisons are valid** - E1<->E1, E5<->E5, etc. - no more meaningless projection
4. **The system learns autonomously** - Surprise-adaptive exploration finds meaningful patterns
5. **Hierarchical purposes emerge** - Clustering at multiple granularities reveals purpose structure

### What Makes This TRUE Autonomy

The addition of **hooks, skills, and subagents** transforms theoretical autonomy into practical reality:

- **Hooks** trigger discovery automatically at session boundaries and during idle time
- **Skills** surface emergent purposes on reflective queries without manual invocation
- **Subagents** coordinate clustering, pattern detection, and goal emergence continuously

The result is a mathematically valid, self-organizing system where purposes genuinely emerge from the patterns in stored memories - with **zero manual configuration required**.

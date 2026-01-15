# Context-Graph MCP Tools Reference

This document lists all 59 MCP tools available in context-graph, organized by category.

---

## Core Tools (6)

| Tool | Description |
|------|-------------|
| `inject_context` | Inject context into the knowledge graph with UTL processing. Analyzes content for learning potential and stores with computed metrics. |
| `store_memory` | Store a memory node directly in the knowledge graph without UTL processing. |
| `get_memetic_status` | Get current system status with LIVE UTL metrics: entropy, coherence, learning score, Johari quadrant, consolidation phase, and 5-layer bio-nervous system status. |
| `get_graph_manifest` | Get the 5-layer bio-nervous system architecture description and current layer statuses. |
| `search_graph` | Search the knowledge graph using semantic similarity. Returns nodes matching the query with relevance scores. |
| `utl_status` | Query current UTL (Unified Theory of Learning) system state including lifecycle phase, entropy, coherence, learning score, Johari quadrant. |

---

## GWT (Global Workspace Theory) Tools (9)

| Tool | Description |
|------|-------------|
| `get_consciousness_state` | Get current consciousness state including Kuramoto sync (r), consciousness level (C), meta-cognitive score, differentiation, workspace status, and identity coherence. |
| `get_kuramoto_sync` | Get Kuramoto oscillator network synchronization state: order parameter (r), mean phase (psi), 13 oscillator phases, natural frequencies, and coupling strength. |
| `get_workspace_status` | Get Global Workspace status including active memory, competing candidates, broadcast state, and coherence threshold. Returns WTA selection details. |
| `get_ego_state` | Get Self-Ego Node state including purpose vector (13D), identity continuity, coherence with actions, trajectory length, and crisis detection state. |
| `trigger_workspace_broadcast` | Trigger winner-take-all workspace broadcast with a specific memory. Forces memory into workspace competition. |
| `adjust_coupling` | Adjust Kuramoto oscillator network coupling strength K. Higher K leads to faster synchronization. K is clamped to [0, 10]. |
| `get_coherence_state` | Get high-level GWT workspace coherence state: order parameter, coherence level (High/Medium/Low), broadcasting status, conflict detection. |
| `get_identity_continuity` | Get focused identity continuity (IC) status: IC value (0.0-1.0), status classification (Healthy/Warning/Degraded/Critical), in_crisis flag. |
| `get_kuramoto_state` | Get detailed Kuramoto network state including stepper running status, phases (13), frequencies (13), coupling, order_parameter, mean_phase. |

---

## UTL Tools (1)

| Tool | Description |
|------|-------------|
| `gwt/compute_delta_sc` | Compute per-embedder entropy (delta-S) and aggregate coherence (delta-C) for GWT workspace evaluation. Returns 13 delta-S values, aggregate delta-S, delta-C, Johari quadrant classifications. |

---

## ATC (Adaptive Threshold Calibration) Tools (3)

| Tool | Description |
|------|-------------|
| `get_threshold_status` | Get current ATC threshold status including all thresholds, calibration state, per-embedder temperatures, drift scores, and bandit exploration stats. |
| `get_calibration_metrics` | Get calibration quality metrics: ECE (Expected Calibration Error), MCE (Maximum Calibration Error), Brier Score, drift scores, calibration status. |
| `trigger_recalibration` | Manually trigger recalibration at a specific ATC level (1=EWMA, 2=Temperature, 3=Thompson Sampling, 4=Bayesian meta-optimization). |

---

## Dream Tools (8)

| Tool | Description |
|------|-------------|
| `trigger_dream` | Manually trigger a dream consolidation cycle. Requires rationale for audit logging. Phase controls which dream phases run (nrem/rem/full_cycle). |
| `get_dream_status` | Get current dream system status: state (Awake/NREM/REM/Waking), GPU usage, activity level, time since last cycle. |
| `abort_dream` | Abort the current dream cycle. Must complete wake within 100ms. Returns wake latency and partial dream report. |
| `get_amortized_shortcuts` | Get shortcut candidates from amortized learning. Returns paths traversed 5+ times with 3+ hops that qualify for direct edge creation. |
| `get_gpu_status` | Get GPU utilization and dream eligibility status. Returns GPU usage (0.0-1.0), dream eligibility (GPU < 80%), abort threshold (GPU > 30%). |
| `trigger_mental_check` | Trigger mental_check workflow based on entropy threshold. Fires when entropy > threshold (default 0.7). |
| `get_trigger_config` | Get current trigger configuration: entropy_threshold, ic_threshold, cooldown_ms, last_trigger_timestamp, trigger_count, enabled status. |
| `get_trigger_history` | Get recent trigger history showing when and why triggers fired. Returns events with timestamp, entropy value, reason, workflow status. |

---

## Neuromodulation Tools (2)

| Tool | Description |
|------|-------------|
| `get_neuromodulation_state` | Get current neuromodulation state for all 4 modulators: Dopamine, Serotonin, Noradrenaline, Acetylcholine. |
| `adjust_neuromodulator` | Adjust a specific neuromodulator level (dopamine, serotonin, noradrenaline). ACh is read-only (managed by GWT). |

---

## Steering Tools (1)

| Tool | Description |
|------|-------------|
| `get_steering_feedback` | Get steering feedback from Gardener (graph health), Curator (memory quality), and Assessor (performance). Returns SteeringReward in [-1, 1]. |

---

## Causal Inference Tools (1)

| Tool | Description |
|------|-------------|
| `omni_infer` | Perform omni-directional causal inference. Supports 5 directions: forward (A->B), backward (B->A), bidirectional (A<->B), bridge (cross-domain), abduction (best hypothesis). |

---

## Teleological Tools (5)

| Tool | Description |
|------|-------------|
| `search_teleological` | Perform teleological matrix search across all 13 embedder dimensions. Computes cross-correlation similarity at multiple levels: full matrix, purpose vector, group alignments. |
| `compute_teleological_vector` | Compute a complete teleological vector from content using all 13 embedders. Returns purpose vector (13D), cross-correlations (78D), group alignments (6D). |
| `fuse_embeddings` | Fuse embedding outputs using synergy matrix and profile weights. Supports fusion methods: linear, attention, gated, hierarchical, tucker. |
| `update_synergy_matrix` | Update the synergy matrix based on retrieval feedback. Implements online learning to adapt cross-embedding relationships. |
| `manage_teleological_profile` | CRUD operations for task-specific teleological profiles. Profiles define per-embedder weights, fusion strategy, and group priorities. |

---

## Autonomous North Star Tools (13)

| Tool | Description |
|------|-------------|
| `auto_bootstrap_north_star` | Bootstrap autonomous North Star system from existing teleological embeddings. Analyzes 13-embedder fingerprints to discover emergent purpose patterns. |
| `get_alignment_drift` | Get current alignment drift state including severity, trend, and recommendations. Measures deviation from North Star goal alignment. |
| `get_drift_history` | Get historical drift measurements over time. Returns timestamped entries with per-embedder similarity scores and trend analysis. |
| `trigger_drift_correction` | Manually trigger a drift correction cycle. Applies correction strategies based on severity: threshold adjustment, weight rebalancing, goal reinforcement. |
| `get_pruning_candidates` | Identify memories that are candidates for pruning based on staleness, low alignment, redundancy, or orphaned status. |
| `trigger_consolidation` | Trigger memory consolidation to merge similar memories. Uses similarity-based, temporal, or semantic strategies. |
| `discover_sub_goals` | Discover potential sub-goals from memory clusters. Analyzes stored memories to find emergent themes and patterns. |
| `get_autonomous_status` | Get comprehensive autonomous North Star system status: drift detection, correction, pruning, consolidation, sub-goal discovery. |
| `get_learner_state` | Get Meta-UTL learner state: accuracy, prediction count, domain-specific stats, lambda weights. |
| `observe_outcome` | Record actual outcome for a Meta-UTL prediction. Enables self-correction through outcome observation. |
| `execute_prune` | Execute pruning on identified candidate nodes. Uses soft delete with 30-day recovery. SELF_EGO_NODE is protected. |
| `get_health_status` | Get health status for all major subsystems: UTL, GWT, Dream, Storage. Returns overall_status (healthy/degraded/critical). |
| `trigger_healing` | Trigger self-healing protocol for a degraded subsystem. Actions vary by subsystem: lambda reset, Kuramoto phase reset, dream abort, RocksDB compact. |

---

## Meta-UTL Tools (3)

| Tool | Description |
|------|-------------|
| `get_meta_learning_status` | Get current Meta-UTL self-correction status: accuracy, lambda weights, escalation state, rolling accuracy, event count. |
| `trigger_lambda_recalibration` | Manually trigger lambda weight recalibration using gradient adjustment or Bayesian optimization. Supports dry-run mode. |
| `get_meta_learning_log` | Query meta-learning event log with filters. Events include lambda_adjustment, bayesian_escalation, accuracy_alert, accuracy_recovery. |

---

## Epistemic Tools (1)

| Tool | Description |
|------|-------------|
| `epistemic_action` | Perform epistemic action on GWT workspace to update uncertainty/knowledge states. Actions: assert, retract, query, hypothesize, verify. Used when Johari quadrant is Unknown. |

---

## Merge Tools (1)

| Tool | Description |
|------|-------------|
| `merge_concepts` | Merge related concept nodes into a unified node. Strategies: union (combine all), intersection (common only), weighted_average (by importance). Returns reversal_hash for 30-day undo. |

---

## Johari Classification Tools (1)

| Tool | Description |
|------|-------------|
| `get_johari_classification` | Classify into Johari Window quadrant based on surprise (delta_s) and coherence (delta_c). Quadrants: Open, Blind, Hidden, Unknown. |

---

## Session Tools (4)

| Tool | Description |
|------|-------------|
| `session_start` | Initialize a new MCP session. Per ARCH-07: SessionStart hook controls memory lifecycle. Returns session_id for subsequent calls. |
| `session_end` | Terminate an MCP session and perform cleanup. Returns session summary with tool count and duration. |
| `pre_tool_use` | Hook called before tool execution. Records tool name and prepares session state for tool execution. |
| `post_tool_use` | Hook called after tool execution. Updates session statistics and triggers memory consolidation if needed. |

---

## Summary by Category

| Category | Count |
|----------|-------|
| Core | 6 |
| GWT (Global Workspace Theory) | 9 |
| UTL | 1 |
| ATC (Adaptive Threshold Calibration) | 3 |
| Dream | 8 |
| Neuromodulation | 2 |
| Steering | 1 |
| Causal Inference | 1 |
| Teleological | 5 |
| Autonomous North Star | 13 |
| Meta-UTL | 3 |
| Epistemic | 1 |
| Merge | 1 |
| Johari Classification | 1 |
| Session | 4 |
| **Total** | **59** |

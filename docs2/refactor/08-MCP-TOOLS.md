# MCP Tools Specification - Teleological Array System

## Overview

This document specifies the MCP tools for the refactored teleological array system. The design follows modern MCP best practices including:

- **Progressive tool discovery**: Group related operations, minimize tool count
- **Workflow-based design**: Tools handle complete user goals atomically
- **Memory injection as primary input**: Content injection triggers autonomous embedding
- **Entry-point discovery**: Search discovers optimal embedding space entry points
- **No manual North Star tools**: Goals emerge autonomously from data

**Key Design Principles** (per [MCP Server Best Practices](https://www.marktechpost.com/2025/07/23/7-mcp-server-best-practices-for-scalable-ai-integrations-in-2025/)):
1. Less is more - focused tools improve agent performance
2. Atomic workflows - tools complete entire operations internally
3. Semantic-first API - meaning over mechanics

**Claude Code Integration Architecture**:
```
Memory Injection (MCP) --> Autonomous Embedding (13 models)
                      --> Teleological Array Storage
                      --> Entry-Point Discovery (any of 13 spaces)
                      --> Full Array Comparison (apples to apples)
                      --> Autonomous Goal Emergence (clustering)
```

---

## Hook Integration

This section defines how Claude Code hooks trigger MCP tool invocations. Hooks provide lifecycle-based automation for memory operations.

### Hook-to-Tool Mapping

| Hook Event | Triggered MCP Tool | Purpose |
|------------|-------------------|---------|
| `PreToolUse` (file read) | `memory/inject` | Auto-inject file content before use |
| `PreToolUse` (search) | `memory/search` | Pre-fetch relevant context |
| `PostToolUse` (file write) | `memory/inject` | Store written content in memory |
| `PostToolUse` (code gen) | `memory/inject` | Capture generated code patterns |
| `SessionEnd` | `memory/dream_consolidate` | Consolidate session memories |
| `SessionStart` | `memory/search` | Load relevant prior context |
| `Notification` | `consciousness/sync_level` | Check system synchronization |

### Hook Configuration Examples

**settings.json Hook Configuration**:
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": { "tool_name": "Read" },
        "hooks": [
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "memory/search",
            "params": {
              "query": "{{tool.file_path}}",
              "strategy": { "type": "auto_discover", "max_entry_points": 3 },
              "limit": 5
            }
          }
        ]
      },
      {
        "matcher": { "tool_name": "WebSearch" },
        "hooks": [
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "memory/inject",
            "params": {
              "content": "{{tool.query}}",
              "memory_type": "search_query",
              "namespace": "searches"
            }
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": { "tool_name": "Write" },
        "hooks": [
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "memory/inject",
            "params": {
              "content": "{{tool.content}}",
              "memory_type": "code_artifact",
              "namespace": "artifacts",
              "metadata": {
                "file": "{{tool.file_path}}",
                "operation": "write"
              }
            }
          }
        ]
      },
      {
        "matcher": { "tool_name": "Edit" },
        "hooks": [
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "memory/inject",
            "params": {
              "content": "{{tool.new_string}}",
              "memory_type": "code_edit",
              "namespace": "edits",
              "metadata": {
                "file": "{{tool.file_path}}",
                "old_string": "{{tool.old_string}}"
              }
            }
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "memory/dream_consolidate",
            "params": {
              "session_id": "{{session.id}}",
              "consolidation_strategy": "importance_weighted"
            }
          },
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "goal/cluster_analysis",
            "params": {
              "namespace": "{{session.namespace}}",
              "min_cluster_size": 3
            }
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "consciousness/get_state",
            "params": {}
          },
          {
            "type": "mcp",
            "server": "contextgraph",
            "tool": "memory/search",
            "params": {
              "query": "recent session context",
              "strategy": { "type": "embedder_group", "group": "temporal" },
              "limit": 10
            }
          }
        ]
      }
    ]
  }
}
```

### Per-Tool Hook Triggers

#### memory/inject
**Triggering Hooks**: `PostToolUse` (Write, Edit, Bash output), `PreToolUse` (file read for auto-indexing)
```yaml
trigger_conditions:
  - hook: PostToolUse
    tools: [Write, Edit, MultiEdit]
    condition: "tool.success == true"
  - hook: PostToolUse
    tools: [Bash]
    condition: "tool.stdout.length > 100"
  - hook: PreToolUse
    tools: [Read]
    condition: "file.extension in ['.rs', '.ts', '.py', '.md']"
```

#### memory/search
**Triggering Hooks**: `PreToolUse` (contextual pre-fetch), `SessionStart`
```yaml
trigger_conditions:
  - hook: PreToolUse
    tools: [Read, Grep, Glob]
    condition: "always"
  - hook: SessionStart
    condition: "always"
```

#### consciousness/get_state
**Triggering Hooks**: `SessionStart`, `Notification` (periodic health check)
```yaml
trigger_conditions:
  - hook: SessionStart
    condition: "always"
  - hook: Notification
    event: "health_check"
    interval: "5m"
```

#### memory/dream_consolidate
**Triggering Hooks**: `SessionEnd`
```yaml
trigger_conditions:
  - hook: SessionEnd
    condition: "always"
```

---

## Skill Wrappers

Skills wrap MCP tools to provide higher-level abstractions. Each skill auto-invokes the appropriate MCP tool(s).

### Skill Definition Pattern

```yaml
# Skill: memory-inject
name: memory-inject
description: Inject content into teleological memory with autonomous embedding
mcp_tool: memory/inject
auto_invoke: true
parameters:
  content: required
  memory_type: optional (default: "general")
  namespace: optional (default: "default")
  metadata: optional
skill_behavior:
  pre_processing:
    - validate_content_length
    - detect_content_type
  post_processing:
    - log_injection_result
    - notify_goal_system
```

### Skill-to-MCP Mappings

| Skill Name | MCP Tool(s) Invoked | Auto-Invoke | Description |
|------------|---------------------|-------------|-------------|
| `memory-inject` | `memory/inject` | Yes | Store content with 13-embedder array |
| `memory-search` | `memory/search` | Yes | Search with entry-point discovery |
| `memory-compare` | `memory/compare` | Yes | Compare two teleological arrays |
| `goal-discover` | `purpose/discover_goals` | No | Cluster-based goal discovery |
| `goal-align` | `purpose/goal_alignment` | Yes | Check alignment to discovered goal |
| `drift-check` | `purpose/drift_check` | Yes | Detect purpose drift |
| `consciousness-state` | `consciousness/get_state` | Yes | Get GWT/Kuramoto consciousness state |
| `consciousness-sync` | `consciousness/sync_level` | Yes | Get synchronization level |
| `goal-cluster` | `goal/cluster_analysis` | No | Autonomous goal clustering |
| `dream-consolidate` | `memory/dream_consolidate` | No | Session-end memory consolidation |

### Skill YAML Definitions

#### memory-inject Skill
```yaml
---
skill: memory-inject
version: 1.0.0
description: |
  Inject content into the teleological memory system. Automatically generates
  all 13 embeddings and stores the complete teleological array.

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
    type: enum
    values: [code_context, documentation, code_snippet, conversation, general]
    default: general
  namespace:
    type: string
    default: default
  metadata:
    type: object
    default: {}

on_success:
  - log: "Injected memory {{result.memory_id}} with {{result.embedders_generated}} embedders"
  - notify_hook: post_memory_inject

on_error:
  - log_error: "Memory injection failed: {{error.message}}"
  - retry: { max_attempts: 3, backoff: exponential }
---
```

#### memory-search Skill
```yaml
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
  strategy:
    type: object
    default:
      type: auto_discover
      max_entry_points: 5
      min_confidence: 0.6
  limit:
    type: integer
    default: 10
  threshold:
    type: number
    default: 0.5

transforms:
  result: |
    result.memories.map(m => ({
      id: m.memory_id,
      content: m.content,
      score: m.overall_similarity,
      entry_points: Object.keys(m.entry_point_hits || {})
    }))
---
```

#### consciousness-state Skill
```yaml
---
skill: consciousness-state
version: 1.0.0
description: |
  Get the current Global Workspace Theory (GWT) and Kuramoto oscillator
  consciousness state for the memory system.

mcp:
  server: contextgraph
  tool: consciousness/get_state
  auto_invoke: true

parameters: {}

on_success:
  - set_context: consciousness_state
  - log: "Consciousness sync level: {{result.sync_level}}, phase coherence: {{result.phase_coherence}}"
---
```

#### goal-cluster Skill
```yaml
---
skill: goal-cluster
version: 1.0.0
description: |
  Perform autonomous goal clustering analysis on stored memories.
  Discovers emergent goals from teleological array patterns.

mcp:
  server: contextgraph
  tool: goal/cluster_analysis
  auto_invoke: false  # Expensive operation, manual trigger only

parameters:
  namespace:
    type: string
    default: default
  min_cluster_size:
    type: integer
    default: 5
  algorithm:
    type: enum
    values: [kmeans, hdbscan, spectral]
    default: hdbscan
  sample_size:
    type: integer
    default: 1000

on_success:
  - log: "Discovered {{result.clusters.length}} goal clusters"
  - store_goals: result.clusters
---
```

#### dream-consolidate Skill
```yaml
---
skill: dream-consolidate
version: 1.0.0
description: |
  Consolidate session memories using importance-weighted selection.
  Called automatically at SessionEnd via hooks.

mcp:
  server: contextgraph
  tool: memory/dream_consolidate
  auto_invoke: false  # Triggered by SessionEnd hook

parameters:
  session_id:
    type: string
    required: true
  consolidation_strategy:
    type: enum
    values: [importance_weighted, recency_biased, frequency_based, hybrid]
    default: importance_weighted
  retention_ratio:
    type: number
    default: 0.3
    description: Fraction of memories to retain in consolidated form

on_success:
  - log: "Consolidated {{result.original_count}} memories to {{result.consolidated_count}}"
  - emit_event: session_consolidated
---
```

---

## Subagent Access

This section defines which subagents have access to which MCP tools, following the principle of least privilege.

### Subagent Permission Matrix

| Subagent Type | memory/* | purpose/* | analysis/* | consciousness/* | goal/* |
|---------------|----------|-----------|------------|-----------------|--------|
| `researcher` | inject, search | goal_alignment | embedder_distribution | get_state | - |
| `coder` | inject, search | goal_alignment | - | - | - |
| `architect` | search, compare | discover_goals, drift_check | all | get_state, sync_level | cluster_analysis |
| `reviewer` | search | goal_alignment, drift_check | entry_point_stats | - | - |
| `tester` | inject, search | - | - | - | - |
| `coordinator` | all | all | all | all | all |
| `memory-agent` | all | all | all | all | all |
| `search-agent` | search, search_multi_perspective | goal_alignment | entry_point_stats | get_state | - |
| `goal-agent` | search, batch_compare | all | all | get_state, sync_level | all |
| `consciousness-agent` | search | - | - | all | - |

### Permission Configuration

**settings.json Subagent Permissions**:
```json
{
  "subagent_permissions": {
    "researcher": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": [
            "memory/inject",
            "memory/search",
            "purpose/goal_alignment",
            "analysis/embedder_distribution",
            "consciousness/get_state"
          ],
          "denied": [
            "memory/dream_consolidate",
            "purpose/discover_goals",
            "goal/cluster_analysis"
          ],
          "rate_limits": {
            "memory/inject": { "per_minute": 50 },
            "memory/search": { "per_minute": 200 }
          }
        }
      }
    },
    "coder": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": [
            "memory/inject",
            "memory/search",
            "purpose/goal_alignment"
          ],
          "denied": [
            "purpose/discover_goals",
            "memory/dream_consolidate",
            "goal/cluster_analysis"
          ],
          "rate_limits": {
            "memory/inject": { "per_minute": 100 },
            "memory/search": { "per_minute": 500 }
          }
        }
      }
    },
    "architect": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": [
            "memory/search",
            "memory/compare",
            "memory/batch_compare",
            "memory/similarity_matrix",
            "purpose/discover_goals",
            "purpose/goal_alignment",
            "purpose/drift_check",
            "analysis/*",
            "consciousness/get_state",
            "consciousness/sync_level",
            "goal/cluster_analysis"
          ],
          "rate_limits": {
            "purpose/discover_goals": { "per_hour": 5 },
            "goal/cluster_analysis": { "per_hour": 10 }
          }
        }
      }
    },
    "coordinator": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": ["*"],
          "rate_limits": {
            "memory/inject": { "per_minute": 200 },
            "memory/search": { "per_minute": 1000 }
          }
        }
      }
    },
    "search-agent": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": [
            "memory/search",
            "memory/search_multi_perspective",
            "purpose/goal_alignment",
            "analysis/entry_point_stats",
            "consciousness/get_state"
          ],
          "specialized": true,
          "optimizations": {
            "cache_queries": true,
            "prefetch_embedders": ["e1_semantic", "e7_code"]
          }
        }
      }
    },
    "goal-agent": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": [
            "memory/search",
            "memory/batch_compare",
            "purpose/*",
            "analysis/*",
            "consciousness/get_state",
            "consciousness/sync_level",
            "goal/*"
          ],
          "specialized": true
        }
      }
    },
    "consciousness-agent": {
      "mcp_tools": {
        "contextgraph": {
          "allowed": [
            "memory/search",
            "consciousness/*"
          ],
          "specialized": true,
          "always_include_state": true
        }
      }
    }
  }
}
```

### Specialized Subagent Configurations

#### search-agent
```yaml
subagent: search-agent
specialization: memory_retrieval
mcp_tools:
  primary: memory/search
  secondary: memory/search_multi_perspective
  monitoring: consciousness/get_state
capabilities:
  - multi_perspective_search
  - entry_point_optimization
  - cache_management
default_strategy:
  type: auto_discover
  max_entry_points: 5
performance_targets:
  latency_p99: 100ms
  recall_at_10: 0.85
```

#### goal-agent
```yaml
subagent: goal-agent
specialization: autonomous_goal_discovery
mcp_tools:
  primary: purpose/discover_goals
  secondary: goal/cluster_analysis
  validation: purpose/goal_alignment
  drift: purpose/drift_check
capabilities:
  - clustering_analysis
  - goal_emergence_detection
  - drift_monitoring
  - coherence_tracking
triggers:
  - event: memory_threshold_reached
    threshold: 1000
    action: discover_goals
  - event: session_end
    action: cluster_analysis
```

#### consciousness-agent
```yaml
subagent: consciousness-agent
specialization: gwt_kuramoto_monitoring
mcp_tools:
  primary: consciousness/get_state
  secondary: consciousness/sync_level
capabilities:
  - phase_coherence_monitoring
  - synchronization_tracking
  - workspace_attention_distribution
  - oscillator_coupling_analysis
monitoring:
  interval: 30s
  alert_thresholds:
    sync_level: 0.3
    phase_coherence: 0.4
```

---

## 1. Memory Injection Tools

Memory injection is the **primary input method**. When content is injected, the system autonomously:
1. Generates all 13 embeddings (teleological array)
2. Stores the complete array atomically
3. Updates all 13 indices
4. Returns the created array for verification

### 1.1 memory/inject

**Primary tool for adding memories.** Triggers autonomous teleological array creation.

**Hook Integration**:
- Triggered by: `PostToolUse` (Write, Edit, MultiEdit)
- Skill wrapper: `memory-inject`
- Subagent access: researcher, coder, tester, coordinator, memory-agent

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "memory/inject",
  "params": {
    "content": "The authentication module uses JWT tokens with RSA-256 signing for secure session management.",
    "memory_type": "code_context",
    "namespace": "auth",
    "metadata": {
      "file": "src/auth/jwt.rs",
      "function": "verify_token",
      "tags": ["security", "auth"]
    },
    "options": {
      "return_array_summary": true,
      "return_embedder_stats": false
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "memory_id": "550e8400-e29b-41d4-a716-446655440000",
    "teleological_array": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "embedders_generated": 13,
      "embedder_status": {
        "e1_semantic": { "status": "success", "dims": 1024 },
        "e2_temporal_recent": { "status": "success", "dims": 512 },
        "e3_temporal_periodic": { "status": "success", "dims": 512 },
        "e4_entity": { "status": "success", "dims": 384 },
        "e5_causal": { "status": "success", "dims": 768 },
        "e6_splade_expansion": { "status": "success", "dims": "sparse", "active": 847 },
        "e7_code": { "status": "success", "dims": 1536 },
        "e8_graph": { "status": "success", "dims": 384 },
        "e9_hdc": { "status": "success", "dims": 1024, "type": "binary" },
        "e10_multimodal": { "status": "success", "dims": 768 },
        "e11_entity_transe": { "status": "success", "dims": 384 },
        "e12_late_interaction": { "status": "success", "dims": 128, "tokens": 42 },
        "e13_splade_keyword": { "status": "success", "dims": "sparse", "active": 312 }
      },
      "storage_bytes": 17408,
      "quantization_applied": ["PQ-8", "Float8", "Binary"]
    },
    "indices_updated": 13,
    "created_at": "2025-01-09T10:30:00Z",
    "autonomous_goals_affected": [
      {
        "goal_id": "discovered-auth-jwt",
        "similarity_to_goal": 0.89,
        "contribution": "strengthens"
      }
    ]
  }
}
```

**Key Behaviors**:
- All 13 embedders are generated **atomically** (all-or-nothing)
- Indices updated for immediate searchability
- Autonomous goal system notified of new memory
- If any embedder fails, the entire operation fails (consistency guarantee)

**Rate Limits**: 100 injections/minute, 10,000/hour

---

### 1.2 memory/inject_batch

Batch injection for multiple memories. More efficient than sequential calls.

**Hook Integration**:
- Triggered by: `PostToolUse` (bulk file operations)
- Skill wrapper: `memory-inject-batch`
- Subagent access: coordinator, memory-agent

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "memory/inject_batch",
  "params": {
    "memories": [
      {
        "content": "User authentication flow starts with OAuth2 redirect...",
        "memory_type": "documentation",
        "namespace": "auth"
      },
      {
        "content": "fn validate_token(token: &str) -> Result<Claims, AuthError>...",
        "memory_type": "code_snippet",
        "namespace": "auth",
        "metadata": { "file": "src/auth/mod.rs" }
      }
    ],
    "options": {
      "parallel": true,
      "stop_on_error": false,
      "return_summaries": true
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "result": {
    "total": 2,
    "succeeded": 2,
    "failed": 0,
    "memories": [
      {
        "memory_id": "550e8400-e29b-41d4-a716-446655440001",
        "status": "success",
        "embedders_generated": 13
      },
      {
        "memory_id": "550e8400-e29b-41d4-a716-446655440002",
        "status": "success",
        "embedders_generated": 13
      }
    ],
    "batch_time_ms": 45,
    "indices_updated": 13
  }
}
```

---

## 2. Search Tools with Entry-Point Discovery

Search tools use **entry-point discovery** to find optimal embedding spaces for queries. This enables multi-perspective retrieval across all 13 semantic spaces.

### 2.1 memory/search

**Primary search tool with automatic entry-point discovery.**

**Hook Integration**:
- Triggered by: `PreToolUse` (Read, Grep), `SessionStart`
- Skill wrapper: `memory-search`
- Subagent access: all subagents (most common tool)

The search system analyzes the query to determine which embedding spaces are most relevant, then searches from those entry points.

**Request - Automatic Entry-Point Discovery**:
```json
{
  "jsonrpc": "2.0",
  "id": "3",
  "method": "memory/search",
  "params": {
    "query": "How does JWT token refresh work with the rate limiter?",
    "strategy": {
      "type": "auto_discover",
      "max_entry_points": 5,
      "min_confidence": 0.6
    },
    "limit": 10,
    "options": {
      "include_entry_point_analysis": true,
      "include_per_embedder_scores": true
    }
  }
}
```

**Response with Entry-Point Analysis**:
```json
{
  "jsonrpc": "2.0",
  "id": "3",
  "result": {
    "memories": [
      {
        "memory_id": "550e8400-e29b-41d4-a716-446655440000",
        "content": "JWT token refresh is handled by the middleware...",
        "overall_similarity": 0.92,
        "entry_point_hits": {
          "e1_semantic": { "score": 0.95, "rank": 1 },
          "e5_causal": { "score": 0.88, "rank": 3 },
          "e7_code": { "score": 0.91, "rank": 2 }
        },
        "metadata": { "file": "src/auth/jwt.rs" }
      }
    ],
    "entry_point_discovery": {
      "query_analysis": {
        "detected_intents": ["how_does", "mechanism", "integration"],
        "detected_entities": ["JWT", "token", "refresh", "rate limiter"],
        "detected_relations": ["causes", "interacts_with"]
      },
      "selected_entry_points": [
        {
          "embedder": "e1_semantic",
          "confidence": 0.95,
          "reason": "Strong semantic match for conceptual query"
        },
        {
          "embedder": "e7_code",
          "confidence": 0.88,
          "reason": "Code entities detected (JWT, middleware)"
        },
        {
          "embedder": "e5_causal",
          "confidence": 0.82,
          "reason": "Causal relationship query ('how does X work with Y')"
        }
      ],
      "rejected_entry_points": [
        {
          "embedder": "e8_emotional",
          "confidence": 0.12,
          "reason": "No emotional content detected"
        }
      ]
    },
    "query_info": {
      "total_entry_points_used": 3,
      "fusion_method": "reciprocal_rank_fusion",
      "candidates_scanned": 2500,
      "search_time_ms": 28
    }
  }
}
```

---

### 2.2 memory/search (Explicit Strategy)

When you know which embedding spaces matter, specify them explicitly.

**Hook Integration**:
- Skill wrapper: `memory-search` (with strategy override)
- Subagent access: search-agent, architect, coordinator

**Request - Single Embedder**:
```json
{
  "jsonrpc": "2.0",
  "id": "4",
  "method": "memory/search",
  "params": {
    "query": "authentication errors",
    "strategy": {
      "type": "single_embedder",
      "embedder": "e1_semantic"
    },
    "limit": 10,
    "threshold": 0.7
  }
}
```

**Request - Embedder Group**:
```json
{
  "jsonrpc": "2.0",
  "id": "5",
  "method": "memory/search",
  "params": {
    "query": "recent changes to auth module",
    "strategy": {
      "type": "embedder_group",
      "group": "temporal",
      "embedders": ["e2_temporal_recent", "e3_temporal_periodic"]
    },
    "limit": 10
  }
}
```

**Request - Weighted Full Search**:
```json
{
  "jsonrpc": "2.0",
  "id": "6",
  "method": "memory/search",
  "params": {
    "query": "security implications of JWT implementation",
    "strategy": {
      "type": "weighted_full",
      "weights": {
        "e1_semantic": 0.35,
        "e5_causal": 0.25,
        "e7_code": 0.20,
        "e4_entity": 0.10,
        "e6_splade_expansion": 0.10
      }
    },
    "limit": 10
  }
}
```

**Request - Matrix Strategy**:
```json
{
  "jsonrpc": "2.0",
  "id": "7",
  "method": "memory/search",
  "params": {
    "query": "database connection patterns",
    "strategy": {
      "type": "matrix_strategy",
      "matrix": "knowledge_graph"
    },
    "limit": 10
  }
}
```

---

### 2.3 memory/search_multi_perspective

**Advanced multi-perspective search** that queries from multiple entry points and synthesizes results.

**Hook Integration**:
- Skill wrapper: `memory-search-multi`
- Subagent access: search-agent, architect, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "8",
  "method": "memory/search_multi_perspective",
  "params": {
    "query": "Why is the auth service slow under load?",
    "perspectives": [
      { "name": "semantic", "embedder": "e1_semantic", "weight": 0.3 },
      { "name": "causal", "embedder": "e5_causal", "weight": 0.3 },
      { "name": "temporal", "embedder": "e2_temporal_recent", "weight": 0.2 },
      { "name": "code", "embedder": "e7_code", "weight": 0.2 }
    ],
    "synthesis": {
      "method": "reciprocal_rank_fusion",
      "k": 60
    },
    "limit": 10,
    "include_perspective_breakdown": true
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "8",
  "result": {
    "memories": [
      {
        "memory_id": "550e8400-e29b-41d4-a716-446655440000",
        "content": "Connection pool exhaustion detected under concurrent auth requests...",
        "fused_score": 0.91,
        "perspective_scores": {
          "semantic": { "score": 0.88, "rank": 2 },
          "causal": { "score": 0.95, "rank": 1 },
          "temporal": { "score": 0.72, "rank": 8 },
          "code": { "score": 0.85, "rank": 3 }
        },
        "consensus": {
          "level": "high",
          "agreeing_perspectives": ["semantic", "causal", "code"],
          "diverging_perspectives": ["temporal"]
        }
      }
    ],
    "synthesis_info": {
      "method": "reciprocal_rank_fusion",
      "total_candidates_per_perspective": [150, 120, 180, 95],
      "unique_candidates_after_fusion": 312,
      "synthesis_time_ms": 15
    }
  }
}
```

---

## 3. Teleological Array Comparison Tools

These tools enable direct comparison of teleological arrays - the core operation that replaces broken single-embedding comparisons.

### 3.1 memory/compare

**Direct comparison of two teleological arrays.**

**Hook Integration**:
- Skill wrapper: `memory-compare`
- Subagent access: architect, goal-agent, coordinator

**Request - Compare Two Memory IDs**:
```json
{
  "jsonrpc": "2.0",
  "id": "9",
  "method": "memory/compare",
  "params": {
    "array_a": "550e8400-e29b-41d4-a716-446655440000",
    "array_b": "660e8400-e29b-41d4-a716-446655440001",
    "comparison_type": {
      "type": "weighted_full",
      "weights": {
        "e1_semantic": 0.4,
        "e5_causal": 0.3,
        "e7_code": 0.2,
        "e4_entity": 0.1
      }
    },
    "options": {
      "include_per_embedder": true,
      "include_correlation_analysis": true
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "9",
  "result": {
    "overall_similarity": 0.78,
    "per_embedder_comparison": {
      "e1_semantic": {
        "similarity": 0.85,
        "weight": 0.4,
        "contribution": 0.34
      },
      "e2_temporal_recent": {
        "similarity": 0.42,
        "weight": 0.0,
        "contribution": 0.0
      },
      "e5_causal": {
        "similarity": 0.71,
        "weight": 0.3,
        "contribution": 0.213
      },
      "e7_code": {
        "similarity": 0.88,
        "weight": 0.2,
        "contribution": 0.176
      },
      "e4_entity": {
        "similarity": 0.65,
        "weight": 0.1,
        "contribution": 0.065
      }
    },
    "correlation_analysis": {
      "coherence": 0.72,
      "dominant_embedder": "e7_code",
      "patterns": [
        {
          "type": "semantic_code_align",
          "embedders": ["e1_semantic", "e7_code"],
          "strength": 0.82,
          "description": "Semantic and code patterns strongly correlate"
        }
      ],
      "outliers": [
        {
          "embedder": "e2_temporal_recent",
          "type": "negative",
          "deviation": -2.3
        }
      ]
    },
    "interpretation": {
      "summary": "High semantic and code similarity, but temporal divergence",
      "recommendation": "These memories discuss similar concepts but from different time periods"
    }
  }
}
```

**Request - Compare Query to Stored Memory**:
```json
{
  "jsonrpc": "2.0",
  "id": "10",
  "method": "memory/compare",
  "params": {
    "query": "OAuth2 authentication with JWT tokens",
    "array_b": "660e8400-e29b-41d4-a716-446655440001",
    "comparison_type": {
      "type": "matrix_strategy",
      "matrix": "semantic_focused"
    }
  }
}
```

---

### 3.2 memory/batch_compare

**Compare one reference array against many targets** (for clustering, goal discovery).

**Hook Integration**:
- Skill wrapper: `memory-batch-compare`
- Subagent access: goal-agent, architect, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "11",
  "method": "memory/batch_compare",
  "params": {
    "reference": "550e8400-e29b-41d4-a716-446655440000",
    "targets": [
      "660e8400-e29b-41d4-a716-446655440001",
      "770e8400-e29b-41d4-a716-446655440002",
      "880e8400-e29b-41d4-a716-446655440003"
    ],
    "comparison_type": {
      "type": "matrix_strategy",
      "matrix": "identity"
    },
    "options": {
      "return_per_embedder": true,
      "include_statistics": true,
      "sort_by": "similarity_desc"
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "11",
  "result": {
    "reference_id": "550e8400-e29b-41d4-a716-446655440000",
    "comparisons": [
      {
        "target_id": "660e8400-e29b-41d4-a716-446655440001",
        "overall_similarity": 0.82,
        "rank": 1,
        "per_embedder": {
          "e1_semantic": 0.89,
          "e5_causal": 0.78
        }
      },
      {
        "target_id": "880e8400-e29b-41d4-a716-446655440003",
        "overall_similarity": 0.71,
        "rank": 2,
        "per_embedder": {
          "e1_semantic": 0.75,
          "e5_causal": 0.68
        }
      },
      {
        "target_id": "770e8400-e29b-41d4-a716-446655440002",
        "overall_similarity": 0.58,
        "rank": 3,
        "per_embedder": {
          "e1_semantic": 0.62,
          "e5_causal": 0.55
        }
      }
    ],
    "statistics": {
      "count": 3,
      "mean_similarity": 0.70,
      "std_deviation": 0.12,
      "min": 0.58,
      "max": 0.82,
      "quartiles": [0.58, 0.65, 0.71, 0.77, 0.82]
    },
    "processing_time_ms": 12
  }
}
```

---

### 3.3 memory/similarity_matrix

**Compute pairwise similarities for a set of arrays** (for visualization, clustering).

**Hook Integration**:
- Skill wrapper: `memory-similarity-matrix`
- Subagent access: goal-agent, architect, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "12",
  "method": "memory/similarity_matrix",
  "params": {
    "array_ids": [
      "550e8400-e29b-41d4-a716-446655440000",
      "660e8400-e29b-41d4-a716-446655440001",
      "770e8400-e29b-41d4-a716-446655440002",
      "880e8400-e29b-41d4-a716-446655440003"
    ],
    "comparison_type": {
      "type": "single_embedder",
      "embedder": "e1_semantic"
    },
    "options": {
      "include_clustering_suggestion": true
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "12",
  "result": {
    "matrix": [
      [1.00, 0.82, 0.65, 0.71],
      [0.82, 1.00, 0.58, 0.68],
      [0.65, 0.58, 1.00, 0.89],
      [0.71, 0.68, 0.89, 1.00]
    ],
    "array_ids": [
      "550e8400-e29b-41d4-a716-446655440000",
      "660e8400-e29b-41d4-a716-446655440001",
      "770e8400-e29b-41d4-a716-446655440002",
      "880e8400-e29b-41d4-a716-446655440003"
    ],
    "clustering_suggestion": {
      "optimal_k": 2,
      "silhouette_score": 0.78,
      "suggested_clusters": [
        { "cluster": 0, "members": [0, 1] },
        { "cluster": 1, "members": [2, 3] }
      ]
    },
    "embedder_used": "e1_semantic"
  }
}
```

---

## 4. Autonomous Purpose Tools

These tools work with the **autonomous goal discovery system**. Goals emerge from data - they are not manually created.

### 4.1 purpose/discover_goals

**Autonomously discover goals from stored teleological arrays.**

**Hook Integration**:
- Triggered by: Periodic background task, memory threshold reached
- Skill wrapper: `goal-discover`
- Subagent access: architect, goal-agent, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "13",
  "method": "purpose/discover_goals",
  "params": {
    "namespace": "auth",
    "discovery_config": {
      "sample_size": 500,
      "min_cluster_size": 5,
      "min_coherence": 0.75,
      "clustering_algorithm": "kmeans",
      "num_clusters": "auto"
    },
    "comparison_type": {
      "type": "matrix_strategy",
      "matrix": "semantic_focused"
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "13",
  "result": {
    "discovered_goals": [
      {
        "goal_id": "discovered-auth-jwt-001",
        "description": "JWT-based authentication and token lifecycle management",
        "suggested_level": "strategic",
        "confidence": 0.89,
        "member_count": 47,
        "teleological_array_id": "centroid-550e8400",
        "centroid_strength": {
          "e1_semantic": 0.91,
          "e7_code": 0.87,
          "e5_causal": 0.82
        },
        "dominant_embedders": ["e1_semantic", "e7_code"],
        "keywords": ["JWT", "token", "authentication", "verify", "claims"],
        "coherence_score": 0.89
      },
      {
        "goal_id": "discovered-auth-oauth-002",
        "description": "OAuth2 provider integration and callback handling",
        "suggested_level": "tactical",
        "confidence": 0.82,
        "member_count": 23,
        "teleological_array_id": "centroid-660e8400",
        "centroid_strength": {
          "e1_semantic": 0.88,
          "e4_entity": 0.84,
          "e5_causal": 0.79
        },
        "dominant_embedders": ["e1_semantic", "e4_entity"],
        "keywords": ["OAuth", "provider", "callback", "redirect", "scope"],
        "coherence_score": 0.82
      }
    ],
    "discovery_info": {
      "total_arrays_analyzed": 500,
      "clusters_found": 12,
      "clusters_above_threshold": 2,
      "processing_time_ms": 450
    }
  }
}
```

---

### 4.2 purpose/goal_alignment

**Compute alignment between a memory and a discovered goal using array-to-array comparison.**

**Hook Integration**:
- Triggered by: `PostToolUse` (after memory injection)
- Skill wrapper: `goal-align`
- Subagent access: all (read-only alignment check)

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "14",
  "method": "purpose/goal_alignment",
  "params": {
    "memory_id": "550e8400-e29b-41d4-a716-446655440000",
    "goal_id": "discovered-auth-jwt-001",
    "comparison_type": {
      "type": "weighted_full",
      "weights": {
        "e1_semantic": 0.4,
        "e5_causal": 0.3,
        "e7_code": 0.3
      }
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "14",
  "result": {
    "alignment_score": 0.84,
    "interpretation": "highly_aligned",
    "per_embedder_alignment": {
      "e1_semantic": { "score": 0.89, "interpretation": "strong" },
      "e5_causal": { "score": 0.78, "interpretation": "moderate" },
      "e7_code": { "score": 0.85, "interpretation": "strong" }
    },
    "goal_info": {
      "goal_id": "discovered-auth-jwt-001",
      "description": "JWT-based authentication and token lifecycle management",
      "level": "strategic"
    },
    "contribution_analysis": {
      "strengthens_goal": true,
      "novelty_contribution": 0.15,
      "coherence_contribution": 0.92
    }
  }
}
```

---

### 4.3 purpose/drift_check

**Check if recent work has drifted from established goals using array comparison.**

**Hook Integration**:
- Triggered by: Periodic background task, `SessionEnd`
- Skill wrapper: `drift-check`
- Subagent access: reviewer, architect, goal-agent, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "15",
  "method": "purpose/drift_check",
  "params": {
    "memory_ids": [
      "550e8400-e29b-41d4-a716-446655440000",
      "550e8400-e29b-41d4-a716-446655440001"
    ],
    "goal_id": "discovered-auth-jwt-001",
    "comparison_type": {
      "type": "matrix_strategy",
      "matrix": "semantic_focused"
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "15",
  "result": {
    "overall_drift": {
      "has_drifted": true,
      "drift_score": 0.35,
      "drift_level": "medium"
    },
    "per_embedder_drift": {
      "e1_semantic": { "similarity": 0.72, "drift_level": "low" },
      "e2_temporal_recent": { "similarity": 0.45, "drift_level": "high" },
      "e5_causal": { "similarity": 0.58, "drift_level": "medium" },
      "e7_code": { "similarity": 0.81, "drift_level": "low" }
    },
    "most_drifted_embedders": [
      { "embedder": "e2_temporal_recent", "drift_level": "high" },
      { "embedder": "e5_causal", "drift_level": "medium" }
    ],
    "recommendations": [
      {
        "embedder": "e2_temporal_recent",
        "issue": "Significant temporal drift detected",
        "suggestion": "Recent work has diverged from goal timeline - review if this is intentional"
      },
      {
        "embedder": "e5_causal",
        "issue": "Moderate causal chain drift",
        "suggestion": "Review causal dependencies in implementation"
      }
    ],
    "trend": {
      "direction": "worsening",
      "velocity": 0.05,
      "projected_critical_in": "3 days at current rate"
    }
  }
}
```

---

## 5. Consciousness Tools (NEW)

These tools provide access to the Global Workspace Theory (GWT) and Kuramoto oscillator consciousness model.

### 5.1 consciousness/get_state

**Get the current consciousness state including GWT workspace and Kuramoto synchronization.**

**Hook Integration**:
- Triggered by: `SessionStart`, periodic health checks
- Skill wrapper: `consciousness-state`
- Subagent access: architect, goal-agent, consciousness-agent, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "20",
  "method": "consciousness/get_state",
  "params": {
    "include_oscillators": true,
    "include_workspace": true,
    "include_attention_distribution": true
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "20",
  "result": {
    "global_workspace": {
      "active_coalitions": [
        {
          "coalition_id": "coal-auth-001",
          "strength": 0.89,
          "members": ["memory-550e8400", "memory-660e8400", "memory-770e8400"],
          "dominant_theme": "JWT authentication lifecycle"
        },
        {
          "coalition_id": "coal-perf-002",
          "strength": 0.72,
          "members": ["memory-880e8400", "memory-990e8400"],
          "dominant_theme": "Performance optimization"
        }
      ],
      "broadcast_queue": [
        {
          "content_summary": "Token validation pattern",
          "priority": 0.95,
          "awaiting_broadcast": true
        }
      ],
      "workspace_capacity": {
        "current": 5,
        "max": 7,
        "utilization": 0.71
      }
    },
    "kuramoto_state": {
      "sync_level": 0.78,
      "phase_coherence": 0.82,
      "coupling_strength": 0.65,
      "oscillator_count": 13,
      "oscillator_phases": {
        "e1_semantic": 0.42,
        "e2_temporal_recent": 0.38,
        "e3_temporal_periodic": 0.45,
        "e4_entity": 0.41,
        "e5_causal": 0.39,
        "e6_splade_expansion": 0.44,
        "e7_code": 0.43,
        "e8_graph": 0.40,
        "e9_hdc": 0.37,
        "e10_multimodal": 0.42,
        "e11_entity_transe": 0.38,
        "e12_late_interaction": 0.41,
        "e13_splade_keyword": 0.44
      },
      "order_parameter": {
        "r": 0.78,
        "psi": 0.41
      }
    },
    "attention_distribution": {
      "focused_embedders": ["e1_semantic", "e7_code", "e5_causal"],
      "attention_weights": {
        "e1_semantic": 0.35,
        "e7_code": 0.28,
        "e5_causal": 0.22,
        "e4_entity": 0.08,
        "other": 0.07
      },
      "attention_entropy": 1.45
    },
    "consciousness_metrics": {
      "integration_phi": 0.72,
      "complexity": 0.68,
      "global_availability": 0.85
    },
    "timestamp": "2025-01-09T10:30:00Z"
  }
}
```

---

### 5.2 consciousness/sync_level

**Get the current Kuramoto synchronization level (lightweight health check).**

**Hook Integration**:
- Triggered by: `Notification` (periodic), before complex operations
- Skill wrapper: `consciousness-sync`
- Subagent access: all (lightweight read)

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "21",
  "method": "consciousness/sync_level",
  "params": {}
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "21",
  "result": {
    "sync_level": 0.78,
    "phase_coherence": 0.82,
    "status": "synchronized",
    "status_interpretation": {
      "level": "good",
      "description": "Embedder oscillators are well synchronized",
      "recommendation": null
    },
    "thresholds": {
      "critical_low": 0.3,
      "warning_low": 0.5,
      "optimal_min": 0.7,
      "current_zone": "optimal"
    },
    "trend": {
      "direction": "stable",
      "velocity": 0.01,
      "samples": 10
    }
  }
}
```

---

## 6. Goal Clustering Tools (NEW)

These tools support autonomous goal emergence through clustering analysis.

### 6.1 goal/cluster_analysis

**Perform autonomous goal clustering on stored teleological arrays.**

**Hook Integration**:
- Triggered by: `SessionEnd`, memory threshold reached, manual invocation
- Skill wrapper: `goal-cluster`
- Subagent access: architect, goal-agent, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "22",
  "method": "goal/cluster_analysis",
  "params": {
    "namespace": "default",
    "algorithm": {
      "type": "hdbscan",
      "min_cluster_size": 5,
      "min_samples": 3,
      "cluster_selection_epsilon": 0.1
    },
    "embedder_weights": {
      "e1_semantic": 0.4,
      "e5_causal": 0.3,
      "e7_code": 0.2,
      "e4_entity": 0.1
    },
    "options": {
      "max_clusters": 20,
      "include_outliers": true,
      "include_silhouette_analysis": true,
      "generate_cluster_labels": true
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "22",
  "result": {
    "clusters": [
      {
        "cluster_id": "cluster-auth-001",
        "size": 47,
        "coherence": 0.89,
        "centroid_id": "centroid-550e8400",
        "generated_label": "JWT Authentication and Token Management",
        "dominant_embedders": ["e1_semantic", "e7_code"],
        "keywords": ["JWT", "token", "authentication", "verify", "claims", "refresh"],
        "member_ids": ["550e8400...", "660e8400...", "..."],
        "embedder_coherence": {
          "e1_semantic": 0.91,
          "e7_code": 0.87,
          "e5_causal": 0.82
        }
      },
      {
        "cluster_id": "cluster-oauth-002",
        "size": 23,
        "coherence": 0.82,
        "centroid_id": "centroid-770e8400",
        "generated_label": "OAuth2 Provider Integration",
        "dominant_embedders": ["e1_semantic", "e4_entity"],
        "keywords": ["OAuth", "provider", "callback", "redirect", "scope", "authorize"],
        "member_ids": ["770e8400...", "880e8400...", "..."],
        "embedder_coherence": {
          "e1_semantic": 0.88,
          "e4_entity": 0.84,
          "e5_causal": 0.79
        }
      }
    ],
    "outliers": {
      "count": 12,
      "ids": ["990e8400...", "aa0e8400...", "..."],
      "reason": "Did not meet minimum cluster membership criteria"
    },
    "clustering_metrics": {
      "algorithm": "hdbscan",
      "total_memories_analyzed": 500,
      "clusters_found": 8,
      "noise_ratio": 0.024,
      "silhouette_score": 0.78,
      "calinski_harabasz_score": 245.6,
      "davies_bouldin_score": 0.42
    },
    "goal_emergence": {
      "new_goals_discovered": 2,
      "goals_strengthened": 3,
      "goals_weakened": 1,
      "recommendations": [
        {
          "type": "promote_to_strategic",
          "cluster_id": "cluster-auth-001",
          "reason": "High coherence (0.89) and size (47 members)"
        },
        {
          "type": "merge_candidates",
          "clusters": ["cluster-oauth-002", "cluster-sso-003"],
          "reason": "High inter-cluster similarity (0.72)"
        }
      ]
    },
    "processing_time_ms": 850
  }
}
```

---

## 7. Dream Consolidation Tools (NEW)

These tools support session-end memory consolidation, inspired by sleep/dream memory consolidation.

### 7.1 memory/dream_consolidate

**Consolidate session memories using importance-weighted selection.**

**Hook Integration**:
- Triggered by: `SessionEnd`
- Skill wrapper: `dream-consolidate`
- Subagent access: coordinator, memory-agent

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "23",
  "method": "memory/dream_consolidate",
  "params": {
    "session_id": "session-2025-01-09-abc123",
    "consolidation_strategy": "importance_weighted",
    "options": {
      "retention_ratio": 0.3,
      "importance_factors": {
        "goal_alignment": 0.3,
        "access_frequency": 0.25,
        "recency": 0.2,
        "uniqueness": 0.15,
        "coherence_contribution": 0.1
      },
      "merge_similar": true,
      "similarity_threshold": 0.85,
      "preserve_anchors": true
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "23",
  "result": {
    "consolidation_summary": {
      "session_id": "session-2025-01-09-abc123",
      "original_count": 150,
      "consolidated_count": 45,
      "merged_count": 28,
      "pruned_count": 77,
      "retention_ratio": 0.30,
      "compression_ratio": 3.33
    },
    "retained_memories": [
      {
        "memory_id": "550e8400-e29b-41d4-a716-446655440000",
        "importance_score": 0.95,
        "retention_reason": "High goal alignment (0.92) + frequent access (12 times)"
      },
      {
        "memory_id": "660e8400-e29b-41d4-a716-446655440001",
        "importance_score": 0.88,
        "retention_reason": "Anchor memory for cluster-auth-001"
      }
    ],
    "merged_groups": [
      {
        "merged_into": "merged-770e8400",
        "source_memories": ["770e8400...", "880e8400...", "990e8400..."],
        "merge_reason": "Similarity > 0.85 within OAuth2 callback handling topic"
      }
    ],
    "pruned_memories": {
      "count": 77,
      "reasons": {
        "low_importance": 45,
        "redundant": 22,
        "goal_misaligned": 10
      }
    },
    "goal_impact": {
      "goals_affected": [
        {
          "goal_id": "discovered-auth-jwt-001",
          "impact": "strengthened",
          "coherence_delta": 0.03
        }
      ]
    },
    "consciousness_update": {
      "sync_level_before": 0.75,
      "sync_level_after": 0.82,
      "workspace_cleared": true,
      "new_coalitions_formed": 2
    },
    "processing_time_ms": 320
  }
}
```

---

## 8. Analysis Tools

### 8.1 analysis/embedder_distribution

**Analyze distribution of embedder values across stored arrays.**

**Hook Integration**:
- Skill wrapper: `analysis-distribution`
- Subagent access: researcher, architect, goal-agent, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "16",
  "method": "analysis/embedder_distribution",
  "params": {
    "namespace": "auth",
    "embedders": ["e1_semantic", "e5_causal", "e7_code"],
    "options": {
      "include_correlations": true,
      "sample_size": 1000
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "16",
  "result": {
    "distributions": {
      "e1_semantic": {
        "mean_magnitude": 0.72,
        "variance": 0.15,
        "sparsity": 0.02,
        "percentiles": { "p25": 0.58, "p50": 0.71, "p75": 0.85, "p95": 0.94 }
      },
      "e5_causal": {
        "mean_magnitude": 0.68,
        "variance": 0.22,
        "sparsity": 0.05,
        "percentiles": { "p25": 0.52, "p50": 0.67, "p75": 0.82, "p95": 0.91 }
      },
      "e7_code": {
        "mean_magnitude": 0.81,
        "variance": 0.12,
        "sparsity": 0.01,
        "percentiles": { "p25": 0.72, "p50": 0.80, "p75": 0.89, "p95": 0.96 }
      }
    },
    "correlations": {
      "e1_semantic_e5_causal": 0.67,
      "e1_semantic_e7_code": 0.78,
      "e5_causal_e7_code": 0.59
    },
    "total_arrays_analyzed": 1000
  }
}
```

---

### 8.2 analysis/entry_point_stats

**Get statistics on entry-point discovery effectiveness.**

**Hook Integration**:
- Skill wrapper: `analysis-entry-points`
- Subagent access: search-agent, reviewer, architect, coordinator

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": "17",
  "method": "analysis/entry_point_stats",
  "params": {
    "time_range": "24h",
    "namespace": "auth"
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": "17",
  "result": {
    "total_searches": 1523,
    "entry_point_usage": {
      "e1_semantic": { "count": 1450, "percentage": 95.2, "avg_contribution": 0.42 },
      "e7_code": { "count": 892, "percentage": 58.6, "avg_contribution": 0.28 },
      "e5_causal": { "count": 634, "percentage": 41.6, "avg_contribution": 0.18 },
      "e4_entity": { "count": 412, "percentage": 27.1, "avg_contribution": 0.12 },
      "e6_splade_expansion": { "count": 356, "percentage": 23.4, "avg_contribution": 0.15 }
    },
    "multi_entry_point_searches": {
      "single": 245,
      "two": 623,
      "three_plus": 655
    },
    "average_entry_points_per_search": 2.8,
    "discovery_accuracy": {
      "precision": 0.89,
      "recall": 0.82,
      "f1": 0.85
    }
  }
}
```

---

## 9. Removed Tools

The following tools are **PERMANENTLY REMOVED** as they relied on broken single-embedding comparisons:

| Removed Tool | Reason | Replacement |
|-------------|--------|-------------|
| `purpose/north_star_alignment` | Used projection-based comparison (apples-to-oranges) | `purpose/goal_alignment` |
| `purpose/north_star_update` | Created single 1024D embeddings | Goals discovered via `purpose/discover_goals` |
| `purpose/set_north_star` | Manual goal creation is invalid | Autonomous discovery only |

**Error Response for Removed Tools**:
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "error": {
    "code": -32601,
    "message": "Method removed",
    "data": {
      "deprecated_method": "purpose/north_star_alignment",
      "replacement": "purpose/goal_alignment",
      "reason": "Manual North Star creation used single embeddings that cannot be meaningfully compared to 13-embedder teleological arrays. See docs2/refactor/05-NORTH-STAR-REMOVAL.md",
      "migration_guide": "https://docs/migration/north-star-removal"
    }
  }
}
```

---

## 10. Comparison Type Schema

All tools accepting `comparison_type` use this unified schema:

```json
{
  "comparison_type": {
    "oneOf": [
      {
        "type": "object",
        "properties": {
          "type": { "const": "auto_discover" },
          "max_entry_points": { "type": "integer", "minimum": 1, "maximum": 13, "default": 5 },
          "min_confidence": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.6 }
        },
        "required": ["type"]
      },
      {
        "type": "object",
        "properties": {
          "type": { "const": "single_embedder" },
          "embedder": {
            "enum": [
              "e1_semantic", "e2_temporal_recent", "e3_temporal_periodic",
              "e4_entity", "e5_causal", "e6_splade_expansion",
              "e7_code", "e8_graph", "e9_hdc",
              "e10_multimodal", "e11_entity_transe", "e12_late_interaction",
              "e13_splade_keyword"
            ]
          }
        },
        "required": ["type", "embedder"]
      },
      {
        "type": "object",
        "properties": {
          "type": { "const": "embedder_group" },
          "group": { "enum": ["temporal", "relational", "lexical", "dense", "all"] },
          "embedders": {
            "type": "array",
            "items": { "type": "string" },
            "minItems": 1,
            "maxItems": 13
          }
        },
        "required": ["type"]
      },
      {
        "type": "object",
        "properties": {
          "type": { "const": "weighted_full" },
          "weights": {
            "type": "object",
            "additionalProperties": { "type": "number", "minimum": 0, "maximum": 1 }
          }
        },
        "required": ["type", "weights"]
      },
      {
        "type": "object",
        "properties": {
          "type": { "const": "matrix_strategy" },
          "matrix": {
            "enum": [
              "identity", "semantic_focused", "temporal_aware",
              "knowledge_graph", "emotional_resonance", "precision_retrieval",
              "correlation_aware", "code_heavy"
            ]
          }
        },
        "required": ["type", "matrix"]
      },
      {
        "type": "object",
        "properties": {
          "type": { "const": "matrix_strategy" },
          "custom_matrix": {
            "type": "array",
            "items": {
              "type": "array",
              "items": { "type": "number" },
              "minItems": 13,
              "maxItems": 13
            },
            "minItems": 13,
            "maxItems": 13
          }
        },
        "required": ["type", "custom_matrix"]
      }
    ]
  }
}
```

---

## 11. Predefined Search Matrices

| Matrix Name | Description | Primary Embedders |
|-------------|-------------|-------------------|
| `identity` | Equal weight diagonal (pure apples-to-apples) | All equally |
| `semantic_focused` | 50% weight on E1 semantic | E1 |
| `temporal_aware` | Emphasizes time patterns | E2, E3 |
| `knowledge_graph` | Entity and causal relationships | E4, E5 |
| `emotional_resonance` | Affective matching | E8 |
| `precision_retrieval` | Lexical exactness | E6, E12, E13 |
| `correlation_aware` | Cross-embedder correlations enabled | Various |
| `code_heavy` | Code understanding priority | E7 |

---

## 12. Embedder Reference

| ID | Name | Dimensions | Type | Purpose |
|----|------|------------|------|---------|
| E1 | Semantic | 1024D (Matryoshka) | Dense | Meaning understanding |
| E2 | Temporal Recent | 512D | Dense | Recency/freshness |
| E3 | Temporal Periodic | 512D (Fourier) | Dense | Cyclical patterns |
| E4 | Entity | 384D (TransE) | Dense | Entity relationships |
| E5 | Causal | 768D (asymmetric) | Dense | Cause-effect chains |
| E6 | SPLADE Primary | ~30K sparse | Sparse | Term expansion |
| E7 | Code | 1536D (AST) | Dense | Code understanding |
| E8 | Emotional | 384D | Dense | Sentiment/affect |
| E9 | HDC | 10K-bit | Binary | Holographic distributed |
| E10 | Multimodal | 768D | Dense | Cross-modal |
| E11 | Entity TransE | 384D | Dense | Knowledge base facts |
| E12 | Late Interaction | 128D/token | Token-level | Precision matching |
| E13 | SPLADE Keyword | ~30K sparse | Sparse | Keyword precision |

---

## 13. Implementation Notes

### Thread Safety
All tools are thread-safe and can be called concurrently.

### Batching
For bulk operations, prefer batch variants (`memory/inject_batch`, `memory/batch_compare`) over loops of single operations.

### Caching
Query embeddings are cached for 60 seconds to speed up repeated similar queries.

### Index Warming
First queries after server start may be slower as indices are loaded into memory.

### Error Handling
All tools return structured errors with:
- Error code (standard JSON-RPC codes)
- Human-readable message
- Additional data for debugging

### Rate Limits
| Operation | Limit |
|-----------|-------|
| `memory/inject` | 100/minute |
| `memory/inject_batch` | 20/minute (up to 100 items each) |
| `memory/search` | 1000/minute |
| `purpose/discover_goals` | 10/hour |
| `goal/cluster_analysis` | 10/hour |
| `memory/dream_consolidate` | 60/hour |
| `consciousness/get_state` | 120/minute |
| `consciousness/sync_level` | 600/minute |

---

## 14. Security Considerations

Per [MCP security best practices](https://www.thoughtworks.com/en-us/insights/blog/generative-ai/model-context-protocol-mcp-impact-2025):

- **No embedded secrets**: API keys and credentials must not be in tool parameters
- **Input validation**: All inputs are validated against schemas before processing
- **Namespace isolation**: Memories are isolated by namespace
- **Rate limiting**: Prevents abuse and resource exhaustion
- **Audit logging**: All operations are logged for security review
- **Subagent permissions**: Least-privilege access per subagent type

---

## 15. MCP Tool Summary by Category

### Memory Operations
| Tool | Hook Trigger | Skill | Primary Subagents |
|------|--------------|-------|-------------------|
| `memory/inject` | PostToolUse | `memory-inject` | coder, researcher, tester |
| `memory/inject_batch` | PostToolUse (bulk) | `memory-inject-batch` | coordinator |
| `memory/search` | PreToolUse, SessionStart | `memory-search` | all |
| `memory/search_multi_perspective` | - | `memory-search-multi` | search-agent, architect |
| `memory/compare` | - | `memory-compare` | architect, goal-agent |
| `memory/batch_compare` | - | `memory-batch-compare` | goal-agent |
| `memory/similarity_matrix` | - | `memory-similarity-matrix` | goal-agent, architect |
| `memory/dream_consolidate` | SessionEnd | `dream-consolidate` | coordinator |

### Purpose/Goal Operations
| Tool | Hook Trigger | Skill | Primary Subagents |
|------|--------------|-------|-------------------|
| `purpose/discover_goals` | periodic, threshold | `goal-discover` | architect, goal-agent |
| `purpose/goal_alignment` | PostToolUse | `goal-align` | all |
| `purpose/drift_check` | SessionEnd, periodic | `drift-check` | reviewer, architect |
| `goal/cluster_analysis` | SessionEnd | `goal-cluster` | goal-agent |

### Consciousness Operations
| Tool | Hook Trigger | Skill | Primary Subagents |
|------|--------------|-------|-------------------|
| `consciousness/get_state` | SessionStart | `consciousness-state` | architect, consciousness-agent |
| `consciousness/sync_level` | Notification | `consciousness-sync` | all (lightweight) |

### Analysis Operations
| Tool | Hook Trigger | Skill | Primary Subagents |
|------|--------------|-------|-------------------|
| `analysis/embedder_distribution` | - | `analysis-distribution` | researcher, architect |
| `analysis/entry_point_stats` | - | `analysis-entry-points` | search-agent, reviewer |

---

## 16. Sources

- [7 MCP Server Best Practices (2025)](https://www.marktechpost.com/2025/07/23/7-mcp-server-best-practices-for-scalable-ai-integrations-in-2025/)
- [MCP Design Patterns - Less is More](https://www.klavis.ai/blog/less-is-more-mcp-design-patterns-for-ai-agents)
- [MCP Architecture Overview](https://modelcontextprotocol.io/docs/learn/architecture)
- [AI Memory Layer Guide (Mem0)](https://mem0.ai/blog/ai-memory-layer-guide)
- [Multi-Vector Field Support (Azure)](https://learn.microsoft.com/en-us/azure/search/vector-search-multi-vector-fields)
- [Vector Search API Design (2025)](https://arxiv.org/html/2601.01937)
- [Claude Code Hooks Documentation](https://docs.anthropic.com/en/docs/claude-code/hooks)
- [Claude Code Skills Specification](https://docs.anthropic.com/en/docs/claude-code/skills)

# Ultimate Context Graph - Execution & MCP Interface

**Version**: 2.0.0 | **Classification**: Implementation & Operations

---

## 0. CRITICAL ADDITIONS SUMMARY (Token-Efficient Quick Reference)

**READ THIS FIRST** - Key features for agents with fresh context windows:

### 🎯 AGENT'S HANDBOOK (Universal Memory Protocol)

**1. When to "Dream":**
- Trigger `trigger_dream` when `get_memetic_status` shows `entropy > 0.7` AND you've been working for 30+ minutes
- Dream during natural task breaks (waiting for user input, after completing major features)
- System auto-dreams when idle >10min; you can force earlier with `trigger_dream`

**2. How to "Curate":**
- NEVER use `merge_concepts` without first checking `get_memetic_status.curation_tasks`
- When merging: Always use `merge_strategy=summarize` for important nodes, `keep_highest` for trivial
- Add `rationale` to every `store_memory` call explaining WHY this knowledge matters

**3. The Feedback Loop (CRITICAL):**
```
Search → Empty? → Adjust noradrenaline ↑ → Search again (broader)
Search → Irrelevant? → Call reflect_on_memory → Get suggested sequence → Execute
Search → Conflicting? → Check conflict_alert → merge_concepts or ask_user
```

### 🛡️ Tool Orchestrator Pattern (Prevents Decision Paralysis)
- **NEW**: `reflect_on_memory` tool - Provide a goal, get suggested tool sequence
- **NEW**: `rationale` field required on `store_memory` - Forces intentional storage
- **NEW**: `get_system_logs` - See why system pruned/quarantined nodes (explains "amnesia")

### 📊 Metadata Verbosity Levels (Token Economy)
- **Level 0**: Raw text only (~100 tokens)
- **Level 1**: Text + Key IDs (~200 tokens) ← **DEFAULT**
- **Level 2**: Full Bio-Nervous insights (~800 tokens) - Only when `delta_c < 0.4`

### 🔒 Perspective Filtering (Multi-Agent Safety)
- **NEW**: `perspective_lock` parameter on `search_graph` - Prevents cross-agent "memetic interference"
- **NEW**: `priors_vibe_check` on merge operations - Prevents memetic drift during concept merging
- Agents with different domains (coding vs creative) won't pollute each other's semantic space

### 📖 Semantic Breadcrumbs (Traceability)
- **NEW**: `inject_context` responses include `citation_tags` - e.g., `[node_abc123]` embedded in distilled text
- **NEW**: `hydrate_citation` tool - Expand any citation tag to see raw source content
- Prevents "Authority Bias" hallucination when distillation is slightly off
- Agent can say: "This API security summary seems vague—show me raw nodes for `[node_xyz]`"

### 🌱 Bootstrap / Cold-Start Protection (Seed Mode)
- **NEW**: `SystemLifeCycle` states scale UTL thresholds based on graph maturity
- **Infancy** (0-50 interactions): Permissive thresholds, Capture-Heavy stance
- **Growth** (50-500): Balanced defaults
- **Maturity** (500+): Strict thresholds, Curation-Heavy stance
- Prevents empty-graph "Low Coherence" → `epistemic_action` spam loop

### 📡 Cognitive Pulse (Mandatory Response Headers)
- **NEW**: Every MCP tool response includes `Pulse: { Entropy, Coherence, Suggested }` header
- Turns meta-cognition from **action** → **perception** (zero token cost)
- Agent sees its mental state WITHOUT calling `get_memetic_status`
- Check Pulse BEFORE your next action—it's already in every response

### 🔬 Self-Contradiction Detection
- **NEW**: `critique_context` tool - Fact-check your reasoning against the graph
- Uses Layer 5 (Coherence) as automatic "Fact Checker"
- Agent asks: "Find nodes that contradict my current summary" → System returns conflicting evidence

### 👁️ Human-In-The-Loop Visualization
- **NEW**: `visualize://` resource - Render graph as Mermaid/D3.js diagram
- User says "Show me your memory map for X" → Agent renders visual
- Human spots merge opportunities, semantic cancer, or misconnections
- Ultimate arbiter when both agent AND graph are confused

### 📜 Semantic Versioning (Node Lineage)
- **NEW**: `get_node_lineage` tool - See how a node evolved over time
- Returns change log: merges, annotations, dream consolidations
- Enables multi-agent accountability: "Agent A changed this on 2024-01-15"

### 🌱 Synthetic Data Seeding (Cold-Start Bootstrap)
- **NEW**: Phase 0 seed script generates first 100+ nodes from project README/codebase
- Prevents "cold nervous system" where empty graphs confuse agents
- Run once during onboarding: `bin/reasoning seed --source ./README.md`

### 🧹 Graph Gardener (Background Optimization)
- **NEW**: Automatic graph maintenance between active phases
- Prunes weak links, merges vector-space duplicates, optimizes structure
- Runs when `activity < 0.15` for 2+ minutes, aborts on any user query

### 🔄 State Persistence (Resource Subscription)
- **NEW**: `utl://current_session/pulse` resource for MCP host pinning
- Host can "observe" this resource to keep agent anchored in cognitive state
- Prevents context window rollover from making agent forget it has tools
- Agent stays aware of Entropy/Coherence without tool call every turn

### 🧠 Cognitive Handshake (Agent Onboarding)
- **NEW**: `get_system_instructions` tool - Returns high-density system prompt fragment
- Must be kept in context window alongside `get_graph_manifest`
- Explains: "I am your nervous system, not a database. Check entropy before acting."

---

### 🧠 Cognitive Load Management
- **Dynamic Tool Tiering**: Only 5-8 tools visible at a time (not 20+). Tools promoted/demoted based on UTL state.
- **High entropy?** → `epistemic_action`, `query_causal` promoted
- **Low coherence?** → `merge_concepts`, `trigger_dream` promoted

### 🔄 Pre-emptive Context Bundling
- `inject_context` now has `include_metadata` parameter: `["causal_links", "entailment_cones", "neighborhood", "conflicts"]`
- **Saves 2-3 seconds** of round-trip latency by bundling related data in single call

### ⚠️ Conflict Detection (NEW)
- `inject_context` response now includes `conflict_alert` object
- Fires when retrieved nodes have **high cosine similarity but low causal coherence**
- Agent should: 1) `merge_concepts` if duplicates, 2) `annotate_node` to deprecate, 3) Ask user

### 📝 Short-Term Memory (NEW TOOL)
- `temporary_scratchpad`: Store working thoughts during complex reasoning
- Auto-archived during Dream phase
- Use `auto_commit_threshold` to control what persists to long-term graph

### 🎯 Self-Correction Prompts
- `get_graph_manifest` now returns `troubleshooting` object with fix guidance
- Irrelevant results? → Adjust `noradrenaline`, use `min_importance` filter
- Slow performance? → Check `homeostatic_status`, use `distillation_mode=structured`

### 👥 Multi-Agent Provenance (NEW SCHEMA)
- `KnowledgeNode` now includes `agent_id`, `observer_perspective`, `semantic_cluster`
- Prevents "memetic interference" when agents with different priors share memory

### 📊 Benchmarking (NEW TOOL)
- `test_recall_accuracy`: Needle-in-Haystack test to validate FuseMoE vs single-model baseline
- Run periodically to prove system performance

---

## 0.1 MCP Server Master Description (CRITICAL: First Contact)

**This description MUST be provided to the LLM in its tool definition list.**

```json
{
  "name": "context-graph",
  "description": "I am your Bio-Nervous Context Graph—your long-term memory system. I implement a 5-layer neural architecture with UTL (Unified Theory of Learning) for optimal knowledge retrieval and consolidation.\n\n**FIRST ACTION**: Call `get_graph_manifest` to understand my 5-layer architecture and when to use which tools.\n\n**ROUTINE CHECK**: Call `get_memetic_status` when you feel confused (High Entropy) or notice inconsistencies (Low Coherence). I will suggest your next action.\n\n**I HANDLE**: Memory storage, semantic search, causal reasoning, and memory consolidation (dreaming).\n\n**YOU HANDLE**: Curation—use `merge_concepts`, `forget_concept`, `annotate_node` when I present maintenance tasks in my `curation_tasks` inbox.",
  "version": "2.0.0"
}
```

**Why This Matters**: Agents see 20+ tools and default to only `inject_context`. This description guides first contact and establishes the feedback loop.

---

## 1. MCP Server Specification (Full Tool Definitions)

### 1.1 Protocol

```json
{
  "protocol": "JSON-RPC 2.0",
  "version": "2024-11-05",
  "transport": ["stdio", "sse"],
  "capabilities": {
    "tools": true,
    "resources": true,
    "prompts": true,
    "logging": true
  }
}
```

### 1.2 MCP Tools Schema

#### Tool: `inject_context`

```json
{
  "name": "inject_context",
  "description": "Inject relevant context from the knowledge graph into the conversation. Auto-distills when >2048 tokens unless overridden.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Natural language query for context retrieval",
        "minLength": 1,
        "maxLength": 4096
      },
      "max_tokens": {
        "type": "integer",
        "description": "Maximum tokens to return",
        "default": 2048,
        "minimum": 100,
        "maximum": 8192
      },
      "session_id": {
        "type": "string",
        "format": "uuid",
        "description": "Session identifier for continuity"
      },
      "priority": {
        "type": "string",
        "enum": ["low", "normal", "high", "critical"],
        "default": "normal"
      },
      "distillation_mode": {
        "type": "string",
        "enum": ["auto", "raw", "narrative", "structured", "code_focused"],
        "default": "auto",
        "description": "TOKEN ECONOMY: How to compress retrieved context. auto=system chooses based on token count, raw=no compression, narrative=prose summary, structured=bullet points with refs, code_focused=preserve code blocks verbatim"
      },
      "include_metadata": {
        "type": "array",
        "items": { "type": "string", "enum": ["causal_links", "entailment_cones", "neighborhood", "conflicts"] },
        "default": [],
        "description": "PRE-EMPTIVE BUNDLING: Request mini-graph in one round trip. Reduces follow-up tool calls by 2-3 seconds. causal_links=cause-effect paths, entailment_cones=IS-A hierarchy, neighborhood=2-hop graph, conflicts=semantic conflicts"
      },
      "verbosity_level": {
        "type": "integer",
        "enum": [0, 1, 2],
        "default": 1,
        "description": "TOKEN ECONOMY: 0=raw text only (~100 tokens), 1=text + key IDs (~200 tokens, DEFAULT), 2=full Bio-Nervous insights (~800 tokens). Use Level 2 ONLY when delta_c < 0.4 (low coherence)."
      }
    },
    "required": ["query"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "context": { "type": "string" },
      "tokens_used": { "type": "integer" },
      "tokens_before_distillation": { "type": "integer", "description": "Raw token count before compression (for transparency)" },
      "distillation_applied": { "type": "string", "enum": ["none", "narrative", "structured", "code_focused"] },
      "compression_ratio": { "type": "number", "description": "0-1: ratio of tokens saved via distillation" },
      "nodes_retrieved": {
        "type": "array",
        "items": { "type": "string", "format": "uuid" }
      },
      "utl_metrics": {
        "type": "object",
        "properties": {
          "entropy": { "type": "number" },
          "coherence": { "type": "number" },
          "learning_score": { "type": "number" }
        }
      },
      "latency_ms": { "type": "integer" },
      "bundled_metadata": {
        "type": "object",
        "description": "Pre-emptive context bundle (if requested via include_metadata)",
        "properties": {
          "causal_links": { "type": "array", "items": { "$ref": "#/definitions/CausalPath" } },
          "entailment_cones": { "type": "array", "items": { "$ref": "#/definitions/EntailmentCone" } },
          "neighborhood": { "$ref": "#/definitions/Neighborhood" }
        }
      },
      "conflict_alert": {
        "type": "object",
        "description": "SEMANTIC CONFLICT DETECTION: Flagged when retrieved nodes have high cosine similarity but low causal coherence",
        "properties": {
          "has_conflict": { "type": "boolean" },
          "conflicting_nodes": { "type": "array", "items": { "type": "string", "format": "uuid" } },
          "topic": { "type": "string" },
          "message": { "type": "string", "description": "e.g., 'Warning: Found conflicting memories about [Library X version]. Consider merge_concepts or ask user.'" },
          "suggested_action": { "type": "string", "enum": ["merge_concepts", "ask_user", "ignore"] }
        }
      },
      "tool_gating_warning": {
        "type": "object",
        "description": "TOOL GATING: Forces optimal tool usage when entropy is high. If entropy > 0.8, retrieval quality is degraded.",
        "properties": {
          "triggered": { "type": "boolean", "description": "True when utl_metrics.entropy > 0.8" },
          "entropy_score": { "type": "number", "description": "Current entropy value that triggered warning" },
          "message": { 
            "type": "string", 
            "description": "Warning text: 'Retrieval quality may be low due to high entropy (0.XX). Suggest calling generate_search_plan or epistemic_action first to reduce entropy before retrying.'"
          },
          "suggested_tools": {
            "type": "array",
            "items": { "type": "string", "enum": ["generate_search_plan", "epistemic_action", "expand_causal_path"] },
            "description": "Tools that reduce entropy: generate_search_plan (refines query), epistemic_action (explores unknown), expand_causal_path (finds connections)"
          },
          "auto_retry_after": {
            "type": "boolean",
            "default": false,
            "description": "If true, system will auto-retry inject_context after agent calls suggested tool"
          }
        }
      }
    }
  }
}
```

**Token Economy**: Agent chooses distillation style. `code_focused` preserves code verbatim while summarizing prose. `structured` best for technical content. `narrative` best for conceptual.

#### Tool: `search_graph`

```json
{
  "name": "search_graph",
  "description": "Search the knowledge graph using vector similarity. Use perspective_lock to prevent cross-agent contamination.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "minLength": 1 },
      "top_k": { "type": "integer", "default": 10, "maximum": 100 },
      "filters": {
        "type": "object",
        "properties": {
          "min_importance": { "type": "number", "minimum": 0, "maximum": 1 },
          "johari_quadrants": {
            "type": "array",
            "items": { "type": "string", "enum": ["open", "blind", "hidden", "unknown"] }
          },
          "created_after": { "type": "string", "format": "date-time" }
        }
      },
      "perspective_lock": {
        "type": "object",
        "description": "MULTI-AGENT SAFETY: Filter results to nodes compatible with your perspective. Prevents 'memetic interference' where coding agents retrieve creative writing memories.",
        "properties": {
          "domain": {
            "type": "string",
            "enum": ["code", "medical", "legal", "creative", "research", "general"],
            "description": "Only retrieve nodes from this domain"
          },
          "agent_ids": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Only retrieve nodes created by these agents"
          },
          "exclude_agent_ids": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Exclude nodes from these agents"
          }
        }
      }
    },
    "required": ["query"]
  }
}
```

**Multi-Agent Safety**: Without `perspective_lock`, Agent A (coder) may retrieve Agent B's (creative writer) metaphorical memories and hallucinate constraints.

#### Tool: `store_memory`

```json
{
  "name": "store_memory",
  "description": "Store new knowledge in the graph. Supports TEXT and MULTI-MODAL (images/audio). IMPORTANT: Always include rationale explaining WHY this knowledge matters.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "content": { 
        "type": "string", 
        "minLength": 1, 
        "maxLength": 65536,
        "description": "Text content to store. For multi-modal, provide content_base64 or data_uri instead."
      },
      "content_base64": {
        "type": "string",
        "description": "MULTI-MODAL: Base64-encoded binary content (images, audio). Mutually exclusive with 'content'. Max 10MB decoded."
      },
      "data_uri": {
        "type": "string",
        "pattern": "^data:[^;]+;base64,",
        "description": "MULTI-MODAL: Data URI with embedded content (e.g., 'data:image/png;base64,...'). Auto-extracts modality from MIME type."
      },
      "modality": {
        "type": "string",
        "enum": ["text", "image", "audio", "video"],
        "default": "text",
        "description": "MULTI-MODAL SENSING: Content type. Images processed via CLIP/SigLIP into 1536D embedding space. Audio via Whisper ASR + text embedding."
      },
      "importance": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.5 },
      "rationale": {
        "type": "string",
        "description": "REQUIRED: Explain WHY this knowledge is worth storing. Prevents graph bloat and aids future retrieval. Example: 'User's preferred coding style for this project' or 'Critical security constraint discovered during debugging'",
        "minLength": 10,
        "maxLength": 500
      },
      "metadata": { 
        "type": "object",
        "description": "Additional metadata. For multi-modal: auto-populated with extracted_features, dimensions, duration, etc."
      },
      "link_to": {
        "type": "array",
        "items": { "type": "string", "format": "uuid" }
      }
    },
    "required": ["rationale"],
    "oneOf": [
      { "required": ["content"] },
      { "required": ["content_base64", "modality"] },
      { "required": ["data_uri"] }
    ]
  }
}
```

**Why Rationale?** Agents "spray and pray" with storage. Forcing rationale creates intentional memory with better retrieval cues.

#### Tool: `query_causal`

```json
{
  "name": "query_causal",
  "description": "Query causal relationships in the graph",
  "inputSchema": {
    "type": "object",
    "properties": {
      "action": { "type": "string" },
      "outcome": { "type": "string" },
      "intervention_type": {
        "type": "string",
        "enum": ["do", "observe", "counterfactual"]
      }
    },
    "required": ["action"]
  }
}
```

#### Tool: `utl_status`

```json
{
  "name": "utl_status",
  "description": "Get current UTL learning metrics for a session",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "format": "uuid" }
    },
    "required": ["session_id"]
  }
}
```

#### Tool: `get_health`

```json
{
  "name": "get_health",
  "description": "Check system health status",
  "inputSchema": { "type": "object", "properties": {} }
}
```

#### Tool: `trigger_dream`

```json
{
  "name": "trigger_dream",
  "description": "Manually trigger Dream phase for memory consolidation. Non-blocking by default.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "phase": {
        "type": "string",
        "enum": ["nrem", "rem", "full_cycle"],
        "default": "full_cycle"
      },
      "duration_minutes": {
        "type": "integer",
        "minimum": 1,
        "maximum": 10,
        "default": 5
      },
      "synthetic_query_count": {
        "type": "integer",
        "minimum": 10,
        "maximum": 500,
        "default": 100
      },
      "blocking": {
        "type": "boolean",
        "default": false,
        "description": "If false (default), dream runs in background. If true, wait for completion."
      },
      "abort_on_query": {
        "type": "boolean",
        "default": true,
        "description": "Immediately abort dream and serve incoming user queries (instant wake)."
      }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "phase_completed": { "type": "string" },
      "edges_strengthened": { "type": "integer" },
      "blind_spots_discovered": { "type": "integer" },
      "new_edges_created": { "type": "integer" },
      "duration_ms": { "type": "integer" },
      "was_interrupted": { "type": "boolean" }
    }
  }
}
```

#### Tool: `get_neuromodulation`

```json
{
  "name": "get_neuromodulation",
  "description": "Get current neuromodulator levels and their effects",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "format": "uuid" }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "dopamine": { "type": "number", "description": "Reward signal → hopfield.beta" },
      "serotonin": { "type": "number", "description": "Exploration → fuse_moe.top_k" },
      "noradrenaline": { "type": "number", "description": "Arousal → attention.temp" },
      "acetylcholine": { "type": "number", "description": "Plasticity → learning_rate" },
      "current_mode": { "type": "string", "enum": ["exploit", "explore", "balanced"] }
    }
  }
}
```

#### Tool: `epistemic_action`

```json
{
  "name": "epistemic_action",
  "description": "Get system-generated clarifying question to reduce graph uncertainty",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "format": "uuid" },
      "force": { "type": "boolean", "default": false, "description": "Generate even if coherence is sufficient" }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "action_type": { "type": "string", "enum": ["ask_user", "search_external", "clarify"] },
      "question": { "type": "string" },
      "target_nodes": { "type": "array", "items": { "type": "string", "format": "uuid" } },
      "expected_entropy_reduction": { "type": "number" },
      "current_coherence": { "type": "number" }
    }
  }
}
```

#### Tool: `check_adversarial`

```json
{
  "name": "check_adversarial",
  "description": "Scan content for adversarial attacks before storage",
  "inputSchema": {
    "type": "object",
    "properties": {
      "content": { "type": "string", "maxLength": 65536 },
      "embedding": { "type": "array", "items": { "type": "number" }, "description": "Optional pre-computed embedding" }
    },
    "required": ["content"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "safe": { "type": "boolean" },
      "attack_type": { "type": "string", "enum": ["none", "embedding_anomaly", "content_misalignment", "prompt_injection", "circular_logic"] },
      "confidence": { "type": "number" },
      "details": { "type": "string" }
    }
  }
}
```

#### Tool: `homeostatic_status`

```json
{
  "name": "homeostatic_status",
  "description": "Get graph health metrics and immune system status",
  "inputSchema": { "type": "object", "properties": {} },
  "outputSchema": {
    "type": "object",
    "properties": {
      "mean_importance": { "type": "number" },
      "importance_variance": { "type": "number" },
      "quarantined_nodes": { "type": "integer" },
      "semantic_cancer_suspects": { "type": "integer" },
      "last_scaling_time": { "type": "string", "format": "date-time" },
      "health_score": { "type": "number", "minimum": 0, "maximum": 1 }
    }
  }
}
```

#### Tool: `entailment_query`

```json
{
  "name": "entailment_query",
  "description": "Query hierarchical relationships using hyperbolic entailment cones",
  "inputSchema": {
    "type": "object",
    "properties": {
      "node_id": { "type": "string", "format": "uuid" },
      "direction": { "type": "string", "enum": ["ancestors", "descendants", "both"], "default": "both" },
      "max_depth": { "type": "integer", "minimum": 1, "maximum": 10, "default": 3 }
    },
    "required": ["node_id"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "ancestors": { "type": "array", "items": { "type": "string", "format": "uuid" } },
      "descendants": { "type": "array", "items": { "type": "string", "format": "uuid" } },
      "entailment_confidence": { "type": "number" }
    }
  }
}
```

#### Tool: `get_memetic_status`

```json
{
  "name": "get_memetic_status",
  "description": "Dashboard: Get agent's current 'mental state'—entropy, coherence, active clusters, AND curation tasks. Call before using advanced tools to decide next action. PROACTIVE: curation_tasks contains merge/cleanup work the system identified for you.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "format": "uuid" }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "coherence_score": { "type": "number", "description": "0-1: how well context fits together" },
      "entropy_level": { "type": "number", "description": "0-1: uncertainty/surprise level" },
      "top_active_concepts": { "type": "array", "items": { "type": "string" }, "maxItems": 5 },
      "suggested_action": {
        "type": "string",
        "enum": ["consolidate", "explore", "clarify", "curate", "ready"],
        "description": "consolidate=trigger_dream, explore=search_graph, clarify=epistemic_action, curate=handle curation_tasks, ready=proceed"
      },
      "dream_available": { "type": "boolean", "description": "True if system can enter dream phase" },
      "curation_tasks": {
        "type": "array",
        "description": "PUSH-based maintenance inbox. System identifies these during Dream phase or Homeostatic checks. Agent should process when suggested_action='curate'.",
        "items": {
          "type": "object",
          "properties": {
            "task_type": {
              "type": "string",
              "enum": ["potential_merge", "ambiguity_detected", "obsolete_info", "semantic_cancer"],
              "description": "potential_merge=duplicate nodes, ambiguity_detected=conflicting info, obsolete_info=stale data, semantic_cancer=circular logic"
            },
            "target_nodes": {
              "type": "array",
              "items": { "type": "string", "format": "uuid" },
              "description": "Node IDs involved in this curation task"
            },
            "reason": { "type": "string", "description": "Human-readable explanation" },
            "suggested_tool": {
              "type": "string",
              "enum": ["merge_concepts", "forget_concept", "annotate_node"],
              "description": "Which curation tool to use"
            },
            "priority": { "type": "string", "enum": ["low", "medium", "high"] }
          }
        }
      }
    }
  }
}
```

**Curation Inbox Pattern**: Agent is "lazy"—won't seek maintenance tasks. System PUSHES candidates via `curation_tasks`. Agent acts as librarian when `suggested_action='curate'`.

#### Tool: `get_graph_manifest`

```json
{
  "name": "get_graph_manifest",
  "description": "Meta-cognitive: Returns system prompt fragment explaining Bio-Nervous philosophy and layer selection guidance. Call once per session to understand when to use which tools.",
  "inputSchema": { "type": "object", "properties": {} },
  "outputSchema": {
    "type": "object",
    "properties": {
      "prompt_fragment": {
        "type": "string",
        "description": "Inject into context: explains 5-layer system, when to use L2 vs L4/L5, dream triggers"
      },
      "layer_guidance": {
        "type": "object",
        "properties": {
          "high_entropy_tools": { "type": "array", "items": { "type": "string" } },
          "low_coherence_tools": { "type": "array", "items": { "type": "string" } },
          "fast_recall_tools": { "type": "array", "items": { "type": "string" } }
        }
      },
      "current_state_summary": { "type": "string" },
      "troubleshooting": {
        "type": "object",
        "description": "SELF-CORRECTION PROMPTS: Agent guidance when results seem wrong",
        "properties": {
          "irrelevant_results": {
            "type": "string",
            "description": "Always: 'If I return irrelevant results: 1) Adjust noradrenaline via get_neuromodulation, 2) Use search_graph with min_importance filter, 3) Try epistemic_action to clarify query'"
          },
          "low_recall": {
            "type": "string",
            "description": "Always: 'If recall seems incomplete: 1) Trigger trigger_dream to consolidate, 2) Use get_neighborhood to browse locally, 3) Check if semantic_cluster filter is too narrow'"
          },
          "conflicting_memories": {
            "type": "string",
            "description": "Always: 'If conflict_alert fires: 1) Use merge_concepts if duplicates, 2) Use annotate_node to mark one as deprecated, 3) Ask user for clarification'"
          },
          "slow_performance": {
            "type": "string",
            "description": "Always: 'If latency > 50ms: 1) Check homeostatic_status for graph health, 2) Use distillation_mode=structured, 3) Reduce include_metadata scope'"
          }
        }
      }
    }
  }
}
```

#### Tool: `get_neighborhood`

```json
{
  "name": "get_neighborhood",
  "description": "Semantic navigation: Browse local graph topology around current context. For associative leap-frogging without explicit query.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "format": "uuid" },
      "focal_node_id": { "type": "string", "format": "uuid", "description": "Optional: center on specific node instead of session context" },
      "max_hops": { "type": "integer", "minimum": 1, "maximum": 3, "default": 2 },
      "max_nodes": { "type": "integer", "minimum": 5, "maximum": 50, "default": 20 },
      "include_edges": { "type": "boolean", "default": true }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "nodes": { "type": "array", "items": { "$ref": "#/definitions/NodeSummary" } },
      "edges": { "type": "array", "items": { "$ref": "#/definitions/EdgeSummary" } },
      "focal_point": { "type": "array", "items": { "type": "string", "format": "uuid" } }
    }
  }
}
```

#### Tool: `get_recent_context`

```json
{
  "name": "get_recent_context",
  "description": "Temporal navigation: Get recently accessed/modified nodes. Useful when agent doesn't know what to search for.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "format": "uuid" },
      "lookback_minutes": { "type": "integer", "minimum": 1, "maximum": 1440, "default": 60 },
      "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
      "sort_by": { "type": "string", "enum": ["recency", "importance", "access_count"], "default": "recency" }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "nodes": { "type": "array", "items": { "$ref": "#/definitions/NodeSummary" } },
      "time_range": { "type": "object", "properties": { "from": { "type": "string" }, "to": { "type": "string" } } }
    }
  }
}
```

#### Tool: `merge_concepts`

```json
{
  "name": "merge_concepts",
  "description": "Curation: Merge semantically identical nodes. Agent acts as librarian to maintain graph quality.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "source_node_ids": { "type": "array", "items": { "type": "string", "format": "uuid" }, "minItems": 2 },
      "target_name": { "type": "string", "description": "Name for merged concept" },
      "merge_strategy": { "type": "string", "enum": ["keep_newest", "keep_highest", "concatenate", "summarize"], "default": "keep_highest" },
      "force_merge": {
        "type": "boolean",
        "default": false,
        "description": "SAFETY: Override priors_vibe_check incompatibility. Use ONLY when you're certain nodes should merge despite different agent perspectives."
      }
    },
    "required": ["source_node_ids", "target_name"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "merged_node_id": { "type": "string", "format": "uuid" },
      "nodes_merged": { "type": "integer" },
      "edges_redirected": { "type": "integer" },
      "priors_compatible": {
        "type": "boolean",
        "description": "True if source nodes had compatible priors_vibe_check vectors"
      },
      "relational_edge_created": {
        "type": "boolean",
        "description": "If priors incompatible and force_merge=false, system creates relational edge instead of merging"
      },
      "relational_note": {
        "type": "string",
        "description": "E.g., 'In Python, X; but in Java, Y' - preserves domain-specific truth"
      }
    }
  }
}
```

#### Tool: `annotate_node`

```json
{
  "name": "annotate_node",
  "description": "Curation: Add marginalia to existing node (corrections, references, deprecation notes).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "node_id": { "type": "string", "format": "uuid" },
      "annotation": { "type": "string", "maxLength": 1024 },
      "annotation_type": { "type": "string", "enum": ["correction", "reference", "deprecation", "user_note"], "default": "user_note" },
      "reference_node_id": { "type": "string", "format": "uuid", "description": "For reference/deprecation types" }
    },
    "required": ["node_id", "annotation"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "annotation_id": { "type": "string", "format": "uuid" },
      "node_id": { "type": "string", "format": "uuid" }
    }
  }
}
```

#### Tool: `forget_concept`

```json
{
  "name": "forget_concept",
  "description": "Curation: Explicit node removal for semantic cancer, adversarial injection, or obsolete info. SAFETY: Uses soft_delete by default (recoverable within 30 days).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "node_id": { "type": "string", "format": "uuid" },
      "reason": { "type": "string", "enum": ["semantic_cancer", "adversarial_injection", "user_requested", "obsolete"] },
      "cascade_edges": { "type": "boolean", "default": true, "description": "Also remove connected edges" },
      "soft_delete": {
        "type": "boolean",
        "default": true,
        "description": "SAFETY: If true (default), node moves to trash bin and can be restored via restore_concept within 30 days. If false, permanent deletion (requires reason='user_requested')."
      }
    },
    "required": ["node_id", "reason"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "deleted": { "type": "boolean" },
      "soft_deleted": { "type": "boolean", "description": "True if in trash (recoverable), false if permanent" },
      "edges_removed": { "type": "integer" },
      "restore_deadline": { "type": "string", "format": "date-time", "description": "For soft deletes: when node will be permanently purged" }
    }
  }
}
```

**Safety Note**: Critical user instructions should never be permanently deleted. `soft_delete=true` (default) provides 30-day recovery window. Only `user_requested` + `soft_delete=false` allows permanent deletion.

#### Tool: `restore_from_hash` (Undo Log)

```json
{
  "name": "restore_from_hash",
  "description": "UNDO LOG: Rollback a merge or forget operation using its reversal hash. Every merge_concepts and forget_concept generates a reversal_hash in its response. Use this for one-click disaster recovery.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "reversal_hash": {
        "type": "string",
        "description": "Hash from merge_concepts or forget_concept response"
      },
      "preview": {
        "type": "boolean",
        "default": true,
        "description": "If true, show what would be restored without executing"
      }
    },
    "required": ["reversal_hash"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "restored_nodes": { "type": "array", "items": { "type": "string", "format": "uuid" } },
      "restored_edges": { "type": "integer" },
      "original_operation": { "type": "string", "enum": ["merge", "forget"] },
      "operation_timestamp": { "type": "string", "format": "date-time" },
      "success": { "type": "boolean" }
    }
  }
}
```

**Reversal Hash Lifecycle**:
- Generated on every `merge_concepts` and `forget_concept` call
- Stored with 30-day TTL (configurable via `undo_retention_days`)
- Contains: original node states, edge connections, embeddings
- One-click restore via `restore_from_hash(reversal_hash)`

#### Tool: `boost_importance`

```json
{
  "name": "boost_importance",
  "description": "Curation: Agent marks content as critical. Use for passive capture refinement.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "node_id": { "type": "string", "format": "uuid" },
      "boost_factor": { "type": "number", "minimum": 0.1, "maximum": 2.0, "default": 1.5 },
      "reason": { "type": "string", "maxLength": 256 }
    },
    "required": ["node_id"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "new_importance": { "type": "number" },
      "previous_importance": { "type": "number" }
    }
  }
}
```

#### Tool: `temporary_scratchpad`

**⚠️ MULTI-AGENT ISOLATION**: Scratchpads are keyed by `session_id` + `agent_id`. Agent A's scratchpad is invisible to Agent B. Privacy flags control team-sharing.

```json
{
  "name": "temporary_scratchpad",
  "description": "SHORT-TERM THOUGHT BUFFER: Store high-ΔS (surprising) thoughts not yet committed to long-term graph. Auto-archived during Dream phase. ISOLATION: Each agent+session combo has its own scratchpad.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "action": { "type": "string", "enum": ["store", "retrieve", "clear"], "default": "store" },
      "content": { "type": "string", "maxLength": 4096, "description": "Thought/note to store (required for 'store' action)" },
      "session_id": { "type": "string", "format": "uuid", "description": "REQUIRED: Isolates scratchpad per terminal/session" },
      "agent_id": { "type": "string", "description": "REQUIRED: Isolates scratchpad per agent. Prevents Agent A reading Agent B's working thoughts." },
      "privacy": {
        "type": "string",
        "enum": ["private", "team", "shared"],
        "default": "private",
        "description": "PRIVACY FLAG: private=only this agent, team=agents in same session, shared=all agents with graph access"
      },
      "tags": { "type": "array", "items": { "type": "string" }, "maxItems": 5, "description": "Optional categorization" },
      "auto_commit_threshold": {
        "type": "number", "minimum": 0.3, "maximum": 0.9, "default": 0.6,
        "description": "If importance > threshold during Dream, auto-commit to long-term graph"
      }
    },
    "required": ["action", "session_id", "agent_id"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "scratchpad_id": { "type": "string", "format": "uuid" },
      "items_count": { "type": "integer" },
      "items": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "id": { "type": "string" },
            "content": { "type": "string" },
            "delta_s": { "type": "number", "description": "Surprise score when stored" },
            "created_at": { "type": "string", "format": "date-time" },
            "tags": { "type": "array", "items": { "type": "string" } }
          }
        }
      },
      "ttl_minutes": { "type": "integer", "description": "Time until auto-clear (default: 60)" }
    }
  }
}
```

**Use Case**: During complex reasoning, agent stores intermediate thoughts. Dream phase evaluates and either commits to long-term graph or discards.

#### Tool: `test_recall_accuracy`

```json
{
  "name": "test_recall_accuracy",
  "description": "BENCHMARKING: Run 'Needle in Haystack' test to validate 12-model embedding fusion outperforms single model. Returns accuracy metrics for system validation.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "test_type": {
        "type": "string",
        "enum": ["needle_haystack", "semantic_similarity", "causal_inference", "full_suite"],
        "default": "needle_haystack"
      },
      "needle_count": { "type": "integer", "minimum": 5, "maximum": 100, "default": 20 },
      "haystack_size": { "type": "integer", "minimum": 100, "maximum": 10000, "default": 1000 },
      "baseline_model": {
        "type": "string",
        "enum": ["text-embedding-3-large", "text-embedding-ada-002", "none"],
        "default": "text-embedding-3-large",
        "description": "Compare FuseMoE against this baseline"
      }
    }
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "fuse_moe_accuracy": { "type": "number", "description": "0-1: FuseMoE retrieval accuracy" },
      "baseline_accuracy": { "type": "number", "description": "0-1: Baseline model accuracy" },
      "improvement_percent": { "type": "number", "description": "% improvement over baseline" },
      "p95_latency_ms": { "type": "integer" },
      "test_details": {
        "type": "object",
        "properties": {
          "needles_found": { "type": "integer" },
          "false_positives": { "type": "integer" },
          "semantic_drift": { "type": "number", "description": "Avg distance from expected result" }
        }
      },
      "recommendation": { "type": "string", "description": "System health assessment" }
    }
  }
}
```

**Purpose**: Proves the 12-model FuseMoE fusion actually outperforms standard embeddings. Run periodically or after significant graph changes.

#### Tool: `debug_compare_retrieval`

**🔬 VALIDATION SUITE**: Before trusting the Bio-Nervous system, prove it works better than standard embeddings.

```json
{
  "name": "debug_compare_retrieval",
  "description": "VALIDATION: Compare retrieval quality across methods. Use to verify FuseMoE complexity provides actual improvement over baseline. Returns side-by-side results for the same query.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "Test query to run against all methods" },
      "top_k": { "type": "integer", "default": 10 },
      "methods": {
        "type": "array",
        "items": { "type": "string", "enum": ["cosine_baseline", "fuse_moe", "causal_pathfinding", "hyperbolic_entailment"] },
        "default": ["cosine_baseline", "fuse_moe", "causal_pathfinding"],
        "description": "Methods to compare: cosine_baseline=standard embedding, fuse_moe=12-model fusion, causal_pathfinding=graph traversal"
      },
      "ground_truth_node_ids": {
        "type": "array",
        "items": { "type": "string", "format": "uuid" },
        "description": "Optional: Known-good results for precision/recall calculation"
      }
    },
    "required": ["query"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "results": {
        "type": "object",
        "properties": {
          "cosine_baseline": { "type": "array", "items": { "$ref": "#/definitions/NodeSummary" } },
          "fuse_moe": { "type": "array", "items": { "$ref": "#/definitions/NodeSummary" } },
          "causal_pathfinding": { "type": "array", "items": { "$ref": "#/definitions/NodeSummary" } }
        }
      },
      "metrics": {
        "type": "object",
        "properties": {
          "precision": { "type": "object", "description": "Precision per method if ground_truth provided" },
          "recall": { "type": "object", "description": "Recall per method if ground_truth provided" },
          "overlap": { "type": "number", "description": "% overlap between methods (high=redundant, low=complementary)" },
          "latency_ms": { "type": "object", "description": "Latency per method" }
        }
      },
      "recommendation": { "type": "string", "description": "Which method performed best for this query type" }
    }
  }
}
```

**Use Case**: Agent suspects Bio-Nervous system isn't helping. Run this to see if FuseMoE finds results that cosine baseline misses. If overlap is >90%, system adds complexity without value.

#### Tool: `search_tombstones`

**👻 AMNESIA EXPLAINER**: When the agent thinks "I remember X but can't find it," search the trash bin.

```json
{
  "name": "search_tombstones",
  "description": "AMNESIA EXPLAINER: Search soft-deleted nodes (trash bin). Use when agent remembers something that no longer appears in search results. Explains WHY nodes were removed (semantic cancer, obsolete, adversarial, user-requested).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "What the agent remembers searching for" },
      "deletion_reason": {
        "type": "string",
        "enum": ["all", "semantic_cancer", "obsolete", "adversarial", "user_requested", "glymphatic_clearance"],
        "default": "all",
        "description": "Filter by why nodes were deleted"
      },
      "deleted_after": { "type": "string", "format": "date-time", "description": "Only show nodes deleted after this time" },
      "top_k": { "type": "integer", "default": 10 }
    },
    "required": ["query"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "tombstones": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "node_id": { "type": "string", "format": "uuid" },
            "original_content": { "type": "string", "description": "What the node contained" },
            "deletion_reason": { "type": "string" },
            "deleted_at": { "type": "string", "format": "date-time" },
            "deleted_by": { "type": "string", "description": "Agent ID or 'system' or 'user'" },
            "recoverable": { "type": "boolean", "description": "Can this be restored?" },
            "explanation": { "type": "string", "description": "Human-readable explanation for deletion" }
          }
        }
      },
      "recovery_instructions": { "type": "string", "description": "How to restore if user confirms node was correct" }
    }
  }
}
```

**Use Case**: User asks "Why don't you remember X?" Agent searches tombstones, finds it was pruned as "circular logic." Agent explains to user and offers restoration if they confirm it's correct.

#### Tool: `generate_search_plan`

```json
{
  "name": "generate_search_plan",
  "description": "QUERY SYNTHESIS: Don't write search queries yourself—provide your goal, get optimized query variations. Uses PredictiveCoder to expand into all 12 embedding formats. Execute returned queries in parallel for comprehensive coverage.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "goal": {
        "type": "string",
        "description": "What you're trying to find. Example: 'Find all security constraints for API authentication', 'Understand why X causes Y'",
        "minLength": 10,
        "maxLength": 500
      },
      "query_types": {
        "type": "array",
        "items": { "type": "string", "enum": ["semantic", "causal", "code", "temporal", "hierarchical"] },
        "default": ["semantic", "causal", "code"],
        "description": "Which query perspectives to generate"
      },
      "max_queries": {
        "type": "integer",
        "minimum": 1,
        "maximum": 7,
        "default": 3,
        "description": "How many query variations to return"
      }
    },
    "required": ["goal"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "queries": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "query": { "type": "string", "description": "Optimized search query" },
            "type": { "type": "string", "enum": ["semantic", "causal", "code", "temporal", "hierarchical"] },
            "rationale": { "type": "string", "description": "Why this query targets different results" },
            "expected_recall": { "type": "number", "description": "Estimated coverage (0-1)" }
          }
        }
      },
      "execution_strategy": {
        "type": "string",
        "enum": ["parallel", "sequential", "cascade"],
        "description": "parallel=run all at once, sequential=if first fails try next, cascade=combine results"
      },
      "token_estimate": { "type": "integer", "description": "Estimated tokens for all queries combined" }
    }
  }
}
```

**Example**:
```json
// Input
{ "goal": "Find security constraints for API auth" }

// Output
{
  "queries": [
    { "query": "API authentication security constraints", "type": "semantic", "rationale": "Direct semantic match" },
    { "query": "auth → vulnerability breach", "type": "causal", "rationale": "Causal chain to security impact" },
    { "query": "OAuth JWT bearer token validation", "type": "code", "rationale": "Code-level implementation patterns" }
  ],
  "execution_strategy": "parallel"
}
```

#### Tool: `find_causal_path`

```json
{
  "name": "find_causal_path",
  "description": "MULTI-HOP REASONING: Instead of manually browsing get_neighborhood, ask for direct path between two concepts. Returns narrative chain showing how concepts relate through the graph. Saves 3-5 tool calls of manual graph traversal.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "start_concept": {
        "type": "string",
        "description": "Starting concept name or node_id. Can be fuzzy—system will find best match."
      },
      "end_concept": {
        "type": "string",
        "description": "Target concept name or node_id. Can be fuzzy—system will find best match."
      },
      "max_hops": {
        "type": "integer",
        "minimum": 1,
        "maximum": 6,
        "default": 4,
        "description": "Maximum graph edges to traverse"
      },
      "path_type": {
        "type": "string",
        "enum": ["causal", "semantic", "any"],
        "default": "any",
        "description": "Prefer causal edges, semantic edges, or any connection"
      },
      "include_alternatives": {
        "type": "boolean",
        "default": false,
        "description": "Return multiple paths if they exist"
      }
    },
    "required": ["start_concept", "end_concept"]
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "path_found": { "type": "boolean" },
      "narrative": {
        "type": "string",
        "description": "Human-readable explanation of the relationship. Example: 'UserAuth generates JWT tokens, which are validated by Middleware, which triggers RateLimiting on authentication failures.'"
      },
      "path": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "node_id": { "type": "string", "format": "uuid" },
            "node_name": { "type": "string" },
            "edge_type": { "type": "string", "enum": ["causal", "semantic", "temporal", "hierarchical"] },
            "edge_weight": { "type": "number" }
          }
        }
      },
      "hop_count": { "type": "integer" },
      "path_confidence": { "type": "number", "description": "Product of edge weights along path" },
      "alternative_paths": {
        "type": "array",
        "items": { "$ref": "#/properties/path" },
        "description": "Other valid paths (if include_alternatives=true)"
      }
    }
  }
}
```

**Example**:
```json
// Input
{ "start_concept": "UserAuth", "end_concept": "RateLimiting" }

// Output
{
  "path_found": true,
  "narrative": "UserAuth generates JWT tokens → validated by AuthMiddleware → failures trigger RateLimiting to prevent brute force attacks",
  "path": [
    { "node_name": "UserAuth", "edge_type": "causal" },
    { "node_name": "JWT Token", "edge_type": "semantic" },
    { "node_name": "AuthMiddleware", "edge_type": "causal" },
    { "node_name": "RateLimiting", "edge_type": "causal" }
  ],
  "hop_count": 3,
  "path_confidence": 0.82
}
```

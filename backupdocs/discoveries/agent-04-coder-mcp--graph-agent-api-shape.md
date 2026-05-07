---
namespace: discoveries
agent_id: agent-04-coder-mcp
timestamp: 2026-04-15T08:15:00Z
status: resolved
tags: graph-agent, llm, validate_graph_link, api
---

# Graph-agent exposes single-edge LLM validation via `GraphDiscoveryService::llm()`

## What I expected
A dedicated `validate_graph_link`-style helper on `GraphDiscoveryService`
that runs LLM inference on one `(source_content, target_content)` pair and
returns a structured verdict with confidence + rationale.

## What's actually there
`context_graph_graph_agent::service::GraphDiscoveryService` exposes:

```rust
pub fn llm(&self) -> &Arc<GraphRelationshipLLM>
```

and `GraphRelationshipLLM` provides:

```rust
pub fn is_loaded(&self) -> bool
pub async fn analyze_relationship(
    &self,
    memory_a: &str,
    memory_b: &str,
) -> GraphAgentResult<GraphAnalysisResult>
pub async fn validate_relationship(
    &self,
    memory_a: &str,
    memory_b: &str,
    expected_type: RelationshipType,
) -> GraphAgentResult<(bool, f32, String)>
```

`GraphAnalysisResult` carries `has_connection: bool`, `confidence: f32`,
`description: String`, plus relationship-type/category/direction fields.

## What I did (F4 handler)
`validate_low_confidence_edges` uses `llm().analyze_relationship(...)` (no
expected type) and maps `GraphAnalysisResult` → `LLMVerdict` as:

- `has_connection == true` → `LLMVerdict::Valid`
- `has_connection == false` → `LLMVerdict::Invalid`

`LLMVerdict::Reclassify { new_edge_type }` is reachable only when the
classifier infers a different `GraphLinkEdgeType` — intentionally not wired
in v1. Spec says "do NOT invent an ad-hoc LLM call path", so I avoided
forcing a reclassify path without a clean mapping from `RelationshipType`
(graph-agent's 20-variant enum) to `GraphLinkEdgeType` (8-variant).

## Skipped-status triggers (hard rule: no silent fallbacks)
The handler returns `{status: "skipped", ...}` with explicit reason when:

1. `llm` feature disabled at compile time →
   `reason: "llm feature not enabled"`.
2. `self.graph_discovery_service` is `None` →
   `reason: "graph-agent API not exposed for single-edge validation"`.
3. `llm.is_loaded()` is `false` →
   `reason: "graph-agent LLM not loaded"`.

All three return `status: "skipped"` with a `duration_ms`, not an error.

## Prompt-hash strategy
`LLMEdgeValidation` stores `prompt_hash: [u8; 32]` so a future change in
the prompt template is detectable. The graph-agent builds its prompt
internally, so I hash a stable envelope over the actual inputs:

```
sha256("graph_agent::analyze_relationship::v1\n"
       || len_le_u64(src) || src
       || "\n"
       || len_le_u64(tgt) || tgt)
```

If the graph-agent ever changes its prompt template, bump the envelope tag
(`v1` → `v2`) to surface the drift.

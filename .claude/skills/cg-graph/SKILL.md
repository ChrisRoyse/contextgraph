---
name: cg-graph
description: |
  Navigate the memory graph: explore neighbors, traverse edges, and discover connections.
  Use for understanding relationships between memories and exploring the knowledge graph.
  Keywords: graph, neighbors, traverse, edges, connections, relationships, navigate, network.
allowed-tools: mcp__context-graph__get_memory_neighbors,mcp__context-graph__traverse_graph,mcp__context-graph__get_unified_neighbors,mcp__context-graph__get_typed_edges
model: inherit
user-invocable: true
---

# Graph Navigator

Navigate and explore the Context Graph memory network.

## Tools

- `mcp__context-graph__get_memory_neighbors`: Get immediate neighbors of a memory node
- `mcp__context-graph__traverse_graph`: Walk the graph from a starting node with depth control
- `mcp__context-graph__get_unified_neighbors`: Get neighbors across all embedding spaces
- `mcp__context-graph__get_typed_edges`: Get edges filtered by relationship type

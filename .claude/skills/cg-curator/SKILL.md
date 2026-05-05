---
name: cg-curator
description: |
  Curate memories by merging duplicates, forgetting irrelevant concepts, or boosting importance.
  Use for memory maintenance, deduplication, and knowledge curation.
  Keywords: merge, forget, delete, boost, importance, curate, deduplicate, cleanup.
allowed-tools: mcp__context-graph__merge_concepts,mcp__context-graph__forget_concept,mcp__context-graph__boost_importance
model: inherit
user-invocable: true
---

# Memory Curator

Curate and maintain the memory knowledge base.

## Tools

- `mcp__context-graph__merge_concepts`: Merge duplicate or related memories
- `mcp__context-graph__forget_concept`: Soft-delete irrelevant memories
- `mcp__context-graph__boost_importance`: Adjust memory importance (0.0-1.0)

---
name: cg-topics
description: |
  Explore emergent topic clusters, stability metrics, and divergence alerts.
  Topics emerge from weighted multi-space clustering (threshold >= 2.5).
  Keywords: topics, portfolio, stability, churn, divergence, clusters.
allowed-tools: mcp__context-graph__get_topic_portfolio,mcp__context-graph__get_topic_stability,mcp__context-graph__detect_topics,mcp__context-graph__get_divergence_alerts
model: inherit
user-invocable: true
---

# Topic Explorer

Explore emergent topic clusters in Context Graph.

## Steps

1. **View topics**: `mcp__context-graph__get_topic_portfolio`
2. **Check stability**: `mcp__context-graph__get_topic_stability`
3. **Detect new topics**: `mcp__context-graph__detect_topics`
4. **Check divergence**: `mcp__context-graph__get_divergence_alerts`

---
name: dream-consolidation
description: Trigger memory consolidation via dream phases. NREM replays high-importance patterns using Hebbian learning. REM discovers blind spots via hyperbolic random walk in Poincare ball. Use when entropy > 0.7 AND churn > 0.5. Keywords: dream, consolidate, nrem, rem, blind spots, entropy, churn.
allowed-tools: Read,Glob,Bash
model: sonnet
version: 1.0.0
user-invocable: true
---
# Dream Consolidation

Execute NREM/REM dream consolidation cycles for memory optimization.

## Overview

The dream layer implements neural-inspired memory consolidation through two distinct phases:

1. **NREM Phase**: Hebbian replay strengthening important connections
2. **REM Phase**: Hyperbolic exploration discovering blind spots

## MCP Tools

### trigger_dream

Execute a full or partial dream consolidation cycle.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `blocking` | boolean | true | Wait for completion. If false, returns immediately with dream_id |
| `dry_run` | boolean | false | Simulate without modifying graph |
| `skip_nrem` | boolean | false | Skip NREM phase, run REM only |
| `skip_rem` | boolean | false | Skip REM phase, run NREM only |
| `max_duration_secs` | integer | 300 | Maximum total duration (10-600 seconds) |

**Example:**

```json
{
  "name": "trigger_dream",
  "arguments": {
    "blocking": true,
    "dry_run": false
  }
}
```

**Response:**

```json
{
  "dream_id": "uuid",
  "status": "completed|interrupted|failed|dry_run_complete",
  "nrem_result": {
    "memories_replayed": 42,
    "edges_strengthened": 15,
    "edges_weakened": 8,
    "edges_pruned": 3,
    "avg_weight_delta": 0.023,
    "duration_ms": 180000,
    "completed": true,
    "params": {
      "learning_rate": 0.01,
      "weight_decay": 0.001,
      "weight_floor": 0.05,
      "weight_cap": 1.0,
      "recency_bias": 0.8
    }
  },
  "rem_result": {
    "queries_generated": 87,
    "blind_spots_found": 5,
    "new_edges_created": 5,
    "avg_semantic_leap": 0.82,
    "exploration_coverage": 0.34,
    "unique_positions_visited": 156,
    "duration_ms": 120000,
    "completed": true
  },
  "report": {
    "total_duration_ms": 300000,
    "pre_entropy": 0.75,
    "post_entropy": 0.58,
    "pre_coherence": 0.62,
    "post_coherence": 0.74,
    "dream_recommended": true,
    "recommendations": ["Dream cycle completed successfully"]
  },
  "dry_run": false
}
```

### get_dream_status

Get status of a running or completed dream cycle.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `dream_id` | string (uuid) | null | Dream cycle ID. If omitted, returns most recent |

**Example:**

```json
{
  "name": "get_dream_status",
  "arguments": {
    "dream_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**Response:**

```json
{
  "dream_id": "uuid",
  "status": "nrem_in_progress",
  "progress_percent": 45,
  "current_phase": "nrem",
  "elapsed_ms": 81000,
  "remaining_ms": 219000
}
```

## Dream Phases

### NREM Phase (3 min)

Constitution Reference: Section dream, lines 446-453

**Purpose:** Hebbian learning replay for memory consolidation

**Parameters:**
- Duration: 3 minutes
- Coupling: 0.9 (tight)
- Recency bias: 0.8

**Hebbian Learning:**
```
Delta_w_ij = eta × phi_i × phi_j
```
Where:
- `eta` = 0.01 (learning rate)
- `phi_i`, `phi_j` = activation levels of connected nodes
- Weight decay: 0.001 per cycle
- Weight floor: 0.05 (below = prune)
- Weight cap: 1.0 (maximum)

**Process:**
1. Select recent memories weighted by recency and importance
2. Replay co-activated memory pairs
3. Strengthen connections via Hebbian rule
4. Detect 3+ hop paths for shortcut amortization

### REM Phase (2 min)

Constitution Reference: Section dream, lines 446-453

**Purpose:** Blind spot discovery via hyperbolic exploration

**Parameters:**
- Duration: 2 minutes
- Temperature: 2.0 (high exploration)
- Semantic leap minimum: 0.7
- Query limit: 100 maximum

**Hyperbolic Random Walk:**
- Model: Poincare ball (64 dimensions, curvature -1.0)
- Step size: 0.1
- Max steps per walk: 100

**Process:**
1. Generate synthetic queries via random walk
2. High-temperature softmax for exploration
3. Detect blind spots (semantic distance >= 0.7)
4. Create new connections for discovered blind spots

## Trigger Conditions (AP-70)

Per Constitution AP-70, dream cycles MUST be triggered when:

| Condition | Threshold | Required |
|-----------|-----------|----------|
| Entropy | > 0.7 for 5+ min | Required |
| Churn | > 0.5 | AND with entropy |
| Activity | < 0.15 | Recommended (idle) |
| GPU Usage | < 80% | Constraint |

Check conditions via `get_memetic_status` before triggering:

```json
{
  "name": "get_memetic_status",
  "arguments": {}
}
```

Look for `utl.entropy` and topic stability in response.

## Constraints (Constitution)

| Constraint | Value | Enforcement |
|------------|-------|-------------|
| Query limit | 100 max | Counter in REM phase |
| Wake latency | <100ms | Abort controller timeout |
| GPU budget | <30% | Resource monitor check |
| Semantic leap | ≥0.7 | Filter in blind spot detection |

## Anti-Patterns

Per Constitution:
- **AP-70**: Dream triggers MUST use entropy > 0.7 AND churn > 0.5
- **AP-71**: Dream NREM/REM returning stubs FORBIDDEN
- **AP-72**: nrem.rs/rem.rs TODO stubs MUST be implemented

## Usage Examples

### Trigger Full Dream Cycle

```bash
# Check if dream is recommended
curl -X POST http://localhost:3100 -d '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_memetic_status",
    "arguments": {}
  }
}'

# If entropy > 0.7, trigger dream
curl -X POST http://localhost:3100 -d '{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "trigger_dream",
    "arguments": {"blocking": true}
  }
}'
```

### Dry Run (Simulation)

```bash
curl -X POST http://localhost:3100 -d '{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "trigger_dream",
    "arguments": {"dry_run": true}
  }
}'
```

### NREM Only (Memory Consolidation)

```bash
curl -X POST http://localhost:3100 -d '{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "trigger_dream",
    "arguments": {"skip_rem": true}
  }
}'
```

### REM Only (Exploration)

```bash
curl -X POST http://localhost:3100 -d '{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "trigger_dream",
    "arguments": {"skip_nrem": true}
  }
}'
```

## Related Skills

- `/topic-explorer` - Check topic stability and churn metrics
- `/curation` - Prune or merge concepts after consolidation
- `/memory-inject` - Retrieve context for session restoration

# TASK-PERF-006: Pre-allocate HashMap capacity in hot paths

```xml
<task_spec id="TASK-PERF-006" version="1.0">
<metadata>
  <title>Pre-allocate HashMap capacity in hot paths</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements><requirement_ref>REQ-PERF-006</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>1</estimated_hours>
</metadata>

<context>
HashMap default capacity starts at 0, causing repeated reallocations during growth.
In hot paths (e.g., embedding processing, graph traversal), this adds latency.
Pre-allocating with known capacity eliminates these reallocations.
Constitution: ISS-016 micro-optimization.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-embeddings/src/ (scan for HashMap::new())
- /home/cabdru/contextgraph/crates/context-graph-graph/src/ (scan for HashMap::new())
</input_context_files>

<scope>
<in_scope>
- Identify HashMap::new() in hot paths
- Replace with HashMap::with_capacity() where size is predictable
- Add comments explaining capacity choice
- Focus on: embedding batch processing, graph node collections, MCP response builders
</in_scope>
<out_of_scope>
- HashMaps with unpredictable sizes
- Cold paths (config loading, init)
- Other collection types
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// Example patterns to apply:

// BEFORE
let mut node_map = HashMap::new();
for node in nodes {
    node_map.insert(node.id, node);
}

// AFTER
let mut node_map = HashMap::with_capacity(nodes.len());
for node in nodes {
    node_map.insert(node.id, node);
}

// For known constants
const EXPECTED_TOOLS: usize = 50;
let mut tools = HashMap::with_capacity(EXPECTED_TOOLS);
```
</signatures>
<constraints>
- Capacity MUST be based on known/estimated size
- MUST NOT over-allocate (waste memory)
- MUST add comment explaining capacity choice
- Focus on loops and batch operations
</constraints>
<verification>
```bash
cargo test -p context-graph-embeddings
cargo test -p context-graph-graph
# Benchmark if available
cargo bench --bench hot_path_bench
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- Files containing HashMap::new() in hot paths (identified during implementation)
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-embeddings
cargo check -p context-graph-graph
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Identifying Hot Paths

Look for:
1. Loop bodies that insert into HashMaps
2. Functions called per-request in MCP handlers
3. Embedding batch processing functions
4. Graph traversal code

### Capacity Guidelines

| Context | Suggested Capacity |
|---------|-------------------|
| MCP tools | 50 (known tool count) |
| Batch embeddings | batch_size |
| Graph neighbors | average_degree (e.g., 10) |
| Response metadata | 8-16 (small) |

### Memory vs Performance Tradeoff

Over-allocation wastes memory but avoids reallocations.
Under-allocation may still cause reallocations.

Rule of thumb: Allocate for expected case, accept occasional realloc for outliers.

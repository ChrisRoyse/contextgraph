---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:10:00Z
status: active
tags: error-handling, silent-failure, fail-fast, training-tools
---

# training_tools.rs: source_metadata error silently swallowed (asymmetric with topic-profile path)

## Classification
**DEFECT** — violates the codebase's fail-fast principle
(`constraints.md`: "NO workarounds / fallbacks — error out with diagnostics").

## Severity
**Medium** — no data corruption, but errors from `get_source_metadata` cause:
1. `filter='session'` to silently include memories whose session-id lookup
   failed (treated as mismatch — possibly excludes valid rows).
2. `session_id` / `source_type` / `source_path` to silently appear as `None`
   in the exported record when the RocksDB read failed, indistinguishable
   from "this memory legitimately has no source metadata row".

## Location
- `crates/context-graph-mcp/src/handlers/tools/training_tools.rs:203-207`:
  ```rust
  let source = self
      .teleological_store
      .get_source_metadata(fp.id)
      .await
      .unwrap_or(None);
  ```

## Contrast within the same file

The adjacent topic-profile lookup at `training_tools.rs:225-231` does it
correctly:
```rust
let stored_profile = match rocksdb_store.get_topic_profile(fp.id).await {
    Ok(p) => p,
    Err(e) => {
        warn!(id = %fp.id, error = %e, "get_topic_profile failed; using fallback");
        None
    }
};
```

One lookup logs; one does not. No principled reason for the asymmetry.

Note: The previous code-simplifier review (H-2) flagged the same pattern in
`constellation_tools.rs:125-128` and it was fixed there with a hard error
return. Training-tools missed the same sweep.

## Minimum fix

Mirror the topic-profile pattern in training_tools.rs (log at warn!, fall
through to None):

```rust
let source = match self.teleological_store.get_source_metadata(fp.id).await {
    Ok(m) => m,
    Err(e) => {
        warn!(id = %fp.id, error = %e, "get_source_metadata failed; treating as absent");
        None
    }
};
```

Or, if the codebase leans fail-fast per the plan's "NO workarounds":

```rust
let source = self.teleological_store.get_source_metadata(fp.id).await
    .map_err(|e| format!("get_source_metadata({}) failed: {}", fp.id, e))?;
```

(requires reshaping the for-loop to bail out via `return self.tool_error(...)`
on storage failure, matching the treatment of topic-profile errors in
constellation_tools.rs:129-141).

## Recommendation

Use the fail-fast form. Storage-level failures while exporting a training
corpus are not recoverable — a silent skip means the training data is
incomplete and the caller never learns. The constellation tool already
chose this path (H-2 fix). Training tools should be consistent.

## Rationale for why fail-fast is better here

1. An export-corpus caller is presumably building a dataset; silently
   dropping rows because the DB hiccuped produces training corruption
   downstream.
2. Error-on-first-failure lets the caller retry or diagnose. Silently
   skipping accumulates errors that need a separate `grep logs | wc -l`
   forensic to find.
3. Symmetry with the H-2 fix in constellation_tools.rs.

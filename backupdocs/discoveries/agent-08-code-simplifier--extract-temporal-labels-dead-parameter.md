---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:22:00Z
status: active
tags: dead-parameter, temporal-labels, api-hygiene
---

# extract_temporal_labels carries a `_source_metadata` parameter it never reads

## Classification
**DOCUMENTATION-GAP + API-HYGIENE** — already called out as **M-3** in the
prior simplifier review. Re-flagging because the fix is one line and was
deferred; marking as a quick-win that should land alongside the H-2 fix
batch.

## Severity
**Low** — purely hygienic; no behavior change.

## Location
- `crates/context-graph-core/src/training/temporal.rs:101-107` — parameter
  declared `_source_metadata: Option<&SourceMetadata>`, then never read in
  the function body (lines 108-138).
- `crates/context-graph-mcp/src/handlers/tools/training_tools.rs:482-488`
  — call site dutifully passes `source.as_ref()`.

## Fix

Remove the parameter. One-site call-site update.

```rust
// temporal.rs
pub fn extract_temporal_labels(
    fingerprint: &TeleologicalFingerprint,
    session_sequence: Option<u32>,
    session_total: Option<u32>,
    export_now: DateTime<Utc>,
) -> TemporalLabels {
    // unchanged body
}

// training_tools.rs
Some(extract_temporal_labels(
    fp,
    session_sequence,
    None,
    export_now,
))
```

Also removes the now-pointless `use crate::types::SourceMetadata;` import
from temporal.rs:14.

## Rationale

The docstring at `temporal.rs:96-100` already states the caller resolves
session bookkeeping externally. The parameter is a lingering
"signature-placeholder" from an earlier design that thought the function
would fish session-sequence out of source_metadata itself; the final
design passes session_sequence pre-resolved. One fewer thing for readers
to wonder about.

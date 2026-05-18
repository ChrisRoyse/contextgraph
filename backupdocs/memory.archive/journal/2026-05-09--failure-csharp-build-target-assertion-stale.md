---
namespace: journal
created: 2026-05-09T04:15:30Z
updated: 2026-05-09T04:15:30Z
status: resolved
severity: low
tags: failure,csharp,semantic-index,test-contract,slnx
---

# Failure: C# Build Target Assertion Was Stale

## What Happened

`cargo test -p context-graph-cli --no-default-features --lib csharp -- --nocapture --test-threads=1`
failed in `csharp_build_target_selection_is_unambiguous_and_fail_closed`.

The production error message correctly changed from `.sln or .csproj` to
`.sln, .slnx, or .csproj`, but the test assertion still checked the old text.

## Why It Matters

The failure was useful because it proved the fail-closed missing-target path was
still exercised after adding `.slnx`, and it caught a stale test contract instead
of allowing documentation and assertion text to diverge.

## Resolution

Updated the assertion to require the new `.sln, .slnx, or .csproj` text. Rerun
the same C# semantic-index test command after this fix.

## Related

- `crates/context-graph-cli/src/commands/dynamicjepa.rs`
- `/tmp/contextgraph-csharp-semantic-fsv/csharp-lib-tests.log`

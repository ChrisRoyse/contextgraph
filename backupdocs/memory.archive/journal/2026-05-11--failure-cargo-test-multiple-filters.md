---
namespace: journal
created: 2026-05-11T00:00:00Z
updated: 2026-05-11T00:00:00Z
status: active
tags: cargo, test-command, mcp, verification
---

# Failure: Cargo Test Multiple Filters

## What Happened

I ran:

```bash
cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates mejepa_utml_tools --lib
```

Cargo accepts only one positional test filter before `--`, so it returned `unexpected argument 'mejepa_utml_tools' found`.

## Why It Matters

This was a command-shape failure, not a code failure. The correct verification is to run the filters as separate commands or run the whole crate test suite.

## Related

- Crate: `context-graph-mcp`
- Current task: Phase 8 + UTML hardening verification

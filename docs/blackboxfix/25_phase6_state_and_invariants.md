# Phase 6 — MCP Gateway, Policy, And FSV Tests

This document is the canonical reference for the **shipped** Phase 6
implementation.

## Source Of Truth

| Surface | Physical source of truth |
|---|---|
| Ledger | SQLite file at `--ledger PATH`, table `ledger_records` |
| MCP registry | `tools/list` response from `blackbox mcp recorder|gateway` |
| Policy | `policy_decision` rows in `ledger_records` |
| Gateway mutation | real Git working tree and `.git` object database |
| Gateway evidence | `expected_delta`, `readback_snapshot`, `verifier_result`, and `evidence_packet` rows |

Phase 6 originally bumped the live ledger schema to `LEDGER_SCHEMA_VERSION = 4`.
The current live ledger schema is `LEDGER_SCHEMA_VERSION = 5` after Phase 11.
Older ledgers are wiped on `blackbox init`, never migrated in place.

## Shipped Commands

```text
blackbox mcp recorder --ledger PATH
blackbox mcp gateway --ledger PATH --policy-mode observe_only|supervised|bounded_autonomous|blocked
blackbox policy evaluate --ledger PATH --mode MODE --tool TOOL --capability CAP [...]
blackbox policy registry [--ledger PATH]
blackbox api surface [--ledger PATH]
```

## Shipped MCP Tools

Recorder and gateway expose:

```text
blackbox_start_run
blackbox_record_decision
blackbox_record_tool_call
blackbox_verify_tool_call
blackbox_get_evidence
blackbox_export_episodes
blackbox_policy_evaluate
```

Gateway additionally exposes:

```text
verified_git_apply_patch
```

`verified_git_apply_patch` requires a real Git repo, path, and patch. It
records `agent_step`, `model_decision`, `policy_decision`, and `tool_call`,
applies the patch through Git, then verifies the physical diff with
`git_file_diff_matches_patch_v1`.

## Invariants

- MCP stdout contains only JSON-RPC messages.
- MCP tool schemas reject missing required fields before ledger mutation.
- Every policy decision is persisted as a `policy_decision` row.
- `blocked` mode denies before external mutation and writes no `tool_call`.
- Mutating actions without verifier coverage are denied in governed modes.
- Successful gateway mutation must return evidence and leave matching physical
  Git readback.

## Automated FSV

Run:

```bash
cargo test --package context-graph-cli --test blackbox_phase6_mcp_policy_fsv -- --nocapture
```

Current suite: 4/4 passing. Coverage includes:

- MCP `tools/list` contains `verified_git_apply_patch`;
- real Git patch happy path writes physical diff and rows:
  `agent_run=1`, `agent_step=1`, `model_decision=1`,
  `policy_decision=1`, `tool_call=1`, `expected_delta=1`,
  `readback_snapshot=1`, `verifier_result=1`, `evidence_packet=1`;
- `blocked` policy mode persists denial and leaves Git diff empty;
- missing verifier on a mutating policy request persists denial and writes no
  tool/evidence rows;
- missing MCP patch argument returns `BBX_MCP_SCHEMA_INVALID` before new rows
  or Git changes.

## Manual FSV

Manual FSV log:

```text
/tmp/bbx-phase6-manual-fsv.98fUrJ/fsv.log
```

Happy-path proof:

- source of truth: `/tmp/bbx-phase6-manual-fsv.98fUrJ/happy.sqlite#ledger_records`
  and `/tmp/bbx-phase6-manual-fsv.98fUrJ/happy-repo/.git`;
- before: row count `1`, only `agent_run=1`, Git diff empty;
- after: row count `9`, policy row `allowed`, evidence row `passed`;
- physical Git diff SHA:
  `c53f17a0f04b39466228cf0139f211e029beae6f1fd529b2dd2727a0b255dbb2`,
  matching the expected patch SHA.

Edge cases:

- `blocked` gateway mode wrote `policy_decision=1`, no `tool_call`, no
  `evidence_packet`, and Git diff remained empty.
- missing verifier wrote one denied `policy_decision` with
  `POLICY_VERIFIER_REQUIRED`, no `tool_call`.
- missing MCP `patch` argument returned `BBX_MCP_SCHEMA_INVALID`, row count
  stayed `1`, and Git diff remained empty.

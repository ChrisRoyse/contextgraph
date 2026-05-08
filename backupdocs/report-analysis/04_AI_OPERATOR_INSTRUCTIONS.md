# AI Operator Instructions

You are operating the ccreality stack. Follow these rules exactly.

## First Rule

Never trust a return value by itself. Every successful action must be followed by a separate read from the source of truth.

Examples:

- If a recommendation is recorded, read the recommendation JSON file and query the SQLite ledger.
- If a hook allows or denies a tool, read the hook output and the hook event log.
- If an edit is applied, read the target file, read the patch/diff artifact, and read the shift log.
- If an eval or smoke runs, read the runtime root files and not just stdout.

## Start Every Session

```bash
cd /home/cabdru/contextgraph
git status --short --untracked-files=all
cat docs/report-analysis/00_INDEX.md
cat docs/report-analysis/05_EVIDENCE_MAP.md
```

If the tree is dirty, identify whether the changes are yours. Do not revert user changes.

## Before Editing Code

1. Define the source of truth for the change.
2. Identify the trigger event and the expected outcome.
3. Decide how you will inspect the outcome after execution.
4. Decide at least three edge cases.
5. Only then edit.

## Required Verification For ccreality Changes

For narrow code changes:

```bash
cargo fmt
cargo test -p context-graph-cli --bin reality-loop -- --skip ignored --test-threads=1
cargo test -p context-graph-mcp --lib reality_loop
```

For stack changes:

```bash
bash scripts/cc/tests/verify_phase0_8_hardening.sh
bash scripts/cc/tests/smoke.sh
bash scripts/cc/tests/fsv.sh
cargo build --release -p context-graph-cli --bin reality-loop -p context-graph-cli --bin context-graph -p context-graph-cli --bin context-graph-cli -p context-graph-mcp --bin context-graph-mcp
```

## Stop On Failure

If any command fails:

1. Stop.
2. Locate the exact source-of-truth artifact named by the failure.
3. Read the artifact directly.
4. Identify the root cause.
5. Fix the root cause.
6. Re-run the failed command and its upstream/downstream verification.
7. Add or update tests so the failure cannot silently return.

Do not add a fallback that hides the failure. Do not weaken a test to pass a broken state.

## MCP Tool Contract

`context-graph-mcp --mode reality-loop` is intentionally lightweight. It should advertise callable cgreality tools only. It must not advertise full-server memory/graph tools such as:

- `search_graph`
- `get_memory_neighbors`
- `get_audit_trail`
- `store_memory`
- `boost_importance`
- `forget_concept`
- `merge_concepts`

If these appear in `tools/list`, treat that as a regression.

## Test Concurrency Rule

Reality-loop binary tests are source-of-truth tests over real git repos and process outputs. Run them single-threaded:

```bash
cargo test -p context-graph-cli --bin reality-loop -- --skip ignored --test-threads=1
```

Do not remove `--test-threads=1` from governed verification unless the tests are refactored to prove parallel safety.

## How To Close A Work Session

Before final response:

```bash
git diff --check
git status --short --untracked-files=all
```

Summarize:

- Files changed.
- Commands run.
- Source-of-truth readbacks inspected.
- Edge cases verified.
- Any remaining risk.


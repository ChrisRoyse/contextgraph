# Phase 12 — Tasks

## Task 12.1 — Pick or create the task manifest

If `tasks/swebench-lite/` exists with manifests, pick one:
```bash
ls /home/cabdru/contextgraph/tasks/swebench-lite/ 2>/dev/null | head -5
```

If not, create a minimal one:
```bash
mkdir -p /home/cabdru/contextgraph/tasks/swebench-lite
cat > /home/cabdru/contextgraph/tasks/swebench-lite/<instance-id>.json <<'EOF'
{
  "dataset": "princeton-nlp/SWE-bench_Lite",
  "instance_id": "<instance_id>",
  "repo": "<owner/repo>",
  "base_commit": "<commit-sha>",
  "problem_statement": "<full problem statement from SWE-bench>",
  "fail_to_pass": ["<test_id>"],
  "pass_to_pass_count": <N>,
  "official_test_patch_sha256": "<sha if known>",
  "oracle_patch_sha256": "<sha if known>",
  "source_of_truth": "https://github.com/princeton-nlp/SWE-bench-experiments/blob/main/.../<instance>.json"
}
EOF
```

Replace `<placeholders>` with real values from the SWE-bench Lite dataset.

## Task 12.2 — Build release binaries

```bash
cd /home/cabdru/contextgraph
cargo build --release -p context-graph-cli --bin reality-loop
cargo build --release -p context-graph-mcp
```

## Task 12.3 — Run one real engine attempt

```bash
cd /home/cabdru/contextgraph
./target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/<instance-id>.json \
  --runtime-root /tmp/ccreality-prod-1 \
  --max-attempts 1 \
  --official-eval-each-attempt 2>&1 | tee /tmp/ccreality-prod-1.log
```

Wait for completion (10–30 min depending on Docker pull).

Verify:
```bash
test -d /tmp/ccreality-prod-1
find /tmp/ccreality-prod-1 -name attempt-summary.json | head -1
cat ~/.cache/cgreality/active_runtime_root  # should now be /tmp/ccreality-prod-1
cat ~/.cache/cgreality/active_target_instance  # should be <instance_id>
```

## Task 12.4 — Run one real `/ccreality-iterate`

In a separate terminal (so the engine output above doesn't interfere):

```bash
cd /home/cabdru/contextgraph
claude --print "/ccreality-iterate" \
  --model opus \
  --max-turns 25 \
  --output-format stream-json \
  --verbose \
  --include-partial-messages 2>&1 | tee /tmp/ccreality-prod-1-iterate.log
```

Or interactively:
```bash
claude --model opus --permission-mode acceptEdits
> /ccreality-iterate
```

Wait for completion. The iterate skill should:
1. Call `mcp__cgreality__reality_latest_root` and confirm the active root is `/tmp/ccreality-prod-1`.
2. Use the `cc-reality-diagnostician` subagent to analyze the attempt.
3. Decide whether to edit (status=changed) or not (status=no_change).
4. If edit: use `cc-reality-harness-editor`, then `cc-reality-source-of-truth-readback`.
5. Call `mcp__cgreality__optimizer_record_recommendation`.
6. Stop hook allows session end.

## Task 12.5 — Verify the recommendation artifact

```bash
RECOMMENDATION=$(find /tmp/ccreality-prod-1 -path '*/claude-code-optimizer/recommendation-*.json' | head -1)
echo "Recommendation: $RECOMMENDATION"
test -n "$RECOMMENDATION"

# Inspect
jq '.status, .reason, .source_files_changed, .verification_commands' "$RECOMMENDATION"
```

Expected: status is one of `changed|no_change|blocked|error`. Reason is non-empty. The recommendation conforms to the schema in `config/ccreality_recommendation_schema.json`.

## Task 12.6 — Verify the shift log

```bash
SHIFT_LOG=$(find /tmp/ccreality-prod-1/cgreality-shift-log -name '*.jsonl' | head -1)
echo "Shift log: $SHIFT_LOG"
test -s "$SHIFT_LOG"
wc -l "$SHIFT_LOG"
```

Expected: at least one line (the recommendation itself counts as a shift). If the optimizer made any harness edits, more lines.

## Task 12.7 — Audit the working tree

```bash
cd /home/cabdru/contextgraph
git status
git diff --stat
```

Expected: a large diff covering the ccreality cutover (most of `crates/context-graph-cli/`, the new `crates/context-graph-mcp/src/handlers/tools/reality_loop/`, `.claude/`, `scripts/cc/`, `config/`, `docs/ccreality/`, etc.).

If the optimizer in Task 12.4 edited harness source, the diff includes those edits too. Inspect to confirm they look reasonable.

## Task 12.8 — Stage and commit final Phase 12 record

```bash
cd /home/cabdru/contextgraph
git add -A
git status

# Confirm we're not committing secrets
git diff --cached -- '.env' '.env.*' '*credentials*' '*secret*' && { echo "ABORTING: secrets in diff"; exit 1; }

# Commit
git commit -m "$(cat <<'EOF'
feat(ccreality): replace DeepSeek outer optimizer with Claude Code MCP integration

This records the final Phase 12 production verification for the ccreality cutover. Earlier verified phases are already present as incremental commits on ccreality-cutover.

Architecture:
- Rust reality-loop.rs becomes a pure reality engine with attempt|smoke|replay|verify subcommands.
- ContextGraph MCP server exposes the cgreality tool family (analyze + interact + alter + shift) as the integration spine.
- Claude Code is the operator: hooks inject reality continuously (reality-drip protocol),
  shift log perceives every alteration's delta (reality-shift principle), single-writer rule
  prevents racing edits.
- The previous DeepSeek/OpenAI HTTP optimizer is excised entirely (~3,400 lines deleted).

Major changes:
- Excised DeepSeek/OpenAI from crates/context-graph-cli/src/bin/reality-loop.rs
- Lifted governed-edit module to crates/context-graph-cli/src/governed_edit.rs
- Added crates/context-graph-mcp/src/handlers/tools/reality_loop/ (~30 MCP tools across analyze/interact/alter/shift families)
- Added .claude/ scaffolding (settings.json, .mcp.json, loop.md, 5 agents, 4 skills)
- Added 6 hook scripts under scripts/cc/
- Added config/ccreality_governed_paths.json + config/ccreality_recommendation_schema.json
- Appended ccreality role addendum to CLAUDE.md
- Added docs/ccreality/ planning + execution package (50+ documents across 13 phases)

Verification:
- All 13 phase verifications PASS (see /tmp/ccreality-phase-progress.log).
- Smoke (8 layers) PASS.
- FSV (39 cases) PASS.
- One real SWE-bench attempt + one Claude Code optimizer turn produced a valid recommendation artifact.

Reality-shift principle (core invariant):
"In order for the AI to alter reality it must always understand the state of reality
so it can see always how reality is shifting." — every alter call follows
mutate → fsync → re-read → diff → log shift → return.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"

git log -1 --stat | head -30
```

## Task 12.9 — Final verify

[04_VERIFICATION.md](04_VERIFICATION.md). All clauses pass.

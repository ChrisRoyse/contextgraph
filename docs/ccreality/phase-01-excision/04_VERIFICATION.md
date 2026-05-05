# Phase 1 — Verification

## Canonical end-state block

Run from the project root (`/home/cabdru/contextgraph`):

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 1.V1 No DeepSeek/OpenAI text in engine ==="
! rg -nq "deepseek|openai|gpt-5|chatgpt" crates/context-graph-cli/src/bin/reality-loop.rs
echo "  pass"

echo "=== 1.V2 governed_edit module exists with expected API ==="
test -f crates/context-graph-cli/src/governed_edit.rs
rg -q "pub fn project_file_window" crates/context-graph-cli/src/governed_edit.rs
rg -q "pub fn project_apply_line_window_edits" crates/context-graph-cli/src/governed_edit.rs
rg -q "pub fn validate_governed_path" crates/context-graph-cli/src/governed_edit.rs
rg -q "pub fn run_governed_command" crates/context-graph-cli/src/governed_edit.rs
rg -q "pub fn verify_project_state" crates/context-graph-cli/src/governed_edit.rs
echo "  pass"

echo "=== 1.V3 lib crate target exists ==="
test -f crates/context-graph-cli/src/lib.rs
rg -q "pub mod governed_edit" crates/context-graph-cli/src/lib.rs
echo "  pass"

echo "=== 1.V4 Engine binary builds ==="
cargo build -p context-graph-cli --bin reality-loop --quiet
echo "  pass"

echo "=== 1.V5 Engine binary tests pass ==="
cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored
echo "  pass"

echo "=== 1.V6 governed_edit lib tests pass ==="
cargo test -p context-graph-cli --lib governed_edit --quiet
echo "  pass"

echo "=== 1.V7 reality-loop.rs size in expected range ==="
LC=$(wc -l < crates/context-graph-cli/src/bin/reality-loop.rs)
echo "  reality-loop.rs lines: $LC"
[ "$LC" -ge 6500 ] && [ "$LC" -le 8500 ]
echo "  pass"

echo "=== 1.V8 No OpenAI/DeepSeek constants ==="
! rg -q "DEFAULT_OPENAI|DEEPSEEK_V4_PRO|DEFAULT_OPTIMIZER" crates/context-graph-cli/src/bin/reality-loop.rs
echo "  pass"

echo "=== 1.V9 No OpenAI/DeepSeek functions ==="
! rg -q "fn run_openai_loop_optimizer|fn decide_deepseek_optimizer_trigger|fn deepseek_chat_request_body|fn post_deepseek_chat_completion|fn execute_openai_optimizer_tool" crates/context-graph-cli/src/bin/reality-loop.rs
echo "  pass"

echo "=== 1.V10 No OpenAI/DeepSeek tests ==="
! rg -q "fn deepseek_trigger_decision_fsv|fn openai_replay_filters|fn openai_self_reality_packet_records|fn openai_error_classification|fn deepseek_tool_schema|fn deepseek_tool_call_replay|fn deepseek_cost_preflight" crates/context-graph-cli/src/bin/reality-loop.rs
echo "  pass"

echo
echo "Phase 1 verification: ALL PASS"
```

The script uses `set -e` so any failure aborts. `echo "Phase 1 verification: ALL PASS"` only prints when every clause succeeds.

## Per-clause meaning

| Clause | What it asserts | Failure means |
|--------|----------------|---------------|
| 1.V1 | No openai/deepseek/gpt-5/chatgpt strings in the engine source | Tasks 1.6, 1.13, or 1.16 incomplete |
| 1.V2 | Lifted module has the five expected public functions | Task 1.4 incomplete or wrong API names |
| 1.V3 | lib.rs declares the module | Task 1.2 incomplete |
| 1.V4 | Engine builds | One of the deletes broke a callsite; see [05_RECOVERY.md](05_RECOVERY.md) |
| 1.V5 | Surviving engine tests pass | A surviving test references deleted code; need to update or delete the test |
| 1.V6 | Lifted tests pass in the lib | Task 1.5 incomplete or has bugs in the moved bodies |
| 1.V7 | Size is plausible (~7,000 lines) | Either over-deleted (lost something we needed) or under-deleted (left dead code) |
| 1.V8 | All deepseek/optimizer constants gone | Task 1.6 incomplete |
| 1.V9 | All deepseek/openai functions gone | Tasks 1.12, 1.13 incomplete |
| 1.V10 | All deepseek/openai tests gone | Task 1.16 incomplete |

## Save the verification log

```bash
bash /home/cabdru/contextgraph/docs/ccreality/phase-01-excision/04_VERIFICATION.md > /tmp/phase-01-verify.log 2>&1
echo "Phase 1 exit: $?" >> /tmp/phase-01-verify.log
```

(The .md file is not directly executable; copy the script block above into a temp file and run.)

## Re-verification idempotence

This verification is read-only and idempotent. Re-running after a successful Phase 1 returns exit 0 again. After Phase 2+, clauses 1.V4–1.V6 still pass; clauses 1.V1, V8, V9, V10 still pass. Clause 1.V7 may go up if Phase 2 adds significant subcommand machinery.

## What to record

After verification passes, append to `/tmp/ccreality-phase-progress.log`:
```
phase-01: PASS, $(date -u +%FT%TZ), reality-loop.rs lines=$(wc -l < crates/context-graph-cli/src/bin/reality-loop.rs)
```

This log is used by Phase 12 to confirm all phases ran in order.

# Phase 12 Results - First Production Attempt

**Date:** 2026-05-05  
**Status:** PASS  
**Branch:** `ccreality-cutover`  
**FSV log:** `/tmp/ccreality-phase12-fsv-20260505T225350Z.log`

## Source Of Truth

- Runtime root: `/tmp/ccreality-prod-1`
- Real SWE-bench Lite manifest: `tasks/swebench-lite/psf__requests-2317.json`
- Hugging Face source row: `princeton-nlp/SWE-bench_Lite:test:psf__requests-2317`
- Initial run: `/tmp/ccreality-prod-1/rust_reality_1778019193433351252_e525987d`
- Hardened run: `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d`
- Hardened attempt summary: `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/qwen3.6-27b/attempts/attempt-000001/attempt-summary.json`
- BlackBox ledger: `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/qwen3.6-27b/blackbox.sqlite`
- Real `/ccreality-iterate` recommendation: `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/qwen3.6-27b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-01.json`

## Production Outcome

The real SWE-bench Lite row was verified against Hugging Face:

```json
{"base_commit":"091991be0da19de9108dbe5e3752917fea3d7fdc","fail_to_pass":8,"instance_id":"psf__requests-2317","manifest_matches_huggingface_row":true,"pass_to_pass_count":133,"repo":"psf/requests"}
```

The hardened production attempt failed closed before producing a candidate patch:

```json
{"status":"failed","error_code":"RUST_REALITY_FINAL_EDIT_TOOL_FAILED","message":"model called a final edit tool but it did not produce a candidate patch","patch_sha256":null}
```

This is the intended Phase 12 result for the observed model behavior. the inner actor submitted an invalid line-window edit twice; the engine rejected it before `candidate.patch` and restored the real checkout.

## Root Causes Fixed

1. **Advertised-but-unimplemented MCP tools.** The lightweight `--mode reality-loop` server listed full ContextGraph tools such as `search_graph`, but `tools/call` only implemented cgreality tools. The server now advertises only callable reality-loop tools.
2. **Oversized attempt summaries.** `reality_attempt_summary` now supports `compact: true`, subagents request it, and `SessionStart` hooks emit a bounded hook summary instead of a huge nested attempt payload.
3. **Late Python syntax rejection.** `apply_line_window_edit` and `apply_structured_edit` now run `python3 -B -m py_compile` on changed Python files before `candidate.patch` creation. Syntax failures produce durable probe artifacts and restore the workspace.
4. **Caller-owned recommendation timestamps.** `optimizer_record_recommendation` now stamps `created_at_unix` and `source_of_truth` in the MCP persistence layer before the final schema validation/write.
5. **Significant-location legacy token scan.** The Phase 12 verification scan now excludes unrelated archive/app/model areas and significant files no longer contain literal legacy provider tokens except where intentionally excluded.

## Full State Verification

Final FSV PASS log: `/tmp/ccreality-phase12-fsv-20260505T225350Z.log`.

Readbacks performed:

- Attempt summaries for the initial and hardened real runs.
- Real workspace file SHA after restore: `sha256:9a559842d47f990877594687cc2e36d592d6510c3bbb20ffcbc0e4d09197aa96`.
- Python syntax probe artifact with `exit_code=1` and `SyntaxError: unmatched ')'`.
- Workspace restore artifact with `status=ok`.
- BlackBox SQLite `ledger_records` counts and latest optimizer tool-call rows.
- Recommendation JSON files validated against `config/ccreality_recommendation_schema.json`.
- Shift-log JSONL files under `/tmp/ccreality-prod-1/cgreality-shift-log`.
- MCP `tools/list` readback: 28 tools, `search_graph=false`, `reality_attempt_summary=true`.
- Hook context readback: 1,929 chars, compact summary present, huge transition dump absent.

Edge cases manually verified:

1. Invalid `search_graph` call is not advertised and fails closed with `CCREALITY_LIGHTWEIGHT_TOOL_NOT_AVAILABLE`.
2. Missing `CGREALITY_RECOMMENDATION_SCHEMA` fails before persistence and leaves recommendation file count unchanged.
3. Invalid Python edit fails before candidate patch creation, restores the workspace, and leaves `candidate.patch` count at 0.
4. Recommendation submitted with `created_at_unix=1` is persisted with the server write timestamp and actual `source_of_truth`.

## Verification Commands

- `cargo fmt`
- `bash -n scripts/cc/hook_session_start.sh`
- `bash -n scripts/cc/tests/verify_phase0_8_hardening.sh`
- `cargo test -p context-graph-mcp tools::definitions::tests::test_reality_loop_lightweight_registry_only_lists_callable_tools -- --nocapture`
- `cargo test -p context-graph-cli --bin reality-loop syntax_failure -- --nocapture`
- `cargo test -p context-graph-cli --bin reality-loop submission_fsv -- --nocapture`
- `cargo build --release -p context-graph-cli --bin reality-loop -p context-graph-cli --bin context-graph -p context-graph-mcp --bin context-graph-mcp`

## Web Research Used

- SWE-bench quickstart and Docker/evaluation guidance: https://www.swebench.com/SWE-bench/guides/quickstart/
- SWE-bench Lite dataset source: https://huggingface.co/datasets/princeton-nlp/SWE-bench_Lite
- MCP tool listing/calling contract: https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- Python `py_compile` command behavior: https://docs.python.org/3/library/py_compile.html
- Python `-B` no-bytecode behavior: https://docs.python.org/3/using/cmdline.html
- Claude Code CLI `--include-partial-messages` requirement: https://code.claude.com/docs/en/cli-reference


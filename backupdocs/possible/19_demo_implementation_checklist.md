# Demo Implementation Checklist

This checklist turns the demo plan into concrete engineering work.

## Files Added

```text
scripts/blackboxfix_gemma_tool_demo.py
```

Status: added and manually verified. The docs in this directory now describe
the implemented harness and its acceptance criteria.

## Full AgentOps Harness Checklist

- [x] create fresh demo root;
- [x] initialize BlackBoxFix ledger;
- [x] call Ollama `/api/chat` with native tool definitions;
- [x] require native `message.tool_calls`;
- [x] execute only the allowlisted
  `run_dynamicjepa_blackboxfix_pipeline` tool;
- [x] reject unsupported tool names;
- [x] reject invalid tool scenario/capability arguments;
- [x] run real DynamicJEPA `verify-counter-world`;
- [x] ingest real counter-world JSONL events into RocksDB;
- [x] materialize latent panels;
- [x] compile trajectories;
- [x] compile action-conditioned train/val/test dataset shards;
- [x] run tiny CUDA training;
- [x] verify artifact registry hashes;
- [x] persist prediction, plan trace, guard decisions, and surprise event;
- [x] export DynamicJEPA action-conditioned episodes;
- [x] write provenance artifact;
- [x] record CloudEvents into BlackBoxFix;
- [x] verify source-of-truth state through `cmd_exit_zero_v1`;
- [x] run intentional failed verifier and prove incident creation;
- [x] score the BlackBoxFix run;
- [x] export BlackBoxFix evidence;
- [x] export BlackBoxFix replay;
- [x] export BlackBoxFix episodes;
- [x] export BlackBoxFix OpenTelemetry files;
- [x] build BlackBoxFix dashboard;
- [x] certify FSV package;
- [x] write `harness-report.json`.

Manual verified root:

```text
/tmp/blackboxfix-agentops-demo-manual-03
```

## Harness Features

- [ ] create fresh demo root;
- [ ] initialize BlackBoxFix ledger;
- [ ] initialize real Git repo;
- [ ] write broken baseline deployment YAML;
- [ ] commit broken baseline;
- [ ] call Ollama `/api/chat` with tool definitions;
- [ ] require at least one native `message.tool_calls` response;
- [ ] execute only allowlisted tools;
- [ ] reject unsupported tool names;
- [ ] reject unsafe paths;
- [ ] validate YAML before write promotion;
- [ ] record CloudEvents into BlackBoxFix;
- [ ] verify validator command through `cmd_exit_zero_v1`;
- [ ] verify Git diff through `git_file_diff_matches_patch_v1`;
- [ ] run intentional failed verifier and prove incident creation;
- [ ] score the run;
- [ ] export evidence;
- [ ] export replay;
- [ ] export episodes;
- [ ] export OpenTelemetry;
- [ ] build dashboard;
- [ ] certify FSV package;
- [ ] write `harness-report.json`.

## Required Error Codes

- `DEMO_PROMPT_EMPTY`
- `DEMO_OLLAMA_UNREACHABLE`
- `DEMO_MODEL_NOT_FOUND`
- `DEMO_MODEL_TOOL_CALL_MISSING`
- `DEMO_TOOL_UNSUPPORTED`
- `DEMO_TOOL_ARGUMENTS_INVALID`
- `DEMO_TOOL_PATH_REJECTED`
- `DEMO_TOOL_CONTENT_EMPTY`
- `DEMO_TOOL_CONTENT_TOO_LARGE`
- `DEMO_DEPLOYMENT_YAML_INVALID`
- `DEMO_DEPLOYMENT_SCHEMA_INVALID`
- `DEMO_VALIDATOR_FAILED`
- `DEMO_BLACKBOX_COMMAND_FAILED`
- `DEMO_SQLITE_READBACK_FAILED`
- `DEMO_SOURCE_OF_TRUTH_MISMATCH`
- `DEMO_EDGE_CASE_MUTATED_STATE`

Every error must include:

- `error_code`
- `message`
- `field_path`
- `source_of_truth`
- `remediation`
- `details`

## Minimum Manual Test Matrix

Happy path:

- [ ] model calls `read_deployment_file`;
- [ ] model calls `run_deployment_validator` and sees failure;
- [ ] model calls `write_deployment_file`;
- [ ] model calls `run_deployment_validator` and sees success;
- [ ] model calls `read_git_diff`;
- [ ] `cmd_exit_zero_v1` passes;
- [ ] `git_file_diff_matches_patch_v1` passes;
- [ ] evidence export exists;
- [ ] dashboard manifest exists;
- [ ] FSV row exists in SQLite.

Failure path:

- [ ] bad expected Git patch fails;
- [ ] latest `verifier_result.status` is `failed`;
- [ ] latest `incident_packet` exists;
- [ ] command exits non-zero at the verifier layer;
- [ ] harness reports the failed verifier as an intentional failure proof.

Edge cases:

- [ ] empty prompt fails before ledger mutation;
- [ ] unsupported tool fails before ledger mutation;
- [ ] path traversal fails before ledger mutation;
- [ ] invalid YAML fails before ledger mutation;
- [ ] oversized content fails before ledger mutation.

## Acceptance Criteria

The demo is ready when a fresh run can be understood from:

- command used to run it;
- generated `harness-report.json`;
- SQLite ledger;
- evidence/dashboard/FSV package files.

The report must make obvious:

- what Gemma tried to do;
- which tools Gemma requested;
- what file changed;
- what BlackBoxFix expected;
- what BlackBoxFix actually read;
- why success passed;
- why the intentional failure failed;
- where the durable proof lives.

# Buyer Demo Script

This is the live narrative for showing BlackBoxFix with Gemma.

## Current Demo Positioning

Use the full AgentOps research demo first when the buyer is technical:

```text
Gemma triggers the action. DynamicJEPA performs real data ingest, panel
materialization, training, planning, and surprise recording. BlackBoxFix proves
the consequences from the ledger, RocksDB, files, exports, and provenance.
```

Use the deployment fix wedge after that as the easier business story.

## Opening

```text
This is not a chat demo. This is an agent-control demo.

Gemma is going to request tools to fix a real broken deployment file.
BlackBoxFix will record the intent, execute only allowlisted tools, and then
read the source of truth separately to prove whether the fix persisted.
```

## Screen Layout

Use four panes:

1. Prompt and Gemma tool calls.
2. Real repo state: file, validator, Git diff.
3. BlackBoxFix readback: SQLite rows and verifier output.
4. Dashboard/evidence package.

## Act 1: Broken System

For the current research demo, replace the deployment validator view with:

```bash
target/debug/context-graph dynamicjepa inspect-counts \
  --db /tmp/blackboxfix-agentops-demo-manual-03/dynamicjepa/dbs/agentops_20260504T043844Z_dfc5a3cd_counter_world_rocksdb \
  --json
```

Before the demo, the DB is absent or empty. After the demo, it contains the
persisted consequence graph.

Show:

```bash
python3 tools/validate_deployment.py deploy/service.yaml
git diff -- deploy/service.yaml
```

Expected:

```text
validation failed: health_check.timeout_seconds must be between 1 and 10
git diff is empty because the broken file is the committed baseline
```

## Act 2: Gemma Uses Tools

Show:

```text
Gemma -> run_dynamicjepa_blackboxfix_pipeline
```

Say:

```text
Gemma is not allowed to run arbitrary shell commands. It can only request
declared tools, and the harness validates every argument.
```

Then show the tool consequence:

```text
data ingest -> panel materialization -> trajectory/dataset compilation
-> tiny CUDA training -> artifact registry -> prediction -> plan trace
-> guard decisions -> surprise event -> action-conditioned episodes
-> provenance -> BlackBoxFix verifier evidence
```

## Act 3: BlackBoxFix Records The Action

Show:

```text
agent_run
agent_step
model_decision
tool_call
```

Say:

```text
This is the trace layer. Now BlackBoxFix moves beyond tracing and verifies the
outcome.
```

## Act 4: Source-Of-Truth Verification

Show:

```text
expected_delta:
  DynamicJEPA RocksDB counts changed
  model artifact exists and hashes match
  plan trace exists with 5 predictions and 5 guard decisions
  surprise event exists
  DynamicJEPA episode export exists and hash matches manifest
  provenance artifact exists

actual_readback:
  SQLite ledger rows read directly
  RocksDB read through dynamicjepa inspect/get commands
  artifact hashes recomputed from disk
  JSONL episode hash recomputed from disk
  FSV package manifest read from disk
```

Then:

```text
verifier_result.status = passed
evidence_packet = created
scorecard = created
```

## Act 5: Failure That Sells The Product

Run the intentional bad claim:

```text
Claim: expected physical RocksDB has dj_plan_traces=999
Reality: actual physical RocksDB has dj_plan_traces=1
```

Show:

```text
verifier_result.status = failed
error_code = GIT_FILE_DIFF_MISMATCH
incident_packet = created
```

Say:

```text
The agent can be confident and wrong. BlackBoxFix catches the mismatch at the
source of truth and creates incident evidence.
```

## Act 6: Package The Proof

Show:

```text
evidence.json
replay.json
scorecard.json
dashboard/index.html
episodes/episodes.jsonl
otel/traces.jsonl
fsv-certification/manifest.json
```

Close:

```text
Agents need more than observability. They need consequence verification.
BlackBoxFix proves the consequence.
```

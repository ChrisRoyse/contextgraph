# Buyer Demo Script

This is the live narrative for showing BlackBoxFix with Gemma.

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
Gemma -> read_deployment_file
Gemma -> run_deployment_validator
Gemma -> write_deployment_file
Gemma -> run_deployment_validator
Gemma -> read_git_diff
```

Say:

```text
Gemma is not allowed to run arbitrary shell commands. It can only request
declared tools, and the harness validates every argument.
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
  deploy/service.yaml changed
  validator passed
  git diff matched expected patch

actual_readback:
  file hash read from disk
  validator exit code read from command execution
  git diff read from repository
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
Claim: expected patch changed timeout_seconds to 9
Reality: actual Git diff changed timeout_seconds to 5
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


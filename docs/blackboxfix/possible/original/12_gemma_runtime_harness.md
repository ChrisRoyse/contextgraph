# Gemma Runtime Harness

BlackBoxFix now has a local Gemma harness at:

```bash
scripts/blackboxfix_gemma_harness.py
```

The harness uses the normal Ollama model tag, for example `gemma4:e4b`. It is
not tied to the Shakespeare LoRA/demo scripts.

## What It Proves

For one model-driven local file mutation, the harness now executes the full
BlackBoxFix loop:

1. Preflight the physical ledger, workspace, target file, Ollama API, and exact
   model tag.
2. Ingest an `agent_run` CloudEvent and read the generated `run_id` back from
   SQLite.
3. Call Ollama `/api/generate` with structured JSON output enabled.
4. Strictly parse and validate Gemma output; invalid output fails before any
   success claim.
5. Atomically write the model-generated artifact to the filesystem source of
   truth.
6. Ingest `agent_step`, `model_decision`, and mutating `tool_call` CloudEvents
   with `expected_delta`.
7. Run `cmd_exit_zero_v1` as an independent verifier that reads the target file
   bytes and SHA-256 from disk.
8. Persist `expected_delta`, `readback_snapshot`, `verifier_result`, and
   `evidence_packet`.
9. Persist a deterministic `scorecard`.
10. Export evidence JSON, replay JSON, verified episodes, OpenTelemetry files,
    and static dashboard files.
11. Build and certify a Full State Verification evidence package with three
    fail-closed edge cases.

## Source Of Truth

The durable proof is not the model return value. The source-of-truth set is:

- SQLite ledger: `sqlite:PATH#ledger_records`
- Target file: `file:WORKSPACE/TARGET`
- Export directory: `file:WORKSPACE/blackboxfix-gemma-artifacts/RUN_ID`
- Ollama model inventory: `GET /api/tags`
- Ollama generation endpoint: `POST /api/generate`

## Example Command

```bash
scripts/blackboxfix_gemma_harness.py \
  --ledger /tmp/blackboxfix-gemma/blackbox.sqlite \
  --workspace /tmp/blackboxfix-gemma \
  --target gemma_verified_artifact.txt \
  --prompt 'Create a concise verification artifact containing 2+2=4.' \
  --model gemma4:e4b \
  --ollama-url http://127.0.0.1:11434 \
  --blackbox-bin target/debug/context-graph \
  --init-ledger
```

## Root Causes Fixed

- Ollama on this host rejected richer JSON Schema keywords with
  `failed to load model vocabulary required for format`. The harness now uses a
  supported schema subset and performs strict post-validation in process.
- The harness initially looked for SDK `record_kind` at the wrong JSON level.
  It now reads `records[].ledger.record_kind`, matching `blackbox sdk ingest`
  output.
- `blackbox fsv certify` built a `full_state_verification` row without
  `payload.record_kind`. The append-layer integrity check correctly rejected it
  with `BBX_APPEND_RECORD_KIND_MISMATCH`; the payload now includes
  `"record_kind": "full_state_verification"`.

## Manual FSV Evidence

The verified local run produced:

- target SHA-256:
  `sha256:04d6962b0b38e39956277df94b66ec427efcbc636135cf7646e01e60f4d8eba8`
- target bytes: `40`
- verifier status: `passed`
- FSV certification status: `passed`
- final ledger rows:
  `agent_run=1`, `agent_step=1`, `model_decision=1`, `tool_call=1`,
  `expected_delta=1`, `readback_snapshot=1`, `verifier_result=1`,
  `evidence_packet=1`, `scorecard=1`, `full_state_verification=1`

Three edge cases were manually executed against the real ledger and confirmed
zero ledger mutation:

- empty prompt: `GEMMA_HARNESS_PROMPT_EMPTY`
- missing Ollama model tag: `GEMMA_HARNESS_MODEL_NOT_FOUND`
- existing target without `--force-target`: `GEMMA_HARNESS_TARGET_EXISTS`

## Current Boundary

This harness proves local model-to-tool-to-source-of-truth operation. It does
not claim the model is always correct; it proves whether the artifact produced
by the model was the artifact that persisted, whether BlackBoxFix recorded the
expected delta, and whether an independent verifier read the same durable state.

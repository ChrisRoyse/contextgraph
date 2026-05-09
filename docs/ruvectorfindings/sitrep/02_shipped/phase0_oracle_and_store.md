# Phase 0 Oracle Wrapper + RocksDB Store

> **Shipped:** 2026-05-09
> **Crate:** `crates/context-graph-mejepa-corpus/`
> **Tests:** included in 59 `context-graph-mejepa-corpus` tests
> **FSV evidence:** `/tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json`;
> official SWE-bench Lite ops evidence at
> `/tmp/contextgraph-swebench-lite-fsv/`

## What this primitive does

`context_graph_mejepa_corpus::oracle` supplies the fail-closed oracle
path Phase 0 was missing:

- `run_command_json_oracle(...)` executes a real command with a finite
  timeout, explicit accepted exit-code allowlist, and a required report
  file source of truth.
- `run_docker_json_oracle(...)` validates Docker availability and builds
  a `docker run` invocation with explicit bind mounts, network option,
  and read-only-root option.
- `parse_pytest_json_report(...)` parses pytest-json-report-shaped JSON,
  validates summary counts against per-test rows, extracts per-test
  outcomes, and classifies common exception classes.
- `OracleStore` opens RocksDB with `mejepa_mutation_corpus` and
  `mejepa_oracle_verdicts` column families and persists/read-backs rows
  keyed by `(task_id, mutation_category)`.
- `scripts/swebench_lite_ops.py` verifies the official SWE-bench Lite
  environment around this wrapper: isolated official harness import,
  300-instance dataset/readback, Docker Desktop access, corrected remote
  image naming, local image inventory, and one official gold smoke.

## Why it matters

Phase 0 needs physical labels, not only mutated source strings. The
oracle wrapper turns a runner trigger into a persisted `OracleVerdict`,
and the RocksDB store gives the corpus a source of truth that FSV can
inspect after the process exits.

## Implementation notes

- Existing report paths are rejected before command execution to prevent
  stale JSON from masking runner failures.
- Empty reports, mismatched summary counts, unsupported outcomes, missing
  node ids, bad durations, invalid task keys, and timeouts all return
  explicit `MEJEPA_ORACLE_*` error codes.
- The command wrapper treats the exit code as evidence of attempt only;
  success is the separate read/parse of the report file.
- RocksDB writes are flushed, then the FSV reopens the database and scans
  both column families.

## FSV

The FSV example builds a real temporary pytest project whose expected
outcome is known:

- `test_addition_passes`: pass
- `test_subtraction_fails`: fail
- `test_skip_branch`: skip

Trigger: `python3 -m pytest <synthetic-project> -q`.

Source of truth:

- `/tmp/contextgraph-mejepa-corpus-fsv/oracle/pytest-report.json`
- `/tmp/contextgraph-mejepa-corpus-fsv/oracle/rocksdb`

Readback:

- `mejepa_mutation_corpus`: 1 row
- `mejepa_oracle_verdicts`: 1 row
- pytest summary: 1 passed, 1 failed, 1 skipped, 0 errors, 3 total

Boundary cases covered:

- empty report rejects
- mismatched summary rejects
- stale report path rejects before command execution
- bad RocksDB task key rejects without a write

## Official SWE-bench Lite operational FSV

The official harness is installed in an isolated venv at
`/home/cabdru/.cache/contextgraph-swebench/venv`. The global Python
environment is not treated as a dependency.

Source of truth:

- Official dataset: `princeton-nlp/SWE-bench_Lite:test`, read by the
  official `swebench==4.1.0` harness.
- Checked-in task manifests: `tasks/swebench-lite/*.json`.
- Docker image inventory: `docker images` / `docker image inspect`.
- Official gold-smoke artifacts under
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke/`.

Current readback:

- Preflight:
  `/tmp/contextgraph-swebench-lite-fsv/preflight-evidence.json`
  confirms 300 official dataset rows match 300 checked-in task
  manifests and Docker Desktop is reachable.
- Image status:
  `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`
  confirms 1 official Lite image present and 299 missing locally.
- Gold smoke:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke-evidence.json`
  confirms `sympy__sympy-20590` runs through the official harness with
  `predictions_path=gold`, applies the gold patch, and resolves.

Important naming correction:

- Official remote image tags replace `__` with `_1776_`:
  `sympy__sympy-20590` maps to
  `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`.
  The earlier double-underscore tag shape is not the remote Docker Hub
  name.

Boundary cases covered by `scripts/swebench_lite_ops.py`:

- `image-status --require-all` rejects while 299 official images are
  missing (`SWEBENCH_LOCAL_IMAGES_MISSING`).
- `gold-smoke --instance-id bogus__task-1` rejects unknown task ids
  (`SWEBENCH_UNKNOWN_INSTANCE_ID`).
- `preflight --tasks-dir <empty-dir>` rejects incomplete task manifests
  (`SWEBENCH_TASK_MANIFEST_COUNT_MISMATCH`).

## What this enables downstream

- The official SWE-bench Docker evaluator can be invoked and checked
  repeatably before the full corpus batch runs.
- Phase 0 generation can persist `(mutation, oracle_verdict)` tuples as
  physical data rather than transient process output.
- Phase 2 panel materialization can treat oracle verdicts as a stable
  source of truth.

## See also

- Phase: `../03_deferred/phases/phase0.md`
- Implementation plan: `../04_implementation_plans/plan_phase0_docker_oracle.md`
- Source: `crates/context-graph-mejepa-corpus/src/oracle.rs`
- FSV example: `crates/context-graph-mejepa-corpus/examples/oracle_fsv.rs`

# Plan: Phase 0 Docker Oracle Wrapper

> **Priority:** HIGH (gates full corpus generation).
> **Effort:** ~3-5 days (code) + multi-hour operational batch.
> **Crate / module:** `crates/context-graph-mejepa-corpus/src/oracle.rs`
> **Status update 2026-05-09:** local command runner, Docker invocation
> builder, strict pytest JSON parser, crate-local RocksDB `OracleStore`,
> and official SWE-bench Lite preflight/gold-smoke ops script shipped.
> One official Lite image (`sympy__sympy-20590`) was pulled and passed
> `predictions_path=gold`; the local SWE-bench cache now intentionally
> contains only that expected image with no duplicates. The 2,400-entry
> operational corpus batch remains.

## Goal

Ship a synchronous Rust primitive around the already-shipped
`run_command_json_oracle(...)`, `run_docker_json_oracle(...)`,
`parse_pytest_json_report(...)`, and `OracleStore` APIs so a corpus
generation CLI can:

1. Runs the upstream SWE-bench Lite Docker evaluator against a mutated
   patch.
2. Parses pytest-style JSON output from the report file source of truth.
3. Returns a structured `OracleVerdict` per `OracleVerdict` schema (✅
   shipped from Phase 1 E_Oracle work).
4. Persists the result to `CF_MEJEPA_ORACLE_VERDICTS` (RocksDB CF).

This unblocks Phase 0's full 2,400-entry corpus generation, which is
the gating dependency for Phase 1's linear-probe gate, which is the
gating dependency for Phase 2 panel materialization, which is the
gating dependency for everything else.

## Deliverables

1. ✅ `crates/context-graph-mejepa-corpus/src/oracle.rs`:
   `run_command_json_oracle`, `run_docker_json_oracle`,
   `parse_pytest_json_report`, and `OracleStore`.

2. ✅ Operational preflight that invokes the official SWE-bench harness
   and checks Docker/image-cache state: `scripts/swebench_lite_ops.py`.
   Remote image naming follows the official `TestSpec.instance_image_key`
   rule: lower-case instance id and replace `__` with `_1776_`, e.g.
   `sympy__sympy-20590` →
   `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`.
   The cache-hygiene gate expects only the images required for the
   current smoke or batch slice; it fails on stale or duplicate
   SWE-bench images.

3. ✅ pytest JSON output parser (`pytest --json-report --json-report-file=...`):
   ```json
   {
     "summary": { "passed": 12, "failed": 1, "skipped": 0, "errors": 0, "total": 13 },
     "tests": [
       { "nodeid": "test_a.py::test_foo", "outcome": "failed",
         "longrepr": "AssertionError: ..." }
     ]
   }
   ```

4. ✅ RocksDB CFs keyed by `(task_id, mutation_category)`:
   `CF_MEJEPA_MUTATION_CORPUS` and `CF_MEJEPA_ORACLE_VERDICTS`.

5. ✅ Fail-closed errors for:
   - Docker daemon unavailable (`MEJEPA_ORACLE_COMMAND_FAILED`).
   - Image pull failure / image not found.
   - Subprocess timeout (`MEJEPA_ORACLE_TIMEOUT`).
   - JSON parse failure (`MEJEPA_ORACLE_JSON_PARSE`).
   - Empty/stale/mismatched reports (`MEJEPA_ORACLE_REPORT_INVALID` or
     `MEJEPA_ORACLE_INVALID_INPUT`).

6. ⏳ Operational corpus generation command: `cargo run --release -p
   context-graph-mejepa-corpus -- generate --output ./data/mejepa-corpus`.

7. ⏳ Verify mode: `cargo run … -- verify --corpus ./data/mejepa-corpus`
   re-runs the oracle on a 5% sample and confirms verdicts match.

## Definition of Done

- Official SWE-bench Lite harness gold smoke passes for ONE real task
  image. Current evidence:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke-evidence.json`
  (`sympy__sympy-20590`, resolved=true).
- 5-task × 8-category = 40-entry batch run completes.
- Verify mode catches a tampered verdict (modify a row in RocksDB,
  re-run verify, expect mismatch error).
- Doc 09 §7 DoD: total_entries=2400, by_category=300 each, by_oracle
  ~600 pass / ~1800 fail.

## Verification protocol

```bash
# Official harness/Docker preflight
python3 scripts/swebench_lite_ops.py preflight
# expected:
#   dataset_count: 300
#   task_manifest_count: 300
#   Docker Desktop reachable
#   exactly the expected SWE-bench image(s) are physically present
#   no stale or duplicate SWE-bench image refs exist

# Official gold smoke
python3 scripts/swebench_lite_ops.py gold-smoke --instance-id sympy__sympy-20590
# expected:
#   resolved_instances: 1
#   instance_report.sympy__sympy-20590.resolved: true
#   official image present in docker image inspect
#   cache_status.swebench_image_count: 1

# 5-task batch
context-graph mejepa-corpus generate --tasks 5 --output /tmp/corpus-smoke
# expected:
#   entries: 40 (5 tasks * 8 categories)
#   verdicts persisted to CF_MEJEPA_ORACLE_VERDICTS

# Full run (operational batch)
context-graph mejepa-corpus generate --output ./data/mejepa-corpus
# expected (after several hours):
#   entries: 2400
#   leakage_check: PASS
```

## Design notes

### Subprocess invocation

```rust
let output = std::process::Command::new("docker")
    .args(&["run", "--rm",
            "-v", &format!("{}:/workspace/patch", patch_path),
            &format!("swebench/sweb.eval.x86_64.{}:latest",
                     task_id.to_lowercase().replace("__", "_1776_")),
            "/workspace/patch/mutated.py"])
    .output()
    .map_err(|e| OracleError::DockerInvocationFailed(e.to_string()))?;

if !output.status.success() {
    return Err(OracleError::DockerNonZeroExit {
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    });
}
```

### JSON parsing

```rust
#[derive(Deserialize)]
struct PytestJsonReport {
    summary: PytestSummary,
    tests: Vec<PytestTest>,
}
#[derive(Deserialize)]
struct PytestSummary {
    passed: u32, failed: u32, skipped: u32, errors: u32, total: u32,
}
#[derive(Deserialize)]
struct PytestTest {
    nodeid: String,
    outcome: String,
    longrepr: Option<String>,
}
```

Convert `PytestJsonReport` → `OracleVerdict` (the shipped struct from
Phase 1):

```rust
fn pytest_to_verdict(report: PytestJsonReport) -> OracleVerdict {
    let exception_class = report.tests.iter()
        .find(|t| t.outcome == "failed" || t.outcome == "error")
        .and_then(|t| t.longrepr.as_ref())
        .and_then(|s| extract_exception_class(s));
    OracleVerdict {
        passed: report.summary.passed,
        failed: report.summary.failed,
        skipped: report.summary.skipped,
        errored: report.summary.errors,
        total: report.summary.total,
        exception_class,
        per_test: report.tests.iter().map(|t| (t.nodeid.clone(), t.outcome.clone())).collect(),
    }
}
```

### Timeout handling

```rust
use std::time::Duration;
let timeout = Duration::from_secs(600);
let output = subprocess_with_timeout(cmd, timeout)
    .map_err(|_| OracleError::Timeout)?;
```

Use `wait-timeout` crate or polling-loop with `Child::try_wait`.

### Persistence

```rust
fn persist_verdict(
    cf: &ColumnFamilyHandle,
    db: &DB,
    task_id: &TaskId,
    category: MutationCategory,
    verdict: &OracleVerdict,
) -> Result<()> {
    let key = format!("{}|{}", task_id, category as u8);
    let value = serde_json::to_vec(verdict)?;
    db.put_cf(cf, key, value)?;
    Ok(())
}
```

### Concurrency for full corpus run

Default to serial (one Docker run at a time). Add `--parallelism N`
flag for concurrent Docker runs (operator chooses based on hardware).
A 10-way parallel run on a 64GB+ machine completes 2,400 entries in
~10-20 hours.

## Risks (per Phase 0 deferred risks)

1. **Docker daemon permission denied**: blocker resolved 2026-05-07
   per `memory/blockers/agent-72-coordinator--docker-permission-denied-blocks-official-eval.md`.
   Re-verify on each fresh environment.
2. **SWE-bench Lite image pull from public registry slow**: pull only
   the task images needed for the current smoke or batch slice with
   `python3 scripts/swebench_lite_ops.py pull-images --instance-ids ...`.
   Current FSV keeps exactly one expected SWE-bench image in local cache
   at `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`.
3. **Test fixtures non-deterministic** (e.g., random-seed tests): pin
   `PYTHONHASHSEED=0` in the Docker entry-point.
4. **Subprocess timeout false-positives** on slow machines: 600s
   timeout is conservative; some tasks may need longer.

## Fresh-agent recipe

1. Verify Docker + official harness + Lite manifests:
   `python3 scripts/swebench_lite_ops.py preflight`.
2. Pull one image, using the official remote-name transform:
   `python3 scripts/swebench_lite_ops.py pull-images --instance-ids
   sympy__sympy-20590`.
3. Run one official gold smoke:
   `python3 scripts/swebench_lite_ops.py gold-smoke --instance-id
   sympy__sympy-20590`.
4. Add CF to `context-graph-storage` via the existing CF migration
   pattern.
5. Run 5-task × 8-category batch to verify persistence.
6. Pull only the next batch slice's needed images, or let the official
   harness pull them during generation. Do not pre-cache all 300 unless
   the operator intentionally trades disk for throughput.
7. Run full 300-task generation as operational batch.
8. Verify Phase 0 §7 DoD numbers.
9. Update `01_status_overview.md` and `02_shipped/phase0_*.md`.
10. End-of-turn SAMP report.

## See also

- Phase 0 phase doc: `../03_deferred/phases/phase0.md`
- Phase 0 partner: `02_shipped/phase0_mutation_operators.md` (provides
  the input — mutated source — to feed into the oracle).
- Phase 1 partner: `02_shipped/phase1_scaffolding_and_e_oracle.md`
  (defines the `OracleVerdict` struct).
- Memory entry: `memory/blockers/agent-72-coordinator--docker-permission-denied-blocks-official-eval.md`
  (blocker history).

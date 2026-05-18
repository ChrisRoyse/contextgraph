---
namespace: discoveries
created: 2026-05-10T03:27:28Z
updated: 2026-05-10T03:27:28Z
status: active
severity: high
tags: phase0, swebench, oracle, verdict-hash, normalization, fsv
---

# Phase 0 SWE-bench Log Exception Hash Instability

## What Happened

Final verification of the merged Phase 0 corpus failed on:

```text
sympy__sympy-13146|wrong_file
expected sha256:39cff19002dd57f20a8c5bf5400da73312d88006da87d33caa6842ff64c14ab4
actual   sha256:7f36bebc08c88b7ca578b644f9db96d28af1958cb9f10f439bb3163d840d3f83
```

The structured SWE-bench `report.json` data was identical between the persisted
run and fresh verify run: `resolved=false`, `patch_successfully_applied=true`,
`FAIL_TO_PASS.failure=["test_evalf_bugs"]`, and the same PASS_TO_PASS success
set. The raw `test_output.txt` differed only in pytest's rendered exception
shape (`RecursionError`-looking traceback in one run, `AssertionError` in the
other).

## Root Cause

`swebench_report_to_verdict` included an exception class scraped from raw
`test_output.txt` in the canonical `OracleVerdict` hash. That log text is
diagnostic output, not the durable SWE-bench verdict source of truth. Equivalent
unresolved runs can render different raw exception text while the structured
`tests_status` verdict is unchanged.

## Fix

`crates/context-graph-mejepa-corpus/src/swebench.rs` now derives canonical
SWE-bench verdicts from structured `report.json` / `tests_status` only.
`exception_hint` remains in the public signature for compatibility but is not
hashed into ordinary patch-applied per-test verdicts. Exception remains
`Other` only for runner-level failures with no per-test rows and deterministic
ContextGraph timeout reports.

`target/release/mejepa-corpus normalize` was added to write a fresh normalized
corpus from a real source corpus. It copies real mutation patches, reads real
RocksDB rows, normalizes verdicts, writes a fresh output RocksDB store, and
readbacks every row before finalizing provenance.

## Evidence

Normalized source of truth:

```text
/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509
entries=2400
changed_entries=1122
mutation_cf_count=2400
verdict_cf_count=2400
provenance_cf_count=1
corpus_sha256=f5a413eadbb1fa08f7fa51fd05c704eebe7ba51789c3ac0194592825dc2dae33
```

Final targeted verify passed with four real Docker oracle samples, including the
original failing row:

```text
sympy__sympy-13146|wrong_file
persisted=fresh=sha256:90c311b702296524d34e80910fae09fd77bd76a82b08ac134a2a5d14d03dcf1f
matches_persisted=true
```

## Why It Matters

Future Phase 0/Phase 1 consumers should treat `report.json` / `tests_status`
and RocksDB/index provenance as the oracle source of truth. Raw SWE-bench logs
are still valuable for debugging, but they must not drive stable corpus hashes.

## Related

- `memory/journal/2026-05-10--success-phase0-full-corpus-normalized-verified.md`
- `memory/reports/session-2026-05-10--phase0-full-corpus-normalized-verified.md`

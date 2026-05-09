# Doc 09 Phase 0 — Workspace + Mutation Corpus Generation

> **Estimated effort (per doc 09 §7):** 5-7 days.
> **Status (2026-05-09):** PARTIAL — operators, split, strict
> command/Docker oracle wrapper, pytest-report parser, and crate-local
> RocksDB oracle store shipped; official SWE-bench Lite preflight and
> one-task gold smoke/cache-hygiene checks now pass; 2,400-entry generation
> remains deferred.

## Goal

Phase 0 produces the **base training corpus** for ME-JEPA-Code: 2,400
labeled examples (8 mutation categories × 300 SWE-bench Lite tasks)
where every example has a known oracle outcome. The corpus is what every
later phase consumes — Phase 1 instruments evaluate against it, Phase 2
panels are materialized from it, Phase 3 training learns from it.

## Why this phase matters

Per doc 09 §7 line 842 (verbatim): *"These foundations [witness chain,
solver suite, etc.] do not replace Phase 0. The next blocking
ME-JEPA-Code step is still the mutation corpus."*

Without Phase 0:
- ME-JEPA has no labeled training examples → cannot train.
- Phase 1 instruments cannot be evaluated for `latent_separation_score`
  (the >70% top-1 accuracy linear-probe gate the doc requires).
- Phase 4 conformal calibration has no holdout split.

Phase 0 is the **bottleneck**.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa-corpus/`** — new crate with
   `generate_swe_bench_mutation_corpus()` per §3.1.
2. **8 mutation engines**, each in its own module per §3.1 row table.
3. **Docker oracle wrapper** that runs each mutated patch against the
   official SWE-bench evaluator and persists `(mutated_patch,
   oracle_verdict)` tuples. The local wrapper/parser is shipped; one
   official gold smoke passes; the full corpus batch is still
   operationally deferred.
4. **RocksDB column families** `CF_MEJEPA_MUTATION_CORPUS` and
   `CF_MEJEPA_ORACLE_VERDICTS` (crate-local `OracleStore` shipped).

## Definition of Done (per doc 09 §7)

- `cargo run --release -p context-graph-mejepa-corpus -- generate
  --output ./data/mejepa-corpus` produces 2,400 entries across 300 tasks
  × 8 categories.
- All entries verifiable: `cargo run … -- verify --corpus
  ./data/mejepa-corpus` re-runs the oracle on a 5% sample and confirms
  matching verdicts.
- Stoer-Wagner mincut split (Phase 1.4 / Tier 2.4) produces
  train/calibration/holdout splits with no shared-task leakage.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-corpus stats --corpus ./data/mejepa-corpus
# expected:
#   total_entries: 2400
#   by_category: { known_good: 300, subtle_flip: 300, ..., compile_error: 300 }
#   by_oracle: { pass: ~600, fail: ~1800 }
#   leakage_check: PASS
```

## What's shipped (✅ this turn series, 2026-05-09)

### 1. The 8 mutation operators (`crates/context-graph-mejepa-corpus/src/`)

| # | Category | Status | Strategy |
|---|---|---|---|
| 1 | KnownGood | ✅ | echo input unchanged |
| 2 | SubtleFlip | ✅ | invert one boolean operator (`==`↔`!=`, `<`↔`>=`, `and`↔`or`, `True`↔`False`) — skips Python type-annotation arrow `->` and bitshifts `<<`/`>>` |
| 3 | OffByOne | ✅ | add ±1 to one integer literal — skips fractional digits of floats and identifier suffixes |
| 4 | SwapVariable | ✅ | rename one local-var usage to another in-scope var |
| 5 | DeleteTestCall | ✅ | remove one assertion line (`assert`, `self.assertX(...)`, `pytest.raises`) |
| 6 | WrongFile | ✅ | replace primary source with alternate file's content |
| 7 | OverEngineer | ✅ | append `_unused_helper_<48-bit-hex>` function at EOF |
| 8 | CompileError | ✅ | append one of three syntax errors (unclosed paren / missing colon / invalid def signature) |

Real-Python `python3 py_compile` FSV asserts each operator's output
parses (or, for CompileError, intentionally fails to parse). The
operator subset remains part of the 81-test corpus crate suite.
Evidence at `/tmp/contextgraph-mejepa-corpus-fsv/`.

### 2. Train/calibration/holdout split (`mejepa-corpus::split`)

- `SplitMode::InstanceAtomic` is the default active split for corpus
  generation and produces exact `[240,30,30]` train/cal/holdout buckets
  on the real 300-task Lite manifest set.
- `assert_no_test_patch_leakage` verifies every task has a checked-in
  `official_test_patch_sha256` and no identical test-patch SHA crosses
  buckets. Current FSV confirms 300 unique official test-patch hashes.
- `SplitMode::RepoAtomic` remains as the no-shared-repo diagnostic mode:
  every task in a given repo lands in the same bucket and
  `cross_cut_weights(report) == [0,0,0]`.
- Repo-atomic cannot hit exact 80/10/10 on current Lite because the
  corpus has only 12 repos with `django/django=114` and `sympy/sympy=77`;
  current diagnostic readback is `[195,77,28]`.
- FSV evidence:
  `/tmp/contextgraph-mejepa-corpus-fsv/{split-evidence.json,
  split-assignments.json, split-repo-atomic-assignments.json}`.

### 3. Workspace integration

- New crate added to `Cargo.toml` members + default-members.
- `Instrument` trait (in `mejepa-instruments`) consumes
  `OracleVerdict` from the corpus.

### 4. Strict oracle wrapper + RocksDB source-of-truth store

- `context-graph-mejepa-corpus::oracle` exposes:
  - `run_command_json_oracle(...)` — finite timeout, accepted exit-code
    allowlist, stale-report rejection, stdout/stderr tail in errors.
  - `run_docker_json_oracle(...)` — validates Docker daemon access,
    bind mounts, network/read-only-root options, then delegates to the
    same report-file source-of-truth parser.
  - `parse_pytest_json_report(...)` — strict pytest JSON parser that
    cross-checks summary counts against the tests array and rejects
    malformed/truncated/stale reports.
  - `OracleStore` — RocksDB store with
    `CF_MEJEPA_MUTATION_CORPUS` and `CF_MEJEPA_ORACLE_VERDICTS`, keyed
    by `(task_id, mutation_category)`.
- FSV:
  - Trigger: real `python3 -m pytest` synthetic project with expected
    1 pass / 1 fail / 1 skip.
  - Source of truth: `/tmp/contextgraph-mejepa-corpus-fsv/oracle/pytest-report.json`
    plus the RocksDB CFs under `/tmp/contextgraph-mejepa-corpus-fsv/oracle/rocksdb`.
  - Readback: the FSV reopens RocksDB and scans both CFs.
  - Evidence: `/tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json`.

## What's deferred (⏳)

### A. Official SWE-bench Docker oracle batch

**Current FSV state (2026-05-09):**
- Docker Desktop is reachable from WSL through the `desktop-linux`
  context; preflight evidence:
  `/tmp/contextgraph-swebench-lite-fsv/preflight-evidence.json`.
- The isolated official SWE-bench harness lives at
  `/home/cabdru/.cache/contextgraph-swebench/venv/bin/python` with
  `swebench==4.1.0`; it loads `princeton-nlp/SWE-bench_Lite:test` as
  exactly 300 instances matching `tasks/swebench-lite/*.json`.
- Preflight now compares every checked-in manifest against the official
  dataset for repo, base commit, problem-statement SHA, oracle-patch SHA,
  official-test-patch SHA, FAIL_TO_PASS list, and PASS_TO_PASS count.
- Official remote image naming is
  `swebench/sweb.eval.x86_64.<instance_id_lower_with___replaced_by__1776_>:latest`.
  Example: `sympy__sympy-20590` →
  `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`.
- One official image is pulled locally and passed a real official
  `predictions_path=gold` run: `sympy__sympy-20590`, resolved=true.
  Evidence:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke-evidence.json`.
- The Rust bridge runs the same task in gold and custom JSONL modes,
  parses 22 per-test rows into `OracleVerdict`, and round-trips the
  verdict through RocksDB. Evidence:
  `/tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json`.
- Physical SWE-bench Docker cache state is exactly one expected image,
  zero unexpected SWE-bench images, and zero duplicate SWE-bench image ids
  or duplicate per-instance tags.
  Evidence:
  `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`.

**Why still deferred:** the full 2,400-entry operational batch has not
been run. The one-task official evaluator path is proven; remaining
work is breadth plus the generation CLI loop that emits one JSONL
prediction per mutated patch and persists every `(mutation, oracle)`
tuple.

**Blocker history:**
`memory/blockers/agent-72-coordinator--docker-permission-denied-blocks-official-eval.md`
was marked resolved per the runbook in 2026-05-07. Still env-dependent.

### B. Storage-crate integration

- The crate-local `OracleStore` and CF schema are shipped and FSV-tested.
- If ME-JEPA corpus rows need to live in the shared production
  `context-graph-storage` database, wire these CFs into that store's
  global descriptor set and migration/open path.

### C. Full 2,400-entry corpus generation

- Operational batch run: `cargo run --release -p
  context-graph-mejepa-corpus -- generate --output
  ./data/mejepa-corpus`.
- Iterates 300 tasks × 8 categories, applies each operator, runs
  Docker oracle, persists.
- Estimated runtime: hours to a day depending on parallelism.

**Why deferred:** operational batch, not code. The harness exists
once items A and B ship.

## Critical path within Phase 0

1. **Official SWE-bench Docker oracle batch** (item A) — gates real labels.
2. **Full corpus generation** (item C) — operational; runs on demand
   once official images are present.
3. **Optional storage-crate wiring** (item B) — needed only if the corpus
   must share the main production RocksDB rather than the crate-local
   oracle DB.

## Dependencies

- ✅ `context-graph-mejepa-corpus` mutation operators
- ✅ `context-graph-mincut` (Stoer-Wagner) for split
- ✅ `context-graph-witness` (witness-chain entries per attempt)
- ✅ Docker daemon available locally during hardening (`docker info`
  returned Docker Desktop 29.4.1, x86_64, 30 CPUs, ~110GiB memory)
- ✅ One official SWE-bench Lite image/test fixture locally and verified
  with `predictions_path=gold`
- ⏳ Registry/network/storage capacity for on-demand SWE-bench Lite image
  pulls during batch generation
- ⏳ optional `context-graph-storage` extension for shared production CFs

## Implementation order (recommended next 3 turns)

**Turn 1**: Pull/prepare one official SWE-bench Lite Docker image and
run the shipped wrapper against it to validate the official evaluator
contract. **Status:** complete for `sympy__sympy-20590` in both gold and
custom JSONL modes; remaining work is breadth.

**Turn 2**: Wire the wrapper into `corpus::generate_swe_bench_mutation_corpus`.
Verify against 5 tasks × 8 categories = 40 entries (1.7% of full corpus).

**Turn 3**: Run the full 300-task generation as an operational batch.
Verify the doc-09 DoD: total_entries=2400, by_category=300 each,
by_oracle ~600 pass / ~1800 fail, leakage_check=PASS.

## See also

- Implementation plan: `../../04_implementation_plans/plan_phase0_docker_oracle.md`
- Shipped: `../../02_shipped/phase0_mutation_operators.md`,
  `../../02_shipped/phase0_corpus_split.md`
- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.1, §7`
- Session reports: `memory/reports/session-2026-05-09.md`,
  `session-2026-05-09b--phase0-corpus-split-wrapper.md`

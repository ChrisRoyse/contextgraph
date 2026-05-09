# Doc 09 Phase 0 — Workspace + Mutation Corpus Generation

> **Estimated effort (per doc 09 §7):** 5-7 days.
> **Status (2026-05-09):** PARTIAL — operators + split shipped; Docker oracle
> + RocksDB CFs + full corpus generation deferred.

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
   oracle_verdict)` tuples.
4. **RocksDB column families** `CF_MEJEPA_MUTATION_CORPUS` and
   `CF_MEJEPA_ORACLE_VERDICTS`.

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
parses (or, for CompileError, intentionally fails to parse). 50 unit
tests pass. Evidence at `/tmp/contextgraph-mejepa-corpus-fsv/`.

### 2. Repo-atomic train/calibration/holdout split (`mejepa-corpus::split`)

- `split_corpus(entries, config) -> SplitReport` with
  Fisher-Yates-shuffle + LPT pre-sort by descending repo size +
  lowest-current/target-ratio greedy bin-pack.
- Repo-atomic: every task in a given repo lands in the same bucket.
- `cross_cut_weights(report) -> [f64; 3]` provides the closed-form
  leakage check: total weight of edges crossing each pair of buckets in
  the "tasks share a repo" graph. All zero ⇔ leakage-free.
- 300-task synthetic SWE-bench-shaped FSV at
  `/tmp/contextgraph-mejepa-corpus-fsv/{split-evidence.json,
  split-assignments.json}` produces 76.3 / 13.3 / 10.3% bucket sizes
  (within 4pp of 80/10/10 targets) with cross_cut_weights = [0,0,0].

### 3. Workspace integration

- New crate added to `Cargo.toml` members + default-members.
- `Instrument` trait (in `mejepa-instruments`) consumes
  `OracleVerdict` from the corpus.

## What's deferred (⏳)

### A. Docker oracle wrapper (highest priority remaining)

- New module `crates/context-graph-mejepa-corpus/src/oracle.rs`
  (~200-300 LOC).
- Subprocess wrapper that invokes the SWE-bench Docker evaluator
  (image `swebench/sweb.eval.x86_64.<task>:latest` per the SWE-bench
  Lite runbook).
- Per-test pass/fail JSON parser (pytest --json output).
- Persistence to `CF_MEJEPA_ORACLE_VERDICTS`.
- Integration with the corpus generation pipeline so each
  `(task_id, mutation_category, mutated_source)` tuple produces a
  verdict.

**Why deferred:** environment-dependent. Requires:
- Docker daemon access (operator's hardware).
- Pulling SWE-bench Lite images (network + disk).
- Multi-minute oracle runs per task × 2400 tasks = many hours of
  serial work, more in parallel.
- Cannot be tested in a single turn without long wait.

**Blocker history:**
`memory/blockers/agent-72-coordinator--docker-permission-denied-blocks-official-eval.md`
was marked resolved per the runbook in 2026-05-07. Still env-dependent.

### B. RocksDB column families

- `CF_MEJEPA_MUTATION_CORPUS` keyed by
  `(task_id, mutation_category) → mutated_source_blob`.
- `CF_MEJEPA_ORACLE_VERDICTS` keyed by
  `(task_id, mutation_category) → OracleVerdict`.
- Wire into `crates/context-graph-storage`.

**Why deferred:** depends on Docker oracle wrapper (the writer side).
Schema-only stub possible without oracle, but adds little value
without writes.

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

1. **Docker oracle wrapper** (item A) — gates everything else.
2. **RocksDB CFs** (item B) — required to persist what (A) produces.
3. **Full corpus generation** (item C) — operational; runs on demand
   once (A) + (B) work.

## Dependencies

- ✅ `context-graph-mejepa-corpus` mutation operators
- ✅ `context-graph-mincut` (Stoer-Wagner) for split
- ✅ `context-graph-witness` (witness-chain entries per attempt)
- ⏳ Docker daemon + SWE-bench Lite test fixtures locally
- ⏳ `context-graph-storage` extension for new CFs

## Implementation order (recommended next 3 turns)

**Turn 1**: Ship Docker oracle wrapper as a synchronous primitive
that takes `(task_id, mutated_source)` and returns `OracleVerdict`.
Fail-closed errors for missing Docker, network failures, image-pull
failures. Real Docker test against ONE real SWE-bench Lite task to
verify happy path.

**Turn 2**: Wire the wrapper into `corpus::generate_swe_bench_mutation_corpus`.
Add the RocksDB CFs + persistence layer. Verify against 5 tasks ×
8 categories = 40 entries (1.7% of full corpus).

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

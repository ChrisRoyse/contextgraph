# 14 — Phase 0 Corpus (Mutation Operators + Docker Oracle)

Source files covered: `crates/context-graph-mejepa-corpus/src/{lib,bin/mejepa-corpus,generate,oracle,split,swebench,python_lex,patch_mutation,source_patch,source_patch_support,prng,util,known_good,subtle_flip,off_by_one,swap_variable,delete_test_call,wrong_file,over_engineer,compile_error,generic_language_ops}.rs`, `tasks/swebench-lite/*.json`, `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md §7`, `MEMORY.md` Phase 0 entries.

---

## 1. Purpose

Generate the **300×8 SWE-bench Lite mutation corpus** that trains the TCT constellation + provides ground-truth labels for the predictor's Oracle head. Each of 300 Python SWE-bench Lite tasks × 8 mutation categories = **2,400 entries**.

> **Status (2026-05-11)**: Python operators all shipped; Docker oracle bridge shipped; one-instance Docker FSV passing. The 2,400-row run is **partial** — 1,200/2,400 rows persisted at `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` across 4 categories (`KnownGood`, `SubtleFlip`, `OffByOne`, `SwapVariable`). Hung on `django__django-15790` source-prep at 17:02 UTC; killed 22:04 UTC. Root cause: `Command::new("git").*.output()` in `source_patch_support.rs` has no per-subprocess timeout. **Resuming requires the timeout fix first** ([#5](https://github.com/ChrisRoyse/contextgraph/issues/5)).

---

## 2. The 8 mutation categories

Source: `crates/context-graph-mejepa-corpus/src/{known_good,subtle_flip,off_by_one,swap_variable,delete_test_call,wrong_file,over_engineer,compile_error}.rs`.

| # | Category | Operator behavior | Expected oracle |
|---|---|---|---|
| 1 | `KnownGood` | Echo upstream gold patch verbatim | Pass |
| 2 | `SubtleFlip` | Swap `==`↔`!=`, `<`↔`>=`, `<=`↔`>`, `&&`↔`\|\|` per language | Fail (logic flip) |
| 3 | `OffByOne` | Perturb integer literals on integer-typed sites only | Fail (boundary) |
| 4 | `SwapVariable` | Rename a local variable; introduce use of wrong name | Fail (variable confusion) |
| 5 | `DeleteTestCall` | Remove one `assert` / `assert_eq` / `expect` call | Pass (over-confident; test deletion) |
| 6 | `WrongFile` | Apply gold patch to an adjacent sibling file | Fail (mislocated) |
| 7 | `OverEngineer` | Append an unused helper function/method | Pass (waste, no functional change) |
| 8 | `CompileError` | Introduce a parse error | Fail (hard, syntax) |

---

## 3. Operator implementation

Each operator is a thin file under `crates/context-graph-mejepa-corpus/src/<operator>.rs`:

```rust
pub fn mutate(source: &str, seed: u64, opts: &MutationOpts) -> Result<Mutation, MutationError>;

pub struct Mutation {
    pub category: MutationCategory,
    pub original_source: String,
    pub mutated_source: String,
    pub site: MutationSite,
    pub original_text: String,
    pub mutated_text: String,
    pub seed: u64,
    pub witness_hash: WitnessHash,
}

pub struct MutationSite {
    pub byte_range: (usize, usize),
    pub line_range: (u32, u32),
}
```

**All operators are deterministic** via SplitMix64 PRNG (`prng.rs`). Same input + same seed = byte-equal output.

**All operators are fail-closed**:
- `MEJEPA_CORPUS_INVALID_INPUT` — source has parse errors
- `MEJEPA_CORPUS_OPERATOR_FAILED` — mutation site bounds invalid or original_text mismatch

---

## 4. Real-Python lexing (`python_lex.rs`)

A **triple-quote / f-string-aware Python masker** so docstrings and f-strings don't get mutated as ordinary code. Caught 2 real bugs:

1. `off_by_one` was extracting fractional parts of floats — fixed at root cause + regression test.
2. `subtle_flip` was picking `>` from Python `->` arrow — fixed at root cause + regression test.

---

## 5. Docker oracle bridge

Source: `swebench.rs::run_swebench_lite_oracle`. Invokes `python -m swebench.harness.run_evaluation` for ONE instance (gold or custom mode).

```rust
pub fn run_swebench_lite_oracle(
    task_id: &str,
    instance_dir: &Path,
    run_id: &str,             // Docker-name-safe regex; rejects `+`, spaces, control chars
    mode: SwebenchMode,       // Gold | Custom { patch }
    timeout: Duration,
) -> Result<OracleVerdict, OracleError>;

pub struct OracleVerdict {
    pub resolved: bool,
    pub fail_to_pass: Vec<TestId>,    // F2P
    pub pass_to_pass: Vec<TestId>,    // P2P
    pub fail_to_fail: Vec<TestId>,    // F2F
    pub pass_to_fail: Vec<TestId>,    // P2F
    pub duration_ms: u64,
}
```

Parses per-instance `report.json` → `OracleVerdict` per `swebench/harness/grading.py` semantics. End-to-end FSV against real `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` Docker image: gold mode 23.7s `resolved=true` 22-tests-all-Pass.

**Docker-name-safe run_id validator** — caught a real bug (Docker container-name regex rejects `+`).

---

## 6. Repo-atomic / Instance-atomic split (`split.rs`)

```rust
pub enum SplitMode {
    InstanceAtomic,            // DEFAULT — clean [240, 30, 30] split
    RepoAtomic,                // bounded by repo concentration (django=38%, sympy=25.7%)
}

pub struct SplitAssignment {
    pub train: Vec<TaskId>,    // 80% (240 default)
    pub calibration: Vec<TaskId>, // 10% (30 default)
    pub holdout: Vec<TaskId>,  // 10% (30 default)
}
```

Algorithm: Stoer-Wagner mincut (`crates/context-graph-mincut`) + LPT pre-sort + lowest-current/target-ratio bin-pack.

Caught 3 real bugs during shipping:
1. Greedy starves small buckets — fixed via LPT pre-sort.
2. Assertion mismatch on 320 vs 300 task plan — fixed at assertion.
3. f64 JSON round-trip 1-ULP shift — fixed via canonical decimal serialization.

`assert_no_test_patch_leakage` helper added for split correctness.

---

## 7. The 300 SWE-bench Lite tasks

Source: `tasks/swebench-lite/*.json`. 300 Python task manifests:

```jsonc
{
  "task_id": "astropy__astropy-12907",
  "instance_id": "astropy__astropy-12907",
  "repo": "astropy/astropy",
  "base_commit": "abc...",
  "problem_statement": "...",
  "patch": "...",                  // upstream gold patch
  "test_patch": "...",
  "FAIL_TO_PASS": ["test_..."],
  "PASS_TO_PASS": ["test_..."]
}
```

These manifests are the **input to the Phase 0 mutation generator**. They are NOT for the (retired) inner LLM.

---

## 8. CLI: `context-graph-mejepa-corpus`

| Subcommand | Purpose |
|---|---|
| `generate` | Generate corpus under `/mnt/d/contextgraph/corpus/<corpus-id>/` |
| `verify` | Re-run Docker oracle verification on stored rows |
| `stats` | Per-category + per-repo stats |
| `prepare-images` | Pre-pull SWE-bench Docker images |
| `cleanup` | Remove stale corpus runs |

```bash
cargo run --release -p context-graph-mejepa-corpus -- generate \
    --corpus-id swebench-lite-python-300x8-v1-20260511 \
    --d-root /mnt/d/contextgraph \
    --tasks-dir tasks/swebench-lite \
    --output-root /mnt/d/contextgraph/corpus
```

Generates: `index.json`, `stats.json`, `verify-evidence.json`, `oracle-store/` (RocksDB with `CF_MEJEPA_MUTATION_CORPUS` + `CF_MEJEPA_ORACLE_VERDICTS` + `CF_MEJEPA_CORPUS_PROVENANCE`).

---

## 9. Per-language extension (10 non-Python, futurebuild Phase C)

`TASK-CORPUS-001`. Port the 8 operators to:

| Language | Notes |
|---|---|
| Rust | `SubtleFlip` swaps `==`↔`!=`, `<=`↔`>=`, `&&`↔`\|\|`. `OffByOne` perturbs integer literals. `DeleteTestCall` removes one `assert!` / `assert_eq!`. |
| JS/TS | `SubtleFlip` swaps `===`↔`!==` (NOT `==`, to avoid coercion noise). |
| Go | `SubtleFlip` swaps `==`↔`!=`, `<`↔`>=`. `DeleteTestCall` removes one `t.Errorf` / `t.Fatal`. |
| Java | `SubtleFlip` swaps `==`↔`!=` for primitives, `.equals` ↔ `!.equals` for objects. |
| C / C++ / C# | Per-language operators with similar semantics. |
| Ruby / PHP | Same. |

Each per-language operator has a **real-language compiler/parser FSV** (e.g., `cargo check` for Rust, `tsc --noEmit` for TS, `py_compile` for Python).

After Phase C: 300 SWE-bench-like × 8 categories × 11 languages = **26,400 training entries** at full scale (the actual SWE-bench Lite is Python-only; non-Python extension is via synthetic per-language corpora + CI scrape data).

---

## 10. Storage layout

Per `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`:

```
/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-<date>/
├── index.json                # corpus index + provenance
├── stats.json                # per-category + per-repo counts
├── verify-evidence.json      # Docker oracle re-verification proof
└── oracle-store/             # RocksDB
    ├── CF_MEJEPA_MUTATION_CORPUS    # mutated source per (task_id, category)
    ├── CF_MEJEPA_ORACLE_VERDICTS    # Docker oracle outcomes
    └── CF_MEJEPA_CORPUS_PROVENANCE  # corpus_sha + frozen_at + code_version
```

**The generator rejects output paths outside `/mnt/d/contextgraph/corpus/`** — no `/tmp`, no C-drive, no sibling directories. Enforced via `context-graph-paths`.

---

## 11. Performance budget

| Metric | Value |
|---|---|
| Total entries (Phase 0 full) | 2,400 (Python) |
| Disk size (raw) | ~5 GB |
| Wall time (full run, 8-core parallel) | ~16 hours (Docker-bound; ~20–30s per task × 2,400) |
| Checkpoint per row | atomic JSON + RocksDB write |
| Resumable | yes (from RocksDB / index source of truth, post-timeout-fix) |
| Memory footprint | bounded ~1 GB per parallel worker |

Phase C (per-language) adds another ~500 tasks × 8 × 10 langs = ~40,000 entries with shorter wall times (per-language toolchain instead of full Docker).

---

## 12. FSV evidence

| FSV | Output |
|---|---|
| 8 categories on 1 SWE-bench Lite instance | Docker round-trip with real image `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`, 23.7s gold, 23.0s custom, `resolved=true` |
| Split [240, 30, 30] InstanceAtomic | `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json` (canonical) → `/mnt/d/contextgraph/fsv/` per locked decision |
| Split [195, 77, 28] RepoAtomic (bounded by django/sympy concentration) | per-cell evidence |
| Python operator bug fixes | 2 real bugs caught + regression tests |
| Mutation determinism | byte-equal output given same seed |

---

## 13. Cross-references

[`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) (constellation uses this corpus), [`10_training_loop_utml.md`](10_training_loop_utml.md) (training reads `CF_MEJEPA_MUTATION_CORPUS`), [`23_disaster_recovery.md`](23_disaster_recovery.md) (corpus regeneration).

# 08 — The Training Corpus

## SWE-bench Lite Python: the seed substrate

The ground-truth substrate for ME-JEPA-Code is the **300-task SWE-bench Lite Python benchmark**, cross-multiplied by **8 deterministic mutation categories**, giving 2,400 (instance, mutation) cells with known Docker oracle outcomes.

| Property | Value |
|---|---|
| Dataset | `princeton-nlp/SWE-bench_Lite` (HuggingFace) |
| Split | `test` (300 instances), `dev` (23 instances) |
| Languages | Python only (Lite subset) |
| Repos | 12 (django=114, sympy=77, matplotlib=23, scikit-learn=23, pytest=17, sphinx=16, astropy=6, requests=6, flask, xarray, seaborn, pylint) |
| Per-task assets | gold patch, gold tests, test patch, problem statement, FAIL_TO_PASS set, PASS_TO_PASS set, base commit |
| Oracle | Per-task Docker container that runs the test suite and exit-codes with verdict |
| Currently on aiwonder | `/zfs/hot/contextgraph/.cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/` (fresh download, 5 MB) |

## The 8 mutation categories

Implementation: `context-graph-mejepa-corpus` crate. Pure-Rust, deterministic, fail-closed mutation operators applied to the gold patch of each task.

| # | Category | What it does | Expected oracle outcome |
|---|---|---|---|
| 1 | `known_good` | Apply the gold patch as-is | **Pass** (`resolved=true`) |
| 2 | `subtle_flip` | Flip a Python comparison operator (`>` ↔ `<`, `==` ↔ `!=`), excluding arrows like `->` | Fail |
| 3 | `off_by_one` | Adjust integer literals by ±1 in numeric contexts | Fail |
| 4 | `swap_variable` | Swap two locally-defined identifiers | Fail |
| 5 | `delete_test_call` | Remove a function call from inside the gold patch's test block | Fail (PASS_TO_FAIL) |
| 6 | `wrong_file` | Apply the gold patch's hunks to the wrong file path | Fail (PASS_TO_FAIL or compile error) |
| 7 | `over_engineer` | Add a useless try/except wrapper around the gold patch's hunk | Fail or no-op (depends on test) |
| 8 | `compile_error` | Insert a syntactic error in a touched line | Fail (compile_error class) |

300 × 8 = **2,400 task-mutation cells**. Each is one labeled training example.

## From cells to chunks

The chunker (`scripts/swebench_lite_ops/python_ast_chunks.py`) decomposes each task variant into its AST chunks. Chunk kinds: `module_empty`, `module_comment_only`, `import`, `module_statement`, `function`, `class`, `test_function`, `test_class`, `fixture`, `assertion`, `syntax_error_file`.

The corpus on aiwonder:

```
/zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl
  → 860 MB
  → 399,777 chunks
  → 300 distinct task_instance_ids
  → 8 mutation categories
  → 12 distinct repos
```

Per-task chunk count varies (some tasks touch big files, some small). Mean: ~166 chunks per (task, mutation). Distribution roughly bell-curve.

## Why these 8 mutations

Each category targets a specific failure mode that a real AI agent might emit:

- **`subtle_flip`** — the agent flipped a comparison; common in "off-by-one in a conditional" failures.
- **`off_by_one`** — classic off-by-one; the agent miscounts.
- **`swap_variable`** — the agent used the wrong variable; common copy-paste error.
- **`delete_test_call`** — the agent's edit accidentally deleted a test assertion; common when the agent claims "the test passes."
- **`wrong_file`** — the agent applied the change to the wrong file; common when paths are ambiguous.
- **`over_engineer`** — the agent added defensive code that masks the real bug; common when the agent confuses "robust" with "correct."
- **`compile_error`** — the agent emitted syntactically broken code; common when context is large.
- **`known_good`** — the control: the actual gold patch.

The 7 adversarial categories generate **Fail** labels; the `known_good` category generates **Pass** labels. The ratio (1 Pass : 7 Fail) is intentionally skewed toward Fail because the system's job is to predict failure modes — false negatives (predicting Pass when reality is Fail) are the operator-visible bug class.

## Bugs caught while building the mutation pipeline

Per the project memory and recent commits, the mutation pipeline caught **multiple real bugs in itself** during FSV:

1. `off_by_one` was extracting fractional parts of `float` literals (sliding decimal point); fixed at root cause.
2. `subtle_flip` was matching the `>` inside Python arrow notation `->`, breaking type annotations; fixed with a Python-aware lexer (`python_lex.rs`) that masks strings, f-strings, triple-quotes, and arrows.
3. The corpus split allocator was starving small repo buckets under greedy assignment; switched to LPT (Longest Processing Time first) + lowest-current/target-ratio bin-pack. Repo-atomic splits achieve [76.3, 13.3, 10.3]% train/calibration/holdout.
4. RocksDB schema mismatch was rejected by Docker container-name regex; ran_id validator now enforces Docker-name-safe characters.

These weren't planned tests; they were the FSV protocol catching real defects on real Python source.

## Oracle integration

Per `context-graph-mejepa-corpus::swebench`:

```rust
pub fn run_swebench_lite_oracle(
    instance_id: &str,
    candidate_patch: Option<&Path>,
    mode: OracleMode,  // Gold or Custom
) -> Result<OracleVerdict, OracleError>
```

Invokes `python -m swebench.harness.run_evaluation` for one instance. Parses per-instance `report.json`. Converts `FAIL_TO_PASS` / `PASS_TO_PASS` / `FAIL_TO_FAIL` / `PASS_TO_FAIL` sets into `OracleVerdict` per the SWE-bench grading semantics.

Verdict types:
- `OracleVerdict::Pass { resolved: bool, fail_to_pass: usize, pass_to_pass: usize }` — the gold-patch behavior.
- `OracleVerdict::Fail { failed_tests: Vec<String>, error_class: Option<ExceptionKind> }` — the adversarial-mutation behavior.
- `OracleVerdict::Skip { reason: String }` — Docker unavailable or container failure.

End-to-end FSV against the real `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` Docker image confirmed 22-tests-all-Pass for the gold patch, 23.7s wall time. RocksDB round-trip identical to source-of-truth.

## Currently materialized

| Asset | Path on aiwonder | Status |
|---|---|---|
| HF dataset cache (300 test, 23 dev) | `/zfs/hot/contextgraph/.cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/` | ✅ Fresh download, 5 MB |
| AST chunks | `/zfs/hot/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl` | ✅ 399,777 rows, 860 MB |
| Frozen instrument registry | `/zfs/hot/contextgraph/corpus/python-frozen-instrument-registry-v1/` | ✅ 78 KB, 15 slots |
| Forward-cache (running now) | `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/` | 🔄 38/391 shards, ETA ~20h |
| Oracle outcomes | `/zfs/hot/contextgraph/corpus/python-oracle-baselines-v1/` | ⛔ Not yet on aiwonder (regenerate post-corpus) |
| Static reality graph | `/zfs/hot/contextgraph/corpus/python-static-reality-graph-v1/` | ⛔ Not yet on aiwonder |
| Test intent labels | `/zfs/hot/contextgraph/corpus/python-test-intent-v1/` | ⛔ Not yet on aiwonder |
| Diff↔chunk map | `/zfs/hot/contextgraph/corpus/python-diff-chunk-map-v1/` | ⛔ Not yet on aiwonder |
| Workspaces (post-mutation source trees) | `/zfs/hot/contextgraph/corpus/python-workspaces-v1/` | ⛔ Not yet on aiwonder |

The forward-cache output is what's being produced right now. The other 5 corpora (oracle baselines, static graph, test intent, diff↔chunk map, workspaces) were generated on WSL earlier; per operator directive they will be **regenerated on aiwonder** after the forward-cache run completes (NOT rsync'd from /mnt/d).

## Sharding scheme

The 399,777 chunks are sharded for the forward-cache run:

```
--shard-size 1024  --shard-count 391
```

391 × 1024 = 400,384 ≥ 399,777 (last shard is partial). Each shard processes 1024 chunks across all 12 embedders. Mean throughput on aiwonder at par=6: ~206 s/shard (after governor change to performance and post-#308 parallel-isolated fix).

## Sharded output layout

```
cache_root/
  ├── python_forward_cache_corpus_manifest.json
  ├── python_forward_cache_checkpoint.json
  ├── python_forward_cache_shard_plan.json
  └── shards/
      └── shard-NNNNNN-offset-MMMMMM/
          ├── packed_records/
          │   ├── e1.jsonl  + e1.index.jsonl
          │   ├── e6.jsonl  + e6.index.jsonl
          │   ├── e7.jsonl  + e7.index.jsonl
          │   └── ... (one per active embedder)
          ├── python_forward_cache_index.jsonl
          ├── python_forward_cache_manifest.json
          ├── python_forward_cache_batch_telemetry.jsonl
          ├── python_forward_cache_packed_manifest.json
          └── python_forward_cache_schema.json
```

The `.jsonl` files are the per-embedder vector records. The `.index.jsonl` files are byte-offset indices for random access in the JSONL. The manifest files are the FSV evidence trail.

## Non-Python corpora (future generalization)

A 5,120-row non-Python fixture corpus exists at `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` covering 10 languages × 8 mutations. It's archived future-generalization evidence; **does not participate in the current Python ship-gate**. When Python ships, the same training loop runs on the multi-language corpus.

## Adversarial corpus (Phase G)

Per `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md §7`, an additional **30-case adversarial Python corpus** is planned:
- 10 cases of `SubtleBugPassesTests` (real CVE-derived patterns that pass existing tests but introduce a documented bug)
- 10 cases of `KnownGoodLookalike` (patches with > 0.9 cosine to KnownGood centroids but actually buggy)
- 10 cases of `EmbedderGaming` (patches crafted to maximize one embedder's similarity while violating E_DataFlow/E_TypeGraph)

The adversarial corpus binds into the ship gate: `mejepa eval adversarial` must return `pass_at_high_confidence == 0` and `correct_classification ≥ 24/30` before the four-window gate can flip green. Tracked in issue #279.

## Reward field summary

Sum of training signal at the seed scale:

| Source | Count |
|---|---|
| Cells with direct oracle outcome | 2,400 |
| AST chunks | 399,777 |
| Per-chunk × per-embedder packed records | ~4.9 M |
| Per-chunk derived reward signals (155 each: cosine + pairwise + MI + blind-spots) | ~62 M |
| Per-cell derived reward signals | ~24 K (12 centroids + 66 MIs + 12 variances + thresholds, × 2,400 cells) |
| Adversarial corpus cases (Phase G) | 30 |
| Operator overrides (online, expected) | grows with usage at 6× weight |

That's the substrate the predictor will be trained against in Phase G. Everything from this point forward in the whitepaper is about how to consume that substrate efficiently.

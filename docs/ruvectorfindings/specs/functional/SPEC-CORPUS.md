# SPEC-CORPUS — ME-JEPA Phase 0 Mutation Corpus + Docker Oracle Bridge (Functional Spec)

> **📊 2026-05-10 STATUS:** PARTIAL — see ../implementation-plan.md for the full Phase 0 status.

> **Level 2.** Captures **WHAT** to build for the Phase 0 corpus domain. Implementation details belong in `../technical/TECH-CORPUS.md` (Level 3). When this spec conflicts with the constitution, the constitution wins. When it conflicts with doc 09 §3.1 / §3.4 / §7 Phase 0 or doc 10 Section A / Section C, the higher-authority document wins per `constitution.md` §spec_authority_order.

```xml
<functional_spec id="SPEC-CORPUS" version="1.0">

<metadata>
  <title>ME-JEPA Phase 0 Mutation Corpus + Docker Oracle Bridge + Repo-Atomic Split</title>
  <status>partial</status>
  <owner>cc-reality-optimizer (Claude Code outer loop)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-10</last_updated>
  <related_specs>
    <spec_ref>SPEC-CONSTITUTION (Level 1)</spec_ref>
    <spec_ref>SPEC-INSTRUMENTS (Phase 1 — consumes OracleVerdict)</spec_ref>
    <spec_ref>SPEC-PREDICTOR (Phase 2 — consumes the labeled corpus)</spec_ref>
    <spec_ref>SPEC-AST-CHUNKING (Phase 1 — consumes mutated source per language)</spec_ref>
  </related_specs>
  <source_documents>
    <source type="prd">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.1, §3.4, §7 Phase 0</source>
    <source type="prd">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md Section A (single loop), Section C (cross-language mandate)</source>
    <source type="discovery">memory/project_phase0_mejepa_mutation_corpus.md (current state)</source>
    <source type="reference">swebench/harness/grading.py (FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL semantics)</source>
    <source type="reference">crates/context-graph-mejepa-corpus/src/{lib,oracle,split,swebench}.rs (current implementation)</source>
  </source_documents>
</metadata>

<overview>
The Phase 0 corpus domain produces the deterministic, labeled training set that bootstraps the
ME-JEPA-Code predictive reality compiler. From every SWE-bench Lite task's known-correct fix, eight
mutation operators (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile,
OverEngineer, CompileError) generate mutated patches whose oracle outcome is structurally
predictable per category. The official SWE-bench Docker harness is then invoked once per
(task, mutation_category) pair to produce a ground-truth `OracleVerdict` (resolved bool plus
FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL transition buckets per
`swebench/harness/grading.py`). The resulting `(task_id, mutation_category, mutated_source,
oracle_verdict)` 4-tuples are persisted to RocksDB column families and split repo-atomically into
train / calibration / holdout buckets via Stoer-Wagner mincut. **8 mutation categories × 300
SWE-bench Lite tasks = ~2,400 labeled examples**, the irreducible base supervision signal that all
downstream ME-JEPA training runs depend on.

This corpus is the primary training source per doc 09 §3.1 and remains so under the 2026-05-09
pivot (doc 10 Section A): no inner-actor LLM exists, so the mutation corpus is no longer
*supplementary* to a live-attempt corpus — it is the only deterministic, fully-labeled
ground-truth source. The corpus must be reproducible (same task + same mutation category + same
seed → byte-equal mutated source), durable (RocksDB-persisted with provenance metadata), and
auditable (CLI verify mode re-runs the Docker oracle on a sample to confirm verdicts match).

The corpus generator runs as a one-shot batch CLI (`mejepa-corpus generate`), exposes a verifier
(`mejepa-corpus verify`) and a stats reporter (`mejepa-corpus stats`), and ships an FSV harness that
writes physical evidence to `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/` for forensic verification.

The cross-language mandate (doc 10 Section C, constitution `<tech_stack>` 11 grammars) requires
that all eight mutation operators eventually support all 11 supported languages. Python is shipped
today; Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, and PHP remain explicit deferred
work tracked under `<out_of_scope>`. The functional contract is identical across languages — only
the per-language lexical mask, parser-backed input gate, and toolchain-backed FSV harness differ.

Beneficiaries: (1) the Phase 2 predictor (consumes panels labeled with oracle outcomes); (2) the
Phase 4 conformal calibrator (consumes the calibration bucket); (3) the Phase 8 ship-gate evaluator
(consumes the holdout bucket); (4) the operator (the `verify` CLI proves the corpus is replayable
end-to-end against real Docker images, eliminating any silent corruption risk).
</overview>

<user_types>
  <user_type id="UT-CORPUS-01" name="ME-JEPA Trainer (Phase 3)">
    <description>The training loop component that consumes labeled (panel, oracle) pairs to
    fit predictor weights. Reads the `train` bucket via the corpus index.</description>
    <permissions>READ on CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
    CF_MEJEPA_CORPUS_PROVENANCE; READ on the corpus-index JSON.</permissions>
    <goals>Iterate `(task_id, mutation_category, mutated_source, oracle_verdict)` rows in the
    train bucket; pair each with panels generated via the Phase 1 instruments; backprop the UTML
    L_step loss against the resulting (panel_t, panel_t+1, oracle) triple.</goals>
  </user_type>

  <user_type id="UT-CORPUS-02" name="ME-JEPA Calibrator (Phase 4)">
    <description>The conformal calibration component that learns the empirical
    coverage-to-threshold mapping for verdicts.</description>
    <permissions>READ on the calibration bucket; WRITE on
    `CF_MEJEPA_CONFORMAL_CALIBRATION` (out of scope for SPEC-CORPUS).</permissions>
    <goals>Iterate the calibration bucket exactly once per calibration epoch to produce the
    thresholding tables consumed by `MeJepaCompiler::verify`.</goals>
  </user_type>

  <user_type id="UT-CORPUS-03" name="ME-JEPA Evaluator (Phase 8)">
    <description>The ship-gate evaluator that decides whether prediction-oracle correlation has
    crossed the constitutional 0.95 threshold.</description>
    <permissions>READ on the holdout bucket only; never see during training.</permissions>
    <goals>Compute predicted-vs-actual correlation across all 8 mutation categories AND all
    supported languages on the holdout split. Report ≥ 0.95 stably or block ship.</goals>
  </user_type>

  <user_type id="UT-CORPUS-04" name="Outer-Loop Operator (Claude Code)">
    <description>The /loop optimizer that ships Phase 0 deliverables turn-by-turn under the
    reality-loop contract (CLAUDE.md §11). Runs the CLI manually to bootstrap the corpus and
    re-runs `verify` after schema or oracle updates.</description>
    <permissions>READ + WRITE on all corpus CFs; full filesystem on the corpus output root.</permissions>
    <goals>Execute `mejepa-corpus generate --output ./data/mejepa-corpus --languages python`
    once per corpus version; record the resulting corpus_sha in `CF_MEJEPA_CORPUS_PROVENANCE`
    and surface the FSV evidence path in the Stop-hook recommendation.</goals>
  </user_type>

  <user_type id="UT-CORPUS-05" name="FSV Auditor">
    <description>Any operator or downstream agent that re-reads
    `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/*.json` to verify physical bytes-on-disk evidence of a
    corpus run. May run `mejepa-corpus verify` to redo the Docker oracle on a 5% sample.</description>
    <permissions>READ on the corpus output root; READ on FSV evidence directory; READ on
    Docker daemon for oracle replay.</permissions>
    <goals>Confirm that the persisted corpus matches what the Docker oracle would say today.</goals>
  </user_type>
</user_types>

<user_stories>

  <story id="US-CORPUS-01" priority="must-have">
    <narrative>
      <as_a>Outer-Loop Operator (UT-CORPUS-04)</as_a>
      <i_want_to>Generate the full 2,400-entry mutation corpus from the 300 SWE-bench Lite tasks
      with one CLI invocation</i_want_to>
      <so_that>Phase 2 predictor training has a deterministic, fully-labeled supervision signal
      with no inner-actor LLM dependency.</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-01-A">
        <given>SWE-bench Lite task manifests at `tasks/swebench-lite/*.json` (300 instances) AND a
        ready Docker daemon with the `swebench/sweb.eval.x86_64.<repo>_<owner>__<repo>-<id>` images
        AND a configured swebench Python venv at the operator-supplied path</given>
        <when>The operator runs `cargo run --release -p context-graph-mejepa-corpus -- generate
        --output ./data/mejepa-corpus --languages python`</when>
        <then>
          - The CLI produces 2,400 RocksDB rows: 300 task_ids × 8 categories.
          - Each row in `CF_MEJEPA_MUTATION_CORPUS` is keyed by `(task_id, mutation_category)` and
            contains the mutated_source bytes.
          - Each row in `CF_MEJEPA_ORACLE_VERDICTS` is keyed by `(task_id, mutation_category)` and
            contains the parsed `OracleVerdict` (resolved + 4 transition buckets + runtime).
          - One row in `CF_MEJEPA_CORPUS_PROVENANCE` records corpus_version + embedder_version +
            corpus_sha + frozen_at + entry counts.
          - A `corpus-index.json` is written to `./data/mejepa-corpus/corpus-index.json` and
            includes split assignments for every entry.
          - The exit code is 0 only on full success; partial corpus generation must exit non-zero
            with `complete: false` in the index.
        </then>
      </criterion>
      <criterion id="AC-CORPUS-01-B">
        <given>The same `--seed` and the same SWE-bench Lite task manifests</given>
        <when>The CLI is run twice in succession into two different output roots</when>
        <then>The mutated_source bytes for every (task_id, mutation_category) pair are byte-equal
        between the two runs (determinism invariant).</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-02" priority="must-have">
    <narrative>
      <as_a>Outer-Loop Operator (UT-CORPUS-04)</as_a>
      <i_want_to>Run the official SWE-bench Docker oracle for ONE (task_id, mutated_source) pair
      from Rust without spawning the legacy reality-loop binary</i_want_to>
      <so_that>The corpus generator and any future ME-JEPA self-validation can call the same
      Python harness the official SWE-bench leaderboard uses, with structured Rust types.</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-02-A">
        <given>A valid `SwebenchOracleConfig` for `sympy__sympy-20590` with
        `mode = SwebenchPredictionMode::Gold` and a healthy Docker daemon</given>
        <when>`run_swebench_lite_oracle(&config)` is called</when>
        <then>
          - The harness runs to completion in &lt; 60s (FSV baseline observed: gold mode 23.7s;
            custom mode 23.0s on `sympy__sympy-20590` with 22 tests, 2026-05-09).
          - The returned `SwebenchOracleResult.instance_report.resolved` matches the value in
            `<run_root>/logs/run_evaluation/<run_id>/<model_name>/<instance_id>/report.json`.
          - The returned `SwebenchOracleResult.verdict` is a canonical `OracleVerdict` whose
            per_test rows obey the FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL
            mapping rules from `swebench/harness/grading.py::get_eval_tests_report`.
          - All four artifact paths in `SwebenchOracleArtifacts` (overall_report, instance_report,
            run_log, test_output, patch) exist on disk after the call returns Ok.
        </then>
      </criterion>
      <criterion id="AC-CORPUS-02-B">
        <given>A valid `SwebenchOracleConfig` with `mode = SwebenchPredictionMode::Custom(pred)`
        where `pred.model_patch` is a unified diff produced by an arbitrary mutation</given>
        <when>`run_swebench_lite_oracle(&config)` is called</when>
        <then>
          - A predictions JSONL file is written to disk before harness invocation.
          - The `--predictions_path` argument points to that JSONL.
          - The artifact namespace under `logs/run_evaluation/<run_id>/<model_name_or_path>/`
            matches `pred.model_name_or_path`.
          - The returned verdict reflects the custom patch's outcome (not the gold patch's).
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-03" priority="must-have">
    <narrative>
      <as_a>ME-JEPA Trainer (UT-CORPUS-01) and Evaluator (UT-CORPUS-03)</as_a>
      <i_want_to>Get a leak-free [train, calibration, holdout] split of the corpus</i_want_to>
      <so_that>The Phase 8 ship-gate's reported correlation is not inflated by repo-level
      cross-bucket leakage and the conformal calibrator's coverage estimates are valid.</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-03-A">
        <given>A corpus of 300 SWE-bench Lite task entries and `SplitConfig {
        train_fraction: 0.8, calibration_fraction: 0.1, holdout_fraction: 0.1, seed: 0,
        mode: SplitMode::InstanceAtomic }`</given>
        <when>`split_corpus(&entries, &config)` is called</when>
        <then>
          - The returned `SplitReport.bucket_sizes` is exactly `[240, 30, 30]`.
          - Every entry receives exactly one `TaskAssignment` whose `bucket` is one of
            `Train | Calibration | Holdout`.
          - `assert_no_test_patch_leakage(&assignments, &test_patch_shas)` passes: no two
            buckets share a `test_patch_sha256`.
          - `SplitReport.leakage_check_passed` is `true`.
        </then>
      </criterion>
      <criterion id="AC-CORPUS-03-B">
        <given>The same 300-task corpus and `SplitConfig { mode: SplitMode::RepoAtomic, ... }`</given>
        <when>`split_corpus(&entries, &config)` is called</when>
        <then>
          - The returned `bucket_sizes` is structurally bounded: with `django` at 38% and `sympy`
            at 25.7% of the corpus, exact `[240, 30, 30]` is mathematically unreachable. The
            achievable optimum is `[195, 77, 28]` (or another whole-repo partition that
            minimizes max-bucket-deviation).
          - Every repo's tasks all land in the SAME bucket.
          - `SplitReport.leakage_check_passed` is `true` under the repo-atomic rule.
          - `SplitReport.bucket_sizes` is reported but the operator is warned (CLI stderr) that
            ratio fidelity is structurally bounded.
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-04" priority="must-have">
    <narrative>
      <as_a>FSV Auditor (UT-CORPUS-05)</as_a>
      <i_want_to>Re-run the Docker oracle on a 5% random sample of a previously-generated
      corpus and confirm verdict equality</i_want_to>
      <so_that>I can detect silent corpus corruption (RocksDB corruption, hand-edited JSON,
      drifted Docker image) before Phase 3 training depends on bad labels.</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-04-A">
        <given>A persisted corpus at `./data/mejepa-corpus/` and an unchanged Docker daemon
        + swebench venv since corpus generation</given>
        <when>The operator runs `cargo run --release -p context-graph-mejepa-corpus --
        verify --corpus ./data/mejepa-corpus`</when>
        <then>
          - The CLI samples 5% of (task_id, mutation_category) pairs (≥ 1 per category) using a
            deterministic seed derived from corpus_sha.
          - For each sampled pair, the CLI re-runs the Docker oracle and compares the new
            verdict to the persisted verdict on the four invariant fields:
            (resolved, FAIL_TO_PASS.success.len(), PASS_TO_PASS.success.len(), exception_class).
          - Exit 0 iff all sampled pairs match; otherwise exit non-zero with a JSON diff per
            mismatched pair.
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-05" priority="should-have">
    <narrative>
      <as_a>Outer-Loop Operator (UT-CORPUS-04)</as_a>
      <i_want_to>See aggregate stats about a generated corpus without paying the cost of a
      Docker re-run</i_want_to>
      <so_that>I can quickly answer questions like "how many KnownGood entries actually have
      `resolved: true` from the oracle?" and "is the split honoring leakage rules?".</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-05-A">
        <given>A persisted corpus at `./data/mejepa-corpus/`</given>
        <when>The operator runs `cargo run --release -p context-graph-mejepa-corpus -- stats
        --corpus ./data/mejepa-corpus`</when>
        <then>
          - Stdout JSON includes: total_entries, by_category {KnownGood: N, ...},
            by_oracle {resolved_true: N, resolved_false: N, exception_classes: {...}},
            by_bucket {Train: N, Calibration: N, Holdout: N}, leakage_check {passed: bool, mode}.
          - Exit 0 always; this is a read-only inspector.
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-06" priority="must-have">
    <narrative>
      <as_a>Outer-Loop Operator (UT-CORPUS-04)</as_a>
      <i_want_to>Have every Docker harness invocation reject Docker-name-unsafe `run_id`
      values fail-closed</i_want_to>
      <so_that>A typo in the CLI does not silently produce 2,400 artifacts under an unreadable
      Docker container name (real bug found by FSV: `+` is rejected by Docker container-name
      regex).</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-06-A">
        <given>A `SwebenchOracleConfig.run_id = "mejepa+v1"` (contains `+`, which Docker
        rejects)</given>
        <when>`run_swebench_lite_oracle(&config)` is called</when>
        <then>
          - The function returns `Err(MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE)` BEFORE invoking the
            harness.
          - The error message includes the offending character and the Docker name regex
            (`^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`).
          - No subprocess is spawned, no JSONL file is written.
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-07" priority="must-have">
    <narrative>
      <as_a>ME-JEPA Trainer (UT-CORPUS-01)</as_a>
      <i_want_to>Have the corpus refuse to ingest source code with parser errors</i_want_to>
      <so_that>Mutation operators never run on syntactically-broken input (which would produce
      meaningless mutated outputs and confuse Phase 3 training).</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-07-A">
        <given>A `primary_source` containing a Python syntax error (e.g., unclosed paren) and
        any `MutationCategory` other than `CompileError`</given>
        <when>`apply_mutation(category, primary_source, config)` is called</when>
        <then>
          - The function returns `Err(MEJEPA_CORPUS_INVALID_INPUT)` whose message includes the
            byte offset and node-kind diagnostics from the tree-sitter parse tree.
          - No mutation is applied; no output is written.
        </then>
      </criterion>
      <criterion id="AC-CORPUS-07-B">
        <given>A `primary_source` whose mutation site bounds (after operator selection) fall
        outside the source length OR whose `original_text` does not match the bytes at the
        recorded offset</given>
        <when>The internal `apply_site` validator runs</when>
        <then>The function returns `Err(MEJEPA_CORPUS_OPERATOR_FAILED)` with the offending
        offset, length, and original_text in the error message. No output is written.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-08" priority="must-have">
    <narrative>
      <as_a>ME-JEPA Trainer (UT-CORPUS-01)</as_a>
      <i_want_to>Be guaranteed that every (task_id, mutation_category) row's bucket assignment
      excludes the row's `test_patch_sha256` from any other bucket</i_want_to>
      <so_that>Holdout-set leakage at the test-patch level (the most common SWE-bench leakage
      mode) is structurally impossible.</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-08-A">
        <given>A `Vec<TaskAssignment>` from a successful split AND a
        `BTreeMap<task_id, test_patch_sha256>` derived from the SWE-bench Lite manifests</given>
        <when>`assert_no_test_patch_leakage(&assignments, &test_patch_shas)` is called</when>
        <then>
          - Returns Ok(()) iff no `test_patch_sha256` appears in two different buckets.
          - Returns `Err(MEJEPA_CORPUS_LEAKAGE_DETECTED)` with the offending sha and the two
            colliding (task_id, bucket) pairs otherwise.
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-09" priority="should-have">
    <narrative>
      <as_a>Outer-Loop Operator (UT-CORPUS-04)</as_a>
      <i_want_to>Get an FSV evidence file at
      `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json` after every CLI
      generate run</i_want_to>
      <so_that>I can inspect physical bytes-on-disk evidence of every (task_id, category) row
      independent of RocksDB and prove the run actually happened (forensic discipline).</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-09-A">
        <given>A successful `mejepa-corpus generate` run</given>
        <when>The CLI exits Ok</when>
        <then>
          - `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json` exists.
          - The JSON contains one row per generated entry with: task_id, category, patch_sha256,
            oracle_verdict_sha256, oracle_all_passed, oracle_exception, oracle_per_test_count,
            bucket.
          - File mode is 0o600 (per SEC-MEJEPA-07).
          - `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json` and `split-assignments.json`
            also exist with bucket sizes, fractions, and per-task assignment.
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-CORPUS-10" priority="nice-to-have">
    <narrative>
      <as_a>Outer-Loop Operator (UT-CORPUS-04)</as_a>
      <i_want_to>Eventually run the same eight mutation operators against Rust, JS, TS, Go, Java,
      C, C++, C#, Ruby, and PHP source extracted from each language's equivalent of SWE-bench Lite</i_want_to>
      <so_that>ME-JEPA-Code can answer the three operator questions (exists / works / impacts
      reality) for any supported language per doc 10 Section C.</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-CORPUS-10-A">
        <given>A `--languages rust` flag passed to `mejepa-corpus generate` (or any non-Python
        language) BEFORE the per-language operators are shipped</given>
        <when>The CLI starts</when>
        <then>The CLI exits non-zero with `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` and a message
        listing the currently-supported language set (only `python` today). No partial output is
        written.</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-CORPUS-01" story_ref="US-CORPUS-01" priority="must">
    <description>The crate `context-graph-mejepa-corpus` SHALL expose a public function
    `generate_swe_bench_mutation_corpus(swe_bench_lite_root: &Path, output_root: &Path,
    categories: &[MutationCategory], languages: &[Language]) -> Result<CorpusReport,
    MejepaCorpusError>` that produces 8 mutated entries per task per language with a
    fully-populated OracleVerdict for each.</description>
    <rationale>This is the doc 09 §3.1 contract — the primary supervision signal for ME-JEPA
    cannot be produced any other way.</rationale>
    <constraints>
      - Determinism: same inputs + same seed → byte-equal output (constitution
        `<anti_patterns reason="Determinism">`).
      - Fail-closed: never silently skip a task; partial runs must persist `complete: false`.
      - Cross-language: function signature accepts a `&[Language]`; today only
        `Language::Python` is allowed. Other variants return
        `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`.
    </constraints>
    <verification_method>
      - Unit: `cargo test -p context-graph-mejepa-corpus generate_full_corpus_python`.
      - FSV: `cargo run --release -p context-graph-mejepa-corpus --example
        mejepa_corpus_fsv` produces evidence JSON with 2,400 rows OR documents the
        `--max-tasks` subset.
      - End-to-end FSV against ≥ 2 mutation categories on at least one real Docker image
        (sympy__sympy-20590 baseline established 2026-05-09).
    </verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-02" story_ref="US-CORPUS-01" priority="must">
    <description>The crate SHALL implement the eight doc 09 §3.1 mutation operators
    (`KnownGood`, `SubtleFlip`, `OffByOne`, `SwapVariable`, `DeleteTestCall`, `WrongFile`,
    `OverEngineer`, `CompileError`), exposed via `apply_mutation(category, primary_source,
    config) -> MutationResult<MutationOutcome>`.</description>
    <rationale>Each operator produces a different oracle outcome class; the diversity is what
    makes the predictor generalize beyond one syntactic shape (doc 09 §3.1).</rationale>
    <constraints>
      - Each operator must respect a per-language lexical mask (Python today: comments,
        ordinary strings, triple-quoted docstrings, prefixed strings, and f-string literal
        text are NOT mutation sites; f-string replacement fields ARE exposed as code).
      - Byte-precise `MutationSite { byte_offset, byte_length, original_text,
        replacement_text, note }` is recorded for every in-place mutation; KnownGood and
        WrongFile may emit `mutation_site: None`.
      - `WrongFile` requires `MutationConfig.alternate_source: Some(_)`, else
        `MEJEPA_CORPUS_INVALID_INPUT`.
    </constraints>
    <verification_method>Unit tests in `src/tests.rs` (currently 81 tests). Per-language FSV
    runs the language's toolchain (py_compile for Python; rustc/javac/etc. for others when
    shipped) against every produced artifact.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-03" story_ref="US-CORPUS-02" priority="must">
    <description>The crate SHALL provide
    `run_swebench_lite_oracle(&SwebenchOracleConfig) -> OracleResult<SwebenchOracleResult>`
    that wraps `python -m swebench.harness.run_evaluation` for one instance, parses the
    per-instance `report.json`, and returns a canonical `OracleVerdict`.</description>
    <rationale>The ME-JEPA training loop needs a Rust-callable oracle; the Python harness is
    the only ground truth (constitution `<tech_stack>`).</rationale>
    <constraints>
      - `SwebenchPredictionMode::Gold` AND `SwebenchPredictionMode::Custom(SwebenchPrediction)`
        are both supported; gold reads from the dataset, custom writes a JSONL.
      - The artifact namespace MUST match the harness's expected layout
        `<run_root>/logs/run_evaluation/<run_id>/<model_name_or_path>/<instance_id>/{report.json,
        run.log, test_output.txt, patch.diff}`.
      - The function MUST NOT silently fall back to text parsing when `report.json` is missing.
    </constraints>
    <verification_method>End-to-end FSV against the `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`
    image in both gold and custom modes.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-04" story_ref="US-CORPUS-02" priority="must">
    <description>The verdict mapping SHALL implement the
    `swebench/harness/grading.py::get_eval_tests_report` rules: F2P.success / P2P.success /
    F2F.failure / P2F.failure → final `Pass`; F2P.failure / P2P.failure / F2F.success /
    P2F.success → final `Fail`. Patch-failure cases (patch_is_None, patch_exists=false,
    patch_successfully_applied=false) MAP to `exception=Other` and zero per_test rows.</description>
    <rationale>SWE-bench's leaderboard semantics are the only legitimate definition of
    "resolved"; we must not invent our own.</rationale>
    <constraints>
      - The `OracleVerdict.exception` field is preferred from `test_output.txt` parsing
        (`extract_exception_class`) when the harness ran but reported failures; it is set to
        `Other` only when the patch did not run.
      - The mapping is unit-tested against synthetic `tests_status` JSON covering all four
        quadrant transitions and the patch-failure paths.
    </constraints>
    <verification_method>Unit tests in `src/swebench.rs` named
    `swebench_report_to_verdict_*` cover the full 8-quadrant matrix.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-05" story_ref="US-CORPUS-06" priority="must">
    <description>The Docker `run_id` SHALL be validated against the regex
    `^[a-zA-Z0-9][a-zA-Z0-9_.-]*$` (Docker container-name rule) BEFORE any subprocess is
    spawned. Any character outside this class — including `+`, spaces, control chars,
    forward slash, colon — MUST cause an `MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE` error with
    the offending character cited.</description>
    <rationale>Real bug found by FSV (2026-05-09e): `+` in `run_id` is rejected by Docker's
    container-name regex; subprocess spawn succeeds but produces no artifacts. Fail-closed
    saves 2,400 silent failures.</rationale>
    <constraints>The validator is callable independently of `run_swebench_lite_oracle` for
    use in CLI argument validation.</constraints>
    <verification_method>Unit tests covering the rejected character set; integration test
    verifying no subprocess is spawned on invalid input.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-06" story_ref="US-CORPUS-03" priority="must">
    <description>The crate SHALL expose `split_corpus(entries: &[CorpusEntry], config:
    &SplitConfig) -> MutationResult<SplitReport>` with two modes: `SplitMode::InstanceAtomic`
    (default; per-task independent assignment) and `SplitMode::RepoAtomic` (whole-repo
    atomicity).</description>
    <rationale>InstanceAtomic achieves clean [240, 30, 30] on Lite; RepoAtomic provides
    stronger generalization separation but is structurally bounded by the dataset's repo-size
    distribution to [195, 77, 28] (memory/project_phase0_mejepa_mutation_corpus.md).</rationale>
    <constraints>
      - `train_fraction + calibration_fraction + holdout_fraction = 1.0 ± 1e-9`; any other
        sum returns `MEJEPA_CORPUS_INVALID_INPUT`.
      - Determinism: same `seed` + same input order → byte-equal `SplitReport`.
      - Bin-pack uses `lowest current/target ratio` with LPT pre-sort (descending unit size).
      - Empty corpus, NaN fractions, duplicate task_ids, or repo-units that exceed 100% of
        any bucket (RepoAtomic only) return `MEJEPA_CORPUS_INVALID_INPUT`.
    </constraints>
    <verification_method>Unit tests over both modes with the full 300-entry SWE-bench Lite
    manifest. FSV harness writes `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json`
    and `split-assignments.json`.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-07" story_ref="US-CORPUS-08" priority="must">
    <description>The crate SHALL provide `assert_no_test_patch_leakage(assignments:
    &[TaskAssignment], test_patch_shas: &BTreeMap<String, String>) -> MutationResult<()>` that
    verifies no `test_patch_sha256` appears in two different buckets.</description>
    <rationale>The SWE-bench-specific leakage failure mode: if `train` and `holdout` share a
    `test_patch_sha256`, the holdout reports inflated correlation. The assertion is the
    structural guard against that.</rationale>
    <constraints>The function returns `Err(MEJEPA_CORPUS_LEAKAGE_DETECTED)` with the
    offending sha and the colliding (task_id, bucket) pairs on detection. Called once per
    split via the `mejepa-corpus generate` and `verify` paths.</constraints>
    <verification_method>Unit test with hand-crafted leakage; integration test on the full
    300-task split.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-08" story_ref="US-CORPUS-01" priority="must">
    <description>The crate SHALL define and use three RocksDB column families:
    `CF_MEJEPA_MUTATION_CORPUS`, `CF_MEJEPA_ORACLE_VERDICTS`, and
    `CF_MEJEPA_CORPUS_PROVENANCE`. Schemas are defined in TECH-CORPUS `<data_models>`.</description>
    <rationale>Persistence is the durability guarantee; CFs let the corpus survive process
    restarts and let the verifier read it back independently.</rationale>
    <constraints>
      - CF names follow the constitution naming rule `CF_MEJEPA_*` (SCREAMING_SNAKE).
      - Keys for the first two CFs are `bincode::serialize(&(task_id: String, category:
        MutationCategory))`; the provenance CF key is `bincode::serialize(&(corpus_version:
        u32, embedder_version: u32))`.
      - Values are JSON for `MutationOutcome` and `OracleVerdict` (constitution rule: JSON
        for all provenance / SourceMetadata; bincode is forbidden for those because
        `skip_serializing_if` corrupts in bincode per memory key lessons).
    </constraints>
    <verification_method>Round-trip unit test: write → close DB → reopen → read; assert
    byte equality. FSV harness writes the corpus, closes the DB, reopens it, and re-reads
    every entry.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-09" story_ref="US-CORPUS-04" priority="must">
    <description>The CLI binary `mejepa-corpus` SHALL provide three subcommands:
    `generate`, `verify`, and `stats`, each operating on a corpus output root path.</description>
    <rationale>The CLI is the operator-facing surface; the subcommands cleanly separate the
    one-time generate batch from the read-only verify and stats inspectors.</rationale>
    <constraints>
      - `generate --output <path> --languages <lang,...> [--instance-id <id>]
        [--max-tasks <n>] [--seed <u64>] [--alternate-wrong-file-path <path>]`
        produces the corpus AND the `corpus-index.json`.
      - `verify --corpus <path>` re-runs Docker on a 5% sample (deterministic seed from
        corpus_sha) and exits non-zero on any verdict mismatch.
      - `stats --corpus <path>` produces a JSON report on stdout (read-only).
    </constraints>
    <verification_method>End-to-end CLI invocation against a real-Docker SWE-bench Lite
    image (sympy__sympy-20590 baseline).</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-10" story_ref="US-CORPUS-09" priority="should">
    <description>The crate SHALL ship two FSV example binaries:
    `examples/mejepa_corpus_fsv.rs` (corpus generation) and `examples/split_fsv.rs`
    (split helpers). Each writes physical evidence to
    `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/` and re-reads + asserts before reporting
    success.</description>
    <rationale>Constitution `<fsv_protocol>` rule: every shipped primitive ships with an FSV
    harness.</rationale>
    <constraints>
      - FSV files are created with mode 0o600 (SEC-MEJEPA-07).
      - The corpus FSV invokes `python3 -m py_compile` against every emitted Python
        artifact (real toolchain, not mock).
      - FSV exit code is non-zero on any assertion failure; outputs are preserved for
        inspection.
    </constraints>
    <verification_method>`cargo run --release -p context-graph-mejepa-corpus --example
    mejepa_corpus_fsv` AND `--example split_fsv` both exit 0 and produce the expected
    files at the canonical paths in `constitution.md` `<fsv_evidence_paths>`.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-11" story_ref="US-CORPUS-07" priority="must">
    <description>Mutation operators SHALL refuse parser-broken input via a
    `python_lex::ensure_parseable_python(source, field) -> MutationResult<()>` gate
    (per-language equivalents to be added when other languages ship). The gate uses
    `tree-sitter-python` and rejects any tree whose `root.has_error()` is true.</description>
    <rationale>Mutation operators on broken input produce meaningless outputs; the Phase 3
    trainer cannot distinguish "intentional CompileError" from "the corpus operator gave up".</rationale>
    <constraints>
      - The gate is bypassed only for `MutationCategory::CompileError`'s output (which IS
        intentionally broken) — but its INPUT must still parse.
      - Diagnostic message includes byte offset, line number, and node-kind.
    </constraints>
    <verification_method>Unit tests against malformed Python (unclosed paren, dangling
    `def`, mismatched indent).</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-12" story_ref="US-CORPUS-01" priority="must">
    <description>Every persisted corpus row SHALL carry provenance metadata: `corpus_sha`
    (SHA-256 over the sorted concatenation of all (task_id, category, mutated_source) tuples),
    `embedder_version` (frozen tag of the instrument set used to label the corpus),
    `frozen_at` (ISO 8601 UTC timestamp), and `code_version` (git SHA of the corpus crate
    HEAD).</description>
    <rationale>Constitution `<provenance>` rule: every persistent artifact carries provenance
    metadata. Without it, a stale corpus could silently be used to train a model that
    diverges from current Docker oracle semantics.</rationale>
    <constraints>The provenance row in `CF_MEJEPA_CORPUS_PROVENANCE` is written exactly once
    per `generate` invocation, after all 2,400 rows commit.</constraints>
    <verification_method>FSV evidence JSON contains the provenance row; its corpus_sha is
    re-computed from the persisted rows and asserted equal.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-13" story_ref="US-CORPUS-10" priority="should">
    <description>The crate SHALL define a `Language` enum mirroring the constitution's
    11-language matrix (Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp,
    Ruby, Php). Today only `Language::Python` is implemented; all others return
    `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`.</description>
    <rationale>Doc 10 Section C: ME-JEPA must answer the three operator questions for any
    supported language. The enum is the structural commitment; per-language operator
    implementations are deferred work tracked under `<out_of_scope>`.</rationale>
    <constraints>The error message lists the currently-supported language set so the
    operator knows which subset of `--languages` is safe.</constraints>
    <verification_method>Unit test: `apply_mutation` with `Language::Rust` returns
    `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-14" story_ref="US-CORPUS-02" priority="must">
    <description>SWE-bench harness invocations SHALL enforce both a per-instance timeout
    (default 1800s) AND an overall subprocess timeout (default 3600s). Exceeding either
    timeout MUST kill the harness subprocess and return
    `MEJEPA_ORACLE_TIMEOUT`.</description>
    <rationale>A hung Docker container can stall the corpus generator for hours; structural
    timeout is the only safe answer.</rationale>
    <constraints>Both timeouts are configurable via CLI flags
    (`--instance-timeout-secs`, `--overall-timeout-secs`). `wait_with_timeout` from
    `oracle.rs` provides the kill mechanic.</constraints>
    <verification_method>Unit test with a synthetic command that sleeps past the timeout;
    assert exit and elapsed time.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-15" story_ref="US-CORPUS-04" priority="should">
    <description>`mejepa-corpus verify` SHALL re-execute the Docker oracle on at least 5%
    of corpus rows (≥ 1 per category, deterministic sample seed = first 8 bytes of
    corpus_sha). Mismatches on `(resolved, FAIL_TO_PASS.success.len(),
    PASS_TO_PASS.success.len(), exception_class)` exit non-zero with a per-row diff.</description>
    <rationale>Detect silent corruption (RocksDB, Docker image drift, hand-edited corpus
    files) before downstream training depends on bad labels.</rationale>
    <constraints>The 5% sample size is configurable via `--sample-fraction`; minimum is 1
    per category. Failure on any sample row stops the run and exits non-zero.</constraints>
    <verification_method>Integration test: corrupt one row in
    `CF_MEJEPA_ORACLE_VERDICTS`, run `verify`, assert non-zero exit + diff in stdout.</verification_method>
  </requirement>

  <requirement id="REQ-CORPUS-16" story_ref="US-CORPUS-01" priority="must">
    <description>The corpus generator SHALL fail-closed on any non-zero exit from the
    Docker oracle for any single (task_id, mutation_category) pair. Partial corpora MAY be
    persisted with `corpus-index.json` setting `complete: false`, but the CLI exit code
    MUST be non-zero in that case.</description>
    <rationale>Silent partial completion is the most dangerous failure mode; downstream
    consumers must be able to trust the `complete: true` flag.</rationale>
    <constraints>An optional `--continue-on-oracle-failure` flag MAY be added later (out of
    scope for this spec); the default is fail-fast.</constraints>
    <verification_method>Integration test: stub one Docker invocation to exit non-zero;
    assert generator exits non-zero AND `complete: false` is set.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-CORPUS-01" req_ref="REQ-CORPUS-01">
    <condition>WHEN `mejepa-corpus generate` is invoked with `--languages` containing any
    language other than `python`</condition>
    <action>The CLI MUST exit non-zero with `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`. NO
    partial output is written.</action>
    <exception>None. Per-language deferral is total; no silent skips.</exception>
  </rule>

  <rule id="BR-CORPUS-02" req_ref="REQ-CORPUS-04">
    <condition>WHEN the SWE-bench harness reports `patch_is_None=true` OR
    `patch_exists=false` OR `patch_successfully_applied=false`</condition>
    <action>The verdict MUST be constructed with `per_test = Vec::new()` AND
    `exception = Some(ExceptionClass::Other)`. The `resolved` flag is honored as-is
    (the harness sets it to false in these cases).</action>
    <exception>If `extract_exception_class(test_output.txt)` returns
    `Some(specific_class)`, that class is preferred over `Other` (the harness ran past
    patch-application and produced typed failure output).</exception>
  </rule>

  <rule id="BR-CORPUS-03" req_ref="REQ-CORPUS-05">
    <condition>WHEN any character outside `[a-zA-Z0-9_.-]` appears in `run_id`, OR the
    first character is not `[a-zA-Z0-9]`</condition>
    <action>RETURN `MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE` with the offending character and
    the regex cited.</action>
    <exception>None. The regex is structural Docker policy, not negotiable.</exception>
  </rule>

  <rule id="BR-CORPUS-04" req_ref="REQ-CORPUS-06">
    <condition>WHEN `SplitMode::RepoAtomic` is requested AND any single repo's task count
    exceeds the smallest `bucket_target` (i.e., a repo cannot fit in any bucket without
    overshoot)</condition>
    <action>WARN on stderr that exact ratios are unreachable, but proceed with the
    minimum-deviation assignment. The `SplitReport.bucket_sizes` reflects the achievable
    optimum.</action>
    <exception>If a single repo's task count exceeds the corpus size (impossible by
    construction; sentinel guard), RETURN `MEJEPA_CORPUS_INVALID_INPUT`.</exception>
  </rule>

  <rule id="BR-CORPUS-05" req_ref="REQ-CORPUS-07">
    <condition>WHEN `assert_no_test_patch_leakage` finds `test_patch_sha256` X in both
    `Train` and any other bucket</condition>
    <action>RETURN `MEJEPA_CORPUS_LEAKAGE_DETECTED { sha: X, collisions: Vec<(task_id,
    bucket)> }`. Caller MUST treat this as a fatal Phase 0 bug, not a warning.</action>
    <exception>None. Leakage is structurally incompatible with valid Phase 8
    correlation reporting.</exception>
  </rule>

  <rule id="BR-CORPUS-06" req_ref="REQ-CORPUS-12">
    <condition>WHEN a `generate` run completes with `complete: true`</condition>
    <action>Write exactly one row into `CF_MEJEPA_CORPUS_PROVENANCE` keyed by
    `(corpus_version=1, embedder_version=<frozen>)`, value = `CorpusProvenance {
    corpus_sha, frozen_at, entry_count, code_version, language_set }`.</action>
    <exception>If `complete: false`, NO provenance row is written. The corpus_sha for an
    incomplete corpus is undefined.</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-CORPUS-01" req_ref="REQ-CORPUS-02">
    <scenario>SubtleFlip operator runs on a Python source containing only one boolean
    operator, but that operator is inside a triple-quoted docstring (lexically masked).</scenario>
    <expected_behavior>RETURN `MEJEPA_CORPUS_NO_MUTATION_SITE` with a message naming the
    operator and the masked-region rule. The lexical mask is non-negotiable.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-02" req_ref="REQ-CORPUS-02">
    <scenario>OffByOne operator runs on Python source containing the function arrow `->`.
    Naive scanning would see `>` as a comparison operator (real bug found 2026-05-09).</scenario>
    <expected_behavior>The lexer must classify `->` as a single token (Python function-return
    arrow), NOT as `-` followed by `>`. The OffByOne site selector MUST skip it.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-03" req_ref="REQ-CORPUS-02">
    <scenario>OffByOne operator runs on Python source containing a float literal `3.14`.
    Naive integer scanning would extract `3` and `14` (real bug found 2026-05-09).</scenario>
    <expected_behavior>The lexer recognizes `3.14` as a single float literal; OffByOne must
    not extract fractional digits.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-04" req_ref="REQ-CORPUS-06">
    <scenario>SplitConfig fractions sum to `1.0 + 1e-10` due to f64 round-trip from JSON
    (real bug found 2026-05-09b).</scenario>
    <expected_behavior>The validator MUST tolerate exactly `±1e-9` deviation; tighter
    rejection breaks legitimate JSON round-trips.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-05" req_ref="REQ-CORPUS-06">
    <scenario>SplitConfig requested target ratios `[0.8, 0.1, 0.1]` against a 25-repo,
    300-task SWE-bench Lite corpus in RepoAtomic mode where `django` alone is 38% of tasks.</scenario>
    <expected_behavior>The bin-packer produces the achievable optimum (`[195, 77, 28]` per
    measured baseline); `bucket_fractions` reflects reality, NOT the requested target;
    stderr warning is emitted; `leakage_check_passed = true`.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-06" req_ref="REQ-CORPUS-03">
    <scenario>The Docker daemon is unreachable when `run_swebench_lite_oracle` is called
    (e.g., dockerd crashed mid-run).</scenario>
    <expected_behavior>RETURN `MEJEPA_ORACLE_COMMAND_FAILED` with the harness exit code,
    stdout tail, and stderr tail. NO partial verdict is constructed; the corpus row is
    not written.</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-07" req_ref="REQ-CORPUS-04">
    <scenario>The harness produces a `report.json` whose root has more than one key (impossible
    in practice but theoretically possible if two reports are concatenated).</scenario>
    <expected_behavior>RETURN `MEJEPA_ORACLE_REPORT_INVALID { field: "root", message: "root
    must contain exactly one key" }`. NO key is silently picked.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-08" req_ref="REQ-CORPUS-02">
    <scenario>WrongFile mutation is requested without `MutationConfig.alternate_source`
    being set.</scenario>
    <expected_behavior>RETURN `MEJEPA_CORPUS_INVALID_INPUT` with field
    `alternate_source` and remediation "pass the path/content of a different but
    structurally-similar file". NO mutation is attempted.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-09" req_ref="REQ-CORPUS-08">
    <scenario>`CF_MEJEPA_MUTATION_CORPUS` write succeeds but the subsequent
    `CF_MEJEPA_ORACLE_VERDICTS` write fails (RocksDB partial-write).</scenario>
    <expected_behavior>The two writes MUST be wrapped in a single RocksDB WriteBatch;
    failure on either rolls back both. NO orphan corpus row may survive.</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-10" req_ref="REQ-CORPUS-15">
    <scenario>`verify` is run AFTER the SWE-bench Docker images have been re-pulled with
    upstream-changed test contents (Docker image drift).</scenario>
    <expected_behavior>The verifier reports the mismatched rows and exits non-zero. The
    operator's remediation is to regenerate the corpus with the new images and bump
    `embedder_version` (or simply `corpus_version`).</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-11" req_ref="REQ-CORPUS-02">
    <scenario>SwapVariable is asked to rename variables in source containing zero local
    variables (e.g., a module-level constants file).</scenario>
    <expected_behavior>RETURN `MEJEPA_CORPUS_NO_MUTATION_SITE`. NO mutation; the operator
    chain logs a warning and the corpus generator records the (task_id, category) entry
    as `null` mutated_source with `MEJEPA_CORPUS_NO_MUTATION_SITE` as the reason. The
    Phase 3 trainer skips null-source rows.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-12" req_ref="REQ-CORPUS-09">
    <scenario>`mejepa-corpus generate` is interrupted (Ctrl-C) mid-run after 1,200 of
    2,400 entries have been written.</scenario>
    <expected_behavior>The CLI traps SIGINT, flushes outstanding RocksDB writes,
    persists the partial `corpus-index.json` with `complete: false`, and exits non-zero.
    Subsequent `generate` runs MUST be re-run from scratch into a fresh output root
    (no resume support in v1).</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-CORPUS-13" req_ref="REQ-CORPUS-13">
    <scenario>A future PR adds Rust mutation operators but does not update `Language`
    enum's `match` arms.</scenario>
    <expected_behavior>The Rust compiler's exhaustiveness check on the `Language` match
    surfaces the missing arm at compile time. NO silent fallback path may exist.</expected_behavior>
    <priority>high</priority>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-CORPUS-01" http_code="400" req_ref="REQ-CORPUS-11">
    <condition>Mutation operator called with primary source that does not parse against
    the language's tree-sitter grammar (`root.has_error() == true`).</condition>
    <user_message>Mutation input invalid at primary_source: source has parse errors at
    byte offset {N}, line {L} (node-kind: {K}); remediation: supply syntactically valid
    source for category {C}.</user_message>
    <internal_message>code=MEJEPA_CORPUS_INVALID_INPUT; tree-sitter ERROR/MISSING node
    detected; do NOT fall back to text scanning.</internal_message>
    <recovery_action>Caller fixes the source; mutation operators NEVER repair input.</recovery_action>
  </error>

  <error id="ERR-CORPUS-02" http_code="500" req_ref="REQ-CORPUS-02">
    <condition>Mutation site located by an operator has byte_offset + byte_length
    overflowing usize, range outside source length, range not aligned to UTF-8 character
    boundaries, OR `original_text` does not match `source[byte_offset..byte_offset +
    byte_length]`.</condition>
    <user_message>Mutation operator failed at mutation_site: site range {B}..{E} invalid
    (reason); remediation: report the operator that produced the invalid site.</user_message>
    <internal_message>code=MEJEPA_CORPUS_OPERATOR_FAILED; this is an operator bug, not a
    source bug.</internal_message>
    <recovery_action>Issue a bug report; abort the corpus generation; do NOT silently
    skip.</recovery_action>
  </error>

  <error id="ERR-CORPUS-03" http_code="400" req_ref="REQ-CORPUS-05">
    <condition>`SwebenchOracleConfig.run_id` contains characters outside
    `[a-zA-Z0-9_.-]` OR starts with a non-alphanumeric character.</condition>
    <user_message>Docker run_id "{run_id}" is unsafe: character '{c}' at position {i}
    violates Docker name regex `^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`; remediation: pick a name
    matching the regex.</user_message>
    <internal_message>code=MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE; subprocess NOT spawned;
    no JSONL written.</internal_message>
    <recovery_action>Caller supplies a Docker-name-safe run_id (typical: replace `+`
    with `-`).</recovery_action>
  </error>

  <error id="ERR-CORPUS-04" http_code="500" req_ref="REQ-CORPUS-04">
    <condition>The per-instance `report.json` cannot be parsed as JSON, has a wrong
    instance_id, has a non-object root, or contains more than one top-level key.</condition>
    <user_message>SWE-bench report.json invalid at field "{field}": {message};
    remediation: do not edit harness output by hand.</user_message>
    <internal_message>code=MEJEPA_CORPUS_REPORT_PARSE_FAIL (mapped from
    MEJEPA_ORACLE_JSON_PARSE / MEJEPA_ORACLE_REPORT_INVALID).</internal_message>
    <recovery_action>Inspect the file at the path cited; preserve for forensic
    analysis; re-run after upstream harness fix.</recovery_action>
  </error>

  <error id="ERR-CORPUS-05" http_code="500" req_ref="REQ-CORPUS-07">
    <condition>`assert_no_test_patch_leakage` finds at least one `test_patch_sha256`
    appearing in two different buckets.</condition>
    <user_message>Test-patch leakage detected: sha256 {SHA} appears in both buckets
    {B1} (task {T1}) and {B2} (task {T2}); remediation: regenerate the split with a
    different seed or switch to RepoAtomic mode.</user_message>
    <internal_message>code=MEJEPA_CORPUS_LEAKAGE_DETECTED; treat as fatal Phase 0 bug,
    not a warning.</internal_message>
    <recovery_action>Caller MUST regenerate the split; partial recovery is not
    safe.</recovery_action>
  </error>

  <error id="ERR-CORPUS-06" http_code="500" req_ref="REQ-CORPUS-14">
    <condition>The harness subprocess exceeds `overall_timeout` (default 3600s) OR an
    individual instance exceeds `instance_timeout` (default 1800s).</condition>
    <user_message>SWE-bench harness timed out after {T}s for instance {I}; remediation:
    inspect Docker daemon state and resource limits.</user_message>
    <internal_message>code=MEJEPA_ORACLE_TIMEOUT; subprocess SIGKILLed; partial
    artifacts MAY exist and SHOULD be inspected.</internal_message>
    <recovery_action>Caller increases timeout OR fixes the underlying Docker
    issue.</recovery_action>
  </error>

  <error id="ERR-CORPUS-07" http_code="400" req_ref="REQ-CORPUS-13">
    <condition>`mejepa-corpus generate --languages` includes a language other than
    `python` (today; will expand as operators ship).</condition>
    <user_message>Unsupported language "{lang}"; current supported set: [python];
    remediation: drop the unsupported flag values OR wait for the language to ship.</user_message>
    <internal_message>code=MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE; CLI exits non-zero
    BEFORE any output is written.</internal_message>
    <recovery_action>Caller restricts `--languages` to the currently-supported set.</recovery_action>
  </error>

  <error id="ERR-CORPUS-08" http_code="500" req_ref="REQ-CORPUS-08">
    <condition>RocksDB write fails during `CF_MEJEPA_MUTATION_CORPUS` or
    `CF_MEJEPA_ORACLE_VERDICTS` commit (disk full, IO error, CF missing).</condition>
    <user_message>Oracle store write failed at {operation}/{cf}: {message}; remediation:
    inspect disk space, RocksDB integrity, and CF configuration.</user_message>
    <internal_message>code=MEJEPA_ORACLE_STORE; the WriteBatch was rolled back; no
    partial corpus row exists.</internal_message>
    <recovery_action>Operator addresses the underlying storage issue; re-runs
    `generate`.</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="MutationCategory">
    <field name="variant" type="enum" required="true">
      <description>One of: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall,
      WrongFile, OverEngineer, CompileError.</description>
      <constraints>Serde rename_all = snake_case; total set fixed by doc 09 §3.1.</constraints>
    </field>
  </entity>

  <entity name="MutationOutcome">
    <field name="category" type="MutationCategory" required="true">
      <description>The category that produced this outcome.</description>
      <constraints>Must match the requested category; no operator may silently substitute.</constraints>
    </field>
    <field name="mutated_source" type="string" required="true">
      <description>The post-mutation source as a UTF-8 string.</description>
      <constraints>Non-empty. For `KnownGood`: byte-equal to input.</constraints>
    </field>
    <field name="seed" type="u64" required="true">
      <description>The seed used by the operator's PRNG (SplitMix64).</description>
      <constraints>Echoed from MutationConfig.seed. Determinism gate.</constraints>
    </field>
    <field name="mutation_site" type="Option&lt;MutationSite&gt;" required="false">
      <description>Byte-precise location of the in-place mutation. None for KnownGood and
      WrongFile.</description>
      <constraints>If present: byte_offset + byte_length ≤ source.len(); UTF-8
      char-aligned; original_text equals source slice at the recorded range.</constraints>
    </field>
  </entity>

  <entity name="OracleVerdict">
    <field name="per_test" type="Vec&lt;PerTestOutcome&gt;" required="true">
      <description>Per-test results parsed from SWE-bench tests_status. Empty when patch
      did not apply.</description>
      <constraints>Each `PerTestOutcome` has a unique `test_id` within the verdict.</constraints>
    </field>
    <field name="exception" type="Option&lt;ExceptionClass&gt;" required="false">
      <description>The dominant exception class extracted from test_output.txt OR
      derived from harness state (Other when patch did not apply).</description>
      <constraints>Per `context-graph-mejepa-instruments::ExceptionClass`.</constraints>
    </field>
    <field name="resolved" type="bool" required="true">
      <description>Mirrors the harness's `resolved` field per
      `swebench/harness/grading.py` semantics.</description>
      <constraints>True iff F2P.success covers the entire F2P set AND P2P.success covers
      the entire P2P set.</constraints>
    </field>
    <field name="runtime_seconds" type="f64" required="true">
      <description>Elapsed wall-clock time of the harness invocation.</description>
      <constraints>Non-negative finite f64.</constraints>
    </field>
  </entity>

  <entity name="CorpusEntry">
    <field name="task_id" type="string" required="true">
      <description>SWE-bench Lite instance_id (e.g., `astropy__astropy-12907`).</description>
      <constraints>Non-empty; matches the instance_id in `tasks/swebench-lite/*.json`.</constraints>
    </field>
    <field name="repo" type="string" required="true">
      <description>Owning repo slug (e.g., `astropy`).</description>
      <constraints>Non-empty; consistent across all tasks of the same repo.</constraints>
    </field>
  </entity>

  <entity name="TaskAssignment">
    <field name="task_id" type="string" required="true"><description>Echoes
    CorpusEntry.task_id.</description><constraints>Non-empty.</constraints></field>
    <field name="repo" type="string" required="true"><description>Echoes
    CorpusEntry.repo.</description><constraints>Non-empty.</constraints></field>
    <field name="bucket" type="SplitBucket" required="true">
      <description>One of `Train | Calibration | Holdout`.</description>
      <constraints>Total over a corpus matches `SplitReport.bucket_sizes`.</constraints>
    </field>
  </entity>

  <entity name="CorpusProvenance">
    <field name="corpus_sha" type="string" required="true">
      <description>SHA-256 (hex) over the sorted concatenation of all (task_id, category,
      mutated_source) tuples.</description>
      <constraints>64-char lowercase hex.</constraints>
    </field>
    <field name="embedder_version" type="u32" required="true"><description>Frozen tag of
    the instrument set in use at corpus generation time.</description><constraints>Monotonic
    non-decreasing.</constraints></field>
    <field name="frozen_at" type="string" required="true"><description>ISO 8601 UTC
    timestamp.</description><constraints>Pattern
    `\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z`.</constraints></field>
    <field name="entry_count" type="u32" required="true"><description>Total entries
    persisted in this corpus.</description><constraints>Equals 8 ×
    |selected_tasks|.</constraints></field>
    <field name="code_version" type="string" required="true"><description>Git SHA of the
    corpus crate HEAD at generation time.</description><constraints>40-char lowercase
    hex.</constraints></field>
    <field name="language_set" type="Vec&lt;Language&gt;" required="true"><description>Languages
    represented in this corpus.</description><constraints>Non-empty; subset of the
    11-language matrix.</constraints></field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-CORPUS-01" category="performance">
    <description>End-to-end Docker oracle invocation per (task_id, mutation_category) on a
    pre-built image SHALL complete within p95 60s on the operator's reference hardware.</description>
    <metric>p95 60s; baseline observed gold mode 23.7s and custom mode 23.0s on
    `sympy_1776_sympy-20590` (22 tests, 2026-05-09).</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-CORPUS-02" category="performance">
    <description>Full corpus generation for 300 SWE-bench Lite tasks × 8 categories =
    2,400 entries SHALL complete within p95 80 hours wall-clock on the operator's
    reference hardware (2400 × 60s sequential ≈ 40h; allow ×2 buffer for Docker
    overhead and parallelism gaps).</description>
    <metric>p95 80 hours wall-clock for the full 2,400-entry batch.</metric>
    <priority>could</priority>
  </nfr>

  <nfr id="NFR-CORPUS-03" category="reliability">
    <description>Mutation operators are deterministic: same `MutationConfig.seed` + same
    `primary_source` + same `MutationCategory` → byte-equal `MutationOutcome.mutated_source`.</description>
    <metric>100% byte-equality across two consecutive runs (FSV asserts).</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-CORPUS-04" category="reliability">
    <description>Repo-atomic split is deterministic: same `SplitConfig.seed` + same input
    `Vec<CorpusEntry>` → byte-equal `SplitReport`.</description>
    <metric>100% byte-equality across two consecutive runs.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-CORPUS-05" category="security">
    <description>Docker `run_id` is validated against the constitutional regex BEFORE any
    subprocess spawn (defense in depth against shell-injection / cgroup-traversal in
    Docker container names).</description>
    <metric>0 subprocess spawns observed for invalid run_id inputs in the unit-test
    suite.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-CORPUS-06" category="security">
    <description>FSV evidence files in `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/` are created
    with mode 0o600 (per SEC-MEJEPA-07 of the constitution).</description>
    <metric>`stat -c %a <file>` returns `600` for every FSV evidence file.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-CORPUS-07" category="usability">
    <description>The CLI `mejepa-corpus generate --help` SHALL list every flag with a
    one-line description AND a usage example for the most common path
    (`--output ./data/mejepa-corpus --languages python`).</description>
    <metric>Manual inspection: `--help` output contains all flags and an example.</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-CORPUS-08" category="reliability">
    <description>The corpus generator is fail-closed: on any non-zero Docker exit OR any
    structural validation failure, the generator surfaces the error and persists
    `complete: false` in the corpus-index.json. NO partial-corpus row may have its
    presence inferred from a `complete: true` flag.</description>
    <metric>Integration test: stub one Docker invocation to fail; assert `complete:
    false` AND non-zero CLI exit AND no corruption in CF_MEJEPA_CORPUS_PROVENANCE.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-CORPUS-09" category="auditability">
    <description>Every persisted corpus row SHALL be re-readable independently of the
    generator code via standard RocksDB tooling. The schema is documented in
    `TECH-CORPUS.md` `<data_models>` and stable for the life of `corpus_version=1`.</description>
    <metric>Manual smoke: `rocksdb_ldb scan --column_family=mejepa_oracle_verdicts`
    produces parseable JSON values.</metric>
    <priority>should</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="external">
    <name>SWE-bench Python harness (`swebench.harness.run_evaluation`)</name>
    <description>The official ground-truth oracle. Pinned to the swebench package version
    in the operator's swebench venv; corpus_version is bumped on harness updates.</description>
    <impact>If the harness changes its `report.json` schema, REQ-CORPUS-04's mapping
    rules must update; until they do, the corpus is invalid.</impact>
  </dependency>

  <dependency type="external">
    <name>Docker daemon + SWE-bench Lite Docker images
    (`swebench/sweb.eval.x86_64.<repo>__<owner>-<id>:latest`)</name>
    <description>The execution substrate for the harness. Each instance has its own
    pre-built image.</description>
    <impact>If Docker is unavailable or images cannot be pulled, oracle invocations
    fail with `MEJEPA_ORACLE_COMMAND_FAILED`; corpus generation halts.</impact>
  </dependency>

  <dependency type="external">
    <name>tree-sitter grammars (Python today; 10 others when shipped)</name>
    <description>The parser-backed input gate (REQ-CORPUS-11) requires a tree-sitter
    grammar for every supported language.</description>
    <impact>If the grammar is missing or rejects a legitimate file, the operator cannot
    mutate that file. Partial corpora MUST persist `complete: false`.</impact>
  </dependency>

  <dependency type="internal">
    <name>`context-graph-mejepa-instruments::OracleVerdict` + `ExceptionClass` +
    `PerTestOutcome` + `TestOutcome`</name>
    <description>The canonical types consumed by the Phase 1 `EOracleInstrument`. The
    SWE-bench bridge maps harness output into these types.</description>
    <impact>Schema changes in `context-graph-mejepa-instruments` cascade to corpus row
    layout; plan a coordinated bump.</impact>
  </dependency>

  <dependency type="internal">
    <name>`context-graph-mincut` (Stoer-Wagner)</name>
    <description>The repo-atomic split's structural fairness check uses Stoer-Wagner
    mincut on the (task → repo) bipartite graph.</description>
    <impact>If the mincut crate ships a regression, the RepoAtomic split silently
    degrades. Mitigation: leakage assertions are computed independently of the mincut
    and provide a final structural guard.</impact>
  </dependency>

  <dependency type="external">
    <name>RocksDB (rocksdb crate v0.21.x per constitution)</name>
    <description>The persistence substrate.</description>
    <impact>If RocksDB version mismatches or CF descriptors drift, corpus reads fail
    with `MEJEPA_ORACLE_STORE`. Wipe-DB-on-schema-change applies (per
    memory/feedback_no_backwards_compat.md).</impact>
  </dependency>

</dependencies>

<out_of_scope>
  <item>Per-language mutation operators for Rust, JavaScript, TypeScript, Go, Java, C, C++,
  C#, Ruby, PHP — handled by future Phase 1 extension specs (one per language) and
  governed by REQ-CORPUS-13's structural commitment.</item>
  <item>The `mejepa-shift-subscriber` (Phase 7 §3.8) that consumes outer-loop edits in
  real time and feeds `mejepa.observe()` — handled by SPEC-SHIFT-SUBSCRIBER (deferred).</item>
  <item>The Phase 4b TCT constellation construction that uses the corpus's labeled
  centroids — handled by SPEC-TCT (deferred).</item>
  <item>Synthetic adversarial perturbations in the panel latent space (doc 09 §3.3) —
  these are generated on-the-fly during Phase 3 training, not persisted to the corpus
  CFs.</item>
  <item>The Phase 5 self-healing online `observe()` loop that adds new entries to the
  corpus from outer-loop activity — handled by SPEC-SELF-HEALING (deferred).</item>
  <item>RocksDB column families OTHER than the three named in REQ-CORPUS-08 (e.g.,
  `CF_MEJEPA_PANELS`, `CF_MEJEPA_TRAIN_CERTS`) — owned by their respective phase specs.</item>
  <item>Resume support for `mejepa-corpus generate` — v1 requires re-running into a
  fresh output root after any interruption.</item>
  <item>Active-learning loop that requests fresh oracle runs for under-confident
  predictions — Phase 5 work.</item>
</out_of_scope>

<test_plan>

  <test_case id="TC-CORPUS-01" type="unit" req_ref="REQ-CORPUS-02">
    <description>Each mutation operator returns a deterministic outcome on a fixed
    Python source with seed=0.</description>
    <preconditions>Test fixture: a real Python source from the SWE-bench Lite corpus.</preconditions>
    <inputs>category ∈ all 8; primary_source = fixture; seed = 0.</inputs>
    <expected_result>Two consecutive `apply_mutation` calls produce byte-equal
    `MutationOutcome.mutated_source`.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-02" type="unit" req_ref="REQ-CORPUS-02">
    <description>SubtleFlip respects the Python lexical mask (no mutation inside
    docstrings or string literals).</description>
    <preconditions>Source: a Python function with a triple-quoted docstring containing
    `True`.</preconditions>
    <inputs>category=SubtleFlip; primary_source=docstring fixture; seed=0.</inputs>
    <expected_result>Returns `MEJEPA_CORPUS_NO_MUTATION_SITE` (the docstring `True` is
    masked out and there is no other boolean operator).</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-03" type="unit" req_ref="REQ-CORPUS-02">
    <description>OffByOne does not mutate `->` or floats (regression for 2026-05-09 bugs).</description>
    <preconditions>Source: a Python function with `def f(x: int) -> int:` and a float
    literal `3.14`.</preconditions>
    <inputs>category=OffByOne; seeds 0..10.</inputs>
    <expected_result>For every seed, the mutation_site does NOT lie inside `->` or the
    fractional part of `3.14`.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-04" type="unit" req_ref="REQ-CORPUS-04">
    <description>`swebench_report_to_verdict` correctly maps all 8 transition quadrants
    AND the three patch-failure cases.</description>
    <preconditions>Synthetic `SwebenchInstanceReport` fixtures covering every quadrant.</preconditions>
    <inputs>11 fixtures: F2P.success, F2P.failure, P2P.success, P2P.failure,
    F2F.success, F2F.failure, P2F.success, P2F.failure, patch_is_None,
    patch_exists=false, patch_successfully_applied=false.</inputs>
    <expected_result>Each fixture produces an `OracleVerdict` consistent with
    `swebench/harness/grading.py::get_eval_tests_report`.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-05" type="unit" req_ref="REQ-CORPUS-05">
    <description>`run_id` validation rejects unsafe characters BEFORE subprocess spawn.</description>
    <preconditions>None.</preconditions>
    <inputs>run_ids: "mejepa+v1", "mejepa v1", "mejepa\\nv1", "mejepa/v1",
    ".mejepa-v1", "1mejepa-v1" (the last 2 violate the "first char alnum" rule).</inputs>
    <expected_result>All return `MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE`. No subprocess
    spawned (verified by absence of harness side effects).</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-06" type="unit" req_ref="REQ-CORPUS-06">
    <description>InstanceAtomic split achieves clean [240, 30, 30] on the 300-task
    SWE-bench Lite corpus.</description>
    <preconditions>Corpus loaded from `tasks/swebench-lite/*.json`.</preconditions>
    <inputs>SplitConfig::default().</inputs>
    <expected_result>SplitReport.bucket_sizes = [240, 30, 30]; leakage_check_passed =
    true.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-07" type="unit" req_ref="REQ-CORPUS-06">
    <description>RepoAtomic split is structurally bounded; reports the achievable
    optimum.</description>
    <preconditions>Same corpus.</preconditions>
    <inputs>SplitConfig with `mode = SplitMode::RepoAtomic`.</inputs>
    <expected_result>SplitReport.bucket_sizes is a valid whole-repo partition of 300
    (e.g., [195, 77, 28]); leakage_check_passed = true; stderr emits the bounded-fidelity
    warning.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-08" type="unit" req_ref="REQ-CORPUS-07">
    <description>`assert_no_test_patch_leakage` detects fabricated leakage.</description>
    <preconditions>Hand-crafted assignments with `task_id` X in Train and Y in Holdout
    sharing `test_patch_sha256`.</preconditions>
    <inputs>The crafted assignments + sha map.</inputs>
    <expected_result>Returns `MEJEPA_CORPUS_LEAKAGE_DETECTED` with both task_ids
    cited.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-09" type="integration" req_ref="REQ-CORPUS-03">
    <description>End-to-end SWE-bench Docker oracle invocation in BOTH gold and custom
    modes against `sympy__sympy-20590`.</description>
    <preconditions>Docker daemon running; image
    `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest` pulled; swebench Python
    venv at the operator-supplied path.</preconditions>
    <inputs>Two `SwebenchOracleConfig` invocations (gold + custom).</inputs>
    <expected_result>Both return `Ok(SwebenchOracleResult)` with `instance_report.resolved
    == true`, 22 tests reported in F2P.success, exit code 0; runtime &lt; 60s p95.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-10" type="integration" req_ref="REQ-CORPUS-08">
    <description>RocksDB round-trip: write corpus row, close DB, reopen, read.</description>
    <preconditions>tempdir RocksDB.</preconditions>
    <inputs>One synthetic (task_id, category, mutated_source, oracle_verdict)
    tuple.</inputs>
    <expected_result>Re-read row is byte-equal to the written row (both
    CFs).</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-CORPUS-11" type="e2e" req_ref="REQ-CORPUS-01,REQ-CORPUS-09">
    <description>Smoke run of `mejepa-corpus generate --max-tasks 2 --languages python`
    against real Docker.</description>
    <preconditions>As TC-CORPUS-09.</preconditions>
    <inputs>tempdir for `--output`; default flags otherwise.</inputs>
    <expected_result>Exit 0; corpus-index.json contains 16 entries (2 tasks × 8
    categories); FSV evidence at
    `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json` mirrors the
    persisted rows.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-12" type="e2e" req_ref="REQ-CORPUS-15">
    <description>`mejepa-corpus verify` re-runs Docker on a 5% sample and reports
    matching verdicts.</description>
    <preconditions>The corpus produced by TC-CORPUS-11 still on disk.</preconditions>
    <inputs>--corpus &lt;the corpus dir&gt;; default sample fraction.</inputs>
    <expected_result>Exit 0; stdout JSON reports `mismatches: []` and
    `sampled_pairs: ≥ 1 per category`.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-13" type="e2e" req_ref="REQ-CORPUS-15">
    <description>`mejepa-corpus verify` detects a hand-corrupted oracle verdict.</description>
    <preconditions>The corpus from TC-CORPUS-11. Corrupt one row in
    CF_MEJEPA_ORACLE_VERDICTS to `resolved=false` for a row that is actually true.</preconditions>
    <inputs>--corpus.</inputs>
    <expected_result>Exit non-zero; stdout JSON reports the offending (task_id,
    category) pair and the diff.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-14" type="manual" req_ref="REQ-CORPUS-13">
    <description>`mejepa-corpus generate --languages rust` exits cleanly without
    spawning anything.</description>
    <preconditions>None.</preconditions>
    <inputs>--languages rust.</inputs>
    <expected_result>Exit non-zero; stderr cites
    `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` and lists `python` as the supported
    set.</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-CORPUS-15" type="unit" req_ref="REQ-CORPUS-11">
    <description>Mutation operators reject syntactically invalid Python.</description>
    <preconditions>Source: `def broken(:` (unclosed paren).</preconditions>
    <inputs>Every category EXCEPT `KnownGood` (which echoes input
    unchanged).</inputs>
    <expected_result>All return `MEJEPA_CORPUS_INVALID_INPUT` with a tree-sitter
    diagnostic message.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-16" type="unit" req_ref="REQ-CORPUS-12">
    <description>CorpusProvenance.corpus_sha is reproducible.</description>
    <preconditions>Two consecutive successful `generate` runs into different output
    roots with the same `--seed` AND same input manifest.</preconditions>
    <inputs>Both runs.</inputs>
    <expected_result>Both `CorpusProvenance.corpus_sha` values are byte-equal.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-17" type="unit" req_ref="REQ-CORPUS-14">
    <description>Oracle invocation respects `overall_timeout`.</description>
    <preconditions>A stub harness command that sleeps past the timeout.</preconditions>
    <inputs>SwebenchOracleConfig with `overall_timeout = Duration::from_secs(2)`.</inputs>
    <expected_result>Returns `MEJEPA_ORACLE_TIMEOUT` within 2.5s; subprocess is
    confirmed killed (no zombie).</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-CORPUS-18" type="manual" req_ref="REQ-CORPUS-10">
    <description>FSV evidence files have the correct mode 0o600 per
    SEC-MEJEPA-07.</description>
    <preconditions>Run any FSV harness that writes
    `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/*`.</preconditions>
    <inputs>None.</inputs>
    <expected_result>`stat -c %a` for every produced file returns `600`.</expected_result>
    <priority>medium</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-CORPUS-01" status="open" assignee="operator">
    <text>Should `mejepa-corpus generate` support resume-from-partial after Ctrl-C
    interruption, or remain wipe-and-restart?</text>
    <context>2,400 entries × ~30s each = ~20 hours. A mid-run failure today wastes the
    completed work. v1 commits to wipe-and-restart for simplicity (per
    memory/feedback_no_backwards_compat.md "no backwards-compat shims unless operator
    explicitly requests"); resume support could be added behind a flag.</context>
    <resolution></resolution>
  </question>

  <question id="Q-CORPUS-02" status="open" assignee="operator">
    <text>What is the language priority order for shipping the remaining 10
    operators?</text>
    <context>Doc 10 Section C lists Python, Rust, C, C++, Java, C#, JS, TS, Ruby, Go, PHP
    as the 11-language matrix. Python is shipped. The 10 deferred languages need an
    explicit priority order so SPEC-CORPUS-LANG-N specs can be authored in
    sequence.</context>
    <resolution></resolution>
  </question>

  <question id="Q-CORPUS-03" status="open" assignee="operator">
    <text>For `WrongFile` mutations, should the alternate file be auto-selected from
    the same repo (heuristic: same file extension + similar size + similar AST node
    count), or always operator-supplied via `--alternate-wrong-file-path`?</text>
    <context>Today's API takes alternate_source explicitly. Auto-selection would
    increase corpus diversity but adds a heuristic that could overfit Phase 3
    training to one syntactic shape (constitution Generic-Only rule). Until
    explicitly approved, we keep the explicit-supply path.</context>
    <resolution></resolution>
  </question>

  <question id="Q-CORPUS-04" status="open" assignee="operator">
    <text>Should the verifier sample fraction (REQ-CORPUS-15) default to 5% or
    adjust based on corpus_version (e.g., 100% on first verification of a new
    corpus)?</text>
    <context>5% is fast (~120 entries × 30s ≈ 1 hour). 100% is a 20-hour rerun. v1
    defaults to 5% with `--sample-fraction 1.0` as the override path.</context>
    <resolution></resolution>
  </question>

</open_questions>

<glossary>

  <term name="ME-JEPA">Joint Embedding Predictive Architecture for Code — the central
  predictive model. See doc 09 §1, doc 10, doc 11.</term>

  <term name="OracleVerdict">The canonical verdict struct consumed by EOracleInstrument:
  {per_test, exception, resolved, runtime_seconds}. See
  context-graph-mejepa-instruments.</term>

  <term name="FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL">SWE-bench's
  per-test transition buckets. F2P measures regression coverage; P2P measures
  no-regression; F2F + P2F measure expected-still-failing tests. See
  swebench/harness/grading.py.</term>

  <term name="resolved">SWE-bench's binary verdict: true iff F2P.success covers the
  entire F2P set AND P2P.success covers the entire P2P set.</term>

  <term name="corpus_sha">SHA-256 over the sorted concatenation of all (task_id,
  category, mutated_source) tuples in a corpus. Provenance fingerprint for
  reproducibility.</term>

  <term name="run_id">A Docker-name-safe identifier (regex
  `^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`) that namespaces the harness run's artifact tree
  under `<run_root>/logs/run_evaluation/<run_id>/`.</term>

  <term name="InstanceAtomic / RepoAtomic">Two split modes. InstanceAtomic assigns each
  task independently; RepoAtomic assigns each repo as a unit. InstanceAtomic
  achieves clean [240, 30, 30] on Lite; RepoAtomic is structurally bounded to
  approximately [195, 77, 28].</term>

  <term name="FSV (Forensic Source Verification)">Constitutional discipline
  (`<fsv_protocol>`) requiring every shipped primitive to ship a verification
  binary that writes evidence to `/tmp/contextgraph-<crate>-fsv/` and re-reads +
  asserts before declaring success.</term>

  <term name="Mutation operator">A function that takes a canonical fix's source and
  returns a mutated version whose oracle outcome is structurally predictable per
  category. 8 operators exist per doc 09 §3.1.</term>

  <term name="lexical mask">A per-language predicate identifying byte ranges that are
  NOT mutation sites: comments, ordinary strings, triple-quoted docstrings,
  prefixed strings, f-string literal text. F-string replacement fields ARE
  exposed as code (Python treats them as expressions).</term>

  <term name="bin-pack">The greedy assignment used in `split_corpus`: pick the bucket
  with the lowest `current_count / target_count` ratio for each unit (LPT
  pre-sorted by descending size).</term>

</glossary>

</functional_spec>
```

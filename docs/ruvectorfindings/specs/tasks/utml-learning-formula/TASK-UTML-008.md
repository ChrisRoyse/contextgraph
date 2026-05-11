# TASK-UTML-008 — EmbedderCoherence (cross-embedder Jaccard NN-set agreement) + pairwise-MI audit job

## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT — PARTIAL (compute primitive shipped; matrix-emitting JOB pending)

> **Codebase reality table:** see `_index.md` in this directory.

**Status: PARTIAL.** The COMPUTE primitive `pairwise_mi_audit(probabilities: &[Vec<f32>]) -> Result<f32, UtmlError>` SHIPPED at `crates/context-graph-mejepa-train/src/learning_signal.rs:275` — returns a SCALAR for ONE pair of distributions.

The MATRIX-emitting periodic AUDIT JOB that walks every instrument pair and persists `/tmp/contextgraph-mejepa-train-fsv/utml-pairwise-mi-matrix.csv` does NOT exist. This is the remaining work.

**Decided scope for the remaining work:**
1. New file `crates/context-graph-mejepa-train/src/aux/pairwise_mi_audit.rs` (alongside the existing `aux/` modules).
2. `pub struct PairwiseMiAuditor { period_steps: u64, output_dir: PathBuf }`.
3. `pub fn run_audit(&self, instrument_outputs: &BTreeMap<InstrumentSlot, Vec<Vec<f32>>>, step: u64) -> Result<PathBuf, UtmlError>`:
   - For each `(slot_a, slot_b)` pair where `slot_a < slot_b` (no self-pairs, no duplicate orderings): call `pairwise_mi_audit(probabilities_a, probabilities_b)?`.
   - Emit `utml-pairwise-mi-matrix-{step:08}.csv` with header `slot_a,slot_b,mi_nats,sample_count` and one row per pair.
   - File mode `0o600` per SEC-MEJEPA-07.
   - Return the file path.
4. `pub struct EmbedderCoherence { matrix: BTreeMap<(InstrumentSlot, InstrumentSlot), f32>, computed_at_step: u64 }` — load from CSV for downstream consumers (Phase 8 TASK-EVAL-011 reads this for Q5 open research question).
5. Schedule: invoked by `Trainer` every `period_steps` (default 1000); skipped during warmup.
6. NEVER `unwrap_or` defaults on MI computation — NaN/Inf surfaces structured errors.

**Cross-phase dep:** Phase 8 TASK-EVAL-011 reads the matrix CSV for the Q5 open-research-question entry.

**FSV for this task:** integration test (a) constructs synthetic instrument outputs with KNOWN pairwise MI (independent: ~0.0 nats; identical: max nats) → asserts CSV matrix matches expected within ε; (b) reads back the CSV via separate file open → asserts byte-equal serialization; (c) verifies file mode `0o600`. Source of Truth: separate file read + ground-truth MI cross-check.

---

```xml
<task_spec id="TASK-UTML-008" version="1.0">

<metadata>
  <title>compute_embedder_coherence (Jaccard agreement of NN sets across E1-E14) + pairwise_mi_audit (writes pairwise_mi_matrix.csv) + bootstrap_neutral fallback for &lt; 1000 corpus entries</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-UTML-03">embedder_coherence term in ΔK</item>
    <item ref="AC-UTML-03-03">Jaccard NN-set agreement across 14 content embedders</item>
    <item ref="AC-UTML-05-03">predictor_redundancy &gt; 0.5 → trigger pairwise-MI audit</item>
    <item ref="EC-UTML-11">First 1,000 corpus entries: bootstrap neutral 0.5</item>
    <item ref="component:EmbedderCoherence">crates/context-graph-mejepa-train/src/learning_signal/embedder_coherence.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-007</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 created</estimated_files>
</metadata>

<context>
embedder_coherence (doc 12 §5.1) is one of the four ΔK sub-components. Across 14 frozen content embedders (E1-E14), each panel has a top-K nearest-neighbor set under each embedder's metric. High mean pairwise Jaccard agreement = panel is recognized in all embedder feature spaces (low OOD risk). For the first 1,000 corpus entries the NN sets are not statistically meaningful, so we return a neutral 0.5 (NEVER 1.0 — would over-credit ΔK). Pairwise MI audit is the periodic job that detects panel-slot redundancy &gt; 0.5 and triggers VICReg coefficient bumps in Phase 5.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-03; AC-UTML-03-03; EC-UTML-11; AC-UTML-05-03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component EmbedderCoherence)</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §5.1 (cross-embedder Jaccard)</file>
  <file purpose="instrument_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel struct)</file>
  <file purpose="prior_tasks">TASK-UTML-001..007</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-007 complete (sampling_weight + L_step shipped)</check>
  <check>Phase 1 instruments shipped (Panel + InstrumentSlot)</check>
  <check>21-embedder ensemble loadable (Phase 1c partial — 14 content embedders)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/embedder_coherence.rs` with:
      - `pub trait ContentEmbedder { fn nn_indices(&amp;self, panel: &amp;Panel, k: usize) -&gt; Result&lt;Vec&lt;usize&gt;, UtmlError&gt;; }`
      - `pub fn compute_embedder_coherence(panel: &amp;Panel, content_embedders: &amp;[Box&lt;dyn ContentEmbedder&gt;; 14], corpus_size: usize, bootstrap_threshold: usize, k_nn: usize) -&gt; Result&lt;(f32, &amp;'static str), UtmlError&gt;`
      - jaccard helper
    - Create `src/learning_signal/pairwise_mi.rs` with:
      - `pub struct PairwiseMiSummary { pub mean_normalized_mi: f32, pub max_normalized_mi: f32, pub matrix_csv_path: PathBuf }`
      - `pub fn pairwise_mi_audit(panels: &amp;[Panel], output_dir: &amp;Path) -&gt; Result&lt;PairwiseMiSummary, UtmlError&gt;` writing `pairwise_mi_matrix.csv` (15×15 matrix per InstrumentSlot)
      - `pub fn predictor_redundancy_from_summary(summary: &amp;PairwiseMiSummary) -&gt; f32` (returns mean_normalized_mi clamped to [0,1])
    - Bootstrap fallback: if `corpus_size &lt; bootstrap_threshold` (default 1,000) → return `(0.5, "bootstrap_neutral_n_lt_1000")` immediately
    - Default k_nn = 50; configurable
    - Compute 91 pairwise Jaccards (14 choose 2 = 91); coherence = mean
    - Pairwise MI: use a deterministic histogram-based estimator over slot activations (1D MI between slot pairs); store full 15×15 symmetric matrix in CSV with InstrumentSlot::slug() row/column labels
    - CSV output mode 0o600 per SEC-MEJEPA-07
    - Unit tests with stub embedders: full agreement → 1.0; full disagreement (disjoint NN sets) → 0.0; bootstrap → 0.5; MI matrix size 15×15; symmetric matrix invariant
  </in_scope>
  <out_of_scope>
    - Real E1-E14 forward passes (Phase 1c neural backends)
    - VICReg coefficient bump intervention (Phase 5)
    - Constellation Gτ rate (Phase 4b)
    - Cert chain emission (TASK-TRAIN-009)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/embedder_coherence.rs">
use context_graph_mejepa_instruments::Panel;
use crate::error::UtmlError;

pub const DEFAULT_BOOTSTRAP_THRESHOLD: usize = 1000;
pub const DEFAULT_K_NN: usize = 50;
pub const NUM_CONTENT_EMBEDDERS: usize = 14;

pub trait ContentEmbedder: Send + Sync {
    fn nn_indices(&amp;self, panel: &amp;Panel, k: usize) -&gt; Result&lt;Vec&lt;usize&gt;, UtmlError&gt;;
    fn slug(&amp;self) -&gt; &amp;'static str; // "e1".."e14"
}

pub fn compute_embedder_coherence(
    panel: &amp;Panel,
    content_embedders: &amp;[Box&lt;dyn ContentEmbedder&gt;], // length must equal NUM_CONTENT_EMBEDDERS
    corpus_size: usize,
    bootstrap_threshold: usize,
    k_nn: usize,
) -&gt; Result&lt;(f32, &amp;'static str), UtmlError&gt;;

fn jaccard(a: &amp;[usize], b: &amp;[usize]) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi.rs">
use std::path::{Path, PathBuf};
use crate::error::UtmlError;
use context_graph_mejepa_instruments::Panel;

#[derive(Debug, Clone)]
pub struct PairwiseMiSummary {
    pub mean_normalized_mi: f32,
    pub max_normalized_mi: f32,
    pub matrix_csv_path: PathBuf,
}

pub fn pairwise_mi_audit(
    panels: &amp;[Panel],
    output_dir: &amp;Path,
) -&gt; Result&lt;PairwiseMiSummary, UtmlError&gt;;

pub fn predictor_redundancy_from_summary(summary: &amp;PairwiseMiSummary) -&gt; f32;
    </signature>
  </signatures>
  <constraints>
    - 14 content embedders: validate slice length; UtmlFactorOutOfRange on mismatch
    - corpus_size, k_nn validated &gt; 0
    - Jaccard = |A ∩ B| / |A ∪ B|; both empty → 0.0
    - Pairwise MI estimator: deterministic histogram bins (e.g., 32 bins per slot); normalized by min(H(X), H(Y))
    - CSV format: header row "slot,e_ast,e_cfg,...,scalars\n" + 15 data rows
    - File mode 0o600 on the CSV
    - Files &lt; 400 lines each
  </constraints>
  <verification>
    - cargo test shows ≥ 8 tests pass: bootstrap fallback, full agreement, full disagreement, jaccard symmetry, MI matrix shape 15×15, MI symmetric, CSV file mode 0o600
  </verification>
</definition_of_done>

<pseudo_code>
fn compute_embedder_coherence(panel, embs, corpus, threshold, k) -&gt; Result&lt;(f32, &amp;'static str), UtmlError&gt; {
    if corpus &lt; threshold { return Ok((0.5, "bootstrap_neutral_n_lt_1000")); }
    if embs.len() != NUM_CONTENT_EMBEDDERS { return Err(UtmlFactorOutOfRange); }
    let mut nn = Vec::with_capacity(NUM_CONTENT_EMBEDDERS);
    for e in embs { nn.push(e.nn_indices(panel, k)?); }
    let mut sum = 0.0; let mut pairs = 0;
    for i in 0..NUM_CONTENT_EMBEDDERS {
        for j in (i+1)..NUM_CONTENT_EMBEDDERS {
            sum += jaccard(&amp;nn[i], &amp;nn[j]); pairs += 1;
        }
    }
    Ok(((sum / pairs as f32).clamp(0.0, 1.0), "computed"))
}

fn pairwise_mi_audit(panels, out_dir) -&gt; Result&lt;PairwiseMiSummary, UtmlError&gt; {
    let slots = InstrumentSlot::all(); // 15 slots
    let n = slots.len();
    let mut mat = vec![vec![0f32; n]; n];
    for i in 0..n { for j in i..n {
        let mi = histogram_mi(panels, slots[i], slots[j], 32)?;
        mat[i][j] = mi; mat[j][i] = mi;
    }}
    write_csv_with_mode_600(out_dir.join("pairwise_mi_matrix.csv"), &amp;slots, &amp;mat)?;
    let mean = mat.iter().flatten().sum::&lt;f32&gt;() / (n*n) as f32;
    let max = mat.iter().flatten().copied().fold(0.0, f32::max);
    Ok(PairwiseMiSummary { mean_normalized_mi: mean.clamp(0.0,1.0), max_normalized_mi: max.clamp(0.0,1.0), matrix_csv_path: ... })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/embedder_coherence.rs">compute_embedder_coherence + ContentEmbedder trait + jaccard + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi.rs">pairwise_mi_audit + PairwiseMiSummary + CSV writer + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">re-export new modules</file>
</files_to_modify>

<validation_criteria>
  <criterion>Bootstrap fallback returns (0.5, "bootstrap_neutral_n_lt_1000") for corpus_size &lt; 1000</criterion>
  <criterion>14-embedder constraint validated</criterion>
  <criterion>Full-agreement NN sets → coherence 1.0; disjoint → 0.0</criterion>
  <criterion>pairwise_mi_matrix.csv is 15×15, symmetric, mode 0o600</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib embedder_coherence</command>
  <command>cargo test -p context-graph-mejepa-train --lib pairwise_mi</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

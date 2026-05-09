# TASK-CORPUS-022 — CorpusProvenance Computer + put_provenance Wiring

```xml
<task_spec id="TASK-CORPUS-022" version="1.0">
<metadata>
  <title>build_corpus_provenance + Provenance Verifier (re-derives corpus_sha)</title>
  <status>pending</status>
  <implementation_state as_of="2026-05-09">
    Not yet shipped. Depends on TASK-CORPUS-007 (CorpusProvenance type) and TASK-CORPUS-008 (`CF_MEJEPA_CORPUS_PROVENANCE` 3rd CF) — both also pending. `bin/mejepa-corpus.rs::generate` does compute per-entry SHAs (`patch_sha256`, `oracle_verdict_sha256` in `CorpusIndex::entries`) and writes them to `index.json`, so the per-row content addresses already exist; the missing piece is the corpus-wide provenance record (Merkle-style `corpus_sha` over the per-row SHAs + operator + run_id + generator_version + dataset/split + ISO-8601 timestamp), the verifier that re-derives `corpus_sha` from the CFs, and the `OracleStore::put_provenance`/`get_provenance` accessors.
  </implementation_state>
  <layer>logic</layer>
  <sequence>22</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-12</requirement_ref>
    <requirement_ref>REQ-CORPUS-16</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-007</task_ref>
    <task_ref>TASK-CORPUS-021</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Logic task. The function that computes a CorpusProvenance struct
from the persisted corpus AND wires it into OracleStore via
put_provenance. Per BR-CORPUS-06: provenance is written EXACTLY
ONCE per generate run AND only when complete: true. This task
also defines a re-derivation verifier (re-reads CFs, recomputes
corpus_sha, asserts equal — used by `verify` CLI in TASK-CORPUS-027).
</context>

<input_context_files>
  <file purpose="data_model">../../technical/TECH-CORPUS.md#data_models (CorpusProvenance)</file>
  <file purpose="business_rule">../../functional/SPEC-CORPUS.md (BR-CORPUS-06)</file>
  <file purpose="constitution_provenance">../../constitution.md#coding_standards (provenance)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-007 complete (CorpusProvenance + compute_corpus_sha exist)</check>
  <check>TASK-CORPUS-021 complete (OracleStore + put_provenance method exist)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn build_corpus_provenance(store:
      &amp;OracleStore, embedder_version: u32, code_version:
      &amp;str, language_set: &amp;[Language]) -&gt;
      MutationResult&lt;CorpusProvenance&gt;`:
      1. Iterate `store.iter_corpus_rows()`, collecting
         `Vec&lt;(task_id, MutationCategory, mutated_source)&gt;`.
      2. corpus_sha = `CorpusProvenance::compute_corpus_sha(&amp;rows)`.
      3. frozen_at = current UTC ISO 8601 (`OffsetDateTime::now_utc().format(...)`).
      4. entry_count = rows.len() as u32.
      5. Construct CorpusProvenance and call `prov.validate()`.
    - Implement `pub fn finalize_provenance(store:
      &amp;OracleStore, embedder_version: u32, code_version:
      &amp;str, language_set: &amp;[Language]) -&gt;
      MutationResult&lt;CorpusProvenance&gt;`:
      builds + writes via `store.put_provenance(&amp;prov)`.
    - Implement `pub fn verify_provenance_consistent(store:
      &amp;OracleStore, expected: &amp;CorpusProvenance) -&gt;
      MutationResult&lt;()&gt;`:
      1. Re-derives corpus_sha from current CF rows.
      2. Asserts `derived == expected.corpus_sha`.
      3. Asserts entry_count matches the CF row count.
      4. Returns `MEJEPA_CORPUS_INVALID_INPUT` with field
         `corpus_sha` or `entry_count` on mismatch.
    - Add tests:
      - build → put → reopen → get_provenance returns equal struct.
      - corpus_sha is reproducible across two `build_corpus_provenance`
        calls on the same store (TC-CORPUS-16).
      - verify_provenance_consistent passes on a fresh corpus.
      - verify_provenance_consistent fails after a manual row
        edit (corpus_sha drift detection).
  </in_scope>
  <out_of_scope>
    - The CLI generate path that calls finalize_provenance only on
      complete: true (TASK-CORPUS-026).
    - The verify CLI re-runner (TASK-CORPUS-027).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    pub fn build_corpus_provenance(
        store: &amp;crate::oracle::OracleStore,
        embedder_version: u32,
        code_version: &amp;str,
        language_set: &amp;[Language],
    ) -&gt; MutationResult&lt;CorpusProvenance&gt;;

    pub fn finalize_provenance(
        store: &amp;crate::oracle::OracleStore,
        embedder_version: u32,
        code_version: &amp;str,
        language_set: &amp;[Language],
    ) -&gt; MutationResult&lt;CorpusProvenance&gt;;

    pub fn verify_provenance_consistent(
        store: &amp;crate::oracle::OracleStore,
        expected: &amp;CorpusProvenance,
    ) -&gt; MutationResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - frozen_at MUST match regex `\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z`.
      No millisecond precision — collisions are fine; we want
      reproducibility on the second granularity.
    - code_version is a 40-char lowercase git SHA passed by the
      caller (CLI reads it from `git rev-parse HEAD`).
    - finalize_provenance MUST NOT be called on incomplete
      corpora — the CLI is the gate; this function trusts the
      caller, but documents the precondition.
    - Use `time` crate (workspace dep) for ISO 8601 — do NOT
      add `chrono` (constitution `<required_libraries>`).
    - All errors carry the field name and the offending value.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- corpus_provenance_compute` passes.
    - `cargo test -p context-graph-mejepa-corpus -- corpus_sha_reproducible` passes (TC-CORPUS-16).
    - `cargo test -p context-graph-mejepa-corpus -- verify_provenance_drift_detected` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn build_corpus_provenance(store: &amp;OracleStore, ev: u32, code_version: &amp;str, lang: &amp;[Language]) -&gt; MutationResult&lt;CorpusProvenance&gt; {
    let mut rows = vec![];
    for r in store.iter_corpus_rows() {
        let (tid, cat, outcome, _verdict) = r.map_err(|e| ...)?;
        rows.push((tid, cat, outcome.mutated_source));
    }
    let corpus_sha = CorpusProvenance::compute_corpus_sha(&amp;rows);
    let frozen_at = current_iso8601_utc(); // 2026-05-09T01:23:45Z
    let prov = CorpusProvenance {
        corpus_sha, embedder_version: ev,
        frozen_at, entry_count: rows.len() as u32,
        code_version: code_version.to_string(),
        language_set: lang.to_vec(),
    };
    prov.validate()?;
    Ok(prov)
}

#[test]
fn corpus_sha_reproducible_two_runs() {
    let store = build_temp_store_with_3_rows();
    let p1 = build_corpus_provenance(&amp;store, 1, &amp;"a".repeat(40), &amp;[Language::Python]).unwrap();
    let p2 = build_corpus_provenance(&amp;store, 1, &amp;"a".repeat(40), &amp;[Language::Python]).unwrap();
    assert_eq!(p1.corpus_sha, p2.corpus_sha);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/Cargo.toml">Add `time` crate dep if missing.</file>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add 3 functions.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 4 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>corpus_sha is reproducible across two build calls on same store (TC-CORPUS-16)</criterion>
  <criterion>finalize_provenance writes one row in CF_MEJEPA_CORPUS_PROVENANCE</criterion>
  <criterion>verify_provenance_consistent detects drift after manual row edit</criterion>
  <criterion>frozen_at matches ISO 8601 UTC pattern</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- corpus_provenance_compute</command>
  <command>cargo test -p context-graph-mejepa-corpus -- corpus_sha_reproducible</command>
  <command>cargo test -p context-graph-mejepa-corpus -- verify_provenance</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

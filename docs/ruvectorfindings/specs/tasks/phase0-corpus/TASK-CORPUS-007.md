# TASK-CORPUS-007 — CorpusProvenance + GeneratedEntry + CorpusIndex Types

```xml
<task_spec id="TASK-CORPUS-007" version="1.0">
<metadata>
  <title>CorpusProvenance + GeneratedEntry + CorpusIndex Types</title>
  <status>pending</status>
  <implementation_state as_of="2026-05-09">
    Partial scaffolding: `CorpusEntry { task_id, repo }` exists in `split.rs` (used by the splitter as a task→bucket mapping input). `CorpusIndex` + `GeneratedEntry` records are defined privately in `bin/mejepa-corpus.rs:96-123` covering schema_version / complete / generated_at_unix_ms / tasks_dir / store_path / entries / split_assignments + per-entry SHAs. The full `CorpusProvenance` record (corpus_sha, operator_id, run_id, generator_version, dataset_name + split, mutation_seeds, source repo commit, language, ISO-8601 timestamp) is NOT yet defined as a public type in `lib.rs`, and is NOT persisted into a third RocksDB column family — both pending.
  </implementation_state>
  <layer>foundation</layer>
  <sequence>7</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-12</requirement_ref>
    <requirement_ref>REQ-CORPUS-16</requirement_ref>
    <requirement_ref>REQ-CORPUS-08</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
    <task_ref>TASK-CORPUS-002</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Provenance metadata + on-disk corpus-index.json
schema. The provenance row binds (corpus_sha, embedder_version,
frozen_at, code_version, language_set) per constitution
`<provenance>` rule. The corpus-index.json includes the
`complete` flag (REQ-CORPUS-16) and per-entry summaries.
</context>

<input_context_files>
  <file purpose="data_models">../../technical/TECH-CORPUS.md#data_models (CorpusProvenance, CorpusIndex, GeneratedEntry)</file>
  <file purpose="provenance_rule">../../constitution.md#coding_standards (provenance)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-01, US-CORPUS-09)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-001 complete (MutationCategory exists)</check>
  <check>TASK-CORPUS-002 complete (Language exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Define `CorpusProvenance { corpus_sha: String,
      embedder_version: u32, frozen_at: String, entry_count: u32,
      code_version: String, language_set: Vec&lt;Language&gt; }`.
    - Define `GeneratedEntry { task_id, repo, category: MutationCategory,
      bucket: String, patch_path: String, patch_sha256: String,
      oracle_verdict_sha256: String, oracle_all_passed: bool,
      oracle_exception: Option&lt;String&gt;,
      oracle_per_test_count: usize }`.
    - Define `CorpusIndex { schema_version: u32, complete: bool,
      generated_at_unix_ms: u128, tasks_dir: String,
      store_path: String, entries: Vec&lt;GeneratedEntry&gt;,
      split_assignments: Vec&lt;TaskAssignment&gt; }`.
    - Add validation helpers:
      `CorpusProvenance::validate(&amp;self) -&gt; MutationResult&lt;()&gt;`
      enforcing 64-char lowercase hex on `corpus_sha`, 40-char
      lowercase hex on `code_version`, ISO 8601 UTC pattern on
      `frozen_at`, non-empty `language_set`.
    - Unit tests: validate happy path, validate rejects 63-char sha,
      validate rejects empty language_set, JSON round-trip.
    - Define a `CorpusProvenance::compute_corpus_sha(rows:
      &amp;[(String, MutationCategory, String)]) -&gt; String`
      helper that produces the SHA-256 hex per spec rule (sorted
      concat of (task_id, category.slug(), mutated_source)).
  </in_scope>
  <out_of_scope>
    - put_provenance write path (TASK-CORPUS-022).
    - corpus-index.json write path (TASK-CORPUS-023, 026).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct CorpusProvenance {
        pub corpus_sha: String,
        pub embedder_version: u32,
        pub frozen_at: String,
        pub entry_count: u32,
        pub code_version: String,
        pub language_set: Vec&lt;Language&gt;,
    }

    impl CorpusProvenance {
        pub fn validate(&amp;self) -&gt; MutationResult&lt;()&gt;;
        pub fn compute_corpus_sha(rows: &amp;[(String, MutationCategory, String)]) -&gt; String;
    }

    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct GeneratedEntry {
        pub task_id: String,
        pub repo: String,
        pub category: MutationCategory,
        pub bucket: String,
        pub patch_path: String,
        pub patch_sha256: String,
        pub oracle_verdict_sha256: String,
        pub oracle_all_passed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")] pub oracle_exception: Option&lt;String&gt;,
        pub oracle_per_test_count: usize,
    }

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct CorpusIndex {
        pub schema_version: u32,
        pub complete: bool,
        pub generated_at_unix_ms: u128,
        pub tasks_dir: String,
        pub store_path: String,
        pub entries: Vec&lt;GeneratedEntry&gt;,
        pub split_assignments: Vec&lt;crate::split::TaskAssignment&gt;,
    }
    </signature>
  </signatures>

  <constraints>
    - `compute_corpus_sha` MUST sort tuples deterministically by
      (task_id, category.slug()) BEFORE concatenation; otherwise
      same-corpus runs produce different hashes.
    - `validate()` returns `MEJEPA_CORPUS_INVALID_INPUT` with
      precise field name on every regression.
    - `schema_version: u32 = 1` is the v1 constant; bumped on
      schema change (TECH-CORPUS data model).
    - No `unwrap()` in production code paths.
    - `oracle_exception` is the `ExceptionClass` slug as a string
      (not the enum directly) to keep the corpus-index resilient
      to upstream variant renames.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- corpus_provenance` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl CorpusProvenance {
    pub fn validate(&amp;self) -&gt; MutationResult&lt;()&gt; {
        if self.corpus_sha.len() != 64 || !self.corpus_sha.chars().all(|c| c.is_ascii_hexdigit() &amp;&amp; !c.is_ascii_uppercase()) {
            return Err(MutationError::invalid("corpus_sha", ...));
        }
        if self.code_version.len() != 40 || !self.code_version.chars().all(|c| c.is_ascii_hexdigit() &amp;&amp; !c.is_ascii_uppercase()) {
            return Err(MutationError::invalid("code_version", ...));
        }
        // ISO 8601 UTC: \d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z
        if !is_iso8601_utc(&amp;self.frozen_at) { return Err(...); }
        if self.language_set.is_empty() { return Err(...); }
        Ok(())
    }
    pub fn compute_corpus_sha(rows: &amp;[(String, MutationCategory, String)]) -&gt; String {
        let mut sorted: Vec&lt;_&gt; = rows.iter().collect();
        sorted.sort_by(|a, b| (&amp;a.0, a.1.slug()).cmp(&amp;(&amp;b.0, b.1.slug())));
        let mut hasher = sha2::Sha256::new();
        for (tid, cat, src) in sorted {
            hasher.update(tid.as_bytes());
            hasher.update(b"\0"); hasher.update(cat.slug().as_bytes());
            hasher.update(b"\0"); hasher.update(src.as_bytes());
            hasher.update(b"\n");
        }
        hex_lowercase(hasher.finalize())
    }
}

#[test]
fn compute_corpus_sha_sort_invariant() {
    let r1 = vec![("a".into(), MutationCategory::KnownGood, "x".into()),
                  ("b".into(), MutationCategory::SubtleFlip, "y".into())];
    let r2 = vec![("b".into(), MutationCategory::SubtleFlip, "y".into()),
                  ("a".into(), MutationCategory::KnownGood, "x".into())];
    assert_eq!(CorpusProvenance::compute_corpus_sha(&amp;r1),
               CorpusProvenance::compute_corpus_sha(&amp;r2));
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add 3 types + validate + compute_corpus_sha helpers.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add corpus_provenance tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>CorpusProvenance.validate() catches all 4 documented regression cases</criterion>
  <criterion>compute_corpus_sha is sort-invariant</criterion>
  <criterion>JSON round-trip preserves all fields</criterion>
  <criterion>schema_version constant = 1</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- corpus_provenance</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

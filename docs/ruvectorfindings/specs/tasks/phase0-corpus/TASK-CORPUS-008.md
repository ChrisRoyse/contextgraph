# TASK-CORPUS-008 — RocksDB CF Schemas (3 CF Descriptors)

```xml
<task_spec id="TASK-CORPUS-008" version="1.0">
<metadata>
  <title>RocksDB CF Descriptors: CF_MEJEPA_MUTATION_CORPUS / CF_MEJEPA_ORACLE_VERDICTS / CF_MEJEPA_CORPUS_PROVENANCE</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>8</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-08</requirement_ref>
    <requirement_ref>REQ-CORPUS-12</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Foundation task. Defines the three RocksDB column-family names
and their key/value schemas exactly per TECH-CORPUS
`<rocksdb_column_families>`. The OracleStore (TASK-CORPUS-021)
opens the DB with these descriptors; verify CLI (TASK-CORPUS-027)
reads them back. Names follow constitution naming rule
`CF_MEJEPA_*` (SCREAMING_SNAKE).
</context>

<input_context_files>
  <file purpose="cf_schemas">../../technical/TECH-CORPUS.md#rocksdb_column_families</file>
  <file purpose="naming_rule">../../constitution.md#coding_standards (rocksdb_cfs)</file>
  <file purpose="storage_alignment">crates/context-graph-storage/src/lib.rs (read-only — for cache/compression defaults)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/oracle.rs</file>
</input_context_files>

<prerequisites>
  <check>rocksdb 0.21.x in workspace dependencies</check>
  <check>TASK-CORPUS-001 complete (OracleError exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Add a `cf_descriptors()` constructor in `oracle.rs` that
      returns `Vec&lt;rocksdb::ColumnFamilyDescriptor&gt;` for the
      three CFs: `CF_MEJEPA_MUTATION_CORPUS`,
      `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_CORPUS_PROVENANCE`.
    - Define string constants for each CF name.
    - Each descriptor's `Options` sets:
      - `set_compression_type(rocksdb::DBCompressionType::Lz4)`
      - `set_write_buffer_size(64 MiB)`
      - `set_max_write_buffer_number(3)`
      - `set_min_write_buffer_number_to_merge(1)`
    - Add a `db_options()` helper returning the top-level
      `rocksdb::Options` with `create_if_missing(true)` and
      `create_missing_column_families(true)`.
    - Define `KEY_CORPUS / KEY_VERDICTS` builder helpers:
      `pub(crate) fn corpus_key(task_id: &amp;str, category:
      MutationCategory) -&gt; OracleResult&lt;Vec&lt;u8&gt;&gt;`
      using `bincode::serialize`.
    - Define provenance key builder:
      `pub(crate) fn provenance_key(corpus_version: u32,
      embedder_version: u32) -&gt; OracleResult&lt;Vec&lt;u8&gt;&gt;`.
    - Unit tests: cf_descriptors returns 3 entries with the
      expected names; corpus_key is deterministic; provenance_key
      is deterministic.
  </in_scope>
  <out_of_scope>
    - OracleStore::open / put / get methods (TASK-CORPUS-021).
    - Atomic WriteBatch (TASK-CORPUS-021).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/oracle.rs">
    pub const CF_MEJEPA_MUTATION_CORPUS: &amp;str = "mejepa_mutation_corpus";
    pub const CF_MEJEPA_ORACLE_VERDICTS: &amp;str = "mejepa_oracle_verdicts";
    pub const CF_MEJEPA_CORPUS_PROVENANCE: &amp;str = "mejepa_corpus_provenance";

    pub(crate) fn cf_descriptors() -&gt; Vec&lt;rocksdb::ColumnFamilyDescriptor&gt;;
    pub(crate) fn db_options() -&gt; rocksdb::Options;
    pub(crate) fn corpus_key(task_id: &amp;str, category: crate::MutationCategory) -&gt; OracleResult&lt;Vec&lt;u8&gt;&gt;;
    pub(crate) fn provenance_key(corpus_version: u32, embedder_version: u32) -&gt; OracleResult&lt;Vec&lt;u8&gt;&gt;;
    </signature>
  </signatures>

  <constraints>
    - CF name strings use lowercase form (RocksDB convention) but
      Rust constants spell the SCREAMING_SNAKE constant name. The
      constants holding the strings ARE the canonical CF names
      consumed elsewhere.
    - Compression: Lz4 (per `context-graph-storage` alignment).
      NOT zstd — keep consistent with the rest of the workspace.
    - bincode 1.3.x or whichever is in the workspace lockfile;
      do not introduce a new bincode major.
    - corpus_key serializes `(String, MutationCategory)` tuple;
      provenance_key serializes `(u32, u32)` tuple.
    - The bincode crate dep MUST be added to Cargo.toml if not
      already present (workspace dep).
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- cf_descriptors` passes.
    - `cargo test -p context-graph-mejepa-corpus -- corpus_key_deterministic` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub(crate) fn cf_descriptors() -&gt; Vec&lt;rocksdb::ColumnFamilyDescriptor&gt; {
    let mk_opts = || {
        let mut o = rocksdb::Options::default();
        o.set_compression_type(rocksdb::DBCompressionType::Lz4);
        o.set_write_buffer_size(64 * 1024 * 1024);
        o.set_max_write_buffer_number(3);
        o.set_min_write_buffer_number_to_merge(1);
        o
    };
    vec![
        rocksdb::ColumnFamilyDescriptor::new(CF_MEJEPA_MUTATION_CORPUS, mk_opts()),
        rocksdb::ColumnFamilyDescriptor::new(CF_MEJEPA_ORACLE_VERDICTS, mk_opts()),
        rocksdb::ColumnFamilyDescriptor::new(CF_MEJEPA_CORPUS_PROVENANCE, mk_opts()),
    ]
}

pub(crate) fn corpus_key(task_id: &amp;str, category: MutationCategory) -&gt; OracleResult&lt;Vec&lt;u8&gt;&gt; {
    bincode::serialize(&amp;(task_id.to_string(), category)).map_err(|e| OracleError::Store {
        operation: "serialize", cf: "key", message: e.to_string()
    })
}

#[test]
fn cf_descriptors_returns_three_canonical_cfs() {
    let d = cf_descriptors();
    assert_eq!(d.len(), 3);
    let names: Vec&lt;_&gt; = d.iter().map(|c| c.name().to_string()).collect();
    assert!(names.contains(&amp;CF_MEJEPA_MUTATION_CORPUS.to_string()));
    assert!(names.contains(&amp;CF_MEJEPA_ORACLE_VERDICTS.to_string()));
    assert!(names.contains(&amp;CF_MEJEPA_CORPUS_PROVENANCE.to_string()));
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/Cargo.toml">Add bincode dep if missing (workspace).</file>
  <file path="crates/context-graph-mejepa-corpus/src/oracle.rs">Add 3 CF constants + cf_descriptors + db_options + key builders.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 3 unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>3 CF constants defined with canonical lowercase names</criterion>
  <criterion>cf_descriptors() returns 3 entries with Lz4 compression</criterion>
  <criterion>corpus_key + provenance_key are deterministic across calls</criterion>
  <criterion>db_options sets create_if_missing + create_missing_column_families</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- cf_descriptors</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

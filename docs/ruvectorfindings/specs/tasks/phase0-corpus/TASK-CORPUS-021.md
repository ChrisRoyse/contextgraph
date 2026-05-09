# TASK-CORPUS-021 — OracleStore Atomic WriteBatch

```xml
<task_spec id="TASK-CORPUS-021" version="1.0">
<metadata>
  <title>OracleStore: open / put_corpus_row (atomic WriteBatch) / get_corpus_row / iter_corpus_rows</title>
  <status>partial</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `OracleStore::open(path)`, `put_mutation`, `get_mutation`, `put_verdict`, `get_verdict`, `count_cf`, `scan_cf_json`, `flush` (oracle.rs:318-525) — all keyed `<task_id>|<category_slug>` over the two named CFs.
    Pending: a single atomic `put_corpus_row` (or `put_mutation_and_verdict_and_provenance`) that uses `rocksdb::WriteBatch` to write all three CFs in one transaction so a crash mid-task can never leave a mutation with no verdict (or vice versa). Current CLI calls `put_mutation` + `flush` + `put_verdict` + `flush` separately — durable but not crash-atomic. Also pending: `iter_corpus_rows` cursor-style API for stats / verify subcommands once the 3rd CF lands.
  </implementation_state>
  <layer>logic</layer>
  <sequence>21</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-08</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-007</task_ref>
    <task_ref>TASK-CORPUS-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. The RocksDB-backed persistence layer for corpus
rows. Atomic WriteBatch contract for the
(CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS) pair
(EC-CORPUS-09 — failure on either CF rolls back both). Per
NFR-CORPUS-09 every persisted row is re-readable independently
of the generator code via standard RocksDB tooling.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (OracleStore)</file>
  <file purpose="cf_schemas">../../technical/TECH-CORPUS.md#rocksdb_column_families</file>
  <file purpose="atomicity_rule">../../functional/SPEC-CORPUS.md (EC-CORPUS-09)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/oracle.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-007 complete (CorpusProvenance type exists)</check>
  <check>TASK-CORPUS-008 complete (CF descriptors + key builders exist)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub struct OracleStore { db: rocksdb::DB }` with:
      - `pub fn open(path: &amp;Path) -&gt; OracleResult&lt;Self&gt;`:
        opens or creates the DB with cf_descriptors() and
        db_options(); fail-closed if any CF fails to open.
      - `pub fn put_corpus_row(&amp;self, task_id: &amp;str,
        category: MutationCategory, outcome: &amp;MutationOutcome,
        verdict: &amp;OracleVerdict) -&gt; OracleResult&lt;()&gt;`:
        builds ONE rocksdb::WriteBatch with both CF puts;
        commits atomically; failure returns
        MEJEPA_ORACLE_STORE; rollback is structural (the batch
        is the unit of durability).
      - `pub fn put_provenance(&amp;self, prov:
        &amp;CorpusProvenance) -&gt; OracleResult&lt;()&gt;`:
        single-row write into CF_MEJEPA_CORPUS_PROVENANCE keyed
        by (corpus_version=1, prov.embedder_version).
      - `pub fn get_corpus_row(&amp;self, task_id: &amp;str,
        category: MutationCategory) -&gt;
        OracleResult&lt;Option&lt;(MutationOutcome,
        OracleVerdict)&gt;&gt;`: point-read both CFs; return
        None if either is absent.
      - `pub fn get_provenance(&amp;self, corpus_version: u32,
        embedder_version: u32) -&gt;
        OracleResult&lt;Option&lt;CorpusProvenance&gt;&gt;`:
        point-read.
      - `pub fn iter_corpus_rows&lt;'a&gt;(&amp;'a self) -&gt;
        impl Iterator&lt;Item = OracleResult&lt;(String,
        MutationCategory, MutationOutcome, OracleVerdict)&gt;&gt;
        + 'a`: snapshot-backed full scan that joins the two
        primary CFs.
    - Values: serde_json::to_vec on MutationOutcome /
      OracleVerdict / CorpusProvenance (per REQ-CORPUS-08
      constraint that JSON is mandatory for these — bincode
      forbidden because skip_serializing_if corrupts in
      bincode).
    - Add tests:
      - Round-trip: write row, close DB, reopen, read; assert
        byte-equal (TC-CORPUS-10).
      - Atomicity: simulated WriteBatch failure rolls back both
        CFs (EC-CORPUS-09; use a corrupted CF handle to force
        failure).
      - get_corpus_row returns None when either CF is missing.
      - iter_corpus_rows yields all written rows in CF order.
      - put_provenance + get_provenance round-trip.
  </in_scope>
  <out_of_scope>
    - The corpus_sha computer (TASK-CORPUS-022 wires the
      put_provenance call from generate).
    - The batch generator (TASK-CORPUS-023).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/oracle.rs">
    pub struct OracleStore { /* private fields */ }

    impl OracleStore {
        pub fn open(path: &amp;std::path::Path) -&gt; OracleResult&lt;Self&gt;;

        pub fn put_corpus_row(
            &amp;self,
            task_id: &amp;str,
            category: crate::MutationCategory,
            outcome: &amp;crate::MutationOutcome,
            verdict: &amp;crate::OracleVerdict,
        ) -&gt; OracleResult&lt;()&gt;;

        pub fn put_provenance(&amp;self, prov: &amp;crate::CorpusProvenance) -&gt; OracleResult&lt;()&gt;;

        pub fn get_corpus_row(
            &amp;self,
            task_id: &amp;str,
            category: crate::MutationCategory,
        ) -&gt; OracleResult&lt;Option&lt;(crate::MutationOutcome, crate::OracleVerdict)&gt;&gt;;

        pub fn get_provenance(
            &amp;self,
            corpus_version: u32,
            embedder_version: u32,
        ) -&gt; OracleResult&lt;Option&lt;crate::CorpusProvenance&gt;&gt;;

        pub fn iter_corpus_rows&lt;'a&gt;(
            &amp;'a self,
        ) -&gt; impl Iterator&lt;Item = OracleResult&lt;(String, crate::MutationCategory, crate::MutationOutcome, crate::OracleVerdict)&gt;&gt; + 'a;
    }
    </signature>
  </signatures>

  <constraints>
    - WriteBatch atomicity: BOTH puts in the SAME
      `rocksdb::WriteBatch`; one `db.write(batch)` call —
      partial state is impossible because the batch is the
      unit of durability.
    - Snapshot used in iter_corpus_rows so concurrent writers
      do not corrupt the iteration (per
      `<concurrency_model>` rule).
    - All errors carry context: `Store { operation: &amp;'static
      str, cf: &amp;'static str, message: String }`.
    - JSON not bincode for values (per memory key lessons:
      bincode + skip_serializing_if = BROKEN).
    - The DB path is canonicalized at open() to prevent path
      drift between writes and reads.
    - RocksDB v0.21.x compatible API; if v0.22.x is in workspace,
      use that — DO NOT introduce a new major.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- oracle_store` passes (≥ 5 tests).
    - `cargo test -p context-graph-mejepa-corpus -- write_batch_atomicity` passes (EC-CORPUS-09).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl OracleStore {
    pub fn open(path: &amp;Path) -&gt; OracleResult&lt;Self&gt; {
        let db = rocksdb::DB::open_cf_descriptors(&amp;db_options(), path, cf_descriptors())
            .map_err(|e| OracleError::Store { operation: "open", cf: "ALL", message: e.to_string() })?;
        Ok(Self { db })
    }

    pub fn put_corpus_row(&amp;self, task_id: &amp;str, category: MutationCategory,
                          outcome: &amp;MutationOutcome, verdict: &amp;OracleVerdict) -&gt; OracleResult&lt;()&gt; {
        let key = corpus_key(task_id, category)?;
        let outcome_v = serde_json::to_vec(outcome).map_err(|e| OracleError::Store { ... })?;
        let verdict_v = serde_json::to_vec(verdict).map_err(|e| OracleError::Store { ... })?;
        let cf_corpus = self.db.cf_handle(CF_MEJEPA_MUTATION_CORPUS).ok_or_else(|| ...)?;
        let cf_verdicts = self.db.cf_handle(CF_MEJEPA_ORACLE_VERDICTS).ok_or_else(|| ...)?;
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(&amp;cf_corpus, &amp;key, &amp;outcome_v);
        batch.put_cf(&amp;cf_verdicts, &amp;key, &amp;verdict_v);
        self.db.write(batch).map_err(|e| OracleError::Store { operation: "write_batch", cf: "MUTATION_CORPUS+ORACLE_VERDICTS", message: e.to_string() })
    }
}

#[test]
fn put_get_round_trip_close_reopen() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = OracleStore::open(dir.path()).unwrap();
        store.put_corpus_row("a", MutationCategory::KnownGood,
            &amp;MutationOutcome { ... }, &amp;OracleVerdict { ... }).unwrap();
    }
    let store = OracleStore::open(dir.path()).unwrap();
    let row = store.get_corpus_row("a", MutationCategory::KnownGood).unwrap().unwrap();
    assert_eq!(row.0.mutated_source, ...);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/oracle.rs">Implement OracleStore + 6 methods.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 5+ unit tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>WriteBatch atomicity: both CF puts succeed or neither (EC-CORPUS-09)</criterion>
  <criterion>Round-trip: write → close → reopen → read returns byte-equal data (TC-CORPUS-10)</criterion>
  <criterion>get_corpus_row returns None when either CF is absent</criterion>
  <criterion>iter_corpus_rows yields all written rows under a snapshot</criterion>
  <criterion>put_provenance + get_provenance round-trip</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- oracle_store</command>
  <command>cargo test -p context-graph-mejepa-corpus -- write_batch_atomicity</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

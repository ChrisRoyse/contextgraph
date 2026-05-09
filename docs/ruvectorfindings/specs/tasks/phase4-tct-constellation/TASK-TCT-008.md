# TASK-TCT-008 — ConstellationStore: CF_MEJEPA_CONSTELLATION FROZEN persist + load + provenance check

```xml
<task_spec id="TASK-TCT-008" version="1.0">

<metadata>
  <title>ConstellationStore in `crates/context-graph-mejepa-tct/src/store.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-TCT-05">CF_MEJEPA_CONSTELLATION RocksDB persist; key=(version_id, corpus_sha) 64-byte composite</item>
    <item ref="REQ-TCT-12">Provenance mismatch fail-closed at load (MEJEPA_TCT_PROVENANCE_MISMATCH)</item>
    <item ref="data_model:ConstellationStore">store.persist + load functions</item>
    <item ref="constitution:rocksdb_cfs">CF_ prefix; CF_MEJEPA_CONSTELLATION naming</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (store.rs)</estimated_files>
</metadata>

<context>
ConstellationStore is the FROZEN-at-rest persistence layer. Writes are append-only; updates produce
a new version_id with new corpus_provenance. Load checks provenance (per-embedder weight SHA-256
match runtime) and freshness (max-age — TASK-TCT-009 layers on top). Bincode2 fixed-int + LE
encoding ensures byte-equal output across platforms.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (rocksdb_cfs naming; SEC-MEJEPA-02; SEC-MEJEPA-04; performance_budgets vram_steady_state)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-05, 12; AC-TCT-04-02; ERR-TCT-05)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts ConstellationStore.persist + load; rocksdb tier policy in implementation_notes)</file>
  <file purpose="upstream_constellation">crates/context-graph-mejepa-tct/src/constellation.rs (TctConstellation, version_id, check_provenance)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-001/002/003/004/005 complete; TctConstellation finalized via builder</check>
  <check>`rocksdb = "0.21"` workspace dep</check>
  <check>`bincode = "1"` or `bincode2` workspace dep</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/store.rs` with:
      - `pub const CF_MEJEPA_CONSTELLATION: &str = "CF_MEJEPA_CONSTELLATION";`
      - `pub struct ConstellationStore { db: Arc<rocksdb::DB> }`
      - `pub fn new(db: Arc<rocksdb::DB>) -> Result<Self, TctError>` validating CF exists
      - `pub fn persist(&self, constellation: &TctConstellation) -> Result<[u8; 32], TctError>` — bincode-serialize → key = `version_id || corpus_sha` (64 bytes) → put_cf → return version_id
      - `pub fn load(&self, version_id: [u8; 32], runtime_embedder_versions: &BTreeMap<EmbedderId, [u8; 32]>) -> Result<TctConstellation, TctError>` — find prefix=version_id; deserialize; call check_provenance (TASK-TCT-009 will layer freshness check on top); paranoid version_id integrity check
      - `pub fn list_versions(&self, limit: usize) -> Result<Vec<[u8; 32]>, TctError>` — for operator inspection
    - Add `pub fn check_provenance(&self, runtime_embedder_versions: &BTreeMap<EmbedderId, [u8; 32]>) -> Result<(), TctError>` to `impl TctConstellation` (per TECH-TCT.check_provenance behavior)
    - Add integration tests using `tempfile::TempDir + rocksdb::DB::open_default`:
      - `constellation_store_persist_load_round_trip`: persist + load returns byte-equal serialization
      - `constellation_store_load_provenance_mismatch_fails_closed`: load with one mismatched embedder digest → ProvenanceMismatch
      - `constellation_store_load_unknown_version_id_fails`: load non-existent version_id → DimMismatch
      - `constellation_store_paranoid_version_id_integrity_check_catches_corruption`: synthetic byte flip in stored value → version_id mismatch detected at load
      - `constellation_store_list_versions_returns_all_persisted`: persist 3 → list_versions returns 3
      - `constellation_store_persist_is_append_only`: persist same constellation twice produces same key (no duplicate); persist different constellation produces different key
  </in_scope>
  <out_of_scope>
    - Max-age policy / freshness check (TASK-TCT-009)
    - MCP inspector tool (TASK-TCT-011)
    - CF registration in central storage crate (Phase 6 SPEC-HYGIENE)
    - Tier compression (Phase 6 SPEC-HYGIENE)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/store.rs">
use std::sync::Arc;
use std::collections::BTreeMap;
use rocksdb::DB;
use crate::types::EmbedderId;
use crate::constellation::TctConstellation;
use crate::error::TctError;

pub const CF_MEJEPA_CONSTELLATION: &amp;str = "CF_MEJEPA_CONSTELLATION";

pub struct ConstellationStore {
    db: Arc&lt;DB&gt;,
}

impl ConstellationStore {
    pub fn new(db: Arc&lt;DB&gt;) -&gt; Result&lt;Self, TctError&gt;;
    pub fn persist(&amp;self, constellation: &amp;TctConstellation) -&gt; Result&lt;[u8; 32], TctError&gt;;
    pub fn load(
        &amp;self,
        version_id: [u8; 32],
        runtime_embedder_versions: &amp;BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
    ) -&gt; Result&lt;TctConstellation, TctError&gt;;
    pub fn list_versions(&amp;self, limit: usize) -&gt; Result&lt;Vec&lt;[u8; 32]&gt;, TctError&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/constellation.rs">
impl TctConstellation {
    pub fn check_provenance(
        &amp;self,
        runtime_embedder_versions: &amp;BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
    ) -&gt; Result&lt;(), TctError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - CF name MUST be `CF_MEJEPA_CONSTELLATION` per constitution rocksdb_cfs naming
    - Composite key: 64 bytes = version_id (32) || corpus_sha (32); enables prefix iteration by version_id
    - Serialization: `bincode::config::standard().with_fixed_int_encoding().with_little_endian()` for byte-equal output across platforms
    - check_provenance: ALL 21 embedder digests must match; mismatch on ANY → ProvenanceMismatch with the offending embedder + expected_sha + observed_sha
    - load: paranoid integrity check — recompute version_id from deserialized constellation; mismatch with key version_id → DimMismatch
    - list_versions: BTreeMap iteration provides deterministic order
    - All `?` sites wrap `e.log_context(file!())`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct store` shows ≥ 6 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - Round-trip persist/load preserves bytes
    - Provenance mismatch returns MEJEPA_TCT_PROVENANCE_MISMATCH
    - Paranoid integrity catches byte corruption
    - CF name == "CF_MEJEPA_CONSTELLATION"
  </verification>
</definition_of_done>

<pseudo_code>
impl ConstellationStore {
    pub fn new(db: Arc<DB>) -> Result<Self, TctError> {
        if db.cf_handle(CF_MEJEPA_CONSTELLATION).is_none() {
            return Err(TctError::DimMismatch { expected: 1, actual: 0 });
        }
        Ok(Self { db })
    }

    pub fn persist(&self, constellation: &TctConstellation) -> Result<[u8; 32], TctError> {
        let cf = self.db.cf_handle(CF_MEJEPA_CONSTELLATION).unwrap();
        let bytes = bincode::serialize(constellation)
            .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
        let version_id = constellation.version_id();
        let mut key = Vec::with_capacity(64);
        key.extend_from_slice(&version_id);
        key.extend_from_slice(&constellation.corpus_provenance.corpus_sha);
        self.db.put_cf(cf, key, bytes)
            .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
        Ok(version_id)
    }

    pub fn load(&self, version_id, runtime_embedder_versions) -> Result<TctConstellation, TctError> {
        let cf = self.db.cf_handle(CF_MEJEPA_CONSTELLATION).unwrap();
        let mut iter = self.db.prefix_iterator_cf(cf, &version_id);
        if let Some(Ok((_k, v))) = iter.next() {
            let constellation: TctConstellation = bincode::deserialize(&v)
                .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
            constellation.check_provenance(runtime_embedder_versions)?;
            // Paranoid integrity check
            if constellation.version_id() != version_id {
                return Err(TctError::DimMismatch { expected: 32, actual: 0 });
            }
            return Ok(constellation);
        }
        Err(TctError::DimMismatch { expected: 1, actual: 0 })
    }
}

impl TctConstellation {
    pub fn check_provenance(&self, runtime: &BTreeMap<EmbedderId, [u8; 32]>) -> Result<(), TctError> {
        for (emb, expected) in &self.corpus_provenance.embedder_versions {
            match runtime.get(emb) {
                Some(observed) if observed == expected => continue,
                Some(observed) => return Err(TctError::ProvenanceMismatch {
                    embedder: *emb, expected: *expected, observed: *observed,
                }),
                None => return Err(TctError::ProvenanceMismatch {
                    embedder: *emb, expected: *expected, observed: [0u8; 32],
                }),
            }
        }
        Ok(())
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/store.rs">ConstellationStore + 6 integration tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/constellation.rs">add `check_provenance` method to `impl TctConstellation` + tests</file>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod store;`</file>
  <file path="crates/context-graph-mejepa-tct/Cargo.toml">add `rocksdb`, `bincode`, dev-dep `tempfile`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct store shows ≥ 6 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>CF name == "CF_MEJEPA_CONSTELLATION"</criterion>
  <criterion>Round-trip persist/load byte-equal</criterion>
  <criterion>Provenance mismatch returns MEJEPA_TCT_PROVENANCE_MISMATCH</criterion>
  <criterion>Paranoid integrity catches byte corruption</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct store</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

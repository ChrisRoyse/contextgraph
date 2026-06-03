use crate::materialize::TimeStep;
use crate::panel_graph::PanelGraphEnvelope;
use crate::panel_json::PanelEnvelope;
use crate::{InstrumentError, InstrumentResult, PANEL_DIM};
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, DB};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub const CF_MEJEPA_PANELS: &str = "mejepa_panels";
pub const CF_MEJEPA_PANEL_GRAPHS: &str = "mejepa_panel_graphs";
pub const CF_MEJEPA_PANEL_META: &str = "mejepa_panel_meta";
pub const MEJEPA_PANEL_CFS: &[&str] = &[
    CF_MEJEPA_PANELS,
    CF_MEJEPA_PANEL_GRAPHS,
    CF_MEJEPA_PANEL_META,
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelKey {
    pub attempt_id: String,
    pub time_step: TimeStep,
}

impl PanelKey {
    pub fn new(attempt_id: impl Into<String>, time_step: TimeStep) -> InstrumentResult<Self> {
        let key = Self {
            attempt_id: attempt_id.into(),
            time_step,
        };
        key.validate()?;
        Ok(key)
    }

    pub fn storage_key(&self) -> String {
        format!("{}/{}", self.attempt_id, time_step_slug(self.time_step))
    }

    fn validate(&self) -> InstrumentResult<()> {
        if self.attempt_id.trim().is_empty() {
            return Err(InstrumentError::invalid(
                "PanelKey.attempt_id",
                "attempt_id is empty",
                "use the SWE-bench task id, corpus row id, or stable event id as the panel key",
            ));
        }
        if self.attempt_id.contains('\0') || self.attempt_id.contains('/') {
            return Err(InstrumentError::invalid(
                "PanelKey.attempt_id",
                format!(
                    "attempt_id {:?} contains a forbidden separator",
                    self.attempt_id
                ),
                "use a slash-free UTF-8 identifier so RocksDB keys remain unambiguous",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelMeta {
    pub key: PanelKey,
    pub schema_version: u8,
    pub panel_hash: String,
    pub panel_dim: usize,
    pub filled_mask: u16,
    pub corpus_sha: String,
    pub source_sha256: String,
}

impl PanelMeta {
    fn from_envelope(key: PanelKey, envelope: &PanelEnvelope) -> Self {
        Self {
            key,
            schema_version: envelope.schema_version,
            panel_hash: envelope.panel_hash.clone(),
            panel_dim: PANEL_DIM,
            filled_mask: envelope.panel.filled_mask(),
            corpus_sha: envelope.provenance.corpus_sha.clone(),
            source_sha256: envelope.provenance.source_sha256.clone(),
        }
    }
}

pub struct PanelStore {
    db: DB,
}

impl PanelStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);

        let descriptors = MEJEPA_PANEL_CFS
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, cf_options()))
            .collect::<Vec<_>>();
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                "<all>",
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        for cf in MEJEPA_PANEL_CFS {
            if db.cf_handle(cf).is_none() {
                return Err(InstrumentError::store(
                    "open",
                    cf,
                    "column family missing after RocksDB open",
                    "open the ME-JEPA panel store with the canonical column-family descriptors",
                ));
            }
        }
        Ok(Self { db })
    }

    pub fn put_envelope(&self, key: &PanelKey, envelope: &PanelEnvelope) -> InstrumentResult<()> {
        key.validate()?;
        envelope.validate()?;
        let storage_key = key.storage_key();
        let panel_bytes = serde_json::to_vec(envelope).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_PANELS,
                err.to_string(),
                "ensure PanelEnvelope remains JSON-serializable",
            )
        })?;
        let meta = PanelMeta::from_envelope(key.clone(), envelope);
        let meta_bytes = serde_json::to_vec(&meta).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_PANEL_META,
                err.to_string(),
                "ensure PanelMeta remains JSON-serializable",
            )
        })?;
        let mut batch = WriteBatch::default();
        batch.put_cf(
            self.cf(CF_MEJEPA_PANELS)?,
            storage_key.as_bytes(),
            panel_bytes,
        );
        batch.put_cf(
            self.cf(CF_MEJEPA_PANEL_META)?,
            storage_key.as_bytes(),
            meta_bytes,
        );
        self.db.write(batch).map_err(|err| {
            InstrumentError::store(
                "write_batch",
                "<panel_envelope>",
                err.to_string(),
                "inspect RocksDB write permissions, WAL state, and disk capacity",
            )
        })
    }

    pub fn get_envelope(&self, key: &PanelKey) -> InstrumentResult<Option<PanelEnvelope>> {
        key.validate()?;
        let storage_key = key.storage_key();
        let Some(bytes) = self
            .db
            .get_cf(self.cf(CF_MEJEPA_PANELS)?, storage_key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get",
                    CF_MEJEPA_PANELS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
        else {
            return Ok(None);
        };
        let envelope: PanelEnvelope = serde_json::from_slice(&bytes).map_err(|err| {
            InstrumentError::store(
                "deserialize",
                CF_MEJEPA_PANELS,
                err.to_string(),
                "do not mutate persisted ME-JEPA panel envelopes outside this API",
            )
        })?;
        envelope.validate()?;
        Ok(Some(envelope))
    }

    pub fn get_meta(&self, key: &PanelKey) -> InstrumentResult<Option<PanelMeta>> {
        key.validate()?;
        let storage_key = key.storage_key();
        let Some(bytes) = self
            .db
            .get_cf(self.cf(CF_MEJEPA_PANEL_META)?, storage_key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get",
                    CF_MEJEPA_PANEL_META,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
        else {
            return Ok(None);
        };
        let meta = serde_json::from_slice(&bytes).map_err(|err| {
            InstrumentError::store(
                "deserialize",
                CF_MEJEPA_PANEL_META,
                err.to_string(),
                "do not mutate persisted ME-JEPA panel metadata outside this API",
            )
        })?;
        Ok(Some(meta))
    }

    pub fn put_panel_graph(
        &self,
        key: &PanelKey,
        envelope: &PanelGraphEnvelope,
    ) -> InstrumentResult<()> {
        key.validate()?;
        envelope.validate()?;
        let storage_key = key.storage_key();
        let graph_bytes = serde_json::to_vec(envelope).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_PANEL_GRAPHS,
                err.to_string(),
                "ensure PanelGraphEnvelope remains JSON-serializable",
            )
        })?;
        self.db
            .put_cf(
                self.cf(CF_MEJEPA_PANEL_GRAPHS)?,
                storage_key.as_bytes(),
                graph_bytes,
            )
            .map_err(|err| {
                InstrumentError::store(
                    "put",
                    CF_MEJEPA_PANEL_GRAPHS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })
    }

    pub fn get_panel_graph(&self, key: &PanelKey) -> InstrumentResult<Option<PanelGraphEnvelope>> {
        key.validate()?;
        let storage_key = key.storage_key();
        let Some(bytes) = self
            .db
            .get_cf(self.cf(CF_MEJEPA_PANEL_GRAPHS)?, storage_key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get",
                    CF_MEJEPA_PANEL_GRAPHS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
        else {
            return Ok(None);
        };
        let envelope: PanelGraphEnvelope = serde_json::from_slice(&bytes).map_err(|err| {
            InstrumentError::store(
                "deserialize",
                CF_MEJEPA_PANEL_GRAPHS,
                err.to_string(),
                "do not mutate persisted ME-JEPA PanelGraph envelopes outside this API",
            )
        })?;
        envelope.validate()?;
        Ok(Some(envelope))
    }

    pub fn count_cf(&self, cf_name: &'static str) -> InstrumentResult<usize> {
        let cf = self.cf(cf_name)?;
        let mut count = 0usize;
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    cf_name,
                    err.to_string(),
                    "inspect RocksDB iterator state and column-family health",
                )
            })?;
            count += 1;
        }
        Ok(count)
    }

    pub fn scan_cf_json(&self, cf_name: &'static str) -> InstrumentResult<Vec<(String, Value)>> {
        let cf = self.cf(cf_name)?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    cf_name,
                    err.to_string(),
                    "inspect RocksDB iterator state and column-family health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    cf_name,
                    err.to_string(),
                    "only use UTF-8 ME-JEPA panel keys",
                )
            })?;
            let value = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "decode_value",
                    cf_name,
                    err.to_string(),
                    "only persist JSON ME-JEPA panel records through this API",
                )
            })?;
            rows.push((key, value));
        }
        Ok(rows)
    }

    pub fn last_key(&self, cf_name: &'static str) -> InstrumentResult<Option<String>> {
        let cf = self.cf(cf_name)?;
        let mut iter = self.db.iterator_cf(cf, IteratorMode::End);
        let Some(item) = iter.next() else {
            return Ok(None);
        };
        let (key, _) = item.map_err(|err| {
            InstrumentError::store(
                "iterate_reverse",
                cf_name,
                err.to_string(),
                "inspect RocksDB iterator state and column-family health",
            )
        })?;
        String::from_utf8(key.to_vec()).map(Some).map_err(|err| {
            InstrumentError::store(
                "decode_key",
                cf_name,
                err.to_string(),
                "only use UTF-8 ME-JEPA panel keys",
            )
        })
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush().map_err(|err| {
            InstrumentError::store(
                "flush",
                "<all>",
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn cf(&self, name: &'static str) -> InstrumentResult<&ColumnFamily> {
        self.db.cf_handle(name).ok_or_else(|| {
            InstrumentError::store(
                "cf_handle",
                name,
                "column family handle not found",
                "open the store through PanelStore::open so all required CFs exist",
            )
        })
    }
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts
}

fn time_step_slug(time_step: TimeStep) -> &'static str {
    match time_step {
        TimeStep::T0 => "t0",
        TimeStep::T1 => "t1",
        TimeStep::T2 => "t2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChunkEdge, EdgeKind, InstrumentSlot, PanelBuilder, PanelGraph, PanelGraphDoctrine,
        PanelGraphEnvelope, PanelGraphNode, PanelProvenance,
    };
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;

    fn panel() -> crate::Panel {
        let mut builder = PanelBuilder::new();
        builder
            .set_slot(
                InstrumentSlot::EOracle,
                &vec![1.0; InstrumentSlot::EOracle.dim()],
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn envelope() -> PanelEnvelope {
        PanelEnvelope::try_new(
            TimeStep::T2,
            panel(),
            PanelProvenance {
                code_version: "test-sha".into(),
                embedder_versions: BTreeMap::from([("e_oracle".into(), "deterministic-v1".into())]),
                corpus_sha: "a".repeat(64),
                frozen_at_unix_ms: 1,
                source_sha256: "b".repeat(64),
            },
        )
        .unwrap()
    }

    fn lower_hex(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
        }
        out
    }

    fn hash(value: &str) -> String {
        lower_hex(&Sha256::digest(value.as_bytes()))
    }

    fn panel_graph_envelope() -> PanelGraphEnvelope {
        let nodes = vec![
            PanelGraphNode::try_new("pkg/a.py::chunk:1", panel()).unwrap(),
            PanelGraphNode::try_new("pkg/b.py::chunk:1", panel()).unwrap(),
        ];
        let edges = vec![ChunkEdge::try_new(
            "pkg/a.py::chunk:1",
            "pkg/b.py::chunk:1",
            EdgeKind::Calls,
            hash("call-edge"),
        )
        .unwrap()];
        let graph = PanelGraph::try_new(
            nodes,
            edges,
            hash("dependency-subgraph"),
            "pkg/a.py::chunk:1",
            PanelGraphDoctrine::preserved(),
        )
        .unwrap();
        PanelGraphEnvelope::try_new(
            graph,
            PanelProvenance {
                code_version: "test-sha".into(),
                embedder_versions: BTreeMap::from([("e_oracle".into(), "deterministic-v1".into())]),
                corpus_sha: "a".repeat(64),
                frozen_at_unix_ms: 1,
                source_sha256: "b".repeat(64),
            },
        )
        .unwrap()
    }

    #[test]
    fn panel_store_round_trips_envelope_and_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let key = PanelKey::new("sympy__sympy-20590", TimeStep::T2).unwrap();
        let envelope = envelope();
        {
            let store = PanelStore::open(tmp.path()).unwrap();
            store.put_envelope(&key, &envelope).unwrap();
            store.flush().unwrap();
        }

        let reopened = PanelStore::open(tmp.path()).unwrap();
        assert_eq!(reopened.count_cf(CF_MEJEPA_PANELS).unwrap(), 1);
        assert_eq!(reopened.count_cf(CF_MEJEPA_PANEL_META).unwrap(), 1);
        let readback = reopened.get_envelope(&key).unwrap().unwrap();
        assert_eq!(readback, envelope);
        let meta = reopened.get_meta(&key).unwrap().unwrap();
        assert_eq!(meta.panel_hash, envelope.panel_hash);
        assert_eq!(meta.filled_mask, envelope.panel.filled_mask());
    }

    #[test]
    fn panel_store_round_trips_panel_graph() {
        let tmp = tempfile::tempdir().unwrap();
        let key = PanelKey::new("sympy__sympy-20590", TimeStep::T1).unwrap();
        let graph = panel_graph_envelope();
        {
            let store = PanelStore::open(tmp.path()).unwrap();
            store.put_panel_graph(&key, &graph).unwrap();
            store.flush().unwrap();
        }

        let reopened = PanelStore::open(tmp.path()).unwrap();
        assert_eq!(reopened.count_cf(CF_MEJEPA_PANEL_GRAPHS).unwrap(), 1);
        let readback = reopened.get_panel_graph(&key).unwrap().unwrap();
        assert_eq!(readback, graph);
        assert_eq!(reopened.count_cf(CF_MEJEPA_PANELS).unwrap(), 0);
    }

    #[test]
    fn panel_key_rejects_ambiguous_ids() {
        assert_eq!(
            PanelKey::new("", TimeStep::T0).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        assert_eq!(
            PanelKey::new("bad/key", TimeStep::T0).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}

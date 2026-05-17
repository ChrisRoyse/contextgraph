use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_TYPE_GRAPH_SIGNALS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::e_code::{analyze_python_semantic_facts, CodeInstrumentInput, ETypeGraphInstrument};
use crate::features::fnv1a64;
use crate::{Instrument, InstrumentError, InstrumentResult};

const TYPE_GRAPH_SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeGraphSource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeGraphLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub path: String,
    pub analyzer: String,
    pub signature_count: u32,
    pub call_site_count: u32,
    pub known_call_site_count: u32,
    pub unknown_call_site_count: u32,
    pub typed_argument_count: u32,
    pub untyped_argument_count: u32,
    pub return_edge_count: u32,
    pub any_call_site_count: u32,
    pub arity_mismatch_count: u32,
    pub annotation_count: u32,
    pub generic_count: u32,
    pub union_optional_count: u32,
    pub literal_inference_count: u32,
    pub type_graph_vector_dim: u32,
    pub type_graph_vector_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum TypeGraphSignalRecord {
    Label(TypeGraphLabel),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedTypeGraphSignal {
    pub schema_version: u32,
    pub signal: TypeGraphSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TypeGraphExtraction {
    pub labels: Vec<TypeGraphLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeGraphRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub type_graph_json: PathBuf,
}

impl TypeGraphExtraction {
    pub fn persisted_records(&self) -> Vec<PersistedTypeGraphSignal> {
        self.labels
            .iter()
            .cloned()
            .map(|label| PersistedTypeGraphSignal {
                schema_version: TYPE_GRAPH_SCHEMA_VERSION,
                signal: TypeGraphSignalRecord::Label(label),
            })
            .collect()
    }
}

impl TypeGraphSignalRecord {
    fn identity_parts(&self) -> (&str, &str, &str) {
        match self {
            Self::Label(label) => (&label.corpus_row_id, &label.chunk_id, &label.path),
        }
    }
}

pub fn extract_type_graph_signals(
    source: &TypeGraphSource,
) -> InstrumentResult<TypeGraphExtraction> {
    validate_source(source)?;
    let input = CodeInstrumentInput {
        language: "python".to_string(),
        path: source.path.clone(),
        source: source.source_text.clone(),
    };
    let facts = analyze_python_semantic_facts(&input)?;
    if facts.analyzer != "python_ruff_semantic_v1" {
        return invalid(
            "type_graph.analyzer",
            format!("unexpected analyzer {}", facts.analyzer),
            "route Python E_TypeGraph through the Ruff semantic analyzer",
        );
    }
    let vector = ETypeGraphInstrument.encode(&input)?;
    let label = TypeGraphLabel {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        path: source.path.clone(),
        analyzer: facts.analyzer,
        signature_count: to_u32("signature_count", facts.type_signature_count)?,
        call_site_count: to_u32("call_site_count", facts.type_call_site_count)?,
        known_call_site_count: to_u32("known_call_site_count", facts.type_known_call_site_count)?,
        unknown_call_site_count: to_u32(
            "unknown_call_site_count",
            facts.type_unknown_call_site_count,
        )?,
        typed_argument_count: to_u32("typed_argument_count", facts.type_typed_argument_count)?,
        untyped_argument_count: to_u32(
            "untyped_argument_count",
            facts.type_untyped_argument_count,
        )?,
        return_edge_count: to_u32("return_edge_count", facts.type_return_edge_count)?,
        any_call_site_count: to_u32("any_call_site_count", facts.type_any_call_site_count)?,
        arity_mismatch_count: to_u32("arity_mismatch_count", facts.type_arity_mismatch_count)?,
        annotation_count: to_u32("annotation_count", facts.type_annotated_binding_count)?,
        generic_count: to_u32("generic_count", facts.type_generic_count)?,
        union_optional_count: to_u32("union_optional_count", facts.type_union_optional_count)?,
        literal_inference_count: to_u32(
            "literal_inference_count",
            facts.type_literal_inference_count,
        )?,
        type_graph_vector_dim: to_u32("type_graph_vector_dim", vector.len())?,
        type_graph_vector_sha256: sha256_f32_vector(&vector),
    };
    Ok(TypeGraphExtraction {
        labels: vec![label],
    })
}

pub fn write_type_graph_raw_output(
    root: impl AsRef<Path>,
    row_id: &str,
    extraction: &TypeGraphExtraction,
) -> InstrumentResult<TypeGraphRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-type-graph-signals-v1",
            err.to_string(),
            "ensure the D-root type-graph raw-output directory is writable",
        )
    })?;
    let paths = TypeGraphRawOutputPaths {
        row_id: row_id.to_string(),
        root: root.clone(),
        type_graph_json: root.join("type_graph.json"),
    };
    write_json_readback(&paths.type_graph_json, extraction)?;
    Ok(paths)
}

pub struct TypeGraphSignalStore {
    db: DB,
}

impl TypeGraphSignalStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_TYPE_GRAPH_SIGNALS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        if db.cf_handle(CF_MEJEPA_TYPE_GRAPH_SIGNALS).is_none() {
            return Err(InstrumentError::store(
                "cf_handle",
                CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                "column family missing after RocksDB open",
                "open type-graph signal storage with the canonical CF descriptor",
            ));
        }
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &TypeGraphExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let records = extraction.persisted_records();
        let mut keys = Vec::new();
        for record in records {
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedTypeGraphSignal)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB iterator state and type-graph CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    err.to_string(),
                    "type-graph signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    err.to_string(),
                    "only mutate type-graph rows through TypeGraphSignalStore",
                )
            })?;
            rows.push((key, record));
        }
        Ok(rows)
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedTypeGraphSignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                err.to_string(),
                "ensure type-graph signal records remain JSON-serializable",
            )
        })?;
        let key = type_graph_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self
            .db
            .get_cf(self.cf()?, key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get_cf",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
            .ok_or_else(|| {
                InstrumentError::store(
                    "read_after_write",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    "type-graph row missing after put_cf",
                    "do not advance reward-signal checkpoints until the CF row is readable",
                )
            })?;
        if readback != value {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                "readback bytes differ from written type-graph payload",
                "inspect RocksDB WAL/device integrity before continuing",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_TYPE_GRAPH_SIGNALS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_TYPE_GRAPH_SIGNALS,
                    "column-family handle not found",
                    "open the store through TypeGraphSignalStore::open",
                )
            })
    }
}

fn validate_source(source: &TypeGraphSource) -> InstrumentResult<()> {
    validate_single_line("source.corpus_row_id", &source.corpus_row_id)?;
    validate_single_line("source.chunk_id", &source.chunk_id)?;
    validate_no_nul("source.path", &source.path)?;
    if source.path.trim().is_empty() {
        return invalid(
            "source.path",
            "type-graph source path is empty",
            "persist the analyzed Python file path with every type-graph label",
        );
    }
    if source.source_text.trim().is_empty() {
        return invalid(
            "source.source_text",
            "type-graph source text is empty",
            "capture source text before extracting call-site type evidence",
        );
    }
    if source.source_text.len() > MAX_SOURCE_BYTES {
        return invalid(
            "source.source_text",
            format!(
                "source text is {} bytes, exceeding max {MAX_SOURCE_BYTES}",
                source.source_text.len()
            ),
            "shard very large Python files before type-graph signal extraction",
        );
    }
    if source.source_text.chars().any(|ch| ch == '\0') {
        return invalid(
            "source.source_text",
            "source text contains a NUL byte",
            "persist parser-facing UTF-8 source without embedded NUL bytes",
        );
    }
    Ok(())
}

fn type_graph_record_key(record: &PersistedTypeGraphSignal, value: &[u8]) -> String {
    let (row_id, chunk_id, discriminator) = record.signal.identity_parts();
    format!(
        "type_graph::{row_id}::{chunk_id}::{discriminator}::{:016x}",
        fnv1a64(value)
    )
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_single_line(field, value)?;
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return invalid(
            field,
            format!("{field} must be a single safe path component, got {value:?}"),
            "use a stable row id without path separators for raw-output directories",
        );
    }
    Ok(())
}

fn validate_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_no_nul(field, value)?;
    if value.trim().is_empty() || value.contains('\n') || value.contains('\r') {
        return invalid(
            field,
            format!("{field} must be non-empty single-line text"),
            "persist canonical single-line identifiers in type-graph rows",
        );
    }
    Ok(())
}

fn validate_no_nul(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.chars().any(|ch| ch == '\0') {
        return invalid(
            field,
            format!("{field} contains a NUL byte"),
            "reject analyzer text with embedded NUL bytes before persistence",
        );
    }
    Ok(())
}

fn to_u32(field: &'static str, value: usize) -> InstrumentResult<u32> {
    u32::try_from(value).map_err(|_| {
        InstrumentError::invalid(
            field,
            format!("{field} value {value} exceeds u32"),
            "shard type-graph extraction before counter overflow",
        )
    })
}

fn sha256_f32_vector(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn write_json_readback<T: Serialize>(path: &Path, value: &T) -> InstrumentResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| {
        InstrumentError::store(
            "serialize",
            "python-type-graph-signals-v1",
            err.to_string(),
            "ensure type-graph raw output remains JSON-serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write",
            "python-type-graph-signals-v1",
            err.to_string(),
            "ensure the raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_after_write",
            "python-type-graph-signals-v1",
            err.to_string(),
            "verify type-graph raw-output bytes after writing",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write",
            "python-type-graph-signals-v1",
            "raw-output readback bytes differ from written payload",
            "do not advance reward-signal checkpoints until raw bytes match",
        ));
    }
    Ok(())
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

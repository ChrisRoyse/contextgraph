# PRD 04: Storage Architecture

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Core Principle: Everything on YOUR Machine

**CaseTrack stores ALL data locally on the user's device:**

- **Embedding models**: Downloaded once, stored in `~/Documents/CaseTrack/models/`
- **Vector embeddings**: Stored in RocksDB on your device
- **Document chunks**: Stored in RocksDB on your device
- **Case databases**: Each case is an isolated RocksDB instance
- **Original documents**: Optionally copied to your CaseTrack folder

**Nothing is sent to any cloud service. Ever.**

---

## 2. Directory Structure

```
~/Documents/CaseTrack/                       <-- All CaseTrack data lives here
|
|-- config.toml                              <-- User configuration (optional)
|
|-- models/                                  <-- Embedding models (~400MB)
|   |-- bge-small-en-v1.5/                     Downloaded on first use
|   |   |-- model.onnx                         Cached permanently
|   |   +-- tokenizer.json                     No re-download needed
|   |-- splade-distil/
|   |-- minilm-l6/
|   +-- colbert-small/
|
|-- registry.db/                             <-- Case index (RocksDB)
|   +-- [case metadata, schema version]
|
+-- cases/                                   <-- Per-case databases
    |
    |-- {case-uuid-1}/                       <-- Case "Smith v. Jones"
    |   |-- case.db/                           (Isolated RocksDB instance)
    |   |   |-- documents     CF              Document metadata
    |   |   |-- chunks        CF              Text chunks (bincode)
    |   |   |-- embeddings    CF              Vector embeddings (f32 arrays)
    |   |   |   |-- e1_legal                  384D vectors
    |   |   |   |-- e6_legal                  Sparse vectors
    |   |   |   |-- e7                        384D vectors
    |   |   |   +-- ...                       All active embedders
    |   |   |-- provenance    CF              Source location tracking
    |   |   +-- bm25_index    CF              Inverted index for keyword search
    |   +-- originals/                        Original files (optional copy)
    |       |-- Complaint.pdf
    |       +-- Contract.docx
    |
    |-- {case-uuid-2}/                       <-- Case "Doe v. Corp"
    |   +-- ...                                (Completely isolated)
    |
    +-- {case-uuid-N}/                       <-- More cases...

CF = RocksDB Column Family
```

---

## 3. Storage Estimates

| Data Type | Size Per Unit | Notes |
|-----------|---------------|-------|
| Models (Free tier) | ~165MB total | E1 + E6 + E7 (one-time download) |
| Models (Pro tier) | ~370MB total | All 7 models (one-time download) |
| Registry DB | ~1MB | Scales with number of cases |
| Per document page (Free) | ~30KB | 3 embeddings + chunk text + provenance |
| Per document page (Pro) | ~50KB | 6 embeddings + chunk text + provenance |
| 100-page case (Free) | ~3MB | |
| 100-page case (Pro) | ~5MB | |
| 1000-page case (Pro) | ~50MB | |
| BM25 index per 1000 chunks | ~2MB | Inverted index |

**Example total disk usage:**
- Free tier, 3 cases of 100 pages each: 165MB (models) + 9MB (data) = **~175MB**
- Pro tier, 10 cases of 500 pages each: 370MB (models) + 250MB (data) = **~620MB**

---

## 4. RocksDB Configuration

### 4.1 Why RocksDB

| Requirement | RocksDB | SQLite | LMDB |
|-------------|---------|--------|------|
| Embedded (no server) | Yes | Yes | Yes |
| Column families (namespacing) | Yes | No (tables) | No |
| Prefix iteration | Yes | No | Limited |
| Bulk write performance | Excellent | Good | Good |
| Concurrent reads | Excellent | Limited (WAL) | Excellent |
| Rust crate quality | Good (rust-rocksdb) | Good (rusqlite) | Fair |
| Per-case isolation | Separate DB instances | Separate files | Separate files |

RocksDB was chosen for: column families (clean separation of data types), prefix iteration (efficient case listing), and bulk write performance (ingestion throughput).

### 4.2 Column Family Schema

Each case database uses these column families:

```rust
pub const COLUMN_FAMILIES: &[&str] = &[
    "documents",    // Document metadata
    "chunks",       // Text chunk content
    "embeddings",   // Vector embeddings (all embedders)
    "provenance",   // Source location tracking
    "bm25_index",   // Inverted index for BM25
    "metadata",     // Case-level metadata, stats
];
```

### 4.3 Key Schema

```rust
// === Registry DB Keys ===

// Case listing
"case:{uuid}"                      -> bincode<Case>
"schema_version"                   -> u32 (current: 1)

// === Case DB Keys (per column family) ===

// documents CF
"doc:{uuid}"                       -> bincode<DocumentMetadata>

// chunks CF
"chunk:{uuid}"                     -> bincode<ChunkData>
"doc_chunks:{doc_uuid}:{seq}"      -> chunk_uuid  (index: chunks by document)

// embeddings CF
"e1:{chunk_uuid}"                  -> [f32; 384] as bytes
"e6:{chunk_uuid}"                  -> bincode<SparseVec>
"e7:{chunk_uuid}"                  -> [f32; 384] as bytes
"e8:{chunk_uuid}"                  -> [f32; 256] as bytes
"e11:{chunk_uuid}"                 -> [f32; 384] as bytes
"e12:{chunk_uuid}"                 -> bincode<TokenEmbeddings>

// provenance CF
"prov:{chunk_uuid}"                -> bincode<Provenance>

// bm25_index CF
"term:{term}"                      -> bincode<PostingList>
"doc_len:{doc_uuid}"               -> u32 (document length in tokens)
"stats"                            -> bincode<Bm25Stats> (avg doc length, total docs)

// metadata CF
"case_info"                        -> bincode<Case>
"stats"                            -> bincode<CaseStats>
```

### 4.4 RocksDB Tuning for Consumer Hardware

```rust
pub fn rocks_options() -> rocksdb::Options {
    let mut opts = rocksdb::Options::default();

    // Create column families if missing
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    // Memory budget: ~64MB per open database
    // (allows multiple cases open simultaneously on 8GB machines)
    let mut block_cache = rocksdb::Cache::new_lru_cache(32 * 1024 * 1024); // 32MB
    let mut table_opts = rocksdb::BlockBasedOptions::default();
    table_opts.set_block_cache(&block_cache);
    table_opts.set_block_size(16 * 1024); // 16KB blocks
    opts.set_block_based_table_factory(&table_opts);

    // Write buffer: 16MB (reduces write amplification)
    opts.set_write_buffer_size(16 * 1024 * 1024);
    opts.set_max_write_buffer_number(2);

    // Compression: LZ4 for speed, Zstd for bottom level
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);

    // Limit background threads (save CPU for embedding)
    opts.set_max_background_jobs(2);
    opts.increase_parallelism(2);

    opts
}
```

---

## 5. Serialization Format

### 5.1 Bincode for Structs

All Rust structs stored via `bincode` for fast, compact serialization:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct ChunkData {
    pub id: Uuid,
    pub document_id: Uuid,
    pub text: String,
    pub sequence: u32,        // Position within document
    pub token_count: u32,
}

#[derive(Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: Uuid,
    pub name: String,
    pub original_path: Option<String>,
    pub document_type: DocumentType,
    pub page_count: u32,
    pub chunk_count: u32,
    pub ingested_at: i64,     // Unix timestamp
    pub file_hash: String,    // SHA256 of original file (dedup detection)
    pub extraction_method: ExtractionMethod,
}
```

### 5.2 Embeddings as Raw Bytes

Dense vectors stored as raw `f32` byte arrays for zero-copy reads:

```rust
/// Store embedding
pub fn store_embedding(
    db: &rocksdb::DB,
    embedder: &str,
    chunk_id: &Uuid,
    embedding: &[f32],
) -> Result<()> {
    let key = format!("{}:{}", embedder, chunk_id);
    let bytes: &[u8] = bytemuck::cast_slice(embedding);
    let cf = db.cf_handle("embeddings").unwrap();
    db.put_cf(&cf, key.as_bytes(), bytes)?;
    Ok(())
}

/// Read embedding (zero-copy when possible)
pub fn load_embedding(
    db: &rocksdb::DB,
    embedder: &str,
    chunk_id: &Uuid,
) -> Result<Vec<f32>> {
    let key = format!("{}:{}", embedder, chunk_id);
    let cf = db.cf_handle("embeddings").unwrap();
    let bytes = db.get_cf(&cf, key.as_bytes())?
        .ok_or(CaseTrackError::EmbeddingNotFound)?;
    let embedding: &[f32] = bytemuck::cast_slice(&bytes);
    Ok(embedding.to_vec())
}
```

### 5.3 Sparse Vectors (SPLADE)

```rust
#[derive(Serialize, Deserialize)]
pub struct SparseVec {
    pub indices: Vec<u32>,    // Token IDs with non-zero weights
    pub values: Vec<f32>,     // Corresponding weights
}

impl SparseVec {
    pub fn dot(&self, other: &SparseVec) -> f32 {
        let mut i = 0;
        let mut j = 0;
        let mut sum = 0.0;

        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                Ordering::Equal => {
                    sum += self.values[i] * other.values[j];
                    i += 1;
                    j += 1;
                }
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
            }
        }

        sum
    }
}
```

---

## 6. Data Versioning & Migration

### 6.1 Schema Version Tracking

```rust
const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn check_and_migrate(registry_path: &Path) -> Result<()> {
    let db = rocksdb::DB::open_default(registry_path)?;

    let version = match db.get(b"schema_version")? {
        Some(bytes) => u32::from_le_bytes(bytes.try_into().unwrap()),
        None => {
            // Fresh install
            db.put(b"schema_version", CURRENT_SCHEMA_VERSION.to_le_bytes())?;
            return Ok(());
        }
    };

    if version == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    if version > CURRENT_SCHEMA_VERSION {
        return Err(CaseTrackError::FutureSchemaVersion {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    // Run migrations sequentially
    tracing::info!("Migrating database from v{} to v{}", version, CURRENT_SCHEMA_VERSION);

    // Backup first
    let backup_path = registry_path.with_extension(format!("bak.v{}", version));
    fs::copy_dir_all(registry_path, &backup_path)?;

    for v in version..CURRENT_SCHEMA_VERSION {
        match v {
            0 => migrate_v0_to_v1(&db)?,
            // Future migrations go here
            _ => unreachable!(),
        }
    }

    db.put(b"schema_version", CURRENT_SCHEMA_VERSION.to_le_bytes())?;
    tracing::info!("Migration complete.");

    Ok(())
}
```

### 6.2 Migration Rules

1. Migrations are **idempotent** (safe to re-run)
2. Always **back up** before migrating
3. Migration failures are **fatal** (don't start with corrupt data)
4. Each migration is a separate function with clear documentation
5. Never delete user data during migration -- only restructure

---

## 7. Isolation Guarantees

### 7.1 Per-Customer Isolation

Every CaseTrack installation is **fully isolated per customer**:

- CaseTrack installs on each customer's machine independently
- Each customer has their own `~/Documents/CaseTrack/` directory
- No data is shared between customers -- there is no server, no cloud, no shared state
- For Firm tier (5 seats), each seat is a separate installation on a separate machine with its own database
- Customer A's embeddings, vectors, chunks, and provenance records **never touch** Customer B's data
- There is no central database. Each customer IS their own database.

```
CUSTOMER ISOLATION
=================================================================================

Customer A (Sarah's MacBook)         Customer B (Mike's Windows PC)
~/Documents/CaseTrack/               C:\Users\Mike\Documents\CaseTrack\
|-- models/                          |-- models/
|-- registry.db                      |-- registry.db
+-- cases/                           +-- cases/
    |-- {sarah-case-1}/                  |-- {mike-case-1}/
    +-- {sarah-case-2}/                  |-- {mike-case-2}/
                                         +-- {mike-case-3}/

ZERO shared state. ZERO shared databases. ZERO network communication.
Each installation is a completely independent system.
```

### 7.2 Per-Case Isolation

Within a single customer's installation, each case is a **completely independent RocksDB instance**:

- **Separate database per case**: Each case is its own RocksDB instance on disk
- **Separate embeddings per case**: Embeddings from Case A are in a different database file than Case B
- **No cross-case queries**: Search operates within a single case only
- **No shared vectors**: Case A's vectors cannot influence Case B's search results
- **No embedding bleed**: There is no shared vector index -- each case has its own index files
- **Independent lifecycle**: Deleting Case A has zero impact on Case B
- **Portable**: A case directory can be copied to another machine
- **Cleanly deletable**: `rm -rf cases/{uuid}/` fully removes a case and all its embeddings

```rust
/// Opening a case creates or loads its isolated database
pub struct CaseHandle {
    db: rocksdb::DB,
    case_id: Uuid,
    case_dir: PathBuf,
}

impl CaseHandle {
    pub fn open(case_dir: &Path) -> Result<Self> {
        let db_path = case_dir.join("case.db");
        let mut opts = rocks_options();

        let cfs = COLUMN_FAMILIES.iter()
            .map(|name| rocksdb::ColumnFamilyDescriptor::new(*name, opts.clone()))
            .collect::<Vec<_>>();

        let db = rocksdb::DB::open_cf_descriptors(&opts, &db_path, cfs)?;

        Ok(Self {
            db,
            case_id: Uuid::parse_str(
                case_dir.file_name().unwrap().to_str().unwrap()
            )?,
            case_dir: case_dir.to_path_buf(),
        })
    }

    /// Delete this case entirely
    pub fn destroy(self) -> Result<()> {
        let path = self.case_dir.clone();
        drop(self); // Close DB handle first
        fs::remove_dir_all(&path)?;
        Ok(())
    }
}
```

---

## 8. Backup & Export

### 8.1 Case Export

Cases can be exported as portable archives:

```
casetrack export --case "Smith v. Jones" --output ~/Desktop/smith-v-jones.ctcase
```

The `.ctcase` file is a ZIP containing:
- `case.db/` -- RocksDB snapshot
- `originals/` -- Original documents (if stored)
- `manifest.json` -- Case metadata, schema version, embedder versions

### 8.2 Case Import

```
casetrack import ~/Desktop/smith-v-jones.ctcase
```

1. Validates schema version compatibility
2. Creates new case UUID (avoids collisions)
3. Copies database and originals to `cases/` directory
4. Registers in case registry

---

## 9. What's Stored Where (Summary)

| Data Type | Storage Location | Format | Size Per Unit |
|-----------|------------------|--------|---------------|
| Case metadata | `registry.db` | bincode via RocksDB | ~500 bytes/case |
| Document metadata | `cases/{id}/case.db` documents CF | bincode | ~200 bytes/doc |
| Text chunks (2000 chars) | `cases/{id}/case.db` chunks CF | bincode | ~2.5KB/chunk (text + provenance metadata) |
| E1 embeddings (384D) | `cases/{id}/case.db` embeddings CF | f32 bytes | 1,536 bytes/chunk |
| E6 sparse vectors | `cases/{id}/case.db` embeddings CF | bincode sparse | ~500 bytes/chunk |
| E7 embeddings (384D) | `cases/{id}/case.db` embeddings CF | f32 bytes | 1,536 bytes/chunk |
| E8 embeddings (256D) | `cases/{id}/case.db` embeddings CF | f32 bytes | 1,024 bytes/chunk |
| E11 embeddings (384D) | `cases/{id}/case.db` embeddings CF | f32 bytes | 1,536 bytes/chunk |
| E12 token embeddings | `cases/{id}/case.db` embeddings CF | bincode | ~8KB/chunk |
| BM25 inverted index | `cases/{id}/case.db` bm25_index CF | bincode | ~2MB/1000 chunks |
| Provenance records | `cases/{id}/case.db` provenance CF | bincode | ~300 bytes/chunk |
| Original documents | `cases/{id}/originals/` | original files | varies |
| ONNX models | `models/` | ONNX format | 35-110MB each |

---

*CaseTrack PRD v4.0.0 -- Document 4 of 10*

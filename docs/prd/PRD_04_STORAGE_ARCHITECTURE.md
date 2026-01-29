# PRD 04: Storage Architecture

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust | **Domain**: Legal
**Design Priority**: ACCURACY FIRST -- 768D embeddings for maximum legal semantic resolution

---

## 1. Core Principle: Everything on YOUR Machine

**CaseTrack stores ALL data locally on the user's device:**

- **Embedding models**: Downloaded once, stored in `~/Documents/CaseTrack/models/`
- **Vector embeddings**: Stored in RocksDB on your device
- **Document chunks**: Stored in RocksDB on your device
- **Case databases**: Each case is an isolated RocksDB instance
- **Original documents**: Optionally copied to your CaseTrack folder
- **Legal citations**: Extracted and indexed locally

**Nothing is sent to any cloud service. Ever. Attorney-client privilege preserved.**

---

## 2. Directory Structure

```
~/Documents/CaseTrack/                       <-- All CaseTrack data lives here
|
|-- config.toml                              <-- User configuration (optional)
|-- watches.json                             <-- Folder watch registry (auto-sync)
|
|-- models/                                  <-- Legal embedding models (~550MB)
|   |-- legal-bert-base-uncased/               Downloaded on first use
|   |   |-- model.onnx                         Legal-BERT-base (768D, 110M params)
|   |   +-- tokenizer.json                     Cached permanently
|   |-- splade-cocondenser-selfdistil/         SPLADE (~110M params)
|   +-- colbertv2.0/                           ColBERT-v2 (~110M params, 128D/token)
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
    |   |   |-- embeddings    CF              All embedder vectors + chunk text + provenance
    |   |   |   |-- e1                        768D vectors (Legal-BERT-base)
    |   |   |   |-- e6                        Sparse vectors (SPLADE)
    |   |   |   +-- ...                       All active embedders
    |   |   |-- bm25_index    CF              Inverted index for keyword search
    |   |   |-- citations     CF              Legal citation index
    |   |   +-- ...                           Additional column families
    |   +-- originals/                        Original files (optional copy)
    |       |-- Complaint.pdf
    |       |-- Motion_to_Dismiss.docx
    |       +-- Deposition_Smith.pdf
    |
    |-- {case-uuid-2}/                       <-- Case "Johnson Contract Dispute"
    |   +-- ...                                (Completely isolated)
    |
    +-- {case-uuid-N}/                       <-- More cases...

CF = RocksDB Column Family
```

---

## 3. Storage Estimates

| Data Type | Size Per Unit | Notes |
|-----------|---------------|-------|
| Models (Free tier) | ~330MB total | Legal-BERT-base + SPLADE (one-time download) |
| Models (Pro tier) | ~550MB total | All 3 models + BM25 algorithmic (one-time download) |
| Registry DB | ~1MB | Scales with number of cases |
| Per document page (Free) | ~40KB | 3 embeddings (768D) + chunk text + provenance + citations |
| Per document page (Pro) | ~60KB | 6 embeddings (768D) + chunk text + provenance + citations |
| 100-page case (Free) | ~4MB | |
| 100-page case (Pro) | ~6MB | |
| 1000-page case (Pro) | ~60MB | |
| BM25 index per 1000 chunks | ~2MB | Inverted index |
| Citation index per 1000 chunks | ~1MB | Legal citation cross-references |

**Example total disk usage:**
- Free tier, 3 cases of 100 pages each: 330MB (models) + 12MB (data) = **~342MB**
- Pro tier, 10 cases of 500 pages each: 550MB (models) + 300MB (data) = **~850MB**

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
    "documents",        // Document metadata
    "chunks",           // Text chunk content
    "embeddings",       // All embedder vectors + chunk text + provenance per chunk
    "bm25_index",       // Inverted index for BM25
    "metadata",         // Case-level metadata, stats

    // === Legal Citations ===
    "citations",        // Extracted legal citations (case law, statutes, regulations)
    "citation_index",   // Citation -> chunk mentions index
    "citation_graph",   // Citation-to-citation relationships (citing, distinguishing, overruling)

    // === Context Graph (relationships between documents, chunks, entities) ===
    "entities",         // Extracted entities (party, court, judge, attorney, statute, etc.)
    "entity_index",     // Entity -> chunk mentions index
    "references",       // Cross-document references (shared entities, citations, hyperlinks)
    "doc_graph",        // Document-to-document relationships (similarity, reference links)
    "chunk_graph",      // Chunk-to-chunk relationships (similarity edges, co-reference)
    "knowledge_graph",  // Entity-to-entity relationships, entity-to-chunk mappings
    "case_map",         // Case-level summary: key parties, dates, legal issues, document categories
];
```

### 4.3 Key Schema

```rust
// === Registry DB Keys ===

// Case listing
"case:{uuid}"                                -> bincode<Case>
"schema_version"                             -> u32 (current: 1)

// Folder watches (auto-sync)
"watch:{uuid}"                               -> bincode<FolderWatch>
"watch_case:{case_uuid}:{watch_uuid}"        -> watch_uuid  (index: watches by case)

// === Case DB Keys (per column family) ===

// documents CF
"doc:{uuid}"                       -> bincode<DocumentMetadata>

// chunks CF
"chunk:{uuid}"                     -> bincode<ChunkData>
"doc_chunks:{doc_uuid}:{seq}"      -> chunk_uuid  (index: chunks by document)

// embeddings CF -- each chunk stores all embedder vectors alongside text and provenance
// chunk_id -> { text, provenance, e1_vector, e6_vector, e12_vector, bm25_terms }
"emb:{chunk_uuid}"                 -> bincode<ChunkEmbeddingRecord>

// Legacy per-embedder keys (supported for migration)
"e1:{chunk_uuid}"                  -> [f32; 768] as bytes
"e6:{chunk_uuid}"                  -> bincode<SparseVec>
"e12:{chunk_uuid}"                 -> bincode<TokenEmbeddings>

// bm25_index CF
"term:{term}"                      -> bincode<PostingList>
"doc_len:{doc_uuid}"               -> u32 (document length in tokens)
"stats"                            -> bincode<Bm25Stats> (avg doc length, total docs)

// metadata CF
"case_info"                        -> bincode<Case>
"stats"                            -> bincode<CaseStats>

// === LEGAL CITATIONS COLUMN FAMILIES ===

// citations CF
"cite:{normalized_citation}"       -> bincode<LegalCitation>
"cite_type:{type}:{normalized}"    -> normalized_citation  (index: citations by type)

// citation_index CF (bidirectional)
"cite_chunks:{citation_key}"       -> bincode<Vec<CitationMention>>
"chunk_cites:{chunk_uuid}"         -> bincode<Vec<CitationRef>>

// citation_graph CF
"cite_rel:{citation_a}:{rel_type}:{citation_b}" -> bincode<CitationRelationship>
// rel_type = citing | distinguishing | overruling | following | questioning

// === CONTEXT GRAPH COLUMN FAMILIES ===

// entities CF
"entity:{type}:{normalized_name}"  -> bincode<Entity>
// type = party | court | judge | attorney | statute | case_number | jurisdiction |
//        legal_concept | remedy | witness | exhibit | docket_entry |
//        person | organization | date | amount | location

// entity_index CF (bidirectional)
"ent_chunks:{entity_key}"          -> bincode<Vec<EntityMention>>
"chunk_ents:{chunk_uuid}"          -> bincode<Vec<EntityRef>>

// references CF (cross-document references)
"ref:{reference_key}"              -> bincode<ReferenceRecord>
"ref_chunks:{reference_key}"       -> bincode<Vec<ReferenceMention>>
"chunk_refs:{chunk_uuid}"          -> bincode<Vec<ReferenceRef>>

// doc_graph CF
"doc_sim:{doc_a}:{doc_b}"         -> f32 (cosine similarity)
"doc_refs:{source_doc}:{target_doc}" -> bincode<DocReference>
"doc_entities:{doc_uuid}"         -> bincode<Vec<EntityRef>>
"doc_category:{category}:{doc_uuid}" -> doc_uuid

// chunk_graph CF
"chunk_sim:{chunk_a}:{chunk_b}"   -> f32 (stored only when > 0.7)
"chunk_coref:{chunk_a}:{chunk_b}" -> bincode<CoReference>  (shared entity co-reference)
"chunk_seq:{doc_uuid}:{seq}"      -> chunk_uuid

// knowledge_graph CF
"kg_ent_rel:{entity_a}:{rel_type}:{entity_b}" -> bincode<EntityRelationship>
"kg_ent_chunks:{entity_key}"       -> bincode<Vec<Uuid>>  (entity-to-chunk mappings)
"kg_chunk_ents:{chunk_uuid}"       -> bincode<Vec<String>> (chunk-to-entity mappings)

// case_map CF (rebuilt after ingestion)
"key_parties"                      -> bincode<Vec<PartyInfo>>
"key_dates"                        -> bincode<Vec<KeyDate>>
"key_topics"                       -> bincode<Vec<Topic>>
"legal_issues"                     -> bincode<Vec<LegalIssue>>
"key_citations"                    -> bincode<Vec<LegalCitation>>
"doc_categories"                   -> bincode<HashMap<String, Vec<Uuid>>>
"reference_stats"                  -> bincode<Vec<ReferenceStat>>
"entity_stats"                     -> bincode<Vec<EntityStat>>
```

### 4.4 RocksDB Tuning for Consumer Hardware

```rust
pub fn rocks_options() -> rocksdb::Options {
    let mut opts = rocksdb::Options::default();

    // Create column families if missing
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    // Memory budget: ~128MB per open database
    // (allows multiple cases open simultaneously on 16GB machines)
    let mut block_cache = rocksdb::Cache::new_lru_cache(64 * 1024 * 1024); // 64MB
    let mut table_opts = rocksdb::BlockBasedOptions::default();
    table_opts.set_block_cache(&block_cache);
    table_opts.set_block_size(16 * 1024); // 16KB blocks
    opts.set_block_based_table_factory(&table_opts);

    // Write buffer: 32MB (reduces write amplification)
    opts.set_write_buffer_size(32 * 1024 * 1024);
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
    pub sequence: u32,              // Position within document (0-indexed)
    pub char_count: u32,
    pub provenance: Provenance,     // Full source trace -- see Section 5.2 Provenance Chain
    pub created_at: i64,            // Unix timestamp
    pub embedded_at: i64,           // Unix timestamp: last embedding computation
    pub embedder_versions: Vec<String>, // e.g., ["e1", "e6"] for Free tier
}

#[derive(Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: Uuid,
    pub name: String,                     // Original filename
    pub original_path: Option<String>,    // Absolute path to source file
    pub document_type: DocumentType,
    pub page_count: u32,
    pub chunk_count: u32,
    pub ingested_at: i64,
    pub updated_at: i64,
    pub file_hash: String,                // SHA256 (dedup + staleness detection)
    pub file_size_bytes: u64,
    pub extraction_method: ExtractionMethod,
    pub embedder_coverage: Vec<String>,   // e.g., ["e1", "e6"]
    pub entity_count: u32,
    pub reference_count: u32,
    pub citation_count: u32,              // Legal citations found in this document
}

/// Unified embedding record: all embedder vectors stored alongside chunk text and provenance
#[derive(Serialize, Deserialize)]
pub struct ChunkEmbeddingRecord {
    pub chunk_id: Uuid,
    pub text: String,
    pub provenance: Provenance,
    pub e1_vector: Option<Vec<f32>>,       // 768D dense vector (Legal-BERT-base)
    pub e6_vector: Option<SparseVec>,      // SPLADE sparse vector
    pub e12_vector: Option<TokenEmbeddings>, // ColBERT-v2 per-token embeddings (128D/token)
    pub bm25_terms: Option<Vec<String>>,   // Pre-extracted BM25 terms
}
```

### 5.2 The Provenance Chain (How Embeddings Trace Back to Source)

```
PROVENANCE CHAIN -- EVERY VECTOR TRACES TO ITS SOURCE
=================================================================================

Embedding Vector (e.g., key "e1:{chunk_uuid}")
    |
    +---> chunk_uuid ---> ChunkData (key "chunk:{uuid}")
                           |
                           +-- text: "Defendant's motion to dismiss under Rule 12(b)(6)..."
                           +-- provenance: Provenance {
                           |       document_id:        "doc-abc"
                           |       source_file_path:   "/Users/maria/Cases/Smith_v_Jones/Motion_to_Dismiss.pdf"
                           |       document_filename:  "Motion_to_Dismiss.pdf"
                           |       page_number:        3
                           |       paragraph_number:   5
                           |       line_number:        1
                           |       char_start:         2401
                           |       char_end:           4401
                           |       extraction_method:  Native
                           |       ocr_confidence:     None
                           |       chunk_index:        12
                           |       created_at:         1706367600
                           |       embedded_at:        1706367612
                           |   }
                           +-- created_at:     1706367600  (when chunk was created)
                           +-- embedded_at:    1706367612  (when embedding was computed)

There is NO embedding without a chunk. There is NO chunk without provenance.
There is NO provenance without a source document path and filename.

This chain MUST be maintained through all operations:
  - Ingestion: creates chunk + provenance + embeddings together (atomic)
  - Reindex: deletes old, creates new (preserves source_file_path)
  - Delete: removes all three (chunk, provenance, embeddings) together
  - Sync: detects changed files by source_file_path + SHA256 hash
```

### 5.3 Embeddings as Raw Bytes

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

### 5.4 Sparse Vectors (SPLADE)

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

## 6. Legal Citation Model

Legal citations are first-class entities in CaseTrack, extracted during ingestion and stored in dedicated column families for cross-reference and citation network analysis.

### 6.1 Citation Data Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalCitation {
    pub citation_text: String,           // "Smith v. Jones, 123 F.3d 456 (9th Cir. 2020)"
    pub citation_type: CitationType,
    pub normalized_form: String,         // Canonical citation form
    pub parties: Option<(String, String)>, // ("Smith", "Jones")
    pub reporter: Option<String>,        // "F.3d"
    pub volume: Option<String>,
    pub page: Option<String>,
    pub court: Option<String>,           // "9th Cir."
    pub year: Option<u32>,
    pub pinpoint: Option<String>,        // "at 461" for pinpoint cites
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CitationType {
    CaseLaw,       // Smith v. Jones, 123 F.3d 456
    Statute,       // 42 U.S.C. § 1983
    Regulation,    // 17 C.F.R. § 240.10b-5
    Constitution,  // U.S. Const. amend. XIV
    ShortForm,     // Id. at 461; supra note 12
    Rule,          // Fed. R. Civ. P. 12(b)(6)
    Treaty,
    Other,
}
```

### 6.2 Citation Mention & Cross-Reference

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationMention {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub char_start: u64,
    pub char_end: u64,
    pub context_snippet: String,       // ~100 chars around citation
    pub treatment: Option<CitationTreatment>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CitationTreatment {
    Citing,         // Neutral citation
    Following,      // Approvingly citing
    Distinguishing, // Differentiating from
    Overruling,     // Explicitly overruling
    Questioning,    // Casting doubt on
    Discussing,     // Extended discussion
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationRef {
    pub citation_key: String,          // Normalized citation form
    pub citation_type: CitationType,
    pub display_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationRelationship {
    pub source_citation: String,
    pub target_citation: String,
    pub relationship: CitationTreatment,
    pub source_chunks: Vec<Uuid>,
}
```

---

## 7. Data Versioning & Migration

### 7.1 Schema Version Tracking

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

### 7.2 Migration Rules

1. Migrations are **idempotent** (safe to re-run)
2. Always **back up** before migrating
3. Migration failures are **fatal** (don't start with corrupt data)
4. Each migration is a separate function with clear documentation
5. Never delete user data during migration -- only restructure

---

## 8. Isolation Guarantees

### 8.1 Per-Customer Isolation

Every CaseTrack installation is **fully isolated per customer**:

- CaseTrack installs on each customer's machine independently
- Each customer has their own `~/Documents/CaseTrack/` directory
- No data is shared between customers -- there is no server, no cloud, no shared state
- For Team tier (5 seats), each seat is a separate installation on a separate machine with its own database
- Attorney A's embeddings, vectors, chunks, and provenance records **never touch** Attorney B's data
- There is no central database. Each attorney IS their own database.
- **Attorney-client privilege is preserved by design** -- no data leaves the machine

```
CUSTOMER ISOLATION
=================================================================================

Attorney A (Maria's MacBook)             Attorney B (David's Windows PC)
~/Documents/CaseTrack/                   C:\Users\David\Documents\CaseTrack\
|-- models/                              |-- models/
|-- registry.db                          |-- registry.db
+-- cases/                               +-- cases/
    |-- {smith-v-jones}/                     |-- {johnson-contract}/
    +-- {doe-v-acme}/                        |-- {martinez-estate}/
                                              +-- {regulatory-review}/

ZERO shared state. ZERO shared databases. ZERO network communication.
Each installation is a completely independent system.
Attorney-client privilege preserved by architecture.
```

### 8.2 Per-Case Isolation

Each case is a **completely independent RocksDB instance**:

- Separate database, embeddings, and index files per case
- No cross-case queries, shared vectors, or embedding bleed
- Independent lifecycle: deleting Case A has zero impact on Case B
- Portable: copy a case directory to another machine
- Cleanly deletable: `rm -rf cases/{uuid}/`
- **Critical for privilege**: cases for different clients are physically isolated

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

## 9. Context Graph Data Models

Entities, references, and relationships extracted during ingestion and stored as graph edges for structured case navigation.

### 9.1 Entity Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,                  // Canonical name
    pub entity_type: EntityType,
    pub aliases: Vec<String>,
    pub mention_count: u32,
    pub first_seen_doc: Uuid,
    pub first_seen_chunk: Uuid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EntityType {
    // Legal-specific entity types
    Party,
    Court,
    Judge,
    Attorney,
    Statute,
    CaseNumber,
    Jurisdiction,
    LegalConcept,
    Remedy,
    Witness,
    Exhibit,
    DocketEntry,

    // General entity types
    Person,
    Organization,
    Date,
    Amount,
    Location,
    Concept,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMention {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub char_start: u64,
    pub char_end: u64,
    pub context_snippet: String,       // ~100 chars around mention
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_key: String,            // "party:smith" or "judge:williams"
    pub entity_type: EntityType,
    pub name: String,
}
```

### 9.2 Reference Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceRecord {
    pub reference_key: String,         // Normalized reference identifier
    pub reference_type: ReferenceType,
    pub display_name: String,          // Human-readable reference label
    pub mention_count: u32,
    pub source_documents: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReferenceType {
    // Legal reference types
    CaseLaw,
    Statute,
    Regulation,
    ShortForm,

    // General reference types
    InternalCrossRef,
    ExternalDocument,
    Hyperlink,
    Standard,
    Specification,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceMention {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub context_snippet: String,
    pub relationship: Option<ReferenceRelationship>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReferenceRelationship {
    Cites, Supports, Contradicts, Extends, Supersedes, Discusses,
}
```

### 9.3 Document Graph Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocRelationship {
    pub doc_a: Uuid,
    pub doc_b: Uuid,
    pub relationship_type: DocRelType,
    pub similarity_score: Option<f32>,  // E1 cosine similarity
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DocRelType {
    SharedReferences, SharedEntities, SemanticSimilar,
    ResponseTo, Amends, Attachment,
}
```

### 9.4 Case Summary Model

```rust
/// High-level case overview, rebuilt after each ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSummary {
    // Case identification
    pub key_parties: Vec<PartyInfo>,
    pub legal_issues: Vec<String>,
    pub key_citations: Vec<LegalCitation>,
    pub jurisdiction: Option<String>,
    pub case_number: Option<String>,
    pub judge: Option<String>,
    pub case_type: Option<CaseType>,

    // General summary fields
    pub key_actors: Vec<KeyActor>,
    pub key_dates: Vec<KeyDate>,
    pub key_topics: Vec<Topic>,
    pub document_categories: HashMap<String, Vec<Uuid>>,
    pub top_references: Vec<ReferenceStat>,
    pub top_entities: Vec<EntityStat>,
    pub statistics: CaseStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyInfo {
    pub name: String,
    pub role: PartyRole,           // Plaintiff, Defendant, etc.
    pub aliases: Vec<String>,
    pub attorney: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PartyRole {
    Plaintiff, Defendant, Petitioner, Respondent,
    Appellant, Appellee, Intervenor, ThirdParty,
    CrossClaimant, CrossDefendant, Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaseType {
    CivilLitigation,
    CriminalDefense,
    ContractDispute,
    PersonalInjury,
    FamilyLaw,
    Immigration,
    IntellectualProperty,
    RealEstate,
    Employment,
    Bankruptcy,
    RegulatoryCompliance,
    CorporateTransaction,
    EstatePlanning,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyActor {
    pub name: String,
    pub role: ActorRole,
    pub mention_count: u32,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ActorRole {
    // Legal-specific roles
    Attorney, Judge, Party, Witness, Expert,

    // General roles
    Author, Reviewer, Approver, Contributor,
    Owner, Stakeholder, Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDate {
    pub date: String,
    pub description: String,
    pub source_chunk: Uuid,
    pub source_document: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    pub name: String,
    pub mention_count: u32,
    pub relevant_documents: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalIssue {
    pub description: String,
    pub relevant_statutes: Vec<String>,
    pub relevant_citations: Vec<String>,
    pub relevant_documents: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceStat {
    pub reference_key: String,
    pub display_name: String,
    pub reference_count: u32,
    pub source_documents: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityStat {
    pub entity_key: String,
    pub name: String,
    pub entity_type: EntityType,
    pub mention_count: u32,
    pub document_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseStatistics {
    pub total_documents: u32,
    pub total_pages: u32,
    pub total_chunks: u32,
    pub total_entities: u32,
    pub total_references: u32,
    pub total_citations: u32,
    pub storage_bytes: u64,
    pub document_type_breakdown: HashMap<String, u32>,
    pub embedder_coverage: HashMap<String, u32>,
}
```

---

## 10. Folder Watch & Auto-Sync Storage

Watch configurations persist in `~/Documents/CaseTrack/watches.json` (JSON for human readability) to survive server restarts.

### 10.1 Watch Registry

```rust
#[derive(Serialize, Deserialize)]
pub struct WatchRegistry {
    pub watches: Vec<FolderWatch>,
}

#[derive(Serialize, Deserialize)]
pub struct FolderWatch {
    pub id: Uuid,
    pub case_id: Uuid,
    pub folder_path: String,           // Absolute path to watched folder
    pub recursive: bool,               // Watch subfolders (default: true)
    pub enabled: bool,                 // Can be paused
    pub created_at: i64,               // Unix timestamp
    pub last_sync_at: Option<i64>,     // Last successful sync timestamp
    pub schedule: SyncSchedule,        // When to auto-sync
    pub file_extensions: Option<Vec<String>>,  // Filter (None = all supported)
    pub auto_remove_deleted: bool,     // Remove docs whose source files are gone
}

#[derive(Serialize, Deserialize)]
pub enum SyncSchedule {
    OnChange,                   // OS file-change notifications
    Interval { hours: u32 },    // Fixed interval
    Daily { time: String },     // e.g., "02:00"
    Manual,                     // Only via sync_folder tool
}
```

### 10.2 Per-Document Sync Metadata

Sync uses `file_hash` and `original_path` from `DocumentMetadata` (see Section 5.1) to detect changes:

```
FOR each file in watched folder:
  1. Compute SHA256 of file
  2. Look up file by original_path in case DB
  3. IF not found -> new file -> ingest
  4. IF found AND hash matches -> unchanged -> skip
  5. IF found AND hash differs -> modified -> reindex (delete old, re-ingest)

FOR each document in case DB with original_path under watched folder:
  6. IF source file no longer exists on disk -> deleted
     IF auto_remove_deleted -> delete document from case
     ELSE -> log warning, skip
```

---

## 11. Backup & Export

### 11.1 Case Export

Cases can be exported as portable archives:

```
casetrack export --case "Smith v. Jones" --output ~/Desktop/smith-v-jones.ctcase
```

The `.ctcase` file is a ZIP containing:
- `case.db/` -- RocksDB snapshot
- `originals/` -- Original documents (if stored)
- `manifest.json` -- Case metadata, schema version, embedder versions

### 11.2 Case Import

```
casetrack import ~/Desktop/smith-v-jones.ctcase
```

1. Validates schema version compatibility
2. Creates new case UUID (avoids collisions)
3. Copies database and originals to `cases/` directory
4. Registers in case registry

---

## 12. What's Stored Where (Summary)

| Data Type | Storage Location | Format | Size Per Unit |
|-----------|------------------|--------|---------------|
| Case metadata | `registry.db` | bincode via RocksDB | ~500 bytes/case |
| Document metadata | `cases/{id}/case.db` documents CF | bincode | ~250 bytes/doc |
| Text chunks (2000 chars) | `cases/{id}/case.db` chunks CF | bincode | ~2.5KB/chunk (text + provenance metadata) |
| E1 embeddings (768D) | `cases/{id}/case.db` embeddings CF | f32 bytes | 3,072 bytes/chunk |
| E6 sparse vectors | `cases/{id}/case.db` embeddings CF | bincode sparse | ~500 bytes/chunk |
| E12 token embeddings | `cases/{id}/case.db` embeddings CF | bincode | ~8KB/chunk |
| BM25 inverted index | `cases/{id}/case.db` bm25_index CF | bincode | ~2MB/1000 chunks |
| Legal citations | `cases/{id}/case.db` citations CF | bincode | ~200 bytes/citation |
| Citation index | `cases/{id}/case.db` citation_index CF | bincode | ~500 bytes/citation |
| Provenance records | Embedded in chunk embedding records | bincode | ~300 bytes/chunk |
| Original documents | `cases/{id}/originals/` | original files | varies |
| ONNX models | `models/` | ONNX format | 110-220MB each |

---

## 13. Storage Lifecycle Management

CaseTrack runs 100% locally with no cloud cleanup service. Storage management is the user's responsibility, but CaseTrack must make it easy and proactive. Without lifecycle management, attorneys accumulating 50+ cases over years will silently consume 5-10GB+ of disk.

### 13.1 Storage Budget & Monitoring

CaseTrack tracks disk usage at three levels: per-case, total data, and models.

```rust
/// Storage usage summary returned by get_storage_summary MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSummary {
    /// Total disk usage across all cases + models
    pub total_bytes: u64,
    /// Disk usage by models only
    pub models_bytes: u64,
    /// Disk usage by all case data (sum of all cases)
    pub cases_bytes: u64,
    /// Per-case breakdown, sorted by size descending
    pub cases: Vec<CaseStorageInfo>,
    /// Storage budget (configurable, default 10GB)
    pub budget_bytes: u64,
    /// Percentage of budget used
    pub budget_used_pct: f32,
    /// Warning level: None, Approaching (>70%), Exceeded (>90%)
    pub warning: StorageWarning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseStorageInfo {
    pub case_id: Uuid,
    pub case_name: String,
    pub status: CaseStatus,
    pub storage_bytes: u64,
    pub document_count: u32,
    pub chunk_count: u32,
    /// Days since last search or ingestion
    pub days_inactive: u32,
    /// Whether this case has stale embeddings (source files changed)
    pub has_stale_embeddings: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StorageWarning {
    None,
    /// >70% of budget used
    Approaching,
    /// >90% of budget used
    Exceeded,
}
```

### 13.2 Startup Storage Check

On every server start, CaseTrack computes total disk usage and logs a warning if it exceeds the configured budget. This is non-blocking and costs ~50ms for typical installations.

```rust
/// Called during server startup, after license check
pub fn check_storage_budget(data_dir: &Path, budget_bytes: u64) {
    let total = compute_total_usage(data_dir);
    let pct = (total as f64 / budget_bytes as f64 * 100.0) as u32;

    if pct >= 90 {
        tracing::warn!(
            "CaseTrack using {:.1}GB of disk ({pct}% of {:.1}GB budget). \
             Consider archiving closed cases with archive_case or deleting \
             old cases with delete_case.",
            total as f64 / 1e9,
            budget_bytes as f64 / 1e9,
        );
    } else if pct >= 70 {
        tracing::info!(
            "CaseTrack using {:.1}GB of disk ({pct}% of {:.1}GB budget).",
            total as f64 / 1e9,
            budget_bytes as f64 / 1e9,
        );
    }
}

fn compute_total_usage(data_dir: &Path) -> u64 {
    // Walk data_dir recursively, sum file sizes
    // Skip symlinks, handle permission errors gracefully
    walkdir::WalkDir::new(data_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
```

The storage budget defaults to 10GB and is configurable in `config.toml`:

```toml
# ~/Documents/CaseTrack/config.toml
storage_budget_gb = 10   # Warn when total usage exceeds this
```

### 13.3 Stale Case Detection

Cases that haven't been searched or modified in 6+ months are surfaced in `list_cases` and `get_storage_summary` with a staleness indicator. The `updated_at` timestamp on the `Case` struct already supports this.

```rust
impl CaseStorageInfo {
    pub fn is_stale(&self) -> bool {
        self.days_inactive >= 180  // 6 months
    }
}
```

`list_cases` sorts stale cases to the bottom and includes the `days_inactive` field. `get_storage_summary` groups cases by status (active, stale, archived) so the AI can recommend cleanup actions.

### 13.4 RocksDB Compaction

RocksDB accumulates SST files and tombstones after `delete_document` and `reindex_document` operations. Without periodic compaction, disk usage can grow 2-3x beyond actual data size.

#### 13.4.1 Auto-Compact on Archive

When a case transitions to `Archived` status, CaseTrack runs a full compaction on all column families. This is a one-time CPU cost (seconds to minutes depending on case size) that can halve storage for inactive cases.

```rust
impl CaseHandle {
    /// Compact all column families. Called automatically on archive_case.
    /// Reclaims space from tombstones and applies Zstd compression to all levels.
    pub fn compact_all(&self) -> Result<()> {
        for cf_name in COLUMN_FAMILIES {
            let cf = self.db.cf_handle(cf_name)
                .ok_or_else(|| CaseTrackError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Column family not found: {}", cf_name),
                )))?;
            self.db.compact_range_cf(&cf, None::<&[u8]>, None::<&[u8]>);
        }
        tracing::info!("Compacted all column families for case {}", self.case_id);
        Ok(())
    }
}
```

#### 13.4.2 Post-Delete Compaction

After `delete_document` removes a document's chunks, embeddings, entities, citations, and BM25 entries, the affected column families accumulate tombstones. CaseTrack triggers a targeted background compaction on the affected CFs after deletion completes.

```rust
/// Called after delete_document completes its cascading delete
fn compact_after_delete(case: &CaseHandle) {
    // Compact the heaviest CFs: chunks, embeddings, bm25_index
    // Run in background to avoid blocking the MCP response
    let db = case.db.clone();
    tokio::spawn(async move {
        for cf_name in &["chunks", "embeddings", "bm25_index", "entities", "entity_index"] {
            if let Some(cf) = db.cf_handle(cf_name) {
                db.compact_range_cf(&cf, None::<&[u8]>, None::<&[u8]>);
            }
        }
    });
}
```

#### 13.4.3 Manual Compaction via MCP Tool

The `compact_case` MCP tool (see [PRD 09](PRD_09_MCP_TOOLS.md)) allows explicit compaction when the user or AI notices bloated storage.

### 13.5 Embedding Cleanup on Tier Downgrade

When a user downgrades from Pro to Free (license expires), the ColBERT (E12) embeddings become dead weight -- they're stored but never used. CaseTrack provides a `strip_embeddings` CLI command to reclaim this space.

```rust
/// Strip unused embedder vectors from all chunks in a case.
/// Reclaims ~8KB/chunk for E12 (ColBERT) vectors on downgrade from Pro to Free.
pub fn strip_embeddings(case: &CaseHandle, embedder: &str) -> Result<StripResult> {
    let cf = case.db.cf_handle("embeddings")
        .ok_or_else(|| CaseTrackError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound, "embeddings CF not found",
        )))?;

    let mut stripped = 0u64;
    let mut bytes_reclaimed = 0u64;

    // Iterate all ChunkEmbeddingRecord entries
    for item in case.db.iterator_cf(&cf, rocksdb::IteratorMode::Start) {
        let (key, value) = item?;
        let mut record: ChunkEmbeddingRecord = bincode::deserialize(&value)?;

        let old_size = value.len() as u64;
        let changed = match embedder {
            "e12" => {
                let had = record.e12_vector.is_some();
                record.e12_vector = None;
                had
            }
            "e6" => {
                let had = record.e6_vector.is_some();
                record.e6_vector = None;
                had
            }
            _ => false,
        };

        if changed {
            let new_value = bincode::serialize(&record)?;
            bytes_reclaimed += old_size - new_value.len() as u64;
            case.db.put_cf(&cf, &key, &new_value)?;
            stripped += 1;
        }
    }

    // Compact to physically reclaim disk space
    case.compact_all()?;

    Ok(StripResult { chunks_modified: stripped, bytes_reclaimed })
}
```

This is a CLI command, not an MCP tool, because it's a destructive maintenance operation:

```bash
casetrack strip-embeddings --case "Smith v. Jones" --embedder e12
```

### 13.6 Cascade Original File Deletion

When `delete_document` removes a document's chunks and embeddings, it must also clean up the original file copy in `originals/` (if it exists). Without this, the `originals/` folder accumulates orphaned files.

```rust
impl CaseHandle {
    /// Extended delete_document: cascading delete of chunks, embeddings,
    /// entities, citations, BM25 entries, AND the original file copy.
    pub fn delete_document_cascade(&self, doc_id: Uuid) -> Result<()> {
        let doc = self.get_document(doc_id)?;

        // 1. Delete chunks, embeddings, entities, citations, BM25 (existing logic)
        self.delete_document_data(doc_id)?;

        // 2. Delete original file copy if it exists
        let originals_dir = self.case_dir.join("originals");
        if originals_dir.exists() {
            let original_file = originals_dir.join(&doc.name);
            if original_file.exists() {
                std::fs::remove_file(&original_file)?;
                tracing::debug!("Removed original file copy: {}", original_file.display());
            }
        }

        // 3. Background compaction to reclaim space
        compact_after_delete(self);

        Ok(())
    }
}
```

### 13.7 Case Export for Archival (`purge_archived`)

For attorneys who want to reclaim maximum disk space from archived cases, CaseTrack provides a CLI command to export archived cases to compressed `.ctcase` ZIP files and then delete the expanded RocksDB. The case can be re-imported later if needed.

```bash
# Export all archived cases to ZIP, then delete the expanded databases
casetrack purge-archived --output ~/Desktop/CaseTrack-Archives/

# Export a specific case
casetrack purge-archived --case "Smith v. Jones" --output ~/Desktop/
```

The `.ctcase` format is the same ZIP described in Section 11.1 (Case Export). Purging:

1. Validates the case is in `Archived` status (refuses to purge active cases)
2. Runs `compact_all()` before export (ensures minimal ZIP size)
3. Exports to `.ctcase` ZIP
4. Verifies the export file is valid (re-reads manifest, checks file count)
5. Deletes the expanded case directory
6. Updates the registry to mark the case as `Purged` with the export path

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaseStatus {
    Active,
    Closed,
    Archived,
    OnHold,
    /// Case has been exported to .ctcase and the expanded DB deleted.
    /// The export_path field records where the archive was saved.
    Purged,
}
```

A `Purged` case appears in `list_cases` with its export path but cannot be searched or opened until re-imported.

### 13.8 Storage Lifecycle Summary

| Trigger | Action | Automatic? | Disk Impact |
|---------|--------|-----------|-------------|
| Server startup | Log storage usage, warn if >70% budget | Yes | None |
| `delete_document` | Cascade delete chunks + embeddings + originals + background compact | Yes | Reclaims space |
| `archive_case` | Full RocksDB compaction on all CFs | Yes | ~30-50% reduction |
| `delete_case` | Remove entire case directory | Manual (MCP tool) | Full reclaim |
| `list_cases` / `get_storage_summary` | Surface stale cases (>6 months inactive) | Yes (display) | None |
| License downgrade (Pro -> Free) | `strip-embeddings --embedder e12` CLI command | Manual | ~60% embedding reduction |
| Long-term archival | `purge-archived` exports to .ctcase ZIP, deletes DB | Manual (CLI) | ~70-90% reduction |
| `compact_case` | Manual RocksDB compaction via MCP tool | Manual (MCP tool) | ~20-40% reduction |

### 13.9 Storage Lifecycle Anti-Patterns

| Anti-Pattern | Why Not |
|-------------|---------|
| Auto-delete stale cases | Data loss risk; only the attorney decides what to delete |
| Auto-purge archived cases | Attorney may need quick re-access; export requires explicit action |
| Background compaction on every write | Write amplification; only compact on delete/archive/manual |
| Auto-strip embeddings on downgrade | Destructive; the user may re-upgrade and want E12 vectors back |
| Storage quotas that block operations | Never block an attorney's work; warn instead |
| Silent disk usage growth | Always surface usage in startup logs and get_storage_summary |

---

*CaseTrack PRD v5.1.0 -- Document 4 of 10*

# PRD 07: Case Management & Provenance

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Case Model

```rust
/// A legal case containing related documents, entities, and citations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: Uuid,
    pub name: String,                    // "Smith v. Jones"
    pub case_number: Option<String>,     // "2:24-cv-01234"
    pub description: Option<String>,
    pub case_type: CaseType,
    pub status: CaseStatus,
    pub jurisdiction: Option<String>,    // "S.D.N.Y."
    pub judge: Option<String>,
    pub parties: Vec<PartyInfo>,
    pub tags: Vec<String>,
    pub created_by: Option<String>,      // Attorney name
    pub created_at: i64,                 // Unix timestamp
    pub updated_at: i64,                 // Unix timestamp
    pub stats: CaseStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseStats {
    pub document_count: u32,
    pub page_count: u32,
    pub chunk_count: u32,
    pub entity_count: u32,
    pub citation_count: u32,
    pub storage_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaseType {
    Litigation,
    Corporate,
    RealEstate,
    Bankruptcy,
    Immigration,
    Employment,
    IntellectualProperty,
    Criminal,
    FamilyLaw,
    TaxLaw,
    Other,
}

// Derive FromStr via case-insensitive match on variant names. Default: Other.

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaseStatus {
    Active,
    Closed,
    Archived,
    OnHold,
    /// Case exported to .ctcase ZIP and expanded DB deleted.
    /// See PRD 04 Section 13.7 for purge_archived behavior.
    Purged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyInfo {
    pub name: String,
    pub role: PartyRole,
    pub counsel: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PartyRole {
    Plaintiff,
    Defendant,
    Petitioner,
    Respondent,
    Appellant,
    Appellee,
    ThirdParty,
    Intervenor,
    Other,
}
```

---

## 2. Case Registry

Shared RocksDB instance indexing all cases. Key schema: `case:{uuid}` -> bincode-serialized `Case`.

```rust
pub struct CaseRegistry {
    db: rocksdb::DB,        // registry.db in data_dir
    data_dir: PathBuf,
    active_case: Option<Uuid>,
}

pub struct CreateCaseParams {
    pub name: String,
    pub case_number: Option<String>,
    pub description: Option<String>,
    pub case_type: Option<CaseType>,
    pub jurisdiction: Option<String>,
    pub judge: Option<String>,
    pub parties: Option<Vec<PartyInfo>>,
    pub tags: Option<Vec<String>>,
    pub created_by: Option<String>,
}

impl CaseRegistry {
    /// Opens registry.db from data_dir
    pub fn open(data_dir: &Path) -> Result<Self>;

    /// Creates case dir + originals subdir, initializes CaseHandle DB,
    /// stores in registry, auto-switches active_case to new case
    pub fn create_case(&mut self, params: CreateCaseParams) -> Result<Case>;

    /// Lookup by "case:{id}" key. Error: CaseNotFound
    pub fn get_case(&self, case_id: Uuid) -> Result<Case>;

    /// Prefix scan "case:", returns all cases sorted by updated_at DESC
    pub fn list_cases(&self) -> Result<Vec<Case>>;

    /// Upsert case metadata
    pub fn update_case(&mut self, case: &Case) -> Result<()>;

    /// Deletes registry entry + entire case directory. Clears active_case if matched.
    pub fn delete_case(&mut self, case_id: Uuid) -> Result<()>;

    /// Validates case exists, opens CaseHandle, sets active_case
    pub fn switch_case(&mut self, case_id: Uuid) -> Result<CaseHandle>;

    pub fn active_case_id(&self) -> Option<Uuid>;
    pub fn count_cases(&self) -> Result<u32>;
}
```

---

## 3. Case Handle

Each case has its own `case.db` RocksDB with column families defined in `super::COLUMN_FAMILIES`.

Key schemas:
- Documents CF: `doc:{uuid}` -> bincode `DocumentMetadata`
- Chunks CF: `chunk:{uuid}` -> bincode `Chunk`
- Chunks CF index: `doc_chunks:{doc_uuid}:{sequence:06}` -> chunk UUID string
- Entities CF: `entity:{uuid}` -> bincode `Entity`
- Citations CF: `citation:{uuid}` -> bincode `LegalCitation`

```rust
/// Handle to an open case database
pub struct CaseHandle {
    pub db: rocksdb::DB,
    pub case_id: Uuid,            // Parsed from case_dir directory name
    pub case_dir: PathBuf,
}

impl CaseHandle {
    /// Create case.db with all column families (DB dropped after init, reopened by open())
    pub fn initialize(case_dir: &Path) -> Result<()>;

    /// Open existing case.db. Error: CaseDbOpenFailed
    pub fn open(case_dir: &Path) -> Result<Self>;

    // --- Document Operations (all use "documents" CF) ---
    pub fn store_document(&self, doc: &DocumentMetadata) -> Result<()>;
    pub fn get_document(&self, doc_id: Uuid) -> Result<DocumentMetadata>;
    /// Prefix scan "doc:", sorted by ingested_at DESC
    pub fn list_documents(&self) -> Result<Vec<DocumentMetadata>>;
    /// Deletes doc metadata + all chunks via doc_chunks index + embeddings + provenance
    pub fn delete_document(&self, doc_id: Uuid) -> Result<()>;

    // --- Chunk Operations (all use "chunks" CF) ---
    /// Stores chunk + doc_chunks index entry (keyed by doc_id + zero-padded sequence)
    pub fn store_chunk(&self, chunk: &Chunk) -> Result<()>;
    pub fn get_chunk(&self, chunk_id: Uuid) -> Result<Chunk>;

    // --- Entity Operations (all use "entities" CF) ---
    pub fn store_entity(&self, entity: &Entity) -> Result<()>;
    pub fn get_entity(&self, entity_id: Uuid) -> Result<Entity>;
    pub fn list_entities(&self) -> Result<Vec<Entity>>;

    // --- Citation Operations (all use "citations" CF) ---
    pub fn store_citation(&self, citation: &LegalCitation) -> Result<()>;
    pub fn get_citation(&self, citation_id: Uuid) -> Result<LegalCitation>;
    pub fn list_citations(&self) -> Result<Vec<LegalCitation>>;
}
```

---

## 4. Provenance System (THE MOST IMPORTANT SYSTEM IN CASETRACK)

### 4.1 Provenance Model

```
PROVENANCE IS NON-NEGOTIABLE
=================================================================================

Every piece of information CaseTrack stores or returns MUST trace back to:
  1. The SOURCE FILE (file path + filename on disk)
  2. The exact LOCATION (page, paragraph, line, character offsets)
  3. The EXTRACTION METHOD (Native text, OCR, Hybrid, Email)
  4. TIMESTAMPS (when created, when last embedded)

This applies to:
  - Every text chunk
  - Every embedding vector (linked via chunk_id)
  - Every entity mention (stores chunk_id + char offsets)
  - Every legal citation (stores chunk_id + char offsets)
  - Every reference record (stores chunk_id + document_id)
  - Every search result (includes full provenance)
  - Every MCP tool response that returns text

If the provenance chain is broken, the data is USELESS.
A search result without a source reference is worthless to an attorney.
An uncited legal assertion is malpractice waiting to happen.
```

Every chunk tracks exactly where it came from:

```rust
/// EVERY chunk stores full provenance. This is THE MOST IMPORTANT DATA STRUCTURE
/// in CaseTrack. When the AI returns information, the attorney must know EXACTLY
/// where it came from -- which document, which file on disk, which page, which
/// paragraph, which line, which character range. Without provenance, the data is
/// useless. In legal work, an uncited assertion is worse than no assertion at all.
///
/// The Provenance chain: Embedding vector -> chunk_id -> ChunkData.provenance -> source file
/// This chain is NEVER broken. Every embedding, every entity mention, every citation,
/// every reference, every search result carries its Provenance. If you can't cite
/// the source, you can't return the information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    // === Source Document (WHERE did this come from?) ===
    /// UUID of the ingested document
    pub document_id: Uuid,
    /// Original filename ("Complaint.pdf") -- always stored, never empty
    pub document_name: String,
    /// Full filesystem path where the file was when ingested
    /// ("/Users/attorney/Cases/SmithVJones/Complaint.pdf")
    /// Used for: reindexing (re-reads the file), sync (detects changes), display
    pub document_path: Option<PathBuf>,

    // === Location in Document (EXACTLY where in the document?) ===
    /// Page number (1-indexed) -- which page of the PDF/DOCX/XLSX
    pub page: u32,
    /// First paragraph index included in this chunk (0-indexed within page)
    pub paragraph_start: u32,
    /// Last paragraph index included in this chunk
    pub paragraph_end: u32,
    /// First line number (1-indexed within page)
    pub line_start: u32,
    /// Last line number
    pub line_end: u32,

    // === Character Offsets (for exact highlighting and cursor positioning) ===
    /// Character offset from start of page -- pinpoints exactly where the text starts
    pub char_start: u64,
    /// Character offset end -- pinpoints exactly where the text ends
    pub char_end: u64,

    // === Legal Section (for legal-aware chunking) ===
    /// Clause number, Q&A index, argument heading, statute section, etc.
    pub legal_section: Option<String>,

    // === Extraction Metadata (HOW was the text obtained?) ===
    /// How the text was extracted from the original file
    pub extraction_method: ExtractionMethod,
    /// OCR confidence score (0.0-1.0) if extracted via OCR. Lets the AI warn when
    /// text may be unreliable ("This text was OCR'd with 72% confidence").
    pub ocr_confidence: Option<f32>,

    // === Chunk Position ===
    /// Sequential position of this chunk within the entire document (0-indexed)
    pub chunk_index: u32,

    // === Timestamps (WHEN was this data created/updated?) ===
    /// When this chunk was first created from the source document (Unix timestamp)
    pub created_at: i64,
    /// When the embedding vectors for this chunk were last computed (Unix timestamp)
    /// Updated on reindex. Lets the system detect stale embeddings.
    pub embedded_at: i64,
}

impl Provenance {
    /// Generate a source reference string (general format)
    pub fn cite(&self) -> String {
        let mut parts = vec![self.document_name.clone()];
        parts.push(format!("p. {}", self.page));

        if self.paragraph_start == self.paragraph_end {
            parts.push(format!("para. {}", self.paragraph_start));
        } else {
            parts.push(format!("paras. {}-{}", self.paragraph_start, self.paragraph_end));
        }

        if self.line_start > 0 {
            parts.push(format!("ll. {}-{}", self.line_start, self.line_end));
        }

        parts.join(", ")
    }

    /// Generate a legal citation format reference
    /// Returns: "Smith_v_Jones_Complaint.pdf, p. 12, para. 47"
    pub fn cite_legal(&self) -> String {
        let mut parts = vec![self.document_name.clone()];
        parts.push(format!("p. {}", self.page));

        // Use paragraph symbol for legal citations
        if self.paragraph_start == self.paragraph_end {
            parts.push(format!("\u{00B6} {}", self.paragraph_start));
        } else {
            parts.push(format!("\u{00B6}\u{00B6} {}-{}", self.paragraph_start, self.paragraph_end));
        }

        if let Some(ref section) = self.legal_section {
            parts.push(format!("sec. {}", section));
        }

        parts.join(", ")
    }

    /// Short reference for inline use
    pub fn cite_short(&self) -> String {
        format!("{}, p. {}",
            self.document_name.split('.').next().unwrap_or(&self.document_name),
            self.page
        )
    }
}
```

### 4.2 Search Results with Provenance

```rust
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub text: String,
    pub score: f32,
    pub provenance: Provenance,
    pub citation: String,         // Full citation (cite())
    pub citation_legal: String,   // Legal citation (cite_legal())
    pub citation_short: String,   // Short citation (cite_short())
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}

impl SearchResult {
    pub fn to_mcp_content(&self) -> serde_json::Value {
        json!({
            "text": self.text,
            "score": self.score,
            "citation": self.citation,
            "citation_legal": self.citation_legal,
            "citation_short": self.citation_short,
            "source": {
                "document": self.provenance.document_name,
                "page": self.provenance.page,
                "paragraph_start": self.provenance.paragraph_start,
                "paragraph_end": self.provenance.paragraph_end,
                "lines": format!("{}-{}", self.provenance.line_start, self.provenance.line_end),
                "legal_section": self.provenance.legal_section,
                "extraction_method": format!("{:?}", self.provenance.extraction_method),
                "ocr_confidence": self.provenance.ocr_confidence,
            },
            "context": {
                "before": self.context_before,
                "after": self.context_after,
            }
        })
    }
}
```

### 4.3 Context Window

Search results include surrounding chunks for comprehension. Uses the `doc_chunks` index to look up adjacent chunks by `sequence +/- window`.

```rust
impl CaseHandle {
    /// Returns (before_text, after_text) by looking up adjacent chunks
    /// via doc_chunks:{doc_id}:{sequence +/- 1} index keys
    pub fn get_surrounding_context(
        &self,
        chunk: &Chunk,
        window: usize,
    ) -> Result<(Option<String>, Option<String>)>;
}
```

---

## 5. Case Summary

Each case maintains a summary structure that provides an at-a-glance overview of the case's contents, automatically updated as documents are ingested or removed.

```rust
/// Per-case summary providing an overview of all contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSummary {
    pub case_id: Uuid,

    // === Parties ===
    /// All parties involved in the case with roles and counsel
    pub parties: Vec<PartyInfo>,

    // === Legal Issues ===
    /// Key legal issues identified across the case documents
    pub legal_issues: Vec<String>,

    // === Key Citations ===
    /// Most frequently cited authorities across the case
    pub key_citations: Vec<CitationSummary>,

    // === Key Entities ===
    /// People, organizations, courts, and other named entities mentioned across all documents
    pub entities: Vec<EntitySummary>,

    // === Key Dates & Timelines ===
    /// Important dates extracted from documents with context
    pub key_dates: Vec<DateEntry>,

    // === Top Topics/Themes ===
    /// Dominant legal themes identified across the case
    pub top_topics: Vec<TopicSummary>,

    // === Document Statistics ===
    pub document_count: u32,
    pub total_pages: u32,
    pub total_chunks: u32,
    pub total_entities: u32,
    pub total_citations: u32,
    pub storage_bytes: u64,
    pub file_types: HashMap<String, u32>,  // e.g., {"pdf": 12, "docx": 5, "eml": 8}

    // === Entity Statistics ===
    pub unique_entity_count: u32,
    pub entity_type_counts: HashMap<String, u32>,  // e.g., {"party": 4, "judge": 1, "statute": 15}

    pub last_updated: i64,  // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationSummary {
    pub citation_text: String,        // "Smith v. Jones, 123 F.3d 456 (9th Cir. 2024)"
    pub citation_type: String,        // "case", "statute", "regulation"
    pub mention_count: u32,           // How many documents cite this authority
    pub document_ids: Vec<Uuid>,      // Which documents cite it
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySummary {
    pub name: String,
    pub entity_type: String,        // "party", "court", "judge", "attorney", etc.
    pub mention_count: u32,
    pub document_ids: Vec<Uuid>,    // Which documents mention this entity
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateEntry {
    pub date: String,               // ISO 8601
    pub context: String,            // What the date refers to ("Filing date", "Incident date")
    pub document_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicSummary {
    pub label: String,
    pub chunk_count: u32,           // How many chunks belong to this topic
    pub representative_terms: Vec<String>,
}
```

---

## 6. Reference Network

The reference network is a graph of cross-document references within a case. It enables navigation between related documents based on shared entities, semantic similarity, explicit references, and shared legal citations.

```rust
/// Edge in the reference network connecting two documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceEdge {
    pub source_doc_id: Uuid,
    pub target_doc_id: Uuid,
    pub edge_type: ReferenceEdgeType,
    pub weight: f32,                // Strength of the reference (0.0-1.0)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReferenceEdgeType {
    SharedEntity,           // Both documents mention the same entity (party, judge, etc.)
    SemanticSimilarity,     // Documents contain semantically similar chunks
    ExplicitReference,      // One document explicitly references another
    SharedCitation,         // Both documents cite the same legal authority
    CitationChain,          // One document cites a case that the other discusses
}
```

---

## 7. Knowledge Graph

Every case maintains a knowledge graph linking chunks, documents, entities, and legal citations with full provenance.

```
KNOWLEDGE GRAPH STRUCTURE (Legal Case)
=================================================================================

  Nodes:
    - Document nodes (one per ingested file: complaints, briefs, depositions, etc.)
    - Chunk nodes (one per text chunk, linked to parent document)
    - Entity nodes (parties, courts, judges, attorneys, etc. extracted from chunks)
    - Citation nodes (case citations, statute citations, regulations)

  Edges:
    - Chunk-to-Document: Every chunk linked to its source document with full provenance
    - Entity-to-Chunk: Entity mention links with character offsets
    - Citation-to-Chunk: Citation mention links with character offsets
    - Citation-to-Citation: Short-form citations (Id., supra) linked to their antecedent
    - Document-to-Document: Shared entities, shared citations, semantic similarity
    - Chunk-to-Chunk: Semantic similarity above threshold, co-reference

  Enables queries like:
    - "Show me all documents mentioning Smith v. Jones"
    - "What other filings cite 42 U.S.C. § 1983?"
    - "Which depositions mention the defendant?"
    - "What statutes are cited in the plaintiff's brief?"
    - "Trace the provenance of this legal argument back to the source"
    - "Which documents share the most legal authorities?"
```

```rust
/// Node in the case's knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphNode {
    Document { id: Uuid, name: String },
    Chunk { id: Uuid, document_id: Uuid, text_preview: String },
    Entity { id: Uuid, name: String, entity_type: String },
    Citation { id: Uuid, citation_text: String, citation_type: String },
}

/// Edge in the case's knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub edge_type: GraphEdgeType,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GraphEdgeType {
    ChunkToDocument,        // Chunk belongs to document (provenance link)
    EntityToChunk,          // Entity mentioned in chunk
    CitationToChunk,        // Citation found in chunk
    CitationToAntecedent,   // Short-form (Id.) resolved to full citation
    DocumentToDocument,     // Cross-document reference (shared entities/citations)
    ChunkToChunk,           // Semantic similarity or co-reference
}
```

---

## 8. Case Lifecycle

```
CASE LIFECYCLE
=================================================================================

  create_case("Smith v. Jones", case_type="litigation", case_number="2:24-cv-01234")
       |
       v
  [ACTIVE] -----> ingest_pdf, ingest_docx, ingest_eml, search_case
       |
       |  close_case()                 reopen_case()
       v                                   |
  [CLOSED] --------> (read-only) ---------+
       |
       |  put_on_hold()    resume_case()
       v                       |
  [ON HOLD] --------> -------+
       |
       |  archive_case()  (auto-compacts RocksDB, ~30-50% storage reduction)
       v
  [ARCHIVED] -----> (read-only, hidden from default list, compacted)
       |
       |--- delete_case()  -----> [DELETED]  (case directory removed from disk)
       |
       +--- purge_archived()  --> [PURGED]   (exported to .ctcase ZIP, DB deleted)
                                      |
                                      |  import (re-imports from .ctcase)
                                      v
                                  [ARCHIVED]  (restored from ZIP)

Notes:
  - ACTIVE: Full read/write. Can ingest, search, modify.
  - CLOSED: Read-only. Search works. Cannot ingest new documents.
  - ON HOLD: Read-only. Case paused (e.g., settlement negotiations).
  - ARCHIVED: Same as closed but hidden from default list_cases. DB auto-compacted.
  - PURGED: Exported to .ctcase ZIP, expanded DB deleted. Listed but not searchable.
            Re-import restores to ARCHIVED status. See PRD 04 Section 13.7.
  - DELETED: Completely removed. Not recoverable.
```

---

## 9. Case Management via MCP Tools -- Operations Guide

This section is the definitive reference for how the AI (Claude) and the user manage cases, documents, embeddings, and databases through MCP tools. **Every operation below is exposed as an MCP tool** (see PRD 09 for full input/output schemas).

### 9.1 Isolation Guarantee

```
CRITICAL: DATA NEVER CROSSES CASE BOUNDARIES
=================================================================================

- Each case = its own RocksDB database on disk (separate files, separate directory)
- Embeddings from Case A are in a DIFFERENT DATABASE FILE than Case B
- Search operates within a SINGLE CASE ONLY -- there is no cross-case search
- Ingestion targets the ACTIVE CASE ONLY -- documents go into exactly one case
- Deleting a case deletes ONLY that case's database, chunks, embeddings, and index
- No shared vector index, no shared embedding store, no shared anything

The AI MUST switch_case before performing ANY operation on a different case.
There is no way to accidentally mix data between cases.

This isolation mirrors how law firms operate: Smith v. Jones case files
are NEVER mixed with Doe v. Corporation case files. CaseTrack enforces
this at the database level.
```

### 9.2 Case Lifecycle Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Create a case | `create_case` | Creates a new case directory, initializes an empty RocksDB instance with all column families, registers in the case registry, auto-switches to the new case | New database on disk |
| List all cases | `list_cases` | Lists all cases with status, case type, document count, chunk count, creation date | Read-only |
| Switch active case | `switch_case` | Changes which case all subsequent operations target. Opens that case's RocksDB database. | Changes active DB handle |
| Get case details | `get_case_info` | Shows all documents, parties, legal issues, total pages, total chunks, storage usage, embedder info | Read-only |
| Close a case | `close_case` | Sets case status to Closed. Search still works. Cannot ingest. | Status change only |
| Archive a case | `archive_case` | Sets case status to Archived. Hidden from default list. | Status change only |
| Delete a case | `delete_case` | **Permanently removes**: case directory, RocksDB database, ALL chunks, ALL embeddings, ALL indexes, ALL provenance records, optionally stored original files. Requires `confirm=true`. Not recoverable. | **Destroys entire database** |

### 9.3 Document Management Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Ingest one file | `ingest_document` | Reads file -> detects legal document type -> legal-aware chunking -> embeds with Legal-BERT + SPLADE + ColBERT + BM25 -> extracts entities + citations -> stores in active case's DB | Adds chunks + embeddings + entities + citations to active case |
| Ingest a folder | `ingest_folder` | Recursively walks directory -> ingests all supported files (PDF, DOCX, XLSX, EML, TXT, etc.) -> skips already-ingested (SHA256) | Bulk add to active case |
| Sync a folder | `sync_folder` | Compares disk vs DB -> ingests new files, reindexes changed files, optionally removes deleted | Add/update/remove in active case |
| List documents | `list_documents` | Lists all documents in active case with page count, chunk count, type | Read-only |
| Get document details | `get_document` | Shows one document's metadata, extraction method, chunk stats | Read-only |
| **Delete a document** | `delete_document` | **Removes from active case**: document metadata, ALL chunks for that document, ALL embeddings for those chunks, ALL provenance records, ALL entity mentions, ALL citation mentions, ALL BM25 index entries, AND the original file copy in `originals/` (if it exists). Triggers background RocksDB compaction on affected column families. Requires `confirm=true`. | **Destroys document data** |

### 9.4 Embedding & Index Management Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Check index health | `get_index_status` | Per-document report: embedder coverage (Legal-BERT + SPLADE + ColBERT + BM25), SHA256 staleness, missing source files | Read-only |
| Reindex one document | `reindex_document` | Deletes ALL old chunks + embeddings -> re-reads source file -> re-detects legal type -> re-chunks -> re-embeds -> re-extracts entities/citations -> rebuilds BM25 entries. Option: `reparse=false` keeps chunks, only rebuilds embeddings. | **Replaces** old embeddings with fresh ones |
| Reindex entire case | `reindex_case` | Full rebuild of every document in the case. Option: `skip_unchanged=true` only touches stale documents. Requires `confirm=true`. | **Replaces** all embeddings in case |
| Get chunk provenance | `get_chunk` | Retrieves one chunk with full text and provenance (file, page, paragraph, line, char offsets, legal section) | Read-only |
| List document chunks | `get_document_chunks` | Lists all chunks in a document with their provenance | Read-only |
| Get surrounding context | `get_source_context` | Gets the chunks before/after a given chunk for context | Read-only |

### 9.5 Folder Watch & Auto-Sync Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Watch a folder | `watch_folder` | Starts OS-level file monitoring. New/modified/deleted files automatically trigger ingestion/reindex/removal in the target case. | Automatic ongoing changes |
| Stop watching | `unwatch_folder` | Stops auto-sync. Existing case data is untouched. | No data change |
| List watches | `list_watches` | Shows all active watches, their schedule, last sync, health status | Read-only |
| Change schedule | `set_sync_schedule` | Changes how often a watch syncs (on_change, hourly, daily, manual) | No data change |

### 9.6 Typical AI Workflow

```
User: "New case for Smith v. Jones. Docs in ~/Cases/SmithVJones/"

Claude:
  1. create_case("Smith v. Jones",
       case_type="litigation",
       case_number="2:24-cv-01234",
       jurisdiction="S.D.N.Y.",
       parties=[{name: "Smith", role: "plaintiff"},
                {name: "Jones Corp", role: "defendant"}])    -> isolated DB, auto-switched
  2. ingest_folder("~/Cases/SmithVJones/", recursive=true)   -> chunks + embeds all files
  3. watch_folder("~/Cases/SmithVJones/", schedule="on_change") -> auto-sync future changes

User: "Search for breach of fiduciary duty"
  4. search_case("breach of fiduciary duty", top_k=5)        -> results with full provenance
     Response includes:
       - "Complaint.pdf, p. 12, para. 47: Defendant breached fiduciary duty..."
       - "Jones_Deposition.pdf, p. 89, Q&A 145: Q. Did you understand your
          fiduciary obligations? A. I was told I had a duty of care..."
       - "Defendant_Brief.pdf, p. 5, sec. III.A: No fiduciary relationship existed..."

User: "What statutes are cited in the complaint?"
  5. list_citations(document="Complaint.pdf", type="statute") -> all statute citations
     Response includes:
       - "42 U.S.C. § 1983 (civil rights)" cited at p. 8, para. 23
       - "Cal. Bus. & Prof. Code § 17200 (unfair business practices)" cited at p. 12, para. 41

User: "Switch to the patent infringement case"
  6. switch_case("Doe v. TechCorp")                           -> separate DB, Smith v. Jones inaccessible
  7. search_case("claim construction")                        -> TechCorp-only results

Key invariant: delete_case/delete_document/reindex always cascade through
chunks -> embeddings -> provenance -> entities -> citations -> BM25 entries.
Original source files on disk are NEVER removed. See PRD 09 for full tool schemas.
```

### 9.7 Storage Lifecycle Operations

| Operation | Tool / CLI | What It Does | Data Impact |
|-----------|-----------|--------------|-------------|
| View storage usage | `get_storage_summary` (MCP) | Per-case disk usage, staleness detection, budget warning | Read-only |
| Compact a case | `compact_case` (MCP) | RocksDB compaction on all column families, reclaims tombstone space | ~20-40% storage reduction |
| Archive a case | `archive_case` (MCP) | Sets status to Archived, auto-runs compaction | ~30-50% storage reduction |
| Purge archived cases | `purge-archived` (CLI) | Exports to .ctcase ZIP, deletes expanded DB | ~70-90% storage reduction |
| Strip unused embeddings | `strip-embeddings` (CLI) | Removes E12 vectors on Pro->Free downgrade | ~60% embedding reduction |
| Close a case | `close_case` (MCP) | Sets status to Closed (read-only, still visible) | None |

**Startup behavior**: CaseTrack logs total disk usage on every startup. Warns at >70% of storage budget (default 10GB). See [PRD 04 Section 13](PRD_04_STORAGE_ARCHITECTURE.md#13-storage-lifecycle-management) for full storage lifecycle details.

---

## 10. License Validation

CaseTrack uses Ed25519-signed license keys for offline tier validation. See [PRD 10 Section 12](PRD_10_TECHNICAL_BUILD.md#12-monetization-implementation) for pricing tiers, payment flow, and billing architecture.

### 10.1 Tier Enum

```rust
// crates/casetrack-core/src/license/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    Free,
    Pro,
    Firm,
}

impl Tier {
    pub fn max_cases(&self) -> Option<u32> {
        match self {
            Tier::Free => Some(3),
            Tier::Pro | Tier::Firm => None, // Unlimited
        }
    }

    pub fn max_docs_per_case(&self) -> Option<u32> {
        match self {
            Tier::Free => Some(100),
            Tier::Pro | Tier::Firm => None, // Unlimited
        }
    }

    pub fn has_colbert_rerank(&self) -> bool {
        matches!(self, Tier::Pro | Tier::Firm)
    }

    pub fn has_entity_graph(&self) -> bool {
        matches!(self, Tier::Pro | Tier::Firm)
    }

    pub fn has_citation_network(&self) -> bool {
        matches!(self, Tier::Pro | Tier::Firm)
    }

    pub fn has_auto_sync(&self) -> bool {
        matches!(self, Tier::Pro | Tier::Firm)
    }

    pub fn has_case_export(&self) -> bool {
        matches!(self, Tier::Firm)
    }

    pub fn seats(&self) -> u8 {
        match self {
            Tier::Free | Tier::Pro => 1,
            Tier::Firm => u8::MAX, // Set by license key
        }
    }
}
```

### 10.2 License Key Struct

```rust
// crates/casetrack-core/src/license/mod.rs

use ed25519_dalek::{Signature, VerifyingKey, Verifier};

/// Ed25519-signed license key. Validated entirely offline.
/// The public key is embedded in the binary at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    /// Subscription tier
    pub tier: Tier,
    /// Number of concurrent seats (1 for Free/Pro, N for Firm)
    pub seats: u8,
    /// Unix timestamp: subscription end date + 30-day grace period
    pub expires_at: i64,
    /// Opaque customer identifier (for support lookup, not tracking)
    pub customer_id: String,
    /// Ed25519 signature over the serialized payload (tier + seats + expires_at + customer_id)
    #[serde(with = "signature_bytes")]
    pub signature: [u8; 64],
}

/// Serde helper for [u8; 64] signature bytes
mod signature_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        STANDARD.encode(bytes).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(d)?;
        let decoded = STANDARD.decode(&s).map_err(serde::de::Error::custom)?;
        let arr: [u8; 64] = decoded.try_into().map_err(|_| serde::de::Error::custom("invalid signature length"))?;
        Ok(arr)
    }
}
```

### 10.3 License Validator

```rust
// crates/casetrack-core/src/license/validator.rs

use ed25519_dalek::{Signature, VerifyingKey, Verifier};

/// Ed25519 public key, embedded in the binary at compile time.
/// Generated once by the key server; the private key NEVER leaves the server.
const PUBLIC_KEY_BYTES: [u8; 32] = *include_bytes!("public_key.bin");

pub struct LicenseValidator {
    public_key: VerifyingKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseStatus {
    /// Key is valid and not expired
    Valid,
    /// Key is valid but within the 30-day grace period (subscription lapsed)
    GracePeriod { days_remaining: u32 },
    /// Key has fully expired (past grace period)
    Expired,
    /// No license key provided -- Free tier
    NoKey,
    /// Key failed signature verification
    InvalidSignature,
    /// Key format is malformed
    MalformedKey,
}

impl LicenseValidator {
    pub fn new() -> Result<Self> {
        let public_key = VerifyingKey::from_bytes(&PUBLIC_KEY_BYTES)
            .map_err(|_| CaseTrackError::InvalidLicenseFormat)?;
        Ok(Self { public_key })
    }

    /// Validate a license key. Returns (Tier, LicenseStatus).
    /// On any failure, returns (Tier::Free, <error status>).
    pub fn verify(&self, key: &LicenseKey) -> (Tier, LicenseStatus) {
        // 1. Verify Ed25519 signature over payload
        let payload = self.serialize_payload(key);
        let signature = match Signature::from_bytes(&key.signature) {
            Ok(sig) => sig,
            Err(_) => return (Tier::Free, LicenseStatus::InvalidSignature),
        };

        if self.public_key.verify(&payload, &signature).is_err() {
            return (Tier::Free, LicenseStatus::InvalidSignature);
        }

        // 2. Check expiry
        let now = chrono::Utc::now().timestamp();
        if now > key.expires_at {
            return (Tier::Free, LicenseStatus::Expired);
        }

        // 3. Check grace period (last 30 days before expires_at)
        let grace_start = key.expires_at - (30 * 24 * 60 * 60);
        if now > grace_start {
            let days_remaining = ((key.expires_at - now) / (24 * 60 * 60)) as u32;
            return (key.tier, LicenseStatus::GracePeriod { days_remaining });
        }

        // 4. Valid
        (key.tier, LicenseStatus::Valid)
    }

    /// Serialize the payload fields for signature verification.
    /// Must match the key server's signing order exactly.
    fn serialize_payload(&self, key: &LicenseKey) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&(key.tier as u8).to_le_bytes());
        payload.extend_from_slice(&key.seats.to_le_bytes());
        payload.extend_from_slice(&key.expires_at.to_le_bytes());
        payload.extend_from_slice(key.customer_id.as_bytes());
        payload
    }
}
```

### 10.4 Startup License Check Flow

```rust
// Called during server startup (non-blocking for renewal check)

pub fn resolve_license(config: &Config) -> (Tier, LicenseStatus) {
    let validator = match LicenseValidator::new() {
        Ok(v) => v,
        Err(_) => return (Tier::Free, LicenseStatus::NoKey),
    };

    // 1. Try license from config (CLI flag, env var, or config file)
    let key_string = match &config.license_key {
        Some(k) => k.clone(),
        None => {
            // 2. Try cached license from data_dir/license.key
            match std::fs::read_to_string(config.data_dir.join("license.key")) {
                Ok(k) => k.trim().to_string(),
                Err(_) => return (Tier::Free, LicenseStatus::NoKey),
            }
        }
    };

    // 3. Parse the key
    let key: LicenseKey = match parse_license_key(&key_string) {
        Ok(k) => k,
        Err(_) => return (Tier::Free, LicenseStatus::MalformedKey),
    };

    // 4. Validate offline
    let (tier, status) = validator.verify(&key);

    // 5. Log status
    match &status {
        LicenseStatus::Valid => {
            tracing::info!("License valid: {:?} tier, {} seats", tier, key.seats);
        }
        LicenseStatus::GracePeriod { days_remaining } => {
            tracing::warn!(
                "License in grace period: {} days remaining. Renew at https://casetrack.dev/account",
                days_remaining
            );
        }
        LicenseStatus::Expired => {
            tracing::warn!("License expired. Running as Free tier. Renew at https://casetrack.dev/account");
        }
        LicenseStatus::InvalidSignature => {
            tracing::error!("Invalid license key signature. Running as Free tier.");
        }
        LicenseStatus::MalformedKey => {
            tracing::error!("Malformed license key. Running as Free tier.");
        }
        LicenseStatus::NoKey => {}
    }

    // 6. Spawn non-blocking renewal check (background task)
    if matches!(status, LicenseStatus::GracePeriod { .. }) {
        tokio::spawn(async move {
            if let Ok(new_key) = check_renewal(&key.customer_id).await {
                // Cache the new key locally
                let _ = std::fs::write(
                    config.data_dir.join("license.key"),
                    &new_key,
                );
                tracing::info!("License auto-renewed successfully");
            }
        });
    }

    (tier, status)
}

/// Non-blocking check for a renewed license key.
/// Returns the raw key string if a newer key is available.
async fn check_renewal(customer_id: &str) -> Result<String> {
    let url = format!("https://casetrack.dev/api/license/{}", customer_id);
    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| CaseTrackError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    let key = resp.text().await
        .map_err(|e| CaseTrackError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    Ok(key.trim().to_string())
}
```

### 10.5 Feature Gating at Runtime

```rust
/// Check tier limits before operations. Returns CaseTrackError::FreeTierLimit on violation.
pub fn check_case_limit(tier: Tier, current_count: u32) -> Result<()> {
    if let Some(max) = tier.max_cases() {
        if current_count >= max {
            return Err(CaseTrackError::FreeTierLimit {
                resource: "cases".to_string(),
                current: current_count,
                max,
            });
        }
    }
    Ok(())
}

pub fn check_document_limit(tier: Tier, current_count: u32) -> Result<()> {
    if let Some(max) = tier.max_docs_per_case() {
        if current_count >= max {
            return Err(CaseTrackError::FreeTierLimit {
                resource: "documents per case".to_string(),
                current: current_count,
                max,
            });
        }
    }
    Ok(())
}
```

### 10.6 Expiry Behavior

| State | Behavior |
|-------|----------|
| Valid (subscription active) | Full tier features |
| Grace period (subscription lapsed, <30 days) | Full tier features + warning on startup |
| Expired (>30 days past subscription end) | Downgrade to Free tier |
| No key | Free tier |
| Invalid signature | Free tier + error log |

**On expiry downgrade:**
- Cases beyond the Free limit (>3) become **read-only** (search works, no new ingestion)
- No data is deleted -- the user's work is preserved
- ColBERT rerank, auto-sync, entity graph, citation network are disabled
- Clear upgrade prompt in CLI output and MCP error responses

---

*CaseTrack PRD v5.1.0 -- Document 7 of 10*

# PRD 07: Case Management & Provenance

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Case Model

```rust
/// A legal case/matter containing documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: Uuid,
    pub name: String,
    pub case_number: Option<String>,
    pub case_type: CaseType,
    pub status: CaseStatus,
    pub created_at: i64,     // Unix timestamp
    pub updated_at: i64,     // Unix timestamp
    pub stats: CaseStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseStats {
    pub document_count: u32,
    pub page_count: u32,
    pub chunk_count: u32,
    pub storage_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaseType {
    Civil,
    Criminal,
    Family,
    Bankruptcy,
    Contract,
    Employment,
    PersonalInjury,
    RealEstate,
    IntellectualProperty,
    Immigration,
    Other,
}

impl CaseType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "civil" => Self::Civil,
            "criminal" => Self::Criminal,
            "family" => Self::Family,
            "bankruptcy" => Self::Bankruptcy,
            "contract" => Self::Contract,
            "employment" => Self::Employment,
            "personal_injury" => Self::PersonalInjury,
            "real_estate" => Self::RealEstate,
            "intellectual_property" | "ip" => Self::IntellectualProperty,
            "immigration" => Self::Immigration,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CaseStatus {
    Active,
    Closed,
    Archived,
}
```

---

## 2. Case Registry

The registry is a shared RocksDB instance that indexes all cases:

```rust
pub struct CaseRegistry {
    db: rocksdb::DB,
    data_dir: PathBuf,
    active_case: Option<Uuid>,
}

impl CaseRegistry {
    pub fn open(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("registry.db");
        let db = rocksdb::DB::open_default(&db_path)
            .map_err(|e| CaseTrackError::RegistryOpenFailed { source: e })?;

        Ok(Self {
            db,
            data_dir: data_dir.to_path_buf(),
            active_case: None,
        })
    }

    pub fn create_case(&mut self, params: CreateCaseParams) -> Result<Case> {
        let id = Uuid::new_v4();
        let case_dir = self.data_dir.join("cases").join(id.to_string());
        fs::create_dir_all(case_dir.join("originals"))?;

        let case = Case {
            id,
            name: params.name,
            case_number: params.case_number,
            case_type: params.case_type.unwrap_or(CaseType::Other),
            status: CaseStatus::Active,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
            stats: CaseStats::default(),
        };

        // Initialize the case database (creates column families)
        CaseHandle::initialize(&case_dir)?;

        // Store in registry
        let key = format!("case:{}", id);
        self.db.put(key.as_bytes(), bincode::serialize(&case)?)?;

        // Auto-switch to new case
        self.active_case = Some(id);

        Ok(case)
    }

    pub fn get_case(&self, case_id: Uuid) -> Result<Case> {
        let key = format!("case:{}", case_id);
        let bytes = self.db.get(key.as_bytes())?
            .ok_or(CaseTrackError::CaseNotFound(case_id))?;
        Ok(bincode::deserialize(&bytes)?)
    }

    pub fn list_cases(&self) -> Result<Vec<Case>> {
        let mut cases = Vec::new();
        let iter = self.db.prefix_iterator(b"case:");
        for item in iter {
            let (key, value) = item?;
            if key.starts_with(b"case:") {
                let case: Case = bincode::deserialize(&value)?;
                cases.push(case);
            }
        }
        cases.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(cases)
    }

    pub fn update_case(&mut self, case: &Case) -> Result<()> {
        let key = format!("case:{}", case.id);
        self.db.put(key.as_bytes(), bincode::serialize(case)?)?;
        Ok(())
    }

    pub fn delete_case(&mut self, case_id: Uuid) -> Result<()> {
        // Remove from registry
        let key = format!("case:{}", case_id);
        self.db.delete(key.as_bytes())?;

        // Remove case directory (RocksDB + originals)
        let case_dir = self.data_dir.join("cases").join(case_id.to_string());
        if case_dir.exists() {
            fs::remove_dir_all(&case_dir)?;
        }

        // Clear active case if it was the deleted one
        if self.active_case == Some(case_id) {
            self.active_case = None;
        }

        Ok(())
    }

    pub fn switch_case(&mut self, case_id: Uuid) -> Result<CaseHandle> {
        let _case = self.get_case(case_id)?; // Validates existence
        let case_dir = self.data_dir.join("cases").join(case_id.to_string());
        let handle = CaseHandle::open(&case_dir)?;
        self.active_case = Some(case_id);
        Ok(handle)
    }

    pub fn active_case_id(&self) -> Option<Uuid> {
        self.active_case
    }

    pub fn count_cases(&self) -> Result<u32> {
        Ok(self.list_cases()?.len() as u32)
    }
}

pub struct CreateCaseParams {
    pub name: String,
    pub case_number: Option<String>,
    pub case_type: Option<CaseType>,
}
```

---

## 3. Case Handle

```rust
/// Handle to an open case database
pub struct CaseHandle {
    pub db: rocksdb::DB,
    pub case_id: Uuid,
    pub case_dir: PathBuf,
}

impl CaseHandle {
    /// Initialize a new case database with all column families
    pub fn initialize(case_dir: &Path) -> Result<()> {
        let db_path = case_dir.join("case.db");
        let opts = crate::storage::rocks_options();

        let cfs: Vec<rocksdb::ColumnFamilyDescriptor> = super::COLUMN_FAMILIES
            .iter()
            .map(|name| rocksdb::ColumnFamilyDescriptor::new(*name, opts.clone()))
            .collect();

        let _db = rocksdb::DB::open_cf_descriptors(&opts, &db_path, cfs)?;
        // DB is dropped/closed here -- will be reopened by open()
        Ok(())
    }

    pub fn open(case_dir: &Path) -> Result<Self> {
        let db_path = case_dir.join("case.db");
        let opts = crate::storage::rocks_options();

        let cfs: Vec<rocksdb::ColumnFamilyDescriptor> = super::COLUMN_FAMILIES
            .iter()
            .map(|name| rocksdb::ColumnFamilyDescriptor::new(*name, opts.clone()))
            .collect();

        let db = rocksdb::DB::open_cf_descriptors(&opts, &db_path, cfs)
            .map_err(|e| CaseTrackError::CaseDbOpenFailed {
                path: db_path,
                source: e,
            })?;

        let case_id = Uuid::parse_str(
            case_dir.file_name().unwrap().to_str().unwrap()
        )?;

        Ok(Self { db, case_id, case_dir: case_dir.to_path_buf() })
    }

    // --- Document Operations ---

    pub fn store_document(&self, doc: &DocumentMetadata) -> Result<()> {
        let cf = self.db.cf_handle("documents").unwrap();
        let key = format!("doc:{}", doc.id);
        self.db.put_cf(&cf, key.as_bytes(), bincode::serialize(doc)?)?;
        Ok(())
    }

    pub fn get_document(&self, doc_id: Uuid) -> Result<DocumentMetadata> {
        let cf = self.db.cf_handle("documents").unwrap();
        let key = format!("doc:{}", doc_id);
        let bytes = self.db.get_cf(&cf, key.as_bytes())?
            .ok_or(CaseTrackError::DocumentNotFound(doc_id))?;
        Ok(bincode::deserialize(&bytes)?)
    }

    pub fn list_documents(&self) -> Result<Vec<DocumentMetadata>> {
        let cf = self.db.cf_handle("documents").unwrap();
        let iter = self.db.prefix_iterator_cf(&cf, b"doc:");
        let mut docs = Vec::new();
        for item in iter {
            let (_, value) = item?;
            docs.push(bincode::deserialize(&value)?);
        }
        docs.sort_by(|a, b| b.ingested_at.cmp(&a.ingested_at));
        Ok(docs)
    }

    pub fn delete_document(&self, doc_id: Uuid) -> Result<()> {
        // Delete document metadata
        let cf = self.db.cf_handle("documents").unwrap();
        self.db.delete_cf(&cf, format!("doc:{}", doc_id).as_bytes())?;

        // Delete all chunks for this document
        let chunks_cf = self.db.cf_handle("chunks").unwrap();
        let idx_cf = self.db.cf_handle("chunks").unwrap();
        let prefix = format!("doc_chunks:{}:", doc_id);
        let iter = self.db.prefix_iterator_cf(&idx_cf, prefix.as_bytes());
        for item in iter {
            let (key, value) = item?;
            let chunk_id_str = String::from_utf8_lossy(&value);
            // Delete chunk, embeddings, provenance
            self.delete_chunk_data(&chunk_id_str)?;
            self.db.delete_cf(&idx_cf, &key)?;
        }

        Ok(())
    }

    // --- Chunk Operations ---

    pub fn store_chunk(&self, chunk: &Chunk) -> Result<()> {
        let cf = self.db.cf_handle("chunks").unwrap();
        let key = format!("chunk:{}", chunk.id);
        self.db.put_cf(&cf, key.as_bytes(), bincode::serialize(chunk)?)?;

        // Also store document->chunk index
        let idx_key = format!("doc_chunks:{}:{:06}", chunk.document_id, chunk.sequence);
        self.db.put_cf(&cf, idx_key.as_bytes(), chunk.id.to_string().as_bytes())?;

        Ok(())
    }

    pub fn get_chunk(&self, chunk_id: Uuid) -> Result<Chunk> {
        let cf = self.db.cf_handle("chunks").unwrap();
        let key = format!("chunk:{}", chunk_id);
        let bytes = self.db.get_cf(&cf, key.as_bytes())?
            .ok_or(CaseTrackError::ChunkNotFound(chunk_id))?;
        Ok(bincode::deserialize(&bytes)?)
    }
}
```

---

## 4. Provenance System

### 4.1 Provenance Model

Every chunk tracks exactly where it came from:

```rust
/// EVERY chunk stores full provenance. This is the core of CaseTrack.
/// When the AI returns information, the user must know EXACTLY where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    // === Source Document ===
    /// UUID of the ingested document
    pub document_id: Uuid,
    /// Original filename ("Contract.pdf")
    pub document_name: String,
    /// Full filesystem path where the file was when ingested
    /// ("/Users/sarah/Cases/Smith/Contract.pdf")
    pub document_path: Option<PathBuf>,

    // === Location in Document ===
    /// Page number (1-indexed)
    pub page: u32,
    /// First paragraph index included in this chunk (0-indexed within page)
    pub paragraph_start: u32,
    /// Last paragraph index included in this chunk
    pub paragraph_end: u32,
    /// First line number (1-indexed within page)
    pub line_start: u32,
    /// Last line number
    pub line_end: u32,

    // === Character Offsets (for exact highlighting) ===
    /// Character offset from start of page
    pub char_start: u64,
    /// Character offset end
    pub char_end: u64,

    // === Extraction Metadata ===
    /// How the text was extracted from the original file
    pub extraction_method: ExtractionMethod,
    /// OCR confidence score (0.0-1.0) if extracted via OCR
    pub ocr_confidence: Option<f32>,

    // === Legal Metadata ===
    /// Optional Bates stamp number (for litigation document production)
    pub bates_number: Option<String>,

    // === Chunk Position ===
    /// Sequential position of this chunk within the entire document (0-indexed)
    pub chunk_index: u32,
}

impl Provenance {
    /// Generate a legal citation string
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

        if let Some(bates) = &self.bates_number {
            parts.push(format!("({})", bates));
        }

        parts.join(", ")
    }

    /// Short citation for inline use
    pub fn cite_short(&self) -> String {
        if let Some(bates) = &self.bates_number {
            bates.clone()
        } else {
            format!("{}, p. {}",
                self.document_name.split('.').next().unwrap_or(&self.document_name),
                self.page
            )
        }
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
    pub citation: String,
    pub citation_short: String,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
}

impl SearchResult {
    pub fn to_mcp_content(&self) -> serde_json::Value {
        json!({
            "text": self.text,
            "score": self.score,
            "citation": self.citation,
            "citation_short": self.citation_short,
            "source": {
                "document": self.provenance.document_name,
                "page": self.provenance.page,
                "paragraph_start": self.provenance.paragraph_start,
                "paragraph_end": self.provenance.paragraph_end,
                "lines": format!("{}-{}", self.provenance.line_start, self.provenance.line_end),
                "bates": self.provenance.bates_number,
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

Search results include surrounding context for better comprehension:

```rust
impl CaseHandle {
    /// Get chunks immediately before and after a given chunk
    pub fn get_surrounding_context(
        &self,
        chunk: &Chunk,
        window: usize,  // Number of chunks before/after
    ) -> Result<(Option<String>, Option<String>)> {
        let cf = self.db.cf_handle("chunks").unwrap();

        let before = if chunk.sequence > 0 {
            let prev_idx = format!(
                "doc_chunks:{}:{:06}",
                chunk.document_id,
                chunk.sequence - 1
            );
            self.db.get_cf(&cf, prev_idx.as_bytes())?
                .and_then(|id_bytes| {
                    let id = Uuid::parse_str(&String::from_utf8_lossy(&id_bytes)).ok()?;
                    self.get_chunk(id).ok().map(|c| c.text)
                })
        } else {
            None
        };

        let after = {
            let next_idx = format!(
                "doc_chunks:{}:{:06}",
                chunk.document_id,
                chunk.sequence + 1
            );
            self.db.get_cf(&cf, next_idx.as_bytes())?
                .and_then(|id_bytes| {
                    let id = Uuid::parse_str(&String::from_utf8_lossy(&id_bytes)).ok()?;
                    self.get_chunk(id).ok().map(|c| c.text)
                })
        };

        Ok((before, after))
    }
}
```

---

## 5. Case Lifecycle

```
CASE LIFECYCLE
=================================================================================

  create_case("Smith v. Jones")
       |
       v
  [ACTIVE] -----> ingest_pdf, ingest_docx, search_case
       |
       |  close_case()          reopen_case()
       v                             |
  [CLOSED] --------> (read-only) ---+
       |
       |  archive_case()
       v
  [ARCHIVED] -----> (read-only, not shown in default list)
       |
       |  delete_case()
       v
  [DELETED] -----> case directory removed from disk

Notes:
  - ACTIVE: Full read/write. Can ingest, search, modify.
  - CLOSED: Read-only. Search works. Cannot ingest new documents.
  - ARCHIVED: Same as closed but hidden from default list_cases.
  - DELETED: Completely removed. Not recoverable.
```

---

*CaseTrack PRD v4.0.0 -- Document 7 of 10*

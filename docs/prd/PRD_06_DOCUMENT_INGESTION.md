# PRD 06: Document Ingestion

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Supported Formats

| Format | Method | Quality | Rust Crate | Notes |
|--------|--------|---------|------------|-------|
| PDF (native text) | pdf-extract | Excellent | `pdf-extract`, `lopdf` | Direct text extraction |
| PDF (scanned) | Tesseract OCR | Good (>95%) | `tesseract` | Requires image rendering |
| DOCX | docx-rs | Excellent | `docx-rs` | Preserves structure |
| DOC (legacy) | Convert via LibreOffice | Good | CLI shelling | Optional, warns user |
| XLSX/XLS/ODS | calamine | Excellent | `calamine` | Pure Rust, reads all spreadsheet formats |
| EML | mailparse | Excellent | `mailparse` | Email messages common in litigation |
| Images (JPG/PNG/TIFF) | Tesseract OCR | Good | `tesseract`, `image` | Single page per image |
| TXT/RTF | Direct read | Excellent | `std::fs` | Plain text, no metadata |

### 1.1 EML Processing

Email messages (`.eml`) are a critical format in litigation discovery. CaseTrack extracts:

- **Headers**: From, To, CC, BCC, Date, Subject, Message-ID, In-Reply-To
- **Body**: Plain text and HTML (HTML converted to plain text via `html2text`)
- **Attachments**: Extracted and ingested as separate documents linked to parent email
- **Thread reconstruction**: In-Reply-To and References headers build email chains

```rust
pub struct EmlProcessor;

impl EmlProcessor {
    pub fn process(&self, path: &Path) -> Result<ParsedDocument> {
        let raw = fs::read(path)
            .map_err(|e| CaseTrackError::FileReadError {
                path: path.to_path_buf(),
                source: e,
            })?;
        let parsed = mailparse::parse_mail(&raw)
            .map_err(|e| CaseTrackError::EmlParseError {
                path: path.to_path_buf(),
                source: e,
            })?;

        let headers = self.extract_headers(&parsed);
        let body = self.extract_body(&parsed)?;
        let attachments = self.extract_attachments(&parsed)?;

        // Email body becomes page 1; each attachment becomes a subsequent page
        let mut pages = vec![Page {
            number: 1,
            content: format!(
                "From: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
                headers.from, headers.to, headers.date, headers.subject, body
            ),
            paragraphs: self.detect_paragraphs(&body),
            extraction_method: ExtractionMethod::Native,
            ocr_confidence: None,
        }];

        // Attachments are returned separately for individual ingestion
        Ok(ParsedDocument {
            id: Uuid::new_v4(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            pages,
            metadata: DocumentMetadataRaw {
                title: Some(headers.subject),
                author: Some(headers.from),
                created_date: Some(headers.date),
            },
            file_hash: compute_sha256(path)?,
            attachments,
        })
    }
}
```

---

## 2. Ingestion Pipeline

```
DOCUMENT INGESTION FLOW (Legal Case Management)
=================================================================================

User: "Ingest ~/Cases/SmithVJones/Complaint.pdf"
                    |
                    v
+-----------------------------------------------------------------------+
| 1. VALIDATE                                                            |
|    - Check file exists and is readable                                |
|    - Detect file type (by extension + magic bytes)                    |
|    - Check file size (warn if >100MB)                                 |
|    - Check for duplicates (SHA256 hash comparison)                    |
|    Output: ValidatedFile { path, file_type, hash, size }             |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 2. PARSE                                                               |
|    - Route to format-specific parser                                  |
|    - Extract text with position metadata                              |
|    - For scanned pages: detect and run OCR                            |
|    - For spreadsheets: extract sheets, rows, and cell data            |
|    - For emails (.eml): extract headers, body, attachments            |
|    - Extract document metadata (title, author, dates)                 |
|    Output: ParsedDocument { pages: Vec<Page>, metadata }              |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 3. DETECT LEGAL DOCUMENT TYPE                                          |
|    - Classify document into legal category for chunking strategy      |
|    - Detection by: filename patterns, content heuristics, metadata    |
|    - Categories: Contract, Deposition, Brief, CourtOpinion,           |
|      Statute, Correspondence, Discovery, Pleading, Default            |
|    Output: LegalDocumentType                                          |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 4. CHUNK (provenance is attached here -- THE MOST CRITICAL STEP)       |
|    - Route to legal-aware chunker based on document type (see 7.1)   |
|    - Contracts: clause-level chunking                                 |
|    - Depositions: Q&A pair chunking                                   |
|    - Briefs/Motions: argument-level chunking                          |
|    - Court opinions: section-level chunking                           |
|    - Statutes: section/subsection chunking                            |
|    - Default: paragraph-aware 2000-char chunking                      |
|    - Attach FULL provenance to EVERY chunk:                            |
|      * document_path: absolute file path on disk                       |
|      * document_name: original filename                                |
|      * page: page number (1-indexed)                                   |
|      * paragraph_start/end: which paragraphs this chunk spans          |
|      * line_start/end: which lines this chunk spans                    |
|      * char_start/end: exact character offsets within the page          |
|      * sheet_name: sheet name (for spreadsheets)                       |
|      * row_range: row range (for spreadsheets, e.g., rows 1-45)       |
|      * column_range: column range (for spreadsheets)                   |
|      * legal_section: clause number, Q&A index, argument heading       |
|      * extraction_method: Native / OCR / Hybrid / Spreadsheet / Email  |
|      * ocr_confidence: quality score for OCR-extracted text             |
|      * created_at: Unix timestamp of chunk creation                     |
|      * embedded_at: Unix timestamp (set after Step 5)                   |
|    A chunk without provenance MUST NOT be stored. Period.              |
|    Output: Vec<Chunk> with Provenance                                  |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 5. EMBED                                                               |
|    - Run each chunk through active embedders:                         |
|      * Legal-BERT-base (E1, 768D) -- primary semantic                 |
|      * SPLADE (E6) -- sparse keyword matching                         |
|      * ColBERT-v2 (E12) -- per-token reranking                        |
|      * BM25 (E13) -- inverted index for recall                        |
|    - Batch for efficiency (32 chunks at a time)                       |
|    Output: Vec<ChunkWithEmbeddings>                                   |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 6. EXTRACT LEGAL ENTITIES                                              |
|    - Run NER + regex extractors on each chunk                         |
|    - Legal entity types: Party, Court, Judge, Attorney, Statute,       |
|      CaseNumber, Jurisdiction, LegalConcept, Remedy, Witness,          |
|      Exhibit, DocketEntry, Date, Amount, Person, Organization,         |
|      Location                                                          |
|    - Deduplicate within document (same entity, different mentions)     |
|    - Store Entity and EntityMention records in entities CF             |
|    - Update entity_index for fast lookup                               |
|    Output: Vec<Entity>, Vec<EntityMention>                             |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 7. EXTRACT LEGAL CITATIONS                                             |
|    - Detect Bluebook-format citations in each chunk                   |
|    - Case citations: Party v. Party, Vol. Reporter Page (Court Year)  |
|    - Statute citations: Title U.S.C. § Section                        |
|    - Regulation citations: Title C.F.R. § Section                     |
|    - Short-form: Id., supra, infra                                    |
|    - Store citation records with chunk_id + char offsets               |
|    - Build citation-to-chunk index for cross-referencing               |
|    Output: Vec<LegalCitation>, Vec<CitationMention>                    |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 8. BUILD KNOWLEDGE GRAPH                                               |
|    - Entity-to-Chunk edges (from Step 6)                              |
|    - Citation-to-Chunk edges (from Step 7)                            |
|    - Cross-document entity matching (same entity across documents)     |
|    - Citation graph edges (documents citing the same authority)        |
|    - Document-to-Document edges (shared entities, shared citations)    |
|    - Compute document-level E1 similarity (SemanticSimilar edges)     |
|    Output: Vec<DocRelationship>                                        |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 9. STORE (provenance chain is sealed here)                             |
|    - Write chunks + provenance to case RocksDB (chunks CF)            |
|      Each chunk stored with its FULL provenance inline                |
|    - Write embedding vectors to embeddings CF, keyed by chunk_id      |
|      chunk_id is the bridge: embedding -> chunk -> provenance -> file  |
|    - Update embedded_at timestamp on each chunk                       |
|    - Write provenance records to provenance CF (prov:{chunk_uuid})    |
|    - Write entity records to entities CF                               |
|    - Write citation records to citations CF                            |
|    - Update BM25 inverted index                                       |
|    - Update document metadata (ingested_at, updated_at timestamps)    |
|    - Update case stats                                                 |
|    - Optionally copy original file to case/originals/                  |
|    Output: IngestResult { pages, chunks, entities, citations, dur }   |
+-----------------------------------------------------------------------+
                    |
                    v
Response: "Ingested Complaint.pdf: 12 pages, 67 chunks, 23 entities, 8 citations, 4s"
```

### 2.1 Legal Document Type Detection

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LegalDocumentType {
    Contract,       // Agreements, leases, NDAs -- clause-level chunking
    Deposition,     // Transcripts -- Q&A pair chunking
    Brief,          // Motions, memoranda -- argument-level chunking
    CourtOpinion,   // Judicial opinions -- section-level chunking
    Statute,        // Laws, regulations -- section/subsection chunking
    Correspondence, // Letters, emails -- paragraph-aware chunking
    Discovery,      // Interrogatories, requests -- numbered item chunking
    Pleading,       // Complaints, answers -- paragraph/count chunking
    Default,        // Unrecognized -- paragraph-aware 2000-char chunking
}

impl LegalDocumentType {
    /// Detect document type from filename patterns and content heuristics
    pub fn detect(filename: &str, content_preview: &str) -> Self {
        let lower_name = filename.to_lowercase();
        let lower_content = content_preview.to_lowercase();

        // Filename-based detection
        if lower_name.contains("contract") || lower_name.contains("agreement")
            || lower_name.contains("lease") || lower_name.contains("nda")
        {
            return Self::Contract;
        }
        if lower_name.contains("deposition") || lower_name.contains("transcript") {
            return Self::Deposition;
        }
        if lower_name.contains("brief") || lower_name.contains("motion")
            || lower_name.contains("memorandum")
        {
            return Self::Brief;
        }
        if lower_name.contains("opinion") || lower_name.contains("order")
            || lower_name.contains("ruling")
        {
            return Self::CourtOpinion;
        }

        // Content-based heuristics
        if lower_content.contains("whereas") && lower_content.contains("hereby") {
            return Self::Contract;
        }
        if lower_content.contains("q.") && lower_content.contains("a.") {
            return Self::Deposition;
        }
        if lower_content.contains("argument") && lower_content.contains("conclusion") {
            return Self::Brief;
        }
        if lower_content.contains("holding") && lower_content.contains("affirmed") {
            return Self::CourtOpinion;
        }

        Self::Default
    }
}
```

---

## 3. PDF Processing

```rust
use lopdf::Document as PdfDocument;

pub struct PdfProcessor {
    ocr_enabled: bool,
    ocr_language: String,  // "eng" default
}

impl PdfProcessor {
    pub fn process(&self, path: &Path) -> Result<ParsedDocument> {
        let pdf = PdfDocument::load(path)
            .map_err(|e| CaseTrackError::PdfParseError {
                path: path.to_path_buf(),
                source: e,
            })?;

        let page_count = pdf.get_pages().len();
        let mut pages = Vec::with_capacity(page_count);
        let metadata = self.extract_pdf_metadata(&pdf)?;

        for page_num in 1..=page_count {
            // Try native text extraction first
            let native_text = pdf_extract::extract_text_from_page(&pdf, page_num)
                .unwrap_or_default();

            let trimmed = native_text.trim();

            if trimmed.is_empty() || self.looks_like_scanned(trimmed) {
                if self.ocr_enabled {
                    // Scanned page -- use OCR
                    let image = self.render_page_to_image(&pdf, page_num)?;
                    let ocr_result = self.run_ocr(&image)?;
                    pages.push(Page {
                        number: page_num as u32,
                        content: ocr_result.text,
                        paragraphs: self.detect_paragraphs(&ocr_result.text),
                        extraction_method: ExtractionMethod::Ocr,
                        ocr_confidence: Some(ocr_result.confidence),
                    });
                } else {
                    // OCR disabled -- store empty page with warning
                    tracing::warn!(
                        "Page {} appears scanned but OCR is disabled. Skipping.",
                        page_num
                    );
                    pages.push(Page {
                        number: page_num as u32,
                        content: String::new(),
                        paragraphs: vec![],
                        extraction_method: ExtractionMethod::Skipped,
                        ocr_confidence: None,
                    });
                }
            } else {
                pages.push(Page {
                    number: page_num as u32,
                    content: native_text,
                    paragraphs: self.detect_paragraphs(&native_text),
                    extraction_method: ExtractionMethod::Native,
                    ocr_confidence: None,
                });
            }
        }

        Ok(ParsedDocument {
            id: Uuid::new_v4(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            pages,
            metadata,
            file_hash: compute_sha256(path)?,
            attachments: vec![],
        })
    }

    /// Heuristic: if extracted text is mostly whitespace or control chars, it's scanned
    fn looks_like_scanned(&self, text: &str) -> bool {
        let alpha_ratio = text.chars().filter(|c| c.is_alphanumeric()).count() as f32
            / text.len().max(1) as f32;
        alpha_ratio < 0.3
    }

    fn extract_pdf_metadata(&self, pdf: &PdfDocument) -> Result<DocumentMetadataRaw> {
        // Extract from PDF info dictionary if present
        let info = pdf.trailer.get(b"Info")
            .and_then(|r| pdf.get_object(r.as_reference().ok()?).ok());

        Ok(DocumentMetadataRaw {
            title: self.get_pdf_string(&info, b"Title"),
            author: self.get_pdf_string(&info, b"Author"),
            created_date: self.get_pdf_string(&info, b"CreationDate"),
        })
    }
}
```

---

## 4. DOCX Processing

```rust
pub struct DocxProcessor;

impl DocxProcessor {
    pub fn process(&self, path: &Path) -> Result<ParsedDocument> {
        let docx = docx_rs::read_docx(&fs::read(path)?)
            .map_err(|e| CaseTrackError::DocxParseError {
                path: path.to_path_buf(),
                source: e,
            })?;

        let mut pages = vec![];
        let mut current_page = Page::new(1);
        let mut para_idx = 0;

        for element in &docx.document.children {
            match element {
                DocumentChild::Paragraph(para) => {
                    let text = self.extract_paragraph_text(para);
                    if !text.trim().is_empty() {
                        current_page.paragraphs.push(Paragraph {
                            index: para_idx,
                            text: text.clone(),
                            style: self.detect_style(para),
                        });
                        current_page.content.push_str(&text);
                        current_page.content.push('\n');
                        para_idx += 1;
                    }
                }
                DocumentChild::SectionProperty(sp) => {
                    // Section break = new page (approximate)
                    if !current_page.content.is_empty() {
                        pages.push(current_page);
                        current_page = Page::new(pages.len() as u32 + 1);
                    }
                }
                _ => {}
            }
        }

        // Don't forget the last page
        if !current_page.content.is_empty() {
            pages.push(current_page);
        }

        Ok(ParsedDocument {
            id: Uuid::new_v4(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            pages,
            metadata: DocumentMetadataRaw::default(),
            file_hash: compute_sha256(path)?,
            attachments: vec![],
        })
    }
}
```

---

## 5. XLSX/Excel Processing

```rust
use calamine::{open_workbook_auto, Reader, DataType};

pub struct XlsxProcessor;

impl XlsxProcessor {
    pub fn process(&self, path: &Path) -> Result<ParsedDocument> {
        let mut workbook = open_workbook_auto(path)
            .map_err(|e| CaseTrackError::SpreadsheetParseError {
                path: path.to_path_buf(),
                source: e,
            })?;

        let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
        let mut pages = Vec::with_capacity(sheet_names.len());

        for (sheet_idx, sheet_name) in sheet_names.iter().enumerate() {
            let range = workbook.worksheet_range(sheet_name)
                .map_err(|e| CaseTrackError::SpreadsheetParseError {
                    path: path.to_path_buf(),
                    source: e,
                })?;

            // Detect headers from first row
            let headers: Vec<String> = range.rows().next()
                .map(|row| row.iter().map(|cell| cell.to_string()).collect())
                .unwrap_or_default();

            let mut content = String::new();
            let mut paragraphs = Vec::new();
            let mut para_idx = 0;

            for (row_idx, row) in range.rows().enumerate() {
                let row_text: Vec<String> = row.iter()
                    .enumerate()
                    .filter(|(_, cell)| !cell.is_empty())
                    .map(|(col_idx, cell)| {
                        let header = headers.get(col_idx)
                            .filter(|h| !h.is_empty() && row_idx > 0);
                        match header {
                            Some(h) => format!("{}: {}", h, cell),
                            None => cell.to_string(),
                        }
                    })
                    .collect();

                if !row_text.is_empty() {
                    let line = row_text.join(" | ");
                    paragraphs.push(Paragraph {
                        index: para_idx,
                        text: line.clone(),
                        style: if row_idx == 0 { ParagraphStyle::Heading } else { ParagraphStyle::Body },
                    });
                    content.push_str(&line);
                    content.push('\n');
                    para_idx += 1;
                }
            }

            // Each sheet becomes a logical "page"
            pages.push(Page {
                number: (sheet_idx + 1) as u32,
                content,
                paragraphs,
                extraction_method: ExtractionMethod::Spreadsheet,
                ocr_confidence: None,
            });
        }

        Ok(ParsedDocument {
            id: Uuid::new_v4(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            pages,
            metadata: DocumentMetadataRaw::default(),
            file_hash: compute_sha256(path)?,
            attachments: vec![],
        })
    }
}
```

**Spreadsheet provenance**: Each chunk from a spreadsheet includes `sheet_name`, `row_range` (e.g., rows 1-45), and `column_range` in its provenance record, enabling precise traceability back to specific cells.

---

## 6. OCR (Tesseract)

### 6.1 Bundling Strategy

| Platform | Method |
|----------|--------|
| macOS | Statically linked via `leptonica-sys` + `tesseract-sys` |
| Windows | Tesseract DLLs in installer/MCPB bundle |
| Linux | Statically linked via musl build |

The `eng.traineddata` (~15MB) is bundled or downloaded on first OCR use.

### 6.2 OCR Pipeline

```rust
pub struct OcrEngine {
    tesseract: tesseract::Tesseract,
}

impl OcrEngine {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let tessdata = data_dir.join("models").join("tessdata");
        let tesseract = tesseract::Tesseract::new(tessdata.to_str().unwrap(), "eng")?;
        Ok(Self { tesseract })
    }

    pub fn recognize(&self, image: &image::DynamicImage) -> Result<OcrResult> {
        let processed = self.preprocess(image);
        let bytes = processed.to_luma8();

        let mut tess = self.tesseract.clone();
        tess.set_image(bytes.as_raw(), bytes.width() as i32, bytes.height() as i32, 1, bytes.width() as i32)?;

        Ok(OcrResult {
            text: tess.get_text()?,
            confidence: tess.mean_text_conf() as f32 / 100.0,
        })
    }

    fn preprocess(&self, image: &image::DynamicImage) -> image::DynamicImage {
        image.grayscale().adjust_contrast(1.5)
    }
}
```

---

## 7. Legal-Aware Chunking Strategy

### 7.1 Chunking by Legal Document Type (MANDATORY)

Legal documents have distinct structures. Chunking MUST respect these structures to preserve semantic coherence. The chunker selects a strategy based on the `LegalDocumentType` detected in pipeline Step 3.

| Document Type | Chunking Strategy | Unit | Target Size | Overlap |
|---------------|-------------------|------|-------------|---------|
| Contract | Clause-level | Numbered section/clause | 1500-2500 chars | None (clause boundaries are hard) |
| Deposition | Q&A pair | Question + Answer together | 1000-3000 chars | None (Q&A pairs are atomic) |
| Brief/Motion | Argument-level | Argument heading + body | 1500-2500 chars | 10% at section breaks |
| Court Opinion | Section-level | Holding, reasoning, history | 1500-2500 chars | 10% at section breaks |
| Statute | Section/subsection | Statutory section | 1000-2000 chars | None (section boundaries are hard) |
| Correspondence | Paragraph-aware | Paragraphs | 2000 chars | 10% (200 chars) |
| Discovery | Numbered item | Request/Interrogatory + Response | 1000-3000 chars | None |
| Pleading | Paragraph/count | Numbered paragraphs | 2000 chars | 10% (200 chars) |
| Default | Paragraph-aware | Paragraphs | 2000 chars | 10% (200 chars) |

Character-based (not token-based) for deterministic, reproducible chunking.

**Boundary priority**: (1) legal structure boundary (clause, Q&A pair, section), (2) paragraph break, (3) sentence boundary, (4) word boundary. Never split mid-word. Chunks do NOT cross page boundaries.

### 7.2 Legal Chunking Anti-Patterns (FORBIDDEN)

```
LEGAL CHUNKING ANTI-PATTERNS -- NEVER DO THESE
=================================================================================

AP-LEGAL-01: NEVER split a contract clause across chunks.
  A clause like "Section 4.2(a) Indemnification..." is ONE unit.
  If a clause exceeds max_chars, it becomes its own oversized chunk
  rather than being split mid-clause.

AP-LEGAL-02: NEVER split a deposition Q&A pair.
  "Q. Did you see the defendant on January 5th?
   A. Yes, I saw him at the office around 3pm."
  This is ONE chunk. The answer is meaningless without its question.

AP-LEGAL-03: NEVER split a statute subsection.
  "§ 1291(a)(1) The courts of appeals shall have jurisdiction..."
  is ONE unit. Splitting mid-subsection destroys legal meaning.

AP-LEGAL-04: NEVER split a court opinion's holding from its reasoning.
  If they fit together under max_chars, keep them in one chunk.

AP-LEGAL-05: NEVER chunk legal documents by flat character count alone.
  Always use the legal-aware chunker first. Fall back to default
  ONLY for unrecognized document types.
```

### 7.3 Contract Chunking

```rust
/// Chunks contracts by clause/section boundaries
pub struct ContractChunker {
    min_chars: usize,   // 400
    max_chars: usize,   // 2500
}

impl ContractChunker {
    pub fn chunk(&self, doc: &ParsedDocument) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut chunk_seq: u32 = 0;

        for page in &doc.pages {
            let sections = self.detect_clauses(&page.content);

            let mut current_text = String::new();
            let mut current_section_label = String::new();

            for section in &sections {
                // If adding this clause would exceed max, emit current chunk
                if current_text.len() + section.text.len() > self.max_chars
                    && current_text.len() >= self.min_chars
                {
                    chunks.push(self.make_chunk(
                        doc, page, &current_text, chunk_seq,
                        &current_section_label,
                    ));
                    chunk_seq += 1;
                    current_text.clear();
                }

                // A single clause exceeding max gets its own chunk (never split)
                if section.text.len() > self.max_chars && current_text.is_empty() {
                    chunks.push(self.make_chunk(
                        doc, page, &section.text, chunk_seq,
                        &section.label,
                    ));
                    chunk_seq += 1;
                    continue;
                }

                if current_text.is_empty() {
                    current_section_label = section.label.clone();
                }
                current_text.push_str(&section.text);
                current_text.push('\n');
            }

            if current_text.len() >= self.min_chars {
                chunks.push(self.make_chunk(
                    doc, page, &current_text, chunk_seq,
                    &current_section_label,
                ));
                chunk_seq += 1;
            }
        }

        chunks
    }

    /// Detect clause boundaries: "Section 1.2", "Article III", "1.2(a)", etc.
    fn detect_clauses(&self, content: &str) -> Vec<ClauseSection> {
        // Regex patterns for common legal clause numbering:
        //   Section \d+(\.\d+)*
        //   Article [IVX]+
        //   \d+\.\d+(\([a-z]\))?
        //   RECITALS, WHEREAS, NOW THEREFORE
        // Split content at these boundaries, preserving the heading with its body
        todo!()
    }
}
```

### 7.4 Deposition Chunking

```rust
/// Chunks depositions by Q&A pairs -- NEVER split a Q&A pair
pub struct DepositionChunker {
    min_chars: usize,   // 200 (some Q&A pairs are short)
    max_chars: usize,   // 3000 (long answers are kept whole)
}

impl DepositionChunker {
    pub fn chunk(&self, doc: &ParsedDocument) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut chunk_seq: u32 = 0;

        for page in &doc.pages {
            let qa_pairs = self.detect_qa_pairs(&page.content);

            let mut current_text = String::new();
            let mut current_qa_start: u32 = 0;

            for (qa_idx, qa) in qa_pairs.iter().enumerate() {
                let pair_text = format!("Q. {}\nA. {}\n", qa.question, qa.answer);

                // If adding this Q&A pair would exceed max, emit current chunk
                if current_text.len() + pair_text.len() > self.max_chars
                    && current_text.len() >= self.min_chars
                {
                    chunks.push(self.make_chunk(
                        doc, page, &current_text, chunk_seq,
                        current_qa_start, qa_idx as u32 - 1,
                    ));
                    chunk_seq += 1;
                    current_text.clear();
                    current_qa_start = qa_idx as u32;
                }

                current_text.push_str(&pair_text);
            }

            if current_text.len() >= self.min_chars {
                chunks.push(self.make_chunk(
                    doc, page, &current_text, chunk_seq,
                    current_qa_start, qa_pairs.len() as u32 - 1,
                ));
                chunk_seq += 1;
            }
        }

        chunks
    }

    /// Detect Q&A boundaries: lines starting with "Q." or "Q:" followed by "A." or "A:"
    fn detect_qa_pairs(&self, content: &str) -> Vec<QaPair> {
        // Pattern: "Q." or "Q:" at line start, followed by text until next "A." or "A:"
        // The answer continues until the next "Q." or end of content
        todo!()
    }
}
```

### 7.5 Default Chunking (Paragraph-Aware 2000-char)

For unrecognized document types, CaseTrack falls back to the paragraph-aware chunking strategy.

| Parameter | Value |
|-----------|-------|
| Target size | 2000 characters |
| Overlap | 10% = 200 characters (from end of previous chunk) |
| Min size | 400 characters (no tiny fragments) |
| Max size | 2200 characters (small overrun to avoid mid-sentence splits) |

### 7.6 Provenance Per Chunk (MANDATORY)

**Every chunk MUST store its complete provenance at creation time.** Fields: `document_id`, `document_name`, `document_path`, `page`, `paragraph_start/end`, `line_start/end`, `char_start/end`, `extraction_method`, `ocr_confidence`, `sheet_name` (spreadsheets), `row_range` (spreadsheets), `column_range` (spreadsheets), `legal_section` (legal document types), `chunk_index`.

Provenance is: (1) stored in RocksDB with chunk text and embeddings, (2) returned in every search result, (3) queryable via MCP tools, (4) immutable after creation. See [PRD 04 Section 5.2](PRD_04_STORAGE_ARCHITECTURE.md) for the canonical Provenance struct and storage layout.

### 7.7 Default Chunking Implementation

```rust
/// Chunker configuration: 2000 chars, 10% overlap
pub struct DocumentChunker {
    target_chars: usize,   // 2000
    max_chars: usize,      // 2200 (small overrun to avoid mid-sentence)
    min_chars: usize,      // 400 (don't emit tiny fragments)
    overlap_chars: usize,  // 200 (10% of target)
}

impl Default for DocumentChunker {
    fn default() -> Self {
        Self {
            target_chars: 2000,
            max_chars: 2200,
            min_chars: 400,
            overlap_chars: 200,  // 10% overlap
        }
    }
}

impl DocumentChunker {
    pub fn chunk(&self, doc: &ParsedDocument) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut chunk_seq: u32 = 0;

        for page in &doc.pages {
            if page.content.trim().is_empty() {
                continue;
            }

            // Track character offset within the page
            let mut page_char_offset: u64 = 0;

            let paragraphs = &page.paragraphs;
            let mut current_text = String::new();
            let mut current_start_para: u32 = 0;
            let mut current_start_line: u32 = 0;
            let mut current_char_start: u64 = 0;

            for (para_idx, paragraph) in paragraphs.iter().enumerate() {
                let para_chars = paragraph.text.len();

                // Single paragraph exceeds max? Split by sentence
                if para_chars > self.max_chars {
                    // Flush current chunk first
                    if current_text.len() >= self.min_chars {
                        chunks.push(self.make_chunk(
                            doc, page, &current_text, chunk_seq,
                            current_start_para, para_idx.saturating_sub(1) as u32,
                            current_start_line,
                            current_char_start,
                        ));
                        chunk_seq += 1;
                    }

                    // Split long paragraph by sentences
                    let sub_chunks = self.split_long_paragraph(
                        doc, page, paragraph, para_idx as u32,
                        page_char_offset, &mut chunk_seq,
                    );
                    chunks.extend(sub_chunks);

                    page_char_offset += para_chars as u64;
                    current_text.clear();
                    current_start_para = (para_idx + 1) as u32;
                    current_char_start = page_char_offset;
                    continue;
                }

                // Would adding this paragraph exceed 2000 chars?
                if current_text.len() + para_chars > self.target_chars
                    && current_text.len() >= self.min_chars
                {
                    // Emit current chunk
                    chunks.push(self.make_chunk(
                        doc, page, &current_text, chunk_seq,
                        current_start_para, para_idx.saturating_sub(1) as u32,
                        current_start_line,
                        current_char_start,
                    ));
                    chunk_seq += 1;

                    // Start new chunk with 200-char overlap from end of previous
                    let overlap = self.compute_overlap(&current_text);
                    current_text = overlap;
                    current_start_para = para_idx as u32;
                    current_char_start = page_char_offset.saturating_sub(self.overlap_chars as u64);
                }

                current_text.push_str(&paragraph.text);
                current_text.push('\n');
                page_char_offset += para_chars as u64 + 1; // +1 for newline
            }

            // Emit remaining text for this page
            if current_text.len() >= self.min_chars {
                chunks.push(self.make_chunk(
                    doc, page, &current_text, chunk_seq,
                    current_start_para, paragraphs.len().saturating_sub(1) as u32,
                    current_start_line,
                    current_char_start,
                ));
                chunk_seq += 1;
            }
        }

        chunks
    }

    fn make_chunk(
        &self,
        doc: &ParsedDocument,
        page: &Page,
        text: &str,
        sequence: u32,
        para_start: u32,
        para_end: u32,
        line_start: u32,
        char_start: u64,
    ) -> Chunk {
        let line_end = line_start + text.lines().count() as u32;
        let char_end = char_start + text.len() as u64;

        Chunk {
            id: Uuid::new_v4(),
            document_id: doc.id,
            text: text.to_string(),
            sequence,
            char_count: text.len() as u32,
            provenance: Provenance {
                document_id: doc.id,
                document_name: doc.filename.clone(),
                document_path: doc.original_path.clone(),
                page: page.number,
                paragraph_start: para_start,
                paragraph_end: para_end,
                line_start,
                line_end,
                char_start,
                char_end,
                extraction_method: page.extraction_method,
                ocr_confidence: page.ocr_confidence,
                chunk_index: sequence,
            },
        }
    }

    /// Take last 200 characters as overlap for next chunk
    fn compute_overlap(&self, text: &str) -> String {
        if text.len() <= self.overlap_chars {
            return text.to_string();
        }
        let start = text.len() - self.overlap_chars;
        // Find nearest word boundary after the cut point
        let boundary = text[start..].find(' ').map(|i| start + i + 1).unwrap_or(start);
        text[boundary..].to_string()
    }

    /// Split a paragraph longer than 2200 chars into sentence-bounded chunks
    fn split_long_paragraph(
        &self,
        doc: &ParsedDocument,
        page: &Page,
        paragraph: &Paragraph,
        para_idx: u32,
        char_offset: u64,
        chunk_seq: &mut u32,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let sentences = split_sentences(&paragraph.text);
        let mut current = String::new();
        let mut local_offset = 0u64;

        for sentence in &sentences {
            if current.len() + sentence.len() > self.target_chars && current.len() >= self.min_chars {
                chunks.push(self.make_chunk(
                    doc, page, &current, *chunk_seq,
                    para_idx, para_idx,
                    0, // line tracking within paragraph
                    char_offset + local_offset,
                ));
                *chunk_seq += 1;

                let overlap = self.compute_overlap(&current);
                local_offset += (current.len() - overlap.len()) as u64;
                current = overlap;
            }
            current.push_str(sentence);
        }

        if current.len() >= self.min_chars {
            chunks.push(self.make_chunk(
                doc, page, &current, *chunk_seq,
                para_idx, para_idx,
                0,
                char_offset + local_offset,
            ));
            *chunk_seq += 1;
        }

        chunks
    }
}

/// Split text into sentences (period/question/exclamation + space + uppercase)
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if (ch == '.' || ch == '?' || ch == '!') {
            // Check if next char is space + uppercase (sentence boundary)
            // Simplified: just split at sentence-ending punctuation
            sentences.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        sentences.push(current);
    }
    sentences
}
```

---

## 8. Legal Entity Extraction

### 8.1 Entity Types Extracted

| Type | Detection Method | Examples |
|------|-----------------|----------|
| Party | NER + patterns | "Smith", "Jones Corp", "Plaintiff Smith" |
| Court | NER + patterns | "S.D.N.Y.", "Ninth Circuit", "Supreme Court of California" |
| Judge | NER + patterns | "Hon. Jane Smith", "Judge Martinez", "J. Roberts" |
| Attorney | NER + patterns | "John Doe, Esq.", "Attorney Chen" |
| Statute | Regex | "42 U.S.C. § 1983", "Cal. Civ. Code § 1714" |
| CaseNumber | Regex | "2:24-cv-01234", "No. 23-1456" |
| Jurisdiction | NER + patterns | "Southern District of New York", "State of California" |
| LegalConcept | NER | "breach of fiduciary duty", "negligence per se" |
| Remedy | NER + patterns | "injunctive relief", "compensatory damages" |
| Witness | NER + context | "Witness testified", "deponent stated" |
| Exhibit | Regex + patterns | "Exhibit A", "Ex. 12", "Plaintiff's Exhibit 3" |
| DocketEntry | Regex | "Dkt. No. 45", "ECF No. 12" |
| Date | Regex + NER | "January 15, 2024", "filed on 01/15/2024" |
| Amount | Regex | "$1,250,000.00", "1.25 million dollars", "15.7%" |
| Person | NER | "John Smith", "Sarah Chen" |
| Organization | NER | "Acme Corp", "Department of Justice" |
| Location | NER | "New York, NY", "123 Main Street" |

### 8.2 Knowledge Graph Integration During Ingestion

After entity extraction, the pipeline builds knowledge graph connections:

| Step | Description |
|------|-------------|
| Entity-to-Chunk edges | Each extracted entity links to the chunk(s) where it appears |
| Cross-document entity matching | Same entity (e.g., "Acme Corp") across multiple documents creates shared-entity edges |
| Document relationship edges | Documents sharing 3+ entities are linked with SharedEntities relationship |
| Entity co-occurrence | Entities appearing in the same chunk are linked with co-occurrence edges |
| Citation graph edges | Documents citing the same authority (case, statute) are linked |

---

## 9. Legal Citation Extraction

### 9.1 Citation Types and Patterns

CaseTrack detects Bluebook-format legal citations and indexes them for cross-referencing across the case file.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LegalCitationType {
    CaseCitation,       // Smith v. Jones, 123 F.3d 456 (9th Cir. 2024)
    StatuteCitation,    // 42 U.S.C. § 1983
    RegulationCitation, // 29 C.F.R. § 1630.2
    ShortForm,          // Id. at 459; supra note 5; infra Part III
    ConstitutionCitation, // U.S. Const. amend. XIV, § 1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalCitation {
    pub id: Uuid,
    pub citation_type: LegalCitationType,
    pub full_text: String,          // "Smith v. Jones, 123 F.3d 456, 459 (9th Cir. 2024)"
    pub normalized: String,         // Canonical form for deduplication
    pub parties: Option<String>,    // "Smith v. Jones" (for case citations)
    pub volume: Option<String>,     // "123"
    pub reporter: Option<String>,   // "F.3d"
    pub page: Option<String>,       // "456"
    pub pinpoint: Option<String>,   // "459" (specific page within the case)
    pub court: Option<String>,      // "9th Cir."
    pub year: Option<String>,       // "2024"
    pub title: Option<String>,      // "42" (for statute title)
    pub code: Option<String>,       // "U.S.C." (for statute code)
    pub section: Option<String>,    // "1983" (for statute section)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationMention {
    pub citation_id: Uuid,
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub char_start: u64,
    pub char_end: u64,
}
```

### 9.2 Citation Detection Patterns

```rust
/// Regex patterns for Bluebook citation detection
pub struct CitationExtractor {
    case_pattern: Regex,
    statute_pattern: Regex,
    regulation_pattern: Regex,
    short_form_pattern: Regex,
    constitution_pattern: Regex,
}

impl CitationExtractor {
    pub fn new() -> Self {
        Self {
            // Case citations: Party v. Party, Vol. Reporter Page (Court Year)
            // Examples: "Smith v. Jones, 123 F.3d 456 (9th Cir. 2024)"
            //           "Brown v. Board of Education, 347 U.S. 483 (1954)"
            case_pattern: Regex::new(
                r"([A-Z][a-zA-Z'.]+(?:\s+[A-Z][a-zA-Z'.]+)*)\s+v\.\s+([A-Z][a-zA-Z'.]+(?:\s+[A-Z][a-zA-Z'.]+)*),\s+(\d+)\s+([A-Z][a-zA-Z.]+(?:\s+\d[a-z]{1,2})?)\s+(\d+)(?:,\s+(\d+))?\s+\(([^)]+)\s+(\d{4})\)"
            ).unwrap(),

            // Statute citations: Title U.S.C. § Section
            // Examples: "42 U.S.C. § 1983", "15 U.S.C. §§ 1-7"
            statute_pattern: Regex::new(
                r"(\d+)\s+(U\.S\.C\.|U\.S\.C\.A\.|[A-Z][a-z]+\.\s+(?:Civ\.|Crim\.|Pen\.|Bus\.\s+&\s+Prof\.|Gov(?:'t|t)?\.?)\s+Code)\s+§§?\s+([\d]+(?:\([a-z]\)(?:\(\d+\))?)?)"
            ).unwrap(),

            // Regulation citations: Title C.F.R. § Section
            // Examples: "29 C.F.R. § 1630.2", "17 C.F.R. § 240.10b-5"
            regulation_pattern: Regex::new(
                r"(\d+)\s+C\.F\.R\.\s+§§?\s+([\d]+(?:\.[\d]+)?(?:-[\d]+)?)"
            ).unwrap(),

            // Short-form citations
            // Examples: "Id.", "Id. at 459", "supra note 5", "infra Part III"
            short_form_pattern: Regex::new(
                r"(?:Id\.(?:\s+at\s+\d+)?|[Ss]upra\s+(?:note\s+\d+|at\s+\d+)|[Ii]nfra\s+(?:Part\s+[IVX]+|at\s+\d+|note\s+\d+))"
            ).unwrap(),

            // Constitution citations
            // Examples: "U.S. Const. amend. XIV, § 1", "U.S. Const. art. III, § 2"
            constitution_pattern: Regex::new(
                r"U\.S\.\s+Const\.\s+(?:amend\.|art\.)\s+([IVX]+|\d+)(?:,\s+§\s+(\d+))?"
            ).unwrap(),
        }
    }

    pub fn extract(&self, text: &str) -> Vec<LegalCitation> {
        let mut citations = Vec::new();

        // Extract case citations
        for cap in self.case_pattern.captures_iter(text) {
            citations.push(LegalCitation {
                id: Uuid::new_v4(),
                citation_type: LegalCitationType::CaseCitation,
                full_text: cap[0].to_string(),
                normalized: self.normalize_case_citation(&cap),
                parties: Some(format!("{} v. {}", &cap[1], &cap[2])),
                volume: Some(cap[3].to_string()),
                reporter: Some(cap[4].to_string()),
                page: Some(cap[5].to_string()),
                pinpoint: cap.get(6).map(|m| m.as_str().to_string()),
                court: Some(cap[7].to_string()),
                year: Some(cap[8].to_string()),
                title: None,
                code: None,
                section: None,
            });
        }

        // Extract statute citations
        for cap in self.statute_pattern.captures_iter(text) {
            citations.push(LegalCitation {
                id: Uuid::new_v4(),
                citation_type: LegalCitationType::StatuteCitation,
                full_text: cap[0].to_string(),
                normalized: self.normalize_statute_citation(&cap),
                parties: None,
                volume: None,
                reporter: None,
                page: None,
                pinpoint: None,
                court: None,
                year: None,
                title: Some(cap[1].to_string()),
                code: Some(cap[2].to_string()),
                section: Some(cap[3].to_string()),
            });
        }

        // Extract regulation citations
        for cap in self.regulation_pattern.captures_iter(text) {
            citations.push(LegalCitation {
                id: Uuid::new_v4(),
                citation_type: LegalCitationType::RegulationCitation,
                full_text: cap[0].to_string(),
                normalized: format!("{} C.F.R. § {}", &cap[1], &cap[2]),
                parties: None,
                volume: None,
                reporter: None,
                page: None,
                pinpoint: None,
                court: None,
                year: None,
                title: Some(cap[1].to_string()),
                code: Some("C.F.R.".to_string()),
                section: Some(cap[2].to_string()),
            });
        }

        // Extract short-form citations
        for cap in self.short_form_pattern.captures_iter(text) {
            citations.push(LegalCitation {
                id: Uuid::new_v4(),
                citation_type: LegalCitationType::ShortForm,
                full_text: cap[0].to_string(),
                normalized: cap[0].to_string(),
                parties: None, volume: None, reporter: None, page: None,
                pinpoint: None, court: None, year: None,
                title: None, code: None, section: None,
            });
        }

        citations
    }
}
```

---

## 10. Batch Ingestion

```rust
/// Ingest all supported files in a directory
pub async fn ingest_folder(
    case: &mut CaseHandle,
    engine: &EmbeddingEngine,
    folder: &Path,
    recursive: bool,
) -> Result<BatchIngestResult> {
    let files = discover_files(folder, recursive)?;
    let total = files.len();
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (idx, file) in files.iter().enumerate() {
        tracing::info!("[{}/{}] Ingesting: {}", idx + 1, total, file.display());

        match ingest_single_file(case, engine, file).await {
            Ok(result) => results.push(result),
            Err(e) => {
                tracing::error!("Failed to ingest {}: {}", file.display(), e);
                errors.push(IngestError {
                    file: file.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(BatchIngestResult {
        total_files: total,
        succeeded: results.len(),
        failed: errors.len(),
        results,
        errors,
    })
}

fn discover_files(folder: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let supported = &[
        "pdf", "docx", "doc", "xlsx", "xls", "ods",
        "eml",
        "txt", "rtf", "jpg", "jpeg", "png", "tiff", "tif",
    ];

    let walker = if recursive {
        walkdir::WalkDir::new(folder)
    } else {
        walkdir::WalkDir::new(folder).max_depth(1)
    };

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if supported.contains(&ext.to_lowercase().as_str()) {
                    files.push(entry.into_path());
                }
            }
        }
    }

    files.sort(); // Deterministic order
    Ok(files)
}
```

---

## 11. Duplicate Detection

Check SHA256 hash against existing documents before ingesting. If duplicate found, return error with existing document ID and `--force` hint.

```rust
pub fn check_duplicate(case: &CaseHandle, file_hash: &str) -> Result<Option<Uuid>> {
    let cf = case.db.cf_handle("documents").unwrap();
    for item in case.db.iterator_cf(&cf, rocksdb::IteratorMode::Start) {
        let (_, value) = item?;
        let doc: DocumentMetadata = bincode::deserialize(&value)?;
        if doc.file_hash == file_hash {
            return Ok(Some(doc.id));
        }
    }
    Ok(None)
}
```

---

## 12. Data Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub id: Uuid,
    pub filename: String,
    pub pages: Vec<Page>,
    pub metadata: DocumentMetadataRaw,
    pub file_hash: String,
    pub attachments: Vec<Attachment>,  // Email attachments
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub number: u32,
    pub content: String,
    pub paragraphs: Vec<Paragraph>,
    pub extraction_method: ExtractionMethod,
    pub ocr_confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub index: usize,
    pub text: String,
    pub style: ParagraphStyle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ParagraphStyle {
    Body,
    Heading,
    ListItem,
    BlockQuote,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExtractionMethod {
    Native,       // Direct text extraction from PDF/DOCX
    Ocr,          // Tesseract OCR
    Spreadsheet,  // calamine spreadsheet extraction
    Email,        // mailparse email extraction
    Skipped,      // OCR disabled, scanned page skipped
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub text: String,
    pub sequence: u32,        // Position within document
    pub char_count: u32,      // Length in characters (target: 2000)
    pub provenance: Provenance, // FULL source location (MANDATORY)
}

#[derive(Debug, Serialize)]
pub struct IngestResult {
    pub document_id: Uuid,
    pub document_name: String,
    pub page_count: u32,
    pub chunk_count: u32,
    pub entity_count: u32,
    pub citation_count: u32,
    pub extraction_method: ExtractionMethod,
    pub ocr_pages: u32,
    pub duration_ms: u64,
}
```

---

*CaseTrack PRD v5.1.0 -- Document 6 of 10*

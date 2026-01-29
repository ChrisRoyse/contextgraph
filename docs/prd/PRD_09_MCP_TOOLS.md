# PRD 09: MCP Tools

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Tool Overview

| Tool | Description | Tier | Requires Active Case |
|------|-------------|------|---------------------|
| `create_case` | Create a new case | Free | No |
| `list_cases` | List all cases | Free | No |
| `switch_case` | Switch active case | Free | No |
| `delete_case` | Delete a case and all its data | Free | No |
| `get_case_info` | Get details about active case | Free | Yes |
| `ingest_document` | Ingest a PDF, DOCX, or image | Free | Yes |
| `ingest_folder` | Batch ingest all files in folder | Pro | Yes |
| `list_documents` | List documents in active case | Free | Yes |
| `get_document` | Get document details and stats | Free | Yes |
| `delete_document` | Remove a document from a case | Free | Yes |
| `search_case` | Search across all documents | Free (limited) | Yes |
| `find_entity` | Find mentions of a legal entity | Pro | Yes |
| `get_chunk` | Get a specific chunk with full provenance | Free | Yes |
| `get_document_chunks` | List all chunks in a document with provenance | Free | Yes |
| `get_source_context` | Get surrounding text for a chunk (context window) | Free | Yes |
| `get_status` | Get server status and model info | Free | No |

---

## 2. Tool Specifications

### 2.1 `create_case`

```json
{
  "name": "create_case",
  "description": "Create a new legal case. Creates an isolated database for this case on your machine. Automatically switches to the new case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "name": {
        "type": "string",
        "description": "Case name (e.g., 'Smith v. Jones')"
      },
      "case_number": {
        "type": "string",
        "description": "Optional docket or case number"
      },
      "case_type": {
        "type": "string",
        "enum": ["civil", "criminal", "family", "bankruptcy", "contract", "employment", "personal_injury", "real_estate", "intellectual_property", "immigration", "other"],
        "description": "Type of legal case"
      }
    },
    "required": ["name"]
  }
}
```

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Created case \"Smith v. Jones Corp\" (ID: a1b2c3d4).\nType: Contract\nThis is now your active case.\n\nNext: Ingest documents with ingest_document."
  }]
}
```

**Error (Free tier limit):**
```json
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Free tier allows 3 cases (you have 3). Delete a case or upgrade to Pro for unlimited cases: https://casetrack.legal/upgrade"
  }]
}
```

---

### 2.2 `list_cases`

```json
{
  "name": "list_cases",
  "description": "List all cases. Shows name, type, status, document count, and which case is active.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "status_filter": {
        "type": "string",
        "enum": ["active", "closed", "archived", "all"],
        "default": "active",
        "description": "Filter by case status"
      }
    }
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Cases (2 active):\n\n* Smith v. Jones Corp [ACTIVE] <-- current\n  Contract | 5 documents | 234 chunks | Created 2026-01-15\n\n  Doe v. State [ACTIVE]\n  Criminal | 12 documents | 890 chunks | Created 2026-01-20"
  }]
}
```

---

### 2.3 `switch_case`

```json
{
  "name": "switch_case",
  "description": "Switch to a different case. All subsequent operations (ingest, search) will use this case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "case_name": {
        "type": "string",
        "description": "Case name or ID to switch to"
      }
    },
    "required": ["case_name"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Switched to case \"Doe v. State\" (12 documents, 890 chunks)."
  }]
}
```

**Error (not found):**
```json
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Case not found: \"Smith\". Did you mean:\n  - Smith v. Jones Corp (ID: a1b2c3d4)\nUse the full name or ID."
  }]
}
```

---

### 2.4 `delete_case`

```json
{
  "name": "delete_case",
  "description": "Permanently delete a case and all its documents, embeddings, and data. This cannot be undone.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "case_name": {
        "type": "string",
        "description": "Case name or ID to delete"
      },
      "confirm": {
        "type": "boolean",
        "description": "Must be true to confirm deletion",
        "default": false
      }
    },
    "required": ["case_name", "confirm"]
  }
}
```

**Error (no confirmation):**
```json
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Deletion requires confirm=true. This will permanently delete case \"Smith v. Jones Corp\" and all 5 documents. This cannot be undone."
  }]
}
```

---

### 2.5 `get_case_info`

```json
{
  "name": "get_case_info",
  "description": "Get detailed information about the active case including document list and storage usage.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Case: Smith v. Jones Corp\nType: Contract | Status: Active\nCreated: 2026-01-15\n\nDocuments (5):\n  1. Complaint.pdf - 45 pages, 234 chunks (Native extraction)\n  2. Contract.pdf - 28 pages, 156 chunks (Native extraction)\n  3. Exhibit_A.jpg - 1 page, 3 chunks (OCR, 97% confidence)\n  4. Deposition.docx - 120 pages, 580 chunks (Native extraction)\n  5. Motion.pdf - 15 pages, 78 chunks (Native extraction)\n\nTotal: 209 pages, 1,051 chunks\nStorage: 52 MB (embeddings + index)\nEmbedders: E1-Legal, E6-Legal, E7, E13-BM25 (Free tier)"
  }]
}
```

---

### 2.6 `ingest_document`

```json
{
  "name": "ingest_document",
  "description": "Ingest a document (PDF, DOCX, or image) into the active case. Extracts text (with OCR for scans), chunks the text, computes embeddings, and indexes for search. All processing and storage happens locally on your machine.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "file_path": {
        "type": "string",
        "description": "Absolute path to the file on your computer"
      },
      "document_name": {
        "type": "string",
        "description": "Optional display name (defaults to filename)"
      },
      "document_type": {
        "type": "string",
        "enum": ["pleading", "motion", "brief", "contract", "exhibit", "correspondence", "deposition", "discovery", "statute", "case_law", "other"],
        "description": "Type of legal document"
      },
      "copy_original": {
        "type": "boolean",
        "default": false,
        "description": "Copy the original file into the case folder"
      }
    },
    "required": ["file_path"]
  }
}
```

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Ingested \"Complaint.pdf\" into Smith v. Jones Corp\n\n  Pages:      45\n  Chunks:     234\n  Extraction: Native text\n  Embedders:  E1-Legal, E6-Legal, E7, BM25\n  Duration:   12.3 seconds\n  Storage:    3.2 MB\n\nThis document is now searchable."
  }]
}
```

**Error (no active case):**
```json
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "No active case. Create or switch to a case first:\n  - create_case: Create a new case\n  - switch_case: Switch to an existing case\n  - list_cases: See all cases"
  }]
}
```

**Error (file not found):**
```json
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "File not found: /Users/sarah/Downloads/Complaint.pdf\n\nCheck that the path is correct and the file exists."
  }]
}
```

**Error (unsupported format):**
```json
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Unsupported file format: .xlsx\n\nSupported formats: PDF, DOCX, DOC, TXT, RTF, JPG, PNG, TIFF"
  }]
}
```

---

### 2.7 `ingest_folder`

```json
{
  "name": "ingest_folder",
  "description": "Batch ingest all supported documents in a folder. Pro tier only.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "folder_path": {
        "type": "string",
        "description": "Path to folder containing documents"
      },
      "recursive": {
        "type": "boolean",
        "default": false,
        "description": "Include subfolders"
      },
      "document_type": {
        "type": "string",
        "enum": ["pleading", "motion", "brief", "contract", "exhibit", "correspondence", "deposition", "discovery", "other"],
        "description": "Default type for all documents"
      }
    },
    "required": ["folder_path"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Batch ingestion complete for Smith v. Jones Corp\n\n  Folder:    ~/Cases/Smith/Documents/\n  Found:     23 supported files\n  Succeeded: 21\n  Failed:    2\n  Duration:  3 minutes 45 seconds\n\n  Failures:\n  - corrupted.pdf: PDF parsing error (file may be corrupted)\n  - scan_2019.tiff: OCR failed (image too low resolution)"
  }]
}
```

---

### 2.8 `list_documents`

```json
{
  "name": "list_documents",
  "description": "List all documents in the active case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "sort_by": {
        "type": "string",
        "enum": ["name", "date", "pages", "type"],
        "default": "date",
        "description": "Sort order"
      }
    }
  }
}
```

---

### 2.9 `get_document`

```json
{
  "name": "get_document",
  "description": "Get detailed information about a specific document including page count, extraction method, and chunk statistics.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID"
      }
    },
    "required": ["document_name"]
  }
}
```

---

### 2.10 `delete_document`

```json
{
  "name": "delete_document",
  "description": "Remove a document and all its chunks, embeddings, and index entries from the active case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID to delete"
      },
      "confirm": {
        "type": "boolean",
        "default": false,
        "description": "Must be true to confirm deletion"
      }
    },
    "required": ["document_name", "confirm"]
  }
}
```

---

### 2.11 `search_case`

```json
{
  "name": "search_case",
  "description": "Search across all documents in the active case using semantic and keyword search. Returns results with full source citations (document, page, paragraph, line).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Natural language search query (e.g., 'What are the termination provisions?')"
      },
      "top_k": {
        "type": "integer",
        "default": 10,
        "minimum": 1,
        "maximum": 50,
        "description": "Number of results to return"
      },
      "document_filter": {
        "type": "string",
        "description": "Optional: restrict search to a specific document name or ID"
      }
    },
    "required": ["query"]
  }
}
```

**Full Response Example:**
```json
{
  "content": [{
    "type": "text",
    "text": "Search: \"early termination clause\"\nCase: Smith v. Jones Corp | 5 documents, 1,051 chunks searched\nTime: 87ms | Tier: Pro (4-stage pipeline)\n\n--- Result 1 (score: 0.94) ---\nContract.pdf, p. 12, para. 8, ll. 1-4\n\n\"Either party may terminate this Agreement upon thirty (30) days written notice to the other party. In the event of material breach, the non-breaching party may terminate immediately upon written notice specifying the breach.\"\n\n--- Result 2 (score: 0.89) ---\nContract.pdf, p. 13, para. 10, ll. 1-6\n\n\"In the event of early termination, the non-breaching party shall be entitled to recover all damages, including but not limited to lost profits, reasonable attorney's fees, and costs of enforcement.\"\n\n--- Result 3 (score: 0.76) ---\nComplaint.pdf, p. 8, para. 22, ll. 3-5\n\n\"Defendant terminated the Agreement without the required thirty days notice, in direct violation of Section 8.1 of the Agreement.\""
  }]
}
```

---

### 2.12 `find_entity`

```json
{
  "name": "find_entity",
  "description": "Find all mentions of a legal entity (person, court, statute, case citation) across documents. Pro tier only. Uses E11-LEGAL entity embedder.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity": {
        "type": "string",
        "description": "Entity to find (e.g., 'Judge Smith', '42 USC 1983', 'Miranda v. Arizona')"
      },
      "entity_type": {
        "type": "string",
        "enum": ["person", "court", "statute", "case_citation", "organization", "any"],
        "default": "any",
        "description": "Type of entity to search for"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 100
      }
    },
    "required": ["entity"]
  }
}
```

---

### 2.13 `get_status`

```json
{
  "name": "get_status",
  "description": "Get CaseTrack server status including version, license tier, loaded models, and storage usage.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "CaseTrack v1.0.0\n\nLicense: Pro\nActive Case: Smith v. Jones Corp\n\nModels Loaded:\n  E1-Legal (bge-small-en-v1.5): Ready\n  E6-Legal (SPLADE): Ready\n  E7 (MiniLM-L6): Ready\n  E8-Legal (Citation): Ready\n  E11-Legal (Entity): Ready\n  E12 (ColBERT): Ready\n  E13 (BM25): Ready (algorithmic)\n\nStorage: ~/Documents/CaseTrack/\n  Models: 370 MB\n  Cases: 2 (52 MB total)\n  Total: 422 MB\n\nSystem:\n  RAM: 1.4 GB used / 16 GB available\n  OS: macOS 14.2 (Apple M2)\n  Memory Mode: Full"
  }]
}
```

### 2.14 `get_chunk`

```json
{
  "name": "get_chunk",
  "description": "Get a specific chunk by ID with its full text, provenance (source file, page, paragraph, line, character offsets), and embedding status.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "chunk_id": {
        "type": "string",
        "description": "UUID of the chunk"
      }
    },
    "required": ["chunk_id"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Chunk abc-123 (2000 chars)\n\nText:\n\"Either party may terminate this Agreement upon thirty (30) days written notice to the other party...\"\n\nProvenance:\n  Document:   Contract.pdf\n  File Path:  /Users/sarah/Cases/Smith/Contract.pdf\n  Page:       12\n  Paragraphs: 8-9\n  Lines:      1-14\n  Chars:      2401-4401 (within page)\n  Extraction: Native text\n  Chunk Index: 47 of 234\n\nEmbeddings: E1-Legal, E6-Legal, E7, E8-Legal, E11-Legal, E12"
  }]
}
```

---

### 2.15 `get_document_chunks`

```json
{
  "name": "get_document_chunks",
  "description": "List all chunks in a document with their provenance. Shows where every piece of text came from: page, paragraph, line numbers, and character offsets. Use this to understand how a document was chunked and indexed.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID"
      },
      "page_filter": {
        "type": "integer",
        "description": "Optional: only show chunks from this page number"
      }
    },
    "required": ["document_name"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Document: Contract.pdf (28 pages, 156 chunks)\nFile Path: /Users/sarah/Cases/Smith/Contract.pdf\n\nChunk 0: p.1, paras 0-3, ll.1-12, chars 0-1987\n  \"AGREEMENT made this 15th day of January 2024 between...\"\n\nChunk 1: p.1, paras 3-6, ll.10-22, chars 1788-3790\n  \"...hereinafter referred to as 'Contractor'. WHEREAS the parties...\"\n  [200-char overlap from chunk 0]\n\nChunk 2: p.2, paras 0-2, ll.1-8, chars 0-1834\n  \"Section 2. SCOPE OF WORK. The Contractor shall provide...\"\n\n... (156 total chunks)"
  }]
}
```

---

### 2.16 `get_source_context`

```json
{
  "name": "get_source_context",
  "description": "Get the surrounding context for a chunk -- the chunks immediately before and after it in the original document. Useful for understanding the full context around a search result.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "chunk_id": {
        "type": "string",
        "description": "UUID of the chunk to get context for"
      },
      "window": {
        "type": "integer",
        "default": 1,
        "minimum": 1,
        "maximum": 5,
        "description": "Number of chunks before and after to include"
      }
    },
    "required": ["chunk_id"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Context for chunk abc-123 (Contract.pdf, p.12)\n\n--- BEFORE (chunk 46, p.12, paras 6-7) ---\n\"The parties agree that in the event of any dispute arising under this Agreement, they shall first attempt to resolve the dispute through good faith negotiation...\"\n\n--- TARGET (chunk 47, p.12, paras 8-9) ---\n\"Either party may terminate this Agreement upon thirty (30) days written notice to the other party. In the event of material breach, the non-breaching party may terminate immediately upon written notice specifying the breach...\"\n\n--- AFTER (chunk 48, p.13, paras 0-1) ---\n\"Upon termination, all confidential information shall be returned or destroyed within fourteen (14) days. The obligations under Sections 5, 7, and 9 shall survive termination...\""
  }]
}
```

---

## 3. MCP Server Lifecycle

### 3.1 Server Initialization

```rust
use rmcp::{ServerBuilder, ServerHandler, tool};

#[derive(Clone)]
pub struct CaseTrackServer {
    state: Arc<RwLock<ServerState>>,
}

pub struct ServerState {
    pub registry: CaseRegistry,
    pub embedding_engine: EmbeddingEngine,
    pub search_engine: SearchEngine,
    pub active_case: Option<CaseHandle>,
    pub tier: LicenseTier,
    pub config: Config,
}

impl CaseTrackServer {
    pub async fn start(config: Config) -> Result<()> {
        // Initialize state
        let state = initialize(&config).await?;

        // Check for updates (non-blocking)
        check_for_updates(env!("CARGO_PKG_VERSION")).await;

        // Build MCP server
        let server = ServerBuilder::new("casetrack", env!("CARGO_PKG_VERSION"))
            .with_capabilities(ServerCapabilities {
                tools: Some(json!({ "listChanged": false })),
                ..Default::default()
            })
            .build(CaseTrackServer {
                state: Arc::new(RwLock::new(state)),
            });

        // Run on stdio transport
        let transport = rmcp::StdioTransport::new();
        server.run(transport).await?;

        Ok(())
    }
}
```

### 3.2 Tool Registration

```rust
#[rmcp::tool]
impl CaseTrackServer {
    #[tool(description = "Create a new legal case")]
    async fn create_case(
        &self,
        name: String,
        case_number: Option<String>,
        case_type: Option<String>,
    ) -> Result<ToolResult> {
        let mut state = self.state.write().await;

        // Check license limits
        check_case_limit(&state.registry, state.tier)?;

        let params = CreateCaseParams {
            name,
            case_number,
            case_type: case_type.map(|t| CaseType::from_str(&t)),
        };

        let case = state.registry.create_case(params)?;
        let handle = state.registry.switch_case(case.id)?;
        state.active_case = Some(handle);

        Ok(ToolResult::text(format!(
            "Created case \"{}\" (ID: {}).\nType: {:?}\nThis is now your active case.\n\nNext: Ingest documents with ingest_document.",
            case.name, case.id, case.case_type
        )))
    }

    #[tool(description = "Search across all documents in the active case")]
    async fn search_case(
        &self,
        query: String,
        top_k: Option<u32>,
        document_filter: Option<String>,
    ) -> Result<ToolResult> {
        let state = self.state.read().await;

        let case = state.active_case.as_ref()
            .ok_or(CaseTrackError::NoCaseActive)?;

        let doc_filter = document_filter
            .map(|f| self.resolve_document_filter(case, &f))
            .transpose()?;

        let results = state.search_engine.search(
            case,
            &query,
            top_k.unwrap_or(10) as usize,
            doc_filter,
        )?;

        Ok(ToolResult::text(self.format_search_results(&query, case, &results)))
    }

    // ... other tools follow the same pattern
}
```

### 3.3 Error Handling in MCP

All errors returned to Claude follow a consistent format:

```rust
impl From<CaseTrackError> for ToolError {
    fn from(err: CaseTrackError) -> Self {
        match &err {
            CaseTrackError::NoCaseActive => ToolError {
                code: ErrorCode::InvalidRequest,
                message: "No active case. Create or switch to a case first:\n  \
                          - create_case: Create a new case\n  \
                          - switch_case: Switch to an existing case\n  \
                          - list_cases: See all cases".to_string(),
            },
            CaseTrackError::CaseNotFound(id) => ToolError {
                code: ErrorCode::InvalidRequest,
                message: format!("Case not found: {}. Use list_cases to see available cases.", id),
            },
            CaseTrackError::FreeTierLimit { resource, current, max } => ToolError {
                code: ErrorCode::InvalidRequest,
                message: format!(
                    "Free tier allows {} {} (you have {}). \
                     Upgrade to Pro: https://casetrack.legal/upgrade",
                    max, resource, current
                ),
            },
            CaseTrackError::FileNotFound(path) => ToolError {
                code: ErrorCode::InvalidRequest,
                message: format!(
                    "File not found: {}\n\nCheck that the path is correct and the file exists.",
                    path.display()
                ),
            },
            // All other errors
            other => ToolError {
                code: ErrorCode::InternalError,
                message: format!("Internal error: {}. Please report this at https://github.com/casetrack-legal/casetrack/issues", other),
            },
        }
    }
}
```

---

## 4. Active Case State

The server maintains an "active case" that all document and search operations target:

```
STATE MANAGEMENT
=================================================================================

- Server starts with NO active case
- create_case automatically switches to the new case
- switch_case explicitly changes the active case
- Tools that require a case (ingest, search, etc.) return clear errors if none active
- Active case persists for the duration of the MCP session (conversation)
- No persistence of active case across sessions (fresh start each time)
```

This design means Claude naturally manages case context through conversation:

```
User: "Create a case called Smith v. Jones"
Claude: [calls create_case] -> case is now active

User: "Ingest this PDF"
Claude: [calls ingest_document] -> goes into Smith v. Jones (active)

User: "Switch to the Doe case"
Claude: [calls switch_case] -> Doe is now active

User: "Search for damages"
Claude: [calls search_case] -> searches Doe case (active)
```

---

*CaseTrack PRD v4.0.0 -- Document 9 of 10*

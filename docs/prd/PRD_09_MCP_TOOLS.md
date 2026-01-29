# PRD 09: MCP Tools

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Tool Overview

| Tool | Description | Tier | Requires Active Case |
|------|-------------|------|----------------------|
| `create_case` | Create a new legal case | Free | No |
| `list_cases` | List all cases | Free | No |
| `switch_case` | Switch active case | Free | No |
| `delete_case` | Delete a case and all its data | Free | No |
| `get_case_info` | Get details about active case | Free | Yes |
| `ingest_document` | Ingest a PDF, DOCX, XLSX, or image | Free | Yes |
| `ingest_folder` | Ingest all supported files in a folder and subfolders | Free | Yes |
| `sync_folder` | Sync a folder -- ingest new/changed files, optionally remove deleted | Free | Yes |
| `list_documents` | List documents in active case | Free | Yes |
| `get_document` | Get document details and stats | Free | Yes |
| `delete_document` | Remove a document from a case | Free | Yes |
| `search_case` | Search across all documents in the active case | Free (limited) | Yes |
| `find_entity` | Find mentions of an entity across documents | Pro | Yes |
| `get_chunk` | Get a specific chunk with full provenance | Free | Yes |
| `get_document_chunks` | List all chunks in a document with provenance | Free | Yes |
| `get_source_context` | Get surrounding text for a chunk (context window) | Free | Yes |
| `reindex_document` | Delete old embeddings/indexes for a document and rebuild from scratch | Free | Yes |
| `reindex_case` | Rebuild all embeddings and indexes for the entire active case | Free | Yes |
| `get_index_status` | Show embedding/index health for all documents in active case | Free | Yes |
| `watch_folder` | Start watching a folder for file changes -- auto-sync on change or schedule | Free | Yes |
| `unwatch_folder` | Stop watching a folder | Free | Yes |
| `list_watches` | List all active folder watches and their sync status | Free | No |
| `set_sync_schedule` | Set the auto-sync schedule (on_change, hourly, daily, manual) | Free | Yes |
| `get_status` | Get server status and model info | Free | No |
| `get_storage_summary` | Get disk usage breakdown by case with staleness and budget warnings | Free | No |
| `compact_case` | Trigger RocksDB compaction to reclaim disk space in active case | Free | Yes |
| `close_case` | Close a case (read-only, search still works) | Free | Yes |
| `archive_case` | Archive a case (read-only, hidden from default list, auto-compacts DB) | Free | Yes |
| | | | |
| **--- Context Graph: Case Overview ---** | | | |
| `get_case_summary` | High-level case briefing: key parties, counsel, key dates, legal issues, document categories, top entities, key citations, statistics | Free | Yes |
| `get_case_timeline` | Chronological view of key dates and events extracted from case documents | Free | Yes |
| `get_case_statistics` | Document counts, page counts, chunk counts, entity counts, citation counts, embedder coverage | Free | Yes |
| | | | |
| **--- Context Graph: Entity & Citation Search ---** | | | |
| `list_entities` | List all extracted entities in the case, grouped by type (party, court, judge, attorney, statute, etc.) | Free | Yes |
| `get_entity_mentions` | Get all chunks mentioning a specific entity, with context snippets | Free | Yes |
| `search_entity_relationships` | Find chunks mentioning two or more entities together | Pro | Yes |
| `get_entity_graph` | Show entity relationships across documents in the case | Pro | Yes |
| `list_references` | List all legal citations (case law, statutes, regulations) with citation counts | Free | Yes |
| `get_reference_citations` | Get all chunks citing a specific case, statute, or regulation, with context | Free | Yes |
| | | | |
| **--- Context Graph: Document Navigation ---** | | | |
| `get_document_structure` | Get headings, sections, and table of contents for a document | Free | Yes |
| `browse_pages` | Get all chunks from a specific page range within a document | Free | Yes |
| `find_related_documents` | Find documents similar to a given document (by shared entities, citations, or semantic similarity) | Free | Yes |
| `get_related_documents` | Given a document, find related docs via knowledge graph (shared entities, citations) | Free | Yes |
| `list_documents_by_type` | List documents filtered by type (complaint, motion, brief, contract, etc.) | Free | Yes |
| `traverse_chunks` | Navigate forward/backward through chunks in a document from a starting point | Free | Yes |
| | | | |
| **--- Context Graph: Advanced Search ---** | | | |
| `search_similar_chunks` | Find chunks semantically similar to a given chunk across all documents | Free | Yes |
| `compare_documents` | Compare what two documents say about a topic (side-by-side search) | Pro | Yes |
| `find_document_clusters` | Group documents by theme/topic using semantic clustering | Pro | Yes |
| | | | |
| **--- Legal-Specific Tools ---** | | | |
| `search_citations` | Search for legal citations (case law, statutes, regulations) across the case | Free | Yes |
| `get_case_parties` | List all parties, their roles, and counsel | Free | Yes |
| `get_citation_network` | Show which documents cite which cases/statutes | Pro | Yes |
| `compare_clauses` | Compare specific clauses across contract versions | Pro | Yes |

---

## 2. Tool Specifications

> **PROVENANCE IN EVERY RESPONSE**: Every MCP tool that returns text from a document
> MUST include the full provenance chain: source document filename, file path on disk,
> page number, paragraph range, line range, character offsets, extraction method, OCR
> confidence (if applicable), and timestamps (when ingested, when last embedded).
> A tool response that returns document text without telling the user exactly where
> it came from is a **bug**. The AI must always be able to cite its sources.

### Common Error Patterns

All tools return errors in a consistent MCP format. The four common error types:

```json
// NoCaseActive -- returned by any tool that requires an active case
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "No active case. Create or switch to a case first:\n  - create_case: Create a new case\n  - switch_case: Switch to an existing case\n  - list_cases: See all cases"
  }]
}

// FileNotFound -- returned by ingest_document, reindex_document, etc.
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "File not found: /Users/sarah/Cases/SmithVJones/Complaint.pdf\n\nCheck that the path is correct and the file exists."
  }]
}

// FreeTierLimit -- returned when a free tier quota is exceeded
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Free tier allows 3 cases (you have 3). Delete a case or upgrade to Pro for unlimited cases: https://casetrack.dev/upgrade"
  }]
}

// NotFound -- returned when a case, document, or chunk ID is not found
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Case not found: \"Smith\". Did you mean:\n  - Smith v. Jones (2024-CV-01234) (ID: a1b2c3d4)\nUse the full name or ID."
  }]
}
```

Per-tool error examples are omitted below; all errors follow these patterns.

---

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
        "description": "Case name (e.g., 'Smith v. Jones', 'In re Acme Corp Bankruptcy', 'Acme/BigCo Merger')"
      },
      "case_number": {
        "type": "string",
        "description": "Court case number or internal reference (e.g., '2024-CV-01234', 'M-2024-0567')"
      },
      "case_type": {
        "type": "string",
        "enum": ["litigation", "corporate", "real_estate", "bankruptcy", "immigration", "employment", "intellectual_property", "criminal", "family_law", "tax_law", "other"],
        "description": "Type of legal case"
      },
      "jurisdiction": {
        "type": "string",
        "description": "Court or jurisdiction (e.g., 'S.D.N.Y.', 'Cal. Super. Ct.', 'Del. Ch.')"
      },
      "client_name": {
        "type": "string",
        "description": "Primary client name"
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
    "text": "Created case \"Smith v. Jones\" (ID: a1b2c3d4).\nCase Number: 2024-CV-01234\nType: Litigation\nJurisdiction: S.D.N.Y.\nThis is now your active case.\n\nNext: Ingest documents with ingest_document."
  }]
}
```

---

### 2.2 `list_cases`

```json
{
  "name": "list_cases",
  "description": "List all cases. Shows name, case number, type, status, document count, and which case is active.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "status_filter": {
        "type": "string",
        "enum": ["active", "closed", "on_hold", "archived", "purged", "all"],
        "default": "active",
        "description": "Filter by case status. 'purged' shows cases exported to .ctcase ZIP."
      }
    }
  }
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
        "description": "Case name, case number, or ID to switch to"
      }
    },
    "required": ["case_name"]
  }
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
        "description": "Case name, case number, or ID to delete"
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

---

### 2.5 `get_case_info`

```json
{
  "name": "get_case_info",
  "description": "Get detailed information about the active case including document list, parties, and storage usage.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

---

### 2.6 `ingest_document`

```json
{
  "name": "ingest_document",
  "description": "Ingest a document (PDF, DOCX, XLSX, or image) into the active case. Extracts text (with OCR for scans), chunks the text, computes embeddings, and indexes for search. All processing and storage happens locally on your machine.",
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
        "enum": ["complaint", "answer", "motion", "brief", "memorandum", "deposition", "affidavit", "declaration", "order", "opinion", "judgment", "contract", "amendment", "exhibit", "correspondence", "discovery_request", "discovery_response", "subpoena", "notice", "stipulation", "other"],
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

---

### 2.7 `ingest_folder`

```json
{
  "name": "ingest_folder",
  "description": "Ingest all supported documents in a folder and all subfolders. Walks the entire directory tree recursively. Automatically skips files already ingested (matched by SHA256 hash). Supported formats: PDF, DOCX, DOC, XLSX, TXT, RTF, JPG, PNG, TIFF. Each file is chunked into 2000-character segments with full provenance.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "folder_path": {
        "type": "string",
        "description": "Absolute path to folder containing documents. All subfolders are included automatically."
      },
      "recursive": {
        "type": "boolean",
        "default": true,
        "description": "Include subfolders (default: true). Set to false to only process the top-level folder."
      },
      "skip_existing": {
        "type": "boolean",
        "default": true,
        "description": "Skip files already ingested (matched by SHA256 hash). Set to false to re-ingest everything."
      },
      "document_type": {
        "type": "string",
        "enum": ["complaint", "answer", "motion", "brief", "memorandum", "deposition", "affidavit", "declaration", "order", "opinion", "judgment", "contract", "amendment", "exhibit", "correspondence", "discovery_request", "discovery_response", "subpoena", "notice", "stipulation", "other"],
        "description": "Default document type for all files. If omitted, CaseTrack infers from file content."
      },
      "file_extensions": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Optional filter: only ingest files with these extensions (e.g., [\"pdf\", \"docx\", \"xlsx\"]). Default: all supported formats."
      }
    },
    "required": ["folder_path"]
  }
}
```

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Folder ingestion complete for Smith v. Jones (2024-CV-01234)\n\n  Folder:     ~/Cases/SmithVJones/Documents/\n  Subfolders: 5 (Pleadings/, Motions/, Discovery/, Correspondence/, Exhibits/)\n  Found:      47 supported files\n  New:        23 (ingested)\n  Skipped:    22 (already ingested, matching SHA256)\n  Failed:     2\n  Duration:   4 minutes 12 seconds\n\n  New documents ingested:\n  - Pleadings/Complaint.pdf (28 pages, 156 chunks)\n  - Pleadings/Answer.pdf (15 pages, 89 chunks)\n  - Motions/Motion_to_Dismiss.pdf (22 pages, 134 chunks)\n  - Discovery/Deposition_Smith.pdf (180 pages, 1,024 chunks)\n  - Exhibits/Contract_v1.pdf (45 pages, 234 chunks)\n  ... 18 more\n\n  Failures:\n  - Exhibits/corrupted_scan.pdf: PDF parsing error (file may be corrupted)\n  - Discovery/fax_2019.tiff: OCR failed (image too low resolution)\n\nAll 23 new documents are now searchable."
  }]
}
```

---

### 2.8 `sync_folder`

```json
{
  "name": "sync_folder",
  "description": "Sync a folder with the active case. Compares files on disk against what is already ingested and: (1) ingests new files not yet in the case, (2) re-ingests files that have changed since last ingestion (detected by SHA256 mismatch), (3) optionally removes documents whose source files no longer exist on disk. This is the easiest way to keep a case up to date with a directory of documents -- just point it at the folder and run it whenever files change.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "folder_path": {
        "type": "string",
        "description": "Absolute path to folder to sync. All subfolders are included."
      },
      "remove_deleted": {
        "type": "boolean",
        "default": false,
        "description": "If true, documents whose source files no longer exist on disk will be removed from the case (chunks + embeddings deleted). Default: false (only add/update, never remove)."
      },
      "document_type": {
        "type": "string",
        "enum": ["complaint", "answer", "motion", "brief", "memorandum", "deposition", "affidavit", "declaration", "order", "opinion", "judgment", "contract", "amendment", "exhibit", "correspondence", "discovery_request", "discovery_response", "subpoena", "notice", "stipulation", "other"],
        "description": "Default document type for newly ingested files."
      },
      "dry_run": {
        "type": "boolean",
        "default": false,
        "description": "If true, report what would change without actually ingesting or removing anything. Useful for previewing a sync."
      }
    },
    "required": ["folder_path"]
  }
}
```

---

### 2.9 `list_documents`

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

### 2.10 `get_document`

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

### 2.11 `delete_document`

```json
{
  "name": "delete_document",
  "description": "Remove a document and all its chunks, embeddings, index entries, and original file copy from the active case. Automatically compacts affected column families in the background to reclaim disk space.",
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

### 2.12 `search_case`

```json
{
  "name": "search_case",
  "description": "Search across all documents in the active case using semantic and keyword search. Returns results with FULL provenance: source document filename, file path, page, paragraph, line numbers, character offsets, extraction method, timestamps. Every result is traceable to its exact source location.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Natural language search query (e.g., 'indemnification obligations', 'breach of fiduciary duty', 'motion to dismiss standard')"
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

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Search: \"indemnification obligations\"\nCase: Smith v. Jones (2024-CV-01234) | 47 documents, 4,821 chunks searched\nTime: 87ms | Tier: Pro (3-stage pipeline)\n\n--- Result 1 (score: 0.94) ---\nComplaint.pdf, p. 8, para. 24, ll. 1-6\n\n\"Defendant shall indemnify, defend, and hold harmless Plaintiff from and against any and all claims, damages, losses, costs, and expenses (including reasonable attorneys' fees) arising out of or relating to Defendant's breach of this Agreement.\"\n\n--- Result 2 (score: 0.91) ---\nSmith_v_Jones_Opinion.pdf, p. 14, para. 3, ll. 1-8\n\n\"The Court finds that the indemnification clause in the Agreement is enforceable under New York law. See Hooper Assocs., Ltd. v. AGS Computers, Inc., 74 N.Y.2d 487 (1989).\"\n\n--- Result 3 (score: 0.82) ---\nMotion_to_Dismiss.pdf, p. 6, para. 12, ll. 1-5\n\n\"Respondent's Motion to Dismiss argues that the indemnification provision is unconscionable. However, the standard set forth in Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993) requires...\""
  }]
}
```

---

### 2.13 `find_entity`

```json
{
  "name": "find_entity",
  "description": "Find all mentions of an entity (party, court, judge, attorney, statute, case number) across documents. Uses the entity index built during ingestion.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity": {
        "type": "string",
        "description": "Entity to find (e.g., 'Judge Williams', 'Acme Corp', '42 U.S.C. ss 1983', '$1.2 million')"
      },
      "entity_type": {
        "type": "string",
        "enum": ["party", "court", "judge", "attorney", "statute", "case_number", "jurisdiction", "legal_concept", "remedy", "witness", "exhibit", "docket_entry", "person", "organization", "date", "amount", "location", "any"],
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

### 2.14 `reindex_document`

```json
{
  "name": "reindex_document",
  "description": "Rebuild all embeddings, chunks, and search indexes for a single document. Deletes all existing chunks and embeddings for the document, re-extracts text from the original file, re-chunks into 2000-character segments, re-embeds with all active models, and rebuilds the BM25 index. Use this when: (1) a document's source file has been updated on disk, (2) you upgraded to Pro tier and want the document embedded with all 4 models, (3) embeddings seem stale or corrupt, (4) OCR results need refreshing. The original file path stored in provenance is used to re-read the source file.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID to reindex"
      },
      "force": {
        "type": "boolean",
        "default": false,
        "description": "If true, reindex even if the source file SHA256 has not changed. Default: only reindex if the file has changed."
      },
      "reparse": {
        "type": "boolean",
        "default": true,
        "description": "If true (default), re-extract text from the source file and re-chunk. If false, keep existing chunks but only rebuild embeddings and indexes (faster, useful after tier upgrade)."
      }
    },
    "required": ["document_name"]
  }
}
```

---

### 2.15 `reindex_case`

```json
{
  "name": "reindex_case",
  "description": "Rebuild all embeddings, chunks, and search indexes for every document in the active case. This is a full rebuild -- it deletes ALL existing chunks and embeddings, re-reads every source file, re-chunks, re-embeds with all active models, and rebuilds the entire BM25 index. Use this when: (1) upgrading from Free to Pro tier (re-embed everything with 4 models instead of 3), (2) after a CaseTrack update that changes chunking or embedding logic, (3) the case index seems corrupted or stale, (4) you want a clean rebuild. WARNING: This can be slow for large cases (hundreds of documents).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "confirm": {
        "type": "boolean",
        "default": false,
        "description": "Must be true to confirm. This deletes and rebuilds ALL embeddings in the case."
      },
      "reparse": {
        "type": "boolean",
        "default": true,
        "description": "If true (default), re-extract text from source files and re-chunk everything. If false, keep existing chunks but only rebuild embeddings and indexes (faster, useful after tier upgrade)."
      },
      "skip_unchanged": {
        "type": "boolean",
        "default": false,
        "description": "If true, skip documents whose source files have not changed (SHA256 match) and whose embeddings are complete for the current tier. Default: false (rebuild everything)."
      }
    },
    "required": ["confirm"]
  }
}
```

---

### 2.16 `get_index_status`

```json
{
  "name": "get_index_status",
  "description": "Show the embedding and index health status for all documents in the active case. Reports which documents have complete embeddings for the current tier, which need reindexing (source file changed, missing embedder coverage, stale embeddings), and overall case index health. Use this to diagnose issues or decide whether to run reindex_document or reindex_case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_filter": {
        "type": "string",
        "description": "Optional: check a specific document instead of all"
      }
    }
  }
}
```

---

### 2.17 `get_status`

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

---

### 2.18 `get_storage_summary`

```json
{
  "name": "get_storage_summary",
  "description": "Get a detailed breakdown of CaseTrack's disk usage. Shows: total usage across all cases and models, per-case usage sorted by size, stale case detection (cases inactive for 6+ months), storage budget usage percentage, and warnings when approaching the configured storage budget. Use this to help attorneys manage disk space and identify cases that can be archived or deleted.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "include_models": {
        "type": "boolean",
        "default": true,
        "description": "Include model sizes in the summary"
      }
    }
  }
}
```

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "STORAGE SUMMARY\n\n  Total:     2.4 GB (24% of 10 GB budget)\n  Models:    550 MB (Pro tier: Legal-BERT + SPLADE + ColBERT)\n  Cases:     1.85 GB across 12 cases\n\n  ACTIVE CASES (1.2 GB):\n    Smith v. Jones          480 MB    47 docs    4,821 chunks    2 days ago\n    Doe v. TechCorp         320 MB    31 docs    2,145 chunks    5 days ago\n    Johnson Contract        210 MB    18 docs    1,230 chunks    12 days ago\n    Martinez Estate         190 MB    14 docs      890 chunks    30 days ago\n\n  STALE CASES (>6 months inactive, 520 MB):\n    ⚠ Wilson IP Dispute     280 MB    22 docs    1,567 chunks    8 months ago\n    ⚠ Adams v. Corp         240 MB    19 docs    1,234 chunks    11 months ago\n\n  ARCHIVED CASES (130 MB):\n    Brown Bankruptcy         80 MB     8 docs      456 chunks    Archived\n    Regulatory Review        50 MB     5 docs      312 chunks    Archived\n\n  💡 Tip: 2 stale cases are using 520 MB. Consider:\n     - archive_case to mark them read-only + auto-compact\n     - delete_case to permanently remove them"
  }]
}
```

---

### 2.19 `compact_case`

```json
{
  "name": "compact_case",
  "description": "Trigger RocksDB compaction on the active case to reclaim disk space. Compacts all column families, removing tombstones from deleted documents and applying full compression. This is automatically run on archive_case, but can be run manually anytime. Safe to run while the case is in use -- compaction runs in the background. Typically reduces case storage by 20-40% after deleting or reindexing documents.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Compaction complete for Smith v. Jones.\n  Before: 480 MB\n  After:  312 MB\n  Saved:  168 MB (35%)"
  }]
}
```

---

### 2.20 `close_case`

```json
{
  "name": "close_case",
  "description": "Close the active case. A closed case is read-only: search still works, but new documents cannot be ingested. Use this when a case has concluded but you may still need to reference it. The case remains visible in list_cases.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "case_name": {
        "type": "string",
        "description": "Case name, case number, or ID to close. Defaults to active case if omitted."
      }
    }
  }
}
```

---

### 2.21 `archive_case`

```json
{
  "name": "archive_case",
  "description": "Archive a case. An archived case is read-only and hidden from the default list_cases view (use status_filter='archived' or 'all' to see it). Automatically runs RocksDB compaction on all column families to reclaim disk space (typically 30-50% reduction). Use this for cases that are fully resolved and unlikely to be accessed frequently.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "case_name": {
        "type": "string",
        "description": "Case name, case number, or ID to archive. Defaults to active case if omitted."
      }
    }
  }
}
```

**Success Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Archived: Smith v. Jones\n  Status: Archived (read-only, hidden from default list)\n  Compacted: 480 MB → 312 MB (saved 168 MB)\n\n  To see archived cases: list_cases with status_filter='archived'\n  To restore: reopen_case"
  }]
}
```

---

### 2.22 `get_chunk`

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
    "text": "Chunk abc-123 (2000 chars)\n\nText:\n\"Defendant shall indemnify, defend, and hold harmless Plaintiff from and against any and all claims, damages, losses, costs, and expenses (including reasonable attorneys' fees)...\"\n\nProvenance:\n  Document:   Complaint.pdf\n  File Path:  /Users/sarah/Cases/SmithVJones/Pleadings/Complaint.pdf\n  Page:       8\n  Paragraphs: 24-25\n  Lines:      1-14\n  Chars:      18240-20240 (within page)\n  Extraction: Native text\n  Chunk Index: 47 of 156\n\nEmbeddings: E1, E6, E12"
  }]
}
```

---

### 2.23 `get_document_chunks`

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

---

### 2.24 `get_source_context`

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

---

### 2.25 `watch_folder`

```json
{
  "name": "watch_folder",
  "description": "Start watching a folder for file changes. When files are added, modified, or deleted in the watched folder (or any subfolder), CaseTrack automatically syncs the changes into the active case -- new files are ingested, modified files are reindexed (old chunks/embeddings deleted, new ones created), and optionally deleted files are removed from the case. Uses OS-level file notifications (inotify on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows) for instant detection. Also supports scheduled sync as a safety net (daily, hourly, or custom interval). Watch persists across server restarts.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "folder_path": {
        "type": "string",
        "description": "Absolute path to the folder to watch. All subfolders are included."
      },
      "schedule": {
        "type": "string",
        "enum": ["on_change", "hourly", "daily", "every_6h", "every_12h", "manual"],
        "default": "on_change",
        "description": "When to sync: 'on_change' = real-time via OS file notifications (recommended), 'hourly'/'daily'/'every_6h'/'every_12h' = scheduled interval (runs in addition to on_change), 'manual' = only sync when you call sync_folder."
      },
      "auto_remove_deleted": {
        "type": "boolean",
        "default": false,
        "description": "If true, documents whose source files are deleted from disk will be automatically removed from the case (chunks + embeddings deleted). Default: false (only add/update, never auto-remove)."
      },
      "file_extensions": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Optional filter: only watch files with these extensions (e.g., [\"pdf\", \"docx\", \"xlsx\"]). Default: all supported formats."
      }
    },
    "required": ["folder_path"]
  }
}
```

---

### 2.26 `unwatch_folder`

```json
{
  "name": "unwatch_folder",
  "description": "Stop watching a folder. Removes the watch but does NOT delete any documents already ingested from that folder. The case data remains intact -- only the automatic sync is stopped.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "folder_path": {
        "type": "string",
        "description": "Path to the folder to stop watching (or watch ID)"
      }
    },
    "required": ["folder_path"]
  }
}
```

---

### 2.27 `list_watches`

```json
{
  "name": "list_watches",
  "description": "List all active folder watches across all cases. Shows the watched folder, which case it syncs to, the schedule, last sync time, and current status.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "case_filter": {
        "type": "string",
        "description": "Optional: only show watches for a specific case name or ID"
      }
    }
  }
}
```

---

### 2.28 `set_sync_schedule`

```json
{
  "name": "set_sync_schedule",
  "description": "Change the sync schedule for an existing folder watch. Controls how often CaseTrack checks for file changes and reindexes.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "folder_path": {
        "type": "string",
        "description": "Path to the watched folder (or watch ID)"
      },
      "schedule": {
        "type": "string",
        "enum": ["on_change", "hourly", "daily", "every_6h", "every_12h", "manual"],
        "description": "New schedule: 'on_change' = real-time OS notifications, 'hourly'/'daily' etc = interval-based, 'manual' = only when you call sync_folder"
      },
      "auto_remove_deleted": {
        "type": "boolean",
        "description": "Optionally update auto-remove behavior"
      }
    },
    "required": ["folder_path", "schedule"]
  }
}
```

---

## 2b. Context Graph Tool Specifications

The context graph tools give the AI structured navigation of the case beyond flat search. They are built on the entity, citation, and document graph data extracted during ingestion (see PRD 04 Section 8).

### 2.29 `get_case_summary`

```json
{
  "name": "get_case_summary",
  "description": "Get a high-level briefing on the active case. Returns: parties and counsel, key dates and events (filing, deadlines, hearings), legal issues, document breakdown by category, key legal citations (most-referenced cases and statutes), most-mentioned entities, and case statistics. This is the FIRST tool the AI should call when starting work on a case -- it provides the structural overview needed to plan search strategy for cases with hundreds of documents.",
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
    "text": "CASE SUMMARY: Smith v. Jones (2024-CV-01234) -- Litigation\nJurisdiction: S.D.N.Y. | Judge: Hon. Patricia Williams\n\n  PARTIES & COUNSEL:\n    Plaintiff:  Smith Industries LLC\n      Counsel:  Sarah Chen, Chen & Associates LLP\n    Defendant:  Jones Holdings Inc.\n      Counsel:  Michael Brown, Brown Kraft LLP\n\n  KEY DATES:\n    2024-01-15  Complaint filed (Complaint.pdf, p.1)\n    2024-02-28  Answer filed (Answer.pdf, p.1)\n    2024-03-15  Motion to Dismiss filed (Motion_to_Dismiss.pdf, p.1)\n    2024-05-01  Opposition to MTD due\n    2024-06-15  Discovery deadline\n    2024-09-01  Summary judgment deadline\n    2024-12-01  Trial date\n\n  KEY LEGAL ISSUES:\n    1. Breach of contract (indemnification clause) -- 23 documents, 187 chunks\n    2. Breach of fiduciary duty -- 18 documents, 145 chunks\n    3. Fraudulent misrepresentation -- 8 documents, 42 chunks\n    4. Damages calculation -- 5 documents, 28 chunks\n\n  DOCUMENTS (47 total, 2,341 pages, 4,821 chunks):\n    Pleadings:          5 docs (Complaint, Answer, Counterclaim...)\n    Motions:            8 docs (MTD, Opposition, Reply, MSJ...)\n    Discovery:         20 docs (Depositions, Interrogatories, RFPs...)\n    Contracts:          5 docs (Master Agreement, Amendments...)\n    Correspondence:     7 docs (Demand Letters, Settlement Offers...)\n    Exhibits:           2 docs\n\n  KEY CITATIONS (most cited):\n    1. Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993) -- 12 citations across 6 documents\n    2. Hooper Assocs., Ltd. v. AGS Computers, Inc., 74 N.Y.2d 487 (1989) -- 8 citations across 4 documents\n    3. 42 U.S.C. ss 1983 -- 6 citations across 3 documents\n    4. N.Y. Bus. Corp. Law ss 720 -- 5 citations across 3 documents\n\n  TOP ENTITIES:\n    Smith Industries LLC -- 892 mentions in 45 documents\n    Jones Holdings Inc. -- 756 mentions in 42 documents\n    Judge Patricia Williams -- 234 mentions in 28 documents\n    Master Agreement -- 187 mentions in 23 documents\n\n  EMBEDDINGS: 4/4 embedders (Pro tier), all 4,821 chunks fully embedded"
  }]
}
```

---

### 2.30 `get_case_timeline`

```json
{
  "name": "get_case_timeline",
  "description": "Get a chronological timeline of key dates and events extracted from case documents. Each event includes the date, description, and source document/chunk provenance. Use this to understand the procedural history and upcoming deadlines.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "start_date": {
        "type": "string",
        "description": "Optional: filter events from this date (YYYY-MM-DD)"
      },
      "end_date": {
        "type": "string",
        "description": "Optional: filter events until this date (YYYY-MM-DD)"
      }
    }
  }
}
```

---

### 2.31 `get_case_statistics`

```json
{
  "name": "get_case_statistics",
  "description": "Get detailed statistics about the active case: document counts by type, page/chunk totals, entity and citation counts, embedder coverage, storage usage. Useful for understanding case scope and data quality.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

---

### 2.32 `list_entities`

```json
{
  "name": "list_entities",
  "description": "List all entities extracted from documents in the active case, grouped by type. Shows name, type, mention count, and number of documents mentioning each entity. Entity types include: party, court, judge, attorney, statute, case_number, jurisdiction, legal_concept, remedy, witness, exhibit, docket_entry, person, organization, date, amount, location.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_type": {
        "type": "string",
        "enum": ["party", "court", "judge", "attorney", "statute", "case_number", "jurisdiction", "legal_concept", "remedy", "witness", "exhibit", "docket_entry", "person", "organization", "date", "amount", "location", "all"],
        "default": "all",
        "description": "Filter by entity type"
      },
      "sort_by": {
        "type": "string",
        "enum": ["mentions", "documents", "name"],
        "default": "mentions",
        "description": "Sort order"
      },
      "top_k": {
        "type": "integer",
        "default": 50,
        "maximum": 500,
        "description": "Maximum entities to return"
      }
    }
  }
}
```

---

### 2.33 `get_entity_mentions`

```json
{
  "name": "get_entity_mentions",
  "description": "Get all chunks that mention a specific entity, with context snippets showing how the entity is referenced. Uses the entity index built during ingestion. Supports fuzzy matching on entity name.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_name": {
        "type": "string",
        "description": "Name of the entity to find (e.g., 'Judge Williams', 'Smith Industries', 'indemnification', '42 U.S.C. ss 1983')"
      },
      "entity_type": {
        "type": "string",
        "enum": ["party", "court", "judge", "attorney", "statute", "case_number", "jurisdiction", "legal_concept", "remedy", "witness", "exhibit", "docket_entry", "person", "organization", "date", "amount", "location", "any"],
        "default": "any"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 100
      }
    },
    "required": ["entity_name"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Mentions of \"Judge Williams\" (judge) -- 234 total, showing top 20:\n\n  1. Order_on_MTD.pdf, p.1, para.1\n     \"UNITED STATES DISTRICT COURT, SOUTHERN DISTRICT OF NEW YORK\n      The Honorable Patricia Williams, presiding.\"\n\n  2. Deposition_Smith.pdf, p.45, para.8\n     \"Q: Were you aware that Judge Williams had issued the preliminary injunction?\"\n     \"A: Yes, our counsel informed us on March 10, 2024...\"\n\n  3. Motion_to_Dismiss.pdf, p.1, para.1 (caption)\n     \"Before the Honorable Patricia Williams, United States District Judge\"\n\n  ... 17 more mentions"
  }]
}
```

---

### 2.34 `search_entity_relationships`

```json
{
  "name": "search_entity_relationships",
  "description": "Find chunks where two or more entities are mentioned together. Use this to trace relationships (which parties interacted, what statutes apply to which claims, which attorney argued which motion). Pro tier only.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entities": {
        "type": "array",
        "items": { "type": "string" },
        "minItems": 2,
        "maxItems": 5,
        "description": "Entity names to find together (e.g., ['Smith Industries', 'indemnification'], ['Judge Williams', 'motion to dismiss'])"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 100
      }
    },
    "required": ["entities"]
  }
}
```

---

### 2.35 `get_entity_graph`

```json
{
  "name": "get_entity_graph",
  "description": "Show entity relationships across documents in the active case. Returns a graph of entities connected by co-occurrence in documents and chunks. Use this to understand how parties, attorneys, courts, statutes, and legal concepts relate to each other across the case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_name": {
        "type": "string",
        "description": "Optional: center the graph on a specific entity. If omitted, returns the top entities by connectivity."
      },
      "depth": {
        "type": "integer",
        "default": 2,
        "minimum": 1,
        "maximum": 4,
        "description": "How many relationship hops to include from the center entity"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 100,
        "description": "Maximum entities to include in the graph"
      }
    }
  }
}
```

---

### 2.36 `list_references`

```json
{
  "name": "list_references",
  "description": "List all legal citations referenced in the active case: case law, statutes, regulations, rules, constitutional provisions, treaties, and secondary sources. Shows the citation, type, reference count, and number of citing documents. Use this to understand which legal authorities matter most in the case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "reference_type": {
        "type": "string",
        "enum": ["case_law", "statute", "regulation", "rule", "constitution", "treaty", "secondary_source", "all"],
        "default": "all"
      },
      "sort_by": {
        "type": "string",
        "enum": ["citations", "documents", "name"],
        "default": "citations"
      },
      "top_k": {
        "type": "integer",
        "default": 50,
        "maximum": 200
      }
    }
  }
}
```

---

### 2.37 `get_reference_citations`

```json
{
  "name": "get_reference_citations",
  "description": "Get all chunks that cite a specific legal authority. Shows the context of each citation and how it is used (cited for what proposition). Use this to understand how a case, statute, or regulation is applied throughout the case documents.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "reference": {
        "type": "string",
        "description": "The legal citation to look up (e.g., 'Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993)', '42 U.S.C. ss 1983', 'Fed. R. Civ. P. 12(b)(6)')"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 100
      }
    },
    "required": ["reference"]
  }
}
```

---

### 2.38 `get_document_structure`

```json
{
  "name": "get_document_structure",
  "description": "Get the structural outline of a document: headings, sections, numbered clauses, and their page/chunk locations. This gives the AI a table-of-contents view for navigation. Works best with structured documents (contracts, briefs, motions, opinions).",
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

### 2.39 `browse_pages`

```json
{
  "name": "browse_pages",
  "description": "Get all chunks from a specific page range within a document. Use this to read through a section of a document sequentially. Returns chunks in order with full provenance.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID"
      },
      "start_page": {
        "type": "integer",
        "minimum": 1,
        "description": "First page to read"
      },
      "end_page": {
        "type": "integer",
        "minimum": 1,
        "description": "Last page to read"
      }
    },
    "required": ["document_name", "start_page", "end_page"]
  }
}
```

---

### 2.40 `find_related_documents`

```json
{
  "name": "find_related_documents",
  "description": "Find documents related to a given document. Relationships detected: shared entities, shared legal citations, semantic similarity (E1 cosine), and version chains. Returns related documents ranked by relationship strength.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID to find relationships for"
      },
      "relationship_type": {
        "type": "string",
        "enum": ["all", "shared_entities", "shared_citations", "semantic_similar", "version_chain"],
        "default": "all"
      },
      "top_k": {
        "type": "integer",
        "default": 10,
        "maximum": 50
      }
    },
    "required": ["document_name"]
  }
}
```

---

### 2.41 `get_related_documents`

```json
{
  "name": "get_related_documents",
  "description": "Given a document, find related docs via the knowledge graph. Uses shared entities, legal citations, and semantic similarity to surface connections. This is a knowledge-graph-first approach compared to find_related_documents which also supports explicit relationship types.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID"
      },
      "top_k": {
        "type": "integer",
        "default": 10,
        "maximum": 50
      }
    },
    "required": ["document_name"]
  }
}
```

---

### 2.42 `list_documents_by_type`

```json
{
  "name": "list_documents_by_type",
  "description": "List all documents in the active case filtered by document type (complaint, motion, brief, contract, etc.). Includes page count, chunk count, and ingestion date.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_type": {
        "type": "string",
        "enum": ["complaint", "answer", "motion", "brief", "memorandum", "deposition", "affidavit", "declaration", "order", "opinion", "judgment", "contract", "amendment", "exhibit", "correspondence", "discovery_request", "discovery_response", "subpoena", "notice", "stipulation", "other"],
        "description": "Type to filter by"
      }
    },
    "required": ["document_type"]
  }
}
```

---

### 2.43 `traverse_chunks`

```json
{
  "name": "traverse_chunks",
  "description": "Navigate forward or backward through chunks in a document from a starting point. Use this to read through a document sequentially from any position. Returns N chunks in document order with full provenance.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "start_chunk_id": {
        "type": "string",
        "description": "UUID of the starting chunk"
      },
      "direction": {
        "type": "string",
        "enum": ["forward", "backward"],
        "default": "forward",
        "description": "Direction to traverse"
      },
      "count": {
        "type": "integer",
        "default": 5,
        "minimum": 1,
        "maximum": 20,
        "description": "Number of chunks to return"
      }
    },
    "required": ["start_chunk_id"]
  }
}
```

---

### 2.44 `search_similar_chunks`

```json
{
  "name": "search_similar_chunks",
  "description": "Find chunks across all documents that are semantically similar to a given chunk. Uses E1 Legal-BERT-base cosine similarity (768D). Use this to find related passages in other documents (e.g., 'find other places in the case that discuss the same legal issue as this paragraph').",
  "inputSchema": {
    "type": "object",
    "properties": {
      "chunk_id": {
        "type": "string",
        "description": "UUID of the chunk to find similar content for"
      },
      "exclude_same_document": {
        "type": "boolean",
        "default": true,
        "description": "Exclude results from the same document (default: true, for cross-document discovery)"
      },
      "min_similarity": {
        "type": "number",
        "default": 0.6,
        "minimum": 0.0,
        "maximum": 1.0,
        "description": "Minimum cosine similarity threshold"
      },
      "top_k": {
        "type": "integer",
        "default": 10,
        "maximum": 50
      }
    },
    "required": ["chunk_id"]
  }
}
```

---

### 2.45 `compare_documents`

```json
{
  "name": "compare_documents",
  "description": "Compare what two documents say about a specific topic. Searches both documents independently, then returns side-by-side results showing how each document addresses the topic. Pro tier only. Use this for: complaint vs. answer comparison, motion vs. opposition, contract v1 vs. v2, any 'what does X say vs. what does Y say' question.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_a": {
        "type": "string",
        "description": "First document name or ID"
      },
      "document_b": {
        "type": "string",
        "description": "Second document name or ID"
      },
      "topic": {
        "type": "string",
        "description": "Topic to compare (e.g., 'indemnification', 'damages calculation', 'standard of review', 'statute of limitations')"
      },
      "top_k_per_document": {
        "type": "integer",
        "default": 5,
        "maximum": 20
      }
    },
    "required": ["document_a", "document_b", "topic"]
  }
}
```

---

### 2.46 `find_document_clusters`

```json
{
  "name": "find_document_clusters",
  "description": "Group all documents in the case by theme or topic using semantic clustering. Returns clusters of related documents with a label describing what they share. Pro tier only. Use this to understand the structure of a large case (100+ documents) at a glance.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "strategy": {
        "type": "string",
        "enum": ["topical", "entity", "citation", "document_type"],
        "default": "topical",
        "description": "Clustering strategy: 'topical' = semantic similarity, 'entity' = shared parties/courts, 'citation' = shared legal authorities, 'document_type' = by type"
      },
      "max_clusters": {
        "type": "integer",
        "default": 10,
        "maximum": 20
      }
    }
  }
}
```

---

## 2c. Legal-Specific Tool Specifications

These tools provide legal-domain capabilities beyond generic document search.

### 2.47 `search_citations`

```json
{
  "name": "search_citations",
  "description": "Search for legal citations (case law, statutes, regulations) across the active case. Unlike search_case which searches document text, this tool searches the citation index built during ingestion. It finds all documents and chunks that cite a specific legal authority, resolves short-form citations ('Id.', 'supra'), and supports partial matching (e.g., 'Daubert' matches 'Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993)').",
  "inputSchema": {
    "type": "object",
    "properties": {
      "citation": {
        "type": "string",
        "description": "Legal citation to search for (e.g., 'Daubert v. Merrell Dow', '42 U.S.C. ss 1983', 'Fed. R. Civ. P. 12(b)(6)'). Supports partial matching."
      },
      "citation_type": {
        "type": "string",
        "enum": ["case_law", "statute", "regulation", "rule", "constitution", "treaty", "secondary_source", "any"],
        "default": "any",
        "description": "Filter by citation type"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 100,
        "description": "Maximum results to return"
      }
    },
    "required": ["citation"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Citations of \"Daubert v. Merrell Dow\" (case_law) -- 12 total, showing top 20:\n\n  Full citation: Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993)\n\n  1. Motion_to_Dismiss.pdf, p. 6, para. 12\n     Cited for: Standard for admissibility of expert testimony\n     \"...the standard set forth in Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993) requires that expert testimony be both relevant and reliable...\"\n\n  2. Opposition_to_MTD.pdf, p. 14, para. 8\n     Cited for: Flexible inquiry, not rigid checklist\n     \"The Daubert inquiry is a flexible one. Id. at 594. The Court should consider...\"\n\n  3. Expert_Report.pdf, p. 2, para. 4\n     Cited for: Methodology requirements\n     \"Under Daubert, 509 U.S. at 593-94, the expert's methodology must be...\"\n\n  ... 9 more citations"
  }]
}
```

---

### 2.48 `get_case_parties`

```json
{
  "name": "get_case_parties",
  "description": "List all parties involved in the active case, their roles (plaintiff, defendant, third-party, intervenor, amicus), and their counsel of record. Built from entity extraction during ingestion, augmented by caption and signature block parsing.",
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
    "text": "PARTIES: Smith v. Jones (2024-CV-01234)\n\n  PLAINTIFF:\n    Smith Industries LLC\n      Role:     Plaintiff\n      Counsel:  Sarah Chen, Esq.\n                Chen & Associates LLP\n                100 Park Avenue, New York, NY 10017\n      First Appearance: Complaint.pdf, p.1 (2024-01-15)\n      Mentions: 892 across 45 documents\n\n  DEFENDANT:\n    Jones Holdings Inc.\n      Role:     Defendant\n      Counsel:  Michael Brown, Esq.\n                Brown Kraft LLP\n                200 Broadway, New York, NY 10007\n      First Appearance: Complaint.pdf, p.1 (caption)\n      Mentions: 756 across 42 documents\n\n  COURT:\n    United States District Court, Southern District of New York\n    Judge: Hon. Patricia Williams\n    Magistrate: Hon. David Park (discovery disputes)"
  }]
}
```

---

### 2.49 `get_citation_network`

```json
{
  "name": "get_citation_network",
  "description": "Show the citation network for the active case: which case documents cite which legal authorities (cases, statutes, regulations), and how those authorities connect to each other. Pro tier only. Use this to understand the legal authority structure underpinning the case.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "center_citation": {
        "type": "string",
        "description": "Optional: center the network on a specific citation. If omitted, returns the most-cited authorities."
      },
      "depth": {
        "type": "integer",
        "default": 2,
        "minimum": 1,
        "maximum": 3,
        "description": "How many citation hops to include"
      },
      "top_k": {
        "type": "integer",
        "default": 20,
        "maximum": 50,
        "description": "Maximum authorities to include in the network"
      }
    }
  }
}
```

---

### 2.50 `compare_clauses`

```json
{
  "name": "compare_clauses",
  "description": "Compare specific clauses across contract versions or related agreements. Identifies a clause by section number or heading in one document and finds the corresponding clause in another. Shows differences in language, scope, and terms. Pro tier only.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_a": {
        "type": "string",
        "description": "First document name or ID (e.g., 'Contract_v1.pdf')"
      },
      "document_b": {
        "type": "string",
        "description": "Second document name or ID (e.g., 'Contract_v2.pdf')"
      },
      "clause_identifier": {
        "type": "string",
        "description": "Section number, heading, or description of the clause to compare (e.g., 'Section 8.2', 'Indemnification', 'Limitation of Liability')"
      }
    },
    "required": ["document_a", "document_b", "clause_identifier"]
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "CLAUSE COMPARISON: Indemnification (Section 8.2)\n\n  --- Contract_v1.pdf, p.12, Section 8.2 ---\n  \"Vendor shall indemnify Client against third-party claims arising\n   from Vendor's negligence. Cap: $500,000.\"\n\n  --- Contract_v2.pdf, p.14, Section 8.2 ---\n  \"Vendor shall indemnify, defend, and hold harmless Client from\n   and against any and all claims, damages, losses, costs, and\n   expenses (including reasonable attorneys' fees) arising out of\n   or relating to Vendor's breach of this Agreement or negligence.\n   Cap: $2,000,000.\"\n\n  KEY DIFFERENCES:\n  1. Scope expanded: v1 covers only 'negligence'; v2 adds 'breach of Agreement'\n  2. Protection broadened: v2 adds 'defend and hold harmless' language\n  3. Coverage expanded: v2 includes 'attorneys' fees'\n  4. Cap increased: $500,000 -> $2,000,000"
  }]
}
```

---

## 3. Background Watch System

The folder watch system runs as background tasks inside the MCP server process using the `notify` crate for cross-platform OS file notifications. Key data structures:

```rust
pub struct WatchManager {
    watches: Arc<RwLock<Vec<ActiveWatch>>>,
    fs_watcher: notify::RecommendedWatcher,
    event_tx: mpsc::Sender<FsEvent>,
}

struct ActiveWatch {
    config: FolderWatch,
    case_handle: Arc<CaseHandle>,
}

enum FsEventKind { Created, Modified, Deleted }
```

Behavior: On startup, `WatchManager::init` restores saved watches from `watches.json`, starts OS watchers, and spawns two background tasks -- an event processor (with 2-second debounce) and a scheduled sync runner (checks every 60 seconds). Events are batched: Created triggers ingest, Modified triggers reindex, Deleted triggers removal (if `auto_remove_deleted` is enabled).

For full implementation details (server initialization, tool registration, error handling), see [PRD 10: Technical Build Guide](PRD_10_TECHNICAL_BUILD.md).

---

## 4. Active Case State

The server maintains an "active case" that all document and search operations target. The server starts with no active case; `create_case` automatically switches to the new case, and `switch_case` explicitly changes it. Tools requiring a case return a `NoCaseActive` error if none is set. The active case persists for the MCP session duration but not across sessions.

---

*CaseTrack PRD v5.1.0 -- Document 9 of 10*

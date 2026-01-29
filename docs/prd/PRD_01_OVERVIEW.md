# PRD 01: CaseTrack Overview

## Legal Case Management Intelligence for Claude Code & Claude Desktop

**Version**: 5.1.0
**Date**: 2026-01-29
**Status**: Draft
**Scope**: Fresh greenfield project build
**Language**: Rust (entire project -- no exceptions)
**Domain**: Legal case management
**Design Priority**: ACCURACY FIRST -- never sacrifice retrieval accuracy for speed or size

> **BUILD MANDATE**: CaseTrack is built entirely in Rust. The binary crate, core
> library, MCP server, document processing, embedding engine, storage layer,
> search engine, license validation, CLI, and all tooling are Rust. The only
> non-Rust code is a Python helper script for one-time ONNX model conversion
> (a build-time tool, not shipped to users). There is no JavaScript, TypeScript,
> Python, Go, or C++ in the product. All dependencies are Rust crates. The
> output is a single statically-linked Rust binary with zero runtime dependencies.

> **ACCURACY MANDATE**: CaseTrack exists to give legal professionals CORRECT answers
> with EXACT citations. Accuracy is the #1 priority. Every design tradeoff is
> resolved in favor of accuracy. If a larger model produces better results, we use
> the larger model. If a slower search produces more precise results, we use the
> slower search. Legal professionals depend on CaseTrack to find the right clause,
> the right precedent, the right statute -- a wrong answer in legal work can mean
> malpractice. The total model footprint targets ≤ 16GB RAM at peak, which allows
> full-size legal-domain models without compromise.

> **PROVENANCE MANDATE**: Every piece of information CaseTrack returns MUST trace
> back to its exact source. This is non-negotiable for legal work -- attorneys must
> cite sources precisely. The provenance chain is:
>
> **Embedding vector → Chunk → Provenance → Source document (file path + filename)**
>
> Every chunk stores: source file path, document filename, page number, paragraph,
> line number, character offsets, extraction method, and timestamps (created_at,
> embedded_at). Every embedding vector is keyed to a chunk_id. Every entity mention,
> reference, and graph edge stores the chunk_id and document_id it came from. Every
> search result, every MCP tool response, every piece of retrieved text includes
> its full provenance. There are ZERO orphaned vectors -- every embedding can be
> traced back to the original document, page, and paragraph it came from.
> **If the provenance chain is broken, the data is useless.**

> **LEGAL CITATION MANDATE**: CaseTrack understands legal citation formats
> (Bluebook, case citations, statute references). Every search result includes
> provenance formatted for legal citation. Legal citations found in documents
> (e.g., "Smith v. Jones, 123 F.3d 456 (9th Cir. 2024)") are extracted,
> indexed, and cross-referenced across the case file.

---

## Document Index

This PRD is split across 10 documents. Each is self-contained but references the others.

| Doc | Title | Covers |
|-----|-------|--------|
| **01 (this)** | Overview | Executive summary, vision, principles, glossary |
| [02](PRD_02_TARGET_USER_HARDWARE.md) | Target User & Hardware | Users, hardware tiers, performance targets |
| [03](PRD_03_DISTRIBUTION_INSTALLATION.md) | Distribution & Installation | Channels, MCPB, manifest, install flows, updates |
| [04](PRD_04_STORAGE_ARCHITECTURE.md) | Storage Architecture | Local storage, RocksDB schema, data versioning |
| [05](PRD_05_EMBEDDER_STACK.md) | Embedder Stack | 4 legal-domain embedders (accuracy-first), ONNX, model management |
| [06](PRD_06_DOCUMENT_INGESTION.md) | Document Ingestion | PDF, DOCX, XLSX, OCR, legal-aware chunking |
| [07](PRD_07_CASE_MANAGEMENT.md) | Case Management & Provenance | Case model, isolation, legal references |
| [08](PRD_08_SEARCH_RETRIEVAL.md) | Search & Retrieval | 3-stage pipeline, RRF, legal-domain ranking |
| [09](PRD_09_MCP_TOOLS.md) | MCP Tools | All tool specs, examples, error responses |
| [10](PRD_10_TECHNICAL_BUILD.md) | Technical Build Guide | Bootstrap, crate structure, CI/CD, testing, security |

---

## 1. What is CaseTrack?

CaseTrack is a **one-click installable MCP server** that plugs into **Claude Code** and **Claude Desktop**, giving Claude the ability to ingest, search, and analyze **legal case files**. It supports PDF, DOCX, XLSX, and scanned images -- the standard formats found in legal case folders. Everything runs on the user's machine -- **all embeddings, vectors, and databases are stored locally** on the user's device with zero cloud dependencies.

CaseTrack is purpose-built for **legal professionals** -- solo attorneys, paralegals, small law firms, and legal departments. It organizes documents into **cases** (each case = an isolated database) and builds a legal knowledge graph that lets Claude answer questions about case files with full source provenance and legal citation formatting.

**Accuracy is the defining feature.** CaseTrack uses full-size legal-domain embedding models (Legal-BERT-base, 768D) trained on 12GB+ of legal corpora. It does NOT sacrifice accuracy for smaller model size. The target hardware is a 16GB RAM machine, which accommodates all models loaded simultaneously with headroom for large case databases.

```
+---------------------------------------------------------------------------+
|  CASETRACK -- "Accurate answers. Exact citations. On YOUR machine."       |
+---------------------------------------------------------------------------+
|  - Ingests case files: PDFs, DOCX, XLSX, scanned documents               |
|  - Embeds with FULL-SIZE legal-domain models (Legal-BERT-base 768D)      |
|  - 3-stage search: BM25 → Legal-BERT + SPLADE → ColBERT rerank          |
|  - Every answer cites document, page, paragraph, line, char offset       |
|  - Extracts legal entities: parties, courts, judges, statutes, case #s   |
|  - Cross-references citations across case documents                      |
|  - MCP server for Claude Code + Claude Desktop                           |
|  - Your data NEVER leaves your computer -- privilege preserved            |
+---------------------------------------------------------------------------+
```

---

## 2. The Problem

Legal professionals waste hours searching through case files:

- **Keyword search fails**: "breach of fiduciary duty" won't find "violation of duty of loyalty"
- **General-purpose AI misses legal nuance**: "consideration" means payment in contract law, not thoughtfulness
- **No provenance**: When you find something, you can't cite the exact source for court filings
- **Citation chaos**: Tracking which documents cite which cases, statutes, and regulations is manual
- **Complex tools**: Enterprise legal tech requires IT departments, training, and cloud subscriptions
- **Expensive**: Legal document platforms cost $500-2000+/seat/month (Relativity, Everlaw, etc.)
- **Scattered files**: Thousands of case documents spread across folders with no unified search
- **Confidentiality risk**: Cloud-based tools require uploading privileged attorney-client materials
- **Inaccurate results are dangerous**: A missed clause or wrong citation can mean malpractice

---

## 3. The Solution

CaseTrack solves this with:

1. **Accuracy first** -- full-size Legal-BERT-base (768D, 110M params) trained on 12GB of legal text; never downgraded for speed
2. **One-click install** -- single command or MCPB file, legal embedding models included
3. **100% local** -- all data stored on YOUR device in per-case RocksDB instances (attorney-client privilege preserved)
4. **3-stage search pipeline** -- BM25 recall → Legal-BERT + SPLADE semantic ranking → ColBERT precision reranking
5. **Full provenance** -- every answer cites source file, document name, page, paragraph, and line number
6. **Legal-aware chunking** -- clause-level for contracts, paragraph-level for briefs, Q&A grouping for depositions
7. **Legal citation extraction** -- Bluebook citations, case references, statute citations automatically indexed
8. **Legal entity extraction** -- parties, courts, judges, statutes, case numbers, legal concepts
9. **Claude Code + Desktop** -- works with both CLI and Desktop via MCP stdio
10. **Auto-sync** -- watches case folders for changes; optional scheduled reindexing
11. **Runs on 16GB hardware** -- no GPU needed; all models loaded simultaneously for maximum accuracy

---

## 4. Key Metrics

| Metric | Target |
|--------|--------|
| Install time | < 2 minutes |
| First search after install | < 5 minutes |
| Search latency (3-stage) | < 300ms on 16GB laptop |
| Search accuracy (top-5 recall) | > 90% on legal retrieval benchmarks |
| PDF ingestion | < 1.5 seconds per page |
| RAM usage (all models loaded) | < 3GB for models; < 16GB total peak |
| Model download | ~600MB one-time |
| Provenance accuracy | 100% -- every result traceable to source |

---

## 5. Vision Statement

> **Any legal professional can ask Claude questions about their case files and get accurate, cited answers -- without IT support, cloud accounts, or risking client confidentiality. Accuracy is never sacrificed.**

---

## 6. Design Principles

```
DESIGN PRINCIPLES
=================================================================================

1. ACCURACY ABOVE ALL (THE MOST IMPORTANT PRINCIPLE)
   Use the best legal-domain models that fit in 16GB RAM
   Never downgrade model quality to save RAM or speed
   3-stage search pipeline for maximum precision
   Legal-BERT-base (768D, 110M params) -- not the small variant
   ColBERT reranking for token-level precision on legal terminology
   If there's a tradeoff between speed and accuracy, choose accuracy

2. PROVENANCE ALWAYS (THE SECOND MOST IMPORTANT PRINCIPLE)
   Every answer includes exact source citation
   Document name, file path, page, paragraph, line number, character offsets
   Every embedding vector links back to its chunk, which links to its source
   Every entity, reference, and graph edge traces to its source chunk
   Timestamps on everything: when ingested, when embedded, when last synced
   One click to view original context
   If you can't cite the source, you can't return the information

3. ZERO CONFIGURATION
   User downloads file -> double-clicks -> starts using
   No terminal, no config files, no environment variables
   Claude Code: single curl command + one settings.json entry

4. RUNS ON 16GB HARDWARE
   16GB RAM laptop is the target (common for legal professionals)
   No GPU required, ever
   Intel, AMD, Apple Silicon all supported
   All models loaded simultaneously -- no lazy loading compromises

5. PRIVILEGE FIRST
   Documents never leave the device
   No telemetry, no analytics, no cloud
   Attorney-client privilege preserved by design
   License validation works offline after first activation

6. LEGAL-DOMAIN INTELLIGENCE
   Embedding models trained on legal corpora (case law, statutes, contracts)
   Legal entity extraction (parties, judges, courts, statutes, case numbers)
   Citation extraction and cross-referencing (Bluebook format)
   Legal document type awareness (pleadings, motions, briefs, discovery, contracts)
   Clause-aware chunking for contracts, paragraph-aware for briefs

7. GRACEFUL DEGRADATION
   8GB machine? Load models sequentially (slower but still accurate)
   Slow CPU? Longer ingestion, same quality -- never reduce model quality
   Free tier? ColBERT reranking disabled, 2-stage search (still Legal-BERT)

8. FAIL LOUDLY
   Errors are specific and actionable
   No silent failures -- every operation reports success or explains failure
   MCP error responses include recovery instructions
```

---

## 7. What CaseTrack is NOT

- **Not a document management system**: Use iManage/NetDocuments/SharePoint for storage
- **Not a cloud service**: Everything runs locally, we never see your data
- **Not an LLM**: CaseTrack provides tools to Claude; it does not generate answers itself
- **Not an eDiscovery platform**: CaseTrack indexes and searches documents; it does not handle production, privilege review, or coding workflows
- **Not a file sync tool**: CaseTrack indexes and searches documents; it does not replicate or sync files between devices
- **Not a practice management tool**: No billing, calendaring, or client intake -- only case file intelligence
- **Not a compromise on accuracy**: We will NOT ship smaller/faster models that produce worse results

---

## 8. Architecture at a Glance

```
+-----------------------------------------------------------------------+
|                         USER'S MACHINE (16GB RAM)                      |
+-----------------------------------------------------------------------+
|                                                                       |
|  +----------------------------+                                       |
|  | Claude Code / Desktop      |                                       |
|  |                            |                                       |
|  |  User asks a question      |                                       |
|  |        |                   |                                       |
|  |        v  MCP (stdio)      |                                       |
|  +--------+-------------------+                                       |
|           |                                                           |
|  +--------v-------------------+                                       |
|  | CaseTrack MCP Server       |   Single Rust binary                  |
|  |  (casetrack binary)        |   No runtime dependencies             |
|  |                            |                                       |
|  |  +----------+  +---------+ |                                       |
|  |  | Document |  | 3-Stage | |                                       |
|  |  | Parser   |  | Search  | |                                       |
|  |  | (PDF,    |  | Engine  | |                                       |
|  |  |  DOCX,   |  +---------+ |                                       |
|  |  |  XLSX)   |  +---------+ |                                       |
|  |  +----------+  | Legal   | |  Legal-BERT-base (768D, ~440MB)       |
|  |  +----------+  | ONNX    | |  SPLADE (~110MB)                     |
|  |  | Legal    |  | Models  | |  ColBERT (~220MB)                    |
|  |  | Chunker  |  | (~3GB)  | |  BM25 (algorithmic)                  |
|  |  +----------+  +---------+ |                                       |
|  |  +----------+              |                                       |
|  |  | Citation |              |                                       |
|  |  | Extract  |              |                                       |
|  |  +----------+              |                                       |
|  +--------+-------------------+                                       |
|           |                                                           |
|  +--------v-------------------+                                       |
|  | Local Storage              |   ~/Documents/CaseTrack/              |
|  |  +---------+ +-----------+ |                                       |
|  |  | Case A  | | Case B    | |   Each case = isolated RocksDB       |
|  |  | RocksDB | | RocksDB   | |   Vectors, chunks, provenance        |
|  |  +---------+ +-----------+ |                                       |
|  +----------------------------+                                       |
|                                                                       |
|  NOTHING LEAVES THIS MACHINE -- PRIVILEGE PRESERVED                   |
+-----------------------------------------------------------------------+
```

---

## 9. Technology Summary

| Component | Technology | Why |
|-----------|------------|-----|
| Language | Rust | Single binary, no runtime, cross-platform |
| MCP SDK | rmcp | Official Rust MCP SDK, stdio transport |
| Storage | RocksDB | Embedded KV store, zero-config, local disk |
| ML Inference | ONNX Runtime | CPU-optimized, cross-platform, quantized INT8 |
| Legal Embedder (E1) | Legal-BERT-base (nlpaueb, 768D, 110M params) | Full-size model trained on 12GB legal text -- accuracy first |
| Sparse Retrieval (E6) | SPLADE-distil | Keyword expansion handles legal synonyms |
| Reranking (E12) | ColBERT-v2 | Token-level matching for precise legal terminology |
| Keyword Recall (E13) | BM25 | Fast lexical recall, zero model overhead |
| PDF | pdf-extract + lopdf | Pure Rust |
| DOCX | docx-rs | Pure Rust |
| XLSX | calamine | Pure Rust spreadsheet reader (XLS, XLSX, ODS) |
| OCR | Tesseract (bundled) | Best open-source OCR |
| Model Download | hf-hub | Hugging Face model registry |
| Serialization | bincode + serde | Fast binary serialization for vectors |
| Async | tokio | Standard Rust async runtime |
| File watching | notify | Cross-platform OS file notifications |
| CLI | clap | Standard Rust CLI parsing |
| Logging | tracing | Structured logging with subscriber |
| License | ed25519-dalek | Offline cryptographic validation |
| Build/Release | cargo-dist | Cross-platform binary distribution |
| CI | GitHub Actions | Multi-platform CI/CD |

---

## 10. Glossary

| Term | Definition |
|------|------------|
| **BM25** | Best Match 25 -- classical keyword ranking algorithm |
| **Bluebook** | The standard legal citation format in the United States (e.g., "Smith v. Jones, 123 F.3d 456 (9th Cir. 2024)") |
| **Case** | A legal matter containing related documents stored in an isolated database. Each case has its own RocksDB instance, embeddings, and knowledge graph. |
| **Case Map** | A per-case summary structure containing key parties, important dates, core legal issues, top citations, entity statistics, and document counts. Built incrementally during ingestion. |
| **Chunk** | A segment of a document (target: 2000 chars for general text, clause-level for contracts), the unit of search. Every chunk stores full provenance. |
| **Context Graph** | The graph layer built on top of chunks and embeddings that stores entities, references, document relationships, chunk similarity edges, and the case map. |
| **Document Graph** | Relationship edges between documents based on shared entities, shared citations, semantic similarity, or explicit references. |
| **Embedder** | A model that converts text to a numerical vector |
| **Entity** | A named thing extracted from document text: party, court, judge, attorney, statute, case number, date, monetary amount, legal concept. |
| **Knowledge Graph** | The combined structure of entities, references, document relationships, and chunk similarity edges within a case. |
| **Legal-BERT** | A BERT model (110M params, 768D) pre-trained on 12GB of legal text. The PRIMARY embedder -- chosen for accuracy, not size. |
| **MCP** | Model Context Protocol -- standard for AI tool integration |
| **MCPB** | MCP Bundle -- a ZIP file format for distributing MCP servers |
| **ONNX** | Open Neural Network Exchange -- cross-platform ML model format |
| **Provenance** | The exact source location of text: file path, document name, page number, paragraph number, line number, and character offsets. |
| **Reference Network** | The graph of cross-references between documents -- which documents cite the same cases, statutes, or regulations. |
| **RocksDB** | Embedded key-value database by Meta, used for local storage |
| **RRF** | Reciprocal Rank Fusion -- method to combine search rankings |
| **rmcp** | Official Rust MCP SDK |
| **SPLADE** | Sparse Lexical and Expansion Model -- keyword expansion embedder. Expands "tort" to also match "negligence", "liability", "damages". |
| **stdio** | Standard input/output transport for MCP server communication |

---

*CaseTrack PRD v5.1.0 -- Document 1 of 10*

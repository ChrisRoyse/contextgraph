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


---

# PRD 02: Target User & Hardware

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust | **Domain**: Legal
**Design Priority**: Accuracy first -- 16GB RAM budget allows full-size legal models

---

## 1. Target Users

| Segment | Profile | Key Pain Point | Why CaseTrack |
|---------|---------|---------------|---------------|
| **Primary: Solo Attorneys & Small Firms** (1-5 attorneys) | No IT staff, 16GB laptop, 10-50 active cases, already uses Claude | Can't semantically search case files; no budget for Relativity/Everlaw ($500+/seat); must preserve attorney-client privilege | Works on existing laptop, no cloud upload, full-accuracy legal models, $29/mo or free tier |
| **Secondary: Paralegals & Legal Assistants** | Standard office hardware (16GB), manages document organization for attorneys, needs to find specific clauses and citations quickly | Manual document review for case prep is tedious; need exact citations for briefs and filings | Batch ingest case folders, search returns cited sources, full-size legal embedders for maximum accuracy |
| **Tertiary: In-House Legal Departments** (5-20 people) | 16-32GB hardware, corporate counsel handling contracts, compliance, and litigation | Organizing contract portfolios, finding cross-document obligations, tracking regulatory citations across hundreds of agreements | Full 3-stage search pipeline, legal entity extraction, cross-document citation network |

---

## 2. User Personas

| Persona | Role / Domain | Hardware | CaseTrack Use | Key Need |
|---------|--------------|----------|---------------|----------|
| **Maria** | Solo litigator, personal injury | MacBook Pro M2, 16GB | Ingests all case documents (medical records, depositions, police reports), asks Claude to find relevant evidence for motions | Accuracy -- cannot miss a relevant deposition passage; privilege preservation |
| **David** | Senior paralegal at boutique firm (8 attorneys) | Windows 11, 16GB | Manages 30+ active cases, searches for specific contract clauses, cross-references depositions with exhibits | Precision search across large case files, exact citations for attorney review |
| **Sarah** | Corporate counsel, mid-size company | Windows 11, 32GB | Batch ingests 200+ vendor contracts, searches for indemnification clauses, tracks regulatory compliance requirements | Contract-aware chunking, entity extraction for party names, cross-reference network |

---

## 3. Hardware Requirements

```
RECOMMENDED HARDWARE (Full Accuracy)
=================================================================================

CPU:     Any 64-bit processor (2018 or newer recommended)
         - Intel Core i5 or better
         - AMD Ryzen 5 or better
         - Apple M1 or better

RAM:     16GB RECOMMENDED (all models loaded simultaneously for best accuracy)
         - 8GB minimum (models loaded sequentially -- slower but same accuracy)
         - 32GB ideal for large cases (5000+ pages)

Storage: 5GB available
         - 600MB for legal embedding models (one-time download)
         - 4.4GB for case data (scales with usage)
         - SSD strongly recommended (HDD works but slower ingestion)

OS:      - macOS 11 (Big Sur) or later
         - Windows 10 (64-bit) or later
         - Ubuntu 20.04 or later (other Linux distros likely work)

GPU:     NOT REQUIRED
         - Optional: Metal (macOS), CUDA (NVIDIA), DirectML (Windows)
         - GPU provides ~2x speedup for ingestion if available
         - Search latency unaffected (small batch sizes)

Network: Required ONLY for:
         - Initial model download (~600MB, one-time)
         - License activation (one-time, then cached offline)
         - Software updates (optional)
         ALL document processing is 100% offline

Prerequisites:
         - Claude Code or Claude Desktop installed
         - No other runtime dependencies (Rust binary is self-contained)
         - Tesseract OCR bundled with binary (no separate install)

WHY 16GB:
         CaseTrack prioritizes ACCURACY over minimal RAM usage.
         Legal-BERT-base (768D, 110M params) is 4x more accurate on
         legal retrieval tasks than the small variant (512D, 35M params).
         With 16GB, all 3 neural models + BM25 + RocksDB run simultaneously
         with no lazy loading, no model swapping, no accuracy compromises.
         8GB machines still work -- models load sequentially, same quality
         per query, just slower between model switches.
```

---

## 4. Performance by Hardware Tier

### 4.1 Ingestion Performance

| Hardware | 50-page PDF | 500-page PDF | OCR (50 scanned pages) |
|----------|-------------|--------------|------------------------|
| **8GB** (M1 Air) | 60 seconds | 10 minutes | 4 minutes |
| **16GB** (M2 Pro) | 30 seconds | 5 minutes | 2 minutes |
| **32GB** (i7 desktop) | 20 seconds | 3 minutes | 90 seconds |
| **With GPU** (RTX 3060) | 10 seconds | 90 seconds | 45 seconds |

### 4.2 Search Performance

| Hardware | Free Tier (2-stage) | Pro Tier (3-stage) | All Models Loaded |
|----------|--------------------|--------------------|-------------------|
| **8GB** (M1 Air) | 150ms | 300ms | No -- sequential loading |
| **16GB** (M2 Pro) | 80ms | 180ms | Yes -- all 3 + BM25 |
| **32GB** (i7 desktop) | 50ms | 120ms | Yes -- all 3 + BM25 |
| **With GPU** (RTX 3060) | 30ms | 70ms | Yes -- all 3 + BM25 |

### 4.3 Memory Usage

| Scenario | RAM Usage |
|----------|-----------|
| Idle (server running, no models loaded) | ~50MB |
| Legal-BERT-base loaded | ~900MB |
| All models loaded (E1 + E6 + E12 + BM25) | ~2.5GB |
| During ingestion (peak, all models) | ~3.5GB |
| During search (peak, all models) | ~3.0GB |
| RocksDB per open case (typical) | ~64MB |
| **Total peak (search + 2 cases open)** | **~3.2GB** |
| **Available for OS + Claude + other apps** | **~12.8GB on 16GB machine** |

---

## 5. Supported Platforms

### 5.1 Build Targets

| Platform | Architecture | Binary Name | Status |
|----------|-------------|-------------|--------|
| macOS | x86_64 (Intel) | `casetrack-darwin-x64` | Supported |
| macOS | aarch64 (Apple Silicon) | `casetrack-darwin-arm64` | Supported |
| Windows | x86_64 | `casetrack-win32-x64.exe` | Supported |
| Linux | x86_64 | `casetrack-linux-x64` | Supported |
| Linux | aarch64 | `casetrack-linux-arm64` | Future |

### 5.2 Platform-Specific Notes

**macOS:**
- CoreML execution provider available for ~2x inference speedup
- Universal binary option (fat binary for Intel + Apple Silicon)
- Code signing required for Gatekeeper (`codesign --sign`)
- Notarization required for distribution outside App Store

**Windows:**
- DirectML execution provider available for GPU acceleration
- Binary should be signed with Authenticode certificate
- Windows Defender may flag unsigned binaries
- Long path support: use `\\?\` prefix or registry setting

**Linux:**
- CPU-only by default; CUDA available if NVIDIA drivers present
- Statically linked against musl for maximum compatibility
- AppImage format as alternative distribution

### 5.3 Claude Integration Compatibility

| Client | Transport | Config Location | Status |
|--------|-----------|-----------------|--------|
| Claude Code (CLI) | stdio | `~/.claude/settings.json` | Primary target |
| Claude Desktop (macOS) | stdio | `~/Library/Application Support/Claude/claude_desktop_config.json` | Supported |
| Claude Desktop (Windows) | stdio | `%APPDATA%\Claude\claude_desktop_config.json` | Supported |
| Claude Desktop (Linux) | stdio | `~/.config/Claude/claude_desktop_config.json` | Supported |

---

## 6. Graceful Degradation Strategy

| Tier | RAM | Models Loaded | Behavior |
|------|-----|---------------|----------|
| **Full** | 16GB+ | All 3 neural models + BM25 simultaneously | Zero load latency, parallel embedding, maximum accuracy |
| **Standard** | 8-16GB | Legal-BERT + BM25 always; SPLADE + ColBERT lazy-loaded | ~300ms first-use penalty per model; **same accuracy per query** -- models just load on demand |
| **Constrained** | <8GB | Legal-BERT + BM25 only; others loaded one-at-a-time, unloaded after use | Sequential embedding, higher search latency, startup warning. **Still uses full-size Legal-BERT-base** -- never downgrades model quality |

**Key principle**: Degradation affects **speed**, never **accuracy**. Even on 8GB, CaseTrack uses Legal-BERT-base (768D, 110M params). It just loads models one at a time instead of all at once.

**Detection**: On startup, check available RAM via `sysinfo` crate. Set tier automatically, log the decision. User override: `--memory-mode=full|standard|constrained`.

---

*CaseTrack PRD v5.1.0 -- Document 2 of 10*


---

# PRD 03: Distribution & Installation

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust | **Domain**: Legal
**Design Priority**: ACCURACY FIRST -- legal-domain models downloaded on first use

---

## 1. Distribution Channels

```
DISTRIBUTION CHANNELS (Priority Order)
=================================================================================

1. CLAUDE CODE (Primary - Recommended)
   ────────────────────────────────
   # macOS/Linux - One command:
   curl -fsSL https://casetrack.dev/install.sh | sh

   # Windows - PowerShell:
   irm https://casetrack.dev/install.ps1 | iex

   # Or via cargo:
   cargo binstall casetrack   # Pre-compiled binary (fast)
   cargo install casetrack    # From source (slow, needs Rust toolchain)

   # Then add to Claude Code settings (~/.claude/settings.json):
   {
     "mcpServers": {
       "casetrack": {
         "command": "casetrack",
         "args": ["--data-dir", "~/Documents/CaseTrack"]
       }
     }
   }

2. CLAUDE DESKTOP (Secondary)
   ────────────────────────────────
   - Download casetrack.mcpb from website
   - Double-click or drag to Claude Desktop window
   - Click "Install" in dialog
   - Done (same binary, just packaged for GUI install)

3. PACKAGE MANAGERS (Tertiary)
   ────────────────────────────────
   macOS:     brew install casetrack
   Windows:   winget install CaseTrack.CaseTrack
   Linux:     cargo binstall casetrack

4. GITHUB RELEASES (Developer)
   ────────────────────────────────
   - Pre-built binaries attached to GitHub releases
   - SHA256 checksums for verification
   - Source tarball for audit
```

---

## 2. Install Script Specification

### 2.1 macOS/Linux Install Script (`install.sh`)

```
Steps:
1. Detect platform (darwin/linux) and architecture (x86_64/arm64/aarch64)
2. Map to binary name: casetrack-{os}-{arch}
   Supported: darwin-arm64, darwin-x64, linux-x64, linux-arm64
3. Download binary from GitHub releases to ~/.local/bin/casetrack
4. Add ~/.local/bin to PATH via .zshrc or .bashrc (if not already present)
5. If ~/.claude/ exists, run: casetrack --setup-claude-code
6. Print success with next steps (restart terminal, try a command)
```

### 2.2 Windows Install Script (`install.ps1`)

```
Steps:
1. Require 64-bit Windows
2. Download binary from GitHub releases to %LOCALAPPDATA%\CaseTrack\casetrack.exe
3. Add install dir to user PATH
4. Run: casetrack.exe --setup-claude-code
5. Print success with next steps
```

---

## 3. Self-Setup CLI Command

```
casetrack --setup-claude-code
```

Reads or creates `~/.claude/settings.json`, merges `mcpServers.casetrack` entry (using the current binary path and configured data-dir), writes back with pretty JSON formatting.

---

## 4. MCPB Bundle Structure

`.mcpb` = ZIP archive (~50MB) for Claude Desktop GUI install. Contains platform binaries, manifest, icon, and shared resources (tokenizer, vocabulary).

### 4.1 Manifest Specification

```json
{
  "manifest_version": "1.0",
  "name": "casetrack",
  "version": "5.1.0",
  "display_name": "CaseTrack Legal Case Intelligence",
  "description": "Legal case management intelligence. Ingest case documents -- complaints, motions, briefs, depositions, contracts, orders. Search with AI using legal-domain models. Every answer cites the source. Attorney-client privilege preserved -- 100% local.",

  "author": {
    "name": "CaseTrack",
    "url": "https://casetrack.dev"
  },

  "server": {
    "type": "binary",
    "entry_point": "server/casetrack"
  },

  "compatibility": {
    "platforms": ["darwin", "win32", "linux"]
  },

  "user_config": [
    {
      "id": "data_dir",
      "name": "Data Location",
      "description": "Where to store cases and models on your computer",
      "type": "directory",
      "default": "${DOCUMENTS}/CaseTrack",
      "required": true
    },
    {
      "id": "license_key",
      "name": "License Key (Optional)",
      "description": "Leave blank for free tier. Purchase at casetrack.dev",
      "type": "string",
      "sensitive": true,
      "required": false
    }
  ],

  "mcp_config": {
    "command": "server/casetrack",
    "args": ["--data-dir", "${user_config.data_dir}"],
    "env": {
      "CASETRACK_LICENSE": "${user_config.license_key}",
      "CASETRACK_HOME": "${user_config.data_dir}"
    }
  },

  "platform_overrides": {
    "darwin-arm64": {
      "mcp_config": { "command": "server/casetrack-darwin-arm64" }
    },
    "darwin-x64": {
      "mcp_config": { "command": "server/casetrack-darwin-x64" }
    },
    "win32": {
      "mcp_config": { "command": "server/casetrack-win32-x64.exe" }
    }
  },

  "permissions": {
    "filesystem": {
      "read": ["${user_config.data_dir}"],
      "write": ["${user_config.data_dir}"]
    },
    "network": {
      "domains": ["huggingface.co"],
      "reason": "Download legal embedding models on first use"
    }
  },

  "icons": { "256": "icon.png" },
  "tools": { "_generated": true }
}
```

---

## 5. Installation Flow (Claude Desktop)

```
1. Download: User downloads casetrack.mcpb (~50MB) from casetrack.dev
2. Install:  Double-click .mcpb, drag to Claude Desktop, or Settings > Extensions > Install
3. Configure: Dialog prompts for data location and optional license key
   +-------------------------------------------------------+
   | Install CaseTrack?                                     |
   |                                                        |
   | CaseTrack is legal case management intelligence.       |
   | Ingest case documents, search with legal-domain AI.    |
   | Attorney-client privilege preserved -- 100% local.     |
   | All processing happens on your computer.               |
   |                                                        |
   | Data Location:  [~/Documents/CaseTrack            ] [F]|
   | License Key:    [optional - blank for free tier   ] [L]|
   |                                                        |
   | [Y] Read and write files in your Data Location        |
   | [Y] Download legal AI models from huggingface.co      |
   |     (~550MB: Legal-BERT, SPLADE, ColBERT)             |
   | [N] NOT send your documents anywhere                  |
   |                                                        |
   |                         [Cancel]  [Install Extension]  |
   +-------------------------------------------------------+
4. First Run: Server starts, downloads missing models (~550MB) in background
5. Ready:     CaseTrack icon appears in Extensions panel
```

---

## 6. First-Run Experience

Initialization sequence on first launch:

```
1. Create directory structure: models/, cases/ (registry.db created by RocksDB::open)
2. Check for missing models based on tier; download any missing (with progress logging)
3. Open or create registry database
4. Validate license (offline-first)
5. Log ready state: tier + case count
```

### 6.1 Model Download Strategy

Models are NOT bundled (would make `.mcpb` too large). Downloaded on first use:

```rust
pub struct ModelSpec {
    pub id: &'static str,
    pub repo: &'static str,
    pub files: &'static [&'static str],
    pub size_mb: u32,
    pub required: bool,  // false = only download for Pro tier
}

pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "e1",
        repo: "nlpaueb/legal-bert-base-uncased",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 220,
        required: true,
    },
    ModelSpec {
        id: "e6",
        repo: "naver/splade-cocondenser-selfdistil",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 110,
        required: true,
    },
    ModelSpec {
        id: "e12",
        repo: "colbert-ir/colbertv2.0",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 220,
        required: false,
    },
];
// E13 (BM25) requires no model download -- pure algorithm
```

**Model download totals:**
- Free tier: ~330MB (Legal-BERT-base + SPLADE)
- Pro tier: ~550MB (Legal-BERT-base + SPLADE + ColBERT-v2)

### 6.2 Download Resilience

- Skip files already downloaded with valid checksums
- Retry up to 3 attempts with exponential backoff (2s, 4s, 8s)
- Fatal error after 3 failures for any single file

---

## 7. Update Mechanism

### 7.1 Version Checking

Non-blocking check on startup (fire-and-forget via `tokio::spawn`):
- Queries GitHub releases API for latest version
- Compares via semver; logs update notice if newer version exists
- Silently ignores failures (offline, rate-limited)

### 7.2 Self-Update Command

```
casetrack --update
```

Downloads the latest binary and replaces itself:

1. Download new binary to temporary path
2. Verify SHA256 checksum
3. Replace current binary (platform-specific swap)
4. Print success message

On Windows, use the "rename on restart" pattern since running binaries can't be replaced directly.

### 7.3 Data Migration

When a new version introduces schema changes:

1. On startup, check `schema_version` in `registry.db`
2. If schema is older, run migration functions sequentially
3. Migrations are idempotent (safe to re-run)
4. Back up existing DB before migration (copy `registry.db` to `registry.db.bak.{version}`)

See [PRD 04: Storage Architecture](PRD_04_STORAGE_ARCHITECTURE.md) for schema versioning details.

---

## 8. Uninstallation

### 8.1 CLI Uninstall

```
casetrack --uninstall
```

This command:
1. Asks for confirmation ("This will remove CaseTrack. Your case data will NOT be deleted.")
2. Removes the binary from PATH
3. Removes the Claude Code/Desktop configuration entry
4. Prints location of data directory for manual cleanup
5. Does NOT delete `~/Documents/CaseTrack/` (user's case data is sacred)

### 8.2 Manual Uninstall

```
# Remove binary
rm ~/.local/bin/casetrack   # macOS/Linux
# OR delete %LOCALAPPDATA%\CaseTrack\ on Windows

# Remove Claude Code config (edit ~/.claude/settings.json, remove "casetrack" key)

# Optionally remove data (YOUR CHOICE -- this deletes all cases):
rm -rf ~/Documents/CaseTrack/
```

---

*CaseTrack PRD v5.1.0 -- Document 3 of 10*


---

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


---

# PRD 05: 4-Embedder Stack (Legal Domain -- Accuracy First)

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust | **Domain**: Legal
**Design Priority**: ACCURACY FIRST -- use the best legal models that fit in 16GB RAM

---

## 1. Design Philosophy

The embedder stack is designed for **maximum legal retrieval accuracy** within a 16GB RAM budget:

- **Full-size Legal-BERT-base (768D, 110M params)** -- NOT the small variant; accuracy over size
- **4 embedders** (3 neural + 1 algorithmic): Each chosen for best-in-class legal text performance
- **768D embeddings**: Full dimensionality for maximum semantic resolution on legal language
- **ONNX format**: CPU-optimized, cross-platform
- **Quantized (INT8)**: 50% smaller storage without meaningful accuracy loss
- **No LLM inference**: No GPU-requiring models
- **All models loaded simultaneously on 16GB**: No lazy loading compromises on target hardware

### Why Full-Size Legal-BERT-base (Not Small)?

| Property | legal-bert-small | legal-bert-base | Accuracy Impact |
|----------|-----------------|-----------------|-----------------|
| Parameters | 35M | **110M** | 3x more parameters = richer legal representations |
| Dimensions | 512 | **768** | 50% more dimensions = finer semantic distinctions |
| Layers | 6 | **12** | 2x deeper = better contextual understanding |
| RAM (ONNX INT8) | ~70MB | **~220MB** | +150MB is trivial on 16GB machine |
| Legal synonym recall | Good | **Excellent** | base catches "breach of fiduciary duty" ↔ "violation of duty of loyalty" |
| Legal concept precision | Good | **Excellent** | base distinguishes "preponderance of evidence" from "beyond reasonable doubt" |

**The small model saves 150MB RAM but loses measurable accuracy on legal retrieval tasks. On a 16GB machine, there is no reason to use it. Accuracy is the #1 priority.**

### Why Legal-Domain Models (Not General-Purpose)?

General-purpose embedding models (like bge-small) treat legal text as ordinary English. They miss:

- **Legal synonyms**: "breach of fiduciary duty" ≠ "violation of duty of loyalty" (to a general model)
- **Legal entities**: "Miranda" is a landmark case, not a person's name in most contexts
- **Term precision**: "consideration" means "payment/exchange" in contract law, not "thoughtfulness"
- **Citation semantics**: "42 U.S.C. § 1983" is a civil rights statute, not a random number
- **Legal phrasing**: "notwithstanding the foregoing" is a scope limiter, not noise

Legal-BERT was pre-trained on 12GB of diverse legal text including:
- UK legislation (parliament.uk)
- EU legislation (Eurlex)
- European Court of Human Rights case law
- US court cases and opinions
- US contracts and agreements

This domain-specific pre-training produces embeddings that understand legal language natively.

---

## 2. Embedder Specifications

### E1: Legal Semantic Similarity (PRIMARY -- ACCURACY-CRITICAL)

| Property | Value |
|----------|-------|
| Model | **legal-bert-base-uncased** (nlpaueb) |
| Dimension | **768** |
| Parameters | **110M** |
| Size | ~220MB (INT8 ONNX) |
| Speed | 60ms/chunk (M2), 120ms/chunk (Intel i5) |
| Tier | FREE (accuracy is not a paid feature) |
| Purpose | Core legal semantic search -- THE accuracy foundation |
| Training Data | 12GB legal text: UK/EU/US legislation, ECHR court cases, US contracts |
| HuggingFace | nlpaueb/legal-bert-base-uncased |
| License | CC BY-SA 4.0 |

**What it finds**: "breach of fiduciary duty" matches "violation of duty of loyalty"
**What general models miss**: Legal synonyms, legal concepts, citation-related semantics
**Role in pipeline**: Foundation embedder. All search queries start here. Stage 2 dense ranking.

### E6: Keyword Expansion (SPLADE)

| Property | Value |
|----------|-------|
| Model | SPLADE-cocondenser-selfdistil (Naver) |
| Dimension | Sparse (30K vocabulary) |
| Parameters | ~110M |
| Size | ~110MB (INT8 ONNX) |
| Speed | 30ms/chunk |
| Tier | FREE |
| Purpose | Exact legal term matching + expansion |

**What it finds**: "negligence" also matches "tortious conduct", "duty of care", "breach of duty"
**Why SPLADE for legal**: Legal text is terminology-heavy. Attorneys search for exact terms ("indemnification", "force majeure", "liquidated damages"). SPLADE preserves exact keyword matching while expanding to related legal terms.
**Role in pipeline**: Stage 2 sparse ranking alongside E1. Catches exact legal terminology E1 misses.

**Note**: No legal-domain-specific SPLADE exists. The general SPLADE model performs well because its term expansion naturally handles legal synonyms through learned vocabulary relationships.

### E12: Precision Reranking (ColBERT)

| Property | Value |
|----------|-------|
| Model | ColBERT-v2 |
| Dimension | 128 per token |
| Parameters | ~110M |
| Size | ~220MB (INT8 ONNX) |
| Speed | 150ms for top 50 candidates |
| Tier | PRO |
| Purpose | Final reranking for exact legal phrase matches -- ACCURACY MAXIMIZER |

**What it finds**: "beyond a reasonable doubt" ranks correctly against "preponderance of the evidence"
**Why ColBERT for legal**: Token-level matching is critical for legal precision. Legal standards like "beyond a reasonable doubt" vs. "preponderance of the evidence" differ by just a few words but mean completely different things. ColBERT's per-token MaxSim scoring distinguishes them where single-vector models cannot.
**Role in pipeline**: Stage 3 (final rerank). Token-level MaxSim scoring. Only runs on top 50 candidates from Stage 2. This is where accuracy gets its final boost.

### E13: Fast Recall (BM25)

| Property | Value |
|----------|-------|
| Model | None (algorithmic -- BM25/TF-IDF) |
| Dimension | N/A (inverted index) |
| Size | ~2MB index per 1000 documents |
| Speed | <5ms for any query |
| Tier | FREE |
| Purpose | Fast initial candidate retrieval -- ensures no relevant document is missed |

**What it finds**: Exact keyword matches for legal terms like "indemnification", "force majeure", "42 U.S.C. § 1983"
**Role in pipeline**: Stage 1. Retrieves initial 500 candidates from inverted index. BM25 ensures high recall -- no relevant document is filtered out before the neural models get to score it.

---

## 3. Footprint Summary

| Metric | Free Tier | Pro Tier |
|--------|-----------|----------|
| Models to download | 2 (E1, E6) | 3 (+ E12) |
| Model disk space | ~330MB | ~550MB |
| RAM (all loaded) | ~1.5GB | ~2.5GB |
| Per-chunk embed time | ~90ms | ~240ms |
| Search latency (full pipeline) | <150ms (2-stage) | <300ms (3-stage) |

### RAM Budget (16GB Machine)

```
MEMORY BUDGET -- 16GB TARGET
=================================================================================

Component                          RAM         Notes
─────────────────────────────────────────────────────────────────────────
Legal-BERT-base (E1, 768D)        ~900MB      Always loaded (primary)
SPLADE (E6, sparse)               ~450MB      Always loaded (Stage 2)
ColBERT-v2 (E12, 128D/token)      ~900MB      Always loaded (Pro)
BM25 inverted index                ~50MB       Per-case, in memory
RocksDB (2 cases open)            ~128MB       Block cache per DB
Application overhead               ~200MB       Binary + runtime
─────────────────────────────────────────────────────────────────────────
TOTAL CASETRACK                    ~2.6GB      (Pro tier, 2 cases open)
OS + Claude + other apps          ~13.4GB      Comfortable headroom

On 8GB machine: Load E1 + BM25 always (~1GB), lazy-load E6/E12 as needed.
Same accuracy per query -- just slower model switching.
```

---

## 4. Provenance Linkage

**Every embedding vector is traceable back to its source document, page, and paragraph.** The chain is: `embedding key (e1:{chunk_uuid})` -> `ChunkData` (text + full `Provenance`) -> source file on disk. No embedding is stored without its chunk existing first; the ingestion pipeline (PRD 06) creates ChunkData with full Provenance before calling `embed_chunk()`.

For the canonical Provenance struct fields, storage layout, and complete chain specification, see [PRD 04 Section 5.2](PRD_04_STORAGE_ARCHITECTURE.md#52-the-provenance-chain-how-embeddings-trace-back-to-source).

---

## 5. Embedding Engine Implementation

```rust
use ort::{Session, Environment, GraphOptimizationLevel, ExecutionProvider};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Legal-domain embedding engine -- accuracy-first design
pub struct EmbeddingEngine {
    env: Arc<Environment>,
    models: HashMap<EmbedderId, Option<Session>>,
    tier: LicenseTier,
    model_dir: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EmbedderId {
    E1,     // Legal-BERT-base Semantic (FREE) -- 768D, accuracy-critical
    E6,     // SPLADE Keywords (FREE)
    E12,    // ColBERT Rerank (PRO) -- accuracy maximizer
    // E13 is BM25, not a neural model
}

impl EmbedderId {
    pub fn model_dir_name(&self) -> &'static str {
        match self {
            Self::E1 => "legal-bert-base",       // Full-size, not small
            Self::E6 => "splade-distil",
            Self::E12 => "colbert-v2",           // Full ColBERT-v2
        }
    }

    pub fn dimension(&self) -> usize {
        match self {
            Self::E1 => 768,   // Full 768D -- accuracy first
            Self::E6 => 0,     // Sparse
            Self::E12 => 128,  // Per token -- full ColBERT dimensionality
        }
    }

    pub fn is_sparse(&self) -> bool {
        matches!(self, Self::E6)
    }

    pub fn is_free_tier(&self) -> bool {
        matches!(self, Self::E1 | Self::E6)
    }
}

impl EmbeddingEngine {
    pub fn new(model_dir: &Path, tier: LicenseTier) -> Result<Self> {
        let env = Environment::builder()
            .with_name("casetrack")
            .with_execution_providers([
                #[cfg(target_os = "macos")]
                ExecutionProvider::CoreML(Default::default()),
                #[cfg(target_os = "windows")]
                ExecutionProvider::DirectML(Default::default()),
                ExecutionProvider::CPU(Default::default()),
            ])
            .build()?;

        let mut engine = Self {
            env: Arc::new(env),
            models: HashMap::new(),
            tier,
            model_dir: model_dir.to_path_buf(),
        };

        // Load ALL models for the tier -- no lazy loading on 16GB
        for id in Self::models_for_tier(tier) {
            engine.load_model(id)?;
        }

        Ok(engine)
    }

    fn load_model(&mut self, id: EmbedderId) -> Result<()> {
        let path = self.model_dir
            .join(id.model_dir_name())
            .join("model.onnx");

        if path.exists() {
            let session = Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(4)?  // Use more threads -- accuracy over battery
                .with_model_from_file(&path)?;
            self.models.insert(id, Some(session));
        } else {
            self.models.insert(id, None);  // Will download on demand
        }
        Ok(())
    }

    fn models_for_tier(tier: LicenseTier) -> Vec<EmbedderId> {
        match tier {
            LicenseTier::Free => vec![
                EmbedderId::E1,     // Legal-BERT-base (always)
                EmbedderId::E6,     // SPLADE (always)
            ],
            _ => vec![
                EmbedderId::E1,     // Legal-BERT-base (always)
                EmbedderId::E6,     // SPLADE (always)
                EmbedderId::E12,    // ColBERT (Pro -- accuracy maximizer)
            ],
        }
    }

    /// Embed a chunk with all active models
    pub fn embed_chunk(&self, text: &str) -> Result<ChunkEmbeddings> {
        let mut embeddings = ChunkEmbeddings::default();

        for (id, session) in &self.models {
            if let Some(session) = session {
                match id {
                    EmbedderId::E6 => {
                        embeddings.e6 = Some(self.run_sparse_inference(session, text)?);
                    }
                    EmbedderId::E12 => {
                        embeddings.e12 = Some(self.run_token_inference(session, text)?);
                    }
                    _ => {
                        let vec = self.run_dense_inference(session, text)?;
                        match id {
                            EmbedderId::E1 => embeddings.e1 = Some(vec),
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(embeddings)
    }

    /// Embed a query
    pub fn embed_query(&self, query: &str, embedder: EmbedderId) -> Result<QueryEmbedding> {
        let session = self.models.get(&embedder)
            .ok_or(CaseTrackError::EmbedderNotLoaded(embedder))?
            .as_ref()
            .ok_or(CaseTrackError::ModelNotDownloaded(embedder))?;

        match embedder {
            EmbedderId::E6 => {
                Ok(QueryEmbedding::Sparse(self.run_sparse_inference(session, query)?))
            }
            EmbedderId::E12 => {
                Ok(QueryEmbedding::Token(self.run_token_inference(session, query)?))
            }
            _ => {
                Ok(QueryEmbedding::Dense(self.run_dense_inference(session, query)?))
            }
        }
    }

    fn run_dense_inference(&self, session: &Session, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenize(text, 512)?;  // Max 512 tokens

        let outputs = session.run(ort::inputs![
            "input_ids" => tokens.input_ids,
            "attention_mask" => tokens.attention_mask,
        ]?)?;

        let hidden = outputs["last_hidden_state"].extract_tensor::<f32>()?;
        Ok(mean_pool(&hidden, &tokens.attention_mask))
    }

    fn run_sparse_inference(&self, session: &Session, text: &str) -> Result<SparseVec> {
        let tokens = self.tokenize(text, 512)?;

        let outputs = session.run(ort::inputs![
            "input_ids" => tokens.input_ids,
            "attention_mask" => tokens.attention_mask,
        ]?)?;

        let logits = outputs["logits"].extract_tensor::<f32>()?;
        Ok(splade_max_pool(&logits, &tokens.attention_mask))
    }

    fn run_token_inference(&self, session: &Session, text: &str) -> Result<TokenEmbeddings> {
        let tokens = self.tokenize(text, 512)?;

        let outputs = session.run(ort::inputs![
            "input_ids" => tokens.input_ids,
            "attention_mask" => tokens.attention_mask,
        ]?)?;

        let hidden = outputs["last_hidden_state"].extract_tensor::<f32>()?;
        Ok(extract_token_embeddings(&hidden, &tokens.attention_mask))
    }
}

/// Embeddings for a single chunk
#[derive(Default)]
pub struct ChunkEmbeddings {
    pub e1: Option<Vec<f32>>,           // 768D (Legal-BERT-base -- full size)
    pub e6: Option<SparseVec>,          // Sparse (SPLADE)
    pub e12: Option<TokenEmbeddings>,   // 128D per token (ColBERT-v2)
}

pub enum QueryEmbedding {
    Dense(Vec<f32>),
    Sparse(SparseVec),
    Token(TokenEmbeddings),
}
```

---

## 6. Model Management

### 6.1 Model Download Specifications

```rust
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "e1",
        repo: "nlpaueb/legal-bert-base-uncased",  // BASE, not small
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 220,
        required: true,  // Accuracy-critical -- always required
    },
    ModelSpec {
        id: "e6",
        repo: "naver/splade-cocondenser-selfdistil",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 110,
        required: true,
    },
    ModelSpec {
        id: "e12",
        repo: "colbert-ir/colbertv2.0",  // Full v2, not small
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 220,
        required: false,  // Pro tier only
    },
];
// E13 (BM25) requires no model download -- pure algorithm
```

### 6.2 Download Resilience

- Skip files already downloaded with valid checksums
- Retry up to 3 attempts with exponential backoff (2s, 4s, 8s)
- Fatal error after 3 failures for any single file -- do not start with incomplete models

### 6.3 Memory Pressure Handling (8GB Fallback)

```rust
/// On 8GB machines: load models one at a time instead of simultaneously.
/// NEVER downgrade model quality -- just load/unload sequentially.
pub fn handle_memory_pressure(&mut self) {
    let available_mb = sysinfo::System::new_all()
        .available_memory() / (1024 * 1024);

    if available_mb < 2048 {  // Less than 2GB free
        tracing::warn!(
            "Low memory ({} MB free). Switching to sequential model loading. \
             Accuracy is unchanged -- models will load on demand.",
            available_mb
        );

        // Unload non-primary models (keep E1 always loaded)
        for id in &[EmbedderId::E12, EmbedderId::E6] {
            if let Some(slot) = self.models.get_mut(id) {
                *slot = None;
            }
        }
    }
}
```

---

## 7. ONNX Model Conversion Notes

For the fresh project build, models must be converted from PyTorch to ONNX:

```python
# Example: Convert legal-bert-base-uncased to ONNX (FULL SIZE for accuracy)
import torch
from transformers import AutoModel, AutoTokenizer

model = AutoModel.from_pretrained("nlpaueb/legal-bert-base-uncased")
tokenizer = AutoTokenizer.from_pretrained("nlpaueb/legal-bert-base-uncased")

dummy_input = tokenizer("hello world", return_tensors="pt")

torch.onnx.export(
    model,
    (dummy_input["input_ids"], dummy_input["attention_mask"]),
    "model.onnx",
    input_names=["input_ids", "attention_mask"],
    output_names=["last_hidden_state"],
    dynamic_axes={
        "input_ids": {0: "batch", 1: "seq"},
        "attention_mask": {0: "batch", 1: "seq"},
        "last_hidden_state": {0: "batch", 1: "seq"},
    },
    opset_version=14,
)

# Quantize to INT8 (reduces disk size ~50%, negligible accuracy loss)
from onnxruntime.quantization import quantize_dynamic, QuantType

quantize_dynamic(
    "model.onnx",
    "model_int8.onnx",
    weight_type=QuantType.QInt8,
)
```

A `scripts/convert_models.py` script should be included in the repository to automate this for all 3 neural models. Pre-converted ONNX models should be hosted on Hugging Face under a `casetrack/` organization.

---

## 8. Model Selection Rationale (vs. Alternatives Considered)

| Model | Params | Dim | Legal Training | Size (ONNX) | Decision |
|-------|--------|-----|---------------|-------------|----------|
| **nlpaueb/legal-bert-base** | **110M** | **768** | **12GB legal text** | **~220MB** | **CHOSEN (E1)**: Best accuracy for legal retrieval within RAM budget |
| nlpaueb/legal-bert-small | 35M | 512 | 12GB legal text | ~70MB | REJECTED: Measurably less accurate; RAM savings irrelevant on 16GB |
| law-ai/InLegalBERT | 110M | 768 | 27GB Indian legal | ~220MB | CONSIDERED: Excellent but trained primarily on Indian legal corpus; less suited for US/UK/EU law |
| casehold/legalbert | 110M | 768 | US case law | ~220MB | CONSIDERED: Good US focus but nlpaueb's model has broader training data |
| pile-of-law/legalbert-large | 340M | 1024 | 256GB legal (Black's Law Dict.) | ~680MB | CONSIDERED: Best vocabulary but 3x RAM cost; base is sufficient |
| BAAI/bge-small-en-v1.5 | 33M | 384 | General text | ~65MB | REJECTED: No legal training; misses legal terminology and concepts |
| voyage-law-2 | Unknown | 1024 | 1T+ legal tokens | API only | REJECTED: Best quality but requires cloud API -- violates privacy/privilege principle |
| **SPLADE-cocondenser** | **~110M** | **Sparse** | **General (MSMARCO)** | **~110MB** | **CHOSEN (E6)**: Best sparse retrieval model; legal term expansion works well |
| **ColBERT-v2** | **~110M** | **128/tok** | **General (MSMARCO)** | **~220MB** | **CHOSEN (E12)**: Token-level precision critical for legal phrase matching |

---

## 9. Accuracy Validation

### 9.1 Legal Retrieval Benchmarks

CaseTrack's embedder stack should be evaluated against:

- **MLEB (Massive Legal Embedding Benchmark)**: 10 expert-annotated datasets across US, UK, EU, Australia, Ireland, Singapore jurisdictions (MIT licensed, github.com/isaacus-dev/mleb)
- **CaseHOLD**: Multiple-choice legal holding identification from US case law
- **LexGLUE**: Legal NLU benchmark with 7 datasets (ECtHR, SCOTUS, EUR-LEX, LEDGAR, UNFAIR-ToS, CaseHOLD, ILDC)

### 9.2 Accuracy Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Top-5 recall (legal queries) | > 90% | Manual evaluation on 100 legal queries |
| Top-1 precision (exact clause retrieval) | > 75% | Exact clause match on contract search |
| Citation extraction accuracy | > 95% | Regex + NER on Bluebook citations |
| Entity extraction accuracy | > 90% | NER on legal entity types (parties, courts, judges) |
| Provenance accuracy | **100%** | Every result traceable to exact source location |

---

*CaseTrack PRD v5.1.0 -- Document 5 of 10*


---

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


---

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


---

# PRD 08: Search & Retrieval

**Version**: 5.1.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. 3-Stage Search Pipeline

```
+-----------------------------------------------------------------------+
|                        3-STAGE SEARCH PIPELINE                         |
+-----------------------------------------------------------------------+
|                                                                       |
|  Query: "What does the contract say about indemnification?"           |
|                                                                       |
|  +---------------------------------------------------------------+   |
|  | STAGE 1: BM25 RECALL                                  [<5ms]   |   |
|  |                                                                |   |
|  | - E13 inverted index lookup                                   |   |
|  | - Terms: "contract", "indemnification"                        |   |
|  | - Fast lexical matching                                       |   |
|  |                                                                |   |
|  | Output: 500 candidate chunks                                  |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 2: SEMANTIC RANKING                             [<80ms]  |   |
|  |                                                                |   |
|  | - E1: Legal-BERT-base semantic similarity (768D dense cosine) |   |
|  | - E6: SPLADE keyword expansion (sparse dot product)           |   |
|  | - Score fusion via Reciprocal Rank Fusion (RRF)               |   |
|  |                                                                |   |
|  | Output: 100 candidates, ranked                                |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 3: COLBERT RERANK (PRO TIER ONLY)              [<100ms] |   |
|  |                                                                |   |
|  | - E12: ColBERT-v2 token-level MaxSim scoring (128D/token)     |   |
|  | - Ensures exact phrase matches rank highest                   |   |
|  | - "breach of fiduciary duty" > "fiduciary duty breach"        |   |
|  |                                                                |   |
|  | Output: Top K results with provenance                         |   |
|  +---------------------------------------------------------------+   |
|                                                                       |
|  LATENCY TARGETS                                                      |
|  ----------------                                                     |
|  Free tier (Stages 1-2):  <100ms                                     |
|  Pro tier (Stages 1-3):   <200ms                                     |
|                                                                       |
+-----------------------------------------------------------------------+
```

---

## 2. Search Engine Implementation

```rust
pub struct SearchEngine {
    embedder: Arc<EmbeddingEngine>,
    citation_index: Arc<CitationIndex>,
    tier: LicenseTier,
}

impl SearchEngine {
    pub fn search(
        &self,
        case: &CaseHandle,
        query: &str,
        top_k: usize,
        document_filter: Option<Uuid>,
    ) -> Result<Vec<SearchResult>> {
        let start = std::time::Instant::now();

        // Legal citation detection: if query contains a citation pattern,
        // check the citation index first for direct matches
        if let Some(citation_results) = self.citation_lookup(case, query)? {
            if !citation_results.is_empty() {
                return Ok(citation_results);
            }
        }

        // Stage 1: BM25 recall
        let bm25_candidates = self.bm25_recall(case, query, 500, document_filter)?;

        if bm25_candidates.is_empty() {
            return Ok(vec![]);
        }

        // Stage 2: Semantic ranking (E1 Legal-BERT-base 768D + E6 SPLADE)
        let query_e1 = self.embedder.embed_query(query, EmbedderId::E1)?;
        let query_e6 = self.embedder.embed_query(query, EmbedderId::E6)?;

        let mut scored: Vec<(Uuid, f32)> = bm25_candidates
            .iter()
            .map(|chunk_id| {
                let e1_score = self.score_dense(case, "e1", chunk_id, &query_e1)?;
                let e6_score = self.score_sparse(case, "e6", chunk_id, &query_e6)?;

                let rrf = rrf_fusion(&[
                    (e1_score, 1.0),   // E1: weight 1.0
                    (e6_score, 0.8),   // E6: weight 0.8
                ]);

                Ok((*chunk_id, rrf))
            })
            .collect::<Result<Vec<_>>>()?;

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(100);

        // Stage 3: ColBERT-v2 rerank (Pro only)
        if self.tier.is_pro() {
            scored = self.colbert_rerank(case, query, scored)?;
        }

        // Build results with provenance
        let results: Vec<SearchResult> = scored
            .into_iter()
            .take(top_k)
            .map(|(chunk_id, score)| self.build_result(case, chunk_id, score))
            .collect::<Result<Vec<_>>>()?;

        let elapsed = start.elapsed();
        tracing::info!(
            "Search completed: {} results in {}ms (query: '{}')",
            results.len(),
            elapsed.as_millis(),
            query
        );

        Ok(results)
    }

    fn build_result(
        &self,
        case: &CaseHandle,
        chunk_id: Uuid,
        score: f32,
    ) -> Result<SearchResult> {
        let chunk = case.get_chunk(chunk_id)?;
        let (ctx_before, ctx_after) = case.get_surrounding_context(&chunk, 1)?;

        Ok(SearchResult {
            text: chunk.text,
            score,
            provenance: chunk.provenance.clone(),
            citation: chunk.provenance.cite(),
            citation_short: chunk.provenance.cite_short(),
            context_before: ctx_before,
            context_after: ctx_after,
        })
    }
}
```

---

## 3. Legal Citation Search

Queries containing legal citations (case names, statute references, regulatory codes) trigger a specialized citation index lookup before the standard BM25 pipeline. This ensures precise citation retrieval without relying on semantic similarity.

```
LEGAL CITATION SEARCH
=================================================================================

  Detection patterns:
    Case law:    "Daubert v. Merrell Dow", "Smith v. Jones", "In re Enron"
    Statutes:    "42 U.S.C. ss 1983", "Cal. Civ. Code ss 1750"
    Regulations: "17 C.F.R. ss 240.10b-5", "29 C.F.R. Part 1910"
    Short forms: "Id.", "Id. at 579", "Daubert, 509 U.S. at 593"
    Supra:       "Smith, supra, at 42"

  Pipeline:
    1. Regex detects citation pattern in query
    2. Short-form resolution: "Id." -> last full citation in context
       "Daubert" -> "Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993)"
    3. Citation index lookup -> exact matches with provenance
    4. If no citation match, fall through to standard 3-stage pipeline
    5. Citation results include: citing document, page, paragraph,
       and surrounding context showing how the citation is used
```

```rust
pub struct CitationIndex;

impl CitationIndex {
    /// Detect legal citations in query text using regex patterns
    pub fn detect_citations(query: &str) -> Vec<CitationPattern> {
        let mut citations = Vec::new();

        // Case law: "X v. Y" or "X v Y"
        // Statutes: "NN U.S.C. ss NNNN"
        // Regulations: "NN C.F.R. ss NNN"
        // Short forms: "Id.", "Id. at NNN"
        for pattern in &CITATION_REGEXES {
            for m in pattern.find_iter(query) {
                citations.push(CitationPattern {
                    text: m.as_str().to_string(),
                    citation_type: pattern.citation_type(),
                    span: (m.start(), m.end()),
                });
            }
        }

        citations
    }

    /// Resolve short-form citations to their full form
    /// "Id." -> last full citation; "Daubert" -> full Daubert citation
    pub fn resolve_short_form(
        case: &CaseHandle,
        short_form: &str,
    ) -> Result<Option<String>>;

    /// Look up all chunks citing a specific case, statute, or regulation
    pub fn search(
        case: &CaseHandle,
        citation: &str,
        top_k: usize,
    ) -> Result<Vec<SearchResult>>;
}

#[derive(Debug)]
pub struct CitationPattern {
    pub text: String,
    pub citation_type: CitationType,
    pub span: (usize, usize),
}

#[derive(Debug)]
pub enum CitationType {
    CaseLaw,       // "Daubert v. Merrell Dow"
    Statute,       // "42 U.S.C. ss 1983"
    Regulation,    // "17 C.F.R. ss 240.10b-5"
    ShortForm,     // "Id.", "Id. at 579"
    Supra,         // "Smith, supra, at 42"
}
```

---

## 4. BM25 Implementation

Standard BM25 with `k1=1.2, b=0.75`. Stored in `bm25_index` column family.

**Key schema**: `term:{token}` -> bincode `PostingList`, `stats` -> bincode `Bm25Stats`

**Tokenization**: lowercase, split on non-alphanumeric (preserving apostrophes and legal citation markers like "ss" and "v."), filter stopwords and single-char tokens.

```rust
pub struct Bm25Index;

impl Bm25Index {
    /// Tokenize query -> lookup postings per term -> accumulate BM25 scores
    /// per chunk -> apply optional document_filter -> return top `limit` chunk IDs
    pub fn search(case: &CaseHandle, query: &str, limit: usize,
                  document_filter: Option<Uuid>) -> Result<Vec<Uuid>>;

    /// Tokenize chunk text -> upsert PostingList per term -> update Bm25Stats
    pub fn index_chunk(case: &CaseHandle, chunk: &Chunk) -> Result<()>;
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Bm25Stats {
    pub total_docs: u32,
    pub total_tokens: u64,
    pub avg_doc_length: f32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PostingList {
    pub doc_freq: u32,
    pub entries: Vec<PostingEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostingEntry {
    pub chunk_id: Uuid,
    pub document_id: Uuid,
    pub term_freq: u32,
    pub doc_length: u32,
}
```

---

## 5. SPLADE Expansion

E6 (SPLADE) expands queries with semantically related legal terms before sparse matching. This bridges vocabulary gaps common in legal documents where the same concept uses different terminology.

```
SPLADE EXPANSION EXAMPLES (Legal Domain)
=================================================================================

  Query: "negligence"
  Expanded: "negligence", "tortious conduct", "duty of care", "breach",
            "reasonable person", "proximate cause", "foreseeability"

  Query: "breach of fiduciary duty"
  Expanded: "breach", "fiduciary", "duty", "loyalty", "self-dealing",
            "conflict of interest", "trustee", "beneficiary"

  Query: "motion to dismiss standard"
  Expanded: "motion to dismiss", "12(b)(6)", "failure to state a claim",
            "plausibility", "Twombly", "Iqbal", "pleading standard"

  Query: "indemnification"
  Expanded: "indemnification", "indemnify", "hold harmless",
            "defense costs", "third-party claims", "indemnitor",
            "indemnitee", "contribution"
```

---

## 6. Reciprocal Rank Fusion (RRF)

```rust
/// Combine scores from multiple embedders using RRF
/// Each (score, weight) pair represents one embedder's score and its importance
pub fn rrf_fusion(scored_weights: &[(f32, f32)]) -> f32 {
    const K: f32 = 60.0;

    scored_weights
        .iter()
        .map(|(score, weight)| {
            if *score <= 0.0 {
                0.0
            } else {
                // Convert similarity score to rank-like value, then apply RRF
                weight / (K + (1.0 / score))
            }
        })
        .sum()
}
```

---

## 7. ColBERT Reranking (Stage 3)

```rust
impl SearchEngine {
    fn colbert_rerank(
        &self,
        case: &CaseHandle,
        query: &str,
        candidates: Vec<(Uuid, f32)>,
    ) -> Result<Vec<(Uuid, f32)>> {
        // Embed query at token level (ColBERT-v2, 128D per token)
        let query_tokens = self.embedder.embed_query(query, EmbedderId::E12)?;
        let query_vecs = match query_tokens {
            QueryEmbedding::Token(t) => t,
            _ => unreachable!(),
        };

        let mut reranked: Vec<(Uuid, f32)> = candidates
            .into_iter()
            .map(|(chunk_id, base_score)| {
                // Load chunk's token embeddings (128D per token)
                let chunk_tokens = self.load_token_embeddings(case, &chunk_id)?;

                // MaxSim: for each query token, find max similarity to any chunk token
                let maxsim_score = query_vecs.vectors.iter()
                    .map(|q_vec| {
                        chunk_tokens.vectors.iter()
                            .map(|c_vec| cosine_similarity(q_vec, c_vec))
                            .fold(f32::NEG_INFINITY, f32::max)
                    })
                    .sum::<f32>() / query_vecs.vectors.len() as f32;

                // Blend ColBERT score with previous ranking
                let final_score = base_score * 0.4 + maxsim_score * 0.6;
                Ok((chunk_id, final_score))
            })
            .collect::<Result<Vec<_>>>()?;

        reranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(reranked)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenEmbeddings {
    pub vectors: Vec<Vec<f32>>,  // One 128D vector per token (ColBERT-v2)
    pub token_count: usize,
}
```

---

## 8. Knowledge Graph Integration in Search

After vector search returns chunks, results can optionally be expanded via the case's knowledge graph to surface related content the user did not directly query.

```
KNOWLEDGE GRAPH EXPANSION (POST-RETRIEVAL)
=================================================================================

  1. Vector search returns top K chunks (from Stages 1-3)
  2. For each result chunk:
     a. Look up entities mentioned in that chunk (parties, courts, statutes)
     b. Find other chunks/documents sharing those entities -> "Related documents"
     c. Traverse chunk-to-chunk edges (semantic similarity, co-reference) -> "Related chunks"
  3. Deduplicate and rank expanded results by graph edge weight
  4. Return expanded results alongside primary results

  Enables:
    - "Show me all documents citing Daubert" via citation entity overlap
    - "What other cases reference the same statute?" via shared statute entities
    - Cross-document discovery without explicit search terms
    - "Related motions" via shared legal concepts and party references
```

```rust
impl SearchEngine {
    /// Expand search results via knowledge graph edges
    pub fn expand_via_graph(
        &self,
        case: &CaseHandle,
        results: &[SearchResult],
        max_expansions: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut expanded = Vec::new();

        for result in results {
            // Find entities in this chunk (parties, courts, statutes, case citations)
            let entities = case.get_chunk_entities(result.provenance.document_id)?;

            // Find other documents mentioning the same entities
            let related_docs = case.find_documents_by_entities(&entities)?;

            // Find chunks connected via graph edges
            let related_chunks = case.get_related_chunks(
                result.provenance.document_id,
                max_expansions,
            )?;

            for chunk in related_chunks {
                expanded.push(self.build_result(case, chunk.id, chunk.edge_weight)?);
            }
        }

        // Deduplicate by chunk_id
        expanded.dedup_by_key(|r| r.provenance.document_id);
        expanded.truncate(max_expansions);

        Ok(expanded)
    }
}
```

---

## 9. Search Response Format (Canonical)

This is the canonical MCP response format for `search_case` (also referenced by PRD 09).
Document-scoped search uses the same pipeline via the `document_filter` parameter on `SearchEngine::search`.

Every search result includes full provenance: file path, document name, page, paragraph, line, and character offsets.

```json
{
  "query": "indemnification obligations",
  "case": "Smith v. Jones (2024-CV-01234)",
  "results_count": 5,
  "search_time_ms": 87,
  "tier": "pro",
  "stages_used": ["bm25", "semantic", "colbert"],
  "results": [
    {
      "text": "Defendant shall indemnify, defend, and hold harmless Plaintiff from and against any and all claims, damages, losses, costs, and expenses (including reasonable attorneys' fees) arising out of or relating to Defendant's breach of this Agreement...",
      "score": 0.94,
      "citation": "Complaint.pdf, p. 8, para. 24",
      "citation_short": "Complaint, p. 8",
      "source": {
        "document": "Complaint.pdf",
        "document_path": "/Users/sarah/Cases/SmithVJones/Pleadings/Complaint.pdf",
        "document_id": "abc-123",
        "chunk_id": "chunk-456",
        "chunk_index": 14,
        "page": 8,
        "paragraph_start": 24,
        "paragraph_end": 24,
        "line_start": 1,
        "line_end": 6,
        "char_start": 18240,
        "char_end": 20240,
        "extraction_method": "Native",
        "ocr_confidence": null,
        "chunk_created_at": "2026-01-15T14:30:00Z",
        "chunk_embedded_at": "2026-01-15T14:30:12Z",
        "document_ingested_at": "2026-01-15T14:29:48Z"
      },
      "context": {
        "before": "...Section 8.2 of the Master Agreement provides that...",
        "after": "...The indemnification obligations shall survive termination of this Agreement for a period of three (3) years..."
      },
      "legal_citations": [
        "Master Agreement, Section 8.2"
      ]
    },
    {
      "text": "The Court finds that the indemnification clause in the Agreement is enforceable under New York law. See Hooper Assocs., Ltd. v. AGS Computers, Inc., 74 N.Y.2d 487 (1989)...",
      "score": 0.91,
      "citation": "Smith_v_Jones_Opinion.pdf, p. 14, para. 3",
      "citation_short": "Opinion, p. 14",
      "source": {
        "document": "Smith_v_Jones_Opinion.pdf",
        "document_path": "/Users/sarah/Cases/SmithVJones/Orders/Smith_v_Jones_Opinion.pdf",
        "document_id": "def-789",
        "chunk_id": "chunk-012",
        "chunk_index": 42,
        "page": 14,
        "paragraph_start": 3,
        "paragraph_end": 3,
        "line_start": 1,
        "line_end": 8,
        "char_start": 32100,
        "char_end": 34100,
        "extraction_method": "Native",
        "ocr_confidence": null,
        "chunk_created_at": "2026-01-15T14:31:00Z",
        "chunk_embedded_at": "2026-01-15T14:31:12Z",
        "document_ingested_at": "2026-01-15T14:30:48Z"
      },
      "context": {
        "before": "...Defendant moves to dismiss the indemnification claim on the grounds that...",
        "after": "...Accordingly, Defendant's motion to dismiss Count III is DENIED..."
      },
      "legal_citations": [
        "Hooper Assocs., Ltd. v. AGS Computers, Inc., 74 N.Y.2d 487 (1989)"
      ]
    },
    {
      "text": "Respondent's Motion to Dismiss argues that the indemnification provision is unconscionable. However, the standard set forth in Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993) requires...",
      "score": 0.82,
      "citation": "Motion_to_Dismiss.pdf, p. 6, para. 12",
      "citation_short": "MTD, p. 6",
      "source": {
        "document": "Motion_to_Dismiss.pdf",
        "document_path": "/Users/sarah/Cases/SmithVJones/Motions/Motion_to_Dismiss.pdf",
        "document_id": "ghi-345",
        "chunk_id": "chunk-678",
        "chunk_index": 18,
        "page": 6,
        "paragraph_start": 12,
        "paragraph_end": 12,
        "line_start": 1,
        "line_end": 5,
        "char_start": 12400,
        "char_end": 14400,
        "extraction_method": "Native",
        "ocr_confidence": null,
        "chunk_created_at": "2026-01-15T14:32:00Z",
        "chunk_embedded_at": "2026-01-15T14:32:12Z",
        "document_ingested_at": "2026-01-15T14:31:48Z"
      },
      "context": {
        "before": "...Count III of the Complaint alleges breach of the indemnification provision...",
        "after": "...For the foregoing reasons, Respondent respectfully requests that this Court dismiss..."
      },
      "legal_citations": [
        "Daubert v. Merrell Dow Pharmaceuticals, Inc., 509 U.S. 579 (1993)"
      ]
    }
  ]
}
```

---

*CaseTrack PRD v5.1.0 -- Document 8 of 10*


---

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


---

# PRD 10: Technical Build Guide

**Version**: 5.1.0 | **Document**: 10 of 10 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

> **ACCURACY FIRST** -- CaseTrack is a legal case management intelligence system.
> Every search result, every citation, every provenance record must be accurate.
> Legal professionals depend on correctness. When in doubt, return nothing rather
> than return something wrong.

> **LANGUAGE: RUST** -- This entire project is built in Rust. Every crate, every
> module, every line of product code is Rust. The final deliverable is a single
> statically-linked Rust binary per platform. No runtime dependencies. The only
> non-Rust code is `scripts/convert_models.py` (one-time build tool, not shipped).

> **16GB RAM TARGET** -- All memory budgets, model selections, and batch sizes are
> tuned for machines with 16GB RAM. Legal-BERT-base (768D, 110M params) was chosen
> specifically to fit within this envelope alongside SPLADE, ColBERT-v2, and BM25.

---

## 1. Project Bootstrap

### 1.1 Create Fresh Project

```bash
mkdir casetrack && cd casetrack
cargo init --name casetrack
mkdir -p crates/casetrack-core
cd crates/casetrack-core && cargo init --lib --name casetrack-core && cd ../..
git init
echo -e "target/\n*.onnx\nmodels/" > .gitignore
```

### 1.2 Workspace Structure

```
casetrack/
|-- Cargo.toml                   # Workspace root (legal case management)
|-- Cargo.lock
|-- .github/workflows/
|   |-- ci.yml
|   +-- release.yml
|-- scripts/
|   |-- convert_models.py
|   |-- build_mcpb.sh
|   +-- install.sh
|-- crates/
|   |-- casetrack/               # Binary crate (MCP server entry point)
|   |   |-- Cargo.toml
|   |   +-- src/
|   |       |-- main.rs
|   |       |-- cli.rs
|   |       |-- server.rs
|   |       +-- format.rs
|   +-- casetrack-core/          # Library crate (all legal domain business logic)
|       |-- Cargo.toml
|       +-- src/
|           |-- lib.rs
|           |-- error.rs
|           |-- config.rs
|           |-- case/            # registry, handle, model
|           |-- document/        # pdf, docx, xlsx, ocr, chunker, model, legal_chunker
|           |-- embedding/       # engine, models, download, types
|           |-- search/          # engine, bm25, ranking, result
|           |-- provenance/      # citation formatting, legal citations
|           |-- entity/          # legal entity extraction, citation parsing
|           |-- storage/         # rocks, schema
|           +-- license/         # validator (ed25519)
|-- tests/
|   |-- integration/
|   |   |-- test_case_lifecycle.rs
|   |   |-- test_ingest_pdf.rs
|   |   |-- test_search.rs
|   |   |-- test_legal_domain.rs
|   |   +-- test_mcp_tools.rs
|   +-- fixtures/
|       |-- complaint_sample.pdf       # 3-page legal complaint
|       |-- contract_sample.docx       # Contract with clauses
|       |-- billing_sample.xlsx        # Legal billing data
|       |-- scanned.png
|       |-- empty.pdf
|       +-- large_paragraph.txt
+-- docs/prd/
```

### 1.3 Workspace Cargo.toml

```toml
[workspace]
members = ["crates/casetrack", "crates/casetrack-core"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "LicenseRef-Commercial"
repository = "https://github.com/casetrack-dev/casetrack"

[workspace.dependencies]
rmcp = { version = "0.13", features = ["server", "transport-io", "macros"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"
rocksdb = "0.22"
ort = { version = "2.0", features = ["download-binaries"] }
pdf-extract = "0.7"
lopdf = "0.32"
docx-rs = "0.4"
calamine = "0.24"
image = "0.25"
tesseract = { version = "0.14", optional = true }
hf-hub = "0.3"
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4.4", features = ["derive"] }
thiserror = "2.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
ed25519-dalek = "2.1"
base64 = "0.22"
bytemuck = { version = "1.14", features = ["derive"] }
sha2 = "0.10"
sysinfo = "0.30"
walkdir = "2.4"
notify = "6.1"
semver = "1.0"
dirs = "5.0"
regex = "1.10"
```

### 1.4 Crate Cargo.toml Files

**Binary crate** (`crates/casetrack/Cargo.toml`):

```toml
[package]
name = "casetrack"
version.workspace = true
edition.workspace = true

[[bin]]
name = "casetrack"
path = "src/main.rs"

[dependencies]
casetrack-core = { path = "../casetrack-core" }
rmcp.workspace = true
tokio.workspace = true
serde_json.workspace = true
clap.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
dirs.workspace = true
anyhow.workspace = true

[features]
default = ["ocr"]
ocr = ["casetrack-core/ocr"]

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

**Core library** (`crates/casetrack-core/Cargo.toml`):

```toml
[package]
name = "casetrack-core"
version.workspace = true
edition.workspace = true

[dependencies]
rocksdb.workspace = true
ort.workspace = true
pdf-extract.workspace = true
lopdf.workspace = true
docx-rs.workspace = true
calamine.workspace = true
image.workspace = true
tesseract = { workspace = true, optional = true }
hf-hub.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
bincode.workspace = true
uuid.workspace = true
chrono.workspace = true
thiserror.workspace = true
anyhow.workspace = true
tracing.workspace = true
ed25519-dalek.workspace = true
base64.workspace = true
bytemuck.workspace = true
sha2.workspace = true
sysinfo.workspace = true
walkdir.workspace = true
notify.workspace = true
semver.workspace = true
dirs.workspace = true
regex.workspace = true

[features]
default = ["ocr"]
ocr = ["dep:tesseract"]
```

---

## 2. Entry Point

```rust
// crates/casetrack/src/main.rs
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod server;
mod format;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    match &args.command {
        Some(cli::Command::SetupClaudeCode) => return casetrack_core::setup_claude_code(&args.data_dir()),
        Some(cli::Command::Update) => return casetrack_core::self_update().await,
        Some(cli::Command::Uninstall) => return casetrack_core::uninstall(),
        Some(cli::Command::StripEmbeddings { case, embedder }) => {
            return casetrack_core::strip_embeddings_cli(&args.data_dir(), case, embedder);
        }
        Some(cli::Command::PurgeArchived { output, case }) => {
            return casetrack_core::purge_archived_cli(&args.data_dir(), output, case.as_deref());
        }
        None => {}
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("casetrack=info")))
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("CaseTrack v{} starting...", env!("CARGO_PKG_VERSION"));

    server::CaseTrackServer::start(casetrack_core::Config {
        data_dir: args.data_dir(),
        license_key: args.license.clone(),
    }).await
}
```

---

## 3. CLI Arguments

```rust
// crates/casetrack/src/cli.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "casetrack", about = "Legal case management intelligence MCP server for Claude", version)]
pub struct Args {
    #[arg(long, env = "CASETRACK_HOME")]
    pub data_dir: Option<PathBuf>,

    #[arg(long, env = "CASETRACK_LICENSE")]
    pub license: Option<String>,

    #[arg(long, value_enum)]
    pub memory_mode: Option<MemoryMode>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    SetupClaudeCode,
    Update,
    Uninstall,
    /// Strip unused embedder vectors from a case (e.g., E12 after Pro->Free downgrade)
    StripEmbeddings {
        #[arg(long)]
        case: String,
        #[arg(long)]
        embedder: String,
    },
    /// Export archived cases to .ctcase ZIP and delete expanded databases
    PurgeArchived {
        #[arg(long)]
        output: std::path::PathBuf,
        #[arg(long)]
        case: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum MemoryMode { Full, Standard, Constrained }
```

`Args::data_dir()` defaults to `~/Documents/CaseTrack/` via `dirs::document_dir()`.

---

## 4. Error Handling

### 4.1 Error Types

```rust
// crates/casetrack-core/src/error.rs
use thiserror::Error;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum CaseTrackError {
    // === Case Errors ===
    #[error("Case not found: {0}")]
    CaseNotFound(Uuid),

    #[error("No active case. Create or switch to a case first.")]
    NoCaseActive,

    #[error("Case name not found: \"{0}\"")]
    CaseNameNotFound(String),

    // === Document Errors ===
    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("File not found: {}", .0.display())]
    FileNotFound(PathBuf),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Duplicate document (SHA256 matches existing document ID: {0})")]
    DuplicateDocument(Uuid),

    // === PDF Errors ===
    #[error("PDF parse error for {}: {source}", .path.display())]
    PdfParseError { path: PathBuf, source: lopdf::Error },

    // === DOCX Errors ===
    #[error("DOCX parse error for {}: {source}", .path.display())]
    DocxParseError { path: PathBuf, source: String },

    // === XLSX Errors ===
    #[error("XLSX parse error for {}: {source}", .path.display())]
    XlsxParseError { path: PathBuf, source: String },

    // === OCR Errors ===
    #[error("OCR not available (build without OCR feature)")]
    OcrNotAvailable,

    #[error("OCR failed: {0}")]
    OcrFailed(String),

    // === Legal Domain Errors ===
    #[error("Legal citation parse error: {0}")]
    LegalCitationParseError(String),

    #[error("Invalid Bluebook citation format: \"{0}\"")]
    InvalidBluebookCitation(String),

    // === Embedding Errors ===
    #[error("Embedder not loaded: {0:?}")]
    EmbedderNotLoaded(crate::embedding::EmbedderId),

    #[error("Model not downloaded: {0:?}. Run server with network access to download.")]
    ModelNotDownloaded(crate::embedding::EmbedderId),

    #[error("ONNX inference failed: {0}")]
    InferenceFailed(String),

    #[error("Embedding not found for chunk {0}")]
    EmbeddingNotFound(Uuid),

    // === Storage Errors ===
    #[error("Registry database failed to open: {source}")]
    RegistryOpenFailed { source: rocksdb::Error },

    #[error("Case database failed to open at {}: {source}", .path.display())]
    CaseDbOpenFailed { path: PathBuf, source: rocksdb::Error },

    #[error("Database schema version {found} is newer than supported version {supported}. Update CaseTrack.")]
    FutureSchemaVersion { found: u32, supported: u32 },

    #[error("BM25 index is empty. Ingest documents first.")]
    Bm25IndexEmpty,

    // === Search Errors ===
    #[error("Chunk not found: {0}")]
    ChunkNotFound(Uuid),

    // === License Errors ===
    #[error("Free tier limit: {resource} ({current}/{max}). Upgrade: https://casetrack.dev/upgrade")]
    FreeTierLimit { resource: String, current: u32, max: u32 },

    #[error("Invalid license key format")]
    InvalidLicenseFormat,

    // === System Errors ===
    #[error("Home directory not found")]
    NoHomeDir,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),
}

pub type Result<T> = std::result::Result<T, CaseTrackError>;
```

### 4.2 Error Design Principles

1. **Specific**: Every error tells you exactly what went wrong
2. **Actionable**: Every error tells you what to do about it
3. **No silent failures**: Every operation returns `Result<T>`
4. **User-facing errors include guidance**: "Create or switch to a case first"
5. **Legal domain errors**: Citation parse failures include the malformed input
6. **Internal errors include report URL**: "Please report this at github.com/..."

---

## 5. Configuration

```rust
// crates/casetrack-core/src/config.rs
use std::path::PathBuf;

pub struct Config {
    pub data_dir: PathBuf,
    pub license_key: Option<String>,
}

/// Optional config file (~/Documents/CaseTrack/config.toml) -- zero-config by default
#[derive(serde::Deserialize, Default)]
pub struct ConfigFile {
    pub data_dir: Option<PathBuf>,
    pub license_key: Option<String>,
    pub ocr_language: Option<String>,
    pub copy_originals: Option<bool>,
    pub memory_mode: Option<String>,
    pub inference_threads: Option<u32>,
    /// Storage budget in GB. Startup warns when total usage exceeds this.
    /// Default: 10 GB. See PRD 04 Section 13.
    pub storage_budget_gb: Option<u32>,
}
```

---

## 6. Embedding Engine

### 6.1 Model Specifications

CaseTrack uses legal-domain-optimized models. E1 is **Legal-BERT-base** (768D, 110M params) from `nlpaueb/legal-bert-base-uncased`, trained on 12GB of legal text (EU legislation, US court opinions, contracts). This provides significantly better legal term understanding compared to generic embedders.

| Embedder | Model | Dimensions | HuggingFace Repo | Role |
|----------|-------|-----------|-------------------|------|
| E1 | Legal-BERT-base | 768 | `nlpaueb/legal-bert-base-uncased` | Dense semantic (legal domain) |
| E6 | SPLADE | Sparse | `naver/splade-cocondenser-ensembledistil` | Sparse keyword + term expansion |
| E12 | ColBERT-v2 | 128/token | `colbert-ir/colbertv2.0` | Token-level reranking (Pro) |
| E13 | BM25 | N/A (index) | N/A | Exact keyword recall (Stage 1) |

### 6.2 Model Management

```rust
// crates/casetrack-core/src/embedding/download.rs

pub struct ModelSpec {
    pub id: EmbedderId,
    pub repo: &'static str,
    pub files: &'static [&'static str],
    pub size_mb: u32,
    pub tier: Tier,
}

pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: EmbedderId::E1,
        repo: "nlpaueb/legal-bert-base-uncased",
        files: &["model.onnx", "tokenizer.json", "config.json"],
        size_mb: 260,
        tier: Tier::Free,
    },
    ModelSpec {
        id: EmbedderId::E6,
        repo: "naver/splade-cocondenser-ensembledistil",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 70,
        tier: Tier::Free,
    },
    ModelSpec {
        id: EmbedderId::E12,
        repo: "colbert-ir/colbertv2.0",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 220,
        tier: Tier::Pro,
    },
];
```

### 6.3 Inference Pipeline

```rust
// crates/casetrack-core/src/embedding/engine.rs

pub struct EmbeddingEngine {
    e1_session: Option<ort::Session>,   // Legal-BERT-base (768D)
    e6_session: Option<ort::Session>,   // SPLADE sparse
    e12_session: Option<ort::Session>,  // ColBERT-v2 (Pro only)
    bm25_index: Bm25Index,             // E13 BM25
}

impl EmbeddingEngine {
    /// Embed text using Legal-BERT-base (E1). Returns 768-dimensional dense vector.
    pub fn embed_e1(&self, text: &str) -> Result<Vec<f32>> { ... }

    /// Embed text using SPLADE (E6). Returns sparse vector with term expansion.
    /// Legal terms like "breach" expand to related terms: "violation", "default", "nonperformance".
    pub fn embed_e6(&self, text: &str) -> Result<SparseVector> { ... }

    /// Embed text using ColBERT-v2 (E12). Returns per-token 128D embeddings.
    /// Pro tier only. Used for MaxSim reranking.
    pub fn embed_e12(&self, text: &str) -> Result<Vec<Vec<f32>>> { ... }
}
```

---

## 7. Logging

Logging goes to stderr (stdout is MCP transport). Controlled by `RUST_LOG` env var.

| Level | Usage |
|-------|-------|
| ERROR | Failures preventing operations (file not found, DB corruption, citation parse failure) |
| WARN  | Degraded functionality (low memory, OCR disabled, legal citation format unrecognized) |
| INFO  | Normal operations (server started, case created, search completed) |
| DEBUG | Internal details (model loading times, RocksDB stats, embedding dimensions) |
| TRACE | Verbose (individual chunk embeddings, token counts, legal entity matches) |

---

## 8. Cross-Platform Concerns

### 8.1 Path Handling

```rust
pub fn resolve_path(input: &str) -> PathBuf {
    let expanded = if input.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(&input[2..])
        } else {
            PathBuf::from(input)
        }
    } else {
        PathBuf::from(input)
    };
    expanded
}
```

### 8.2 Default Data Directory

| Platform | Default Path |
|----------|-------------|
| macOS | `~/Documents/CaseTrack/` |
| Windows | `C:\Users\{user}\Documents\CaseTrack\` |
| Linux | `~/Documents/CaseTrack/` (or `~/.local/share/casetrack/`) |

### 8.3 Platform Dependencies

| Component | macOS | Windows | Linux |
|-----------|-------|---------|-------|
| RocksDB | Works via `rust-rocksdb` | Requires MSVC build tools | Static link |
| Tesseract | Static link (vendored) | Bundle DLLs in installer | Static link or system pkg |
| ONNX Runtime | CoreML + CPU fallback | DirectML + CPU fallback | CPU only (CUDA optional) |

---

## 9. Security

### 9.1 Input Validation

```rust
pub fn validate_file_path(path: &Path, data_dir: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()
        .map_err(|_| CaseTrackError::FileNotFound(path.to_path_buf()))?;
    Ok(canonical)
}

pub fn validate_write_path(path: &Path, data_dir: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()
        .map_err(|_| CaseTrackError::FileNotFound(path.to_path_buf()))?;
    let data_canonical = data_dir.canonicalize()?;
    if !canonical.starts_with(&data_canonical) {
        return Err(CaseTrackError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Write path must be within data directory: {}", data_dir.display()),
        )));
    }
    Ok(canonical)
}
```

### 9.2 License Key Security

- ed25519 signed keys, validated offline. Public key embedded in binary.
- Key format: `TIER-XXXXXX-XXXXXX-XXXXXX-SIG`
- Cached validation avoids repeated network calls. No user data sent.

### 9.3 No Network After Setup

After initial model download and license activation, CaseTrack makes zero network requests. Document processing, search, and storage are 100% local. Update checks are optional and non-blocking.

### 9.4 Data Privacy and Confidentiality

All documents and embeddings are stored locally on the user's machine. CaseTrack never transmits document content, metadata, or search queries to any external service. This is critical for legal professionals handling privileged attorney-client communications, work product, and confidential case materials.

---

## 10. Testing Strategy

### 10.1 Unit Tests

Key test areas (every module has unit tests):

```rust
#[cfg(test)]
mod tests {
    // Chunking: 2000-char target, 200-char overlap, paragraph-aware
    fn test_chunk_respects_paragraph_boundaries() { ... }
    fn test_chunk_target_2000_chars() { ... }
    fn test_chunk_overlap_200_chars() { ... }
    fn test_chunk_min_400_chars() { ... }
    fn test_chunk_max_2200_chars() { ... }
    fn test_chunk_provenance_complete() { ... }

    // Legal-specific chunking
    fn test_legal_chunking_preserves_clauses() { ... }
    fn test_legal_chunking_preserves_qa_pairs() { ... }

    // Legal citation extraction
    fn test_citation_extraction_bluebook() { ... }     // "Brown v. Board of Education, 347 U.S. 483 (1954)"
    fn test_citation_extraction_statutes() { ... }     // "42 U.S.C. § 1983", "28 C.F.R. § 50.10"

    // Legal entity extraction
    fn test_legal_entity_extraction() { ... }          // Parties, judges, attorneys, courts

    // Legal-BERT embedding
    fn test_legal_bert_embedding_768d() { ... }        // Verify 768-dimensional output

    // BM25
    fn test_bm25_basic_search() { ... }
    fn test_bm25_term_frequency() { ... }

    // PROVENANCE (MOST CRITICAL TEST SUITE)
    // Every chunk MUST have: file path, document name, page, paragraph, line, char offsets, timestamps.
    // Every embedding MUST link back to a chunk with valid provenance.
    // Every search result MUST include complete provenance.
    fn test_citation_format() { ... }
    fn test_short_citation() { ... }
    fn test_provenance_includes_file_path() { ... }
    fn test_provenance_includes_document_name() { ... }
    fn test_provenance_includes_page_number() { ... }
    fn test_provenance_includes_paragraph_range() { ... }
    fn test_provenance_includes_line_range() { ... }
    fn test_provenance_includes_char_offsets() { ... }
    fn test_provenance_includes_timestamps() { ... }
    fn test_provenance_round_trip() { ... }
    fn test_embedding_links_to_valid_chunk() { ... }
    fn test_no_orphaned_embeddings() { ... }
    fn test_no_chunk_without_provenance() { ... }

    // Document parsers
    fn test_pdf_extraction() { ... }
    fn test_docx_extraction() { ... }
    fn test_xlsx_extraction() { ... }

    // RRF, cosine similarity, license
    fn test_rrf_fusion() { ... }
    fn test_cosine_identical_vectors() { ... }
    fn test_cosine_orthogonal_vectors() { ... }
    fn test_free_tier_limits() { ... }
    fn test_valid_license_key() { ... }
}
```

### 10.2 Integration Tests

```rust
// tests/integration/test_case_lifecycle.rs
#[tokio::test]
async fn test_create_list_switch_delete_case() {
    let dir = tempdir().unwrap();
    let mut registry = CaseRegistry::open(dir.path()).unwrap();

    let case = registry.create_case(CreateCaseParams {
        name: "Smith v. Jones".to_string(),
        case_id: None,
        case_type: Some(CaseType::Litigation),
    }).unwrap();
    assert_eq!(case.name, "Smith v. Jones");

    let cases = registry.list_cases().unwrap();
    assert_eq!(cases.len(), 1);

    let handle = registry.switch_case(case.id).unwrap();
    assert_eq!(registry.active_case_id(), Some(case.id));

    drop(handle);
    registry.delete_case(case.id).unwrap();
    assert_eq!(registry.list_cases().unwrap().len(), 0);
}

// tests/integration/test_search.rs
#[tokio::test]
async fn test_search_returns_relevant_results() {
    // Setup: create case, ingest complaint_sample.pdf with known legal content
    let results = search_engine.search(&case_handle, "breach of fiduciary duty", 10, None).unwrap();

    assert!(!results.is_empty());
    assert!(results[0].score > 0.5);
    assert!(results[0].citation.contains("complaint_sample.pdf"));

    // Verify full provenance on every result
    for result in &results {
        assert!(!result.provenance.document_name.is_empty());
        assert!(!result.provenance.document_path.is_empty());
        assert!(result.provenance.page > 0);
        assert!(result.provenance.char_start < result.provenance.char_end);
    }
}

// tests/integration/test_search.rs
#[tokio::test]
async fn test_search_result_has_legal_citation() {
    // Verify search results include legal-format citations
    let results = search_engine.search(&case_handle, "motion to dismiss", 5, None).unwrap();

    for result in &results {
        // Citation must include document name, page, and paragraph for legal reference
        assert!(!result.provenance.citation.is_empty());
        assert!(result.provenance.citation.contains("p."));  // Page reference
    }
}

#[tokio::test]
async fn test_case_isolation() {
    // Verify chunks from one case never appear in another case's search
    // Ingest into Case A ("Smith v. Jones"), search Case B ("Doe v. Roe") -- must return zero results
}
```

### 10.3 Test Fixtures

- `complaint_sample.pdf` -- 3-page legal complaint with causes of action, parties, and relief requested
- `contract_sample.docx` -- Contract with numbered clauses, definitions, and signature blocks
- `billing_sample.xlsx` -- Legal billing data with timekeeper entries, rates, and matter codes
- `scanned.png` -- Image of typed legal text for OCR testing
- `empty.pdf` -- Edge case: empty PDF
- `large_paragraph.txt` -- Edge case: single paragraph >2000 characters

### 10.4 Running Tests

```bash
cargo test              # All tests
cargo test --lib        # Unit tests only (fast)
cargo test --test '*'   # Integration tests (needs fixtures)
RUST_LOG=debug cargo test -- --nocapture   # With logging
cargo test test_bm25_basic_search          # Specific test
cargo test test_legal                      # All legal-specific tests
```

---

## 11. CI/CD Pipeline

### 11.1 GitHub Actions CI

```yaml
name: CI
on:
  push: { branches: [main] }
  pull_request: { branches: [main] }

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - run: cargo build --release
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo fmt -- --check

  size-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - name: Check binary size (<50MB)
        run: |
          SIZE=$(stat -f%z target/release/casetrack 2>/dev/null || stat -c%s target/release/casetrack)
          echo "Binary size: $SIZE bytes ($(($SIZE / 1024 / 1024)) MB)"
          [ "$SIZE" -le 52428800 ] || exit 1
```

### 11.2 Release Pipeline

```yaml
name: Release
on:
  push: { tags: ['v*'] }
permissions: { contents: write }

jobs:
  build:
    strategy:
      matrix:
        include:
          - { target: x86_64-apple-darwin, os: macos-latest, name: casetrack-darwin-x64 }
          - { target: aarch64-apple-darwin, os: macos-latest, name: casetrack-darwin-arm64 }
          - { target: x86_64-pc-windows-msvc, os: windows-latest, name: casetrack-win32-x64.exe }
          - { target: x86_64-unknown-linux-gnu, os: ubuntu-latest, name: casetrack-linux-x64 }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: "${{ matrix.target }}" }
      - run: cargo build --release --target ${{ matrix.target }}
      - shell: bash
        run: |
          if [ "${{ matrix.os }}" = "windows-latest" ]; then
            cp target/${{ matrix.target }}/release/casetrack.exe ${{ matrix.name }}
          else
            cp target/${{ matrix.target }}/release/casetrack ${{ matrix.name }}
          fi
      - uses: actions/upload-artifact@v4
        with: { name: "${{ matrix.name }}", path: "${{ matrix.name }}" }

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v1
        with:
          files: |
            casetrack-darwin-x64/casetrack-darwin-x64
            casetrack-darwin-arm64/casetrack-darwin-arm64
            casetrack-win32-x64.exe/casetrack-win32-x64.exe
            casetrack-linux-x64/casetrack-linux-x64
          generate_release_notes: true

  mcpb:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
      - run: bash scripts/build_mcpb.sh
      - uses: softprops/action-gh-release@v1
        with: { files: casetrack.mcpb }
```

---

## 12. Monetization Implementation

### 12.1 Billing Model: Flat Subscription with Ed25519 License Keys

CaseTrack uses a flat subscription model with offline-validated license keys. This is the simplest billing model that works for a 100% local, privacy-first legal tool.

**Why flat subscription (not usage-based)?**

1. **Lawyers hate unpredictable bills** -- they want to know exactly what they pay
2. **Local-first means no reliable metering** -- can't phone home per-call without violating attorney-client privilege
3. **Simplest to implement** -- license key encodes tier + expiry, verified offline with Ed25519
4. **No metering infrastructure needed** -- no usage DB, no sync protocol, no credit counters
5. **Aligns with legal software market** -- Westlaw, LexisNexis, Clio all use subscription pricing

### 12.2 Pricing Tiers (3 Tiers)

| Tier | Price | Cases | Docs/Case | Search Pipeline | Key Features |
|------|-------|-------|-----------|-----------------|-------------|
| Free | $0 | 3 | 100 | 2-stage (BM25 + Legal-BERT) | Basic search, provenance, citation extraction |
| Pro | $29/mo | Unlimited | Unlimited | 3-stage (+ ColBERT rerank) | Auto-sync, entity graph, citation network, priority support |
| Firm | $99/mo per seat | Unlimited | Unlimited | 3-stage | Multi-seat, shared case export/import, priority support |

### 12.3 Feature Gating Table

| Feature | Free | Pro | Firm |
|---------|------|-----|------|
| Case creation | 3 max | Unlimited | Unlimited |
| Documents per case | 100 max | Unlimited | Unlimited |
| BM25 search (E13) | Yes | Yes | Yes |
| Legal-BERT dense search (E1) | Yes | Yes | Yes |
| SPLADE sparse search (E6) | Yes | Yes | Yes |
| ColBERT-v2 rerank (E12) | No | Yes | Yes |
| Provenance & citations | Yes | Yes | Yes |
| Legal entity extraction | Basic | Full graph | Full graph |
| Citation network | No | Yes | Yes |
| Folder auto-sync | No | Yes | Yes |
| Case export/import | No | No | Yes |
| Concurrent seats | 1 | 1 | N (per key) |

### 12.4 License Key Format

```
TIER-SEATS-EXPIRY-PAYLOAD-SIGNATURE
```

Ed25519 signed. Encoded as base64 for easy copy-paste. See [PRD 07 Section 10](PRD_07_CASE_MANAGEMENT.md#10-license-validation) for the `LicenseKey` struct and validation code.

Key fields:
- `tier`: `free` / `pro` / `firm`
- `seats`: 1 for Free/Pro, N for Firm
- `expires_at`: Unix timestamp (subscription end date + 30-day grace)
- `customer_id`: Opaque identifier (for support, not tracking)
- `signature`: 64-byte Ed25519 signature over the payload

### 12.5 Payment Flow

```
User visits casetrack.dev/pricing
  → Selects tier (Pro or Firm)
  → Lemon Squeezy checkout (handles payment, tax, invoicing)
  → Webhook fires → key server generates Ed25519-signed license key
  → User receives key via email
  → Pastes key into CaseTrack:
      casetrack --license CTPRO-... (CLI flag)
      OR
      CASETRACK_LICENSE=CTPRO-... (env var)
      OR
      license_key = "CTPRO-..." in ~/Documents/CaseTrack/config.toml
  → Key validated offline via Ed25519 public key embedded in binary
  → Features unlocked based on tier in key
```

No network call required for validation. No usage tracking. No telemetry.

### 12.6 Renewal Flow

```
Lemon Squeezy auto-renews subscription monthly
  → Webhook → Key server generates new key with extended expiry
  → Email to user with new key

  OR (automatic, non-blocking):
  → CaseTrack checks casetrack.dev/api/license/{customer_id} on startup
  → If new key available, auto-updates locally cached key
  → If no network, uses cached key (30-day grace period after expiry)
  → Startup check is non-blocking: server starts immediately, check runs in background
```

### 12.7 Grace Period & Expiry

- License key `expires_at` = subscription end date + 30 days grace
- During grace period: full functionality, warning message on startup
- After grace period expires: graceful downgrade to Free tier
- **No data loss, no lockout** -- only feature reduction
- Clear message: `"Pro license expired. Renewing at casetrack.dev/account"`
- Cases beyond Free limit (>3) become read-only, not deleted

### 12.8 Key Server (Minimal)

The key server is a single-purpose service that:

1. Receives Lemon Squeezy webhooks on subscription events (created, renewed, cancelled)
2. Generates Ed25519-signed license keys with the private key
3. Stores keys for the `/api/license/{customer_id}` renewal endpoint
4. Sends keys via email (Lemon Squeezy handles this natively)

That's it. No usage metering, no analytics, no user tracking.

### 12.9 Anti-Patterns (What We Explicitly Don't Build)

| Anti-Pattern | Why Not |
|-------------|---------|
| Per-call metering | Requires phoning home -- violates privacy guarantee |
| Credit system | Adds complexity for users; unpredictable costs |
| Usage counters | Requires local usage DB + sync -- over-engineered |
| Telemetry server | Violates 100% local promise |
| Seat-counting enforcement daemon | Over-engineering; trust the license key |
| "Phone home" requirement | Breaks offline-first guarantee |
| Complex sync protocols | Unnecessary for flat subscription |
| Token-based auth | Adds server dependency; Ed25519 keys are simpler |

---

## 13. Implementation Roadmap

### Phase 1: Foundation

```
PROJECT SETUP
  [ ] Create workspace with crates/casetrack and crates/casetrack-core
  [ ] Configure Cargo.toml workspace dependencies
  [ ] Set up GitHub repo + CI (ci.yml)
  [ ] Implement error types (error.rs) including LegalCitationParseError, CaseNotFound
  [ ] Implement config + CLI parsing (config.rs, cli.rs)
  [ ] Set up tracing/logging to stderr

CASE MANAGEMENT
  [ ] Implement Case, CaseType, CaseStatus structs
  [ ] CaseType variants: Litigation, Contract, RegulatoryCompliance, IntellectualProperty, Corporate, Other
  [ ] Implement CaseRegistry (create, list, switch, delete)
  [ ] Implement CaseHandle (open case DB, column families)
  [ ] RocksDB configuration (rocks_options)
  [ ] Schema versioning (check_and_migrate)
  [ ] Unit tests for case operations

MCP SERVER SKELETON
  [ ] Set up rmcp server with stdio transport
  [ ] Register create_case, list_cases, switch_case, delete_case tools
  [ ] Test with Claude Code manually
```

### Phase 2: Legal Document Processing

```
PDF PROCESSING
  [ ] Implement PdfProcessor (native text extraction)
  [ ] PDF metadata extraction
  [ ] Scanned page detection heuristic
  [ ] Page/paragraph/line detection

DOCX PROCESSING
  [ ] Implement DocxProcessor
  [ ] Paragraph and heading extraction
  [ ] Section break handling
  [ ] Legal clause numbering detection (1.1, 1.2, etc.)

XLSX PROCESSING
  [ ] Implement XlsxProcessor (calamine crate)
  [ ] Sheet enumeration and cell extraction
  [ ] Table structure preservation
  [ ] Header row detection

LEGAL-AWARE CHUNKING (2000-character chunks, 10% overlap -- see PRD 06)
  [ ] Implement DocumentChunker (2000-char target, 200-char overlap, paragraph-aware)
  [ ] Character counting (not token-based)
  [ ] Long paragraph splitting (>2200 chars)
  [ ] Provenance attachment per chunk (file path, document name, page, paragraph, line, char offsets)
  [ ] Chunk boundary validation (min 400 chars, max 2200 chars)
  [ ] Legal clause boundary preservation (never split mid-clause)
  [ ] Q&A pair preservation (deposition transcripts, interrogatories)

LEGAL CITATION EXTRACTION
  [ ] Bluebook case citation parser ("Brown v. Board of Education, 347 U.S. 483 (1954)")
  [ ] Statute citation parser ("42 U.S.C. § 1983", "28 C.F.R. § 50.10")
  [ ] Regulatory citation parser ("Fed. R. Civ. P. 12(b)(6)")
  [ ] Citation normalization and deduplication
  [ ] Citation linking to source chunks

STORAGE (Per-case isolated databases -- see PRD 04)
  [ ] Store chunks in RocksDB (one DB per case)
  [ ] Store document metadata
  [ ] Store provenance records (full path, page, paragraph, line, char offsets per chunk)
  [ ] Duplicate detection (SHA256)
  [ ] ingest_document MCP tool
  [ ] list_documents, get_document, delete_document tools
  [ ] get_chunk, get_document_chunks, get_source_context provenance tools (see PRD 09)
```

### Phase 3: Legal Embedding & Search

```
MODEL MANAGEMENT
  [ ] Model download via hf-hub (with retry)
  [ ] Download Legal-BERT-base from nlpaueb/legal-bert-base-uncased
  [ ] Download SPLADE from naver/splade-cocondenser-ensembledistil
  [ ] Download ColBERT-v2 from colbert-ir/colbertv2.0 (Pro tier)
  [ ] Model spec definitions (repo, files, sizes)
  [ ] First-run download flow
  [ ] Model existence checking

EMBEDDING ENGINE
  [ ] ONNX Runtime setup (Environment, Session)
  [ ] E1 (Legal-BERT-base 768D dense embedding -- legal domain optimized)
  [ ] E6 (SPLADE sparse embedding with legal term expansion)
  [ ] Batch embedding for ingestion
  [ ] Store embeddings in RocksDB

BM25 INDEX (E13)
  [ ] Tokenization (lowercase, stopword removal, legal abbreviation expansion)
  [ ] Inverted index (posting lists in RocksDB)
  [ ] BM25 scoring formula
  [ ] Index update during ingestion

SEARCH ENGINE
  [ ] Stage 1: BM25 recall (E13)
  [ ] Stage 2: Semantic ranking (E1 Legal-BERT + E6 SPLADE via RRF)
  [ ] Cosine similarity (768D), sparse dot product
  [ ] search_documents MCP tool
  [ ] Result formatting with legal citations and provenance
```

### Phase 4: Pro Features

```
PRO EMBEDDERS
  [ ] E12 (ColBERT-v2 token-level)
  [ ] Stage 3: ColBERT-v2 rerank (MaxSim)

LICENSE SYSTEM
  [ ] ed25519 key validation
  [ ] Online activation (Lemon Squeezy)
  [ ] Offline cache (30-day)
  [ ] Feature gating per tier
  [ ] Upgrade prompts in error messages

FOLDER INGESTION & SYNC
  [ ] ingest_folder tool (recursive directory walking via walkdir)
  [ ] SHA256 duplicate detection (skip already-ingested files)
  [ ] sync_folder tool (differential sync: new/changed/deleted detection)
  [ ] sync_folder dry_run mode (preview changes without applying)
  [ ] sync_folder remove_deleted option (remove docs whose source files are gone)
  [ ] File extension filtering
  [ ] Progress reporting (per-file status via stderr logging)
  [ ] Error collection and summary for batch operations

REINDEXING & EMBEDDING FRESHNESS
  [ ] reindex_document tool (delete old chunks/embeddings, re-extract, re-chunk, re-embed)
  [ ] reindex_case tool (full rebuild of all documents in a case)
  [ ] reparse=false mode (keep chunks, rebuild embeddings only -- fast tier upgrade path)
  [ ] skip_unchanged mode (only reindex docs whose source SHA256 changed or embeddings incomplete)
  [ ] get_index_status tool (health check: per-document embedder coverage, SHA256 staleness)
  [ ] Embedder coverage tracking (store which embedders were used per chunk)
  [ ] Automatic stale detection (compare stored SHA256 vs source file on disk)
  [ ] Force reindex flag (rebuild even if SHA256 matches)

AUTO-SYNC & FOLDER WATCHING (see PRD 09 Section 3)
  [ ] WatchManager struct (manages all active watches)
  [ ] notify crate integration (cross-platform OS file notifications)
      - inotify (Linux), FSEvents (macOS), ReadDirectoryChangesW (Windows)
  [ ] watches.json persistence (survives server restarts)
  [ ] FolderWatch config struct (folder_path, schedule, auto_remove, extensions)
  [ ] SyncSchedule enum (OnChange, Interval, Daily, Manual)
  [ ] Real-time event processing with 2-second debounce
  [ ] Event batching (Created -> ingest, Modified -> reindex, Deleted -> remove)
  [ ] Scheduled sync runner (tokio interval, checks every 60 seconds)
  [ ] watch_folder MCP tool
  [ ] unwatch_folder MCP tool
  [ ] list_watches MCP tool
  [ ] set_sync_schedule MCP tool
  [ ] Restore watches on server startup (WatchManager::init)
  [ ] Graceful shutdown (stop watchers, flush pending events)
```

### Phase 4b: Storage Lifecycle Management (see PRD 04 Section 13)

```
STORAGE MONITORING
  [ ] StorageSummary struct (total_bytes, models_bytes, cases breakdown)
  [ ] CaseStorageInfo struct (case_id, storage_bytes, days_inactive, has_stale_embeddings)
  [ ] compute_total_usage (walkdir-based recursive disk usage calculation)
  [ ] Startup storage check (log warn at >70%, >90% of budget)
  [ ] storage_budget_gb config field (default: 10 GB)
  [ ] get_storage_summary MCP tool (per-case usage, staleness, budget warnings)

CASE LIFECYCLE EXTENSIONS
  [ ] close_case MCP tool (set status to Closed, read-only)
  [ ] archive_case MCP tool (set status to Archived, auto-compact all CFs)
  [ ] CaseHandle::compact_all() (full compaction on all 15 column families)
  [ ] compact_case MCP tool (manual compaction trigger)
  [ ] Purged status variant in CaseStatus enum

AUTOMATIC CLEANUP
  [ ] Cascade original file deletion on delete_document
  [ ] Background compaction after delete_document (affected CFs only)
  [ ] Stale case detection (>6 months since last search/ingestion)
  [ ] days_inactive field in list_cases response

CLI MAINTENANCE COMMANDS
  [ ] strip-embeddings --case --embedder e12 (remove unused embedder vectors)
  [ ] purge-archived --output (export archived cases to .ctcase ZIP, delete expanded DB)
  [ ] purge-archived --case (single case export + delete)
  [ ] Purged case registry tracking (export_path recorded)
```

### Phase 4c: Legal Knowledge Graph

```
LEGAL ENTITY EXTRACTION (runs during ingestion, after chunking)
  [ ] Entity extraction pipeline (post-chunk processing step)
  [ ] Legal-specific regex extractors:
      - Case citations (Bluebook format, statute references)
      - Date patterns (filing dates, deadlines, statute of limitations)
      - Monetary amounts ("$1,250,000.00 in damages", "1.25 million dollars")
      - Court references (District Court, Circuit Court, Supreme Court)
  [ ] Legal NER-based extractors:
      - Party names (plaintiffs, defendants, third parties)
      - Judge names and attorney names
      - Organization names (law firms, companies, agencies)
      - Legal concepts and causes of action
  [ ] Entity deduplication (same entity across chunks/documents)
  [ ] Entity storage in `entities` and `entity_index` column families
  [ ] EntityMention records linking entities to chunks with char offsets

CITATION NETWORK & REFERENCE EXTRACTION
  [ ] Legal citation parser (Bluebook cases, statutes, regulations, rules)
  [ ] Citation normalization and canonical form (dedup across documents)
  [ ] ReferenceRecord storage with source_doc, target, context
  [ ] Reference type classification (CaseLaw, Statute, Regulation, Rule, Contract, Exhibit)
  [ ] Citation network storage in `references` column family
  [ ] Cross-document citation linking (Doc A cites same case as Doc B)
  [ ] Citation frequency analysis (most-cited authorities)

PARTY RELATIONSHIP GRAPH
  [ ] Party relationship extraction (plaintiff vs. defendant, attorney-client)
  [ ] Co-occurrence detection (entities appearing in same chunks)
  [ ] Cross-document entity linking (same party across filings)
  [ ] Graph storage in `knowledge_graph` column family
  [ ] Graph traversal queries (shortest path, neighbors, clusters)

DOCUMENT GRAPH
  [ ] DocRelationship storage in `doc_graph` column family
  [ ] Relationship types: SharedCitations, SharedParties, SemanticSimilar, VersionOf, Exhibits, ResponseTo
  [ ] Automatic relationship detection during ingestion:
      - SharedCitations: documents citing same legal authorities
      - SharedParties: documents involving same parties
      - SemanticSimilar: E1 cosine > 0.75 between document-level embeddings
  [ ] Chunk similarity graph in `chunk_graph` column family
  [ ] Cross-chunk similarity edges (E1 cosine > 0.8 between chunks)

CASE SUMMARY
  [ ] CaseSummary builder (aggregates parties, citations, relationships per case)
  [ ] Party extraction and role classification (plaintiffs, defendants, counsel, judges)
  [ ] Key date extraction and litigation timeline construction
  [ ] Cause of action extraction from content analysis
  [ ] Citation statistics (most-cited authorities in the case)
  [ ] Party statistics (most-mentioned entities)
  [ ] CaseStatistics computation (doc count, chunk count, entity count, citation count)
  [ ] Case summary storage in `case_summary` column family
  [ ] Incremental case summary updates (on ingest/delete/reindex)

CONTEXT GRAPH MCP TOOLS (18 tools -- see PRD 09 Section 2b)
  [ ] Case Overview tools:
      - get_case_summary (parties, causes of action, key dates, key citations)
      - get_case_timeline (chronological events: filing dates, hearings, deadlines)
      - get_case_statistics (counts, coverage, health metrics)
  [ ] Entity & Citation tools:
      - list_entities (filter by type: party, judge, attorney, court)
      - get_entity_mentions (all mentions of an entity across documents)
      - search_entity_relationships (entities connected via shared documents)
      - get_entity_graph (party relationship visualization)
      - list_citations (all cited legal authorities with citation counts)
      - get_citation_references (all documents citing a specific authority)
  [ ] Document Navigation tools:
      - get_document_structure (headings, sections, page count, entity/citation summary)
      - browse_pages (paginated page content with entities highlighted)
      - find_related_documents (documents related via citations, parties, or semantics)
      - get_related_documents (knowledge-graph-first document discovery)
      - list_documents_by_type (filter by: complaint, answer, motion, contract, exhibit)
      - traverse_chunks (sequential chunk navigation with prev/next)
  [ ] Advanced Search tools:
      - search_similar_chunks (find chunks semantically similar to a given chunk)
      - compare_documents (side-by-side entity, citation, and semantic comparison)
      - find_document_clusters (group documents by topic/entity similarity)
```

### Phase 5: OCR & Polish

```
OCR
  [ ] Tesseract integration
  [ ] Image preprocessing (grayscale, contrast)
  [ ] Scanned PDF detection + automatic OCR
  [ ] ingest_image support
  [ ] OCR confidence in provenance

MEMORY MANAGEMENT (16GB target)
  [ ] System RAM detection (sysinfo)
  [ ] Auto memory tier selection (16GB target: Legal-BERT + SPLADE + BM25 comfortably)
  [ ] Lazy model loading
  [ ] Memory pressure handling (model unloading)

CROSS-PLATFORM
  [ ] Test on macOS (Intel + Apple Silicon)
  [ ] Test on Windows 10/11
  [ ] Test on Ubuntu
  [ ] Path handling (~ expansion, separators)
  [ ] CoreML / DirectML execution providers
```

### Phase 6: Distribution

```
DISTRIBUTION
  [ ] Install script (install.sh, install.ps1)
  [ ] --setup-claude-code command
  [ ] MCPB bundle creation script
  [ ] manifest.json
  [ ] Release pipeline (GitHub Actions)
  [ ] Cross-platform builds
  [ ] Binary signing (macOS notarization, Windows Authenticode)

UPDATE MECHANISM
  [ ] Version check on startup (non-blocking)
  [ ] --update self-update command
  [ ] --uninstall command
  [ ] Data migration on upgrade

DOCUMENTATION
  [ ] README.md
  [ ] CHANGELOG.md
  [ ] Landing page content
```

---

## 14. Success Metrics

### 14.1 Product Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Install success rate | >95% | Opt-in telemetry |
| Time to first search | <5 minutes | User testing |
| Search relevance (top 5) | >85% | Manual evaluation |
| Provenance accuracy | 100% | Automated tests |
| Legal citation accuracy | >95% | Automated tests against Bluebook corpus |
| Crash rate | <0.1% | Error reporting |

### 14.2 Performance Metrics

| Metric | Free Tier | Pro Tier |
|--------|-----------|----------|
| Search latency (p95) | <150ms | <250ms |
| Ingestion speed | <1.5s/page | <1s/page |
| RAM usage (idle) | <1.5GB | <2GB |
| RAM usage (search) | <2.5GB | <3GB |
| Model download | <3 min | <5 min |
| Binary size | <30MB | <30MB |

### 14.3 Business Metrics

| Metric | Year 1 | Year 2 | Year 3 |
|--------|--------|--------|--------|
| Downloads | 10,000 | 50,000 | 200,000 |
| Free users | 5,000 | 25,000 | 100,000 |
| Pro conversions (2%) | 100 | 500 | 2,000 |
| ARR | $35K | $174K | $696K |

---

## Appendix A: File Size Estimates

| Component | Size |
|-----------|------|
| Binary (release, stripped) | ~15-25MB |
| MCPB bundle (all platforms) | ~50MB |
| Models (Free tier: Legal-BERT + SPLADE) | ~330MB |
| Models (Pro tier: + ColBERT-v2) | ~550MB |
| Case database (per 100 docs) | ~5-50MB |
| Total install (Free, 1 case) | ~370MB |
| Total install (Pro, 10 cases) | ~1.1GB |

## Appendix B: Comparison with Alternatives

| Feature | CaseTrack | Traditional SaaS | DIY RAG |
|---------|-----------|-------------------|---------|
| Price | $0-99/mo | $200-400/mo | Free |
| Install time | 2 min | N/A (SaaS) | Hours |
| Runs locally | Yes | No | Yes |
| No GPU required | Yes | N/A | Usually no |
| Claude integration | Native MCP | No | Manual |
| Provenance | Always | Sometimes | DIY |
| Legal-domain models | Legal-BERT-base (768D) | Generic embeddings | Generic embeddings |
| Legal citation extraction | Built-in (Bluebook, statutes) | Add-on or manual | DIY |
| Privacy | 100% local | Cloud | Local |
| Offline capable | Yes | No | Yes |

---

*CaseTrack PRD v5.1.0 -- Document 10 of 10*

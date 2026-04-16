# PRD 01: CaseTrack Overview

## One-Click Document Intelligence for Claude Code & Claude Desktop

**Version**: 4.0.0
**Date**: 2026-01-28
**Status**: Draft
**Scope**: Fresh greenfield project build
**Language**: Rust (entire project -- no exceptions)

> **BUILD MANDATE**: CaseTrack is built entirely in Rust. The binary crate, core
> library, MCP server, document processing, embedding engine, storage layer,
> search engine, license validation, CLI, and all tooling are Rust. The only
> non-Rust code is a Python helper script for one-time ONNX model conversion
> (a build-time tool, not shipped to users). There is no JavaScript, TypeScript,
> Python, Go, or C++ in the product. All dependencies are Rust crates. The
> output is a single statically-linked Rust binary with zero runtime dependencies.

> **PROVENANCE MANDATE**: Every piece of information CaseTrack returns MUST trace
> back to its exact source. This is non-negotiable. The provenance chain is:
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

---

## Document Index

This PRD is split across 10 documents. Each is self-contained but references the others.

| Doc | Title | Covers |
|-----|-------|--------|
| **01 (this)** | Overview | Executive summary, vision, principles, glossary |
| [02](PRD_02_TARGET_USER_HARDWARE.md) | Target User & Hardware | Users, hardware tiers, performance targets |
| [03](PRD_03_DISTRIBUTION_INSTALLATION.md) | Distribution & Installation | Channels, MCPB, manifest, install flows, updates |
| [04](PRD_04_STORAGE_ARCHITECTURE.md) | Storage Architecture | Local storage, RocksDB schema, data versioning |
| [05](PRD_05_EMBEDDER_STACK.md) | Embedder Stack | 4 embedders, ONNX, model management |
| [06](PRD_06_DOCUMENT_INGESTION.md) | Document Ingestion | PDF, DOCX, XLSX, OCR, chunking |
| [07](PRD_07_CASE_MANAGEMENT.md) | Collection Management & Provenance | Collection model, isolation, references |
| [08](PRD_08_SEARCH_RETRIEVAL.md) | Search & Retrieval | 3-stage pipeline, RRF, ranking |
| [09](PRD_09_MCP_TOOLS.md) | MCP Tools | All tool specs, examples, error responses |
| [10](PRD_10_TECHNICAL_BUILD.md) | Technical Build Guide | Bootstrap, crate structure, CI/CD, testing, security |

---

## 1. What is CaseTrack?

CaseTrack is a **one-click installable MCP server** that plugs into **Claude Code** and **Claude Desktop**, giving Claude the ability to ingest, search, and analyze **any documents**. It supports PDF, DOCX, XLSX, and scanned images. Everything runs on the user's machine -- **all embeddings, vectors, and databases are stored locally** on the user's device with zero cloud dependencies.

The name "CaseTrack" reflects its ability to track any *case* -- whether that is a business case, a use case, a research case, or a project case. It organizes documents into collections and builds a knowledge graph that lets Claude answer questions with full source provenance.

```
+---------------------------------------------------------------------------+
|  CASETRACK -- "Install once. Everything runs on YOUR machine."            |
+---------------------------------------------------------------------------+
|  - Ingests PDFs, DOCX, XLSX, scanned images                              |
|  - Embeds documents with 4 specialized embedders                         |
|  - Stores all vectors/embeddings locally (RocksDB)                       |
|  - Provides semantic search with full source citations                   |
|  - MCP server for Claude Code + Claude Desktop                           |
|  - Your data NEVER leaves your computer                                  |
+---------------------------------------------------------------------------+
```

---

## 2. The Problem

Professionals waste hours searching through documents:

- **Keyword search fails**: "revenue decline" won't find "decrease in quarterly earnings"
- **No AI integration**: Can't ask questions about documents in natural language
- **No provenance**: When you find something, you can't cite the exact source
- **Complex tools**: Existing document intelligence tools require IT departments and training
- **Expensive**: Enterprise document platforms cost $200-500+/seat/month
- **Scattered files**: Thousands of documents spread across folders with no unified search

---

## 3. The Solution

CaseTrack solves this with:

1. **One-click install** -- single command or MCPB file, embedders and database included
2. **100% local** -- all data stored on YOUR device in per-collection RocksDB instances (collection and customer isolation)
3. **4 specialized embedders** -- semantic search that understands document content across domains
4. **Full provenance** -- every answer cites source file path, document name, page, paragraph, and line number
5. **2000-char chunks** -- 10% overlap, each chunk stores its exact origin
6. **Claude Code + Desktop** -- works with both CLI and Desktop via MCP stdio
7. **Auto-sync** -- watches folders for changes; optional scheduled reindexing (daily/hourly/custom)
8. **Runs anywhere** -- 8GB laptop, no GPU needed; free tier useful, Pro $29/month

---

## 4. Key Metrics

| Metric | Target |
|--------|--------|
| Install time | < 2 minutes |
| First search after install | < 5 minutes |
| Search latency | < 200ms on any laptop |
| PDF ingestion | < 1 second per page |
| RAM usage | < 2GB peak |
| Model download | ~400MB one-time |

---

## 5. Vision Statement

> **Any professional can ask Claude questions about their documents and get accurate, cited answers -- without IT support, cloud accounts, or technical knowledge.**

---

## 6. Design Principles

```
DESIGN PRINCIPLES
=================================================================================

1. ZERO CONFIGURATION
   User downloads file -> double-clicks -> starts using
   No terminal, no config files, no environment variables
   Claude Code: single curl command + one settings.json entry

2. RUNS ON ANYTHING
   8GB RAM laptop from 2020 should work fine
   No GPU required, ever
   Intel, AMD, Apple Silicon all supported

3. PRIVACY FIRST
   Documents never leave the device
   No telemetry, no analytics, no cloud
   Data privacy preserved
   License validation works offline after first activation

4. INSTANT VALUE
   First useful search within 5 minutes of download
   No training required
   Works like asking a research assistant

5. PROVENANCE ALWAYS (THE MOST IMPORTANT PRINCIPLE)
   Every answer includes exact source citation
   Document name, file path, page, paragraph, line number, character offsets
   Every embedding vector links back to its chunk, which links to its source
   Every entity, reference, and graph edge traces to its source chunk
   Timestamps on everything: when ingested, when embedded, when last synced
   One click to view original context
   If you can't cite the source, you can't return the information

6. GRACEFUL DEGRADATION
   Low RAM? Use fewer models (lazy loading)
   Slow CPU? Longer ingestion, same quality
   Free tier? Fewer features, still useful

7. FAIL LOUDLY
   Errors are specific and actionable
   No silent failures -- every operation reports success or explains failure
   MCP error responses include recovery instructions
```

---

## 7. What CaseTrack is NOT

- **Not a document management system**: Use Dropbox/OneDrive/SharePoint for storage
- **Not a cloud service**: Everything runs locally, we never see your data
- **Not an LLM**: CaseTrack provides tools to Claude; it does not generate answers itself
- **Not a file sync tool**: CaseTrack indexes and searches documents; it does not replicate or sync files between devices
- **Not a database admin tool**: No SQL, no queries to write; everything is automatic

---

## 8. Architecture at a Glance

```
+-----------------------------------------------------------------------+
|                         USER'S MACHINE                                 |
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
|  |  +----------+  +--------+  |                                       |
|  |  | Document |  | Search |  |                                       |
|  |  | Parser   |  | Engine |  |                                       |
|  |  | (PDF,    |  +--------+  |                                       |
|  |  |  DOCX,   |  +--------+  |                                       |
|  |  |  XLSX)   |  | 4 ONNX |  |                                       |
|  |  +----------+  | Models |  |                                       |
|  |  +----------+  +--------+  |                                       |
|  |  | Chunking |              |                                       |
|  |  | Engine   |              |                                       |
|  |  +----------+              |                                       |
|  +--------+-------------------+                                       |
|           |                                                           |
|  +--------v-------------------+                                       |
|  | Local Storage              |   ~/Documents/CaseTrack/              |
|  |  +---------+ +-----------+ |                                       |
|  |  |Collect. | | Collect.  | |   Each collection = isolated RocksDB  |
|  |  |A RocksDB| | B RocksDB | |   Vectors, chunks, provenance        |
|  |  +---------+ +-----------+ |                                       |
|  +----------------------------+                                       |
|                                                                       |
|  NOTHING LEAVES THIS MACHINE                                          |
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
| PDF | pdf-extract + lopdf | Pure Rust |
| DOCX | docx-rs | Pure Rust |
| XLSX | calamine | Pure Rust spreadsheet reader (XLS, XLSX, ODS) |
| OCR | Tesseract (bundled) | Best open-source OCR |
| Model Download | hf-hub | Hugging Face model registry |
| Serialization | bincode + serde | Fast binary serialization for vectors |
| Async | tokio | Standard Rust async runtime |
| File watching | notify | Cross-platform OS file notifications (inotify/FSEvents/ReadDirectoryChanges) |
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
| **Chunk** | A 2000-character segment of a document with 10% (200 char) overlap, the unit of search. Every chunk stores full provenance: source file path, document name, page number, paragraph, line number, and character offsets. |
| **Collection** | A group of related documents stored in an isolated database. Each collection has its own RocksDB instance, embeddings, and knowledge graph. |
| **Collection Map** | A per-collection summary structure containing key parties, important dates, core topics, top references, entity statistics, and document counts. Built incrementally during ingestion. |
| **Context Graph** | The graph layer built on top of chunks and embeddings that stores entities, references, document relationships, chunk similarity edges, and the collection map. Enables AI navigation of large document collections. |
| **Document Graph** | Relationship edges between documents based on shared entities, shared references, semantic similarity, or explicit references (ResponseTo, Amends, Exhibits). |
| **Embedder** | A model that converts text to a numerical vector |
| **Entity** | A named thing extracted from document text: person, organization, date, monetary amount, location, or concept. Stored with mentions linking to source chunks. |
| **Knowledge Graph** | The combined structure of entities, references, document relationships, and chunk similarity edges that represents the interconnections within a collection. Built automatically during ingestion and used by Claude to navigate and answer questions across documents. |
| **MCP** | Model Context Protocol -- standard for AI tool integration |
| **MCPB** | MCP Bundle -- a ZIP file format for distributing MCP servers |
| **ONNX** | Open Neural Network Exchange -- cross-platform ML model format |
| **Provenance** | The exact source location of text: file path, document name, page number, paragraph number, line number, and character offsets. Attached to every chunk and included in every search result and MCP tool response. |
| **Reference Network** | The graph of cross-references between documents -- which documents reference, cite, or relate to other documents. Stored in the `references` column family. |
| **RocksDB** | Embedded key-value database by Meta, used for local storage |
| **RRF** | Reciprocal Rank Fusion -- method to combine search rankings |
| **rmcp** | Official Rust MCP SDK |
| **SPLADE** | Sparse Lexical and Expansion Model -- keyword expansion embedder |
| **stdio** | Standard input/output transport for MCP server communication |

---

*CaseTrack PRD v4.0.0 -- Document 1 of 10*


---

# PRD 02: Target User & Hardware

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Target Users

| Segment | Profile | Key Pain Point | Why CaseTrack |
|---------|---------|---------------|---------------|
| **Primary: Solo Professionals** (consultants, analysts, researchers) | No IT staff, consumer hardware, 5-50 active projects, already uses Claude | Can't semantically search documents; no budget for enterprise platforms ($500+/seat) | Works on existing laptop, no setup, $29/mo or free tier |
| **Secondary: Small Teams** (5-20 people) | Standard business hardware, shared document collections, need consistent search across team members | Manual document review is tedious; need exact citations for reporting and collaboration | Batch ingest folders, search returns cited sources, 70%+ review reduction |
| **Tertiary: Enterprise Departments** | Mixed hardware across the organization, limited IT support for individual tools | Organizing research, finding cross-document connections, accurate citations across large document sets | Free tier (3 collections), provenance generates proper citations |

---

## 2. User Personas

| Persona | Role / Domain | Hardware | CaseTrack Use | Key Need |
|---------|--------------|----------|---------------|----------|
| **Sarah** | Solo management consultant | MacBook Air M1, 8GB | Ingests client deliverables and research, asks Claude questions about project documents | Zero setup friction |
| **Mike** | Team lead at a market research firm (12 analysts) | Windows 11, 16GB | Team license, each analyst searches their own collections | Windows support, multi-seat, worth $99/mo |
| **Alex** | Operations coordinator, mid-size company | Windows 10, 8GB | Batch ingests policy documents and reports, builds searchable knowledge bases | Fast ingestion, reliable OCR |

---

## 3. Minimum Hardware Requirements

```
MINIMUM REQUIREMENTS (Must Run)
=================================================================================

CPU:     Any 64-bit processor (2018 or newer recommended)
         - Intel Core i3 or better
         - AMD Ryzen 3 or better
         - Apple M1 or better

RAM:     8GB minimum
         - 16GB recommended for large collections (1000+ pages)

Storage: 5GB available
         - 400MB for embedding models (one-time download)
         - 4.6GB for collection data (scales with usage)
         - SSD strongly recommended (HDD works but slower ingestion)

OS:      - macOS 11 (Big Sur) or later
         - Windows 10 (64-bit) or later
         - Ubuntu 20.04 or later (other Linux distros likely work)

GPU:     NOT REQUIRED
         - Optional: Metal (macOS), CUDA (NVIDIA), DirectML (Windows)
         - GPU provides ~2x speedup for ingestion if available
         - Search latency unaffected (small batch sizes)

Network: Required ONLY for:
         - Initial model download (~400MB, one-time)
         - License activation (one-time, then cached offline)
         - Software updates (optional)
         ALL document processing is 100% offline

Prerequisites:
         - Claude Code or Claude Desktop installed
         - No other runtime dependencies (Rust binary is self-contained)
         - Tesseract OCR bundled with binary (no separate install)
```

---

## 4. Performance by Hardware Tier

### 4.1 Ingestion Performance

| Hardware | 50-page PDF | 500-page PDF | OCR (50 scanned pages) |
|----------|-------------|--------------|------------------------|
| **Entry** (M1 Air 8GB) | 45 seconds | 7 minutes | 3 minutes |
| **Mid** (M2 Pro 16GB) | 25 seconds | 4 minutes | 2 minutes |
| **High** (i7 32GB) | 20 seconds | 3 minutes | 90 seconds |
| **With GPU** (RTX 3060) | 10 seconds | 90 seconds | 45 seconds |

### 4.2 Search Performance

| Hardware | Free Tier (2-stage) | Pro Tier (3-stage) | Concurrent Models |
|----------|--------------------|--------------------|-------------------|
| **Entry** (M1 Air 8GB) | 100ms | 200ms | 2 (lazy loaded) |
| **Mid** (M2 Pro 16GB) | 60ms | 120ms | 3 |
| **High** (i7 32GB) | 40ms | 80ms | 3 (all loaded) |
| **With GPU** (RTX 3060) | 20ms | 50ms | 3 (all loaded) |

### 4.3 Memory Usage

| Scenario | RAM Usage |
|----------|-----------|
| Idle (server running, no models loaded) | ~50MB |
| Free tier (3 models loaded) | ~800MB |
| Pro tier (all models loaded) | ~1.5GB |
| During ingestion (peak) | +300MB above baseline |
| During search (peak) | +100MB above baseline |

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
| **Full** | 16GB+ | All 3 neural models + BM25 simultaneously | Zero load latency, parallel embedding |
| **Standard** | 8-16GB | E1 + BM25 always (~400MB); others lazy-loaded | ~200ms first-use penalty; models stay loaded until memory pressure |
| **Constrained** | <8GB | E1 + BM25 only (~400MB); others loaded one-at-a-time | Sequential embedding, higher search latency, startup warning |

**Detection**: On startup, check available RAM via `sysinfo` crate. Set tier automatically, log the decision. User override: `--memory-mode=full|standard|constrained`.

---

*CaseTrack PRD v4.0.0 -- Document 2 of 10*


---

# PRD 03: Distribution & Installation

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

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
  "version": "1.0.0",
  "display_name": "CaseTrack Document Intelligence",
  "description": "Ingest PDFs, Word docs, Excel spreadsheets, and scans. Search with AI. Every answer cites the source.",

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
      "description": "Where to store collections and models on your computer",
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
      "reason": "Download embedding models on first use"
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
   | CaseTrack lets you search documents with AI.           |
   | All processing happens on your computer.               |
   |                                                        |
   | Data Location:  [~/Documents/CaseTrack            ] [F]|
   | License Key:    [optional - blank for free tier   ] [L]|
   |                                                        |
   | [Y] Read and write files in your Data Location        |
   | [Y] Download AI models from huggingface.co (~400MB)   |
   | [N] NOT send your documents anywhere                  |
   |                                                        |
   |                         [Cancel]  [Install Extension]  |
   +-------------------------------------------------------+
4. First Run: Server starts, downloads missing models (~400MB) in background
5. Ready:     CaseTrack icon appears in Extensions panel
```

---

## 6. First-Run Experience

Initialization sequence on first launch:

```
1. Create directory structure: models/, collections/ (registry.db created by RocksDB::open)
2. Check for missing models based on tier; download any missing (with progress logging)
3. Open or create registry database
4. Validate license (offline-first)
5. Log ready state: tier + collection count
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
        repo: "BAAI/bge-small-en-v1.5",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 65,
        required: true,
    },
    ModelSpec {
        id: "e6",
        repo: "naver/splade-cocondenser-selfdistil",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 55,
        required: true,
    },
    ModelSpec {
        id: "e12",
        repo: "colbert-ir/colbertv2.0-msmarco-passage",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 110,
        required: false,
    },
];
// E13 (BM25) requires no model download -- pure algorithm
```

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
1. Asks for confirmation ("This will remove CaseTrack. Your data will NOT be deleted.")
2. Removes the binary from PATH
3. Removes the Claude Code/Desktop configuration entry
4. Prints location of data directory for manual cleanup
5. Does NOT delete `~/Documents/CaseTrack/` (user's data is sacred)

### 8.2 Manual Uninstall

```
# Remove binary
rm ~/.local/bin/casetrack   # macOS/Linux
# OR delete %LOCALAPPDATA%\CaseTrack\ on Windows

# Remove Claude Code config (edit ~/.claude/settings.json, remove "casetrack" key)

# Optionally remove data (YOUR CHOICE -- this deletes all collections):
rm -rf ~/Documents/CaseTrack/
```

---

*CaseTrack PRD v4.0.0 -- Document 3 of 10*


---

# PRD 04: Storage Architecture

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Core Principle: Everything on YOUR Machine

**CaseTrack stores ALL data locally on the user's device:**

- **Embedding models**: Downloaded once, stored in `~/Documents/CaseTrack/models/`
- **Vector embeddings**: Stored in RocksDB on your device
- **Document chunks**: Stored in RocksDB on your device
- **Collection databases**: Each collection is an isolated RocksDB instance
- **Original documents**: Optionally copied to your CaseTrack folder

**Nothing is sent to any cloud service. Ever.**

---

## 2. Directory Structure

```
~/Documents/CaseTrack/                       <-- All CaseTrack data lives here
|
|-- config.toml                              <-- User configuration (optional)
|-- watches.json                             <-- Folder watch registry (auto-sync)
|
|-- models/                                  <-- Embedding models (~400MB)
|   |-- bge-small-en-v1.5/                     Downloaded on first use
|   |   |-- model.onnx                         Cached permanently
|   |   +-- tokenizer.json                     No re-download needed
|   |-- splade-distil/
|   +-- colbert-small/
|
|-- registry.db/                             <-- Collection index (RocksDB)
|   +-- [collection metadata, schema version]
|
+-- collections/                             <-- Per-collection databases
    |
    |-- {collection-uuid-1}/                 <-- Collection "Project Alpha"
    |   |-- collection.db/                     (Isolated RocksDB instance)
    |   |   |-- documents     CF              Document metadata
    |   |   |-- chunks        CF              Text chunks (bincode)
    |   |   |-- embeddings    CF              All embedder vectors + chunk text + provenance
    |   |   |   |-- e1                        384D vectors
    |   |   |   |-- e6                        Sparse vectors
    |   |   |   +-- ...                       All active embedders
    |   |   |-- bm25_index    CF              Inverted index for keyword search
    |   |   +-- ...                           Additional column families
    |   +-- originals/                        Original files (optional copy)
    |       |-- Report.pdf
    |       |-- Summary.docx
    |       +-- Data.xlsx
    |
    |-- {collection-uuid-2}/                 <-- Collection "Q4 Analysis"
    |   +-- ...                                (Completely isolated)
    |
    +-- {collection-uuid-N}/                 <-- More collections...

CF = RocksDB Column Family
```

---

## 3. Storage Estimates

| Data Type | Size Per Unit | Notes |
|-----------|---------------|-------|
| Models (Free tier) | ~120MB total | E1 + E6 (one-time download) |
| Models (Pro tier) | ~230MB total | All 4 models (one-time download) |
| Registry DB | ~1MB | Scales with number of collections |
| Per document page (Free) | ~30KB | 3 embeddings + chunk text + provenance |
| Per document page (Pro) | ~50KB | 6 embeddings + chunk text + provenance |
| 100-page collection (Free) | ~3MB | |
| 100-page collection (Pro) | ~5MB | |
| 1000-page collection (Pro) | ~50MB | |
| BM25 index per 1000 chunks | ~2MB | Inverted index |

**Example total disk usage:**
- Free tier, 3 collections of 100 pages each: 165MB (models) + 9MB (data) = **~175MB**
- Pro tier, 10 collections of 500 pages each: 370MB (models) + 250MB (data) = **~620MB**

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
| Per-collection isolation | Separate DB instances | Separate files | Separate files |

RocksDB was chosen for: column families (clean separation of data types), prefix iteration (efficient collection listing), and bulk write performance (ingestion throughput).

### 4.2 Column Family Schema

Each collection database uses these column families:

```rust
pub const COLUMN_FAMILIES: &[&str] = &[
    "documents",        // Document metadata
    "chunks",           // Text chunk content
    "embeddings",       // All embedder vectors + chunk text + provenance per chunk
    "bm25_index",       // Inverted index for BM25
    "metadata",         // Collection-level metadata, stats

    // === Context Graph (relationships between documents, chunks, entities) ===
    "entities",         // Extracted entities (person, organization, date, amount, location, concept)
    "entity_index",     // Entity -> chunk mentions index
    "references",       // Cross-document references (shared entities, citations, hyperlinks)
    "doc_graph",        // Document-to-document relationships (similarity, reference links)
    "chunk_graph",      // Chunk-to-chunk relationships (similarity edges, co-reference)
    "knowledge_graph",  // Entity-to-entity relationships, entity-to-chunk mappings
    "collection_map",   // Collection-level summary: key actors, dates, document categories
];
```

### 4.3 Key Schema

```rust
// === Registry DB Keys ===

// Collection listing
"collection:{uuid}"                          -> bincode<Collection>
"schema_version"                             -> u32 (current: 1)

// Folder watches (auto-sync)
"watch:{uuid}"                               -> bincode<FolderWatch>
"watch_collection:{collection_uuid}:{watch_uuid}" -> watch_uuid  (index: watches by collection)

// === Collection DB Keys (per column family) ===

// documents CF
"doc:{uuid}"                       -> bincode<DocumentMetadata>

// chunks CF
"chunk:{uuid}"                     -> bincode<ChunkData>
"doc_chunks:{doc_uuid}:{seq}"      -> chunk_uuid  (index: chunks by document)

// embeddings CF -- each chunk stores all embedder vectors alongside text and provenance
// chunk_id -> { text, provenance, e1_vector, e6_vector, e12_vector, bm25_terms }
"emb:{chunk_uuid}"                 -> bincode<ChunkEmbeddingRecord>

// Legacy per-embedder keys (supported for migration)
"e1:{chunk_uuid}"                  -> [f32; 384] as bytes
"e6:{chunk_uuid}"                  -> bincode<SparseVec>
"e12:{chunk_uuid}"                 -> bincode<TokenEmbeddings>

// bm25_index CF
"term:{term}"                      -> bincode<PostingList>
"doc_len:{doc_uuid}"               -> u32 (document length in tokens)
"stats"                            -> bincode<Bm25Stats> (avg doc length, total docs)

// metadata CF
"collection_info"                  -> bincode<Collection>
"stats"                            -> bincode<CollectionStats>

// === CONTEXT GRAPH COLUMN FAMILIES ===

// entities CF
"entity:{type}:{normalized_name}"  -> bincode<Entity>
// type = person | organization | date | amount | location | concept

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

// collection_map CF (rebuilt after ingestion)
"key_actors"                       -> bincode<Vec<KeyActor>>
"key_dates"                        -> bincode<Vec<KeyDate>>
"key_topics"                       -> bincode<Vec<Topic>>
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

    // Memory budget: ~64MB per open database
    // (allows multiple collections open simultaneously on 8GB machines)
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
}

/// Unified embedding record: all embedder vectors stored alongside chunk text and provenance
#[derive(Serialize, Deserialize)]
pub struct ChunkEmbeddingRecord {
    pub chunk_id: Uuid,
    pub text: String,
    pub provenance: Provenance,
    pub e1_vector: Option<Vec<f32>>,       // 384D dense vector
    pub e6_vector: Option<SparseVec>,      // SPLADE sparse vector
    pub e12_vector: Option<TokenEmbeddings>, // ColBERT per-token embeddings
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
                           +-- text: "Either party may terminate..."
                           +-- provenance: Provenance {
                           |       document_id:        "doc-abc"
                           |       source_file_path:   "/Users/alex/Projects/Alpha/Contract.pdf"
                           |       document_filename:  "Contract.pdf"
                           |       page_number:        12
                           |       paragraph_number:   8
                           |       line_number:        1
                           |       char_start:         2401
                           |       char_end:           4401
                           |       extraction_method:  Native
                           |       ocr_confidence:     None
                           |       chunk_index:        47
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
- For Team tier (5 seats), each seat is a separate installation on a separate machine with its own database
- Customer A's embeddings, vectors, chunks, and provenance records **never touch** Customer B's data
- There is no central database. Each customer IS their own database.

```
CUSTOMER ISOLATION
=================================================================================

Customer A (Sarah's MacBook)         Customer B (Mike's Windows PC)
~/Documents/CaseTrack/               C:\Users\Mike\Documents\CaseTrack\
|-- models/                          |-- models/
|-- registry.db                      |-- registry.db
+-- collections/                     +-- collections/
    |-- {sarah-collection-1}/            |-- {mike-collection-1}/
    +-- {sarah-collection-2}/            |-- {mike-collection-2}/
                                         +-- {mike-collection-3}/

ZERO shared state. ZERO shared databases. ZERO network communication.
Each installation is a completely independent system.
```

### 7.2 Per-Collection Isolation

Each collection is a **completely independent RocksDB instance**:

- Separate database, embeddings, and index files per collection
- No cross-collection queries, shared vectors, or embedding bleed
- Independent lifecycle: deleting Collection A has zero impact on Collection B
- Portable: copy a collection directory to another machine
- Cleanly deletable: `rm -rf collections/{uuid}/`

```rust
/// Opening a collection creates or loads its isolated database
pub struct CollectionHandle {
    db: rocksdb::DB,
    collection_id: Uuid,
    collection_dir: PathBuf,
}

impl CollectionHandle {
    pub fn open(collection_dir: &Path) -> Result<Self> {
        let db_path = collection_dir.join("collection.db");
        let mut opts = rocks_options();

        let cfs = COLUMN_FAMILIES.iter()
            .map(|name| rocksdb::ColumnFamilyDescriptor::new(*name, opts.clone()))
            .collect::<Vec<_>>();

        let db = rocksdb::DB::open_cf_descriptors(&opts, &db_path, cfs)?;

        Ok(Self {
            db,
            collection_id: Uuid::parse_str(
                collection_dir.file_name().unwrap().to_str().unwrap()
            )?,
            collection_dir: collection_dir.to_path_buf(),
        })
    }

    /// Delete this collection entirely
    pub fn destroy(self) -> Result<()> {
        let path = self.collection_dir.clone();
        drop(self); // Close DB handle first
        fs::remove_dir_all(&path)?;
        Ok(())
    }
}
```

---

## 8. Context Graph Data Models

Entities, references, and relationships extracted during ingestion and stored as graph edges for structured collection navigation.

### 8.1 Entity Model

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
    Person, Organization, Date, Amount, Location, Concept,
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
    pub entity_key: String,            // "person:john_smith"
    pub entity_type: EntityType,
    pub name: String,
}
```

### 8.2 Reference Model

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
    InternalCrossRef, ExternalDocument, Hyperlink,
    Standard, Specification, Other,
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

### 8.3 Document Graph Model

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

### 8.4 Collection Summary Model

```rust
/// High-level collection overview, rebuilt after each ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSummary {
    pub key_actors: Vec<KeyActor>,
    pub key_dates: Vec<KeyDate>,
    pub key_topics: Vec<Topic>,
    pub document_categories: HashMap<String, Vec<Uuid>>,
    pub top_references: Vec<ReferenceStat>,
    pub top_entities: Vec<EntityStat>,
    pub statistics: CollectionStatistics,
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
pub struct CollectionStatistics {
    pub total_documents: u32,
    pub total_pages: u32,
    pub total_chunks: u32,
    pub total_entities: u32,
    pub total_references: u32,
    pub storage_bytes: u64,
    pub document_type_breakdown: HashMap<String, u32>,
    pub embedder_coverage: HashMap<String, u32>,
}
```

---

## 9. Folder Watch & Auto-Sync Storage

Watch configurations persist in `~/Documents/CaseTrack/watches.json` (JSON for human readability) to survive server restarts.

### 9.1 Watch Registry

```rust
#[derive(Serialize, Deserialize)]
pub struct WatchRegistry {
    pub watches: Vec<FolderWatch>,
}

#[derive(Serialize, Deserialize)]
pub struct FolderWatch {
    pub id: Uuid,
    pub collection_id: Uuid,
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

### 9.2 Per-Document Sync Metadata

Sync uses `file_hash` and `original_path` from `DocumentMetadata` (see Section 5.1) to detect changes:

```
FOR each file in watched folder:
  1. Compute SHA256 of file
  2. Look up file by original_path in collection DB
  3. IF not found -> new file -> ingest
  4. IF found AND hash matches -> unchanged -> skip
  5. IF found AND hash differs -> modified -> reindex (delete old, re-ingest)

FOR each document in collection DB with original_path under watched folder:
  6. IF source file no longer exists on disk -> deleted
     IF auto_remove_deleted -> delete document from collection
     ELSE -> log warning, skip
```

---

## 10. Backup & Export

### 10.1 Collection Export

Collections can be exported as portable archives:

```
casetrack export --collection "Project Alpha" --output ~/Desktop/project-alpha.ctcollection
```

The `.ctcollection` file is a ZIP containing:
- `collection.db/` -- RocksDB snapshot
- `originals/` -- Original documents (if stored)
- `manifest.json` -- Collection metadata, schema version, embedder versions

### 10.2 Collection Import

```
casetrack import ~/Desktop/project-alpha.ctcollection
```

1. Validates schema version compatibility
2. Creates new collection UUID (avoids collisions)
3. Copies database and originals to `collections/` directory
4. Registers in collection registry

---

## 11. What's Stored Where (Summary)

| Data Type | Storage Location | Format | Size Per Unit |
|-----------|------------------|--------|---------------|
| Collection metadata | `registry.db` | bincode via RocksDB | ~500 bytes/collection |
| Document metadata | `collections/{id}/collection.db` documents CF | bincode | ~200 bytes/doc |
| Text chunks (2000 chars) | `collections/{id}/collection.db` chunks CF | bincode | ~2.5KB/chunk (text + provenance metadata) |
| E1 embeddings (384D) | `collections/{id}/collection.db` embeddings CF | f32 bytes | 1,536 bytes/chunk |
| E6 sparse vectors | `collections/{id}/collection.db` embeddings CF | bincode sparse | ~500 bytes/chunk |
| E12 token embeddings | `collections/{id}/collection.db` embeddings CF | bincode | ~8KB/chunk |
| BM25 inverted index | `collections/{id}/collection.db` bm25_index CF | bincode | ~2MB/1000 chunks |
| Provenance records | Embedded in chunk embedding records | bincode | ~300 bytes/chunk |
| Original documents | `collections/{id}/originals/` | original files | varies |
| ONNX models | `models/` | ONNX format | 35-110MB each |

---

*CaseTrack PRD v4.0.0 -- Document 4 of 10*


---

# PRD 05: 4-Embedder Stack

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Design Philosophy

The embedder stack is designed for **consumer hardware**:

- **4 embedders** (not 13-15): Reduced from research system for practical use
- **384D max**: Smaller dimensions = less RAM, faster search
- **ONNX format**: CPU-optimized, cross-platform
- **Quantized (INT8)**: 50% smaller, nearly same quality
- **No LLM inference**: Removed causal/reasoning embedders that need GPUs
- **Tiered loading**: Free tier loads 2 models; Pro loads 3 (BM25 is algorithmic)

---

## 2. Embedder Specifications

### E1: Semantic Similarity (PRIMARY)

| Property | Value |
|----------|-------|
| Model | bge-small-en-v1.5 (BAAI) |
| Dimension | 384 |
| Size | 65MB (INT8 ONNX) |
| Speed | 50ms/chunk (M1), 100ms/chunk (Intel i5) |
| Tier | FREE |
| Purpose | Core semantic search |

**What it finds**: "quarterly revenue decline" matches "Q3 earnings drop"
**Role in pipeline**: Foundation embedder. All search queries start here. Stage 2 ranking.

### E6: Keyword Expansion (SPLADE)

| Property | Value |
|----------|-------|
| Model | SPLADE-cocondenser-selfdistil (Naver) |
| Dimension | Sparse (30K vocabulary) |
| Size | 55MB (INT8 ONNX) |
| Speed | 30ms/chunk |
| Tier | FREE |
| Purpose | Exact term matching + expansion |

**What it finds**: "Q3 earnings" also matches "third quarter revenue", "Q3 financial results"
**Role in pipeline**: Stage 2 ranking alongside E1. Catches exact terminology E1 misses.

### E12: Precision Reranking (ColBERT)

| Property | Value |
|----------|-------|
| Model | ColBERT-v2-small |
| Dimension | 64 per token |
| Size | 110MB (INT8 ONNX) |
| Speed | 100ms for top 50 candidates |
| Tier | PRO |
| Purpose | Final reranking for exact phrase matches |

**What it finds**: "revenue increased significantly" ranks higher than "revenue did not increase"
**Role in pipeline**: Stage 3 (final rerank). Token-level MaxSim scoring. Only runs on top 50 candidates.

### E13: Fast Recall (BM25)

| Property | Value |
|----------|-------|
| Model | None (algorithmic -- BM25/TF-IDF) |
| Dimension | N/A (inverted index) |
| Size | ~2MB index per 1000 documents |
| Speed | <5ms for any query |
| Tier | FREE |
| Purpose | Fast initial candidate retrieval |

**What it finds**: Exact keyword matches for terms like "invoice", "contract", "deadline"
**Role in pipeline**: Stage 1. Retrieves initial 500 candidates from inverted index.

---

## 3. Footprint Summary

| Metric | Free Tier | Pro Tier |
|--------|-----------|----------|
| Models to download | 2 (E1, E6) | 3 (+ E12) |
| Model disk space | ~120MB | ~230MB |
| RAM at runtime | ~600MB | ~1.0GB |
| Per-chunk embed time | ~80ms | ~180ms |
| Search latency | <100ms | <200ms |

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

/// Consumer-optimized embedding engine
pub struct EmbeddingEngine {
    env: Arc<Environment>,
    models: HashMap<EmbedderId, Option<Session>>,
    tier: LicenseTier,
    model_dir: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EmbedderId {
    E1,     // Semantic (FREE)
    E6,     // Keywords (FREE)
    E12,    // ColBERT (PRO)
    // E13 is BM25, not a neural model
}

impl EmbedderId {
    pub fn model_dir_name(&self) -> &'static str {
        match self {
            Self::E1 => "bge-small-en-v1.5",
            Self::E6 => "splade-distil",
            Self::E12 => "colbert-small",
        }
    }

    pub fn dimension(&self) -> usize {
        match self {
            Self::E1 => 384,
            Self::E6 => 0,    // Sparse
            Self::E12 => 64,  // Per token
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

        // Load models based on tier
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
                .with_intra_threads(2)?  // Limit threads for consumer hardware
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
                EmbedderId::E1,
                EmbedderId::E6,
            ],
            _ => vec![
                EmbedderId::E1,
                EmbedderId::E6,
                EmbedderId::E12,
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

    /// Embed a query (same models, but may use query-specific prefixes)
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
    pub e1: Option<Vec<f32>>,           // 384D
    pub e6: Option<SparseVec>,          // Sparse
    pub e12: Option<TokenEmbeddings>,   // 64D per token
}

pub enum QueryEmbedding {
    Dense(Vec<f32>),
    Sparse(SparseVec),
    Token(TokenEmbeddings),
}
```

---

## 6. Model Management

### 6.1 Lazy Loading

Models not needed for the current operation are not loaded:

```rust
/// Load a model on demand if not already loaded
pub fn ensure_model_loaded(&mut self, id: EmbedderId) -> Result<&Session> {
    if let Some(Some(session)) = self.models.get(&id) {
        return Ok(session);
    }

    // Check if model files exist
    let model_path = self.model_dir
        .join(id.model_dir_name())
        .join("model.onnx");

    if !model_path.exists() {
        return Err(CaseTrackError::ModelNotDownloaded(id));
    }

    // Load model
    tracing::info!("Lazy-loading model {:?}", id);
    self.load_model(id)?;

    self.models.get(&id)
        .and_then(|opt| opt.as_ref())
        .ok_or(CaseTrackError::ModelLoadFailed(id))
}
```

### 6.2 Memory Pressure Handling

```rust
/// Unload least-recently-used models when memory is constrained
pub fn handle_memory_pressure(&mut self) {
    let available_mb = sysinfo::System::new_all()
        .available_memory() / (1024 * 1024);

    if available_mb < 1024 {  // Less than 1GB free
        tracing::warn!("Low memory ({} MB free). Unloading Pro models.", available_mb);

        // Unload Pro-tier models (keep Free tier loaded)
        for id in &[EmbedderId::E12] {
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
# Example: Convert bge-small-en-v1.5 to ONNX
import torch
from transformers import AutoModel, AutoTokenizer

model = AutoModel.from_pretrained("BAAI/bge-small-en-v1.5")
tokenizer = AutoTokenizer.from_pretrained("BAAI/bge-small-en-v1.5")

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

# Quantize to INT8
from onnxruntime.quantization import quantize_dynamic, QuantType

quantize_dynamic(
    "model.onnx",
    "model_int8.onnx",
    weight_type=QuantType.QInt8,
)
```

A `scripts/convert_models.py` script should be included in the repository to automate this for all 3 neural models. Pre-converted ONNX models should be hosted on Hugging Face under a `casetrack/` organization.

---

*CaseTrack PRD v4.0.0 -- Document 5 of 10*


---

# PRD 06: Document Ingestion

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Supported Formats

| Format | Method | Quality | Rust Crate | Notes |
|--------|--------|---------|------------|-------|
| PDF (native text) | pdf-extract | Excellent | `pdf-extract`, `lopdf` | Direct text extraction |
| PDF (scanned) | Tesseract OCR | Good (>95%) | `tesseract` | Requires image rendering |
| DOCX | docx-rs | Excellent | `docx-rs` | Preserves structure |
| DOC (legacy) | Convert via LibreOffice | Good | CLI shelling | Optional, warns user |
| XLSX/XLS/ODS | calamine | Excellent | `calamine` | Pure Rust, reads all spreadsheet formats |
| Images (JPG/PNG/TIFF) | Tesseract OCR | Good | `tesseract`, `image` | Single page per image |
| TXT/RTF | Direct read | Excellent | `std::fs` | Plain text, no metadata |

---

## 2. Ingestion Pipeline

```
DOCUMENT INGESTION FLOW
=================================================================================

User: "Ingest ~/Downloads/Report.pdf"
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
|    - Extract document metadata (title, author, dates)                 |
|    Output: ParsedDocument { pages: Vec<Page>, metadata }              |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 3. CHUNK (provenance is attached here -- THE MOST CRITICAL STEP)       |
|    - Split into 2000-character chunks                                  |
|    - 10% overlap (200 chars from end of previous chunk)                |
|    - Respect paragraph and sentence boundaries                         |
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
|      * extraction_method: Native / OCR / Hybrid / Spreadsheet          |
|      * ocr_confidence: quality score for OCR-extracted text             |
|      * created_at: Unix timestamp of chunk creation                     |
|      * embedded_at: Unix timestamp (set after Step 4)                   |
|    A chunk without provenance MUST NOT be stored. Period.              |
|    Output: Vec<Chunk> with Provenance                                  |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 4. EMBED                                                               |
|    - Run each chunk through active embedders (3-6 depending on tier) |
|    - Batch for efficiency (32 chunks at a time)                      |
|    - Build BM25 inverted index entries                                |
|    Output: Vec<ChunkWithEmbeddings>                                   |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 5. STORE (provenance chain is sealed here)                             |
|    - Write chunks + provenance to collection RocksDB (chunks CF)      |
|      Each chunk stored with its FULL provenance inline                |
|    - Write embedding vectors to embeddings CF, keyed by chunk_id      |
|      chunk_id is the bridge: embedding → chunk → provenance → file    |
|    - Update embedded_at timestamp on each chunk                       |
|    - Write provenance records to provenance CF (prov:{chunk_uuid})    |
|    - Update BM25 inverted index                                       |
|    - Update document metadata (ingested_at, updated_at timestamps)    |
|    - Update collection stats                                          |
|    - Optionally copy original file to collection/originals/           |
|    Output: IngestResult { pages, chunks, duration, timestamps }       |
+-----------------------------------------------------------------------+
                    |
                    v
Response: "Ingested Report.pdf: 45 pages, 234 chunks, 12s"
```

### 2.1 Post-Chunk Processing: Entity Extraction & Knowledge Graph

After Step 5 (STORE), the pipeline extracts entities and builds the **Context Graph** (see [PRD 04 Section 8](PRD_04_STORAGE_ARCHITECTURE.md)).

**Step 6 -- EXTRACT ENTITIES**: For each chunk, run regex extractors (dates, amounts, percentages) and NER (persons, organizations, locations). Deduplicate within the document. Store Entity and EntityMention records in `entities` CF, update `entity_index`. Output: `Vec<Entity>`, `Vec<EntityMention>`.

**Step 7 -- BUILD DOCUMENT GRAPH EDGES**: Find shared entities (SharedEntities edges) across documents and compute document-level E1 similarity (SemanticSimilar edges). Cross-document entity matching links the same entity appearing in different documents. Store in `doc_graph` and `chunk_graph` CFs. Output: `Vec<DocRelationship>`.

**Step 8 -- UPDATE COLLECTION MAP**: Incrementally add new entities, key dates, entity statistics. Recompute CollectionStatistics. Output: Updated CollectionMap.

Complete response: `"Ingested Report.pdf: 45 pages, 234 chunks, 47 entities, 12s"`

#### Entity Types Extracted

| Type | Detection Method | Examples |
|------|-----------------|----------|
| Person | NER | "John Smith", "Sarah Chen" |
| Organization | NER | "Acme Corp", "Finance Department" |
| Date | Regex + NER | "January 15, 2024", "Q3 2024", "filed on 01/15/2024" |
| Amount | Regex | "$1,250,000.00", "1.25 million dollars", "15.7%" |
| Location | NER | "New York, NY", "123 Main Street", "United Kingdom" |
| Concept | NER | "supply chain optimization", "revenue forecast" |

#### Knowledge Graph Integration During Ingestion

After entity extraction, the pipeline builds knowledge graph connections:

| Step | Description |
|------|-------------|
| Entity-to-Chunk edges | Each extracted entity links to the chunk(s) where it appears |
| Cross-document entity matching | Same entity (e.g., "Acme Corp") across multiple documents creates shared-entity edges |
| Document relationship edges | Documents sharing 3+ entities are linked with SharedEntities relationship |
| Entity co-occurrence | Entities appearing in the same chunk are linked with co-occurrence edges |

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

## 7. Chunking Strategy

### 7.1 Chunking Rules (MANDATORY)

| Parameter | Value |
|-----------|-------|
| Target size | 2000 characters |
| Overlap | 10% = 200 characters (from end of previous chunk) |
| Min size | 400 characters (no tiny fragments) |
| Max size | 2200 characters (small overrun to avoid mid-sentence splits) |

Character-based (not token-based) for deterministic, reproducible chunking.

**Boundary priority**: (1) paragraph break, (2) sentence boundary, (3) word boundary. Never split mid-word. Chunks do NOT cross page boundaries.

### 7.2 Provenance Per Chunk (MANDATORY)

**Every chunk MUST store its complete provenance at creation time.** Fields: `document_id`, `document_name`, `document_path`, `page`, `paragraph_start/end`, `line_start/end`, `char_start/end`, `extraction_method`, `ocr_confidence`, `sheet_name` (spreadsheets), `row_range` (spreadsheets), `column_range` (spreadsheets), `chunk_index`.

Provenance is: (1) stored in RocksDB with chunk text and embeddings, (2) returned in every search result, (3) queryable via MCP tools, (4) immutable after creation. See [PRD 04 Section 5.2](PRD_04_STORAGE_ARCHITECTURE.md) for the canonical Provenance struct and storage layout.

### 7.3 Chunking Implementation

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

## 8. Batch Ingestion (Pro Tier)

```rust
/// Ingest all supported files in a directory
pub async fn ingest_folder(
    collection: &mut CollectionHandle,
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

        match ingest_single_file(collection, engine, file).await {
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

## 9. Duplicate Detection

Check SHA256 hash against existing documents before ingesting. If duplicate found, return error with existing document ID and `--force` hint.

```rust
pub fn check_duplicate(collection: &CollectionHandle, file_hash: &str) -> Result<Option<Uuid>> {
    let cf = collection.db.cf_handle("documents").unwrap();
    for item in collection.db.iterator_cf(&cf, rocksdb::IteratorMode::Start) {
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

## 10. Data Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub id: Uuid,
    pub filename: String,
    pub pages: Vec<Page>,
    pub metadata: DocumentMetadataRaw,
    pub file_hash: String,
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
    Skipped,      // OCR disabled, scanned page skipped
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
    pub extraction_method: ExtractionMethod,
    pub ocr_pages: u32,
    pub duration_ms: u64,
}
```

---

*CaseTrack PRD v4.0.0 -- Document 6 of 10*


---

# PRD 07: Collection Management & Provenance

**Version**: 5.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Collection Model

```rust
/// A document collection containing related files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub collection_type: CollectionType,
    pub status: CollectionStatus,
    pub tags: Vec<String>,
    pub created_by: Option<String>,
    pub created_at: i64,     // Unix timestamp
    pub updated_at: i64,     // Unix timestamp
    pub stats: CollectionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionStats {
    pub document_count: u32,
    pub page_count: u32,
    pub chunk_count: u32,
    pub storage_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CollectionType {
    Business,
    Research,
    Project,
    Archive,
    Compliance,
    Financial,
    Technical,
    HR,
    Sales,
    Other,
}

// Derive FromStr via case-insensitive match on variant names. Default: Other.

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CollectionStatus {
    Active,
    Closed,
    Archived,
}
```

---

## 2. Collection Registry

Shared RocksDB instance indexing all collections. Key schema: `collection:{uuid}` -> bincode-serialized `Collection`.

```rust
pub struct CollectionRegistry {
    db: rocksdb::DB,        // registry.db in data_dir
    data_dir: PathBuf,
    active_collection: Option<Uuid>,
}

pub struct CreateCollectionParams {
    pub name: String,
    pub description: Option<String>,
    pub collection_type: Option<CollectionType>,
    pub tags: Option<Vec<String>>,
    pub created_by: Option<String>,
}

impl CollectionRegistry {
    /// Opens registry.db from data_dir
    pub fn open(data_dir: &Path) -> Result<Self>;

    /// Creates collection dir + originals subdir, initializes CollectionHandle DB,
    /// stores in registry, auto-switches active_collection to new collection
    pub fn create_collection(&mut self, params: CreateCollectionParams) -> Result<Collection>;

    /// Lookup by "collection:{id}" key. Error: CollectionNotFound
    pub fn get_collection(&self, collection_id: Uuid) -> Result<Collection>;

    /// Prefix scan "collection:", returns all collections sorted by updated_at DESC
    pub fn list_collections(&self) -> Result<Vec<Collection>>;

    /// Upsert collection metadata
    pub fn update_collection(&mut self, collection: &Collection) -> Result<()>;

    /// Deletes registry entry + entire collection directory. Clears active_collection if matched.
    pub fn delete_collection(&mut self, collection_id: Uuid) -> Result<()>;

    /// Validates collection exists, opens CollectionHandle, sets active_collection
    pub fn switch_collection(&mut self, collection_id: Uuid) -> Result<CollectionHandle>;

    pub fn active_collection_id(&self) -> Option<Uuid>;
    pub fn count_collections(&self) -> Result<u32>;
}
```

---

## 3. Collection Handle

Each collection has its own `collection.db` RocksDB with column families defined in `super::COLUMN_FAMILIES`.

Key schemas:
- Documents CF: `doc:{uuid}` -> bincode `DocumentMetadata`
- Chunks CF: `chunk:{uuid}` -> bincode `Chunk`
- Chunks CF index: `doc_chunks:{doc_uuid}:{sequence:06}` -> chunk UUID string

```rust
/// Handle to an open collection database
pub struct CollectionHandle {
    pub db: rocksdb::DB,
    pub collection_id: Uuid,       // Parsed from collection_dir directory name
    pub collection_dir: PathBuf,
}

impl CollectionHandle {
    /// Create collection.db with all column families (DB dropped after init, reopened by open())
    pub fn initialize(collection_dir: &Path) -> Result<()>;

    /// Open existing collection.db. Error: CollectionDbOpenFailed
    pub fn open(collection_dir: &Path) -> Result<Self>;

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
  3. The EXTRACTION METHOD (Native text, OCR, Hybrid)
  4. TIMESTAMPS (when created, when last embedded)

This applies to:
  - Every text chunk
  - Every embedding vector (linked via chunk_id)
  - Every entity mention (stores chunk_id + char offsets)
  - Every reference record (stores chunk_id + document_id)
  - Every search result (includes full provenance)
  - Every MCP tool response that returns text

If the provenance chain is broken, the data is USELESS.
A search result without a source reference is worthless to a professional.
```

Every chunk tracks exactly where it came from:

```rust
/// EVERY chunk stores full provenance. This is THE MOST IMPORTANT DATA STRUCTURE
/// in CaseTrack. When the AI returns information, the user must know EXACTLY where
/// it came from -- which document, which file on disk, which page, which paragraph,
/// which line, which character range. Without provenance, the data is useless.
///
/// The Provenance chain: Embedding vector -> chunk_id -> ChunkData.provenance -> source file
/// This chain is NEVER broken. Every embedding, every entity mention, every reference,
/// every search result carries its Provenance. If you can't cite the source, you can't
/// return the information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    // === Source Document (WHERE did this come from?) ===
    /// UUID of the ingested document
    pub document_id: Uuid,
    /// Original filename ("Report.pdf") -- always stored, never empty
    pub document_name: String,
    /// Full filesystem path where the file was when ingested
    /// ("/Users/sarah/Projects/Alpha/Report.pdf")
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
    /// Generate a source reference string
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
impl CollectionHandle {
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

## 5. Collection Summary

Each collection maintains a summary structure that provides an at-a-glance overview of the collection's contents, automatically updated as documents are ingested or removed.

```rust
/// Per-collection summary providing an overview of all contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSummary {
    pub collection_id: Uuid,

    // === Key Entities ===
    /// People, organizations, and other named entities mentioned across all documents
    pub entities: Vec<EntitySummary>,

    // === Key Dates & Timelines ===
    /// Important dates extracted from documents with context
    pub key_dates: Vec<DateEntry>,

    // === Top Topics/Themes ===
    /// Dominant themes identified across the collection
    pub top_topics: Vec<TopicSummary>,

    // === Document Statistics ===
    pub document_count: u32,
    pub total_pages: u32,
    pub total_chunks: u32,
    pub storage_bytes: u64,
    pub file_types: HashMap<String, u32>,  // e.g., {"pdf": 12, "docx": 5, "xlsx": 3}

    // === Entity Statistics ===
    pub unique_entity_count: u32,
    pub entity_type_counts: HashMap<String, u32>,  // e.g., {"person": 45, "org": 12}

    pub last_updated: i64,  // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySummary {
    pub name: String,
    pub entity_type: String,        // "person", "organization", "location", etc.
    pub mention_count: u32,
    pub document_ids: Vec<Uuid>,    // Which documents mention this entity
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateEntry {
    pub date: String,               // ISO 8601
    pub context: String,            // What the date refers to
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

The reference network is a graph of cross-document references within a collection. It enables navigation between related documents based on shared entities, semantic similarity, and explicit references.

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
    SharedEntity,           // Both documents mention the same entity
    SemanticSimilarity,     // Documents contain semantically similar chunks
    ExplicitReference,      // One document explicitly references another
}
```

---

## 7. Knowledge Graph

Every collection maintains a knowledge graph linking chunks, documents, and entities with full provenance.

```
KNOWLEDGE GRAPH STRUCTURE
=================================================================================

  Nodes:
    - Document nodes (one per ingested file)
    - Chunk nodes (one per text chunk, linked to parent document)
    - Entity nodes (people, organizations, dates, etc. extracted from chunks)

  Edges:
    - Chunk-to-Document: Every chunk linked to its source document with full provenance
    - Entity-to-Chunk: Entity mention links with character offsets
    - Document-to-Document: Shared entities, semantic similarity, explicit references
    - Chunk-to-Chunk: Semantic similarity above threshold, co-reference

  Enables queries like:
    - "Show me all documents mentioning Company X"
    - "What other documents relate to this one?"
    - "Which entities appear across multiple documents?"
    - "Trace the provenance of this information back to the source"
```

```rust
/// Node in the collection's knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphNode {
    Document { id: Uuid, name: String },
    Chunk { id: Uuid, document_id: Uuid, text_preview: String },
    Entity { id: Uuid, name: String, entity_type: String },
}

/// Edge in the collection's knowledge graph
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
    DocumentToDocument,     // Cross-document reference
    ChunkToChunk,           // Semantic similarity or co-reference
}
```

---

## 8. Collection Lifecycle

```
COLLECTION LIFECYCLE
=================================================================================

  create_collection("Project Alpha")
       |
       v
  [ACTIVE] -----> ingest_pdf, ingest_docx, ingest_xlsx, search_collection
       |
       |  close_collection()          reopen_collection()
       v                                   |
  [CLOSED] --------> (read-only) ---------+
       |
       |  archive_collection()
       v
  [ARCHIVED] -----> (read-only, not shown in default list)
       |
       |  delete_collection()
       v
  [DELETED] -----> collection directory removed from disk

Notes:
  - ACTIVE: Full read/write. Can ingest, search, modify.
  - CLOSED: Read-only. Search works. Cannot ingest new documents.
  - ARCHIVED: Same as closed but hidden from default list_collections.
  - DELETED: Completely removed. Not recoverable.
```

---

## 9. Collection Management via MCP Tools -- Operations Guide

This section is the definitive reference for how the AI (Claude) and the user manage collections, documents, embeddings, and databases through MCP tools. **Every operation below is exposed as an MCP tool** (see PRD 09 for full input/output schemas).

### 9.1 Isolation Guarantee

```
CRITICAL: DATA NEVER CROSSES COLLECTION BOUNDARIES
=================================================================================

- Each collection = its own RocksDB database on disk (separate files, separate directory)
- Embeddings from Collection A are in a DIFFERENT DATABASE FILE than Collection B
- Search operates within a SINGLE COLLECTION ONLY -- there is no cross-collection search
- Ingestion targets the ACTIVE COLLECTION ONLY -- documents go into exactly one collection
- Deleting a collection deletes ONLY that collection's database, chunks, embeddings, and index
- No shared vector index, no shared embedding store, no shared anything

The AI MUST switch_collection before performing ANY operation on a different collection.
There is no way to accidentally mix data between collections.
```

### 9.2 Collection Lifecycle Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Create a collection | `create_collection` | Creates a new collection directory, initializes an empty RocksDB instance with all column families, registers in the collection registry, auto-switches to the new collection | New database on disk |
| List all collections | `list_collections` | Lists all collections with status, document count, chunk count, creation date | Read-only |
| Switch active collection | `switch_collection` | Changes which collection all subsequent operations target. Opens that collection's RocksDB database. | Changes active DB handle |
| Get collection details | `get_collection_info` | Shows all documents, total pages, total chunks, storage usage, embedder info | Read-only |
| Delete a collection | `delete_collection` | **Permanently removes**: collection directory, RocksDB database, ALL chunks, ALL embeddings, ALL indexes, ALL provenance records, optionally stored original files. Requires `confirm=true`. Not recoverable. | **Destroys entire database** |

### 9.3 Document Management Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Ingest one file | `ingest_document` | Reads file -> extracts text -> chunks into 2000-char segments -> embeds with all active models -> stores in active collection's DB | Adds chunks + embeddings to active collection |
| Ingest a folder | `ingest_folder` | Recursively walks directory -> ingests all supported files (PDF, DOCX, XLSX, TXT, etc.) -> skips already-ingested (SHA256) | Bulk add to active collection |
| Sync a folder | `sync_folder` | Compares disk vs DB -> ingests new files, reindexes changed files, optionally removes deleted | Add/update/remove in active collection |
| List documents | `list_documents` | Lists all documents in active collection with page count, chunk count, type | Read-only |
| Get document details | `get_document` | Shows one document's metadata, extraction method, chunk stats | Read-only |
| **Delete a document** | `delete_document` | **Removes from active collection**: document metadata, ALL chunks for that document, ALL embeddings for those chunks, ALL provenance records, ALL BM25 index entries. Requires `confirm=true`. | **Destroys document data** |

### 9.4 Embedding & Index Management Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Check index health | `get_index_status` | Per-document report: embedder coverage (2/4 vs 4/4), SHA256 staleness, missing source files | Read-only |
| Reindex one document | `reindex_document` | Deletes ALL old chunks + embeddings -> re-reads source file -> re-chunks -> re-embeds -> rebuilds BM25 entries. Option: `reparse=false` keeps chunks, only rebuilds embeddings. | **Replaces** old embeddings with fresh ones |
| Reindex entire collection | `reindex_collection` | Full rebuild of every document in the collection. Option: `skip_unchanged=true` only touches stale documents. Requires `confirm=true`. | **Replaces** all embeddings in collection |
| Get chunk provenance | `get_chunk` | Retrieves one chunk with full text and provenance (file, page, paragraph, line, char offsets) | Read-only |
| List document chunks | `get_document_chunks` | Lists all chunks in a document with their provenance | Read-only |
| Get surrounding context | `get_source_context` | Gets the chunks before/after a given chunk for context | Read-only |

### 9.5 Folder Watch & Auto-Sync Operations (MCP Tools)

| Operation | MCP Tool | What It Does | Data Impact |
|-----------|----------|-------------|-------------|
| Watch a folder | `watch_folder` | Starts OS-level file monitoring. New/modified/deleted files automatically trigger ingestion/reindex/removal in the target collection. | Automatic ongoing changes |
| Stop watching | `unwatch_folder` | Stops auto-sync. Existing collection data is untouched. | No data change |
| List watches | `list_watches` | Shows all active watches, their schedule, last sync, health status | Read-only |
| Change schedule | `set_sync_schedule` | Changes how often a watch syncs (on_change, hourly, daily, manual) | No data change |

### 9.6 Typical AI Workflow

```
User: "New collection for Project Alpha. Docs in ~/Projects/Alpha/"

Claude:
  1. create_collection("Project Alpha", collection_type="project")  -> isolated DB, auto-switched
  2. ingest_folder("~/Projects/Alpha/", recursive=true)             -> chunks + embeds all files
  3. watch_folder("~/Projects/Alpha/", schedule="on_change")        -> auto-sync future changes

User: "Search for customer retention strategy"
  4. search_collection("customer retention strategy", top_k=5)      -> results with full provenance

User: "Switch to Q3 Reports collection and search revenue figures"
  5. switch_collection("Q3 Reports")                                -> separate DB, Alpha inaccessible
  6. search_collection("revenue figures")                           -> Q3-only results

Key invariant: delete_collection/delete_document/reindex always cascade through
chunks -> embeddings -> provenance -> BM25 entries. Original source files on
disk are NEVER removed. See PRD 09 for full tool schemas.
```

---

*CaseTrack PRD v5.0.0 -- Document 7 of 10*


---

# PRD 08: Search & Retrieval

**Version**: 5.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. 3-Stage Search Pipeline

```
+-----------------------------------------------------------------------+
|                        3-STAGE SEARCH PIPELINE                         |
+-----------------------------------------------------------------------+
|                                                                       |
|  Query: "What does the report say about customer retention?"          |
|                                                                       |
|  +---------------------------------------------------------------+   |
|  | STAGE 1: BM25 RECALL                                  [<5ms]   |   |
|  |                                                                |   |
|  | - E13 inverted index lookup                                   |   |
|  | - Terms: "report", "customer", "retention"                    |   |
|  | - Fast lexical matching                                       |   |
|  |                                                                |   |
|  | Output: 500 candidate chunks                                  |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 2: SEMANTIC RANKING                             [<80ms]  |   |
|  |                                                                |   |
|  | - E1: Semantic similarity (384D dense cosine)                 |   |
|  | - E6: Keyword expansion (sparse dot product)                  |   |
|  | - Score fusion via Reciprocal Rank Fusion (RRF)               |   |
|  |                                                                |   |
|  | Output: 100 candidates, ranked                                |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 3: COLBERT RERANK (PRO TIER ONLY)              [<100ms] |   |
|  |                                                                |   |
|  | - E12: Token-level MaxSim scoring                             |   |
|  | - Ensures exact phrase matches rank highest                   |   |
|  | - "customer retention" > "retention of the customer"          |   |
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
    tier: LicenseTier,
}

impl SearchEngine {
    pub fn search(
        &self,
        collection: &CollectionHandle,
        query: &str,
        top_k: usize,
        document_filter: Option<Uuid>,
    ) -> Result<Vec<SearchResult>> {
        let start = std::time::Instant::now();

        // Stage 1: BM25 recall
        let bm25_candidates = self.bm25_recall(collection, query, 500, document_filter)?;

        if bm25_candidates.is_empty() {
            return Ok(vec![]);
        }

        // Stage 2: Semantic ranking
        let query_e1 = self.embedder.embed_query(query, EmbedderId::E1)?;
        let query_e6 = self.embedder.embed_query(query, EmbedderId::E6)?;

        let mut scored: Vec<(Uuid, f32)> = bm25_candidates
            .iter()
            .map(|chunk_id| {
                let e1_score = self.score_dense(collection, "e1", chunk_id, &query_e1)?;
                let e6_score = self.score_sparse(collection, "e6", chunk_id, &query_e6)?;

                let rrf = rrf_fusion(&[
                    (e1_score, 1.0),   // E1: weight 1.0
                    (e6_score, 0.8),   // E6: weight 0.8
                ]);

                Ok((*chunk_id, rrf))
            })
            .collect::<Result<Vec<_>>>()?;

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(100);

        // Stage 3: ColBERT rerank (Pro only)
        if self.tier.is_pro() {
            scored = self.colbert_rerank(collection, query, scored)?;
        }

        // Build results with provenance
        let results: Vec<SearchResult> = scored
            .into_iter()
            .take(top_k)
            .map(|(chunk_id, score)| self.build_result(collection, chunk_id, score))
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
        collection: &CollectionHandle,
        chunk_id: Uuid,
        score: f32,
    ) -> Result<SearchResult> {
        let chunk = collection.get_chunk(chunk_id)?;
        let (ctx_before, ctx_after) = collection.get_surrounding_context(&chunk, 1)?;

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

## 3. BM25 Implementation

Standard BM25 with `k1=1.2, b=0.75`. Stored in `bm25_index` column family.

**Key schema**: `term:{token}` -> bincode `PostingList`, `stats` -> bincode `Bm25Stats`

**Tokenization**: lowercase, split on non-alphanumeric (preserving apostrophes), filter stopwords and single-char tokens.

```rust
pub struct Bm25Index;

impl Bm25Index {
    /// Tokenize query -> lookup postings per term -> accumulate BM25 scores
    /// per chunk -> apply optional document_filter -> return top `limit` chunk IDs
    pub fn search(collection: &CollectionHandle, query: &str, limit: usize,
                  document_filter: Option<Uuid>) -> Result<Vec<Uuid>>;

    /// Tokenize chunk text -> upsert PostingList per term -> update Bm25Stats
    pub fn index_chunk(collection: &CollectionHandle, chunk: &Chunk) -> Result<()>;
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

## 4. Reciprocal Rank Fusion (RRF)

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

/// RRF constant. Higher K smooths out rank differences.
const RRF_K: f32 = 60.0;
}
```

---

## 5. ColBERT Reranking (Stage 3)

```rust
impl SearchEngine {
    fn colbert_rerank(
        &self,
        collection: &CollectionHandle,
        query: &str,
        candidates: Vec<(Uuid, f32)>,
    ) -> Result<Vec<(Uuid, f32)>> {
        // Embed query at token level
        let query_tokens = self.embedder.embed_query(query, EmbedderId::E12)?;
        let query_vecs = match query_tokens {
            QueryEmbedding::Token(t) => t,
            _ => unreachable!(),
        };

        let mut reranked: Vec<(Uuid, f32)> = candidates
            .into_iter()
            .map(|(chunk_id, base_score)| {
                // Load chunk's token embeddings
                let chunk_tokens = self.load_token_embeddings(collection, &chunk_id)?;

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
    pub vectors: Vec<Vec<f32>>,  // One 64D vector per token
    pub token_count: usize,
}
```

---

## 6. Knowledge Graph Integration in Search

After vector search returns chunks, results can optionally be expanded via the collection's knowledge graph to surface related content the user did not directly query.

```
KNOWLEDGE GRAPH EXPANSION (POST-RETRIEVAL)
=================================================================================

  1. Vector search returns top K chunks (from Stages 1-3)
  2. For each result chunk:
     a. Look up entities mentioned in that chunk
     b. Find other chunks/documents sharing those entities -> "Related documents"
     c. Traverse chunk-to-chunk edges (semantic similarity, co-reference) -> "Related chunks"
  3. Deduplicate and rank expanded results by graph edge weight
  4. Return expanded results alongside primary results

  Enables:
    - "Related documents" via entity overlap
    - "Related chunks" via graph edges
    - Cross-document discovery without explicit search terms
```

```rust
impl SearchEngine {
    /// Expand search results via knowledge graph edges
    pub fn expand_via_graph(
        &self,
        collection: &CollectionHandle,
        results: &[SearchResult],
        max_expansions: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut expanded = Vec::new();

        for result in results {
            // Find entities in this chunk
            let entities = collection.get_chunk_entities(result.provenance.document_id)?;

            // Find other documents mentioning the same entities
            let related_docs = collection.find_documents_by_entities(&entities)?;

            // Find chunks connected via graph edges
            let related_chunks = collection.get_related_chunks(
                result.provenance.document_id,
                max_expansions,
            )?;

            for chunk in related_chunks {
                expanded.push(self.build_result(collection, chunk.id, chunk.edge_weight)?);
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

## 7. Search Response Format (Canonical)

This is the canonical MCP response format for `search_collection` (also referenced by PRD 09).
Document-scoped search uses the same pipeline via the `document_filter` parameter on `SearchEngine::search`.

Every search result includes full provenance: file path, document name, page, paragraph, line, and character offsets.

```json
{
  "query": "customer retention strategy",
  "collection": "Project Alpha",
  "results_count": 5,
  "search_time_ms": 87,
  "tier": "pro",
  "stages_used": ["bm25", "semantic", "colbert"],
  "results": [
    {
      "text": "The recommended customer retention strategy focuses on quarterly business reviews and proactive account management...",
      "score": 0.94,
      "citation": "Q3_Report.pdf, p. 12, para. 8",
      "citation_short": "Q3_Report, p. 12",
      "source": {
        "document": "Q3_Report.pdf",
        "document_path": "/Users/sarah/Projects/Alpha/originals/Q3_Report.pdf",
        "document_id": "abc-123",
        "chunk_id": "chunk-456",
        "chunk_index": 14,
        "page": 12,
        "paragraph_start": 8,
        "paragraph_end": 8,
        "line_start": 1,
        "line_end": 4,
        "char_start": 24580,
        "char_end": 26580,
        "extraction_method": "Native",
        "ocr_confidence": null,
        "chunk_created_at": "2026-01-15T14:30:00Z",
        "chunk_embedded_at": "2026-01-15T14:30:12Z",
        "document_ingested_at": "2026-01-15T14:29:48Z"
      },
      "context": {
        "before": "...the previous paragraph text...",
        "after": "...the next paragraph text..."
      }
    }
  ]
}
```

---

*CaseTrack PRD v5.0.0 -- Document 8 of 10*


---

# PRD 09: MCP Tools

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Tool Overview

| Tool | Description | Tier | Requires Active Collection |
|------|-------------|------|---------------------------|
| `create_collection` | Create a new document collection | Free | No |
| `list_collections` | List all collections | Free | No |
| `switch_collection` | Switch active collection | Free | No |
| `delete_collection` | Delete a collection and all its data | Free | No |
| `get_collection_info` | Get details about active collection | Free | Yes |
| `ingest_document` | Ingest a PDF, DOCX, XLSX, or image | Free | Yes |
| `ingest_folder` | Ingest all supported files in a folder and subfolders | Free | Yes |
| `sync_folder` | Sync a folder -- ingest new/changed files, optionally remove deleted | Free | Yes |
| `list_documents` | List documents in active collection | Free | Yes |
| `get_document` | Get document details and stats | Free | Yes |
| `delete_document` | Remove a document from a collection | Free | Yes |
| `search_documents` | Search across all documents | Free (limited) | Yes |
| `find_entity` | Find mentions of an entity across documents | Pro | Yes |
| `get_chunk` | Get a specific chunk with full provenance | Free | Yes |
| `get_document_chunks` | List all chunks in a document with provenance | Free | Yes |
| `get_source_context` | Get surrounding text for a chunk (context window) | Free | Yes |
| `reindex_document` | Delete old embeddings/indexes for a document and rebuild from scratch | Free | Yes |
| `reindex_collection` | Rebuild all embeddings and indexes for the entire active collection | Free | Yes |
| `get_index_status` | Show embedding/index health for all documents in active collection | Free | Yes |
| `watch_folder` | Start watching a folder for file changes -- auto-sync on change or schedule | Free | Yes |
| `unwatch_folder` | Stop watching a folder | Free | Yes |
| `list_watches` | List all active folder watches and their sync status | Free | No |
| `set_sync_schedule` | Set the auto-sync schedule (on_change, hourly, daily, manual) | Free | Yes |
| `get_status` | Get server status and model info | Free | No |
| | | | |
| **--- Context Graph: Collection Overview ---** | | | |
| `get_collection_summary` | High-level collection briefing: key stakeholders, key dates, topics, document categories, top entities, key references, statistics | Free | Yes |
| `get_collection_timeline` | Chronological view of key dates and events extracted from documents | Free | Yes |
| `get_collection_statistics` | Document counts, page counts, chunk counts, entity counts, reference counts, embedder coverage | Free | Yes |
| | | | |
| **--- Context Graph: Entity & Reference Search ---** | | | |
| `list_entities` | List all extracted entities in the collection, grouped by type (person, org, date, amount, etc.) | Free | Yes |
| `get_entity_mentions` | Get all chunks mentioning a specific entity, with context snippets | Free | Yes |
| `search_entity_relationships` | Find chunks mentioning two or more entities together | Pro | Yes |
| `get_entity_graph` | Show entity relationships across documents in the collection | Pro | Yes |
| `list_references` | List all referenced external sources (documents, standards, regulations) with reference counts | Free | Yes |
| `get_reference_citations` | Get all chunks citing a specific reference, with context | Free | Yes |
| | | | |
| **--- Context Graph: Document Navigation ---** | | | |
| `get_document_structure` | Get headings, sections, and table of contents for a document | Free | Yes |
| `browse_pages` | Get all chunks from a specific page range within a document | Free | Yes |
| `find_related_documents` | Find documents similar to a given document (by shared entities, references, or semantic similarity) | Free | Yes |
| `get_related_documents` | Given a document, find related docs via knowledge graph (shared entities, references) | Free | Yes |
| `list_documents_by_type` | List documents filtered by type (contract, report, spreadsheet, etc.) | Free | Yes |
| `traverse_chunks` | Navigate forward/backward through chunks in a document from a starting point | Free | Yes |
| | | | |
| **--- Context Graph: Advanced Search ---** | | | |
| `search_similar_chunks` | Find chunks semantically similar to a given chunk across all documents | Free | Yes |
| `compare_documents` | Compare what two documents say about a topic (side-by-side search) | Pro | Yes |
| `find_document_clusters` | Group documents by theme/topic using semantic clustering | Pro | Yes |

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
// NoCollectionActive -- returned by any tool that requires an active collection
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "No active collection. Create or switch to a collection first:\n  - create_collection: Create a new collection\n  - switch_collection: Switch to an existing collection\n  - list_collections: See all collections"
  }]
}

// FileNotFound -- returned by ingest_document, reindex_document, etc.
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "File not found: /Users/sarah/Downloads/Contract.pdf\n\nCheck that the path is correct and the file exists."
  }]
}

// FreeTierLimit -- returned when a free tier quota is exceeded
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Free tier allows 3 collections (you have 3). Delete a collection or upgrade to Pro for unlimited collections: https://casetrack.dev/upgrade"
  }]
}

// NotFound -- returned when a collection, document, or chunk ID is not found
{
  "isError": true,
  "content": [{
    "type": "text",
    "text": "Collection not found: \"Acme\". Did you mean:\n  - Acme Corp Partnership (ID: a1b2c3d4)\nUse the full name or ID."
  }]
}
```

Per-tool error examples are omitted below; all errors follow these patterns.

---

### 2.1 `create_collection`

```json
{
  "name": "create_collection",
  "description": "Create a new document collection. Creates an isolated database for this collection on your machine. Automatically switches to the new collection.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "name": {
        "type": "string",
        "description": "Collection name (e.g., 'Project Alpha', 'Acme Corp Partnership')"
      },
      "collection_id": {
        "type": "string",
        "description": "Optional identifier or reference number"
      },
      "collection_type": {
        "type": "string",
        "enum": ["project", "contract", "financial", "compliance", "research", "hr", "operations", "other"],
        "description": "Type of document collection"
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
    "text": "Created collection \"Acme Corp Partnership\" (ID: a1b2c3d4).\nType: Contract\nThis is now your active collection.\n\nNext: Ingest documents with ingest_document."
  }]
}
```

---

### 2.2 `list_collections`

```json
{
  "name": "list_collections",
  "description": "List all collections. Shows name, type, status, document count, and which collection is active.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "status_filter": {
        "type": "string",
        "enum": ["active", "closed", "archived", "all"],
        "default": "active",
        "description": "Filter by collection status"
      }
    }
  }
}
```

---

### 2.3 `switch_collection`

```json
{
  "name": "switch_collection",
  "description": "Switch to a different collection. All subsequent operations (ingest, search) will use this collection.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "collection_name": {
        "type": "string",
        "description": "Collection name or ID to switch to"
      }
    },
    "required": ["collection_name"]
  }
}
```

---

### 2.4 `delete_collection`

```json
{
  "name": "delete_collection",
  "description": "Permanently delete a collection and all its documents, embeddings, and data. This cannot be undone.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "collection_name": {
        "type": "string",
        "description": "Collection name or ID to delete"
      },
      "confirm": {
        "type": "boolean",
        "description": "Must be true to confirm deletion",
        "default": false
      }
    },
    "required": ["collection_name", "confirm"]
  }
}
```

---

### 2.5 `get_collection_info`

```json
{
  "name": "get_collection_info",
  "description": "Get detailed information about the active collection including document list and storage usage.",
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
  "description": "Ingest a document (PDF, DOCX, XLSX, or image) into the active collection. Extracts text (with OCR for scans), chunks the text, computes embeddings, and indexes for search. All processing and storage happens locally on your machine.",
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
        "enum": ["contract", "report", "spreadsheet", "presentation", "correspondence", "memo", "proposal", "invoice", "policy", "other"],
        "description": "Type of document"
      },
      "copy_original": {
        "type": "boolean",
        "default": false,
        "description": "Copy the original file into the collection folder"
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
        "enum": ["contract", "report", "spreadsheet", "presentation", "correspondence", "memo", "proposal", "invoice", "policy", "other"],
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
    "text": "Folder ingestion complete for Acme Corp Partnership\n\n  Folder:     ~/Projects/Acme/Documents/\n  Subfolders: 4 (Contracts/, Reports/, Financials/, Correspondence/)\n  Found:      47 supported files\n  New:        23 (ingested)\n  Skipped:    22 (already ingested, matching SHA256)\n  Failed:     2\n  Duration:   4 minutes 12 seconds\n\n  New documents ingested:\n  - Contracts/Vendor_Agreement.docx (45 pages, 234 chunks)\n  - Contracts/Service_Contract.pdf (12 pages, 67 chunks)\n  - Reports/Q3_Report.xlsx (8 pages, 42 chunks)\n  ... 20 more\n\n  Failures:\n  - Financials/corrupted.pdf: PDF parsing error (file may be corrupted)\n  - Reports/scan_2019.tiff: OCR failed (image too low resolution)\n\nAll 23 new documents are now searchable."
  }]
}
```

---

### 2.8 `sync_folder`

```json
{
  "name": "sync_folder",
  "description": "Sync a folder with the active collection. Compares files on disk against what is already ingested and: (1) ingests new files not yet in the collection, (2) re-ingests files that have changed since last ingestion (detected by SHA256 mismatch), (3) optionally removes documents whose source files no longer exist on disk. This is the easiest way to keep a collection up to date with a directory of documents -- just point it at the folder and run it whenever files change.",
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
        "description": "If true, documents whose source files no longer exist on disk will be removed from the collection (chunks + embeddings deleted). Default: false (only add/update, never remove)."
      },
      "document_type": {
        "type": "string",
        "enum": ["contract", "report", "spreadsheet", "presentation", "correspondence", "memo", "proposal", "invoice", "policy", "other"],
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
  "description": "List all documents in the active collection.",
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
  "description": "Remove a document and all its chunks, embeddings, and index entries from the active collection.",
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

### 2.12 `search_documents`

```json
{
  "name": "search_documents",
  "description": "Search across all documents in the active collection using semantic and keyword search. Returns results with FULL provenance: source document filename, file path, page, paragraph, line numbers, character offsets, extraction method, timestamps. Every result is traceable to its exact source location.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Natural language search query (e.g., 'Q3 revenue analysis', 'vendor payment terms', 'project timeline')"
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
    "text": "Search: \"payment terms\"\nCollection: Acme Corp Partnership | 5 documents, 1,051 chunks searched\nTime: 87ms | Tier: Pro (3-stage pipeline)\n\n--- Result 1 (score: 0.94) ---\nVendor_Agreement.docx, p. 12, para. 8, ll. 1-4\n\n\"Payment shall be made within thirty (30) days of receipt of invoice. Late payments shall accrue interest at a rate of 1.5% per month on the outstanding balance.\"\n\n--- Result 2 (score: 0.89) ---\nVendor_Agreement.docx, p. 13, para. 10, ll. 1-6\n\n\"In the event of early termination, the service provider shall be entitled to recover all outstanding fees, including accrued interest and reasonable costs of transition.\"\n\n--- Result 3 (score: 0.76) ---\nQ3_Report.xlsx, p. 8, para. 22, ll. 3-5\n\n\"Vendor payments exceeded budget by 12% in Q3, primarily due to accelerated delivery schedules under the revised service level agreement.\""
  }]
}
```

---

### 2.13 `find_entity`

```json
{
  "name": "find_entity",
  "description": "Find all mentions of an entity (person, organization, date, amount) across documents. Uses the entity index built during ingestion.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity": {
        "type": "string",
        "description": "Entity to find (e.g., 'John Smith', 'Acme Corp', '$1.2 million')"
      },
      "entity_type": {
        "type": "string",
        "enum": ["person", "organization", "date", "amount", "location", "concept", "any"],
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

### 2.15 `reindex_collection`

```json
{
  "name": "reindex_collection",
  "description": "Rebuild all embeddings, chunks, and search indexes for every document in the active collection. This is a full rebuild -- it deletes ALL existing chunks and embeddings, re-reads every source file, re-chunks, re-embeds with all active models, and rebuilds the entire BM25 index. Use this when: (1) upgrading from Free to Pro tier (re-embed everything with 4 models instead of 3), (2) after a CaseTrack update that changes chunking or embedding logic, (3) the collection index seems corrupted or stale, (4) you want a clean rebuild. WARNING: This can be slow for large collections (hundreds of documents).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "confirm": {
        "type": "boolean",
        "default": false,
        "description": "Must be true to confirm. This deletes and rebuilds ALL embeddings in the collection."
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
  "description": "Show the embedding and index health status for all documents in the active collection. Reports which documents have complete embeddings for the current tier, which need reindexing (source file changed, missing embedder coverage, stale embeddings), and overall collection index health. Use this to diagnose issues or decide whether to run reindex_document or reindex_collection.",
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

### 2.18 `get_chunk`

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
    "text": "Chunk abc-123 (2000 chars)\n\nText:\n\"Payment shall be made within thirty (30) days of receipt of invoice. Late payments shall accrue interest...\"\n\nProvenance:\n  Document:   Vendor_Agreement.docx\n  File Path:  /Users/sarah/Projects/Acme/Vendor_Agreement.docx\n  Page:       12\n  Paragraphs: 8-9\n  Lines:      1-14\n  Chars:      2401-4401 (within page)\n  Extraction: Native text\n  Chunk Index: 47 of 234\n\nEmbeddings: E1, E6, E12"
  }]
}
```

---

### 2.19 `get_document_chunks`

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

### 2.20 `get_source_context`

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

### 2.21 `watch_folder`

```json
{
  "name": "watch_folder",
  "description": "Start watching a folder for file changes. When files are added, modified, or deleted in the watched folder (or any subfolder), CaseTrack automatically syncs the changes into the active collection -- new files are ingested, modified files are reindexed (old chunks/embeddings deleted, new ones created), and optionally deleted files are removed from the collection. Uses OS-level file notifications (inotify on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows) for instant detection. Also supports scheduled sync as a safety net (daily, hourly, or custom interval). Watch persists across server restarts.",
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
        "description": "If true, documents whose source files are deleted from disk will be automatically removed from the collection (chunks + embeddings deleted). Default: false (only add/update, never auto-remove)."
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

### 2.22 `unwatch_folder`

```json
{
  "name": "unwatch_folder",
  "description": "Stop watching a folder. Removes the watch but does NOT delete any documents already ingested from that folder. The collection data remains intact -- only the automatic sync is stopped.",
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

### 2.23 `list_watches`

```json
{
  "name": "list_watches",
  "description": "List all active folder watches across all collections. Shows the watched folder, which collection it syncs to, the schedule, last sync time, and current status.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "collection_filter": {
        "type": "string",
        "description": "Optional: only show watches for a specific collection name or ID"
      }
    }
  }
}
```

---

### 2.24 `set_sync_schedule`

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

The context graph tools give the AI structured navigation of the collection beyond flat search. They are built on the entity, reference, and document graph data extracted during ingestion (see PRD 04 Section 8).

### 2.25 `get_collection_summary`

```json
{
  "name": "get_collection_summary",
  "description": "Get a high-level briefing on the active collection. Returns: key stakeholders (people and organizations mentioned most), key dates and events, key topics, document breakdown by category, key references (most-referenced documents or external sources), most-mentioned entities, and collection statistics. This is the FIRST tool the AI should call when starting work on a collection -- it provides the structural overview needed to plan search strategy for 1000+ documents.",
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
    "text": "COLLECTION SUMMARY: Acme Corp Partnership (Contract)\n\n  KEY STAKEHOLDERS:\n    Client:     Acme Corp (CEO: John Smith)\n    Vendor:     Summit Services LLC (CEO: Mary Jones)\n    Analysts:   Sarah Chen (Acme), Michael Brown (Summit)\n\n  KEY DATES:\n    2022-01-15  Contract signed (Vendor_Agreement.docx, p.1)\n    2023-06-01  Service level review (Q3_Report.xlsx, p.5)\n    2023-07-01  Renewal proposal submitted (Proposal.pdf, p.1)\n    2023-09-15  Budget approved (Budget.xlsx, p.1)\n    2024-01-10  Q1 deliverables deadline (Status_Report.docx, p.2)\n    2024-06-15  Partnership review date (Meeting_Notes.pdf, p.3)\n\n  KEY TOPICS:\n    1. Service level agreement compliance -- 23 documents, 187 chunks\n    2. Payment terms and schedules -- 18 documents, 145 chunks\n    3. Vendor performance metrics -- 8 documents, 42 chunks\n    4. Cost optimization -- 5 documents, 28 chunks\n\n  DOCUMENTS (47 total, 2,341 pages, 12,450 chunks):\n    Contracts:       5 docs (Vendor Agreement, Service Contract, Amendments...)\n    Reports:        20 docs (Q3 Report, Performance Reviews, Audits...)\n    Financials:     15 docs (Budgets, Invoices, Cost Analyses...)\n    Correspondence:  7 docs (Meeting Notes, Status Updates, Memos...)\n\n  KEY REFERENCES (most cited):\n    1. Master Service Agreement v2.1 -- 47 references across 15 documents\n    2. SLA Framework 2023 -- 23 references across 8 documents\n    3. Industry Benchmark Report -- 12 references across 6 documents\n\n  TOP ENTITIES:\n    Acme Corp -- 892 mentions in 45 documents\n    Summit Services LLC -- 756 mentions in 42 documents\n    John Smith -- 234 mentions in 28 documents\n    Service level agreement -- 187 mentions in 23 documents\n\n  EMBEDDINGS: 4/4 embedders (Pro tier), all 12,450 chunks fully embedded"
  }]
}
```

---

### 2.26 `get_collection_timeline`

```json
{
  "name": "get_collection_timeline",
  "description": "Get a chronological timeline of key dates and events extracted from documents in the active collection. Each event includes the date, description, and source document/chunk provenance. Use this to understand the narrative sequence of events.",
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

### 2.27 `get_collection_statistics`

```json
{
  "name": "get_collection_statistics",
  "description": "Get detailed statistics about the active collection: document counts by type, page/chunk totals, entity and reference counts, embedder coverage, storage usage. Useful for understanding collection scope and data quality.",
  "inputSchema": {
    "type": "object",
    "properties": {}
  }
}
```

---

### 2.28 `list_entities`

```json
{
  "name": "list_entities",
  "description": "List all entities extracted from documents in the active collection, grouped by type. Shows name, type, mention count, and number of documents mentioning each entity. Entities include: persons, organizations, dates, monetary amounts, locations, and concepts.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_type": {
        "type": "string",
        "enum": ["person", "organization", "date", "amount", "location", "concept", "all"],
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

### 2.29 `get_entity_mentions`

```json
{
  "name": "get_entity_mentions",
  "description": "Get all chunks that mention a specific entity, with context snippets showing how the entity is referenced. Uses the entity index built during ingestion. Supports fuzzy matching on entity name.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_name": {
        "type": "string",
        "description": "Name of the entity to find (e.g., 'John Smith', 'Acme Corp', 'payment terms')"
      },
      "entity_type": {
        "type": "string",
        "enum": ["person", "organization", "date", "amount", "location", "concept", "any"],
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
    "text": "Mentions of \"John Smith\" (person) -- 234 total, showing top 20:\n\n  1. Proposal.pdf, p.2, para.3\n     \"John Smith, CEO of Acme Corp, presented the partnership proposal...\"\n\n  2. Meeting_Notes.pdf, p.15, para.8\n     \"Q: Mr. Smith, when did you first review the vendor performance report?\"\n     \"A: I received the summary from our VP on March 10, 2023...\"\n\n  3. Vendor_Agreement.docx, p.12, para.1 (signature block)\n     \"John Smith, Chief Executive Officer, Acme Corp\"\n\n  ... 17 more mentions"
  }]
}
```

---

### 2.30 `search_entity_relationships`

```json
{
  "name": "search_entity_relationships",
  "description": "Find chunks where two or more entities are mentioned together. Use this to trace relationships (who interacted with whom, what terms apply to which party). Pro tier only.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entities": {
        "type": "array",
        "items": { "type": "string" },
        "minItems": 2,
        "maxItems": 5,
        "description": "Entity names to find together (e.g., ['Acme Corp', 'Summit Services'])"
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

### 2.31 `get_entity_graph`

```json
{
  "name": "get_entity_graph",
  "description": "Show entity relationships across documents in the active collection. Returns a graph of entities connected by co-occurrence in documents and chunks. Use this to understand how people, organizations, and concepts relate to each other across the collection.",
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

### 2.32 `list_references`

```json
{
  "name": "list_references",
  "description": "List all referenced external sources (documents, standards, regulations, reports) cited in the active collection. Shows the reference, type, citation count, and number of citing documents. Use this to understand which external sources matter most in the collection.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "reference_type": {
        "type": "string",
        "enum": ["document", "standard", "regulation", "report", "all"],
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

### 2.33 `get_reference_citations`

```json
{
  "name": "get_reference_citations",
  "description": "Get all chunks that cite a specific reference. Shows the context of each citation. Use this to understand how a reference is used throughout the collection.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "reference": {
        "type": "string",
        "description": "The reference to look up (e.g., 'Master Service Agreement v2.1', 'ISO 27001')"
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

### 2.34 `get_document_structure`

```json
{
  "name": "get_document_structure",
  "description": "Get the structural outline of a document: headings, sections, numbered clauses, and their page/chunk locations. This gives the AI a table-of-contents view for navigation. Works best with structured documents (contracts, reports, policies).",
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

### 2.35 `browse_pages`

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

### 2.36 `find_related_documents`

```json
{
  "name": "find_related_documents",
  "description": "Find documents related to a given document. Relationships detected: shared entities, shared references, semantic similarity (E1 cosine), and version chains. Returns related documents ranked by relationship strength.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_name": {
        "type": "string",
        "description": "Document name or ID to find relationships for"
      },
      "relationship_type": {
        "type": "string",
        "enum": ["all", "shared_entities", "shared_references", "semantic_similar", "version_chain"],
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

### 2.37 `get_related_documents`

```json
{
  "name": "get_related_documents",
  "description": "Given a document, find related docs via the knowledge graph. Uses shared entities, references, and semantic similarity to surface connections. This is a knowledge-graph-first approach compared to find_related_documents which also supports explicit relationship types.",
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

### 2.38 `list_documents_by_type`

```json
{
  "name": "list_documents_by_type",
  "description": "List all documents in the active collection filtered by document type (contract, report, spreadsheet, etc.). Includes page count, chunk count, and ingestion date.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "document_type": {
        "type": "string",
        "enum": ["contract", "report", "spreadsheet", "presentation", "correspondence", "memo", "proposal", "invoice", "policy", "other"],
        "description": "Type to filter by"
      }
    },
    "required": ["document_type"]
  }
}
```

---

### 2.39 `traverse_chunks`

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

### 2.40 `search_similar_chunks`

```json
{
  "name": "search_similar_chunks",
  "description": "Find chunks across all documents that are semantically similar to a given chunk. Uses E1 cosine similarity. Use this to find related passages in other documents (e.g., 'find other places in the collection that discuss the same topic as this paragraph').",
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

### 2.41 `compare_documents`

```json
{
  "name": "compare_documents",
  "description": "Compare what two documents say about a specific topic. Searches both documents independently, then returns side-by-side results showing how each document addresses the topic. Pro tier only. Use this for: contract vs. proposal comparison, report A vs. report B, any 'what does X say vs. what does Y say' question.",
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
        "description": "Topic to compare (e.g., 'payment terms', 'delivery schedule', 'performance metrics')"
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

### 2.42 `find_document_clusters`

```json
{
  "name": "find_document_clusters",
  "description": "Group all documents in the collection by theme or topic using semantic clustering. Returns clusters of related documents with a label describing what they share. Pro tier only. Use this to understand the structure of a large collection (100+ documents) at a glance.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "strategy": {
        "type": "string",
        "enum": ["topical", "entity", "reference", "document_type"],
        "default": "topical",
        "description": "Clustering strategy: 'topical' = semantic similarity, 'entity' = shared people/orgs, 'reference' = shared external sources, 'document_type' = by type"
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
    collection_handle: Arc<CollectionHandle>,
}

enum FsEventKind { Created, Modified, Deleted }
```

Behavior: On startup, `WatchManager::init` restores saved watches from `watches.json`, starts OS watchers, and spawns two background tasks -- an event processor (with 2-second debounce) and a scheduled sync runner (checks every 60 seconds). Events are batched: Created triggers ingest, Modified triggers reindex, Deleted triggers removal (if `auto_remove_deleted` is enabled).

For full implementation details (server initialization, tool registration, error handling), see [PRD 10: Technical Build Guide](PRD_10_TECHNICAL_BUILD.md).

---

## 4. Active Collection State

The server maintains an "active collection" that all document and search operations target. The server starts with no active collection; `create_collection` automatically switches to the new collection, and `switch_collection` explicitly changes it. Tools requiring a collection return a `NoCollectionActive` error if none is set. The active collection persists for the MCP session duration but not across sessions.

---

*CaseTrack PRD v4.0.0 -- Document 9 of 10*


---

# PRD 10: Technical Build Guide

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

> **LANGUAGE: RUST** -- This entire project is built in Rust. Every crate, every
> module, every line of product code is Rust. The final deliverable is a single
> statically-linked Rust binary per platform. No runtime dependencies. The only
> non-Rust code is `scripts/convert_models.py` (one-time build tool, not shipped).

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
|-- Cargo.toml                   # Workspace root
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
|   +-- casetrack-core/          # Library crate (all business logic)
|       |-- Cargo.toml
|       +-- src/
|           |-- lib.rs
|           |-- error.rs
|           |-- config.rs
|           |-- collection/      # registry, handle, model
|           |-- document/        # pdf, docx, xlsx, ocr, chunker, model
|           |-- embedding/       # engine, models, download, types
|           |-- search/          # engine, bm25, ranking, result
|           |-- provenance/      # citation formatting
|           |-- storage/         # rocks, schema
|           +-- license/         # validator (ed25519)
|-- tests/
|   |-- integration/
|   |   |-- test_collection_lifecycle.rs
|   |   |-- test_ingest_pdf.rs
|   |   |-- test_search.rs
|   |   +-- test_mcp_tools.rs
|   +-- fixtures/
|       |-- sample.pdf
|       |-- sample.docx
|       |-- sample.xlsx
|       +-- scanned.png
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
#[command(name = "casetrack", about = "Document intelligence MCP server for Claude", version)]
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
    // === Collection Errors ===
    #[error("Collection not found: {0}")]
    CollectionNotFound(Uuid),

    #[error("No active collection. Create or switch to a collection first.")]
    NoCollectionActive,

    #[error("Collection name not found: \"{0}\"")]
    CollectionNameNotFound(String),

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

    #[error("Collection database failed to open at {}: {source}", .path.display())]
    CollectionDbOpenFailed { path: PathBuf, source: rocksdb::Error },

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
4. **User-facing errors include guidance**: "Create or switch to a collection first"
5. **Internal errors include report URL**: "Please report this at github.com/..."

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
}
```

---

## 6. Logging

Logging goes to stderr (stdout is MCP transport). Controlled by `RUST_LOG` env var.

| Level | Usage |
|-------|-------|
| ERROR | Failures preventing operations (file not found, DB corruption) |
| WARN  | Degraded functionality (low memory, OCR disabled) |
| INFO  | Normal operations (server started, collection created, search completed) |
| DEBUG | Internal details (model loading times, RocksDB stats) |
| TRACE | Verbose (individual chunk embeddings, token counts) |

---

## 7. Cross-Platform Concerns

### 7.1 Path Handling

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

### 7.2 Default Data Directory

| Platform | Default Path |
|----------|-------------|
| macOS | `~/Documents/CaseTrack/` |
| Windows | `C:\Users\{user}\Documents\CaseTrack\` |
| Linux | `~/Documents/CaseTrack/` (or `~/.local/share/casetrack/`) |

### 7.3 Platform Dependencies

| Component | macOS | Windows | Linux |
|-----------|-------|---------|-------|
| RocksDB | Works via `rust-rocksdb` | Requires MSVC build tools | Static link |
| Tesseract | Static link (vendored) | Bundle DLLs in installer | Static link or system pkg |
| ONNX Runtime | CoreML + CPU fallback | DirectML + CPU fallback | CPU only (CUDA optional) |

---

## 8. Security

### 8.1 Input Validation

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

### 8.2 License Key Security

- ed25519 signed keys, validated offline. Public key embedded in binary.
- Key format: `TIER-XXXXXX-XXXXXX-XXXXXX-SIG`
- Cached validation avoids repeated network calls. No user data sent.

### 8.3 No Network After Setup

After initial model download and license activation, CaseTrack makes zero network requests. Document processing, search, and storage are 100% local. Update checks are optional and non-blocking.

### 8.4 Data Privacy and Confidentiality

All documents and embeddings are stored locally on the user's machine. CaseTrack never transmits document content, metadata, or search queries to any external service. This ensures data privacy and confidentiality for sensitive business documents.

---

## 9. Testing Strategy

### 9.1 Unit Tests

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

### 9.2 Integration Tests

```rust
// tests/integration/test_collection_lifecycle.rs
#[tokio::test]
async fn test_create_list_switch_delete_collection() {
    let dir = tempdir().unwrap();
    let mut registry = CollectionRegistry::open(dir.path()).unwrap();

    let collection = registry.create_collection(CreateCollectionParams {
        name: "Project Alpha".to_string(),
        collection_id: None,
        collection_type: Some(CollectionType::Contract),
    }).unwrap();
    assert_eq!(collection.name, "Project Alpha");

    let collections = registry.list_collections().unwrap();
    assert_eq!(collections.len(), 1);

    let handle = registry.switch_collection(collection.id).unwrap();
    assert_eq!(registry.active_collection_id(), Some(collection.id));

    drop(handle);
    registry.delete_collection(collection.id).unwrap();
    assert_eq!(registry.list_collections().unwrap().len(), 0);
}

// tests/integration/test_search.rs
#[tokio::test]
async fn test_search_returns_relevant_results() {
    // Setup: create collection, ingest sample PDF with known content
    let results = search_engine.search(&collection_handle, "payment terms", 10, None).unwrap();

    assert!(!results.is_empty());
    assert!(results[0].score > 0.5);
    assert!(results[0].citation.contains("sample.pdf"));

    // Verify full provenance on every result
    for result in &results {
        assert!(!result.provenance.document_name.is_empty());
        assert!(!result.provenance.document_path.is_empty());
        assert!(result.provenance.page > 0);
        assert!(result.provenance.char_start < result.provenance.char_end);
    }
}

#[tokio::test]
async fn test_collection_isolation() {
    // Verify chunks from one collection never appear in another collection's search
    // Ingest into Collection A, search Collection B -- must return zero results
}
```

### 9.3 Test Fixtures

- `sample.pdf` -- 3-page PDF with known contract terms
- `sample.docx` -- Word document with headings, paragraphs, lists
- `sample.xlsx` -- Spreadsheet with financial data and multiple sheets
- `scanned.png` -- Image of typed text for OCR testing
- `empty.pdf` -- Edge case: empty PDF
- `large_paragraph.txt` -- Edge case: single paragraph >2000 characters

### 9.4 Running Tests

```bash
cargo test              # All tests
cargo test --lib        # Unit tests only (fast)
cargo test --test '*'   # Integration tests (needs fixtures)
RUST_LOG=debug cargo test -- --nocapture   # With logging
cargo test test_bm25_basic_search          # Specific test
```

---

## 10. CI/CD Pipeline

### 10.1 GitHub Actions CI

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

### 10.2 Release Pipeline

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

## 11. Monetization Implementation

### 11.1 Pricing Tiers

| Tier | Price | Collections | Docs/Collection | Embedders | Key Features |
|------|-------|-------------|-----------------|-----------|-------------|
| Free | $0 | 3 | 100 | 2 (E1, E6) | BM25, basic search, provenance |
| Pro | $29/mo | Unlimited | Unlimited | 3 (+ E12) | ColBERT rerank, auto-sync, priority support |

### 11.2 License Key System

See [PRD 07](PRD_07_COLLECTION_MANAGEMENT.md) for details. Key points: ed25519 offline validation, online activation via Lemon Squeezy on first use, 30-day cache, graceful degradation to Free tier on failure.

---

## 12. Implementation Roadmap

### Phase 1: Foundation

```
PROJECT SETUP
  [ ] Create workspace with crates/casetrack and crates/casetrack-core
  [ ] Configure Cargo.toml workspace dependencies
  [ ] Set up GitHub repo + CI (ci.yml)
  [ ] Implement error types (error.rs)
  [ ] Implement config + CLI parsing (config.rs, cli.rs)
  [ ] Set up tracing/logging to stderr

COLLECTION MANAGEMENT
  [ ] Implement Collection, CollectionType, CollectionStatus structs
  [ ] Implement CollectionRegistry (create, list, switch, delete)
  [ ] Implement CollectionHandle (open collection DB, column families)
  [ ] RocksDB configuration (rocks_options)
  [ ] Schema versioning (check_and_migrate)
  [ ] Unit tests for collection operations

MCP SERVER SKELETON
  [ ] Set up rmcp server with stdio transport
  [ ] Register create_collection, list_collections, switch_collection, delete_collection tools
  [ ] Test with Claude Code manually
```

### Phase 2: Document Ingestion

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

XLSX PROCESSING
  [ ] Implement XlsxProcessor (calamine crate)
  [ ] Sheet enumeration and cell extraction
  [ ] Table structure preservation
  [ ] Header row detection

CHUNKING (2000-character chunks, 10% overlap -- see PRD 06)
  [ ] Implement DocumentChunker (2000-char target, 200-char overlap, paragraph-aware)
  [ ] Character counting (not token-based)
  [ ] Long paragraph splitting (>2200 chars)
  [ ] Provenance attachment per chunk (file path, document name, page, paragraph, line, char offsets)
  [ ] Chunk boundary validation (min 400 chars, max 2200 chars)

STORAGE (Per-collection isolated databases -- see PRD 04)
  [ ] Store chunks in RocksDB (one DB per collection)
  [ ] Store document metadata
  [ ] Store provenance records (full path, page, paragraph, line, char offsets per chunk)
  [ ] Duplicate detection (SHA256)
  [ ] ingest_document MCP tool
  [ ] list_documents, get_document, delete_document tools
  [ ] get_chunk, get_document_chunks, get_source_context provenance tools (see PRD 09)
```

### Phase 3: Embedding & Search

```
MODEL MANAGEMENT
  [ ] Model download via hf-hub (with retry)
  [ ] Model spec definitions (repo, files, sizes)
  [ ] First-run download flow
  [ ] Model existence checking

EMBEDDING ENGINE
  [ ] ONNX Runtime setup (Environment, Session)
  [ ] E1 (bge-small dense embedding)
  [ ] E6 (SPLADE sparse embedding)
  [ ] Batch embedding for ingestion
  [ ] Store embeddings in RocksDB

BM25 INDEX
  [ ] Tokenization (lowercase, stopword removal)
  [ ] Inverted index (posting lists in RocksDB)
  [ ] BM25 scoring formula
  [ ] Index update during ingestion

SEARCH ENGINE
  [ ] Stage 1: BM25 recall
  [ ] Stage 2: Semantic ranking (E1 + E6 via RRF)
  [ ] Cosine similarity, sparse dot product
  [ ] search_documents MCP tool
  [ ] Result formatting with citations
```

### Phase 4: Pro Features

```
PRO EMBEDDERS
  [ ] E12 (ColBERT token-level)
  [ ] Stage 3: ColBERT rerank (MaxSim)

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
  [ ] reindex_collection tool (full rebuild of all documents in a collection)
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

### Phase 4b: Context Graph

```
ENTITY EXTRACTION (runs during ingestion, after chunking)
  [ ] Entity extraction pipeline (post-chunk processing step)
  [ ] Regex-based extractors:
      - Date patterns (deadlines, milestones, event dates)
      - Monetary amounts ("$1,250,000.00", "1.25 million dollars")
      - Location references (addresses, regions, countries)
  [ ] NER-based extractors:
      - Person names (stakeholders, contacts, signatories)
      - Organization names (companies, agencies, departments)
      - Concepts and topics (domain-specific terms)
  [ ] Entity deduplication (same entity across chunks/documents)
  [ ] Entity storage in `entities` and `entity_index` column families
  [ ] EntityMention records linking entities to chunks with char offsets

REFERENCE EXTRACTION & NETWORK
  [ ] Reference parser (regex for document references, standards, external sources)
  [ ] Reference normalization (canonical form for dedup)
  [ ] ReferenceRecord storage with source_doc, target, context
  [ ] Reference type classification (Document, Standard, Regulation, Report)
  [ ] Reference network storage in `references` column family
  [ ] Cross-document reference linking (Doc A references same source as Doc B)

KNOWLEDGE GRAPH
  [ ] Entity relationship graph construction
  [ ] Co-occurrence detection (entities appearing in same chunks)
  [ ] Cross-document entity linking
  [ ] Graph storage in `knowledge_graph` column family
  [ ] Graph traversal queries (shortest path, neighbors, clusters)

DOCUMENT GRAPH
  [ ] DocRelationship storage in `doc_graph` column family
  [ ] Relationship types: SharedReferences, SharedEntities, SemanticSimilar, VersionOf, Exhibits
  [ ] Automatic relationship detection during ingestion:
      - SharedReferences: documents referencing same external sources
      - SharedEntities: documents mentioning same entities
      - SemanticSimilar: E1 cosine > 0.75 between document-level embeddings
  [ ] Chunk similarity graph in `chunk_graph` column family
  [ ] Cross-chunk similarity edges (E1 cosine > 0.8 between chunks)

COLLECTION SUMMARY
  [ ] CollectionSummary builder (aggregates entities, references, relationships per collection)
  [ ] Stakeholder extraction and role classification (key people and organizations)
  [ ] Key date extraction and timeline construction
  [ ] Topic extraction from headings, content analysis
  [ ] Reference statistics (most-cited references in the collection)
  [ ] Entity statistics (most-mentioned entities)
  [ ] CollectionStatistics computation (doc count, chunk count, entity count, etc.)
  [ ] Collection summary storage in `collection_summary` column family
  [ ] Incremental collection summary updates (on ingest/delete/reindex)

CONTEXT GRAPH MCP TOOLS (18 tools -- see PRD 09 Section 2b)
  [ ] Collection Overview tools:
      - get_collection_summary (stakeholders, topics, key dates, key references)
      - get_collection_timeline (chronological events extracted from documents)
      - get_collection_statistics (counts, coverage, health metrics)
  [ ] Entity & Reference tools:
      - list_entities (filter by type, sort by mention count)
      - get_entity_mentions (all mentions of an entity across documents)
      - search_entity_relationships (entities connected via shared documents)
      - get_entity_graph (entity relationship visualization)
      - list_references (all referenced external sources with citation counts)
      - get_reference_citations (all documents citing a specific reference)
  [ ] Document Navigation tools:
      - get_document_structure (headings, sections, page count, entity/reference summary)
      - browse_pages (paginated page content with entities highlighted)
      - find_related_documents (documents related via references, entities, or semantics)
      - get_related_documents (knowledge-graph-first document discovery)
      - list_documents_by_type (filter by inferred document type)
      - traverse_chunks (sequential chunk navigation with prev/next)
  [ ] Advanced Search tools:
      - search_similar_chunks (find chunks semantically similar to a given chunk)
      - compare_documents (side-by-side entity, reference, and semantic comparison)
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

MEMORY MANAGEMENT
  [ ] System RAM detection (sysinfo)
  [ ] Auto memory tier selection
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

## 13. Success Metrics

### 13.1 Product Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Install success rate | >95% | Opt-in telemetry |
| Time to first search | <5 minutes | User testing |
| Search relevance (top 5) | >85% | Manual evaluation |
| Provenance accuracy | 100% | Automated tests |
| Crash rate | <0.1% | Error reporting |

### 13.2 Performance Metrics

| Metric | Free Tier | Pro Tier |
|--------|-----------|----------|
| Search latency (p95) | <150ms | <250ms |
| Ingestion speed | <1.5s/page | <1s/page |
| RAM usage (idle) | <500MB | <800MB |
| RAM usage (search) | <1.5GB | <2GB |
| Model download | <3 min | <5 min |
| Binary size | <30MB | <30MB |

### 13.3 Business Metrics

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
| Models (Free tier) | ~165MB |
| Models (Pro tier) | ~370MB |
| Collection database (per 100 docs) | ~5-50MB |
| Total install (Free, 1 collection) | ~200MB |
| Total install (Pro, 10 collections) | ~900MB |

## Appendix B: Comparison with Alternatives

| Feature | CaseTrack | Traditional SaaS | DIY RAG |
|---------|-----------|-------------------|---------|
| Price | $0-29/mo | $200-400/mo | Free |
| Install time | 2 min | N/A (SaaS) | Hours |
| Runs locally | Yes | No | Yes |
| No GPU required | Yes | N/A | Usually no |
| Claude integration | Native MCP | No | Manual |
| Provenance | Always | Sometimes | DIY |
| Document-optimized models | Yes | Varies | No |
| Privacy | 100% local | Cloud | Local |
| Offline capable | Yes | No | Yes |

---

*CaseTrack PRD v4.0.0 -- Document 10 of 10*

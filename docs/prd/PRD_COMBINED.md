# PRD 01: CaseTrack Overview

## One-Click Legal Document Analysis for Claude Code & Claude Desktop

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

---

## Document Index

This PRD is split across 10 documents. Each is self-contained but references the others.

| Doc | Title | Covers |
|-----|-------|--------|
| **01 (this)** | Overview | Executive summary, vision, principles, glossary |
| [02](PRD_02_TARGET_USER_HARDWARE.md) | Target User & Hardware | Users, hardware tiers, performance targets |
| [03](PRD_03_DISTRIBUTION_INSTALLATION.md) | Distribution & Installation | Channels, MCPB, manifest, install flows, updates |
| [04](PRD_04_STORAGE_ARCHITECTURE.md) | Storage Architecture | Local storage, RocksDB schema, data versioning |
| [05](PRD_05_EMBEDDER_STACK.md) | Embedder Stack | 7 embedders, ONNX, model management |
| [06](PRD_06_DOCUMENT_INGESTION.md) | Document Ingestion | PDF, DOCX, OCR, chunking |
| [07](PRD_07_CASE_MANAGEMENT.md) | Case Management & Provenance | Case model, isolation, citations |
| [08](PRD_08_SEARCH_RETRIEVAL.md) | Search & Retrieval | 4-stage pipeline, RRF, ranking |
| [09](PRD_09_MCP_TOOLS.md) | MCP Tools | All tool specs, examples, error responses |
| [10](PRD_10_TECHNICAL_BUILD.md) | Technical Build Guide | Bootstrap, crate structure, CI/CD, testing, security |

---

## 1. What is CaseTrack?

CaseTrack is a **one-click installable MCP server** that plugs into **Claude Code** and **Claude Desktop**, giving Claude the ability to ingest, search, and analyze legal documents. Everything runs on the user's machine -- **all embeddings, vectors, and databases are stored locally** on the user's device with zero cloud dependencies.

```
+---------------------------------------------------------------------------+
|                              CASETRACK                                     |
|                                                                           |
|        "Install once. Everything runs on YOUR machine."                   |
|                                                                           |
+---------------------------------------------------------------------------+
|                                                                           |
|   WHAT IT DOES                                                            |
|   ============                                                            |
|   - Ingests PDFs, DOCX, scanned images                                   |
|   - Embeds documents with 7 specialized legal embedders                  |
|   - Stores all vectors/embeddings in LOCAL database on YOUR device       |
|   - Provides semantic search with full source citations                  |
|                                                                           |
|   WHAT YOU GET                                                            |
|   ============                                                            |
|   - MCP server that plugs into Claude Code or Claude Desktop             |
|   - All 7 embedding models downloaded and ready to use                   |
|   - RocksDB database installed on your machine                           |
|   - Your data NEVER leaves your computer                                 |
|                                                                           |
|   RUNS ON                        REQUIRES                                 |
|   --------                       --------                                 |
|   - macOS (Intel + Apple Silicon)  - 8GB RAM minimum                     |
|   - Windows 10/11                  - 5GB storage                         |
|   - Linux (Ubuntu 20.04+)          - NO GPU needed                       |
|                                                                           |
|   STORAGE (All on YOUR device)                                            |
|   ============================                                            |
|   - Embedding models: ~400MB                                             |
|   - Vector database: RocksDB (scales with documents)                     |
|   - Embeddings: ~50KB per document page                                  |
|   - Everything stored in ~/Documents/CaseTrack/                          |
|                                                                           |
+---------------------------------------------------------------------------+
```

---

## 2. The Problem

Legal professionals waste hours searching through case documents:

- **Keyword search fails**: "breach of duty" won't find "violation of fiduciary obligation"
- **No AI integration**: Can't ask questions about documents in natural language
- **No provenance**: When you find something, you can't cite the exact source
- **Complex tools**: Existing legal tech requires IT departments and training
- **Expensive**: Enterprise legal tech costs $200-500+/seat/month

---

## 3. The Solution

CaseTrack solves this with:

1. **One-click install**: Single command or MCPB file -- embedders and database included
2. **100% local storage**: All embeddings and vectors stored on YOUR device in RocksDB
3. **Per-case isolation**: Every case has its own separate database -- embeddings from one case NEVER touch another
4. **Per-customer isolation**: Each customer's installation is fully independent (no shared servers or data)
5. **7 specialized embedders**: Semantic search that understands legal language
6. **Full provenance**: Every answer cites the source file path, document name, page, paragraph, and line number
7. **2000-character chunks**: Documents chunked into 2000-char segments with 10% overlap, each chunk stores its exact origin
8. **Claude Code + Desktop integration**: Works with both Claude Code CLI and Claude Desktop app
9. **Runs anywhere**: Works on an 8GB MacBook Air, no GPU needed
10. **Affordable**: Free tier is genuinely useful; Pro is $29/month

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

> **Any attorney can ask Claude questions about their case documents and get accurate, cited answers -- without IT support, cloud accounts, or technical knowledge.**

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
   Attorney-client privilege preserved
   License validation works offline after first activation

4. INSTANT VALUE
   First useful search within 5 minutes of download
   No training required
   Works like asking a research assistant

5. PROVENANCE ALWAYS
   Every answer includes exact source citation
   Document name, page, paragraph, line number
   One click to view original context

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

- **Not a document management system**: Use Dropbox/OneDrive for storage
- **Not a practice management tool**: No billing, calendaring, or client management
- **Not e-discovery software**: Not built for litigation holds or productions
- **Not a cloud service**: Everything runs locally, we never see your data
- **Not an LLM**: CaseTrack provides tools to Claude; it does not generate answers itself

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
|  |  +----------+  +--------+  |                                       |
|  |  +----------+  +--------+  |                                       |
|  |  | Chunking |  | 7 ONNX |  |                                       |
|  |  | Engine   |  | Models |  |                                       |
|  |  +----------+  +--------+  |                                       |
|  +--------+-------------------+                                       |
|           |                                                           |
|  +--------v-------------------+                                       |
|  | Local Storage              |   ~/Documents/CaseTrack/              |
|  |  +-------+  +-----------+  |                                       |
|  |  |Case A |  | Case B    |  |   Each case = isolated RocksDB       |
|  |  |RocksDB|  | RocksDB   |  |   Vectors, chunks, provenance        |
|  |  +-------+  +-----------+  |                                       |
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
| OCR | Tesseract (bundled) | Best open-source OCR |
| Model Download | hf-hub | Hugging Face model registry |
| Serialization | bincode + serde | Fast binary serialization for vectors |
| Async | tokio | Standard Rust async runtime |
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
| **ColBERT** | Contextualized Late Interaction over BERT -- token-level reranking |
| **Embedder** | A model that converts text to a numerical vector |
| **MCP** | Model Context Protocol -- standard for AI tool integration |
| **MCPB** | MCP Bundle -- a ZIP file format for distributing MCP servers |
| **ONNX** | Open Neural Network Exchange -- cross-platform ML model format |
| **Provenance** | The exact source location of text: file path, document name, page number, paragraph number, line number, and character offsets. Attached to every chunk and included in every search result and MCP tool response. |
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

## 1. Primary Users

```
PRIMARY: SOLO PRACTITIONERS & SMALL FIRMS
=================================================================================

Profile:
  - 1-10 attorney firm
  - No dedicated IT staff
  - Uses consumer hardware (MacBook, Windows laptop)
  - Handles 5-50 active matters
  - Documents stored in folders on local drive or cloud sync (Dropbox, OneDrive)
  - Already uses Claude Code or Claude Desktop for other work

Pain Points:
  - Can't find documents they know exist
  - Spend hours re-reading to find specific facts
  - No budget for enterprise legal tech ($500+/seat/month)
  - Frustrated by keyword search limitations
  - Need to cite sources precisely for court filings

Why CaseTrack:
  - Works on their existing laptop
  - No IT support needed
  - Affordable ($29/month or free tier)
  - Immediate productivity boost
  - Integrates with Claude they already use
```

---

## 2. Secondary Users

```
SECONDARY: PARALEGALS & LEGAL ASSISTANTS
=================================================================================

Profile:
  - Support 1-5 attorneys
  - Manage document collections per case
  - Responsible for organizing and indexing case files
  - Often do initial research and fact-finding

Pain Points:
  - Manual document review is tedious and error-prone
  - No way to semantically search across hundreds of documents
  - Need to produce exact citations for attorney review

Why CaseTrack:
  - Batch ingest entire case folders at once
  - Search returns cited sources ready for attorney review
  - Reduces manual document review by 70%+
```

```
TERTIARY: LAW STUDENTS & RESEARCHERS
=================================================================================

Profile:
  - Studying case law, writing papers
  - Limited budget (free tier)
  - Comfortable with CLI tools

Pain Points:
  - Organizing research materials across multiple cases
  - Finding connections between documents
  - Citing sources accurately

Why CaseTrack:
  - Free tier handles 3 cases with full search
  - Provenance system generates proper citations
```

---

## 3. User Personas

### Persona A: Sarah (Solo Practitioner)

- **Age**: 42
- **Practice**: Family law, solo
- **Hardware**: MacBook Air M1, 8GB RAM
- **Tech comfort**: Uses email, Word, basic cloud storage
- **Current workflow**: Ctrl+F in PDFs, manual notes in Word
- **CaseTrack use**: Ingests all case documents, asks Claude "What did the respondent claim about custody in the deposition?"
- **Key need**: Just works, no setup friction

### Persona B: Mike (Small Firm Partner)

- **Age**: 55
- **Practice**: Contract/commercial litigation, 5-attorney firm
- **Hardware**: Windows 11 desktop, 16GB RAM
- **Tech comfort**: Moderate, uses practice management software
- **Current workflow**: Associates do manual document review
- **CaseTrack use**: Firm license, each attorney searches their own cases
- **Key need**: Works on Windows, multiple seats, worth $99/month

### Persona C: Alex (Paralegal)

- **Age**: 28
- **Practice**: Personal injury firm
- **Hardware**: Windows 10 laptop, 8GB RAM
- **Tech comfort**: High, uses multiple software tools daily
- **Current workflow**: Manually indexes documents in spreadsheets
- **CaseTrack use**: Batch ingests entire case folders, builds searchable case databases
- **Key need**: Fast ingestion, reliable OCR for scanned documents

---

## 4. Minimum Hardware Requirements

```
MINIMUM REQUIREMENTS (Must Run)
=================================================================================

CPU:     Any 64-bit processor (2018 or newer recommended)
         - Intel Core i3 or better
         - AMD Ryzen 3 or better
         - Apple M1 or better

RAM:     8GB minimum
         - 16GB recommended for large cases (1000+ pages)

Storage: 5GB available
         - 400MB for embedding models (one-time download)
         - 4.6GB for case data (scales with usage)
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

## 5. Performance by Hardware Tier

### 5.1 Ingestion Performance

| Hardware | 50-page PDF | 500-page PDF | OCR (50 scanned pages) |
|----------|-------------|--------------|------------------------|
| **Entry** (M1 Air 8GB) | 45 seconds | 7 minutes | 3 minutes |
| **Mid** (M2 Pro 16GB) | 25 seconds | 4 minutes | 2 minutes |
| **High** (i7 32GB) | 20 seconds | 3 minutes | 90 seconds |
| **With GPU** (RTX 3060) | 10 seconds | 90 seconds | 45 seconds |

### 5.2 Search Performance

| Hardware | Free Tier (2-stage) | Pro Tier (4-stage) | Concurrent Models |
|----------|--------------------|--------------------|-------------------|
| **Entry** (M1 Air 8GB) | 100ms | 200ms | 3 (lazy loaded) |
| **Mid** (M2 Pro 16GB) | 60ms | 120ms | 5 |
| **High** (i7 32GB) | 40ms | 80ms | 7 (all loaded) |
| **With GPU** (RTX 3060) | 20ms | 50ms | 7 (all loaded) |

### 5.3 Memory Usage

| Scenario | RAM Usage |
|----------|-----------|
| Idle (server running, no models loaded) | ~50MB |
| Free tier (3 models loaded) | ~800MB |
| Pro tier (6 models loaded) | ~1.5GB |
| During ingestion (peak) | +300MB above baseline |
| During search (peak) | +100MB above baseline |

---

## 6. Supported Platforms

### 6.1 Build Targets

| Platform | Architecture | Binary Name | Status |
|----------|-------------|-------------|--------|
| macOS | x86_64 (Intel) | `casetrack-darwin-x64` | Supported |
| macOS | aarch64 (Apple Silicon) | `casetrack-darwin-arm64` | Supported |
| Windows | x86_64 | `casetrack-win32-x64.exe` | Supported |
| Linux | x86_64 | `casetrack-linux-x64` | Supported |
| Linux | aarch64 | `casetrack-linux-arm64` | Future |

### 6.2 Platform-Specific Notes

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

### 6.3 Claude Integration Compatibility

| Client | Transport | Config Location | Status |
|--------|-----------|-----------------|--------|
| Claude Code (CLI) | stdio | `~/.claude/settings.json` | Primary target |
| Claude Desktop (macOS) | stdio | `~/Library/Application Support/Claude/claude_desktop_config.json` | Supported |
| Claude Desktop (Windows) | stdio | `%APPDATA%\Claude\claude_desktop_config.json` | Supported |
| Claude Desktop (Linux) | stdio | `~/.config/Claude/claude_desktop_config.json` | Supported |

---

## 7. Graceful Degradation Strategy

CaseTrack adapts to available hardware:

```
DEGRADATION TIERS
=================================================================================

TIER 1: FULL (16GB+ RAM)
  - All models loaded in memory simultaneously
  - Zero model loading latency on search
  - Maximum ingestion throughput (parallel embedding)

TIER 2: STANDARD (8-16GB RAM)
  - Free models always loaded (E1, E6, E7 = ~800MB)
  - Pro models lazy-loaded on demand
  - ~200ms model load penalty on first Pro search
  - Models stay loaded after first use until memory pressure

TIER 3: CONSTRAINED (<8GB RAM)
  - Only E1 + E13 (BM25) always loaded (~400MB)
  - Other models loaded one at a time, unloaded after use
  - Ingestion uses sequential (not parallel) embedding
  - Search still works but with higher latency
  - Warning shown on startup: "Low memory mode active"

DETECTION:
  - On startup, check available RAM via sysinfo crate
  - Set tier automatically, log the decision
  - User can override via --memory-mode=full|standard|constrained
```

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
   curl -fsSL https://casetrack.legal/install.sh | sh

   # Windows - PowerShell:
   irm https://casetrack.legal/install.ps1 | iex

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

The install script must:

```bash
#!/bin/sh
set -e

# 1. Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')   # darwin, linux
ARCH=$(uname -m)                                # x86_64, arm64, aarch64

# 2. Map to binary name
case "${OS}-${ARCH}" in
  darwin-arm64)   BINARY="casetrack-darwin-arm64" ;;
  darwin-x86_64)  BINARY="casetrack-darwin-x64" ;;
  linux-x86_64)   BINARY="casetrack-linux-x64" ;;
  linux-aarch64)  BINARY="casetrack-linux-arm64" ;;
  *) echo "Unsupported platform: ${OS}-${ARCH}"; exit 1 ;;
esac

# 3. Download binary
VERSION="latest"
URL="https://github.com/casetrack-legal/casetrack/releases/${VERSION}/download/${BINARY}"
INSTALL_DIR="${HOME}/.local/bin"

mkdir -p "${INSTALL_DIR}"
curl -fsSL "${URL}" -o "${INSTALL_DIR}/casetrack"
chmod +x "${INSTALL_DIR}/casetrack"

# 4. Add to PATH if needed
if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
  SHELL_RC=""
  if [ -f "${HOME}/.zshrc" ]; then SHELL_RC="${HOME}/.zshrc"
  elif [ -f "${HOME}/.bashrc" ]; then SHELL_RC="${HOME}/.bashrc"
  fi
  if [ -n "${SHELL_RC}" ]; then
    echo "export PATH=\"${INSTALL_DIR}:\${PATH}\"" >> "${SHELL_RC}"
  fi
fi

# 5. Configure Claude Code (if installed)
CLAUDE_SETTINGS="${HOME}/.claude/settings.json"
if [ -d "${HOME}/.claude" ]; then
  # Merge MCP server config into existing settings
  casetrack --setup-claude-code
fi

# 6. Print success
echo ""
echo "CaseTrack installed successfully!"
echo ""
echo "  Binary: ${INSTALL_DIR}/casetrack"
echo "  Data:   ~/Documents/CaseTrack/"
echo ""
echo "Next steps:"
echo "  1. Restart your terminal (or run: source ${SHELL_RC})"
echo "  2. Open Claude Code and ask: 'Create a new case called Test'"
echo ""
```

### 2.2 Windows Install Script (`install.ps1`)

```powershell
# Detect architecture
$arch = if ([Environment]::Is64BitOperatingSystem) { "x64" } else {
    Write-Error "CaseTrack requires 64-bit Windows"; exit 1
}

# Download
$version = "latest"
$url = "https://github.com/casetrack-legal/casetrack/releases/$version/download/casetrack-win32-$arch.exe"
$installDir = "$env:LOCALAPPDATA\CaseTrack"

New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Invoke-WebRequest -Uri $url -OutFile "$installDir\casetrack.exe"

# Add to PATH
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$installDir;$currentPath", "User")
}

# Configure Claude Code
& "$installDir\casetrack.exe" --setup-claude-code

Write-Host ""
Write-Host "CaseTrack installed successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "  Binary: $installDir\casetrack.exe"
Write-Host "  Data:   $env:USERPROFILE\Documents\CaseTrack\"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Restart your terminal"
Write-Host "  2. Open Claude Code and ask: 'Create a new case called Test'"
```

---

## 3. Self-Setup CLI Command

The binary itself handles Claude Code configuration:

```
casetrack --setup-claude-code
```

This command:

1. Detects Claude Code settings file location:
   - `~/.claude/settings.json` (all platforms)
2. Reads existing settings (or creates empty `{}`)
3. Merges `mcpServers.casetrack` entry
4. Writes back with proper JSON formatting
5. Prints confirmation

```rust
/// CLI setup command for Claude Code integration
pub fn setup_claude_code(data_dir: &Path) -> Result<()> {
    let settings_path = dirs::home_dir()
        .ok_or(CaseTrackError::NoHomeDir)?
        .join(".claude")
        .join("settings.json");

    // Read existing settings or create new
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        fs::create_dir_all(settings_path.parent().unwrap())?;
        json!({})
    };

    // Add MCP server config
    let mcp_servers = settings
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert(json!({}));

    mcp_servers["casetrack"] = json!({
        "command": std::env::current_exe()?.to_string_lossy(),
        "args": ["--data-dir", data_dir.to_string_lossy()]
    });

    // Write back
    let formatted = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, formatted)?;

    println!("Claude Code configured. CaseTrack will be available in your next session.");
    Ok(())
}
```

---

## 4. MCPB Bundle Structure

The `.mcpb` file is a ZIP archive for Claude Desktop GUI installation:

```
casetrack.mcpb (ZIP archive, ~50MB)
|-- manifest.json           # MCP configuration
|-- icon.png               # Extension icon (256x256)
|-- server/
|   |-- casetrack-darwin-x64      # macOS Intel
|   |-- casetrack-darwin-arm64    # macOS Apple Silicon
|   |-- casetrack-win32-x64.exe   # Windows
|   +-- casetrack-linux-x64       # Linux
+-- resources/
    |-- tokenizer.json     # Shared tokenizer (~5MB)
    +-- legal-vocab.txt    # Legal term expansions (~2MB)
```

### 4.1 Manifest Specification

```json
{
  "manifest_version": "1.0",
  "name": "casetrack",
  "version": "1.0.0",
  "display_name": "CaseTrack Legal Document Analysis",
  "description": "Ingest PDFs, Word docs, and scans. Search with AI. Every answer cites the source.",

  "author": {
    "name": "CaseTrack",
    "url": "https://casetrack.legal"
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
      "description": "Leave blank for free tier. Purchase at casetrack.legal",
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
INSTALLATION FLOW
=================================================================================

Step 1: Download (User Action)
---------------------------------
User visits casetrack.legal
Clicks "Download for Claude Desktop"
Browser downloads casetrack.mcpb (~50MB)

Step 2: Install (User Action)
---------------------------------
User double-clicks casetrack.mcpb
  OR drags to Claude Desktop window
  OR Settings -> Extensions -> Install from file

Step 3: Configure (Dialog)
---------------------------------
+-------------------------------------------------------+
| Install CaseTrack?                                     |
|                                                        |
| CaseTrack lets you search legal documents with AI.     |
| All processing happens on your computer.               |
|                                                        |
| +----------------------------------------------------+ |
| | Data Location                                      | |
| | [~/Documents/CaseTrack                        ] [F]| |
| +----------------------------------------------------+ |
|                                                        |
| +----------------------------------------------------+ |
| | License Key (optional - leave blank for free tier)  | |
| | [                                             ] [L] | |
| +----------------------------------------------------+ |
|                                                        |
| This extension will:                                   |
| [Y] Read and write files in your Data Location        |
| [Y] Download AI models from huggingface.co (~400MB)   |
| [N] NOT send your documents anywhere                  |
|                                                        |
|                         [Cancel]  [Install Extension]  |
+-------------------------------------------------------+

Step 4: First Run (Automatic)
---------------------------------
Claude Desktop starts CaseTrack server
Server detects missing models
Downloads models in background (~400MB)
Shows progress notification in Claude Desktop

Step 5: Ready (Automatic)
---------------------------------
CaseTrack icon appears in Extensions panel
User can now use CaseTrack tools in conversation
```

---

## 6. First-Run Experience

On first launch, the server must handle model bootstrapping gracefully:

```rust
/// First-run initialization sequence
pub async fn initialize(config: &Config) -> Result<ServerState> {
    let data_dir = &config.data_dir;

    // Step 1: Create directory structure
    create_directory_structure(data_dir)?;

    // Step 2: Check and download models
    let model_manager = ModelManager::new(&data_dir.join("models"));
    let missing = model_manager.check_missing_models(config.tier)?;

    if !missing.is_empty() {
        // Report progress via MCP notification (if supported)
        // or via stderr logging
        tracing::info!(
            "First run detected. Downloading {} models ({} MB)...",
            missing.len(),
            missing.iter().map(|m| m.size_mb).sum::<u32>()
        );

        for model in &missing {
            tracing::info!("Downloading {}...", model.id);
            model_manager.download(model).await?;
            tracing::info!("Downloaded {} ({} MB)", model.id, model.size_mb);
        }

        tracing::info!("All models ready.");
    }

    // Step 3: Open or create registry database
    let registry = CaseRegistry::open(&data_dir.join("registry.db"))?;

    // Step 4: Validate license (offline-first)
    let tier = LicenseManager::new(data_dir)
        .validate(config.license_key.as_deref());

    tracing::info!("CaseTrack ready. Tier: {:?}, Cases: {}", tier, registry.count()?);

    Ok(ServerState { registry, model_manager, tier })
}

fn create_directory_structure(data_dir: &Path) -> Result<()> {
    fs::create_dir_all(data_dir.join("models"))?;
    fs::create_dir_all(data_dir.join("cases"))?;
    // registry.db is created by RocksDB::open
    Ok(())
}
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
        id: "e1-legal",
        repo: "BAAI/bge-small-en-v1.5",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 65,
        required: true,
    },
    ModelSpec {
        id: "e6-legal",
        repo: "naver/splade-cocondenser-selfdistil",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 55,
        required: true,
    },
    ModelSpec {
        id: "e7",
        repo: "sentence-transformers/all-MiniLM-L6-v2",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 45,
        required: true,
    },
    ModelSpec {
        id: "e8-legal",
        repo: "casetrack/citation-minilm-v1",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 35,
        required: false,
    },
    ModelSpec {
        id: "e11-legal",
        repo: "nlpaueb/legal-bert-small-uncased",
        files: &["model.onnx", "tokenizer.json"],
        size_mb: 60,
        required: false,
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

```rust
/// Download with retry and resume support
pub async fn download_model(&self, spec: &ModelSpec) -> Result<()> {
    let model_dir = self.cache_dir.join(spec.id);
    fs::create_dir_all(&model_dir)?;

    for file in spec.files {
        let dest = model_dir.join(file);

        // Skip if already downloaded and valid
        if dest.exists() && self.verify_checksum(&dest, spec, file)? {
            continue;
        }

        // Download with retry (3 attempts, exponential backoff)
        let mut attempts = 0;
        loop {
            attempts += 1;
            match self.download_file(spec.repo, file, &dest).await {
                Ok(()) => break,
                Err(e) if attempts < 3 => {
                    tracing::warn!(
                        "Download attempt {}/3 failed for {}/{}: {}. Retrying...",
                        attempts, spec.id, file, e
                    );
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempts))).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    Ok(())
}
```

---

## 7. Update Mechanism

### 7.1 Version Checking

CaseTrack checks for updates on startup (non-blocking, does not delay server start):

```rust
/// Non-blocking update check
pub async fn check_for_updates(current_version: &str) {
    // Fire and forget -- do not block server startup
    tokio::spawn(async move {
        let check = async {
            let url = "https://api.github.com/repos/casetrack-legal/casetrack/releases/latest";
            let resp: GithubRelease = reqwest::get(url).await?.json().await?;
            let latest = resp.tag_name.trim_start_matches('v');

            if semver::Version::parse(latest)? > semver::Version::parse(current_version)? {
                tracing::info!(
                    "Update available: v{} -> v{}. \
                     Run 'casetrack --update' or download from https://casetrack.legal",
                    current_version, latest
                );
            }
            Ok::<(), anyhow::Error>(())
        };

        if let Err(e) = check.await {
            // Silently ignore update check failures -- user is offline or rate limited
            tracing::debug!("Update check failed (non-critical): {}", e);
        }
    });
}
```

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
5. Does NOT delete `~/Documents/CaseTrack/` (user's data is sacred)

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


---

# PRD 05: 7-Embedder Legal Stack

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. Design Philosophy

The embedder stack is designed for **consumer hardware**:

- **7 embedders** (not 13-15): Reduced from research system for practical use
- **384D max**: Smaller dimensions = less RAM, faster search
- **ONNX format**: CPU-optimized, cross-platform
- **Quantized (INT8)**: 50% smaller, nearly same quality
- **No LLM inference**: Removed causal/reasoning embedders that need GPUs
- **Tiered loading**: Free tier loads 3 models; Pro loads 6 (BM25 is algorithmic)

---

## 2. Embedder Specifications

### E1-LEGAL: Semantic Similarity (PRIMARY)

| Property | Value |
|----------|-------|
| Model | bge-small-en-v1.5 (BAAI) |
| Dimension | 384 |
| Size | 65MB (INT8 ONNX) |
| Speed | 50ms/chunk (M1), 100ms/chunk (Intel i5) |
| Tier | FREE |
| Purpose | Core semantic search |

**What it finds**: "breach of duty" matches "violation of fiduciary obligation"
**Role in pipeline**: Foundation embedder. All search queries start here. Stage 2 ranking.

### E6-LEGAL: Keyword Expansion (SPLADE)

| Property | Value |
|----------|-------|
| Model | SPLADE-cocondenser-selfdistil (Naver) |
| Dimension | Sparse (30K vocabulary) |
| Size | 55MB (INT8 ONNX) |
| Speed | 30ms/chunk |
| Tier | FREE |
| Purpose | Exact term matching + expansion |

**What it finds**: "Daubert" also matches "expert testimony", "Rule 702"
**Role in pipeline**: Stage 2 ranking alongside E1. Catches exact terminology E1 misses.

### E7: Structured Text

| Property | Value |
|----------|-------|
| Model | all-MiniLM-L6-v2 (Sentence Transformers) |
| Dimension | 384 |
| Size | 45MB (INT8 ONNX) |
| Speed | 40ms/chunk |
| Tier | FREE |
| Purpose | Contracts, statutes, numbered clauses |

**What it finds**: Understands "Section 4.2(a)" structure, numbered lists, clause references
**Role in pipeline**: Stage 2 (Free tier), Stage 3 boost (Pro tier)

### E8-LEGAL: Citation Relationships

| Property | Value |
|----------|-------|
| Model | MiniLM-L3-v2 fine-tuned on citation pairs |
| Dimension | 256 (asymmetric: citing/cited) |
| Size | 35MB (INT8 ONNX) |
| Speed | 25ms/chunk |
| Tier | PRO |
| Purpose | Find documents citing same authorities |

**What it finds**: "Cases citing Miranda v. Arizona", related precedents
**Role in pipeline**: Stage 3 multi-signal boost. Asymmetric: citing direction matters.

### E11-LEGAL: Legal Entities

| Property | Value |
|----------|-------|
| Model | legal-bert-small-uncased (nlpaueb) |
| Dimension | 384 |
| Size | 60MB (INT8 ONNX) |
| Speed | 45ms/chunk |
| Tier | PRO |
| Purpose | Find by party, court, statute, doctrine |

**What it finds**: "Documents mentioning Judge Smith", "References to 42 USC 1983"
**Role in pipeline**: Stage 3 multi-signal boost. Entity-aware similarity.

### E12: Precision Reranking (ColBERT)

| Property | Value |
|----------|-------|
| Model | ColBERT-v2-small |
| Dimension | 64 per token |
| Size | 110MB (INT8 ONNX) |
| Speed | 100ms for top 50 candidates |
| Tier | PRO |
| Purpose | Final reranking for exact phrase matches |

**What it finds**: "breach of fiduciary duty" ranks higher than "fiduciary duty was not breached"
**Role in pipeline**: Stage 4 (final rerank). Token-level MaxSim scoring. Only runs on top 50 candidates.

### E13: Fast Recall (BM25)

| Property | Value |
|----------|-------|
| Model | None (algorithmic -- BM25/TF-IDF) |
| Dimension | N/A (inverted index) |
| Size | ~2MB index per 1000 documents |
| Speed | <5ms for any query |
| Tier | FREE |
| Purpose | Fast initial candidate retrieval |

**What it finds**: Exact keyword matches, high recall
**Role in pipeline**: Stage 1. Retrieves initial 500 candidates from inverted index.

---

## 3. Footprint Summary

| Metric | Free Tier | Pro Tier |
|--------|-----------|----------|
| Models to download | 3 (E1, E6, E7) | 6 (+ E8, E11, E12) |
| Model disk space | ~165MB | ~370MB |
| RAM at runtime | ~800MB | ~1.5GB |
| Per-chunk embed time | ~120ms | ~290ms |
| Search latency | <100ms | <200ms |

---

## 4. What Was Removed (vs. Research System)

| Removed | Original Purpose | Why Removed | Alternative in CaseTrack |
|---------|------------------|-------------|--------------------------|
| E2-E4 Temporal | Document recency/sequence | Complex, minimal value for legal | Date metadata filter |
| E5 Causal | "X caused Y" reasoning | Requires LLM-scale compute | Keyword patterns via E6 |
| E9 HDC | Noise robustness | Experimental, adds 200MB+ RAM | BM25 fallback |
| E10 Intent | Same-goal matching | 400MB+ model, GPU preferred | E1 semantic covers this |
| E14 SAILER | Legal doc structure | Research model, no ONNX export | E7 handles structure |
| E15 Citation Network | Graph-based citation | Requires graph training infrastructure | E8 simpler pairwise version |

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
    E1Legal,    // Semantic (FREE)
    E6Legal,    // Keywords (FREE)
    E7,         // Structured (FREE)
    E8Legal,    // Citations (PRO)
    E11Legal,   // Entities (PRO)
    E12,        // ColBERT (PRO)
    // E13 is BM25, not a neural model
}

impl EmbedderId {
    pub fn model_dir_name(&self) -> &'static str {
        match self {
            Self::E1Legal => "bge-small-en-v1.5",
            Self::E6Legal => "splade-distil",
            Self::E7 => "minilm-l6",
            Self::E8Legal => "citation-minilm",
            Self::E11Legal => "legal-bert-small",
            Self::E12 => "colbert-small",
        }
    }

    pub fn dimension(&self) -> usize {
        match self {
            Self::E1Legal => 384,
            Self::E6Legal => 0,    // Sparse
            Self::E7 => 384,
            Self::E8Legal => 256,
            Self::E11Legal => 384,
            Self::E12 => 64,       // Per token
        }
    }

    pub fn is_sparse(&self) -> bool {
        matches!(self, Self::E6Legal)
    }

    pub fn is_free_tier(&self) -> bool {
        matches!(self, Self::E1Legal | Self::E6Legal | Self::E7)
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
                EmbedderId::E1Legal,
                EmbedderId::E6Legal,
                EmbedderId::E7,
            ],
            _ => vec![
                EmbedderId::E1Legal,
                EmbedderId::E6Legal,
                EmbedderId::E7,
                EmbedderId::E8Legal,
                EmbedderId::E11Legal,
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
                    EmbedderId::E6Legal => {
                        embeddings.e6_legal = Some(self.run_sparse_inference(session, text)?);
                    }
                    EmbedderId::E12 => {
                        embeddings.e12 = Some(self.run_token_inference(session, text)?);
                    }
                    _ => {
                        let vec = self.run_dense_inference(session, text)?;
                        match id {
                            EmbedderId::E1Legal => embeddings.e1_legal = Some(vec),
                            EmbedderId::E7 => embeddings.e7 = Some(vec),
                            EmbedderId::E8Legal => embeddings.e8_legal = Some(vec),
                            EmbedderId::E11Legal => embeddings.e11_legal = Some(vec),
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
            EmbedderId::E6Legal => {
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
    pub e1_legal: Option<Vec<f32>>,        // 384D
    pub e6_legal: Option<SparseVec>,       // Sparse
    pub e7: Option<Vec<f32>>,              // 384D
    pub e8_legal: Option<Vec<f32>>,        // 256D
    pub e11_legal: Option<Vec<f32>>,       // 384D
    pub e12: Option<TokenEmbeddings>,      // 64D per token
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
        for id in &[EmbedderId::E8Legal, EmbedderId::E11Legal, EmbedderId::E12] {
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

A `scripts/convert_models.py` script should be included in the repository to automate this for all 6 neural models. Pre-converted ONNX models should be hosted on Hugging Face under a `casetrack/` organization.

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
| Images (JPG/PNG/TIFF) | Tesseract OCR | Good | `tesseract`, `image` | Single page per image |
| TXT/RTF | Direct read | Excellent | `std::fs` | Plain text, no metadata |

---

## 2. Ingestion Pipeline

```
DOCUMENT INGESTION FLOW
=================================================================================

User: "Ingest ~/Downloads/Complaint.pdf"
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
|    - Extract document metadata (title, author, dates)                 |
|    Output: ParsedDocument { pages: Vec<Page>, metadata }              |
+-----------------------------------------------------------------------+
                    |
                    v
+-----------------------------------------------------------------------+
| 3. CHUNK                                                               |
|    - Split into 2000-character chunks                                  |
|    - 10% overlap (200 chars from end of previous chunk)                |
|    - Respect paragraph and sentence boundaries                         |
|    - Attach FULL provenance to every chunk:                            |
|      file path, doc name, page, paragraph, line, char offsets          |
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
| 5. STORE                                                               |
|    - Write chunks + embeddings to case RocksDB                        |
|    - Write provenance records                                         |
|    - Update BM25 inverted index                                       |
|    - Update document metadata and case stats                          |
|    - Optionally copy original file to case/originals/                 |
|    Output: IngestResult { pages, chunks, duration }                   |
+-----------------------------------------------------------------------+
                    |
                    v
Response: "Ingested Complaint.pdf: 45 pages, 234 chunks, 12s"
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

## 5. OCR (Tesseract)

### 5.1 Bundling Strategy

Tesseract is bundled with the CaseTrack binary:
- **macOS**: Statically linked via `leptonica-sys` and `tesseract-sys`
- **Windows**: Tesseract DLLs included in installer/MCPB bundle
- **Linux**: Statically linked via musl build

The `eng.traineddata` language model (~15MB) is included in the MCPB bundle or downloaded on first OCR use.

### 5.2 OCR Pipeline

```rust
pub struct OcrEngine {
    tesseract: tesseract::Tesseract,
}

impl OcrEngine {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let tessdata = data_dir.join("models").join("tessdata");
        let tesseract = tesseract::Tesseract::new(
            tessdata.to_str().unwrap(),
            "eng",
        )?;
        Ok(Self { tesseract })
    }

    pub fn recognize(&self, image: &image::DynamicImage) -> Result<OcrResult> {
        // Preprocess image for better OCR accuracy
        let processed = self.preprocess(image);

        // Convert to bytes
        let bytes = processed.to_luma8();

        let mut tess = self.tesseract.clone();
        tess.set_image(
            bytes.as_raw(),
            bytes.width() as i32,
            bytes.height() as i32,
            1,  // bytes per pixel
            bytes.width() as i32,  // bytes per line
        )?;

        let text = tess.get_text()?;
        let confidence = tess.mean_text_conf();

        Ok(OcrResult {
            text,
            confidence: confidence as f32 / 100.0,
        })
    }

    /// Image preprocessing for better OCR results
    fn preprocess(&self, image: &image::DynamicImage) -> image::DynamicImage {
        image
            .grayscale()          // Convert to grayscale
            .adjust_contrast(1.5) // Increase contrast
            // Binarization handled by Tesseract internally
    }
}
```

---

## 6. Chunking Strategy

### 6.1 Chunking Rules (MANDATORY)

```
CHUNKING SPECIFICATION
=================================================================================

CHUNK SIZE:    2000 characters (hard target)
OVERLAP:       10% = 200 characters (from end of previous chunk)
MIN SIZE:      400 characters (don't emit tiny fragments)
MAX SIZE:      2200 characters (allow small overrun to avoid mid-sentence splits)

WHY 2000 CHARACTERS:
  - Large enough to capture full legal paragraphs and clauses
  - Small enough for focused embedding (semantic dilution above 2500 chars)
  - Character-based (not token-based) for deterministic, reproducible chunking
  - 10% overlap ensures context continuity across chunk boundaries

BOUNDARY RULES:
  1. Prefer splitting at paragraph boundaries
  2. If no paragraph break, split at sentence boundary (period + space)
  3. If no sentence break, split at word boundary (space)
  4. NEVER split mid-word
  5. Chunks do NOT cross page boundaries (each page starts a new chunk)
```

### 6.2 Provenance Per Chunk (MANDATORY)

**Every chunk MUST store its complete provenance at creation time.** This is not optional.

```
PROVENANCE STORED WITH EVERY CHUNK
=================================================================================

Every chunk records:
  - document_id:       UUID of the ingested document
  - document_name:     Original filename ("Contract.pdf")
  - document_path:     Full filesystem path ("/Users/sarah/Cases/Contract.pdf")
  - page:              Page number in the original document
  - paragraph_start:   First paragraph index included in this chunk
  - paragraph_end:     Last paragraph index included in this chunk
  - line_start:        First line number in the original page
  - line_end:          Last line number in the original page
  - char_start:        Character offset from start of page
  - char_end:          Character offset from start of page
  - extraction_method: How text was extracted (Native / OCR)
  - ocr_confidence:    OCR confidence score (if applicable)
  - bates_number:      Optional Bates stamp (for litigation)
  - chunk_index:       Sequential position of this chunk within the document

This provenance is:
  1. STORED in RocksDB alongside the chunk text and embeddings
  2. RETURNED in every search result
  3. QUERYABLE via MCP tools (get_chunk_provenance, get_document_chunks)
  4. IMMUTABLE once created (never modified after ingestion)
```

### 6.3 Chunking Implementation

```rust
/// Chunker configuration: 2000 chars, 10% overlap
pub struct LegalChunker {
    target_chars: usize,   // 2000
    max_chars: usize,      // 2200 (small overrun to avoid mid-sentence)
    min_chars: usize,      // 400 (don't emit tiny fragments)
    overlap_chars: usize,  // 200 (10% of target)
}

impl Default for LegalChunker {
    fn default() -> Self {
        Self {
            target_chars: 2000,
            max_chars: 2200,
            min_chars: 400,
            overlap_chars: 200,  // 10% overlap
        }
    }
}

impl LegalChunker {
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
                bates_number: None,
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

## 7. Batch Ingestion (Pro Tier)

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
    let supported = &["pdf", "docx", "doc", "txt", "rtf", "jpg", "jpeg", "png", "tiff", "tif"];

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

## 8. Duplicate Detection

Before ingesting, check if the document already exists in the case:

```rust
pub fn check_duplicate(case: &CaseHandle, file_hash: &str) -> Result<Option<Uuid>> {
    let cf = case.db.cf_handle("documents").unwrap();
    let iter = case.db.iterator_cf(&cf, rocksdb::IteratorMode::Start);

    for item in iter {
        let (_, value) = item?;
        let doc: DocumentMetadata = bincode::deserialize(&value)?;
        if doc.file_hash == file_hash {
            return Ok(Some(doc.id));
        }
    }

    Ok(None)
}
```

If duplicate is found, return an error with the existing document ID:

```
"Document already ingested as 'Complaint.pdf' (ID: abc-123).
 Use --force to re-ingest."
```

---

## 9. Data Types

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
    Native,   // Direct text extraction from PDF/DOCX
    Ocr,      // Tesseract OCR
    Skipped,  // OCR disabled, scanned page skipped
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


---

# PRD 08: Search & Retrieval

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

## 1. 4-Stage Search Pipeline

```
+-----------------------------------------------------------------------+
|                        4-STAGE SEARCH PIPELINE                         |
+-----------------------------------------------------------------------+
|                                                                       |
|  Query: "What does the contract say about early termination?"         |
|                                                                       |
|  +---------------------------------------------------------------+   |
|  | STAGE 1: BM25 RECALL                                  [<5ms]   |   |
|  |                                                                |   |
|  | - E13 inverted index lookup                                   |   |
|  | - Terms: "contract", "early", "termination"                   |   |
|  | - Fast lexical matching                                       |   |
|  |                                                                |   |
|  | Output: 500 candidate chunks                                  |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 2: SEMANTIC RANKING                             [<80ms]  |   |
|  |                                                                |   |
|  | - E1-LEGAL: Semantic similarity (384D dense cosine)           |   |
|  | - E6-LEGAL: Keyword expansion (sparse dot product)            |   |
|  | - E7: Structured text similarity (Free) / boost (Pro)         |   |
|  | - Score fusion via Reciprocal Rank Fusion (RRF)               |   |
|  |                                                                |   |
|  | Output: 100 candidates, ranked                                |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 3: MULTI-SIGNAL BOOST (PRO TIER ONLY)          [<30ms]  |   |
|  |                                                                |   |
|  | - E8-LEGAL: Boost citation similarity                         |   |
|  | - E11-LEGAL: Boost entity matches                             |   |
|  |                                                                |   |
|  | Weights: 0.6 x semantic + 0.2 x structure                    |   |
|  |        + 0.1 x citation + 0.1 x entity                       |   |
|  |                                                                |   |
|  | Output: 50 candidates, re-ranked                              |   |
|  +---------------------------------------------------------------+   |
|                              |                                        |
|                              v                                        |
|  +---------------------------------------------------------------+   |
|  | STAGE 4: COLBERT RERANK (PRO TIER ONLY)              [<100ms] |   |
|  |                                                                |   |
|  | - E12: Token-level MaxSim scoring                             |   |
|  | - Ensures exact phrase matches rank highest                   |   |
|  | - "early termination" > "termination that was early"          |   |
|  |                                                                |   |
|  | Output: Top K results with provenance                         |   |
|  +---------------------------------------------------------------+   |
|                                                                       |
|  LATENCY TARGETS                                                      |
|  ----------------                                                     |
|  Free tier (Stages 1-2):  <100ms                                     |
|  Pro tier (Stages 1-4):   <200ms                                     |
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
        case: &CaseHandle,
        query: &str,
        top_k: usize,
        document_filter: Option<Uuid>,
    ) -> Result<Vec<SearchResult>> {
        let start = std::time::Instant::now();

        // Stage 1: BM25 recall
        let bm25_candidates = self.bm25_recall(case, query, 500, document_filter)?;

        if bm25_candidates.is_empty() {
            return Ok(vec![]);
        }

        // Stage 2: Semantic ranking
        let query_e1 = self.embedder.embed_query(query, EmbedderId::E1Legal)?;
        let query_e6 = self.embedder.embed_query(query, EmbedderId::E6Legal)?;
        let query_e7 = self.embedder.embed_query(query, EmbedderId::E7)?;

        let mut scored: Vec<(Uuid, f32)> = bm25_candidates
            .iter()
            .map(|chunk_id| {
                let e1_score = self.score_dense(case, "e1", chunk_id, &query_e1)?;
                let e6_score = self.score_sparse(case, "e6", chunk_id, &query_e6)?;
                let e7_score = self.score_dense(case, "e7", chunk_id, &query_e7)?;

                let rrf = rrf_fusion(&[
                    (e1_score, 1.0),   // E1: weight 1.0
                    (e6_score, 0.8),   // E6: weight 0.8
                    (e7_score, 0.6),   // E7: weight 0.6
                ]);

                Ok((*chunk_id, rrf))
            })
            .collect::<Result<Vec<_>>>()?;

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(100);

        // Stage 3: Multi-signal boost (Pro only)
        if self.tier.is_pro() {
            scored = self.multi_signal_boost(case, query, scored)?;
            scored.truncate(50);
        }

        // Stage 4: ColBERT rerank (Pro only)
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

## 3. BM25 Implementation

```rust
pub struct Bm25Index;

impl Bm25Index {
    /// Search the inverted index
    pub fn search(
        case: &CaseHandle,
        query: &str,
        limit: usize,
        document_filter: Option<Uuid>,
    ) -> Result<Vec<Uuid>> {
        let cf = case.db.cf_handle("bm25_index").unwrap();

        // Load stats
        let stats: Bm25Stats = {
            let bytes = case.db.get_cf(&cf, b"stats")?
                .ok_or(CaseTrackError::Bm25IndexEmpty)?;
            bincode::deserialize(&bytes)?
        };

        // Tokenize query
        let terms = tokenize_for_bm25(query);

        // Accumulate scores per chunk
        let mut scores: HashMap<Uuid, f32> = HashMap::new();

        for term in &terms {
            let key = format!("term:{}", term);
            if let Some(bytes) = case.db.get_cf(&cf, key.as_bytes())? {
                let postings: PostingList = bincode::deserialize(&bytes)?;

                let idf = ((stats.total_docs as f32 - postings.doc_freq as f32 + 0.5)
                    / (postings.doc_freq as f32 + 0.5) + 1.0).ln();

                for posting in &postings.entries {
                    // Apply document filter if specified
                    if let Some(filter_doc) = document_filter {
                        if posting.document_id != filter_doc {
                            continue;
                        }
                    }

                    let tf = posting.term_freq as f32;
                    let dl = posting.doc_length as f32;
                    let avgdl = stats.avg_doc_length;

                    // BM25 formula
                    let k1 = 1.2;
                    let b = 0.75;
                    let score = idf * (tf * (k1 + 1.0))
                        / (tf + k1 * (1.0 - b + b * dl / avgdl));

                    *scores.entry(posting.chunk_id).or_default() += score;
                }
            }
        }

        // Sort by score, return top N
        let mut results: Vec<(Uuid, f32)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(limit);

        Ok(results.into_iter().map(|(id, _)| id).collect())
    }

    /// Index a chunk's text into the inverted index
    pub fn index_chunk(
        case: &CaseHandle,
        chunk: &Chunk,
    ) -> Result<()> {
        let cf = case.db.cf_handle("bm25_index").unwrap();
        let terms = tokenize_for_bm25(&chunk.text);
        let term_freqs = count_term_frequencies(&terms);

        for (term, freq) in &term_freqs {
            let key = format!("term:{}", term);

            let mut postings: PostingList = case.db.get_cf(&cf, key.as_bytes())?
                .map(|b| bincode::deserialize(&b).unwrap_or_default())
                .unwrap_or_default();

            postings.doc_freq += 1;
            postings.entries.push(PostingEntry {
                chunk_id: chunk.id,
                document_id: chunk.document_id,
                term_freq: *freq,
                doc_length: terms.len() as u32,
            });

            case.db.put_cf(&cf, key.as_bytes(), bincode::serialize(&postings)?)?;
        }

        // Update global stats
        let mut stats: Bm25Stats = case.db.get_cf(&cf, b"stats")?
            .map(|b| bincode::deserialize(&b).unwrap_or_default())
            .unwrap_or_default();

        stats.total_docs += 1;
        stats.total_tokens += terms.len() as u64;
        stats.avg_doc_length = stats.total_tokens as f32 / stats.total_docs as f32;

        case.db.put_cf(&cf, b"stats", bincode::serialize(&stats)?)?;

        Ok(())
    }
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

/// Simple tokenization for BM25 (lowercased, alphanumeric, stopwords removed)
fn tokenize_for_bm25(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 1 && !is_stopword(w))
        .collect()
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

/// Alternative: weighted combination for Stage 3 multi-signal boost
pub fn weighted_combination(
    base_score: f32,
    signals: &[(f32, f32)],  // (score, weight) pairs
) -> f32 {
    let total_weight: f32 = signals.iter().map(|(_, w)| w).sum();
    let weighted_sum: f32 = signals.iter().map(|(s, w)| s * w).sum();
    base_score * 0.6 + (weighted_sum / total_weight) * 0.4
}
```

---

## 5. ColBERT Reranking (Stage 4)

```rust
impl SearchEngine {
    fn colbert_rerank(
        &self,
        case: &CaseHandle,
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
    pub vectors: Vec<Vec<f32>>,  // One 64D vector per token
    pub token_count: usize,
}
```

---

## 6. Similarity Functions

```rust
/// Cosine similarity between two dense vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Dot product for sparse vectors (SPLADE)
pub fn sparse_dot(a: &SparseVec, b: &SparseVec) -> f32 {
    a.dot(b)  // Implementation in SparseVec struct
}
```

---

## 7. Document-Filtered Search

Search can be restricted to a single document:

```rust
/// Search within a specific document only
pub fn search_document(
    &self,
    case: &CaseHandle,
    query: &str,
    document_id: Uuid,
    top_k: usize,
) -> Result<Vec<SearchResult>> {
    self.search(case, query, top_k, Some(document_id))
}
```

This is useful for queries like:
- "What does the contract say about non-compete?"
- "Find all mentions of damages in the complaint"

---

## 8. Search Response Format

The MCP tool returns results in this structure:

```json
{
  "query": "early termination clause",
  "case": "Smith v. Jones Corp",
  "results_count": 5,
  "search_time_ms": 87,
  "tier": "pro",
  "stages_used": ["bm25", "semantic", "multi_signal", "colbert"],
  "results": [
    {
      "text": "Either party may terminate this Agreement upon thirty (30) days written notice...",
      "score": 0.94,
      "citation": "Contract.pdf, p. 12, para. 8",
      "citation_short": "Contract, p. 12",
      "source": {
        "document": "Contract.pdf",
        "document_path": "/Users/sarah/Documents/CaseTrack/cases/smith-v-jones/originals/Contract.pdf",
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
        "bates": null,
        "extraction_method": "Native",
        "ocr_confidence": null
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

*CaseTrack PRD v4.0.0 -- Document 8 of 10*


---

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


---

# PRD 10: Technical Build Guide

**Version**: 4.0.0 | **Parent**: [PRD 01 Overview](PRD_01_OVERVIEW.md) | **Language**: Rust

---

> **LANGUAGE: RUST** -- This entire project is built in Rust. Every crate, every
> module, every line of product code is Rust. Use `cargo` for building, testing,
> and releasing. All dependencies are Rust crates from crates.io. The final
> deliverable is a single statically-linked Rust binary per platform. No runtime
> dependencies (no Node.js, no Python, no JVM, no .NET). The only non-Rust code
> in the repository is `scripts/convert_models.py`, a one-time build tool for
> converting PyTorch models to ONNX format (not shipped to users).

---

## 1. Project Bootstrap

### 1.1 Create Fresh Project

```bash
# Create new project directory
mkdir casetrack && cd casetrack

# Initialize workspace
cargo init --name casetrack
mkdir -p crates/casetrack-core

# Initialize core library crate
cd crates/casetrack-core
cargo init --lib --name casetrack-core
cd ../..

# Initialize git
git init
echo "target/" > .gitignore
echo "*.onnx" >> .gitignore
echo "models/" >> .gitignore
```

### 1.2 Workspace Structure

```
casetrack/
|-- Cargo.toml                   # Workspace root
|-- Cargo.lock
|-- .github/
|   +-- workflows/
|       |-- ci.yml               # CI pipeline
|       +-- release.yml          # Release builds
|-- scripts/
|   |-- convert_models.py        # PyTorch -> ONNX conversion
|   |-- build_mcpb.sh           # Build MCPB bundle
|   +-- install.sh              # macOS/Linux installer
|-- crates/
|   |-- casetrack/               # Binary crate (MCP server entry point)
|   |   |-- Cargo.toml
|   |   +-- src/
|   |       |-- main.rs          # Entry point, CLI parsing, server start
|   |       |-- cli.rs           # CLI argument definitions (clap)
|   |       |-- server.rs        # MCP server setup + tool registration
|   |       +-- format.rs        # Output formatting for MCP responses
|   |
|   +-- casetrack-core/          # Library crate (all business logic)
|       |-- Cargo.toml
|       +-- src/
|           |-- lib.rs           # Public API re-exports
|           |-- error.rs         # Error types
|           |-- config.rs        # Configuration
|           |-- case/
|           |   |-- mod.rs
|           |   |-- registry.rs  # CaseRegistry (manages all cases)
|           |   |-- handle.rs    # CaseHandle (open case database)
|           |   +-- model.rs     # Case, CaseType, CaseStatus structs
|           |-- document/
|           |   |-- mod.rs
|           |   |-- pdf.rs       # PDF text extraction
|           |   |-- docx.rs      # DOCX parsing
|           |   |-- ocr.rs       # Tesseract OCR
|           |   |-- chunker.rs   # Legal-aware text chunking
|           |   +-- model.rs     # Page, Paragraph, Chunk structs
|           |-- embedding/
|           |   |-- mod.rs
|           |   |-- engine.rs    # EmbeddingEngine (ONNX inference)
|           |   |-- models.rs    # Model specs, download config
|           |   |-- download.rs  # Model download from Hugging Face
|           |   +-- types.rs     # ChunkEmbeddings, SparseVec, TokenEmbeddings
|           |-- search/
|           |   |-- mod.rs
|           |   |-- engine.rs    # SearchEngine (4-stage pipeline)
|           |   |-- bm25.rs      # BM25 inverted index
|           |   |-- ranking.rs   # RRF, cosine similarity, ColBERT MaxSim
|           |   +-- result.rs    # SearchResult struct
|           |-- provenance/
|           |   |-- mod.rs
|           |   +-- citation.rs  # Provenance struct, cite() formatting
|           |-- storage/
|           |   |-- mod.rs
|           |   |-- rocks.rs     # RocksDB configuration and helpers
|           |   +-- schema.rs    # Column families, key formats, migration
|           +-- license/
|               |-- mod.rs
|               +-- validator.rs # License key validation (ed25519)
|
|-- tests/
|   |-- integration/
|   |   |-- test_case_lifecycle.rs
|   |   |-- test_ingest_pdf.rs
|   |   |-- test_search.rs
|   |   +-- test_mcp_tools.rs
|   +-- fixtures/
|       |-- sample.pdf           # 3-page test PDF
|       |-- sample.docx          # Test Word document
|       +-- scanned.png          # Test scanned image
|
+-- docs/
    +-- prd/                     # These PRD documents
```

### 1.3 Workspace Cargo.toml

```toml
[workspace]
members = [
    "crates/casetrack",
    "crates/casetrack-core",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "LicenseRef-Commercial"
repository = "https://github.com/casetrack-legal/casetrack"

[workspace.dependencies]
# MCP server
rmcp = { version = "0.13", features = ["server", "transport-io", "macros"] }

# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Storage
rocksdb = "0.22"

# ML inference
ort = { version = "2.0", features = ["download-binaries"] }

# PDF processing
pdf-extract = "0.7"
lopdf = "0.32"

# DOCX processing
docx-rs = "0.4"

# Image processing
image = "0.25"

# OCR
tesseract = { version = "0.14", optional = true }

# Model download
hf-hub = "0.3"
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Identifiers
uuid = { version = "1.6", features = ["v4", "serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# CLI
clap = { version = "4.4", features = ["derive"] }

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Cryptography (license validation)
ed25519-dalek = "2.1"
base64 = "0.22"

# Byte casting (zero-copy embedding reads)
bytemuck = { version = "1.14", features = ["derive"] }

# Hashing (duplicate detection)
sha2 = "0.10"

# System info (memory detection)
sysinfo = "0.30"

# File walking (batch ingest)
walkdir = "2.4"

# Semver (update checking)
semver = "1.0"

# Directories (platform-specific paths)
dirs = "5.0"
```

### 1.4 Binary Crate Cargo.toml

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

### 1.5 Core Library Cargo.toml

```toml
[package]
name = "casetrack-core"
version.workspace = true
edition.workspace = true

[dependencies]
# Storage
rocksdb.workspace = true

# ML
ort.workspace = true

# Document processing
pdf-extract.workspace = true
lopdf.workspace = true
docx-rs.workspace = true
image.workspace = true

# OCR (optional)
tesseract = { workspace = true, optional = true }

# Model download
hf-hub.workspace = true
reqwest.workspace = true

# Serialization
serde.workspace = true
serde_json.workspace = true
bincode.workspace = true

# Identifiers + time
uuid.workspace = true
chrono.workspace = true

# Error handling
thiserror.workspace = true
anyhow.workspace = true

# Logging
tracing.workspace = true

# Crypto
ed25519-dalek.workspace = true
base64.workspace = true

# Utilities
bytemuck.workspace = true
sha2.workspace = true
sysinfo.workspace = true
walkdir.workspace = true
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

    // Handle non-server commands
    match &args.command {
        Some(cli::Command::SetupClaudeCode) => {
            return casetrack_core::setup_claude_code(&args.data_dir());
        }
        Some(cli::Command::Update) => {
            return casetrack_core::self_update().await;
        }
        Some(cli::Command::Uninstall) => {
            return casetrack_core::uninstall();
        }
        None => {} // Default: run MCP server
    }

    // Initialize logging (to stderr, since stdout is MCP transport)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("casetrack=info"))
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("CaseTrack v{} starting...", env!("CARGO_PKG_VERSION"));

    // Start MCP server
    server::CaseTrackServer::start(casetrack_core::Config {
        data_dir: args.data_dir(),
        license_key: args.license_key(),
    })
    .await
}
```

---

## 3. CLI Arguments

```rust
// crates/casetrack/src/cli.rs

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "casetrack")]
#[command(about = "Legal document analysis MCP server for Claude")]
#[command(version)]
pub struct Args {
    /// Data directory (models, cases, config)
    #[arg(long, env = "CASETRACK_HOME")]
    pub data_dir: Option<PathBuf>,

    /// License key
    #[arg(long, env = "CASETRACK_LICENSE")]
    pub license: Option<String>,

    /// Memory mode override
    #[arg(long, value_enum)]
    pub memory_mode: Option<MemoryMode>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Configure Claude Code to use CaseTrack
    SetupClaudeCode,

    /// Update CaseTrack to the latest version
    Update,

    /// Uninstall CaseTrack
    Uninstall,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum MemoryMode {
    Full,
    Standard,
    Constrained,
}

impl Args {
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(|| {
            dirs::document_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap().join("Documents"))
                .join("CaseTrack")
        })
    }

    pub fn license_key(&self) -> Option<String> {
        self.license.clone()
    }
}
```

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
    #[error("Free tier limit: {resource} ({current}/{max}). Upgrade: https://casetrack.legal/upgrade")]
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
5. **Internal errors include report URL**: "Please report this at github.com/..."

---

## 5. Configuration

```rust
// crates/casetrack-core/src/config.rs

use std::path::PathBuf;

/// Server configuration (from CLI args + env vars + optional config file)
pub struct Config {
    pub data_dir: PathBuf,
    pub license_key: Option<String>,
}

/// Optional config file (~/Documents/CaseTrack/config.toml)
/// NOT required -- zero-config is a design principle
#[derive(serde::Deserialize, Default)]
pub struct ConfigFile {
    /// Override default data directory
    pub data_dir: Option<PathBuf>,

    /// License key
    pub license_key: Option<String>,

    /// OCR language (default: "eng")
    pub ocr_language: Option<String>,

    /// Copy originals to case folder on ingest
    pub copy_originals: Option<bool>,

    /// Memory mode override
    pub memory_mode: Option<String>,

    /// Max threads for embedding inference
    pub inference_threads: Option<u32>,
}
```

---

## 6. Logging & Diagnostics

```rust
// Logging goes to STDERR (stdout is MCP transport)
// Controlled by RUST_LOG env var or --log-level CLI arg

// Log levels:
// ERROR: Failures that prevent operations (file not found, DB corruption)
// WARN:  Degraded functionality (low memory, OCR disabled, slow inference)
// INFO:  Normal operations (server started, case created, search completed)
// DEBUG: Internal details (model loading times, RocksDB stats)
// TRACE: Verbose (individual chunk embeddings, token counts)

// Examples of what gets logged:
tracing::info!("CaseTrack v{} starting...", version);
tracing::info!("License tier: {:?}", tier);
tracing::info!("Models loaded: {} ({} MB RAM)", count, ram_mb);
tracing::warn!("Low memory mode active ({} MB available)", available);
tracing::info!("Ingested {} ({} pages, {} chunks, {}ms)", filename, pages, chunks, ms);
tracing::info!("Search: {} results in {}ms (query: '{}')", results, ms, query);
tracing::error!("Failed to ingest {}: {}", filename, error);
tracing::debug!("E1 inference: {}ms for {} chunks", ms, count);
```

---

## 7. Cross-Platform Concerns

### 7.1 Path Handling

```rust
/// Resolve user-provided paths across platforms
pub fn resolve_path(input: &str) -> PathBuf {
    let expanded = if input.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(&input[2..])  // Skip "~/"
        } else {
            PathBuf::from(input)
        }
    } else {
        PathBuf::from(input)
    };

    // Normalize path separators
    // On Windows: handle both / and \
    // On Unix: just canonicalize
    expanded
}
```

### 7.2 Default Data Directory

| Platform | Default Path |
|----------|-------------|
| macOS | `~/Documents/CaseTrack/` |
| Windows | `C:\Users\{user}\Documents\CaseTrack\` |
| Linux | `~/Documents/CaseTrack/` (or `~/.local/share/casetrack/` if no Documents) |

### 7.3 RocksDB Platform Notes

- **macOS**: Works out of the box via `rust-rocksdb`
- **Windows**: Requires MSVC build tools. `rocksdb` crate handles compilation.
- **Linux**: Statically link to avoid `librocksdb.so` dependency

### 7.4 Tesseract Bundling

| Platform | Strategy |
|----------|----------|
| macOS | Static link via `tesseract-sys` with vendored feature |
| Windows | Bundle `tesseract.dll` + `leptonica.dll` in installer |
| Linux | Static link via musl build OR require system package |

### 7.5 ONNX Runtime

| Platform | Execution Providers |
|----------|-------------------|
| macOS | CoreML (hardware accel) + CPU fallback |
| Windows | DirectML (GPU) + CPU fallback |
| Linux | CPU only (CUDA optional via feature flag) |

---

## 8. Security

### 8.1 Input Validation

```rust
/// Validate file paths to prevent path traversal
pub fn validate_file_path(path: &Path, data_dir: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()
        .map_err(|_| CaseTrackError::FileNotFound(path.to_path_buf()))?;

    // For reads: allow any path the user provides (they own their machine)
    // For writes: only within data_dir
    Ok(canonical)
}

/// Validate that a write path is within the data directory
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

- Keys are ed25519 signed: can be validated offline
- Public key embedded in binary (cannot be extracted to forge keys)
- Key format: `TIER-XXXXXX-XXXXXX-XXXXXX-SIG` (human-readable prefix + signature)
- Cached validation avoids repeated network calls
- No user data in license validation requests

### 8.3 No Network After Setup

After initial model download and license activation, CaseTrack makes ZERO network requests:
- Document processing: 100% local
- Search: 100% local
- Storage: 100% local
- Update checks: optional, non-blocking, can be disabled

---

## 9. Testing Strategy

### 9.1 Unit Tests

Every module has unit tests. Key areas:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Chunking (2000-char target, 200-char overlap)
    #[test]
    fn test_chunk_respects_paragraph_boundaries() { ... }

    #[test]
    fn test_chunk_target_2000_chars() { ... }

    #[test]
    fn test_chunk_overlap_200_chars() { ... }

    #[test]
    fn test_chunk_min_400_chars() { ... }

    #[test]
    fn test_chunk_max_2200_chars() { ... }

    #[test]
    fn test_chunk_provenance_complete() { ... }  // Every chunk has file path, page, paragraph, line, char offsets

    // BM25
    #[test]
    fn test_bm25_basic_search() { ... }

    #[test]
    fn test_bm25_term_frequency() { ... }

    // Provenance (must include: file path, document name, page, paragraph, line, char offsets)
    #[test]
    fn test_citation_format() { ... }

    #[test]
    fn test_short_citation() { ... }

    #[test]
    fn test_provenance_includes_file_path() { ... }

    #[test]
    fn test_provenance_includes_char_offsets() { ... }

    #[test]
    fn test_provenance_round_trip() { ... }  // Store + retrieve preserves all fields

    // RRF
    #[test]
    fn test_rrf_fusion() { ... }

    // Cosine similarity
    #[test]
    fn test_cosine_identical_vectors() { ... }

    #[test]
    fn test_cosine_orthogonal_vectors() { ... }

    // License
    #[test]
    fn test_free_tier_limits() { ... }

    #[test]
    fn test_valid_license_key() { ... }
}
```

### 9.2 Integration Tests

```rust
// tests/integration/test_case_lifecycle.rs

#[tokio::test]
async fn test_create_list_switch_delete_case() {
    let dir = tempdir().unwrap();
    let mut registry = CaseRegistry::open(dir.path()).unwrap();

    // Create
    let case = registry.create_case(CreateCaseParams {
        name: "Test Case".to_string(),
        case_number: None,
        case_type: Some(CaseType::Contract),
    }).unwrap();

    assert_eq!(case.name, "Test Case");
    assert_eq!(case.case_type, CaseType::Contract);

    // List
    let cases = registry.list_cases().unwrap();
    assert_eq!(cases.len(), 1);

    // Switch
    let handle = registry.switch_case(case.id).unwrap();
    assert_eq!(registry.active_case_id(), Some(case.id));

    // Delete
    drop(handle);
    registry.delete_case(case.id).unwrap();
    assert_eq!(registry.list_cases().unwrap().len(), 0);
}
```

```rust
// tests/integration/test_ingest_pdf.rs

#[tokio::test]
async fn test_ingest_sample_pdf() {
    let dir = tempdir().unwrap();
    let mut registry = CaseRegistry::open(dir.path()).unwrap();
    let case = registry.create_case(/* ... */).unwrap();
    let handle = registry.switch_case(case.id).unwrap();

    // Ingest test PDF
    let result = ingest_document(
        &handle,
        &engine,
        Path::new("tests/fixtures/sample.pdf"),
        None,
    ).await.unwrap();

    assert_eq!(result.page_count, 3);
    assert!(result.chunk_count > 0);

    // Verify chunks stored
    let docs = handle.list_documents().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].page_count, 3);
}
```

```rust
// tests/integration/test_search.rs

#[tokio::test]
async fn test_search_returns_relevant_results() {
    // Setup: create case, ingest sample PDF with known content
    // ...

    let results = search_engine.search(
        &case_handle,
        "termination clause",
        10,
        None,
    ).unwrap();

    assert!(!results.is_empty());
    assert!(results[0].score > 0.5);
    assert!(results[0].citation.contains("sample.pdf"));
    assert!(results[0].provenance.page > 0);

    // Verify full provenance on every result
    for result in &results {
        assert!(!result.provenance.document_name.is_empty());
        assert!(!result.provenance.document_path.is_empty());
        assert!(result.provenance.page > 0);
        assert!(result.provenance.char_start < result.provenance.char_end);
    }
}

#[tokio::test]
async fn test_case_isolation() {
    // Verify chunks from one case never appear in another case's search
    let dir = tempdir().unwrap();
    let mut registry = CaseRegistry::open(dir.path()).unwrap();

    let case_a = registry.create_case(CreateCaseParams {
        name: "Case A".to_string(), ..Default::default()
    }).unwrap();
    let case_b = registry.create_case(CreateCaseParams {
        name: "Case B".to_string(), ..Default::default()
    }).unwrap();

    // Ingest into Case A only
    let handle_a = registry.switch_case(case_a.id).unwrap();
    ingest_document(&handle_a, &engine, Path::new("tests/fixtures/sample.pdf"), None).await.unwrap();
    drop(handle_a);

    // Search Case B -- must return zero results
    let handle_b = registry.switch_case(case_b.id).unwrap();
    let results = search_engine.search(&handle_b, "termination clause", 10, None).unwrap();
    assert!(results.is_empty(), "Case B must not contain Case A documents");
}
```

### 9.3 Test Fixtures

The `tests/fixtures/` directory contains:
- `sample.pdf` -- 3-page PDF with known text content about contract terms
- `sample.docx` -- Word document with headings, paragraphs, lists
- `scanned.png` -- Image of typed text for OCR testing
- `empty.pdf` -- Edge case: empty PDF
- `large_paragraph.txt` -- Edge case: single paragraph >2000 characters

### 9.4 Running Tests

```bash
# All tests
cargo test

# Unit tests only (fast, no fixtures needed)
cargo test --lib

# Integration tests (needs fixtures)
cargo test --test '*'

# With logging
RUST_LOG=debug cargo test -- --nocapture

# Specific test
cargo test test_bm25_basic_search
```

---

## 10. CI/CD Pipeline

### 10.1 GitHub Actions CI

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Build
        run: cargo build --release

      - name: Test
        run: cargo test

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Format check
        run: cargo fmt -- --check

  # Ensure binary size is reasonable
  size-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - name: Check binary size
        run: |
          SIZE=$(stat -f%z target/release/casetrack 2>/dev/null || stat -c%s target/release/casetrack)
          echo "Binary size: $SIZE bytes ($(($SIZE / 1024 / 1024)) MB)"
          if [ "$SIZE" -gt 52428800 ]; then  # 50MB limit
            echo "ERROR: Binary too large (>50MB)"
            exit 1
          fi
```

### 10.2 Release Pipeline

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: write

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
            name: casetrack-darwin-x64
          - target: aarch64-apple-darwin
            os: macos-latest
            name: casetrack-darwin-arm64
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            name: casetrack-win32-x64.exe
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            name: casetrack-linux-x64

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }}

      - name: Rename binary
        shell: bash
        run: |
          if [ "${{ matrix.os }}" = "windows-latest" ]; then
            cp target/${{ matrix.target }}/release/casetrack.exe ${{ matrix.name }}
          else
            cp target/${{ matrix.target }}/release/casetrack ${{ matrix.name }}
          fi

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.name }}
          path: ${{ matrix.name }}

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - name: Create release
        uses: softprops/action-gh-release@v1
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

      - name: Build MCPB bundle
        run: bash scripts/build_mcpb.sh

      - name: Upload MCPB to release
        uses: softprops/action-gh-release@v1
        with:
          files: casetrack.mcpb
```

---

## 11. Monetization Implementation

### 11.1 Pricing Tiers

| Tier | Price | Cases | Docs/Case | Embedders | Key Features |
|------|-------|-------|-----------|-----------|-------------|
| Free | $0 | 3 | 100 | 4 (E1,E6,E7,E13) | Basic search, provenance |
| Pro | $29/mo | Unlimited | Unlimited | 7 (all) | ColBERT rerank, entities, batch ingest |
| Firm | $99/mo | Unlimited | Unlimited | 7 (all) | 5 seats, phone support |

### 11.2 License Key System

See [PRD 07](PRD_07_CASE_MANAGEMENT.md) for monetization details. Key implementation points:

- ed25519 signature validation (offline-capable)
- Online activation on first use (one HTTP call to Lemon Squeezy API)
- Cached validation for 30 days
- Graceful degradation: if license check fails, fall back to Free tier (never block the user)

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

CASE MANAGEMENT
  [ ] Implement Case, CaseType, CaseStatus structs
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

CHUNKING (2000-character chunks, 10% overlap -- see PRD 06)
  [ ] Implement LegalChunker (2000-char target, 200-char overlap, paragraph-aware)
  [ ] Character counting (not token-based)
  [ ] Long paragraph splitting (>2200 chars)
  [ ] Provenance attachment per chunk (file path, document name, page, paragraph, line, char offsets)
  [ ] Chunk boundary validation (min 400 chars, max 2200 chars)

STORAGE (Per-case isolated databases -- see PRD 04)
  [ ] Store chunks in RocksDB (one DB per case)
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
  [ ] E1-LEGAL (bge-small dense embedding)
  [ ] E7 (MiniLM dense embedding)
  [ ] E6-LEGAL (SPLADE sparse embedding)
  [ ] Batch embedding for ingestion
  [ ] Store embeddings in RocksDB

BM25 INDEX
  [ ] Tokenization (lowercase, stopword removal)
  [ ] Inverted index (posting lists in RocksDB)
  [ ] BM25 scoring formula
  [ ] Index update during ingestion

SEARCH ENGINE
  [ ] Stage 1: BM25 recall
  [ ] Stage 2: Semantic ranking (E1 + E6 + E7 via RRF)
  [ ] Cosine similarity, sparse dot product
  [ ] search_case MCP tool
  [ ] Result formatting with citations
```

### Phase 4: Pro Features

```
PRO EMBEDDERS
  [ ] E8-LEGAL (citation embedding)
  [ ] E11-LEGAL (entity embedding)
  [ ] E12 (ColBERT token-level)
  [ ] Stage 3: Multi-signal boost
  [ ] Stage 4: ColBERT rerank (MaxSim)
  [ ] find_entity MCP tool

LICENSE SYSTEM
  [ ] ed25519 key validation
  [ ] Online activation (Lemon Squeezy)
  [ ] Offline cache (30-day)
  [ ] Feature gating per tier
  [ ] Upgrade prompts in error messages

BATCH OPERATIONS
  [ ] ingest_folder tool (directory walking)
  [ ] Progress reporting
  [ ] Error collection for batch
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
| Firm conversions | 20 | 100 | 400 |
| ARR | $58K | $318K | $1.37M |

---

## Appendix A: File Size Estimates

| Component | Size |
|-----------|------|
| Binary (release, stripped) | ~15-25MB |
| MCPB bundle (all platforms) | ~50MB |
| Models (Free tier) | ~165MB |
| Models (Pro tier) | ~370MB |
| Case database (per 100 docs) | ~5-50MB |
| Total install (Free, 1 case) | ~200MB |
| Total install (Pro, 10 cases) | ~900MB |

## Appendix B: Comparison with Alternatives

| Feature | CaseTrack | Casetext | Westlaw | DIY RAG |
|---------|-----------|----------|---------|---------|
| Price | $0-29/mo | $200/mo | $400/mo | Free |
| Install time | 2 min | N/A (SaaS) | N/A (SaaS) | Hours |
| Runs locally | Yes | No | No | Yes |
| No GPU required | Yes | N/A | N/A | Usually no |
| Claude integration | Native MCP | No | No | Manual |
| Provenance | Always | Sometimes | Sometimes | DIY |
| Legal-specific models | Yes | Yes | Yes | No |
| Privacy | 100% local | Cloud | Cloud | Local |
| Offline capable | Yes | No | No | Yes |

---

*CaseTrack PRD v4.0.0 -- Document 10 of 10*

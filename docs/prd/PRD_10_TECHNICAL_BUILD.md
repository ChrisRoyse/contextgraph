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

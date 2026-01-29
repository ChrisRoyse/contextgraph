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

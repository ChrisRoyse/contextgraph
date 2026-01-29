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

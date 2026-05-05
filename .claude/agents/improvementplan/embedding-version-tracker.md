---
name: embedding-version-tracker
description: |
  Implements embedding model version tracking per fingerprint via CF_EMBEDDING_REGISTRY.
  Records which embedder model versions generated each set of vectors, enabling stale
  embedding detection and targeted re-embedding. Use for Phase 6 item 5.15 of the
  provenance improvement plan. Keywords: embedding version, model tracking, re-embedding,
  stale embeddings, E7 qodo, fingerprint.
tools: Read,Write,Edit,Glob,Grep,Bash
model: sonnet
---

You are an expert Rust developer implementing embedding model version tracking for the Context Graph system. You are implementing Phase 6 item 5.15 of the Provenance Improvement Plan.

## Your Mission

When embeddings are computed for a fingerprint, no record exists of which model versions generated them. If models are updated, there's no way to find stale embeddings. Fix this.

## What You Must Build

### 1. EmbeddingVersionRecord

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingVersionRecord {
    pub fingerprint_id: Uuid,
    pub computed_at: DateTime<Utc>,
    pub embedder_versions: HashMap<String, String>,
    pub e7_model_version: Option<String>,
    pub computation_time_ms: Option<u64>,
}
```

### 2. CF_EMBEDDING_REGISTRY Column Family

Add to `crates/context-graph-storage/src/teleological/column_families.rs`.
- Key: `fingerprint_id` (Uuid as bytes)
- Value: Serialized `EmbeddingVersionRecord`
- Permanent retention

### 3. Storage Methods

In the storage module:
- `store_embedding_version(record: &EmbeddingVersionRecord) -> Result<()>`
- `get_embedding_version(fingerprint_id: Uuid) -> Result<Option<EmbeddingVersionRecord>>`
- `find_stale_embeddings(current_versions: &HashMap<String, String>) -> Result<Vec<Uuid>>`
  - Compares stored versions against current versions to find fingerprints needing re-embedding

### 4. Record During store_memory

In `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`:
- After embedding computation, create an EmbeddingVersionRecord
- Store in CF_EMBEDDING_REGISTRY
- Capture the current embedder model versions from the system configuration

### 5. Query Stale Embeddings

Consider adding a helper or MCP tool endpoint that can scan the registry and report which fingerprints have outdated embeddings compared to current model versions.

## Process

1. Read column_families.rs for CF registration pattern
2. Read the memory_tools handler to find where embeddings are computed
3. Read the TeleologicalFingerprint type to understand existing embedding storage
4. Identify where model version strings are available in the codebase
5. Create EmbeddingVersionRecord type
6. Add CF_EMBEDDING_REGISTRY
7. Create storage methods
8. Wire recording into store_memory flow
9. Run `cargo check` on affected crates

## Critical Rules

- `embedder_versions` map uses embedder names as keys (e.g., "E1" -> "all-MiniLM-L6-v2.1")
- Record should be written AFTER successful embedding computation, not before
- The `find_stale_embeddings` scan should be efficient (iterate CF with prefix scan)
- Do NOT re-embed anything - only track versions. Re-embedding is a separate concern
- All fields backward-compatible (Option where appropriate)
- HashMap<String, String> for embedder_versions allows flexible embedder sets

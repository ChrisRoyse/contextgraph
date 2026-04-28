# Storage Layer

**Source files covered:**
- `crates/context-graph-storage/src/**/*.rs`
- `crates/context-graph-core/src/traits/**/*.rs`

## 1. Scope
The storage crate persists base memory records, teleological fingerprints, quantized embedder records, code entities, causal relationships, training exports, learning events, learner state, constellations, contrastive pairs, and typed-edge records.

## 2. RocksDB Options
| Option family | Behavior |
| --- | --- |
| nodes_options | Bloom filter, shared cache, LZ4, point lookup optimization, 4MB write buffer |
| edges_options | Bloom filter, shared cache, LZ4, 16-byte prefix extractor, 4MB write buffer |
| embeddings_options | 64KB blocks, shared cache, LZ4, 8MB write buffer |
| index_options | Bloom filter, shared cache, LZ4, 16-byte prefix extractor, 2MB write buffer |
| system_options | small rare metadata, no compression |

## 3. Groups
See [04_database_schema.md](04_database_schema.md) for all 73 CFs.

## 4. Repository Areas
content/source metadata, search, export/training, constellation, contrastive, learner, code, graph_edges, and causal relationship modules are under `crates/context-graph-storage/src`.

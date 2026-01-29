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

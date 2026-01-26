# Legal Document Analysis System: Complete Research Report

## Multi-Embedder Knowledge Graph Architecture for Billion-Scale Legal Corpus

**Version**: 2.0.0
**Date**: 2026-01-25
**Scale Target**: 1+ Billion Legal Documents
**Purpose**: Comprehensive architecture for legal document embedding, linking, and insight extraction

---

## Table of Contents

### Part I: Foundation
1. [Executive Summary](#1-executive-summary)
2. [Legal Domain Challenges](#2-legal-domain-challenges)
3. [Scale Requirements Analysis](#3-scale-requirements-analysis)

### Part II: Legal Embedding Models
4. [Legal Embedding Models Survey](#4-legal-embedding-models-survey)
5. [Long Document Models](#5-long-document-models)
6. [Legal Named Entity Recognition](#6-legal-named-entity-recognition)

### Part III: Graph Linking for Legal Documents
7. [Legal Citation Network Architecture](#7-legal-citation-network-architecture)
8. [K-NN Graph Strategies for Legal Corpus](#8-k-nn-graph-strategies-for-legal-corpus)
9. [Multi-Relation Legal Edge Types](#9-multi-relation-legal-edge-types)
10. [Directed Edges for Legal Reasoning](#10-directed-edges-for-legal-reasoning)

### Part IV: Graph Neural Networks for Legal Analysis
11. [GNN Approaches for Legal Knowledge Graphs](#11-gnn-approaches-for-legal-knowledge-graphs)
12. [Hyperbolic Embeddings for Legal Hierarchy](#12-hyperbolic-embeddings-for-legal-hierarchy)
13. [Multi-View Learning for Legal Documents](#13-multi-view-learning-for-legal-documents)

### Part V: Billion-Scale Architecture
14. [Distributed Architecture for 1B+ Documents](#14-distributed-architecture-for-1b-documents)
15. [Legal-Specific Retrieval Pipeline](#15-legal-specific-retrieval-pipeline)
16. [Insight Extraction System](#16-insight-extraction-system)

### Part VI: Implementation
17. [Recommended Legal Embedder Stack (15+ Embedders)](#17-recommended-legal-embedder-stack)
18. [Implementation Roadmap](#18-implementation-roadmap)
19. [References](#19-references)

---

# Part I: Foundation

## 1. Executive Summary

### The Vision

Build the world's most comprehensive legal document analysis system capable of:
- **Storing and indexing 1+ billion legal documents** (cases, statutes, contracts, regulations)
- **Multi-perspective semantic search** using 15+ specialized embedders
- **Graph-based reasoning** enabling multi-hop precedent discovery
- **Insight extraction** that surfaces patterns no single embedder can find

### Scale Context

| Metric | Value | Implication |
|--------|-------|-------------|
| **Document Count** | 1+ billion | Distributed architecture required |
| **Average Doc Size** | 5,000-50,000 tokens | Long document handling critical |
| **Chunks (est.)** | 10-50 billion | Massive index infrastructure |
| **Citation Links** | 50+ billion | Graph becomes primary structure |
| **Storage (embeddings)** | 500TB - 2PB | Cost optimization essential |
| **Query Latency Target** | <500ms P95 | Pre-computed graphs required |

### Top Findings

| Finding | Impact |
|---------|--------|
| **voyage-law-2** outperforms generic by 6-10% | Use as E1-LEGAL foundation |
| **Citation GNNs** improve link prediction 8.5 points | Essential for precedent discovery |
| **Hyperbolic space** captures legal hierarchy 15% better | Critical for statute navigation |
| **SAILER/DELTA** capture legal structure | Add E14 for FIRAC awareness |
| **ContractNLI** shows NLI > cosine for clauses | Consider hybrid similarity |
| **K-NN graphs** enable sub-100ms graph traversal | Pre-compute at scale |
| **Multi-view contrastive learning** learns edge weights without labels | Self-supervised optimization |

### Architecture Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│            BILLION-SCALE LEGAL KNOWLEDGE GRAPH ARCHITECTURE                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    DOCUMENT INGESTION LAYER                          │   │
│  │  Legal Documents → Chunking → 15 Embeddings → Graph Edge Computation │   │
│  │                              ↓                                       │   │
│  │  Throughput: 1M documents/hour (batch), 100 docs/sec (streaming)    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    15-EMBEDDER TELEOLOGICAL ARRAY                    │   │
│  │                                                                      │   │
│  │  FOUNDATION:     E1-LEGAL (voyage-law-2, 1024D)                     │   │
│  │  TEMPORAL:       E2 (recency), E3 (periodic), E4 (sequence)         │   │
│  │  LEGAL SEMANTIC: E5-LEGAL (argument), E6-LEGAL (legal keywords)     │   │
│  │  CODE/STATUTE:   E7-LEGAL (statute structure)                        │   │
│  │  RELATIONAL:     E8-LEGAL (citation network), E11-LEGAL (entities)  │   │
│  │  STRUCTURAL:     E9 (noise-robust), E10 (intent)                     │   │
│  │  PRECISION:      E12 (ColBERT), E13 (SPLADE)                        │   │
│  │  NEW LEGAL:      E14 (SAILER structure), E15 (citation embedding)   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    GRAPH LINKING LAYER                               │   │
│  │                                                                      │   │
│  │  K-NN Graphs (15 per embedder) → Multi-Relation Edges →             │   │
│  │  Citation Network → Hypergraph Topics → Hyperbolic Hierarchy        │   │
│  │                                                                      │   │
│  │  Edge Types: cites, interprets, overrules, distinguishes,           │   │
│  │              semantic_similar, entity_shared, argument_chain        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    GNN REASONING LAYER                               │   │
│  │                                                                      │   │
│  │  R-GCN: Learns relation-specific weights for 15 edge types          │   │
│  │  GAT: Attention over citation network neighbors                      │   │
│  │  Hyperbolic GNN: Navigate statute hierarchy                          │   │
│  │  Contrastive Learning: Self-supervised edge weight optimization     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    RETRIEVAL + INSIGHT LAYER                         │   │
│  │                                                                      │   │
│  │  Multi-Hop Precedent Search → Argument Pattern Mining →             │   │
│  │  Statutory Interpretation Chains → Cross-Jurisdictional Analysis    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Legal Domain Challenges

### 2.1 Why General Embedders Fail on Legal Text

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    GENERAL vs. LEGAL EMBEDDERS                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Query: "What is the standard for summary judgment?"                    │
│                                                                         │
│  GENERAL EMBEDDER (E1 - e5-large-v2):                                   │
│  ├── Matches: "summary", "judgment", "standard"                         │
│  ├── Misses: Fed.R.Civ.P. 56 context                                   │
│  ├── Misses: "no genuine dispute of material fact" doctrine            │
│  └── Returns: Generic summaries about courts                            │
│                                                                         │
│  LEGAL EMBEDDER (voyage-law-2):                                         │
│  ├── Understands: Rule 56 = summary judgment standard                   │
│  ├── Captures: Celotex, Anderson, Matsushita trilogy                   │
│  ├── Links: "material fact" → legal standard of review                 │
│  └── Returns: Relevant caselaw and procedural rules                     │
│                                                                         │
│  WITH GRAPH LINKING:                                                    │
│  ├── Multi-hop: Query → Anderson v. Liberty Lobby → citing cases       │
│  ├── Citation chain: 50+ years of precedent instantly accessible       │
│  ├── Cross-reference: State equivalents of Rule 56                     │
│  └── Returns: Complete precedent landscape                              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Legal Document Characteristics

| Characteristic | Example | Challenge | Graph Solution |
|----------------|---------|-----------|----------------|
| **Latin phrases** | *stare decisis*, *res judicata* | Out-of-vocabulary | Entity linking to legal concepts |
| **Legal citations** | "508 U.S. 366 (1993)" | Citation format parsing | Citation network edges |
| **Hierarchical structure** | Title VII, § 1983, 28 U.S.C. § 1331 | Nested references | Hyperbolic hierarchy embedding |
| **Temporal validity** | "overruled by..." "superseded by..." | Authority changes | Directed temporal edges |
| **Jurisdiction specificity** | UK "duty of care" ≠ US "duty of care" | Same terms, different meanings | Jurisdiction-partitioned graphs |
| **Argument structure** | IRAC method | Structural reasoning | Argument chain edges (E5-LEGAL) |
| **Precedent relationships** | "following", "distinguishing" | Citation treatment | Multi-relation typed edges |

### 2.3 What Lawyers Search For (Use Cases)

| Search Type | Example Query | Required Capability | Graph Feature |
|-------------|---------------|---------------------|---------------|
| **Precedent search** | "Cases where summary judgment denied for employment discrimination" | Citation network + semantic | K-NN + citation edges |
| **Statutory interpretation** | "How have courts interpreted 'reasonable accommodation' under ADA?" | Statute-to-case linking | Hypergraph statute clusters |
| **Fact pattern matching** | "Cases involving slip-and-fall in grocery stores" | Fact extraction + similarity | E14 (SAILER) structure |
| **Argument analysis** | "Strongest arguments against qualified immunity" | Argument mining | E5-LEGAL causal chains |
| **Cross-jurisdictional** | "Compare UK and US approaches to negligent misstatement" | Multi-jurisdiction understanding | Jurisdiction-partitioned K-NN |
| **Temporal tracking** | "How has interpretation of § 230 evolved since 2015?" | Temporal legal reasoning | E2 + directed temporal edges |
| **Citation impact** | "Most influential cases on Fourth Amendment searches" | Citation network centrality | PageRank on citation graph |
| **Dissent mining** | "Find dissents that later became majority opinions" | Opinion type + temporal | Directed edges + time ordering |

---

## 3. Scale Requirements Analysis

### 3.1 Document Volume Breakdown

| Document Type | Estimated Count | Avg Tokens | Total Tokens |
|---------------|-----------------|------------|--------------|
| US Federal Cases | 5M | 8,000 | 40B |
| US State Cases | 50M | 6,000 | 300B |
| EU/UK Cases | 10M | 7,000 | 70B |
| Other Jurisdictions | 35M | 5,000 | 175B |
| Federal Statutes | 500K | 2,000 | 1B |
| State Statutes | 5M | 1,500 | 7.5B |
| Regulations | 10M | 3,000 | 30B |
| Contracts | 500M | 15,000 | 7.5T |
| Legal Commentary | 50M | 4,000 | 200B |
| **TOTAL** | **~700M-1B** | - | **~8T tokens** |

### 3.2 Chunking Strategy for Legal Documents

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    LEGAL DOCUMENT CHUNKING STRATEGY                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  CASE LAW CHUNKING:                                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Document: Brown v. Board of Education, 347 U.S. 483 (1954)      │   │
│  │                                                                  │   │
│  │ Chunk 1: METADATA + PROCEDURAL HISTORY                          │   │
│  │   - Parties, court, date, citation                              │   │
│  │   - How case reached this court                                  │   │
│  │                                                                  │   │
│  │ Chunk 2: FACTS (from SAILER segmentation)                       │   │
│  │   - Key factual allegations                                      │   │
│  │   - Evidence summary                                             │   │
│  │                                                                  │   │
│  │ Chunk 3-N: ISSUES + HOLDINGS                                    │   │
│  │   - Each legal question = separate chunk                        │   │
│  │   - Preserves argument structure                                 │   │
│  │                                                                  │   │
│  │ Chunk N+1: REASONING/ANALYSIS                                   │   │
│  │   - 500-token sliding window with 100-token overlap             │   │
│  │   - Maintains citation context                                   │   │
│  │                                                                  │   │
│  │ Chunk FINAL: DISPOSITION                                        │   │
│  │   - Outcome, remedy, procedural direction                       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  STATUTE CHUNKING:                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Document: 42 U.S.C. § 1983                                      │   │
│  │                                                                  │   │
│  │ Hierarchy-aware chunking:                                       │   │
│  │   Title → Chapter → Section → Subsection → Clause               │   │
│  │                                                                  │   │
│  │ Each level gets:                                                 │   │
│  │   - Own embedding (multi-layered approach)                      │   │
│  │   - Parent-child edges in hyperbolic space                      │   │
│  │   - Sibling edges to related sections                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  CONTRACT CHUNKING:                                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Clause-based chunking:                                          │   │
│  │   - Each clause = chunk                                         │   │
│  │   - Definitions section linked to all usage points              │   │
│  │   - Cross-reference edges within document                       │   │
│  │                                                                  │   │
│  │ Chunk types: Preamble, Definitions, Obligations, Rights,        │   │
│  │              Limitations, Termination, Dispute Resolution       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Storage Estimates

| Component | Per Document | 1B Documents | Notes |
|-----------|--------------|--------------|-------|
| **15 Embeddings** | ~80KB | 80TB | Quantized to int8 = 20TB |
| **K-NN Edges (k=20, 15 embedders)** | ~5KB | 5TB | Sparse storage |
| **Citation Edges** | ~2KB | 2TB | Variable per doc |
| **Typed Edge Metadata** | ~1KB | 1TB | Relation types |
| **Document Metadata** | ~500B | 500GB | JSON compressed |
| **Inverted Indexes (E6, E13)** | ~10KB | 10TB | Term → doc IDs |
| **ColBERT Tokens (E12)** | ~50KB | 50TB | Optional, high precision |
| **TOTAL** | ~150KB | **~90-150TB** | With compression |

### 3.4 Compute Requirements

| Operation | Single Doc | 1B Docs (Batch) | Daily Incremental |
|-----------|------------|-----------------|-------------------|
| Embedding (15 models) | 500ms | 5,800 GPU-days | 50 docs/sec |
| K-NN Graph Update | 10ms | 116 GPU-days | Incremental NN-Descent |
| Citation Extraction | 100ms | 1,160 GPU-days | Batched NER |
| Edge Type Classification | 50ms | 580 GPU-days | R-GCN inference |
| **Query (P95)** | <500ms | - | Pre-computed graphs |

---

# Part II: Legal Embedding Models

## 4. Legal Embedding Models Survey

### 4.1 Foundation Legal Models

#### voyage-law-2 (Recommended for E1-LEGAL)

**Source**: [Voyage AI](https://blog.voyageai.com/2024/04/15/domain-specific-embeddings-and-retrieval-legal-edition-voyage-law-2/)

| Specification | Value |
|---------------|-------|
| **Dimension** | 1024D |
| **Context Length** | 16K tokens |
| **Training Data** | 1T+ legal tokens |
| **Benchmark** | +6% over OpenAI v3-large on MTEB Legal |

**Performance on Legal Benchmarks (NDCG@10)**:

| Benchmark | voyage-law-2 | OpenAI v3-large | Improvement |
|-----------|--------------|-----------------|-------------|
| LeCaRDv2 | 89.3 | 79.1 | +10.2% |
| LegalQuAD | 87.5 | 77.0 | +10.5% |
| GerDaLIR | 85.2 | 73.9 | +11.3% |
| ContractNLI | 82.1 | 78.3 | +3.8% |
| **Average (8 datasets)** | 84.6 | 78.6 | **+6%** |

**Integration**:
```rust
// E1-LEGAL: Replace e5-large-v2 with voyage-law-2 for legal domain
pub struct LegalSemanticFingerprint {
    pub e1_legal: [f32; 1024],  // voyage-law-2 (was e5-large-v2)
    // ... rest unchanged
}
```

#### Legal-BERT Family

| Model | Source | Training | Use Case |
|-------|--------|----------|----------|
| nlpaueb/legal-bert-base-uncased | [HuggingFace](https://huggingface.co/nlpaueb/legal-bert-base-uncased) | 12GB legal text | General legal NLP |
| casehold/legalbert | [HuggingFace](https://huggingface.co/casehold/legalbert) | CaseHold dataset | Holding identification |
| pile-of-law/legalbert-large | [HuggingFace](https://huggingface.co/pile-of-law/legalbert-large-1.7M-2) | 256GB Pile of Law | Fine-tuning base |
| law-ai/InLegalBERT | [HuggingFace](https://huggingface.co/law-ai/InLegalBERT) | Indian legal corpus | Indian jurisdiction |

### 4.2 Massive Legal Embedding Benchmark (MLEB)

**Source**: [MLEB Paper](https://www.researchgate.net/publication/396789828_The_Massive_Legal_Embedding_Benchmark_MLEB)

| Model | Judicial | Contractual | Regulatory | Average |
|-------|----------|-------------|------------|---------|
| Voyage 3.5 | 85.2 | 83.1 | 83.9 | **84.07** |
| Voyage Law 2 | 84.5 | 82.8 | 82.3 | 83.2 |
| Kanon 2 Embedder | 84.1 | 82.0 | 82.6 | 82.9 |
| OpenAI v3-large | 79.5 | 78.2 | 77.8 | 78.5 |
| Legal-BERT (fine-tuned) | 76.3 | 75.1 | 74.9 | 75.4 |

---

## 5. Long Document Models

### 5.1 SAILER (Structure-Aware Pre-trained Language Model)

**Source**: [SAILER (SIGIR 2023)](https://dl.acm.org/doi/10.1145/3539618.3591761)

**Key Innovation**: Captures legal document structure (Facts → Issues → Rules → Analysis → Conclusion)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    SAILER STRUCTURAL UNDERSTANDING                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Legal Document Structure (FIRAC):                                      │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ FACTS                                                            │   │
│  │ • Parties involved, key events, evidence                        │   │
│  │ • SAILER learns to weight factual similarity                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                     ↓ (causal link via E5-LEGAL)                       │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ ISSUES                                                           │   │
│  │ • Legal questions to be decided                                  │   │
│  │ • Issue matching = high-value retrieval signal                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                     ↓                                                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ RULES                                                            │   │
│  │ • Applicable statutes, controlling precedents                   │   │
│  │ • Links to statute hierarchy via E7-LEGAL                       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                     ↓                                                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ APPLICATION/ANALYSIS                                             │   │
│  │ • Application of law to facts                                   │   │
│  │ • Core reasoning - highest weight in argument chains            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                     ↓                                                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ CONCLUSION/HOLDING                                               │   │
│  │ • Court's decision, remedy                                      │   │
│  │ • Creates precedent edges to citing cases                       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Use as E14: Structure-aware embedding for legal document matching     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 5.2 DELTA (Discriminative Encoder for Legal Retrieval)

**Source**: [DELTA (AAAI 2025)](https://arxiv.org/html/2403.18435)

**Key Insight**: Semantic similarity ≠ legal relevance

| Feature | SAILER | DELTA |
|---------|--------|-------|
| Architecture | Asymmetric encoder-decoder | Word alignment + shallow decoder |
| Strength | Structure awareness | Discriminative ability |
| Best for | Case retrieval | Relevance ranking |
| Performance | SOTA on Chinese legal | +1.2% over SAILER |

### 5.3 Multi-Layered Embedding for Statutory Hierarchy

**Source**: [Multi-Layered Embedding Paper](https://arxiv.org/html/2411.07739v1)

```
Statute Hierarchy with Embeddings at Each Level:

United States Code (USC)
├── Title 42 (Public Health and Welfare)     → Title-level embedding
│   ├── Chapter 21 (Civil Rights)            → Chapter-level embedding
│   │   ├── § 1983 (Civil Action for...)     → Section-level embedding
│   │   │   ├── (a) General rule             → Subsection embedding
│   │   │   └── (b) Exceptions               → Subsection embedding
│   │   └── § 1985 (Conspiracy to...)        → Section-level embedding
│   └── Chapter 126 (Equal Opportunity...)   → Chapter-level embedding
└── Title 28 (Judiciary and Judicial...)     → Title-level embedding

Query routing:
- Broad query → Title/Chapter level first
- Specific query → Section/Subsection level
- Hyperbolic distance captures hierarchy naturally
```

---

## 6. Legal Named Entity Recognition

### 6.1 Legal Entity Taxonomy for E11-LEGAL

| Standard NER | Legal NER Extension | Examples |
|--------------|---------------------|----------|
| PERSON | PETITIONER, RESPONDENT, JUDGE, LAWYER, WITNESS | "John Smith, Esq.", "Justice Ginsburg" |
| ORGANIZATION | COURT, LAW_FIRM, AGENCY, COMPANY | "Supreme Court", "DOJ", "Skadden Arps" |
| LOCATION | JURISDICTION, VENUE, DISTRICT | "S.D.N.Y.", "9th Circuit" |
| DATE | FILING_DATE, DECISION_DATE, EFFECTIVE_DATE | "Filed: Jan 1, 2024" |
| (new) | CASE_CITATION | "347 U.S. 483 (1954)" |
| (new) | STATUTE_CITATION | "42 U.S.C. § 1983" |
| (new) | REGULATION_CITATION | "17 C.F.R. § 240.10b-5" |
| (new) | LEGAL_DOCTRINE | "qualified immunity", "stare decisis" |
| (new) | LEGAL_TEST | "Chevron deference", "Lemon test" |
| (new) | CHARGE | "breach of contract", "negligence" |

### 6.2 Citation Parsing for Graph Construction

```rust
// Citation types for edge construction
enum LegalCitation {
    CaseCitation {
        volume: u32,
        reporter: String,      // "U.S.", "F.3d", "S.Ct."
        page: u32,
        year: u16,
        court: Option<Court>,
        pinpoint: Option<u32>, // Specific page cite
    },
    StatuteCitation {
        title: u32,
        code: String,          // "U.S.C.", "C.F.R."
        section: String,
        subsection: Option<String>,
    },
    RegulationCitation {
        title: u32,
        code: String,
        part: u32,
        section: u32,
    },
}

// Extract citations → create graph edges
fn extract_citation_edges(document: &LegalDocument) -> Vec<CitationEdge> {
    let citations = parse_citations(&document.text);
    citations.iter().map(|cite| CitationEdge {
        source: document.id,
        target: resolve_citation(cite),  // Look up in citation index
        citation_type: classify_treatment(&document.context, cite),
        location_in_doc: cite.offset,
    }).collect()
}
```

---

# Part III: Graph Linking for Legal Documents

## 7. Legal Citation Network Architecture

### 7.1 Citation Network as Primary Graph Structure

Legal documents form a natural **citation network** that is fundamental to legal reasoning:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    LEGAL CITATION NETWORK                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Citation Network Properties:                                           │
│                                                                         │
│  • Directed: Case A → cites → Case B                                   │
│  • Typed: Different citation treatments (follows, distinguishes, etc.) │
│  • Weighted: Citation importance (in-text vs. string cite)             │
│  • Temporal: Later cases cite earlier (mostly)                         │
│  • Hierarchical: Higher courts have binding authority                  │
│                                                                         │
│  Example Network:                                                       │
│                                                                         │
│  Brown v. Board (1954) ←──── [100,000+ citing cases]                   │
│       │                                                                 │
│       │ overrules                                                       │
│       ↓                                                                 │
│  Plessy v. Ferguson (1896)                                             │
│                                                                         │
│  Miranda v. Arizona (1966)                                             │
│       │                                                                 │
│       │ follows/applies                                                 │
│       ↓                                                                 │
│  [Thousands of criminal procedure cases]                               │
│       │                                                                 │
│       │ distinguishes                                                   │
│       ↓                                                                 │
│  [Cases finding Miranda doesn't apply]                                 │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Citation Treatment Classification

| Treatment | Meaning | Edge Weight | Query Use |
|-----------|---------|-------------|-----------|
| **Follows** | Agrees with and applies precedent | 1.0 | Find supporting authority |
| **Applies** | Uses rule from cited case | 0.9 | Find applications |
| **Cites** | Neutral reference | 0.5 | General context |
| **Discusses** | Extended analysis | 0.7 | Deep treatment |
| **Distinguishes** | Explains why precedent doesn't apply | 0.6 | Find limitations |
| **Questions** | Expresses doubt | 0.4 | Find weakening cases |
| **Criticizes** | Disagrees but doesn't overrule | 0.3 | Find opposition |
| **Overrules** | Explicitly reverses | -1.0 | Find bad law |
| **Superseded** | Statute overrides case | -0.8 | Find legislative changes |

### 7.3 Citation Edge Schema

```rust
struct CitationEdge {
    // Identifiers
    source_id: Uuid,           // Citing document
    target_id: Uuid,           // Cited document

    // Citation metadata
    treatment: CitationTreatment,
    citation_count: u8,        // How many times cited in source
    depth: CitationDepth,      // String cite, parenthetical, block quote
    location: DocumentSection, // Where in source (Facts, Analysis, etc.)

    // Embedder-derived scores (from E15)
    semantic_similarity: f32,  // E1-LEGAL similarity
    structural_similarity: f32, // E14 (SAILER) similarity
    entity_overlap: f32,       // E11-LEGAL entity Jaccard

    // Temporal
    years_between: i16,        // Temporal distance

    // Graph features
    page_rank_contribution: f32, // Citation importance
}

enum CitationTreatment {
    Follows,
    Applies,
    Cites,
    Discusses,
    Distinguishes,
    Questions,
    Criticizes,
    Overrules,
    Superseded,
}

enum CitationDepth {
    StringCite,        // "See Smith v. Jones, 123 F.3d 456"
    Parenthetical,     // With explanatory parenthetical
    BlockQuote,        // Extended quotation
    PrincipalCase,     // Main authority for proposition
}
```

---

## 8. K-NN Graph Strategies for Legal Corpus

### 8.1 Embedder-Specific K-NN Graphs for Legal

Building on the general K-NN strategy, legal documents require specialized graphs:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    LEGAL K-NN GRAPH ARCHITECTURE                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Graph 1: E1-LEGAL Semantic K-NN (k=30)                                │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Purpose: General legal meaning similarity                        │   │
│  │ Use: Broad precedent discovery                                   │   │
│  │ Scale: 30 neighbors × 1B docs = 30B edges (sparse)              │   │
│  │ Storage: ~300GB (compressed, quantized similarities)            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Graph 2: E14 (SAILER) Structure K-NN (k=20)                           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Purpose: Same legal structure (similar Facts, Issues, Holdings)  │   │
│  │ Use: "Find cases with similar factual patterns"                 │   │
│  │ Scale: 20 neighbors × 1B docs = 20B edges                       │   │
│  │ Storage: ~200GB                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Graph 3: E15 Citation Embedding K-NN (k=50)                           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Purpose: Documents with similar citation patterns               │   │
│  │ Use: "Find cases citing same authorities"                       │   │
│  │ Scale: 50 neighbors × 1B docs = 50B edges                       │   │
│  │ Storage: ~500GB                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Graph 4: E11-LEGAL Entity K-NN (k=20)                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Purpose: Documents mentioning same legal entities               │   │
│  │ Use: "Find all cases involving this party/judge/statute"       │   │
│  │ Scale: 20 neighbors × 1B docs = 20B edges                       │   │
│  │ Storage: ~200GB                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Graph 5: E5-LEGAL Argument K-NN (k=15)                                │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Purpose: Documents with similar argument chains                  │   │
│  │ Use: "Find cases with similar reasoning"                        │   │
│  │ Scale: 15 neighbors × 1B docs = 15B edges                       │   │
│  │ Storage: ~150GB                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  TOTAL K-NN STORAGE: ~1.5TB (for 15 embedder graphs at 1B docs)        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 8.2 NN-Descent at Billion Scale

**Challenge**: O(n²) comparison is impossible at 1B documents.

**Solution**: NN-Descent algorithm with legal-specific optimizations:

```rust
// NN-Descent for billion-scale legal corpus
struct NNDescentConfig {
    k: usize,           // Neighbors per node (20-50)
    rho: f32,           // Sample rate (0.5)
    delta: f32,         // Convergence threshold (0.001)
    max_iterations: u32, // Typically converges in 10-20

    // Legal-specific optimizations
    jurisdiction_blocking: bool,  // Only compare within jurisdiction first
    temporal_window: Option<i32>, // Prefer temporally proximate cases
    doc_type_filter: bool,        // Cases with cases, statutes with statutes
}

impl NNDescentBuilder {
    /// Build K-NN graph incrementally for streaming ingestion
    pub async fn incremental_update(
        &mut self,
        new_fingerprints: &[LegalFingerprint],
        embedder: EmbedderId,
    ) -> Result<()> {
        // For each new document:
        // 1. Find approximate neighbors using existing graph
        // 2. Compare to neighbors' neighbors (NN-Descent principle)
        // 3. Update affected edge lists
        // 4. Propagate changes (limited iterations)

        // Complexity: O(k² × |new_docs|) instead of O(n × |new_docs|)
    }
}
```

### 8.3 Jurisdiction-Partitioned K-NN

Legal relevance is heavily jurisdiction-dependent:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                JURISDICTION-PARTITIONED K-NN STRATEGY                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Hierarchy:                                                             │
│                                                                         │
│  Global K-NN (cross-jurisdiction, k=10)                                │
│       ↓                                                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Country-Level Partitions                                         │   │
│  │                                                                  │   │
│  │  US (500M docs)    UK (50M docs)    EU (100M docs)    ...       │   │
│  │      ↓                  ↓                ↓                       │   │
│  │  ┌──────────┐      ┌──────────┐     ┌──────────┐                │   │
│  │  │ Federal  │      │ England  │     │ CJEU     │                │   │
│  │  │ 9th Cir  │      │ Scotland │     │ Member   │                │   │
│  │  │ S.D.N.Y. │      │ N.Ireland│     │ States   │                │   │
│  │  │ CA State │      └──────────┘     └──────────┘                │   │
│  │  └──────────┘                                                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Query Routing:                                                         │
│  1. Identify query jurisdiction (explicit or inferred)                 │
│  2. Search local partition K-NN first (k=20)                           │
│  3. Expand to parent jurisdiction (k=10)                               │
│  4. Cross-jurisdiction only if explicitly requested (k=5)             │
│                                                                         │
│  Benefits:                                                              │
│  • Binding authority prioritized                                       │
│  • Reduced search space per query                                      │
│  • Parallel partition processing                                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Multi-Relation Legal Edge Types

### 9.1 Legal Edge Type Taxonomy

Building on the general multi-relation approach, legal documents require specialized edge types:

```rust
enum LegalEdgeType {
    // Citation Edges (from citation network)
    Cites { treatment: CitationTreatment },
    CitedBy { treatment: CitationTreatment },

    // Semantic Edges (from embedder K-NN)
    SemanticSimilar { embedder: EmbedderId, score: f32 },

    // Structural Edges (from E14 SAILER)
    SimilarFacts,
    SimilarIssues,
    SimilarHolding,
    SimilarReasoning,

    // Entity Edges (from E11-LEGAL)
    SameParty { party_type: PartyType },
    SameJudge,
    SameCourt,
    SameStatute,
    SameLegalDoctrine,

    // Argument Edges (from E5-LEGAL)
    ArgumentSupports,       // A's holding supports B's argument
    ArgumentConflicts,      // A conflicts with B's reasoning

    // Hierarchical Edges (from hyperbolic embedding)
    ParentSection,          // Statute hierarchy
    ChildSection,
    SiblingSection,

    // Temporal Edges
    Supersedes,             // Later case/statute supersedes earlier
    SupersededBy,
    AmendedBy,

    // Cross-Reference Edges
    Interprets { statute_id: Uuid },  // Case interprets statute
    InterpretedBy { case_id: Uuid },  // Statute interpreted by case
}

struct LegalTypedEdge {
    source: Uuid,
    target: Uuid,
    edge_type: LegalEdgeType,
    weight: f32,
    confidence: f32,         // ML model confidence
    embedder_scores: [f32; 15],  // Full 15-embedder breakdown
    metadata: EdgeMetadata,
}
```

### 9.2 Edge Type Detection Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    EDGE TYPE DETECTION PIPELINE                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Input: Document pair (A, B) with 15 embedder similarities             │
│                                                                         │
│  Step 1: Citation Check                                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ If A explicitly cites B:                                         │   │
│  │   → Extract citation treatment (follows, distinguishes, etc.)   │   │
│  │   → Create Cites edge with treatment                            │   │
│  │   → Weight = treatment_weight × citation_depth                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Step 2: Semantic Similarity Check                                      │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ For each embedder with sim > threshold:                         │   │
│  │   E1-LEGAL > 0.7 → SemanticSimilar                              │   │
│  │   E14 > 0.7 → SimilarFacts/Issues/Holding (based on segment)    │   │
│  │   E5-LEGAL > 0.6 → ArgumentSupports/Conflicts                   │   │
│  │   E11-LEGAL > 0.8 → SameEntity edges                            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Step 3: Entity Overlap Check                                           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Extract entities from both documents                            │   │
│  │ For shared entities → Create SameParty/Judge/Court/Statute      │   │
│  │ Weight = entity_importance × mention_count                      │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Step 4: Hierarchical Check (for statutes)                              │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ If both are statute sections:                                   │   │
│  │   → Determine hierarchy (parent/child/sibling)                  │   │
│  │   → Create hyperbolic hierarchy edge                            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Step 5: Temporal Check                                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ If B supersedes/amends A:                                       │   │
│  │   → Create Supersedes/AmendedBy edge                            │   │
│  │   → Mark A as potentially outdated                              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Output: List of typed edges with weights and confidence scores        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 9.3 Weighted Agreement for Legal Edges

Extending the weighted agreement formula for legal domain:

```
Legal Weighted Agreement = Σ(legal_weight × sim_above_threshold)

Legal Category Weights:
- LEGAL_SEMANTIC (1.0): E1-LEGAL, E5-LEGAL, E14 (SAILER)
- CITATION (1.5): E15 (citation embedding) - HIGHEST WEIGHT
- ENTITY (1.0): E11-LEGAL
- STRUCTURAL (0.8): E7-LEGAL (statute structure)
- TEMPORAL (0.0): E2, E3, E4 (excluded from linking, used for freshness only)

max_legal_weighted_agreement = 1.0×3 + 1.5×1 + 1.0×1 + 0.8×1 = 6.3

Create edge if: legal_weighted_agreement >= 3.0
Edge weight = legal_weighted_agreement / 6.3 (normalized)
```

---

## 10. Directed Edges for Legal Reasoning

### 10.1 Causal Edges for Legal Arguments (E5-LEGAL)

Legal reasoning is inherently directional: facts lead to issues, rules lead to conclusions.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    LEGAL ARGUMENT DIRECTED EDGES                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  E5-LEGAL Causal Direction:                                            │
│                                                                         │
│  Premise → Conclusion (from argument mining)                           │
│                                                                         │
│  Example:                                                               │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ [Premise: "The officer had reasonable suspicion based on..."]   │   │
│  │           │                                                      │   │
│  │           │ supports (E5 causal edge)                           │   │
│  │           ↓                                                      │   │
│  │ [Conclusion: "The stop was constitutional under Terry"]         │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Cross-Document Argument Chains:                                        │
│                                                                         │
│  [Case A: Establishes rule] ───supports──→ [Case B: Applies rule]     │
│                              ↑                                          │
│                              │                                          │
│  [Case C: Distinguishes] ────┘ (weakens support)                       │
│                                                                         │
│  Query: "What supports the holding in Case B?"                         │
│  Answer: Traverse backwards along 'supports' edges                      │
│                                                                         │
│  Query: "What challenges Case B's reasoning?"                          │
│  Answer: Find 'distinguishes', 'criticizes', 'overrules' edges        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 10.2 Precedent Chain Edges

```rust
struct PrecedentChainEdge {
    citing_case: Uuid,
    cited_case: Uuid,
    treatment: CitationTreatment,

    // Direction of legal authority
    authority_direction: AuthorityDirection,
    binding_strength: BindingStrength,

    // Temporal direction (always later → earlier)
    temporal_direction: TemporalDirection,
}

enum AuthorityDirection {
    Binding,      // Higher court → Lower court (same jurisdiction)
    Persuasive,   // Peer court or different jurisdiction
    Advisory,     // Non-precedential (e.g., unpublished)
}

enum BindingStrength {
    Mandatory,    // Must follow
    Strongly,     // Should follow
    Weakly,       // May consider
    None,         // Not binding
}

// Compute precedent chain for a legal proposition
fn trace_precedent_chain(
    proposition: &str,
    start_case: Uuid,
    max_hops: usize,
) -> Vec<PrecedentChainEdge> {
    // 1. Find cases cited for this proposition (using E14 + E5-LEGAL)
    // 2. Recursively trace citations backwards
    // 3. Filter to "follows" and "applies" treatments
    // 4. Order by binding strength and temporal
    // 5. Return chain of precedents supporting the proposition
}
```

### 10.3 Statute Interpretation Edges

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    STATUTE-CASE INTERPRETATION EDGES                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  42 U.S.C. § 1983                                                       │
│       │                                                                 │
│       │ interpreted_by (directed edge)                                  │
│       ↓                                                                 │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Monroe v. Pape (1961) - Established "under color of state law"  │   │
│  │      │                                                           │   │
│  │      │ refined_by                                                │   │
│  │      ↓                                                           │   │
│  │ Monell v. NYC (1978) - Municipal liability                      │   │
│  │      │                                                           │   │
│  │      │ expanded_by                                               │   │
│  │      ↓                                                           │   │
│  │ Harlow v. Fitzgerald (1982) - Qualified immunity standard       │   │
│  │      │                                                           │   │
│  │      │ limited_by                                                │   │
│  │      ↓                                                           │   │
│  │ [Recent qualified immunity cases]                               │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Query: "How has § 1983 been interpreted?"                             │
│  Answer: Traverse interpretation chain chronologically                  │
│                                                                         │
│  Query: "Current state of qualified immunity doctrine?"                │
│  Answer: Latest nodes in interpretation subgraph                        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

# Part IV: Graph Neural Networks for Legal Analysis

## 11. GNN Approaches for Legal Knowledge Graphs

### 11.1 Relational Graph Convolutional Network (R-GCN) for Legal

**Source**: [R-GCN Paper](https://arxiv.org/abs/1703.06103), [Legal Citation GNN Paper](https://arxiv.org/html/2506.22165v1)

R-GCN is ideal for legal because legal graphs have **many relation types** (cites, follows, overrules, interprets, etc.):

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    R-GCN FOR LEGAL KNOWLEDGE GRAPH                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Node Features: 15-embedder summary (15D) + metadata (court, date, etc.)│
│                                                                         │
│  Relation Types (r):                                                    │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ r₁: cites_follows       W_r₁ = learned weight matrix           │   │
│  │ r₂: cites_distinguishes W_r₂ = learned weight matrix           │   │
│  │ r₃: cites_overrules     W_r₃ = learned weight matrix           │   │
│  │ r₄: same_statute        W_r₄ = learned weight matrix           │   │
│  │ r₅: same_judge          W_r₅ = learned weight matrix           │   │
│  │ r₆: semantic_similar    W_r₆ = learned weight matrix           │   │
│  │ r₇: argument_supports   W_r₇ = learned weight matrix           │   │
│  │ ...                                                             │   │
│  │ r₁₅: interprets         W_r₁₅ = learned weight matrix          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  R-GCN Layer Update:                                                    │
│                                                                         │
│  h_i^(l+1) = σ( Σ_r Σ_{j∈N_r(i)} (1/c_{i,r}) W_r^(l) h_j^(l) + W_0 h_i)│
│                                                                         │
│  For each case i:                                                       │
│  - Aggregate from all cases it cites (by treatment type)               │
│  - Aggregate from all cases citing it (by treatment type)              │
│  - Aggregate from cases with same entities                              │
│  - Each relation type has separate learned weights                      │
│                                                                         │
│  The model learns:                                                      │
│  • Which citation treatments matter most for relevance                 │
│  • How to weight semantic vs. citation similarity                      │
│  • Optimal combination of embedder signals                              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 11.2 Graph Attention Network (GAT) for Citation Networks

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    GAT FOR LEGAL CITATION NETWORK                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Key Insight: Not all cited cases are equally important.               │
│  GAT learns attention weights over neighbors.                          │
│                                                                         │
│  For case i citing cases {j₁, j₂, ..., jₙ}:                            │
│                                                                         │
│  Attention coefficient:                                                 │
│  α_ij = softmax_j(LeakyReLU(a^T [W h_i || W h_j || e_ij]))            │
│                                                                         │
│  Where e_ij = edge features:                                           │
│  [treatment_type, citation_depth, years_between, semantic_sim, ...]    │
│                                                                         │
│  Multi-head for legal domain (4 heads):                                │
│  • Head 1: Citation treatment attention                                │
│  • Head 2: Semantic similarity attention                                │
│  • Head 3: Entity overlap attention                                     │
│  • Head 4: Temporal proximity attention                                 │
│                                                                         │
│  Updated representation:                                                │
│  h_i' = ||_{k=1}^{4} σ(Σ_j α_ij^k W^k h_j)                            │
│                                                                         │
│  Use case: Rank citing cases by relevance to a legal proposition       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 11.3 Legal Link Prediction with GNNs

**Source**: [Joint Legal Citation Prediction Paper](https://arxiv.org/html/2506.22165v1)

```rust
// GNN-based citation link prediction
struct LegalLinkPredictor {
    // Node encoder (R-GCN)
    rgcn_layers: Vec<RGCNLayer>,

    // Edge predictor
    edge_predictor: MLP,

    // Treatment classifier
    treatment_classifier: MLP,
}

impl LegalLinkPredictor {
    pub fn predict_citation(&self, case_a: &CaseNode, case_b: &CaseNode) -> CitationPrediction {
        // 1. Encode both cases through R-GCN
        let h_a = self.encode(case_a);
        let h_b = self.encode(case_b);

        // 2. Predict if citation exists
        let edge_features = concat(h_a, h_b, h_a * h_b);
        let cite_prob = self.edge_predictor.forward(edge_features);

        // 3. If citation predicted, classify treatment
        let treatment = if cite_prob > 0.5 {
            self.treatment_classifier.forward(edge_features)
        } else {
            None
        };

        CitationPrediction { cite_prob, treatment }
    }

    pub fn find_missing_citations(&self, case: &CaseNode, candidates: &[CaseNode]) -> Vec<CitationPrediction> {
        // Find cases that should be cited but aren't
        // Useful for: drafting assistance, completeness checking
    }
}
```

---

## 12. Hyperbolic Embeddings for Legal Hierarchy

### 12.1 Why Hyperbolic Space for Legal Documents

Legal documents have **intrinsic hierarchical structure** that hyperbolic geometry captures naturally:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    HYPERBOLIC SPACE FOR LEGAL HIERARCHY                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Euclidean Space Problem:                                               │
│  • Trees require exponentially growing space                            │
│  • Statute hierarchy (thousands of sections) is inefficient            │
│                                                                         │
│  Hyperbolic Space Solution:                                             │
│  • Area grows exponentially with radius                                 │
│  • Trees embed with low distortion                                      │
│  • Parent-child distances are meaningful                               │
│                                                                         │
│  Legal Hierarchies to Embed:                                            │
│                                                                         │
│  1. STATUTE HIERARCHY:                                                  │
│     United States Code                                                  │
│        └── Title 42 (distance: 1)                                      │
│              └── Chapter 21 (distance: 2)                              │
│                    └── § 1983 (distance: 3)                            │
│                          └── Subsection (a) (distance: 4)              │
│                                                                         │
│  2. COURT HIERARCHY:                                                    │
│     Supreme Court (center of Poincaré ball)                            │
│        └── Circuit Courts (radius: 0.3)                                │
│              └── District Courts (radius: 0.6)                         │
│                    └── Bankruptcy Courts (radius: 0.8)                 │
│                                                                         │
│  3. TOPIC HIERARCHY:                                                    │
│     Law (center)                                                        │
│        ├── Civil Law (radius: 0.2)                                     │
│        │     ├── Contract Law (radius: 0.4)                            │
│        │     ├── Tort Law (radius: 0.4)                                │
│        │     └── Property Law (radius: 0.4)                            │
│        └── Criminal Law (radius: 0.2)                                  │
│              ├── Violent Crimes (radius: 0.4)                          │
│              └── White Collar (radius: 0.4)                            │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 12.2 Hyperbolic GNN for Legal Navigation

**Source**: [H2GNN Paper](https://arxiv.org/abs/2412.12158)

```rust
// Hyperbolic embedding for legal hierarchy
struct HyperbolicLegalEmbedding {
    // Poincaré ball model
    curvature: f32,  // -1.0 for standard hyperbolic
    dimension: usize,  // 32-64D is often sufficient
}

impl HyperbolicLegalEmbedding {
    /// Embed statute hierarchy in hyperbolic space
    pub fn embed_statute_hierarchy(&self, statute: &StatuteTree) -> Vec<HyperbolicPoint> {
        // Root (full code) at origin
        // Depth determines radius
        // Siblings spread around parent
    }

    /// Hyperbolic distance captures hierarchy
    pub fn hyperbolic_distance(&self, a: &HyperbolicPoint, b: &HyperbolicPoint) -> f32 {
        // Poincaré distance:
        // d(a, b) = arcosh(1 + 2 * ||a - b||² / ((1 - ||a||²)(1 - ||b||²)))
    }

    /// Navigate hierarchy
    pub fn find_parent(&self, section: &HyperbolicPoint) -> HyperbolicPoint {
        // Move toward origin (higher in hierarchy)
    }

    pub fn find_siblings(&self, section: &HyperbolicPoint) -> Vec<HyperbolicPoint> {
        // Find points at same radius (same hierarchy level)
    }
}
```

### 12.3 Legal Hypergraph for Topics

**Source**: [Hyper-FM Foundation Model](https://arxiv.org/html/2503.01203v1)

Hyperedges connect **all documents sharing a legal topic**:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    LEGAL TOPIC HYPERGRAPH                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Hyperedge 1: "Qualified Immunity"                                      │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Harlow v. Fitzgerald (1982)                                   │   │
│  │ • Anderson v. Creighton (1987)                                  │   │
│  │ • Pearson v. Callahan (2009)                                    │   │
│  │ • ... (10,000+ cases)                                           │   │
│  │ • 42 U.S.C. § 1983 (related statute)                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Hyperedge 2: "Fourth Amendment Searches"                               │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Katz v. United States (1967)                                  │   │
│  │ • Terry v. Ohio (1968)                                          │   │
│  │ • Carpenter v. United States (2018)                             │   │
│  │ • U.S. Const. amend. IV                                        │   │
│  │ • ... (50,000+ cases)                                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Hyperedge Convolution:                                                 │
│  • Aggregate all documents in hyperedge                                │
│  • Propagate topic-level information to all members                    │
│  • Documents in multiple hyperedges get richer representations        │
│                                                                         │
│  Query: "Recent developments in qualified immunity"                    │
│  → Find hyperedge → Get all members → Filter by recency                │
│  → Return comprehensive topic overview                                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 13. Multi-View Learning for Legal Documents

### 13.1 Disentangled Multi-View Learning for Legal

Each legal embedder captures different aspects. Learn them separately to avoid interference:

```
┌─────────────────────────────────────────────────────────────────────────┐
│            DISENTANGLED MULTI-VIEW LEARNING FOR LEGAL                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  View 1: LEGAL SEMANTIC (E1-LEGAL, E14 SAILER)                         │
│       GNN_semantic(G_semantic) → H_semantic                            │
│       • Captures general legal meaning                                  │
│       • FIRAC structure awareness                                       │
│                                    ↓                                    │
│  View 2: CITATION NETWORK (E15, E8-LEGAL)                              │
│       GNN_citation(G_citation) → H_citation                            │
│       • Captures precedent relationships                               │
│       • Authority flow through network                                  │
│                           ↓        ↓                                    │
│  View 3: ENTITY RELATIONS (E11-LEGAL)                                  │
│       GNN_entity(G_entity) → H_entity                                  │
│       • Captures party, judge, statute relationships                   │
│                               ↓    ↓                                    │
│  View 4: ARGUMENT STRUCTURE (E5-LEGAL)                                 │
│       GNN_argument(G_argument) → H_argument                            │
│       • Captures reasoning chains                                       │
│                                    ↓                                    │
│                            ┌───────┴───────┐                           │
│                            │  LEGAL FUSION  │                          │
│                            │  LAYER         │                          │
│                            └───────┬───────┘                           │
│                                    ↓                                    │
│                              H_legal_unified                            │
│                                                                         │
│  Fusion Strategy (Query-Dependent):                                     │
│  • Precedent query → Weight citation view higher                       │
│  • Entity query → Weight entity view higher                            │
│  • Reasoning query → Weight argument view higher                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 13.2 Contrastive Learning for Legal Edge Weights

**Self-supervised learning** to optimize edge weights without manual labels:

```
┌─────────────────────────────────────────────────────────────────────────┐
│            CONTRASTIVE LEARNING FOR LEGAL EDGE WEIGHTS                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Positive Pairs (should have strong edges):                            │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Case A and Case B it explicitly cites with "follows"          │   │
│  │ • Cases with same holding on same legal issue                   │   │
│  │ • Statute section and case interpreting it                      │   │
│  │ • Documents sharing 3+ named entities                           │   │
│  │ • Same case in different embedder views (cross-view positive)   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Negative Pairs (should have weak/no edges):                           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Random document pairs from different practice areas           │   │
│  │ • Cases from different jurisdictions with no citation           │   │
│  │ • Documents with conflicting holdings                           │   │
│  │ • Case that "overrules" or "criticizes" another (hard negative) │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Contrastive Loss:                                                      │
│  L = -log(exp(sim(z_i, z_j⁺) / τ) / Σ_k exp(sim(z_i, z_k) / τ))       │
│                                                                         │
│  Training Signal Sources (self-supervised):                             │
│  • Citation network (cites = positive)                                 │
│  • Topic co-membership (same topic = positive)                         │
│  • Entity co-occurrence (same entities = positive)                     │
│  • Argument chains (premise→conclusion = positive)                     │
│                                                                         │
│  Result: Learned edge weights that capture legal relevance             │
│          without manual annotation                                      │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 13.3 Cross-Embedder Attention for Legal Queries

```rust
// Learn which embedders matter for each legal query type
struct LegalCrossEmbedderAttention {
    attention_weights: Linear,  // Input: 15 similarities → 15 attention scores
    query_type_embeddings: Embedding,  // Different weights per query type
}

impl LegalCrossEmbedderAttention {
    pub fn compute_edge_weight(
        &self,
        doc_a: &LegalFingerprint,
        doc_b: &LegalFingerprint,
        query_type: LegalQueryType,
    ) -> f32 {
        // 1. Compute all 15 embedder similarities
        let similarities = compute_all_similarities(doc_a, doc_b);

        // 2. Get query-type-specific attention
        let query_embedding = self.query_type_embeddings.forward(query_type);

        // 3. Compute attention over similarities
        let attention = softmax(self.attention_weights.forward(
            concat(similarities, query_embedding)
        ));

        // 4. Weighted combination
        dot(attention, similarities)
    }
}

enum LegalQueryType {
    PrecedentSearch,      // Weight citation embedders
    FactPatternMatch,     // Weight SAILER structural
    StatuteInterpretation, // Weight statute hierarchy
    ArgumentAnalysis,     // Weight causal embedders
    EntitySearch,         // Weight entity embedders
    General,              // Balanced weights
}
```

---

# Part V: Billion-Scale Architecture

## 14. Distributed Architecture for 1B+ Documents

### 14.1 Sharding Strategy

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    BILLION-SCALE DISTRIBUTED ARCHITECTURE                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  SHARDING STRATEGY: Jurisdiction + Document Type + Time                │
│                                                                         │
│  Shard Key: hash(jurisdiction) + doc_type + year_bucket                │
│                                                                         │
│  Example Shards (1000 total):                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Shard 001: US_FEDERAL_CASES_2020-2029 (~50M docs)               │   │
│  │ Shard 002: US_FEDERAL_CASES_2010-2019 (~40M docs)               │   │
│  │ Shard 003: US_FEDERAL_CASES_2000-2009 (~30M docs)               │   │
│  │ ...                                                              │   │
│  │ Shard 050: US_9TH_CIRCUIT_CASES (~20M docs)                     │   │
│  │ ...                                                              │   │
│  │ Shard 200: US_FEDERAL_STATUTES (~500K docs, small but critical) │   │
│  │ ...                                                              │   │
│  │ Shard 500: UK_CASES (~50M docs)                                 │   │
│  │ ...                                                              │   │
│  │ Shard 800: CONTRACTS_US (~200M docs)                            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Benefits:                                                              │
│  • Jurisdiction queries hit 1-2 shards (90% of queries)                │
│  • Hot data (recent cases) on faster storage                           │
│  • Cross-jurisdiction queries fan out (rare, accepted latency)         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 14.2 Index Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    INDEX ARCHITECTURE AT SCALE                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  TIER 1: HOT INDEXES (GPU RAM, 32GB per node × 100 nodes)              │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • E1-LEGAL Matryoshka-128D for fast ANN (recent 100M docs)      │   │
│  │ • E15 Citation graph (100M most-cited nodes)                    │   │
│  │ • E11-LEGAL Entity index (high-frequency entities)              │   │
│  │ Storage: 3.2TB GPU RAM total                                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  TIER 2: WARM INDEXES (SSD, faiss-gpu with memory mapping)             │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Full E1-LEGAL 1024D indexes (all 1B docs, sharded)           │   │
│  │ • E14 SAILER structure indexes                                  │   │
│  │ • E5-LEGAL argument indexes                                     │   │
│  │ Storage: 50TB SSD total                                         │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  TIER 3: COLD INDEXES (Object storage, on-demand loading)              │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Historical document indexes (pre-1990)                        │   │
│  │ • Full ColBERT (E12) token embeddings                          │   │
│  │ • Rarely-accessed jurisdiction indexes                          │   │
│  │ Storage: 200TB object storage                                   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  GRAPH STORAGE:                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • Citation edges: Neo4j or TigerGraph cluster                   │   │
│  │ • K-NN edges: RocksDB column families (per shard)              │   │
│  │ • Hypergraph edges: Specialized hypergraph DB                   │   │
│  │ Storage: 10TB for edges + metadata                              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 14.3 Query Routing

```rust
struct LegalQueryRouter {
    shard_map: HashMap<ShardKey, Vec<NodeAddress>>,
    jurisdiction_detector: JurisdictionClassifier,
    query_type_classifier: QueryTypeClassifier,
}

impl LegalQueryRouter {
    pub async fn route_query(&self, query: &LegalQuery) -> QueryPlan {
        // 1. Detect query type
        let query_type = self.query_type_classifier.classify(query);

        // 2. Detect target jurisdiction(s)
        let jurisdictions = self.jurisdiction_detector.detect(query);

        // 3. Determine shards to query
        let shards = match query_type {
            QueryType::PrecedentSearch => {
                // Start with jurisdiction shards, may expand
                self.get_jurisdiction_shards(&jurisdictions)
            },
            QueryType::CrossJurisdictional => {
                // Fan out to all relevant jurisdictions
                self.get_cross_jurisdiction_shards(&jurisdictions)
            },
            QueryType::StatuteSearch => {
                // Statute shards are small, query all relevant
                self.get_statute_shards(&jurisdictions)
            },
            QueryType::EntitySearch => {
                // May need to query all shards if entity is unknown
                self.get_entity_shards(query)
            },
        };

        // 4. Build query plan
        QueryPlan {
            shards,
            embedders: self.select_embedders(&query_type),
            graph_traversal: self.plan_graph_traversal(&query_type),
            result_fusion: self.plan_fusion(&query_type),
        }
    }
}
```

---

## 15. Legal-Specific Retrieval Pipeline

### 15.1 6-Stage Legal Retrieval Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    6-STAGE LEGAL RETRIEVAL PIPELINE                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Query: "What cases support excluding expert testimony under Daubert?"  │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ STAGE 1: SPARSE RECALL (E13 SPLADE + E6-LEGAL)                  │   │
│  │ • Expand "Daubert" → "expert testimony", "scientific evidence"  │   │
│  │ • Query inverted indexes across relevant shards                 │   │
│  │ • Output: 50,000 candidates                                     │   │
│  │ • Latency: <10ms                                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                      ↓                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ STAGE 2: DENSE FOUNDATION (E1-LEGAL Matryoshka-128D)            │   │
│  │ • Fast ANN search on 128D truncated embeddings                  │   │
│  │ • Score fusion with Stage 1 via RRF                             │   │
│  │ • Output: 5,000 candidates                                      │   │
│  │ • Latency: <20ms                                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                      ↓                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ STAGE 3: GRAPH EXPANSION (NEW)                                  │   │
│  │ • For top 500 candidates, traverse K-NN edges                   │   │
│  │ • Add 1-hop neighbors from citation network                     │   │
│  │ • Add documents sharing key entities ("Daubert", "Rule 702")    │   │
│  │ • Output: 2,000 candidates (expanded)                           │   │
│  │ • Latency: <30ms                                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                      ↓                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ STAGE 4: MULTI-SPACE RRF (15 embedders)                         │   │
│  │ • Score all 2,000 candidates across 15 embedders                │   │
│  │ • Legal-weighted RRF (citation embedder 1.5x weight)            │   │
│  │ • Output: 200 candidates                                        │   │
│  │ • Latency: <50ms                                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                      ↓                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ STAGE 5: GNN REASONING (R-GCN inference)                        │   │
│  │ • Run R-GCN on subgraph of 200 candidates + neighbors           │   │
│  │ • Propagate authority through citation edges                    │   │
│  │ • Boost documents with strong precedent support                 │   │
│  │ • Output: 50 candidates (graph-reranked)                        │   │
│  │ • Latency: <50ms                                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                      ↓                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ STAGE 6: ColBERT MaxSim RERANK (E12)                            │   │
│  │ • Token-level similarity for exact matching                     │   │
│  │ • Output: k final results with confidence scores                │   │
│  │ • Latency: <30ms                                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  TOTAL LATENCY TARGET: <200ms P95 at 1B documents                      │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 15.2 Legal-Specific MCP Tools

| Tool | Purpose | Pipeline Stages | Graph Features |
|------|---------|-----------------|----------------|
| `search_legal_precedent` | Find controlling precedent | 1-6 | Citation K-NN + R-GCN |
| `search_statutes` | Find applicable statutes | 1-4 | Hyperbolic hierarchy |
| `find_citing_cases` | All cases citing a given case | 3 only | Citation edges direct |
| `trace_precedent_chain` | Build precedent chain | 3 + 5 | E5-LEGAL causal edges |
| `find_conflicting_authority` | Find opposing cases | 3-5 | Distinguishes/overrules edges |
| `search_by_fact_pattern` | Match on facts | 1-4 | E14 SAILER + entity |
| `find_statute_interpretations` | How statute interpreted | 3-5 | Interprets edges |
| `compare_jurisdictions` | Cross-jurisdiction analysis | 1-4 (fanout) | Jurisdiction-partitioned K-NN |
| `extract_legal_entities` | NER for legal docs | Preprocessing | E11-LEGAL |
| `get_topic_landscape` | Overview of legal topic | Hypergraph | Topic hyperedges |

---

## 16. Insight Extraction System

### 16.1 Insights Only Multi-Embedder + Graph Can Find

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    UNIQUE INSIGHTS FROM GRAPH LINKING                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. PRECEDENT TRAJECTORY ANALYSIS                                       │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Query: "How is qualified immunity doctrine trending?"            │   │
│  │                                                                  │   │
│  │ Single embedder: Returns recent cases mentioning the term       │   │
│  │                                                                  │   │
│  │ Graph-enhanced:                                                  │   │
│  │ • Traces citation chain from Harlow (1982) to present          │   │
│  │ • Identifies "distinguishes" and "criticizes" edges increasing │   │
│  │ • Detects dissent-to-majority pattern (Sotomayor dissents)     │   │
│  │ • Finds circuit splits via cross-jurisdiction graph analysis   │   │
│  │ • Predicts: "Doctrine under increasing pressure, possible       │   │
│  │   Supreme Court revisit likely"                                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  2. HIDDEN PRECEDENT DISCOVERY                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Query: "Find relevant precedent for smart contract disputes"    │   │
│  │                                                                  │   │
│  │ Single embedder: Returns cases mentioning "smart contract"      │   │
│  │                  (very few, all recent)                         │   │
│  │                                                                  │   │
│  │ Graph-enhanced:                                                  │   │
│  │ • E14 SAILER finds structurally similar cases                  │   │
│  │ • Discovers contract formation cases are structurally similar   │   │
│  │ • Entity linking finds "UCC", "statute of frauds" connections  │   │
│  │ • Returns: 1980s contract cases applicable to new technology   │   │
│  │ • Insight: "Pre-digital contract law principles apply"          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  3. ARGUMENT STRENGTH ANALYSIS                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Query: "Evaluate strength of 'state action' defense"            │   │
│  │                                                                  │   │
│  │ Single embedder: Returns cases mentioning "state action"        │   │
│  │                                                                  │   │
│  │ Graph-enhanced:                                                  │   │
│  │ • E5-LEGAL maps argument chains supporting the defense          │   │
│  │ • Counts "follows" vs "distinguishes" on key authorities       │   │
│  │ • R-GCN computes authority flow through network                 │   │
│  │ • Identifies weak points (frequently distinguished cases)       │   │
│  │ • Returns: Strength score + specific vulnerabilities            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  4. STATUTORY INTERPRETATION EVOLUTION                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Query: "How has ADA 'reasonable accommodation' evolved?"        │   │
│  │                                                                  │   │
│  │ Single embedder: Returns cases with those words                 │   │
│  │                                                                  │   │
│  │ Graph-enhanced:                                                  │   │
│  │ • Hyperbolic hierarchy locates exact ADA section                │   │
│  │ • Traverses "interprets" edges chronologically                  │   │
│  │ • Clusters interpretations by circuit via jurisdiction graph   │   │
│  │ • Detects definition expansion/contraction over time            │   │
│  │ • Returns: Timeline of interpretive changes by circuit          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  5. CIRCUIT SPLIT DETECTION                                             │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Query: "Identify circuit splits on Fourth Amendment cell phone  │   │
│  │         searches"                                                │   │
│  │                                                                  │   │
│  │ Single embedder: Returns relevant cases without conflict info   │   │
│  │                                                                  │   │
│  │ Graph-enhanced:                                                  │   │
│  │ • Groups cases by circuit via jurisdiction metadata             │   │
│  │ • E5-LEGAL identifies conflicting holdings                      │   │
│  │ • Cross-citation analysis shows circuits not citing each other │   │
│  │ • Returns: Specific split + cases on each side + likelihood     │   │
│  │   of Supreme Court resolution                                   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 16.2 Automated Insight Generation

```rust
// Insight extraction system
struct LegalInsightExtractor {
    precedent_analyzer: PrecedentTrajectoryAnalyzer,
    split_detector: CircuitSplitDetector,
    argument_evaluator: ArgumentStrengthEvaluator,
    trend_analyzer: DoctrinalTrendAnalyzer,
}

impl LegalInsightExtractor {
    /// Run all analyzers on search results
    pub async fn extract_insights(
        &self,
        query: &LegalQuery,
        results: &[SearchResult],
    ) -> Vec<LegalInsight> {
        let mut insights = Vec::new();

        // 1. Precedent trajectory
        if let Some(trajectory) = self.precedent_analyzer.analyze(results).await {
            insights.push(LegalInsight::PrecedentTrajectory(trajectory));
        }

        // 2. Circuit splits
        if let Some(split) = self.split_detector.detect(results).await {
            insights.push(LegalInsight::CircuitSplit(split));
        }

        // 3. Argument strength
        if query.is_argument_query() {
            let strength = self.argument_evaluator.evaluate(query, results).await;
            insights.push(LegalInsight::ArgumentStrength(strength));
        }

        // 4. Doctrinal trends
        if let Some(trend) = self.trend_analyzer.analyze(results).await {
            insights.push(LegalInsight::DoctrinalTrend(trend));
        }

        insights
    }
}

enum LegalInsight {
    PrecedentTrajectory {
        direction: TrendDirection,  // Strengthening, Weakening, Stable
        key_cases: Vec<CaseSummary>,
        confidence: f32,
    },
    CircuitSplit {
        circuits_side_a: Vec<Circuit>,
        circuits_side_b: Vec<Circuit>,
        key_distinguishing_factors: Vec<String>,
        supreme_court_likelihood: f32,
    },
    ArgumentStrength {
        strength_score: f32,
        supporting_authority: Vec<CaseSummary>,
        weak_points: Vec<String>,
        counter_arguments: Vec<String>,
    },
    DoctrinalTrend {
        interpretation_changes: Vec<InterpretationChange>,
        current_state: String,
        predicted_direction: Option<String>,
    },
}
```

---

# Part VI: Implementation

## 17. Recommended Legal Embedder Stack

### 17.1 Complete 15-Embedder Legal Stack

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    15-EMBEDDER LEGAL STACK                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  FOUNDATION (Topic Weight: 1.0)                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E1-LEGAL: voyage-law-2 (1024D, 16K context)                     │   │
│  │ • Primary legal semantic similarity                             │   │
│  │ • Model: Voyage AI API or self-hosted                          │   │
│  │ • VRAM: API-based or ~4GB self-hosted                          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  TEMPORAL (Topic Weight: 0.0, Post-Retrieval Only)                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E2: V_freshness (512D) - Recency decay                          │   │
│  │ E3: V_periodicity (512D) - Time patterns (legal filings)        │   │
│  │ E4: V_ordering (512D) - Document sequence                       │   │
│  │ • Computed, not learned                                         │   │
│  │ • VRAM: Negligible                                              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  LEGAL SEMANTIC ENHANCERS (Topic Weight: 1.0)                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E5-LEGAL: Legal Argument Embedder (768D×2)                      │   │
│  │ • Model: Legal-BERT fine-tuned on argument mining               │   │
│  │ • Captures: Premise → Conclusion, IRAC structure                │   │
│  │ • Asymmetric: e5_as_premise, e5_as_conclusion                   │   │
│  │ • VRAM: ~1GB                                                    │   │
│  │                                                                  │   │
│  │ E6-LEGAL: Legal Keyword Embedder (sparse, ~50K vocabulary)      │   │
│  │ • Model: SPLADE fine-tuned on legal terms                       │   │
│  │ • Vocabulary: Expanded with Latin phrases, legal jargon         │   │
│  │ • VRAM: ~500MB                                                  │   │
│  │                                                                  │   │
│  │ E7-LEGAL: Statute/Regulation Code (1536D)                       │   │
│  │ • Model: Qodo-Embed (for structured text) + legal fine-tuning   │   │
│  │ • Captures: Statutory hierarchy, regulatory patterns            │   │
│  │ • VRAM: ~3GB                                                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  RELATIONAL ENHANCERS (Topic Weight: 1.5 for citations)                │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E8-LEGAL: Citation Graph Embedder (384D×2)                      │   │
│  │ • Model: MiniLM + citation treatment fine-tuning                │   │
│  │ • Asymmetric: e8_as_citing, e8_as_cited                        │   │
│  │ • Captures: Citation direction and treatment                    │   │
│  │ • VRAM: ~500MB                                                  │   │
│  │                                                                  │   │
│  │ E11-LEGAL: Legal Entity Embedder (768D)                         │   │
│  │ • Model: KEPLER + legal NER fine-tuning                        │   │
│  │ • Extended taxonomy: Courts, parties, doctrines, citations      │   │
│  │ • VRAM: ~1GB                                                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  STRUCTURAL (Topic Weight: 0.5)                                        │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E9: V_robustness (1024D) - Noise-robust HDC                     │   │
│  │ E10: V_multimodality (768D) - Intent alignment                  │   │
│  │ • VRAM: ~1GB combined                                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  PRECISION (Reranking Only)                                            │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E12: ColBERT (128D/token) - Exact phrase matching               │   │
│  │ E13: SPLADE v3 (sparse) - Term expansion                        │   │
│  │ • VRAM: ~2GB combined                                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  NEW LEGAL-SPECIFIC (Topic Weight: 1.0)                                │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ E14: SAILER Legal Structure (768D)                              │   │
│  │ • Model: SAILER or DELTA (implement from paper)                 │   │
│  │ • Captures: FIRAC structure, fact patterns, holdings            │   │
│  │ • VRAM: ~1GB                                                    │   │
│  │                                                                  │   │
│  │ E15: Citation Network Embedding (768D)                          │   │
│  │ • Model: Node2Vec/GraphSAGE on citation graph                   │   │
│  │ • Captures: Citation pattern similarity                         │   │
│  │ • VRAM: ~500MB                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  TOTAL VRAM: ~15GB (fits on single RTX 5090 with headroom)             │
│  For billion-scale: Distribute across GPU cluster                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 17.2 Model Sources and Availability

| Embedder | Model | Source | License | Notes |
|----------|-------|--------|---------|-------|
| E1-LEGAL | voyage-law-2 | [Voyage AI](https://docs.voyageai.com) | Commercial | API or self-host |
| E5-LEGAL | Legal-BERT + argument | [HuggingFace](https://huggingface.co/nlpaueb/legal-bert-base-uncased) | Apache 2.0 | Fine-tune on argument data |
| E6-LEGAL | SPLADE + legal vocab | [HuggingFace](https://huggingface.co/naver/splade-cocondenser-ensembledistil) | Apache 2.0 | Expand vocabulary |
| E7-LEGAL | Qodo-Embed | Self-hosted | Commercial | Statutory structure |
| E8-LEGAL | MiniLM | [HuggingFace](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) | Apache 2.0 | Fine-tune on citations |
| E11-LEGAL | KEPLER | [HuggingFace](https://huggingface.co/facebook/kepler-large) | MIT | Fine-tune on legal entities |
| E14 | SAILER | [Paper](https://dl.acm.org/doi/10.1145/3539618.3591761) | Research | Implement from paper |
| E15 | Node2Vec | [PyG](https://pytorch-geometric.readthedocs.io/) | MIT | Train on citation graph |

---

## 18. Implementation Roadmap

### Phase 1: Foundation (Months 1-3)

```
Month 1: E1-LEGAL Integration
├── Week 1-2: Integrate voyage-law-2 API
├── Week 3: Benchmark against generic E1 on MLEB datasets
└── Week 4: Deploy to staging, A/B test

Month 2: E11-LEGAL + Citation Extraction
├── Week 1-2: Fine-tune Legal-BERT for legal NER
├── Week 3: Implement citation parser (all major formats)
└── Week 4: Build entity + citation extraction pipeline

Month 3: Basic K-NN Graph Construction
├── Week 1-2: Implement NN-Descent for E1-LEGAL space
├── Week 3: Create embedder_edges column family
└── Week 4: Add graph traversal to retrieval pipeline
```

### Phase 2: Legal Structure (Months 4-6)

```
Month 4: E14 (SAILER) Implementation
├── Week 1-2: Implement SAILER architecture
├── Week 3: Pre-train on legal document structure
└── Week 4: Integrate into TeleologicalArray

Month 5: E15 (Citation Network Embedding)
├── Week 1-2: Build citation graph from extracted citations
├── Week 3: Train Node2Vec/GraphSAGE on citation graph
└── Week 4: Integrate citation embeddings

Month 6: Multi-Relation Edge Types
├── Week 1-2: Implement legal edge type taxonomy
├── Week 3: Edge type detection pipeline
└── Week 4: Relation-filtered graph queries
```

### Phase 3: GNN Integration (Months 7-9)

```
Month 7: R-GCN for Legal
├── Week 1-2: Implement R-GCN layer
├── Week 3: Train on citation link prediction
└── Week 4: Integrate into retrieval pipeline

Month 8: Hyperbolic Hierarchy
├── Week 1-2: Implement Poincaré embeddings
├── Week 3: Embed statute/court hierarchy
└── Week 4: Hyperbolic navigation tools

Month 9: Contrastive Learning
├── Week 1-2: Implement cross-view contrastive loss
├── Week 3: Train on self-supervised legal signals
└── Week 4: Update edge weights with learned model
```

### Phase 4: Scale to Billion (Months 10-12)

```
Month 10: Distributed Architecture
├── Week 1-2: Implement jurisdiction-based sharding
├── Week 3: Deploy distributed index infrastructure
└── Week 4: Load 100M documents, stress test

Month 11: Full Corpus Ingestion
├── Week 1-2: Batch ingest US federal (50M docs)
├── Week 3: Batch ingest US state (200M docs)
└── Week 4: Batch ingest contracts (200M docs)

Month 12: Optimization + Launch
├── Week 1-2: Performance tuning (<200ms P95)
├── Week 3: Insight extraction system
└── Week 4: Production launch
```

---

## 19. References

### Legal Embedding Models

1. [voyage-law-2 - Domain-Specific Embeddings and Retrieval: Legal Edition](https://blog.voyageai.com/2024/04/15/domain-specific-embeddings-and-retrieval-legal-edition-voyage-law-2/)
2. [Legal-BERT - nlpaueb/legal-bert-base-uncased](https://huggingface.co/nlpaueb/legal-bert-base-uncased)
3. [The Massive Legal Embedding Benchmark (MLEB)](https://www.researchgate.net/publication/396789828_The_Massive_Legal_Embedding_Benchmark_MLEB)
4. [SAILER: Structure-aware Pre-trained Language Model for Legal Case Retrieval (SIGIR 2023)](https://dl.acm.org/doi/10.1145/3539618.3591761)
5. [DELTA: Pre-train a Discriminative Encoder for Legal Case Retrieval (AAAI 2025)](https://arxiv.org/html/2403.18435)
6. [Unlocking Legal Knowledge with Multi-Layered Embedding-Based Retrieval](https://arxiv.org/html/2411.07739v1)

### Legal Citation Networks & GNNs

7. [Joint Legal Citation Prediction using Heterogeneous Graph Enrichment](https://arxiv.org/html/2506.22165v1)
8. [LeCNet: A Legal Citation Network Benchmark Dataset](https://aclanthology.org/2025.justnlp-main.4.pdf)
9. [R-GCN: Relational Graph Convolutional Networks](https://arxiv.org/abs/1703.06103)
10. [GAT: Graph Attention Networks](https://petar-v.com/GAT/)

### GNN & Graph Learning

11. [Enhancing KGE with GNNs (2025)](https://link.springer.com/article/10.1007/s10115-025-02619-8)
12. [Disentangled Multi-view GNN (DMGNN)](https://www.sciencedirect.com/science/article/abs/pii/S1568494625009160)
13. [HeCo: Self-supervised Heterogeneous GNN](https://dl.acm.org/doi/10.1145/3447548.3467415)
14. [H2GNN: Hyperbolic Hypergraph Neural Networks](https://arxiv.org/abs/2412.12158)
15. [NN-Descent Algorithm](https://www.cs.princeton.edu/cass/papers/www11.pdf)

### Legal NLP & Analysis

16. [Legal Argument Mining: Recent Trends and Open Challenges](https://ceur-ws.org/Vol-4089/paper1.pdf)
17. [ContractNLI: A Dataset for Document-level NLI for Contracts](https://stanfordnlp.github.io/contract-nli/)
18. [Named Entity Recognition in Legal Documents](https://www.researchgate.net/publication/391907393_Named_Entity_Recognition_NER_for_Legal_Document_Analysis)
19. [Similar Cases Recommendation using Legal Knowledge Graphs](https://arxiv.org/html/2107.04771v2)
20. [An Ontology-Driven Graph RAG for Legal Norms](https://arxiv.org/html/2505.00039v5)

### Legal RAG Systems

21. [Legal Document RAG: Multi-Graph Multi-Agent Recursive Retrieval](https://medium.com/enterprise-rag/legal-document-rag-multi-graph-multi-agent-recursive-retrieval-through-legal-clauses-c90e073e0052)
22. [Towards Reliable Retrieval in RAG Systems for Large Legal Datasets](https://arxiv.org/html/2510.06999v1)
23. [Microsoft GraphRAG](https://microsoft.github.io/graphrag/)
24. [RAG: Towards a Promising LLM Architecture for Legal Work (Harvard JOLT)](https://jolt.law.harvard.edu/digest/retrieval-augmented-generation-rag-towards-a-promising-llm-architecture-for-legal-work)

---

## Appendix A: Storage Calculations for 1B Documents

| Component | Per Doc | 1B Documents | Compressed |
|-----------|---------|--------------|------------|
| 15 Embeddings (full) | 85KB | 85TB | 25TB (int8) |
| K-NN Edges (15 graphs, k=20) | 6KB | 6TB | 2TB |
| Citation Edges | 2KB | 2TB | 1TB |
| Typed Edge Metadata | 1KB | 1TB | 500GB |
| Inverted Indexes | 12KB | 12TB | 5TB |
| Document Metadata | 500B | 500GB | 200GB |
| **TOTAL** | ~105KB | ~105TB | **~35TB** |

## Appendix B: Latency Targets

| Operation | Target | At 1B Docs |
|-----------|--------|------------|
| Sparse recall (E13) | <10ms | Inverted index |
| Dense ANN (E1-LEGAL 128D) | <20ms | HNSW on GPU |
| Graph expansion (1-hop) | <30ms | Pre-computed edges |
| Multi-space RRF | <50ms | Parallel scoring |
| R-GCN inference | <50ms | Subgraph of 200 |
| ColBERT rerank | <30ms | 50 candidates |
| **Total P95** | **<200ms** | Full pipeline |

## Appendix C: Implementation Checklist

### Phase 1: Foundation
- [ ] Integrate voyage-law-2 as E1-LEGAL
- [ ] Fine-tune Legal-BERT for legal NER (E11-LEGAL)
- [ ] Implement citation parser
- [ ] Build E1-LEGAL K-NN graph (NN-Descent)
- [ ] Add `embedder_edges` column family

### Phase 2: Legal Structure
- [ ] Implement SAILER architecture (E14)
- [ ] Train Node2Vec on citation graph (E15)
- [ ] Define legal edge type taxonomy
- [ ] Implement edge type detection pipeline
- [ ] Create `typed_edges` column family

### Phase 3: GNN Integration
- [ ] Implement R-GCN for legal KG
- [ ] Implement Poincaré embeddings for hierarchy
- [ ] Implement cross-view contrastive loss
- [ ] Train on self-supervised legal signals
- [ ] Integrate GNN inference into pipeline

### Phase 4: Scale
- [ ] Implement jurisdiction-based sharding
- [ ] Deploy distributed index infrastructure
- [ ] Ingest 1B+ documents
- [ ] Optimize to <200ms P95
- [ ] Deploy insight extraction system

---

*This document provides a comprehensive architecture for building a billion-scale legal document analysis system using multi-embedder knowledge graphs with GNN-enhanced reasoning, enabling insights that single-embedder systems cannot discover.*

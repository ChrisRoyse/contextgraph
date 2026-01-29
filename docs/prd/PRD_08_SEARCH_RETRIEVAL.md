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

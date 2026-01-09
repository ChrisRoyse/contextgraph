# TASK-LOGIC-005: Single Embedder Search

```xml
<task_spec id="TASK-LOGIC-005" version="1.0">
<metadata>
  <title>Implement Single Embedder Search Strategy</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <requirement_ref>REQ-SEARCH-SINGLE-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
First search strategy implementation. Single embedder search queries one of the
13 per-embedder indices directly, enabling fast targeted retrieval when the
relevant embedding space is known.
</context>

<objective>
Implement SingleEmbedderSearch that queries a single embedder index and returns
ranked results with similarity scores.
</objective>

<rationale>
Single embedder search is useful when:
1. Query intent maps clearly to one space (e.g., code queries to E7)
2. Speed is critical (one index vs. 13)
3. Testing/debugging specific embedders
4. First stage of multi-stage retrieval
</rationale>

<input_context_files>
  <file purpose="comparator">crates/context-graph-core/src/teleology/comparator.rs</file>
  <file purpose="index_trait">crates/context-graph-storage/src/teleological/index.rs</file>
  <file purpose="store_trait">crates/context-graph-storage/src/teleological/store.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-004 complete (TeleologicalComparator exists)</check>
  <check>EmbedderIndex trait defined</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create SingleEmbedderSearch struct</item>
    <item>Search on single embedder index</item>
    <item>Support top-k retrieval with similarity threshold</item>
    <item>Validate query embedding matches expected type</item>
    <item>Return ranked results with scores</item>
  </in_scope>
  <out_of_scope>
    <item>Multi-embedder search (TASK-LOGIC-006, 007)</item>
    <item>5-stage pipeline (TASK-LOGIC-008)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/search/single.rs">
      use crate::teleological::store::{SearchResult, SearchFilter};
      use crate::teleological::index::EmbedderIndex;
      use context_graph_core::teleology::array::TeleologicalArray;
      use context_graph_core::teleology::embedder::Embedder;

      /// Single embedder search strategy.
      pub struct SingleEmbedderSearch<I: EmbedderIndex> {
          index: I,
          embedder: Embedder,
      }

      impl<I: EmbedderIndex> SingleEmbedderSearch<I> {
          pub fn new(index: I) -> Self;

          /// Search using a query array's specific embedder.
          pub async fn search(
              &self,
              query: &TeleologicalArray,
              limit: usize,
              threshold: Option<f32>,
          ) -> Result<Vec<SearchResult>, SearchError>;

          /// Search using a raw embedding (for query-time embedding).
          pub async fn search_raw(
              &self,
              query_embedding: &EmbedderOutput,
              limit: usize,
              threshold: Option<f32>,
          ) -> Result<Vec<SearchResult>, SearchError>;

          /// Get the embedder this search uses.
          pub fn embedder(&self) -> Embedder;
      }

      #[derive(Debug, thiserror::Error)]
      pub enum SearchError {
          #[error("Query embedding type mismatch for embedder {embedder:?}")]
          TypeMismatch { embedder: Embedder },
          #[error("Index error: {0}")]
          Index(String),
          #[error("Empty query embedding")]
          EmptyQuery,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Query embedding must match embedder type</constraint>
    <constraint>Results sorted by similarity descending</constraint>
    <constraint>Threshold filtering applied if specified</constraint>
    <constraint>Limit enforced</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-storage search::single</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/search/single.rs

use async_trait::async_trait;
use thiserror::Error;

use crate::teleological::store::{SearchResult, StorageResult};
use crate::teleological::index::{EmbedderIndex, IndexSearchResult};
use context_graph_core::teleology::array::{TeleologicalArray, EmbedderOutput};
use context_graph_core::teleology::embedder::Embedder;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("Query embedding type mismatch for embedder {embedder:?}")]
    TypeMismatch { embedder: Embedder },
    #[error("Index error: {0}")]
    Index(String),
    #[error("Empty query embedding")]
    EmptyQuery,
}

pub struct SingleEmbedderSearch<I: EmbedderIndex> {
    index: I,
    embedder: Embedder,
}

impl<I: EmbedderIndex> SingleEmbedderSearch<I> {
    pub fn new(index: I) -> Self {
        let embedder = index.embedder();
        Self { index, embedder }
    }

    pub fn embedder(&self) -> Embedder {
        self.embedder
    }

    /// Search using query array's embedder.
    pub async fn search(
        &self,
        query: &TeleologicalArray,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let query_embedding = query.get(self.embedder);
        self.search_raw(query_embedding, limit, threshold).await
    }

    /// Search using raw embedding.
    pub async fn search_raw(
        &self,
        query_embedding: &EmbedderOutput,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Validate embedding type
        self.validate_embedding(query_embedding)?;

        // Search the index
        let index_results = self.index
            .search(query_embedding, limit, threshold)
            .await
            .map_err(|e| SearchError::Index(e.to_string()))?;

        // Convert to SearchResults
        // Note: In a real implementation, we'd fetch full arrays from store
        let results = index_results
            .into_iter()
            .map(|ir| self.index_result_to_search_result(ir))
            .collect();

        Ok(results)
    }

    fn validate_embedding(&self, embedding: &EmbedderOutput) -> Result<(), SearchError> {
        match embedding {
            EmbedderOutput::Pending => Err(SearchError::EmptyQuery),
            EmbedderOutput::Failed(_) => Err(SearchError::EmptyQuery),
            EmbedderOutput::Dense(_) => {
                // Check embedder expects dense
                match self.embedder.expected_dims() {
                    context_graph_core::teleology::embedder::EmbedderDims::Dense(_) => Ok(()),
                    _ => Err(SearchError::TypeMismatch { embedder: self.embedder }),
                }
            }
            EmbedderOutput::Sparse(_) => {
                match self.embedder.expected_dims() {
                    context_graph_core::teleology::embedder::EmbedderDims::Sparse { .. } => Ok(()),
                    _ => Err(SearchError::TypeMismatch { embedder: self.embedder }),
                }
            }
            EmbedderOutput::TokenLevel(_) => {
                match self.embedder.expected_dims() {
                    context_graph_core::teleology::embedder::EmbedderDims::TokenLevel { .. } => Ok(()),
                    _ => Err(SearchError::TypeMismatch { embedder: self.embedder }),
                }
            }
            EmbedderOutput::Binary(_) => {
                match self.embedder.expected_dims() {
                    context_graph_core::teleology::embedder::EmbedderDims::Binary { .. } => Ok(()),
                    _ => Err(SearchError::TypeMismatch { embedder: self.embedder }),
                }
            }
        }
    }

    fn index_result_to_search_result(&self, ir: IndexSearchResult) -> SearchResult {
        // Create per-embedder scores array with only this embedder populated
        let mut per_embedder_scores = [None; 13];
        per_embedder_scores[self.embedder.index()] = Some(ir.score);

        SearchResult {
            array: TeleologicalArray::new(ir.id), // Placeholder - needs store lookup
            similarity: ir.score,
            per_embedder_scores,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock index for testing
    struct MockIndex {
        embedder: Embedder,
        results: Vec<IndexSearchResult>,
    }

    #[async_trait::async_trait]
    impl EmbedderIndex for MockIndex {
        fn embedder(&self) -> Embedder { self.embedder }
        // ... implement other methods
    }

    #[tokio::test]
    async fn test_search_returns_ranked_results() {
        // Test implementation
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/search/single.rs">
    Single embedder search implementation
  </file>
  <file path="crates/context-graph-storage/src/teleological/search/mod.rs">
    Search module definition
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/mod.rs">
    Add: pub mod search;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Results sorted by similarity descending</criterion>
  <criterion>Type mismatch returns error</criterion>
  <criterion>Threshold filtering works</criterion>
  <criterion>Limit is enforced</criterion>
  <criterion>Per-embedder score populated for searched embedder</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage search::single -- --nocapture</command>
</test_commands>
</task_spec>
```
